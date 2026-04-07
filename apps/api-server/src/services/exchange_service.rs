use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared_events::{NotificationEvent, NotificationKind, NotificationRecord};
use shared_binance::{
    mask_api_key, matches_symbol_query, sync_symbol_metadata_strict, BinanceClient, CredentialCipher,
    CredentialValidationRequest, ExchangeCredentialCheck, MarketRequirements, SymbolFilters,
    SymbolMetadata,
};
use shared_db::{
    NotificationLogRecord, SharedDb, UserExchangeAccountRecord, UserExchangeCredentialRecord,
    UserExchangeSymbolRecord,
};
use shared_domain::strategy::StrategyStatus;
use std::{collections::BTreeMap, sync::OnceLock, time::Duration as StdDuration};

use crate::services::auth_service::AuthError;

const BINANCE_EXCHANGE: &str = "binance";
#[derive(Clone)]
pub struct ExchangeService {
    db: SharedDb,
}

#[derive(Debug, Deserialize)]
pub struct SaveBinanceCredentialsRequest {
    pub api_key: String,
    pub api_secret: String,
    pub expected_hedge_mode: bool,
    pub selected_markets: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct SaveBinanceCredentialsResponse {
    pub account: BinanceAccountReadModel,
    pub synced_symbols: usize,
}

#[derive(Debug, Serialize)]
pub struct ReadBinanceAccountResponse {
    pub account: BinanceAccountReadModel,
}

#[derive(Debug, Deserialize)]
pub struct SearchSymbolsRequest {
    pub query: String,
}

#[derive(Debug, Serialize)]
pub struct SearchSymbolsResponse {
    pub items: Vec<SymbolMetadata>,
}

#[derive(Debug, Serialize)]
pub struct BinanceAccountReadModel {
    pub exchange: String,
    pub api_key_masked: String,
    pub connection_status: String,
    pub sync_status: String,
    pub last_checked_at: Option<String>,
    pub last_synced_at: Option<String>,
    pub selected_markets: Vec<String>,
    pub validation: ExchangeCredentialCheckDto,
    pub symbol_counts: ExchangeSymbolCountsDto,
}

#[derive(Debug, Serialize)]
pub struct ExchangeCredentialCheckDto {
    pub api_connectivity_ok: bool,
    pub timestamp_in_sync: bool,
    pub can_read_spot: bool,
    pub can_read_usdm: bool,
    pub can_read_coinm: bool,
    pub hedge_mode_ok: bool,
    pub permissions_ok: bool,
    pub withdrawals_disabled: bool,
    pub market_access_ok: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct StoredExchangeMetadata {
    connection_status: String,
    sync_status: String,
    last_synced_at: Option<String>,
    expected_hedge_mode: bool,
    selected_markets: Vec<String>,
    validation: StoredValidationSnapshot,
    symbol_counts: ExchangeSymbolCountsDto,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct StoredValidationSnapshot {
    api_connectivity_ok: bool,
    timestamp_in_sync: bool,
    can_read_spot: bool,
    can_read_usdm: bool,
    can_read_coinm: bool,
    hedge_mode_ok: bool,
    permissions_ok: bool,
    withdrawals_disabled: bool,
    market_access_ok: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ExchangeSymbolCountsDto {
    pub spot: usize,
    pub usdm: usize,
    pub coinm: usize,
}

impl ExchangeService {
    pub fn new(db: SharedDb) -> Self {
        Self { db }
    }

    pub fn new_strict(db: SharedDb) -> Result<Self, shared_db::SharedDbError> {
        credential_cipher().map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
        Ok(Self { db })
    }

    pub fn save_binance_credentials(
        &self,
        user_email: &str,
        request: SaveBinanceCredentialsRequest,
    ) -> Result<SaveBinanceCredentialsResponse, ExchangeError> {
        let api_key = request.api_key.trim().to_owned();
        let api_secret = request.api_secret.trim().to_owned();
        if api_key.is_empty() || api_secret.is_empty() {
            return Err(ExchangeError::bad_request(
                "api_key and api_secret are required",
            ));
        }

        let user_email = normalize_email(user_email);
        self.ensure_no_running_strategies_for_credential_update(&user_email)?;
        let previous_metadata = self
            .db
            .find_exchange_account(&user_email, BINANCE_EXCHANGE)
            .map_err(ExchangeError::storage)?
            .and_then(|record| parse_account_metadata(&record.metadata).ok());

        let selected_markets = request.selected_markets.unwrap_or_default();
        let validation_request =
            CredentialValidationRequest::new(request.expected_hedge_mode, &selected_markets)
                .map_err(|error| ExchangeError::bad_request(error.to_string()))?;
        let client = BinanceClient::new(api_key.clone(), api_secret.clone());
        let check = client.check_credentials_for(&validation_request);
        let symbols = sync_symbol_metadata_strict(&client, &check).map_err(|error| ExchangeError::bad_request(error.to_string()))?;
        let symbol_counts = ExchangeSymbolCountsDto::from_symbols(&symbols);
        let now = Utc::now();

        let stored_metadata = StoredExchangeMetadata {
            connection_status: check.connection_status().to_owned(),
            sync_status: "success".to_owned(),
            last_synced_at: Some(now.to_rfc3339()),
            expected_hedge_mode: request.expected_hedge_mode,
            selected_markets: check.selected_markets.clone(),
            validation: StoredValidationSnapshot::from(&check),
            symbol_counts,
        };

        let cipher = credential_cipher().map_err(map_cipher_storage_error)?;
        let encrypted_secret = cipher.encrypt(&api_key, &api_secret).map_err(|error| {
            ExchangeError::storage(shared_db::SharedDbError::new(error.to_string()))
        })?;

        let account_record = UserExchangeAccountRecord {
            user_email: user_email.clone(),
            exchange: BINANCE_EXCHANGE.to_owned(),
            account_label: "Binance".to_owned(),
            market_scope: check.selected_markets.join(","),
            is_active: true,
            checked_at: Some(now),
            metadata: serde_json::to_value(&stored_metadata).map_err(map_serde_storage_error)?,
        };

        let credential_record = UserExchangeCredentialRecord {
            user_email: user_email.clone(),
            exchange: BINANCE_EXCHANGE.to_owned(),
            api_key_masked: mask_api_key(&api_key),
            encrypted_secret,
        };

        let symbol_records = symbols
            .into_iter()
            .map(|symbol| to_symbol_record(&user_email, symbol, now))
            .collect::<Vec<_>>();

        let synced_symbols = self
            .db
            .save_exchange_account_bundle(&account_record, &credential_record, &symbol_records)
            .map_err(ExchangeError::storage)?;
        self.persist_api_invalidation_notification_if_needed(
            &user_email,
            previous_metadata.as_ref(),
            &check,
            now,
        )?;

        Ok(SaveBinanceCredentialsResponse {
            account: self.read_account_model(&user_email)?,
            synced_symbols,
        })
    }

    pub fn read_binance_account(
        &self,
        user_email: &str,
    ) -> Result<ReadBinanceAccountResponse, ExchangeError> {
        Ok(ReadBinanceAccountResponse {
            account: self.read_account_model(user_email)?,
        })
    }

    pub fn search_symbols(
        &self,
        user_email: &str,
        request: SearchSymbolsRequest,
    ) -> Result<SearchSymbolsResponse, ExchangeError> {
        if request.query.trim().is_empty() {
            return Err(ExchangeError::bad_request("query is required"));
        }

        let items = self
            .db
            .list_exchange_symbols(&normalize_email(user_email), BINANCE_EXCHANGE)
            .map_err(ExchangeError::storage)?
            .into_iter()
            .map(from_symbol_record)
            .filter(|symbol| matches_symbol_query(symbol, &request.query))
            .collect();

        Ok(SearchSymbolsResponse { items })
    }

    #[allow(dead_code)]
    pub fn run_symbol_sync_for_active_accounts(&self) -> Result<usize, ExchangeError> {
        let accounts = self
            .db
            .list_active_exchange_accounts(BINANCE_EXCHANGE)
            .map_err(ExchangeError::storage)?;
        let mut refreshed_accounts = 0usize;

        for account in accounts {
            let Some(credentials) = self
                .db
                .find_exchange_credentials(&account.user_email, BINANCE_EXCHANGE)
                .map_err(ExchangeError::storage)?
            else {
                continue;
            };

            let stored = parse_account_metadata(&account.metadata)?;
            let (api_key, api_secret) = credential_cipher()
                .map_err(map_cipher_storage_error)?
                .decrypt(&credentials.encrypted_secret)
                .map_err(|error| {
                    ExchangeError::storage(shared_db::SharedDbError::new(error.to_string()))
                })?;
            let validation_request = CredentialValidationRequest::new(
                stored.expected_hedge_mode,
                &stored.selected_markets,
            )
            .map_err(|error| ExchangeError::bad_request(error.to_string()))?;
            let client = BinanceClient::new(api_key, api_secret);
            let check = client.check_credentials_for(&validation_request);
            let symbols = sync_symbol_metadata_strict(&client, &check).map_err(|error| ExchangeError::bad_request(error.to_string()))?;
            let symbol_counts = ExchangeSymbolCountsDto::from_symbols(&symbols);
            let synced_at = Utc::now();

            let symbol_records = symbols
                .into_iter()
                .map(|symbol| to_symbol_record(&account.user_email, symbol, synced_at))
                .collect::<Vec<_>>();

            self.db
                .refresh_exchange_account_bundle(
                    &UserExchangeAccountRecord {
                        metadata: serde_json::to_value(StoredExchangeMetadata {
                            connection_status: check.connection_status().to_owned(),
                            sync_status: "success".to_owned(),
                            last_synced_at: Some(synced_at.to_rfc3339()),
                            expected_hedge_mode: stored.expected_hedge_mode,
                            selected_markets: check.selected_markets.clone(),
                            validation: StoredValidationSnapshot::from(&check),
                            symbol_counts,
                        })
                        .map_err(map_serde_storage_error)?,
                        checked_at: Some(synced_at),
                        ..account.clone()
                    },
                    &symbol_records,
                )
                .map_err(ExchangeError::storage)?;
            self.persist_api_invalidation_notification_if_needed(
                &account.user_email,
                Some(&stored),
                &check,
                synced_at,
            )?;

            refreshed_accounts += 1;
        }

        Ok(refreshed_accounts)
    }

    fn ensure_no_running_strategies_for_credential_update(
        &self,
        user_email: &str,
    ) -> Result<(), ExchangeError> {
        let has_existing_account = self
            .db
            .find_exchange_account(user_email, BINANCE_EXCHANGE)
            .map_err(ExchangeError::storage)?
            .is_some();
        if !has_existing_account {
            return Ok(());
        }

        let has_running_strategy = self
            .db
            .list_strategies(user_email)
            .map_err(ExchangeError::storage)?
            .into_iter()
            .any(|strategy| strategy.status == StrategyStatus::Running);
        if has_running_strategy {
            return Err(ExchangeError::conflict(
                "pause running strategies before updating exchange credentials",
            ));
        }

        Ok(())
    }

    fn persist_api_invalidation_notification_if_needed(
        &self,
        user_email: &str,
        previous: Option<&StoredExchangeMetadata>,
        check: &ExchangeCredentialCheck,
        created_at: DateTime<Utc>,
    ) -> Result<(), ExchangeError> {
        if check.is_healthy() {
            return Ok(());
        }
        if previous.is_some_and(|metadata| metadata.connection_status == "degraded") {
            return Ok(());
        }

        let reason = api_invalidation_reason(check);
        let binding = self
            .db
            .find_telegram_binding(user_email)
            .map_err(ExchangeError::storage)?;
        let telegram_delivered = match (binding.as_ref(), telegram_bot_token()) {
            (Some(binding), Some(token)) => send_telegram_message(
                &token,
                &binding.telegram_chat_id,
                "API credentials invalid",
                &format!("Binance validation failed: {reason}."),
            )
            .is_ok(),
            _ => false,
        };
        let record = NotificationRecord {
            event: NotificationEvent {
                email: user_email.to_owned(),
                kind: NotificationKind::ApiCredentialsInvalidated,
                title: "API credentials invalid".to_string(),
                message: format!("Binance validation failed: {reason}."),
                payload: BTreeMap::from([
                    ("exchange".to_string(), BINANCE_EXCHANGE.to_string()),
                    ("reason".to_string(), reason),
                    ("connection_status".to_string(), check.connection_status().to_string()),
                ]),
            },
            telegram_delivered,
            in_app_delivered: true,
            show_expiry_popup: false,
        };
        let payload = serde_json::to_value(&record)
            .map_err(|error| ExchangeError::storage(shared_db::SharedDbError::new(error.to_string())))?;
        self.db
            .insert_notification_log(&NotificationLogRecord {
                user_email: user_email.to_owned(),
                channel: "in_app".to_string(),
                template_key: Some("ApiCredentialsInvalidated".to_string()),
                title: record.event.title.clone(),
                body: record.event.message.clone(),
                status: "delivered".to_string(),
                payload: payload.clone(),
                created_at,
                delivered_at: Some(created_at),
            })
            .map_err(ExchangeError::storage)?;
        if binding.is_some() {
            self.db
                .insert_notification_log(&NotificationLogRecord {
                    user_email: user_email.to_owned(),
                    channel: "telegram".to_string(),
                    template_key: Some("ApiCredentialsInvalidated".to_string()),
                    title: record.event.title,
                    body: record.event.message,
                    status: if telegram_delivered { "delivered" } else { "failed" }.to_string(),
                    payload,
                    created_at,
                    delivered_at: telegram_delivered.then_some(created_at),
                })
                .map_err(ExchangeError::storage)?;
        }
        Ok(())
    }

    fn read_account_model(
        &self,
        user_email: &str,
    ) -> Result<BinanceAccountReadModel, ExchangeError> {
        let user_email = normalize_email(user_email);
        let account = self
            .db
            .find_exchange_account(&user_email, BINANCE_EXCHANGE)
            .map_err(ExchangeError::storage)?
            .ok_or_else(|| ExchangeError::not_found("exchange account not found"))?;
        let credentials = self
            .db
            .find_exchange_credentials(&user_email, BINANCE_EXCHANGE)
            .map_err(ExchangeError::storage)?
            .ok_or_else(|| ExchangeError::not_found("exchange account not found"))?;
        let metadata = parse_account_metadata(&account.metadata)?;

        Ok(BinanceAccountReadModel {
            exchange: account.exchange,
            api_key_masked: credentials.api_key_masked,
            connection_status: metadata.connection_status,
            sync_status: metadata.sync_status,
            last_checked_at: account.checked_at.map(format_timestamp),
            last_synced_at: metadata.last_synced_at,
            selected_markets: metadata.selected_markets,
            validation: ExchangeCredentialCheckDto::from(metadata.validation),
            symbol_counts: metadata.symbol_counts,
        })
    }
}

impl From<ExchangeCredentialCheck> for ExchangeCredentialCheckDto {
    fn from(value: ExchangeCredentialCheck) -> Self {
        Self {
            api_connectivity_ok: value.api_connectivity_ok,
            timestamp_in_sync: value.timestamp_in_sync,
            can_read_spot: value.can_read_spot,
            can_read_usdm: value.can_read_usdm,
            can_read_coinm: value.can_read_coinm,
            hedge_mode_ok: value.hedge_mode_ok,
            permissions_ok: value.permissions_ok,
            withdrawals_disabled: value.withdrawal_disabled,
            market_access_ok: value.market_access_ok,
        }
    }
}

impl From<StoredValidationSnapshot> for ExchangeCredentialCheckDto {
    fn from(value: StoredValidationSnapshot) -> Self {
        Self {
            api_connectivity_ok: value.api_connectivity_ok,
            timestamp_in_sync: value.timestamp_in_sync,
            can_read_spot: value.can_read_spot,
            can_read_usdm: value.can_read_usdm,
            can_read_coinm: value.can_read_coinm,
            hedge_mode_ok: value.hedge_mode_ok,
            permissions_ok: value.permissions_ok,
            withdrawals_disabled: value.withdrawals_disabled,
            market_access_ok: value.market_access_ok,
        }
    }
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
            withdrawals_disabled: check.withdrawal_disabled,
            market_access_ok: check.market_access_ok,
        }
    }
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

#[derive(Debug)]
pub struct ExchangeError {
    status: StatusCode,
    message: String,
}

impl ExchangeError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn storage(error: shared_db::SharedDbError) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ExchangeError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

impl From<AuthError> for ExchangeError {
    fn from(value: AuthError) -> Self {
        Self {
            status: value.status,
            message: value.message.to_owned(),
        }
    }
}

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

fn format_timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339()
}

fn map_serde_storage_error(error: serde_json::Error) -> ExchangeError {
    ExchangeError::storage(shared_db::SharedDbError::new(error.to_string()))
}

fn parse_account_metadata(value: &Value) -> Result<StoredExchangeMetadata, ExchangeError> {
    serde_json::from_value(value.clone()).map_err(map_serde_storage_error)
}

fn api_invalidation_reason(check: &ExchangeCredentialCheck) -> String {
    let mut reasons = Vec::new();
    if !check.api_connectivity_ok {
        reasons.push("api_connectivity");
    }
    if !check.timestamp_in_sync {
        reasons.push("timestamp_in_sync");
    }
    if !check.permissions_ok {
        reasons.push("permissions");
    }
    if !check.withdrawal_disabled {
        reasons.push("withdrawal_permission");
    }
    if !check.market_access_ok {
        reasons.push("market_access");
    }
    if !check.hedge_mode_ok {
        reasons.push("hedge_mode");
    }
    if reasons.is_empty() {
        "validation_failed".to_string()
    } else {
        reasons.join(",")
    }
}

fn telegram_bot_token() -> Option<String> {
    std::env::var("TELEGRAM_BOT_TOKEN")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn telegram_api_base_url() -> String {
    std::env::var("TELEGRAM_API_BASE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "https://api.telegram.org".to_string())
}

fn telegram_http_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| ureq::AgentBuilder::new().timeout(StdDuration::from_secs(5)).build())
}

fn send_telegram_message(
    bot_token: &str,
    chat_id: &str,
    title: &str,
    body: &str,
) -> Result<(), shared_db::SharedDbError> {
    telegram_http_agent()
        .post(&format!("{}/bot{}/sendMessage", telegram_api_base_url(), bot_token))
        .send_json(ureq::json!({
            "chat_id": chat_id,
            "text": format!("{}\n{}", title, body),
        }))
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    Ok(())
}

fn credential_cipher() -> Result<CredentialCipher, shared_binance::CredentialCipherError> {
    CredentialCipher::from_env("EXCHANGE_CREDENTIALS_MASTER_KEY")
}

fn map_cipher_storage_error(error: shared_binance::CredentialCipherError) -> ExchangeError {
    ExchangeError::storage(shared_db::SharedDbError::new(error.to_string()))
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
        metadata: serde_json::to_value(&symbol).unwrap_or_else(|_| json!({})),
        synced_at,
    }
}

fn from_symbol_record(record: UserExchangeSymbolRecord) -> SymbolMetadata {
    serde_json::from_value(record.metadata).unwrap_or_else(|_| SymbolMetadata {
        symbol: record.symbol,
        market: record.market,
        status: record.status,
        base_asset: record.base_asset,
        quote_asset: record.quote_asset,
        price_precision: record.price_precision.max(0) as u32,
        quantity_precision: record.quantity_precision.max(0) as u32,
        filters: SymbolFilters {
            price_tick_size: "0".to_owned(),
            quantity_step_size: "0".to_owned(),
            min_quantity: record.min_quantity,
            min_notional: record.min_notional,
            contract_size: None,
        },
        market_requirements: MarketRequirements {
            supports_isolated_margin: false,
            supports_cross_margin: false,
            hedge_mode_required: false,
            requires_futures_permissions: false,
            leverage_brackets: Vec::new(),
        },
        keywords: record.keywords,
    })
}
