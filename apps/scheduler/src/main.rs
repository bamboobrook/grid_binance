use axum::{http::header, response::IntoResponse, routing::get, Router};
use chrono::{DateTime, Utc};
use scheduler::jobs::symbol_sync::{spawn_hourly_symbol_sync_job, SymbolSyncRuntimeState};
use serde::{Deserialize, Serialize};
use shared_binance::{
    sync_symbol_metadata, BinanceClient, CredentialCipher, CredentialValidationRequest,
    ExchangeCredentialCheck, SymbolMetadata,
};
use shared_db::{SharedDb, UserExchangeAccountRecord, UserExchangeSymbolRecord};
use std::{
    io::{Error as IoError, ErrorKind},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::net::TcpListener;

const DEFAULT_PORT: u16 = 8082;
const SERVICE_NAME: &str = "scheduler";
const DEFAULT_SYMBOL_SYNC_INTERVAL_SECS: u64 = 60 * 60;
const BINANCE_EXCHANGE: &str = "binance";
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = required_env("DATABASE_URL")?;
    let redis_url = required_env("REDIS_URL")?;
    let _cipher = credential_cipher()?;
    let db = SharedDb::connect(&database_url, &redis_url)?;

    let symbol_sync_state = Arc::new(Mutex::new(SymbolSyncRuntimeState::default()));
    let state_for_job = symbol_sync_state.clone();
    let db_for_job = db.clone();
    let interval = Duration::from_secs(configured_symbol_sync_interval_secs());
    let _symbol_sync_handle = spawn_hourly_symbol_sync_job(interval, state_for_job);
    std::thread::spawn(move || loop {
        let result = run_persistent_symbol_sync_once(&db_for_job);
        if result.failed_accounts > 0 {
            eprintln!(
                "scheduler persistent symbol sync completed with {} refreshed / {} failed",
                result.refreshed_accounts, result.failed_accounts
            );
        }
        std::thread::sleep(interval);
    });

    let listener = TcpListener::bind(("0.0.0.0", configured_port(DEFAULT_PORT))).await?;
    let app = Router::new().route("/healthz", get(healthz));

    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        health_payload(SERVICE_NAME),
    )
}

fn configured_port(default_port: u16) -> u16 {
    parse_port(std::env::var("PORT").ok(), default_port)
}

fn configured_symbol_sync_interval_secs() -> u64 {
    std::env::var("SYMBOL_SYNC_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_SYMBOL_SYNC_INTERVAL_SECS)
}

fn required_env(name: &str) -> Result<String, IoError> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| IoError::new(ErrorKind::InvalidInput, format!("{name} is required")))
}

fn parse_port(value: Option<String>, default_port: u16) -> u16 {
    value
        .and_then(|port| port.parse().ok())
        .unwrap_or(default_port)
}

fn health_payload(service_name: &str) -> String {
    format!(
        "# HELP service_up Service health probe status.\n# TYPE service_up gauge\nservice_up{{service=\"{service_name}\"}} 1\n"
    )
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct PersistentSymbolSyncResult {
    refreshed_accounts: usize,
    failed_accounts: usize,
}

fn run_persistent_symbol_sync_once(db: &SharedDb) -> PersistentSymbolSyncResult {
    let accounts = db
        .list_active_exchange_accounts(BINANCE_EXCHANGE)
        .unwrap_or_else(|error| {
            eprintln!("scheduler persistent symbol sync failed to list accounts: {error}");
            Vec::new()
        });
    let mut result = PersistentSymbolSyncResult::default();

    for account in accounts {
        match refresh_account_symbols(db, &account) {
            Ok(()) => result.refreshed_accounts += 1,
            Err(error) => {
                result.failed_accounts += 1;
                eprintln!(
                    "scheduler persistent symbol sync failed for {}: {}",
                    account.user_email, error
                );
            }
        }
    }

    result
}

fn refresh_account_symbols(
    db: &SharedDb,
    account: &UserExchangeAccountRecord,
) -> Result<(), IoError> {
    let Some(credentials) = db
        .find_exchange_credentials(&account.user_email, BINANCE_EXCHANGE)
        .map_err(storage_error)?
    else {
        return Err(IoError::new(
            ErrorKind::NotFound,
            "exchange credentials not found",
        ));
    };

    let metadata = parse_account_metadata(&account.metadata)?;
    let (api_key, api_secret) = credential_cipher()?
        .decrypt(&credentials.encrypted_secret)
        .map_err(|error| IoError::new(ErrorKind::InvalidData, error.to_string()))?;
    let validation_request =
        CredentialValidationRequest::new(metadata.expected_hedge_mode, &metadata.selected_markets)
            .map_err(validation_error)?;
    let client = BinanceClient::new(api_key, api_secret);
    let check = client.check_credentials_for(&validation_request);
    let symbols = sync_symbol_metadata(&client, &check);
    let synced_at = Utc::now();
    let symbol_counts = ExchangeSymbolCountsDto::from_symbols(&symbols);

    let symbol_records = symbols
        .into_iter()
        .map(|symbol| to_symbol_record(&account.user_email, symbol, synced_at))
        .collect::<Vec<_>>();

    db.refresh_exchange_account_bundle(
        &UserExchangeAccountRecord {
            metadata: serde_json::to_value(StoredExchangeMetadata {
                connection_status: check.connection_status().to_owned(),
                sync_status: "success".to_owned(),
                last_synced_at: Some(synced_at.to_rfc3339()),
                expected_hedge_mode: metadata.expected_hedge_mode,
                selected_markets: check.selected_markets.clone(),
                validation: StoredValidationSnapshot::from(&check),
                symbol_counts,
            })
            .map_err(serde_error)?,
            checked_at: Some(synced_at),
            ..account.clone()
        },
        &symbol_records,
    )
    .map_err(storage_error)?;

    Ok(())
}

fn credential_cipher() -> Result<CredentialCipher, IoError> {
    CredentialCipher::from_env("EXCHANGE_CREDENTIALS_MASTER_KEY")
        .map_err(|error| IoError::new(ErrorKind::InvalidInput, error.to_string()))
}

fn parse_account_metadata(value: &serde_json::Value) -> Result<StoredExchangeMetadata, IoError> {
    serde_json::from_value(value.clone()).map_err(serde_error)
}

fn serde_error(error: serde_json::Error) -> IoError {
    IoError::new(ErrorKind::InvalidData, error.to_string())
}

fn storage_error(error: shared_db::SharedDbError) -> IoError {
    IoError::new(ErrorKind::Other, error.to_string())
}

fn validation_error(error: shared_binance::CredentialValidationError) -> IoError {
    IoError::new(ErrorKind::InvalidInput, error.to_string())
}

fn to_symbol_record(
    user_email: &str,
    symbol: SymbolMetadata,
    synced_at: DateTime<Utc>,
) -> UserExchangeSymbolRecord {
    UserExchangeSymbolRecord {
        user_email: user_email.to_owned(),
        exchange: BINANCE_EXCHANGE.to_owned(),
        market: symbol.market.clone(),
        symbol: symbol.symbol.clone(),
        status: symbol.status.clone(),
        base_asset: symbol.base_asset.clone(),
        quote_asset: symbol.quote_asset.clone(),
        price_precision: symbol.price_precision as i32,
        quantity_precision: symbol.quantity_precision as i32,
        min_quantity: symbol.filters.min_quantity.clone(),
        min_notional: symbol.filters.min_notional.clone(),
        keywords: symbol.keywords.clone(),
        metadata: serde_json::to_value(&symbol).unwrap_or_else(|_| serde_json::json!({})),
        synced_at,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoredExchangeMetadata {
    connection_status: String,
    sync_status: String,
    last_synced_at: Option<String>,
    expected_hedge_mode: bool,
    selected_markets: Vec<String>,
    validation: StoredValidationSnapshot,
    symbol_counts: ExchangeSymbolCountsDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoredValidationSnapshot {
    api_connectivity_ok: bool,
    timestamp_in_sync: bool,
    can_read_spot: bool,
    can_read_usdm: bool,
    can_read_coinm: bool,
    hedge_mode_ok: bool,
    permissions_ok: bool,
    market_access_ok: bool,
}

impl StoredValidationSnapshot {
    fn from(check: &ExchangeCredentialCheck) -> Self {
        Self {
            api_connectivity_ok: check.api_connectivity_ok,
            timestamp_in_sync: check.timestamp_in_sync,
            can_read_spot: check.can_read_spot,
            can_read_usdm: check.can_read_usdm,
            can_read_coinm: check.can_read_coinm,
            hedge_mode_ok: check.hedge_mode_ok,
            permissions_ok: check.permissions_ok,
            market_access_ok: check.market_access_ok,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ExchangeSymbolCountsDto {
    spot: usize,
    usdm: usize,
    coinm: usize,
}

impl ExchangeSymbolCountsDto {
    fn from_symbols(symbols: &[SymbolMetadata]) -> Self {
        let mut counts = Self::default();
        for symbol in symbols {
            match symbol.market.as_str() {
                "spot" => counts.spot += 1,
                "usdm" => counts.usdm += 1,
                "coinm" => counts.coinm += 1,
                _ => {}
            }
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::{
        configured_symbol_sync_interval_secs, health_payload, parse_port, required_env,
        run_persistent_symbol_sync_once, DEFAULT_PORT, DEFAULT_SYMBOL_SYNC_INTERVAL_SECS,
        SERVICE_NAME,
    };
    use chrono::Utc;
    use serde_json::json;
    use shared_binance::{mask_api_key, CredentialCipher};
    use shared_db::{SharedDb, UserExchangeAccountRecord, UserExchangeCredentialRecord};
    use std::sync::{Mutex, OnceLock};

    #[test]
    fn health_payload_mentions_service_name() {
        let payload = health_payload(SERVICE_NAME);

        assert!(payload.contains("service_up"));
        assert!(payload.contains("scheduler"));
    }

    #[test]
    fn parse_port_falls_back_when_value_is_missing_or_invalid() {
        assert_eq!(parse_port(None, DEFAULT_PORT), DEFAULT_PORT);
        assert_eq!(
            parse_port(Some("not-a-port".to_string()), DEFAULT_PORT),
            DEFAULT_PORT
        );
    }

    #[test]
    fn required_env_requires_runtime_storage_urls() {
        std::env::remove_var("REDIS_URL");
        assert!(required_env("REDIS_URL").is_err());

        std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379/0");
        assert!(required_env("REDIS_URL").is_ok());
        std::env::remove_var("REDIS_URL");
    }

    #[test]
    fn symbol_sync_interval_uses_hourly_default_and_accepts_override() {
        std::env::remove_var("SYMBOL_SYNC_INTERVAL_SECS");
        assert_eq!(
            configured_symbol_sync_interval_secs(),
            DEFAULT_SYMBOL_SYNC_INTERVAL_SECS
        );

        std::env::set_var("SYMBOL_SYNC_INTERVAL_SECS", "90");
        assert_eq!(configured_symbol_sync_interval_secs(), 90);
        std::env::remove_var("SYMBOL_SYNC_INTERVAL_SECS");
    }

    #[test]
    fn persistent_symbol_sync_updates_active_exchange_accounts_and_symbols() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var(
            "EXCHANGE_CREDENTIALS_MASTER_KEY",
            "scheduler-persistent-sync-test-key",
        );
        let db = SharedDb::ephemeral().expect("ephemeral db");
        let now = Utc::now();
        let cipher = CredentialCipher::new("scheduler-persistent-sync-test-key");

        db.upsert_exchange_account(&UserExchangeAccountRecord {
            user_email: "sync@example.com".to_owned(),
            exchange: "binance".to_owned(),
            account_label: "Binance".to_owned(),
            market_scope: "spot,coinm".to_owned(),
            is_active: true,
            checked_at: Some(now),
            metadata: json!({
                "connection_status": "healthy",
                "sync_status": "success",
                "last_synced_at": "2026-04-01T00:00:00Z",
                "expected_hedge_mode": true,
                "selected_markets": ["spot", "coinm"],
                "validation": {
                    "api_connectivity_ok": true,
                    "timestamp_in_sync": true,
                    "can_read_spot": true,
                    "can_read_usdm": false,
                    "can_read_coinm": false,
                    "hedge_mode_ok": true,
                    "permissions_ok": true,
                    "market_access_ok": false
                },
                "symbol_counts": {
                    "spot": 1,
                    "usdm": 0,
                    "coinm": 0
                }
            }),
        })
        .expect("account");

        db.upsert_exchange_credentials(&UserExchangeCredentialRecord {
            user_email: "sync@example.com".to_owned(),
            exchange: "binance".to_owned(),
            api_key_masked: mask_api_key("demo-key-1234"),
            encrypted_secret: cipher
                .encrypt("demo-key-1234", "demo-secret")
                .expect("encrypt"),
        })
        .expect("credentials");

        let refreshed = run_persistent_symbol_sync_once(&db);
        assert_eq!(refreshed.refreshed_accounts, 1);
        assert_eq!(refreshed.failed_accounts, 0);

        let account = db
            .find_exchange_account("sync@example.com", "binance")
            .expect("find account")
            .expect("account exists");
        let metadata = account.metadata;
        assert_eq!(metadata["selected_markets"], json!(["spot", "coinm"]));
        assert_eq!(metadata["validation"]["can_read_spot"], true);
        assert_eq!(metadata["validation"]["can_read_coinm"], true);
        assert_eq!(metadata["validation"]["market_access_ok"], true);
        assert_eq!(metadata["symbol_counts"]["spot"], 2);
        assert_eq!(metadata["symbol_counts"]["usdm"], 0);
        assert_eq!(metadata["symbol_counts"]["coinm"], 2);
        assert!(metadata["last_synced_at"].is_string());

        let symbols = db
            .list_exchange_symbols("sync@example.com", "binance")
            .expect("symbols");
        assert_eq!(symbols.len(), 4);
    }

    #[test]
    fn persistent_symbol_sync_continues_past_bad_accounts() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var(
            "EXCHANGE_CREDENTIALS_MASTER_KEY",
            "scheduler-persistent-sync-test-key",
        );
        let db = SharedDb::ephemeral().expect("ephemeral db");
        let now = Utc::now();
        let cipher = CredentialCipher::new("scheduler-persistent-sync-test-key");

        db.upsert_exchange_account(&UserExchangeAccountRecord {
            user_email: "broken@example.com".to_owned(),
            exchange: "binance".to_owned(),
            account_label: "Broken".to_owned(),
            market_scope: "spot".to_owned(),
            is_active: true,
            checked_at: Some(now),
            metadata: json!("not-an-object"),
        })
        .expect("broken account");
        db.upsert_exchange_credentials(&UserExchangeCredentialRecord {
            user_email: "broken@example.com".to_owned(),
            exchange: "binance".to_owned(),
            api_key_masked: mask_api_key("broken-key-1234"),
            encrypted_secret: cipher
                .encrypt("broken-key-1234", "broken-secret")
                .expect("encrypt"),
        })
        .expect("broken credentials");

        db.upsert_exchange_account(&UserExchangeAccountRecord {
            user_email: "healthy@example.com".to_owned(),
            exchange: "binance".to_owned(),
            account_label: "Healthy".to_owned(),
            market_scope: "spot,coinm".to_owned(),
            is_active: true,
            checked_at: Some(now),
            metadata: json!({
                "connection_status": "healthy",
                "sync_status": "success",
                "last_synced_at": "2026-04-01T00:00:00Z",
                "expected_hedge_mode": true,
                "selected_markets": ["spot", "coinm"],
                "validation": {
                    "api_connectivity_ok": true,
                    "timestamp_in_sync": true,
                    "can_read_spot": true,
                    "can_read_usdm": false,
                    "can_read_coinm": false,
                    "hedge_mode_ok": true,
                    "permissions_ok": true,
                    "market_access_ok": false
                },
                "symbol_counts": {
                    "spot": 1,
                    "usdm": 0,
                    "coinm": 0
                }
            }),
        })
        .expect("healthy account");
        db.upsert_exchange_credentials(&UserExchangeCredentialRecord {
            user_email: "healthy@example.com".to_owned(),
            exchange: "binance".to_owned(),
            api_key_masked: mask_api_key("healthy-key-1234"),
            encrypted_secret: cipher
                .encrypt("healthy-key-1234", "healthy-secret")
                .expect("encrypt"),
        })
        .expect("healthy credentials");

        let result = run_persistent_symbol_sync_once(&db);
        assert_eq!(result.refreshed_accounts, 1);
        assert_eq!(result.failed_accounts, 1);

        let healthy_symbols = db
            .list_exchange_symbols("healthy@example.com", "binance")
            .expect("healthy symbols");
        assert_eq!(healthy_symbols.len(), 4);
        let broken_symbols = db
            .list_exchange_symbols("broken@example.com", "binance")
            .expect("broken symbols");
        assert_eq!(broken_symbols.len(), 0);
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }
}
