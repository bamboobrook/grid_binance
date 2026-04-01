use std::sync::{Arc, Mutex};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use shared_binance::{
    matches_symbol_query, sync_symbol_metadata, BinanceClient, ExchangeCredentialCheck,
    SymbolMetadata,
};

#[derive(Clone, Default)]
pub struct ExchangeService {
    inner: Arc<Mutex<ExchangeState>>,
}

#[derive(Default)]
struct ExchangeState {
    symbols: Vec<SymbolMetadata>,
}

#[derive(Debug, Deserialize)]
pub struct SaveBinanceCredentialsRequest {
    pub api_key: String,
    pub api_secret: String,
    pub expected_hedge_mode: bool,
}

#[derive(Debug, Serialize)]
pub struct SaveBinanceCredentialsResponse {
    pub check: ExchangeCredentialCheckDto,
    pub synced_symbols: usize,
}

#[derive(Debug, Deserialize)]
pub struct SearchSymbolsRequest {
    pub query: String,
}

#[derive(Debug, Serialize)]
pub struct SearchSymbolsResponse {
    pub items: Vec<SymbolMetadata>,
}

impl ExchangeService {
    pub fn save_binance_credentials(
        &self,
        request: SaveBinanceCredentialsRequest,
    ) -> Result<SaveBinanceCredentialsResponse, ExchangeError> {
        if request.api_key.trim().is_empty() || request.api_secret.trim().is_empty() {
            return Err(ExchangeError::bad_request(
                "api_key and api_secret are required",
            ));
        }

        let client = BinanceClient::new(request.api_key, request.api_secret);
        let check = client.check_credentials(request.expected_hedge_mode);
        let symbols = sync_symbol_metadata(&client, &check);

        let synced_symbols = symbols.len();
        let mut inner = self.inner.lock().expect("exchange state poisoned");
        inner.symbols = symbols;

        Ok(SaveBinanceCredentialsResponse {
            check: ExchangeCredentialCheckDto::from(check),
            synced_symbols,
        })
    }

    pub fn search_symbols(
        &self,
        request: SearchSymbolsRequest,
    ) -> Result<SearchSymbolsResponse, ExchangeError> {
        if request.query.trim().is_empty() {
            return Err(ExchangeError::bad_request("query is required"));
        }

        let inner = self.inner.lock().expect("exchange state poisoned");
        let items = inner
            .symbols
            .iter()
            .filter(|symbol| matches_symbol_query(symbol, &request.query))
            .cloned()
            .collect();

        Ok(SearchSymbolsResponse { items })
    }
}

#[derive(Debug, Serialize)]
pub struct ExchangeCredentialCheckDto {
    pub can_read_spot: bool,
    pub can_read_futures: bool,
    pub hedge_mode_ok: bool,
}

impl From<ExchangeCredentialCheck> for ExchangeCredentialCheckDto {
    fn from(value: ExchangeCredentialCheck) -> Self {
        Self {
            can_read_spot: value.can_read_spot,
            can_read_futures: value.can_read_futures,
            hedge_mode_ok: value.hedge_mode_ok,
        }
    }
}

#[derive(Debug)]
pub struct ExchangeError {
    status: StatusCode,
    message: &'static str,
}

impl ExchangeError {
    fn bad_request(message: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message,
        }
    }
}

impl IntoResponse for ExchangeError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}
