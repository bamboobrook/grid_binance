use axum::{extract::State, routing::post, Json, Router};

use crate::{
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
    State(service): State<ExchangeService>,
    Json(request): Json<SaveBinanceCredentialsRequest>,
) -> Result<Json<SaveBinanceCredentialsResponse>, ExchangeError> {
    Ok(Json(service.save_binance_credentials(request)?))
}

async fn search_symbols(
    State(service): State<ExchangeService>,
    Json(request): Json<SearchSymbolsRequest>,
) -> Result<Json<SearchSymbolsResponse>, ExchangeError> {
    Ok(Json(service.search_symbols(request)?))
}
