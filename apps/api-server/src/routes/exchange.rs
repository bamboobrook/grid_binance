use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};

use crate::{
    routes::auth_guard::require_user_session,
    services::auth_service::AuthService,
    services::exchange_service::{
        ExchangeError, ExchangeService, ReadBinanceAccountResponse, SaveBinanceCredentialsRequest,
        SaveBinanceCredentialsResponse, SearchSymbolsRequest, SearchSymbolsResponse,
        TestBinanceCredentialsResponse,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/exchange/binance/credentials", post(save_credentials))
        .route("/exchange/binance/credentials/test", post(test_credentials))
        .route("/exchange/binance/account", get(read_account))
        .route("/exchange/binance/symbols/search", post(search_symbols))
}

async fn save_credentials(
    State(auth): State<AuthService>,
    State(service): State<ExchangeService>,
    headers: HeaderMap,
    Json(request): Json<SaveBinanceCredentialsRequest>,
) -> Result<Json<SaveBinanceCredentialsResponse>, ExchangeError> {
    let session = require_user_session(&auth, &headers).map_err(ExchangeError::from)?;
    Ok(Json(
        service.save_binance_credentials(&session.email, request)?,
    ))
}

async fn test_credentials(
    State(auth): State<AuthService>,
    State(service): State<ExchangeService>,
    headers: HeaderMap,
    Json(request): Json<SaveBinanceCredentialsRequest>,
) -> Result<Json<TestBinanceCredentialsResponse>, ExchangeError> {
    let session = require_user_session(&auth, &headers).map_err(ExchangeError::from)?;
    Ok(Json(
        service.test_binance_credentials(&session.email, request)?,
    ))
}

async fn read_account(
    State(auth): State<AuthService>,
    State(service): State<ExchangeService>,
    headers: HeaderMap,
) -> Result<Json<ReadBinanceAccountResponse>, ExchangeError> {
    let session = require_user_session(&auth, &headers).map_err(ExchangeError::from)?;
    Ok(Json(service.read_binance_account(&session.email)?))
}

async fn search_symbols(
    State(auth): State<AuthService>,
    State(service): State<ExchangeService>,
    headers: HeaderMap,
    Json(request): Json<SearchSymbolsRequest>,
) -> Result<Json<SearchSymbolsResponse>, ExchangeError> {
    let session = require_user_session(&auth, &headers).map_err(ExchangeError::from)?;
    Ok(Json(service.search_symbols(&session.email, request)?))
}
