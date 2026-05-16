use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};

use crate::{
    routes::auth_guard::require_user_session,
    services::auth_service::AuthService,
    services::strategy_service::{
        ApplyTemplateRequest, BatchDeleteResponse, BatchPauseResponse, BatchStartResponse,
        BatchStrategyRequest, SaveStrategyRequest, StartStrategyResponse, StopAllResponse,
        StrategyError, StrategyListResponse, StrategyService, TemplateListResponse,
    },
    AppState,
};
use shared_domain::strategy::{PreflightReport, Strategy};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/strategies", get(list_strategies).post(create_strategy))
        .route("/strategies/templates", get(list_templates))
        .route(
            "/strategies/templates/{template_id}/apply",
            post(apply_template),
        )
        .route(
            "/strategies/{strategy_id}",
            get(get_strategy).put(update_strategy),
        )
        .route(
            "/strategies/{strategy_id}/preflight",
            post(preflight_strategy),
        )
        .route("/strategies/{strategy_id}/start", post(start_strategy))
        .route("/strategies/{strategy_id}/resume", post(resume_strategy))
        .route("/strategies/{strategy_id}/stop", post(stop_strategy))
        .route("/strategies/{strategy_id}/clone", post(clone_strategy))
        .route("/strategies/batch/start", post(start_strategies))
        .route("/strategies/batch/pause", post(pause_strategies))
        .route("/strategies/batch/delete", post(delete_strategies))
        .route("/strategies/stop-all", post(stop_all_strategies))
}

#[derive(Debug, serde::Deserialize)]
struct ListStrategiesQuery {
    page: Option<u32>,
    per_page: Option<u32>,
}

async fn list_strategies(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Query(query): Query<ListStrategiesQuery>,
) -> Result<Json<StrategyListResponse>, StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    let email = &session.email;
    match (query.page, query.per_page) {
        (Some(page), Some(per_page)) => {
            let page = page.max(1);
            let per_page = per_page.clamp(1, 100);
            Ok(Json(
                service.list_strategies_paginated(email, page, per_page),
            ))
        }
        (Some(page), None) => {
            let page = page.max(1);
            Ok(Json(service.list_strategies_paginated(email, page, 20)))
        }
        (None, Some(per_page)) => {
            let per_page = per_page.clamp(1, 100);
            Ok(Json(service.list_strategies_paginated(email, 1, per_page)))
        }
        (None, None) => Ok(Json(service.list_strategies(email))),
    }
}

async fn list_templates(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
) -> Result<Json<TemplateListResponse>, StrategyError> {
    let _session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok(Json(service.list_templates()?))
}

async fn apply_template(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Path(template_id): Path<String>,
    Json(request): Json<ApplyTemplateRequest>,
) -> Result<(axum::http::StatusCode, Json<Strategy>), StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(service.apply_template(&session.email, &template_id, request)?),
    ))
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

async fn get_strategy(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Path(strategy_id): Path<String>,
) -> Result<Json<Strategy>, StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    let strategies = service.list_strategies(&session.email);
    let strategy = strategies
        .items
        .into_iter()
        .find(|s| s.id == strategy_id)
        .ok_or_else(|| StrategyError::not_found("strategy not found"))?;
    Ok(Json(strategy))
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

async fn resume_strategy(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Path(strategy_id): Path<String>,
) -> Result<Json<StartStrategyResponse>, StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok(Json(service.resume_strategy(&session.email, &strategy_id)?))
}

async fn stop_strategy(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Path(strategy_id): Path<String>,
) -> Result<Json<Strategy>, StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok(Json(service.stop_strategy(&session.email, &strategy_id)?))
}

async fn start_strategies(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Json(request): Json<BatchStrategyRequest>,
) -> Result<Json<BatchStartResponse>, StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok(Json(service.start_strategies(&session.email, request)?))
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

async fn clone_strategy(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Path(strategy_id): Path<String>,
) -> Result<(axum::http::StatusCode, Json<Strategy>), StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(service.clone_strategy(&session.email, &strategy_id)?),
    ))
}
