use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post, put},
    Json, Router,
};

use crate::{
    routes::auth_guard::require_user_session,
    services::auth_service::AuthService,
    services::strategy_service::{
        BatchDeleteResponse, BatchPauseResponse, BatchStrategyRequest, SaveStrategyRequest,
        StartStrategyResponse, StopAllResponse, StrategyError, StrategyListResponse,
        StrategyService,
    },
    AppState,
};
use shared_domain::strategy::{PreflightReport, Strategy};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/strategies", get(list_strategies).post(create_strategy))
        .route("/strategies/{strategy_id}", put(update_strategy))
        .route(
            "/strategies/{strategy_id}/preflight",
            post(preflight_strategy),
        )
        .route("/strategies/{strategy_id}/start", post(start_strategy))
        .route("/strategies/batch/pause", post(pause_strategies))
        .route("/strategies/batch/delete", post(delete_strategies))
        .route("/strategies/stop-all", post(stop_all_strategies))
}

async fn list_strategies(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
) -> Result<Json<StrategyListResponse>, StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok(Json(service.list_strategies(&session.email)))
}

async fn create_strategy(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Json(request): Json<SaveStrategyRequest>,
) -> Result<(axum::http::StatusCode, Json<Strategy>), StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(service.create_strategy(&session.email, request)?),
    ))
}

async fn update_strategy(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Path(strategy_id): Path<String>,
    Json(request): Json<SaveStrategyRequest>,
) -> Result<Json<Strategy>, StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok(Json(service.update_strategy(
        &session.email,
        &strategy_id,
        request,
    )?))
}

async fn preflight_strategy(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Path(strategy_id): Path<String>,
) -> Result<Json<PreflightReport>, StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok(Json(
        service.preflight_strategy(&session.email, &strategy_id)?,
    ))
}

async fn start_strategy(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Path(strategy_id): Path<String>,
) -> Result<Json<StartStrategyResponse>, StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok(Json(service.start_strategy(&session.email, &strategy_id)?))
}

async fn pause_strategies(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Json(request): Json<BatchStrategyRequest>,
) -> Result<Json<BatchPauseResponse>, StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok(Json(service.pause_strategies(&session.email, request)?))
}

async fn delete_strategies(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Json(request): Json<BatchStrategyRequest>,
) -> Result<Json<BatchDeleteResponse>, StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok(Json(service.delete_strategies(&session.email, request)?))
}

async fn stop_all_strategies(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
) -> Result<Json<StopAllResponse>, StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok(Json(service.stop_all(&session.email)))
}
