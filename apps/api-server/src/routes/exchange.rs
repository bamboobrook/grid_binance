use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};

use crate::{
    routes::auth_guard::require_user_session,
    services::auth_service::AuthService,
    services::exchange_service::{
        ExchangeError, ExchangeService, SaveBinanceCredentialsRequest,
        SaveBinanceCredentialsResponse, SearchSymbolsRequest, SearchSymbolsResponse,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/exchange/binance/credentials", post(save_credentials))
        .route("/exchange/binance/symbols/search", post(search_symbols))
}

async fn save_credentials(
    State(auth): State<AuthService>,
    State(service): State<ExchangeService>,
    headers: HeaderMap,
    Json(request): Json<SaveBinanceCredentialsRequest>,
) -> Result<Json<SaveBinanceCredentialsResponse>, ExchangeError> {
    require_user_session(&auth, &headers).map_err(ExchangeError::from)?;
    Ok(Json(service.save_binance_credentials(request)?))
}

async fn search_symbols(
    State(auth): State<AuthService>,
    State(service): State<ExchangeService>,
    headers: HeaderMap,
    Json(request): Json<SearchSymbolsRequest>,
) -> Result<Json<SearchSymbolsResponse>, ExchangeError> {
    require_user_session(&auth, &headers).map_err(ExchangeError::from)?;
    Ok(Json(service.search_symbols(request)?))
}
