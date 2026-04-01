use std::env;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared_binance::{
    mask_api_key, matches_symbol_query, sync_symbol_metadata, BinanceClient, CredentialCipher,
    CredentialValidationRequest, ExchangeCredentialCheck, MarketRequirements, SymbolFilters,
    SymbolMetadata,
};
use shared_db::{
    SharedDb, UserExchangeAccountRecord, UserExchangeCredentialRecord, UserExchangeSymbolRecord,
};
use shared_domain::strategy::StrategyStatus;

use crate::services::auth_service::AuthError;

const BINANCE_EXCHANGE: &str = "binance";
const DEFAULT_EXCHANGE_CREDENTIALS_MASTER_KEY: &str = "grid-binance-dev-exchange-credentials-key";

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

        let selected_markets = request.selected_markets.unwrap_or_default();
        let validation_request =
            CredentialValidationRequest::new(request.expected_hedge_mode, &selected_markets);
        let client = BinanceClient::new(api_key.clone(), api_secret.clone());
        let check = client.check_credentials_for(&validation_request);
        let symbols = sync_symbol_metadata(&client, &check);
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

        self.db
            .upsert_exchange_account(&UserExchangeAccountRecord {
                user_email: user_email.clone(),
                exchange: BINANCE_EXCHANGE.to_owned(),
                account_label: "Binance".to_owned(),
                market_scope: check.selected_markets.join(","),
                is_active: true,
                checked_at: Some(now),
                metadata: serde_json::to_value(&stored_metadata)
                    .map_err(map_serde_storage_error)?,
            })
            .map_err(ExchangeError::storage)?;

        let cipher = credential_cipher();
        let encrypted_secret = cipher.encrypt(&api_key, &api_secret).map_err(|error| {
            ExchangeError::storage(shared_db::SharedDbError::new(error.to_string()))
        })?;

        self.db
            .upsert_exchange_credentials(&UserExchangeCredentialRecord {
                user_email: user_email.clone(),
                exchange: BINANCE_EXCHANGE.to_owned(),
                api_key_masked: mask_api_key(&api_key),
                encrypted_secret,
            })
            .map_err(ExchangeError::storage)?;

        let symbol_records = symbols
            .into_iter()
            .map(|symbol| to_symbol_record(&user_email, symbol, now))
            .collect::<Vec<_>>();

        let synced_symbols = self
            .db
            .replace_exchange_symbols(&user_email, BINANCE_EXCHANGE, &symbol_records)
            .map_err(ExchangeError::storage)?;

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
                .decrypt(&credentials.encrypted_secret)
                .map_err(|error| {
                    ExchangeError::storage(shared_db::SharedDbError::new(error.to_string()))
                })?;
            let validation_request = CredentialValidationRequest::new(
                stored.expected_hedge_mode,
                &stored.selected_markets,
            );
            let client = BinanceClient::new(api_key, api_secret);
            let check = client.check_credentials_for(&validation_request);
            let symbols = sync_symbol_metadata(&client, &check);
            let symbol_counts = ExchangeSymbolCountsDto::from_symbols(&symbols);
            let synced_at = Utc::now();

            self.db
                .replace_exchange_symbols(
                    &account.user_email,
                    BINANCE_EXCHANGE,
                    &symbols
                        .into_iter()
                        .map(|symbol| to_symbol_record(&account.user_email, symbol, synced_at))
                        .collect::<Vec<_>>(),
                )
                .map_err(ExchangeError::storage)?;

            self.db
                .upsert_exchange_account(&UserExchangeAccountRecord {
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
                })
                .map_err(ExchangeError::storage)?;

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

fn credential_cipher() -> CredentialCipher {
    let key_material = env::var("EXCHANGE_CREDENTIALS_MASTER_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| env::var("SESSION_TOKEN_SECRET").ok())
        .unwrap_or_else(|| DEFAULT_EXCHANGE_CREDENTIALS_MASTER_KEY.to_owned());
    CredentialCipher::new(key_material)
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
