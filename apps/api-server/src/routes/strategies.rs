use axum::{
    extract::{Path, State},
    routing::{get, post, put},
    Json, Router,
};

use crate::{
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

async fn list_strategies(State(service): State<StrategyService>) -> Json<StrategyListResponse> {
    Json(service.list_strategies())
}

async fn create_strategy(
    State(service): State<StrategyService>,
    Json(request): Json<SaveStrategyRequest>,
) -> Result<(axum::http::StatusCode, Json<Strategy>), StrategyError> {
    Ok((
        axum::http::StatusCode::CREATED,
        Json(service.create_strategy(request)?),
    ))
}

async fn update_strategy(
    State(service): State<StrategyService>,
    Path(strategy_id): Path<String>,
    Json(request): Json<SaveStrategyRequest>,
) -> Result<Json<Strategy>, StrategyError> {
    Ok(Json(service.update_strategy(&strategy_id, request)?))
}

async fn preflight_strategy(
    State(service): State<StrategyService>,
    Path(strategy_id): Path<String>,
) -> Result<Json<PreflightReport>, StrategyError> {
    Ok(Json(service.preflight_strategy(&strategy_id)?))
}

async fn start_strategy(
    State(service): State<StrategyService>,
    Path(strategy_id): Path<String>,
) -> Result<Json<StartStrategyResponse>, StrategyError> {
    Ok(Json(service.start_strategy(&strategy_id)?))
}

async fn pause_strategies(
    State(service): State<StrategyService>,
    Json(request): Json<BatchStrategyRequest>,
) -> Result<Json<BatchPauseResponse>, StrategyError> {
    Ok(Json(service.pause_strategies(request)?))
}

async fn delete_strategies(
    State(service): State<StrategyService>,
    Json(request): Json<BatchStrategyRequest>,
) -> Result<Json<BatchDeleteResponse>, StrategyError> {
    Ok(Json(service.delete_strategies(request)?))
}

async fn stop_all_strategies(State(service): State<StrategyService>) -> Json<StopAllResponse> {
    Json(service.stop_all())
}
