use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};

use crate::{
    routes::auth_guard::require_user_session,
    services::{
        auth_service::AuthService,
        martingale_exchange_preconfigure_service::{
            response_from_target_without_exchange_readback,
            target_exchange_settings_from_portfolio, validate_preconfigure_confirmations,
            ExchangePreconfigureRequest, ExchangePreconfigureResponse,
        },
        martingale_publish_service::{MartingalePublishService, PublishError},
    },
    AppState,
};
use shared_db::MartingalePortfolioRecord;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/martingale-portfolios", get(list_portfolios))
        .route("/martingale-portfolios/{id}", get(get_portfolio))
        .route(
            "/martingale-portfolios/{id}/exchange-preflight",
            get(exchange_preflight_portfolio),
        )
        .route(
            "/martingale-portfolios/{id}/exchange-preconfigure",
            post(exchange_preconfigure_portfolio),
        )
        .route("/martingale-portfolios/{id}/pause", post(pause_portfolio))
        .route("/martingale-portfolios/{id}/stop", post(stop_portfolio))
}

async fn list_portfolios(
    State(auth): State<AuthService>,
    State(service): State<MartingalePublishService>,
    headers: HeaderMap,
) -> Result<Json<Vec<MartingalePortfolioRecord>>, PublishError> {
    let session = require_user_session(&auth, &headers)
        .map_err(|_error| PublishError::unauthorized("unauthorized"))?;
    Ok(Json(service.list_portfolios(&session.email)?))
}

async fn get_portfolio(
    State(auth): State<AuthService>,
    State(service): State<MartingalePublishService>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<MartingalePortfolioRecord>, PublishError> {
    let session = require_user_session(&auth, &headers)
        .map_err(|_error| PublishError::unauthorized("unauthorized"))?;
    Ok(Json(service.get_portfolio(&session.email, &id)?))
}

async fn exchange_preflight_portfolio(
    State(auth): State<AuthService>,
    State(service): State<MartingalePublishService>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ExchangePreconfigureResponse>, PublishError> {
    let session = require_user_session(&auth, &headers)
        .map_err(|_error| PublishError::unauthorized("unauthorized"))?;
    let portfolio = service.get_portfolio(&session.email, &id)?;
    let target = target_exchange_settings_from_portfolio(&portfolio)
        .map_err(|error| PublishError::bad_request(error.to_string()))?;
    Ok(Json(response_from_target_without_exchange_readback(
        target,
        "readback_required",
        "exchange readback is added in Task 3",
    )))
}

async fn exchange_preconfigure_portfolio(
    State(auth): State<AuthService>,
    State(service): State<MartingalePublishService>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<ExchangePreconfigureRequest>,
) -> Result<Json<ExchangePreconfigureResponse>, PublishError> {
    let session = require_user_session(&auth, &headers)
        .map_err(|_error| PublishError::unauthorized("unauthorized"))?;
    let portfolio = service.get_portfolio(&session.email, &id)?;
    validate_preconfigure_confirmations(&portfolio, &request)
        .map_err(|error| PublishError::bad_request(error.to_string()))?;
    let target = target_exchange_settings_from_portfolio(&portfolio)
        .map_err(|error| PublishError::bad_request(error.to_string()))?;
    Ok(Json(response_from_target_without_exchange_readback(
        target,
        "readback_required",
        "exchange readback is added in Task 3",
    )))
}

async fn pause_portfolio(
    State(auth): State<AuthService>,
    State(service): State<MartingalePublishService>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<MartingalePortfolioRecord>, PublishError> {
    let session = require_user_session(&auth, &headers)
        .map_err(|_error| PublishError::unauthorized("unauthorized"))?;
    Ok(Json(service.pause_portfolio(&session.email, &id)?))
}

async fn stop_portfolio(
    State(auth): State<AuthService>,
    State(service): State<MartingalePublishService>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<MartingalePortfolioRecord>, PublishError> {
    let session = require_user_session(&auth, &headers)
        .map_err(|_error| PublishError::unauthorized("unauthorized"))?;
    Ok(Json(service.stop_portfolio(&session.email, &id)?))
}
