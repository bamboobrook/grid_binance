use axum::{extract::State, http::header, response::IntoResponse, routing::get, Router};
use chrono::{DateTime, Utc};
use scheduler::jobs::{
    membership_grace::run_membership_grace_once,
    reminders::run_membership_reminders_once,
    symbol_sync::{spawn_hourly_symbol_sync_job, SymbolSyncRuntimeState},
};
use serde::{Deserialize, Serialize};
use shared_binance::{
    sync_symbol_metadata, BinanceClient, CredentialCipher, CredentialValidationRequest,
    ExchangeCredentialCheck, SymbolMetadata,
};
use shared_db::{
    AccountProfitSnapshotRecord, ExchangeWalletSnapshotRecord, SharedDb, UserExchangeAccountRecord,
    UserExchangeSymbolRecord,
};
use std::{
    collections::BTreeMap,
    io::{Error as IoError, ErrorKind},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::net::TcpListener;

const DEFAULT_PORT: u16 = 8082;
const SERVICE_NAME: &str = "scheduler";
const DEFAULT_SYMBOL_SYNC_INTERVAL_SECS: u64 = 60 * 60;
const DEFAULT_MEMBERSHIP_GRACE_INTERVAL_SECS: u64 = 60;
const DEFAULT_SNAPSHOT_SYNC_INTERVAL_SECS: u64 = 300;
const DEFAULT_REMINDER_INTERVAL_SECS: u64 = 300;
const DEFAULT_REMINDER_LOOKAHEAD_HOURS: i64 = 24;
const BINANCE_EXCHANGE: &str = "binance";

#[derive(Debug, Clone, Default)]
struct MembershipGraceMetrics {
    failures_total: u64,
    paused_total: u64,
    runs_total: u64,
}

#[derive(Clone)]
struct SchedulerState {
    grace_metrics: Arc<Mutex<MembershipGraceMetrics>>,
    symbol_sync_state: Arc<Mutex<SymbolSyncRuntimeState>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = required_env("DATABASE_URL")?;
    let redis_url = required_env("REDIS_URL")?;
    let _cipher = credential_cipher()?;
    let db = SharedDb::connect(&database_url, &redis_url)?;

    let symbol_sync_state = Arc::new(Mutex::new(SymbolSyncRuntimeState::default()));
    let state_for_job = symbol_sync_state.clone();
    let db_for_symbol_sync = db.clone();
    let grace_db = db.clone();
    let reminder_db = db.clone();
    let snapshot_db = db.clone();
    let grace_metrics = Arc::new(Mutex::new(MembershipGraceMetrics::default()));
    let grace_metrics_for_loop = grace_metrics.clone();

    let symbol_sync_interval = Duration::from_secs(configured_symbol_sync_interval_secs());
    let grace_interval = Duration::from_secs(configured_membership_grace_interval_secs());
    let snapshot_interval = Duration::from_secs(configured_snapshot_sync_interval_secs());
    let reminder_interval = Duration::from_secs(configured_reminder_interval_secs());

    let _symbol_sync_handle = spawn_hourly_symbol_sync_job(symbol_sync_interval, state_for_job);
    std::thread::spawn(move || loop {
        let result = run_persistent_symbol_sync_once(&db_for_symbol_sync);
        if result.failed_accounts > 0 {
            eprintln!(
                "scheduler persistent symbol sync completed with {} refreshed / {} failed",
                result.refreshed_accounts, result.failed_accounts
            );
        }
        std::thread::sleep(symbol_sync_interval);
    });
    std::thread::spawn(move || loop {
        let result = run_persistent_snapshot_sync_once(&snapshot_db);
        if result.failed_accounts > 0 {
            eprintln!(
                "scheduler persistent snapshot sync completed with {} refreshed / {} failed",
                result.refreshed_accounts, result.failed_accounts
            );
        }
        std::thread::sleep(snapshot_interval);
    });
    std::thread::spawn(move || loop {
        if let Err(error) = run_membership_reminders_once(&reminder_db, Utc::now(), chrono::Duration::hours(configured_reminder_lookahead_hours())) {
            eprintln!("scheduler membership reminder job failed: {error}");
        }
        std::thread::sleep(reminder_interval);
    });
    std::thread::spawn(move || loop {
        let mut metrics = grace_metrics_for_loop.lock().expect("grace metrics poisoned");
        metrics.runs_total += 1;
        drop(metrics);
        match run_membership_grace_once(&grace_db, Utc::now()) {
            Ok(paused) if paused > 0 => {
                let mut metrics = grace_metrics_for_loop.lock().expect("grace metrics poisoned");
                metrics.paused_total += paused as u64;
                eprintln!("scheduler membership grace paused {} strategies", paused);
            }
            Ok(_) => {}
            Err(error) => {
                let mut metrics = grace_metrics_for_loop.lock().expect("grace metrics poisoned");
                metrics.failures_total += 1;
                eprintln!("scheduler membership grace job failed: {error}");
            }
        }
        std::thread::sleep(grace_interval);
    });

    let listener = TcpListener::bind(("0.0.0.0", configured_port(DEFAULT_PORT))).await?;
    let app = Router::new()
        .route("/healthz", get(healthz))
        .with_state(SchedulerState {
            grace_metrics,
            symbol_sync_state,
        });

    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz(State(state): State<SchedulerState>) -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        health_payload(SERVICE_NAME, &state.symbol_sync_state, &state.grace_metrics),
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

fn configured_snapshot_sync_interval_secs() -> u64 {
    std::env::var("SNAPSHOT_SYNC_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_SNAPSHOT_SYNC_INTERVAL_SECS)
}

fn configured_reminder_interval_secs() -> u64 {
    std::env::var("REMINDER_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_REMINDER_INTERVAL_SECS)
}

fn configured_reminder_lookahead_hours() -> i64 {
    std::env::var("REMINDER_LOOKAHEAD_HOURS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_REMINDER_LOOKAHEAD_HOURS)
}

fn configured_membership_grace_interval_secs() -> u64 {
    std::env::var("MEMBERSHIP_GRACE_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MEMBERSHIP_GRACE_INTERVAL_SECS)
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

fn health_payload(
    service_name: &str,
    symbol_sync_state: &Arc<Mutex<SymbolSyncRuntimeState>>,
    grace_metrics: &Arc<Mutex<MembershipGraceMetrics>>,
) -> String {
    let symbol = symbol_sync_state.lock().expect("symbol sync metrics poisoned");
    let grace = grace_metrics.lock().expect("grace metrics poisoned");
    format!(
        "# HELP service_up Service health probe status.\n# TYPE service_up gauge\nservice_up{{service=\"{service_name}\"}} 1\n# HELP scheduler_symbol_sync_runs_total Public symbol sync executions.\n# TYPE scheduler_symbol_sync_runs_total counter\nscheduler_symbol_sync_runs_total {sync_runs}\n# HELP scheduler_symbol_sync_last_synced_symbols Last synced public symbol count.\n# TYPE scheduler_symbol_sync_last_synced_symbols gauge\nscheduler_symbol_sync_last_synced_symbols {symbols}\n# HELP scheduler_membership_grace_runs_total Membership grace loop executions.\n# TYPE scheduler_membership_grace_runs_total counter\nscheduler_membership_grace_runs_total {grace_runs}\n# HELP scheduler_membership_grace_paused_total Strategies auto-paused after grace expiry.\n# TYPE scheduler_membership_grace_paused_total counter\nscheduler_membership_grace_paused_total {grace_paused}\n# HELP scheduler_membership_grace_failures_total Membership grace loop failures.\n# TYPE scheduler_membership_grace_failures_total counter\nscheduler_membership_grace_failures_total {grace_failures}\n",
        sync_runs = symbol.run_count,
        symbols = symbol.last_synced_symbols,
        grace_runs = grace.runs_total,
        grace_paused = grace.paused_total,
        grace_failures = grace.failures_total,
    )
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct PersistentSymbolSyncResult {
    refreshed_accounts: usize,
    failed_accounts: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct PersistentSnapshotSyncResult {
    refreshed_accounts: usize,
    failed_accounts: usize,
    account_snapshots: usize,
    wallet_snapshots: usize,
    strategy_snapshots: usize,
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

fn run_persistent_snapshot_sync_once(db: &SharedDb) -> PersistentSnapshotSyncResult {
    let accounts = db
        .list_active_exchange_accounts(BINANCE_EXCHANGE)
        .unwrap_or_else(|error| {
            eprintln!("scheduler persistent snapshot sync failed to list accounts: {error}");
            Vec::new()
        });
    let mut result = PersistentSnapshotSyncResult::default();

    for account in accounts {
        match refresh_account_snapshots(db, &account) {
            Ok((account_snapshots, wallet_snapshots)) => {
                result.refreshed_accounts += 1;
                result.account_snapshots += account_snapshots;
                result.wallet_snapshots += wallet_snapshots;
            }
            Err(error) => {
                result.failed_accounts += 1;
                eprintln!(
                    "scheduler persistent snapshot sync failed for {}: {}",
                    account.user_email, error
                );
            }
        }
    }

    result.strategy_snapshots = run_strategy_snapshot_sync_once(db).unwrap_or_default();
    result
}

fn refresh_account_snapshots(
    db: &SharedDb,
    account: &UserExchangeAccountRecord,
) -> Result<(usize, usize), IoError> {
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
    let client = BinanceClient::new(api_key, api_secret);
    let bundle = client
        .snapshot_bundle(&metadata.selected_markets)
        .map_err(validation_error)?;
    let captured_at = Utc::now();

    for snapshot in &bundle.account_snapshots {
        db.insert_account_profit_snapshot(&AccountProfitSnapshotRecord {
            user_email: account.user_email.clone(),
            exchange: snapshot.exchange.clone(),
            realized_pnl: snapshot.realized_pnl.clone(),
            unrealized_pnl: snapshot.unrealized_pnl.clone(),
            fees: snapshot.fees.clone(),
            funding: snapshot.funding.clone(),
            captured_at,
        })
        .map_err(storage_error)?;
    }

    for snapshot in &bundle.wallet_snapshots {
        db.insert_exchange_wallet_snapshot(&ExchangeWalletSnapshotRecord {
            user_email: account.user_email.clone(),
            exchange: snapshot.exchange.clone(),
            wallet_type: snapshot.wallet_type.clone(),
            balances: serde_json::to_value(&snapshot.balances).map_err(serde_error)?,
            captured_at,
        })
        .map_err(storage_error)?;
    }

    Ok((bundle.account_snapshots.len(), bundle.wallet_snapshots.len()))
}

fn run_strategy_snapshot_sync_once(db: &SharedDb) -> Result<usize, shared_db::SharedDbError> {
    let strategies = db.list_all_strategies()?;
    let mut inserted = 0usize;
    let mut account_snapshots_by_user: BTreeMap<String, Vec<AccountProfitSnapshotRecord>> = BTreeMap::new();
    let mut market_cost_basis_totals: BTreeMap<(String, String), rust_decimal::Decimal> = BTreeMap::new();

    for strategy in &strategies {
        account_snapshots_by_user
            .entry(strategy.owner_email.clone())
            .or_insert(db.list_account_profit_snapshots(&strategy.owner_email)?);
        let cost_basis = strategy_cost_basis(strategy);
        if cost_basis > rust_decimal::Decimal::ZERO {
            let key = (
                strategy.owner_email.clone(),
                strategy_account_exchange(strategy.market).to_string(),
            );
            let entry = market_cost_basis_totals
                .entry(key)
                .or_insert(rust_decimal::Decimal::ZERO);
            *entry += cost_basis;
        }
    }

    for strategy in strategies {
        let realized = strategy
            .runtime
            .fills
            .iter()
            .filter_map(|fill| fill.realized_pnl)
            .fold(rust_decimal::Decimal::ZERO, |acc, value| acc + value);
        let fees = strategy
            .runtime
            .fills
            .iter()
            .filter_map(|fill| fill.fee_amount)
            .fold(rust_decimal::Decimal::ZERO, |acc, value| acc + value);
        let cost_basis = strategy_cost_basis(&strategy);
        let account_snapshot = account_snapshots_by_user
            .get(&strategy.owner_email)
            .and_then(|snapshots| latest_account_snapshot_for_exchange(snapshots, strategy_account_exchange(strategy.market)));
        let group_total = market_cost_basis_totals
            .get(&(strategy.owner_email.clone(), strategy_account_exchange(strategy.market).to_string()))
            .copied()
            .unwrap_or(rust_decimal::Decimal::ZERO);
        let share = if cost_basis > rust_decimal::Decimal::ZERO && group_total > rust_decimal::Decimal::ZERO {
            cost_basis / group_total
        } else {
            rust_decimal::Decimal::ZERO
        };
        let unrealized = account_snapshot
            .and_then(|snapshot| parse_snapshot_decimal(&snapshot.unrealized_pnl))
            .map(|value| (value * share).normalize())
            .unwrap_or(rust_decimal::Decimal::ZERO);
        let funding = account_snapshot
            .and_then(|snapshot| snapshot.funding.as_deref())
            .and_then(parse_snapshot_decimal)
            .map(|value| (value * share).normalize());
        db.insert_strategy_profit_snapshot(&shared_db::StrategyProfitSnapshotRecord {
            strategy_id: strategy.id,
            realized_pnl: realized.normalize().to_string(),
            unrealized_pnl: unrealized.normalize().to_string(),
            fees: fees.normalize().to_string(),
            funding: funding.map(|value| value.to_string()),
            captured_at: Utc::now(),
        })?;
        inserted += 1;
    }
    Ok(inserted)
}

fn strategy_cost_basis(strategy: &shared_domain::strategy::Strategy) -> rust_decimal::Decimal {
    strategy
        .runtime
        .positions
        .iter()
        .fold(rust_decimal::Decimal::ZERO, |acc, position| {
            acc + (position.quantity * position.average_entry_price)
        })
}

fn parse_snapshot_decimal(value: &str) -> Option<rust_decimal::Decimal> {
    value.parse::<rust_decimal::Decimal>().ok()
}

fn latest_account_snapshot_for_exchange<'a>(
    snapshots: &'a [AccountProfitSnapshotRecord],
    exchange: &str,
) -> Option<&'a AccountProfitSnapshotRecord> {
    snapshots
        .iter()
        .filter(|snapshot| snapshot.exchange == exchange)
        .max_by_key(|snapshot| snapshot.captured_at)
}

fn strategy_account_exchange(market: shared_domain::strategy::StrategyMarket) -> &'static str {
    match market {
        shared_domain::strategy::StrategyMarket::Spot => "binance",
        shared_domain::strategy::StrategyMarket::FuturesUsdM => "binance-usdm",
        shared_domain::strategy::StrategyMarket::FuturesCoinM => "binance-coinm",
    }
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
        configured_membership_grace_interval_secs, configured_symbol_sync_interval_secs,
        health_payload, parse_port, required_env, run_persistent_snapshot_sync_once,
        run_persistent_symbol_sync_once, run_strategy_snapshot_sync_once,
        MembershipGraceMetrics, DEFAULT_PORT, DEFAULT_MEMBERSHIP_GRACE_INTERVAL_SECS,
        DEFAULT_SYMBOL_SYNC_INTERVAL_SECS, SERVICE_NAME,
    };
    use scheduler::jobs::symbol_sync::SymbolSyncRuntimeState;
    use chrono::Utc;
    use serde_json::json;
    use shared_binance::{mask_api_key, CredentialCipher};
    use shared_db::{SharedDb, UserExchangeAccountRecord, UserExchangeCredentialRecord};
    use std::{
        collections::VecDeque,
        io::{Read, Write},
        net::TcpListener,
        sync::{Arc, Mutex, OnceLock},
        thread,
    };

    #[test]
    fn health_payload_mentions_service_name() {
        let symbol = Arc::new(Mutex::new(SymbolSyncRuntimeState::default()));
        let grace = Arc::new(Mutex::new(MembershipGraceMetrics::default()));
        let payload = health_payload(SERVICE_NAME, &symbol, &grace);

        assert!(payload.contains("service_up"));
        assert!(payload.contains("scheduler_membership_grace_runs_total"));
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
    fn membership_grace_interval_uses_minute_default_and_accepts_override() {
        std::env::remove_var("MEMBERSHIP_GRACE_INTERVAL_SECS");
        assert_eq!(
            configured_membership_grace_interval_secs(),
            DEFAULT_MEMBERSHIP_GRACE_INTERVAL_SECS
        );

        std::env::set_var("MEMBERSHIP_GRACE_INTERVAL_SECS", "45");
        assert_eq!(configured_membership_grace_interval_secs(), 45);
        std::env::remove_var("MEMBERSHIP_GRACE_INTERVAL_SECS");
    }

    #[test]
    fn persistent_symbol_sync_updates_active_exchange_accounts_and_symbols() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var(
            "EXCHANGE_CREDENTIALS_MASTER_KEY",
            "scheduler-persistent-sync-test-key",
        );
        std::env::set_var("BINANCE_LIVE_MODE", "0");
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
    fn persistent_strategy_snapshot_sync_persists_runtime_aggregates() {
        let db = SharedDb::ephemeral().expect("db");
        let strategy = strategy_for_snapshot("snapshot-strategy", "snap@example.com");
        db.insert_strategy(&shared_db::StoredStrategy { sequence_id: 1, strategy }).expect("strategy");

        let inserted = run_strategy_snapshot_sync_once(&db).expect("snapshot sync");

        assert_eq!(inserted, 1);
        let snapshots = db.list_strategy_profit_snapshots("snap@example.com").expect("strategy snapshots");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].strategy_id, "snapshot-strategy");
        assert_eq!(snapshots[0].realized_pnl, "12.5");
        assert_eq!(snapshots[0].fees, "0.7");
        assert!(snapshots[0].funding.is_none());
    }

    #[test]
    fn persistent_strategy_snapshot_sync_carries_unrealized_and_funding_from_latest_account_snapshot() {
        let db = SharedDb::ephemeral().expect("db");
        let mut strategy = strategy_for_snapshot("snapshot-strategy", "snap@example.com");
        strategy.runtime.positions = vec![shared_domain::strategy::StrategyRuntimePosition {
            market: shared_domain::strategy::StrategyMarket::Spot,
            mode: shared_domain::strategy::StrategyMode::SpotClassic,
            quantity: rust_decimal::Decimal::new(15, 1),
            average_entry_price: rust_decimal::Decimal::new(22, 0),
        }];
        db.insert_strategy(&shared_db::StoredStrategy { sequence_id: 1, strategy }).expect("strategy");
        db.insert_account_profit_snapshot(&shared_db::AccountProfitSnapshotRecord {
            user_email: "snap@example.com".to_string(),
            exchange: "binance".to_string(),
            realized_pnl: "12.5".to_string(),
            unrealized_pnl: "4.2".to_string(),
            fees: "0.7".to_string(),
            funding: Some("-0.4".to_string()),
            captured_at: Utc::now(),
        }).expect("account snapshot");

        let inserted = run_strategy_snapshot_sync_once(&db).expect("snapshot sync");

        assert_eq!(inserted, 1);
        let snapshots = db.list_strategy_profit_snapshots("snap@example.com").expect("strategy snapshots");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].unrealized_pnl, "4.2");
        assert_eq!(snapshots[0].funding.as_deref(), Some("-0.4"));
    }

    #[test]
    fn persistent_snapshot_sync_persists_account_and_wallet_snapshots() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("EXCHANGE_CREDENTIALS_MASTER_KEY", "scheduler-persistent-sync-test-key");
        std::env::set_var("BINANCE_LIVE_MODE", "1");
        let server = spawn_test_server(vec![
            TestRoute {
                path_prefix: "/api/v3/account?",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{"canTrade":true,"canWithdraw":false,"permissions":["SPOT"],"balances":[{"asset":"BTC","free":"0.01","locked":"0.00"},{"asset":"USDT","free":"120.5","locked":"0.5"}]}"#,
            },
            TestRoute {
                path_prefix: "/fapi/v2/account?",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{"totalWalletBalance":"200.5","totalUnrealizedProfit":"3.25","assets":[{"asset":"USDT","walletBalance":"200.5","unrealizedProfit":"3.25"}]}"#,
            },
        ]);
        std::env::set_var("BINANCE_SPOT_REST_BASE_URL", &server.base_url);
        std::env::set_var("BINANCE_USDM_REST_BASE_URL", &server.base_url);
        let db = SharedDb::ephemeral().expect("ephemeral db");
        let now = Utc::now();
        let cipher = CredentialCipher::new("scheduler-persistent-sync-test-key");

        db.upsert_exchange_account(&UserExchangeAccountRecord {
            user_email: "snapshot@example.com".to_owned(),
            exchange: "binance".to_owned(),
            account_label: "Binance".to_owned(),
            market_scope: "spot,usdm".to_owned(),
            is_active: true,
            checked_at: Some(now),
            metadata: json!({
                "connection_status": "healthy",
                "sync_status": "success",
                "last_synced_at": "2026-04-01T00:00:00Z",
                "expected_hedge_mode": true,
                "selected_markets": ["spot", "usdm"],
                "validation": {
                    "api_connectivity_ok": true,
                    "timestamp_in_sync": true,
                    "can_read_spot": true,
                    "can_read_usdm": true,
                    "can_read_coinm": false,
                    "hedge_mode_ok": true,
                    "permissions_ok": true,
                    "market_access_ok": true
                },
                "symbol_counts": {
                    "spot": 2,
                    "usdm": 2,
                    "coinm": 0
                }
            }),
        }).expect("account");
        db.upsert_exchange_credentials(&UserExchangeCredentialRecord {
            user_email: "snapshot@example.com".to_owned(),
            exchange: "binance".to_owned(),
            api_key_masked: mask_api_key("demo-key-1234"),
            encrypted_secret: cipher.encrypt("demo-key-1234", "demo-secret").expect("encrypt"),
        }).expect("credentials");

        let synced = run_persistent_snapshot_sync_once(&db);
        assert_eq!(synced.refreshed_accounts, 1);
        assert_eq!(synced.failed_accounts, 0);
        assert_eq!(synced.account_snapshots, 2);
        assert_eq!(synced.wallet_snapshots, 2);
        let accounts = db.list_account_profit_snapshots("snapshot@example.com").expect("account snapshots");
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].exchange, "binance");
        assert_eq!(accounts[1].exchange, "binance-usdm");
        let wallets = db.list_exchange_wallet_snapshots("snapshot@example.com").expect("wallet snapshots");
        assert_eq!(wallets.len(), 2);
        assert_eq!(wallets[0].wallet_type, "spot");
    }

    #[test]
    fn persistent_symbol_sync_continues_past_bad_accounts() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var(
            "EXCHANGE_CREDENTIALS_MASTER_KEY",
            "scheduler-persistent-sync-test-key",
        );
        std::env::set_var("BINANCE_LIVE_MODE", "0");
        let db = SharedDb::ephemeral().expect("ephemeral db");
        let now = Utc::now();
        let cipher = CredentialCipher::new("scheduler-persistent-sync-test-key");

        for (email, encrypted_secret) in [
            (
                "good@example.com",
                cipher.encrypt("demo-key-1234", "demo-secret").expect("encrypt"),
            ),
            ("bad@example.com", "broken-payload".to_string()),
        ] {
            db.upsert_exchange_account(&UserExchangeAccountRecord {
                user_email: email.to_owned(),
                exchange: "binance".to_owned(),
                account_label: "Binance".to_owned(),
                market_scope: "spot".to_owned(),
                is_active: true,
                checked_at: Some(now),
                metadata: json!({
                    "connection_status": "healthy",
                    "sync_status": "success",
                    "last_synced_at": "2026-04-01T00:00:00Z",
                    "expected_hedge_mode": true,
                    "selected_markets": ["spot"],
                    "validation": {
                        "api_connectivity_ok": true,
                        "timestamp_in_sync": true,
                        "can_read_spot": true,
                        "can_read_usdm": false,
                        "can_read_coinm": false,
                        "hedge_mode_ok": true,
                        "permissions_ok": true,
                        "market_access_ok": true
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
                user_email: email.to_owned(),
                exchange: "binance".to_owned(),
                api_key_masked: mask_api_key("demo-key-1234"),
                encrypted_secret,
            })
            .expect("credentials");
        }

        let refreshed = run_persistent_symbol_sync_once(&db);
        assert_eq!(refreshed.refreshed_accounts, 1);
        assert_eq!(refreshed.failed_accounts, 1);
    }

    #[derive(Clone)]
    struct TestRoute {
        path_prefix: &'static str,
        status_line: &'static str,
        body: &'static str,
    }

    struct TestServer {
        base_url: String,
        join_handle: Option<thread::JoinHandle<()>>,
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            if let Some(handle) = self.join_handle.take() {
                handle.join().expect("scheduler test server thread should exit cleanly");
            }
        }
    }

    fn spawn_test_server(routes: Vec<TestRoute>) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test server");
        let address = listener.local_addr().expect("test server address");
        let queue = Arc::new(Mutex::new(VecDeque::from(routes)));
        let queue_for_thread = queue.clone();
        let join_handle = thread::spawn(move || {
            while let Some(route) = queue_for_thread.lock().expect("route queue poisoned").pop_front() {
                let (mut stream, _) = listener.accept().expect("accept test request");
                let mut buffer = [0u8; 4096];
                let read = stream.read(&mut buffer).expect("read test request");
                let request = String::from_utf8_lossy(&buffer[..read]);
                let path = request.lines().next().and_then(|line| line.split_whitespace().nth(1)).expect("request path");
                assert!(path.starts_with(route.path_prefix), "expected path prefix {} but received {}", route.path_prefix, path);
                let response = format!(
                    "{}
content-type: application/json
content-length: {}
connection: close

{}",
                    route.status_line,
                    route.body.len(),
                    route.body,
                );
                stream.write_all(response.as_bytes()).expect("write test response");
            }
        });
        TestServer {
            base_url: format!("http://{}", address),
            join_handle: Some(join_handle),
        }
    }

    fn strategy_for_snapshot(id: &str, email: &str) -> shared_domain::strategy::Strategy {
        use rust_decimal::Decimal;
        use shared_domain::strategy::{
            GridGeneration, GridLevel, PostTriggerAction, Strategy, StrategyMarket, StrategyMode,
            StrategyRevision, StrategyRuntime, StrategyRuntimeFill, StrategyRuntimeOrder,
            StrategyStatus,
        };

        Strategy {
            id: id.to_string(),
            owner_email: email.to_string(),
            name: id.to_string(),
            symbol: "BTCUSDT".to_string(),
            budget: "1000".to_string(),
            grid_spacing_bps: 100,
            status: StrategyStatus::Running,
            source_template_id: None,
            membership_ready: true,
            exchange_ready: true,
            permissions_ready: true,
            withdrawals_disabled: true,
            hedge_mode_ready: true,
            symbol_ready: true,
            filters_ready: true,
            margin_ready: true,
            conflict_ready: true,
            balance_ready: true,
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotClassic,
            draft_revision: StrategyRevision {
                revision_id: "rev-1".to_string(),
                version: 1,
                generation: GridGeneration::Custom,
                levels: vec![GridLevel {
                    level_index: 0,
                    entry_price: Decimal::new(100, 0),
                    quantity: Decimal::new(1, 0),
                    take_profit_bps: 100,
                    trailing_bps: None,
                }],
                overall_take_profit_bps: None,
                overall_stop_loss_bps: None,
                post_trigger_action: PostTriggerAction::Stop,
            },
            active_revision: None,
            runtime: StrategyRuntime {
                positions: Vec::new(),
                orders: vec![StrategyRuntimeOrder {
                    order_id: format!("{}-order-1", id),
                    exchange_order_id: Some("123".to_string()),
                    level_index: Some(0),
                    side: "Buy".to_string(),
                    order_type: "Limit".to_string(),
                    price: Some(Decimal::new(100, 0)),
                    quantity: Decimal::new(1, 0),
                    status: "Filled".to_string(),
                }],
                fills: vec![
                    StrategyRuntimeFill {
                        fill_id: format!("{}-fill-1", id),
                        order_id: Some(format!("{}-order-1", id)),
                        level_index: Some(0),
                        fill_type: "ExchangeFill".to_string(),
                        price: Decimal::new(110, 0),
                        quantity: Decimal::new(1, 0),
                        realized_pnl: Some(Decimal::new(125, 1)),
                        fee_amount: Some(Decimal::new(5, 1)),
                        fee_asset: Some("USDT".to_string()),
                    },
                    StrategyRuntimeFill {
                        fill_id: format!("{}-fill-2", id),
                        order_id: Some(format!("{}-order-1", id)),
                        level_index: Some(0),
                        fill_type: "ExchangeFill".to_string(),
                        price: Decimal::new(112, 0),
                        quantity: Decimal::new(1, 0),
                        realized_pnl: Some(Decimal::ZERO),
                        fee_amount: Some(Decimal::new(2, 1)),
                        fee_asset: Some("USDT".to_string()),
                    },
                ],
                events: Vec::new(),
                last_preflight: None,
            },
            archived_at: None,
        }
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }
}
