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
        martingale_publish_service::{LivePortfolio, MartingalePublishService, PublishError},
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/martingale-portfolios", get(list_portfolios))
        .route("/martingale-portfolios/{id}", get(get_portfolio))
        .route("/martingale-portfolios/{id}/pause", post(pause_portfolio))
        .route("/martingale-portfolios/{id}/stop", post(stop_portfolio))
}

async fn list_portfolios(
    State(auth): State<AuthService>,
    State(service): State<MartingalePublishService>,
    headers: HeaderMap,
) -> Result<Json<Vec<LivePortfolio>>, PublishError> {
    let session = require_user_session(&auth, &headers)
        .map_err(|_error| PublishError::bad_request("unauthorized"))?;
    Ok(Json(service.list_portfolios(&session.email)?))
}

async fn get_portfolio(
    State(auth): State<AuthService>,
    State(service): State<MartingalePublishService>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<LivePortfolio>, PublishError> {
    let session = require_user_session(&auth, &headers)
        .map_err(|_error| PublishError::bad_request("unauthorized"))?;
    Ok(Json(service.get_portfolio(&session.email, &id)?))
}

async fn pause_portfolio(
    State(auth): State<AuthService>,
    State(service): State<MartingalePublishService>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<LivePortfolio>, PublishError> {
    let session = require_user_session(&auth, &headers)
        .map_err(|_error| PublishError::bad_request("unauthorized"))?;
    Ok(Json(service.pause_portfolio(&session.email, &id)?))
}

async fn stop_portfolio(
    State(auth): State<AuthService>,
    State(service): State<MartingalePublishService>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<LivePortfolio>, PublishError> {
    let session = require_user_session(&auth, &headers)
        .map_err(|_error| PublishError::bad_request("unauthorized"))?;
    Ok(Json(service.stop_portfolio(&session.email, &id)?))
}
