use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared_binance::{
    encrypt_credentials, mask_api_key, matches_symbol_query, sync_symbol_metadata, BinanceClient,
    ExchangeCredentialCheck, SymbolMetadata,
};
use shared_db::{
    SharedDb, UserExchangeAccountRecord, UserExchangeCredentialRecord, UserExchangeSymbolRecord,
};

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
    pub validation: ExchangeCredentialCheckDto,
    pub symbol_counts: ExchangeSymbolCountsDto,
}

#[derive(Debug, Serialize)]
pub struct ExchangeCredentialCheckDto {
    pub can_read_spot: bool,
    pub can_read_usdm: bool,
    pub can_read_coinm: bool,
    pub hedge_mode_ok: bool,
    pub permissions_ok: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct StoredExchangeMetadata {
    connection_status: String,
    sync_status: String,
    last_synced_at: Option<String>,
    validation: StoredValidationSnapshot,
    symbol_counts: ExchangeSymbolCountsDto,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct StoredValidationSnapshot {
    can_read_spot: bool,
    can_read_usdm: bool,
    can_read_coinm: bool,
    hedge_mode_ok: bool,
    permissions_ok: bool,
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
        let client = BinanceClient::new(api_key.clone(), api_secret.clone());
        let check = client.check_credentials(request.expected_hedge_mode);
        let symbols = sync_symbol_metadata(&client, &check);
        let symbol_counts = ExchangeSymbolCountsDto::from_symbols(&symbols);
        let now = Utc::now();

        let stored_metadata = StoredExchangeMetadata {
            connection_status: check.connection_status().to_owned(),
            sync_status: "success".to_owned(),
            last_synced_at: Some(now.to_rfc3339()),
            validation: StoredValidationSnapshot::from(&check),
            symbol_counts,
        };

        self.db
            .upsert_exchange_account(&UserExchangeAccountRecord {
                user_email: user_email.clone(),
                exchange: BINANCE_EXCHANGE.to_owned(),
                account_label: "Binance".to_owned(),
                market_scope: "spot,usdm,coinm".to_owned(),
                is_active: true,
                checked_at: Some(now),
                metadata: serde_json::to_value(&stored_metadata).map_err(|error| {
                    ExchangeError::storage(shared_db::SharedDbError::new(error.to_string()))
                })?,
            })
            .map_err(ExchangeError::storage)?;

        self.db
            .upsert_exchange_credentials(&UserExchangeCredentialRecord {
                user_email: user_email.clone(),
                exchange: BINANCE_EXCHANGE.to_owned(),
                api_key_masked: mask_api_key(&api_key),
                encrypted_secret: encrypt_credentials(&api_key, &api_secret),
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

        let account = self.read_account_model(&user_email)?;
        Ok(SaveBinanceCredentialsResponse {
            account,
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
            validation: ExchangeCredentialCheckDto::from(metadata.validation),
            symbol_counts: metadata.symbol_counts,
        })
    }
}

impl From<ExchangeCredentialCheck> for ExchangeCredentialCheckDto {
    fn from(value: ExchangeCredentialCheck) -> Self {
        Self {
            can_read_spot: value.can_read_spot,
            can_read_usdm: value.can_read_usdm,
            can_read_coinm: value.can_read_coinm,
            hedge_mode_ok: value.hedge_mode_ok,
            permissions_ok: value.permissions_ok,
        }
    }
}

impl From<StoredValidationSnapshot> for ExchangeCredentialCheckDto {
    fn from(value: StoredValidationSnapshot) -> Self {
        Self {
            can_read_spot: value.can_read_spot,
            can_read_usdm: value.can_read_usdm,
            can_read_coinm: value.can_read_coinm,
            hedge_mode_ok: value.hedge_mode_ok,
            permissions_ok: value.permissions_ok,
        }
    }
}

impl StoredValidationSnapshot {
    fn from(check: &ExchangeCredentialCheck) -> Self {
        Self {
            can_read_spot: check.can_read_spot,
            can_read_usdm: check.can_read_usdm,
            can_read_coinm: check.can_read_coinm,
            hedge_mode_ok: check.hedge_mode_ok,
            permissions_ok: check.permissions_ok,
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

fn parse_account_metadata(value: &Value) -> Result<StoredExchangeMetadata, ExchangeError> {
    serde_json::from_value(value.clone()).map_err(|error| {
        ExchangeError::storage(shared_db::SharedDbError::new(format!(
            "failed to decode exchange account metadata: {error}"
        )))
    })
}

fn to_symbol_record(
    user_email: &str,
    symbol: SymbolMetadata,
    synced_at: DateTime<Utc>,
) -> UserExchangeSymbolRecord {
    UserExchangeSymbolRecord {
        user_email: user_email.to_owned(),
        exchange: BINANCE_EXCHANGE.to_owned(),
        market: symbol.market,
        symbol: symbol.symbol,
        status: symbol.status,
        base_asset: symbol.base_asset,
        quote_asset: symbol.quote_asset,
        price_precision: symbol.price_precision as i32,
        quantity_precision: symbol.quantity_precision as i32,
        min_quantity: symbol.min_quantity,
        min_notional: symbol.min_notional,
        keywords: symbol.keywords,
        metadata: json!({}),
        synced_at,
    }
}

fn from_symbol_record(record: UserExchangeSymbolRecord) -> SymbolMetadata {
    SymbolMetadata {
        symbol: record.symbol,
        market: record.market,
        status: record.status,
        base_asset: record.base_asset,
        quote_asset: record.quote_asset,
        price_precision: record.price_precision.max(0) as u32,
        quantity_precision: record.quantity_precision.max(0) as u32,
        min_quantity: record.min_quantity,
        min_notional: record.min_notional,
        keywords: record.keywords,
    }
}
