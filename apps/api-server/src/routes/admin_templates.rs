use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};

use crate::{
    routes::auth_guard::{require_admin_session, require_user_session},
    services::auth_service::AuthService,
    services::strategy_service::{
        ApplyTemplateRequest, CreateTemplateRequest, StrategyError, StrategyService,
        TemplateListResponse,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/admin/templates",
            get(list_templates).post(create_template),
        )
        .route("/admin/templates/{template_id}/apply", post(apply_template))
}

async fn list_templates(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
) -> Result<Json<TemplateListResponse>, StrategyError> {
    require_admin_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok(Json(service.list_templates()))
}

async fn create_template(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Json(request): Json<CreateTemplateRequest>,
) -> Result<
    (
        axum::http::StatusCode,
        Json<shared_domain::strategy::StrategyTemplate>,
    ),
    StrategyError,
> {
    require_admin_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(service.create_template(request)?),
    ))
}

async fn apply_template(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Path(template_id): Path<String>,
    Json(request): Json<ApplyTemplateRequest>,
) -> Result<
    (
        axum::http::StatusCode,
        Json<shared_domain::strategy::Strategy>,
    ),
    StrategyError,
> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(service.apply_template(&session.email, &template_id, request)?),
    ))
}
