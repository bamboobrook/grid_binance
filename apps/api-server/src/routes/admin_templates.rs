use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};

use crate::{
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

async fn list_templates(State(service): State<StrategyService>) -> Json<TemplateListResponse> {
    Json(service.list_templates())
}

async fn create_template(
    State(service): State<StrategyService>,
    Json(request): Json<CreateTemplateRequest>,
) -> Result<
    (
        axum::http::StatusCode,
        Json<shared_domain::strategy::StrategyTemplate>,
    ),
    StrategyError,
> {
    Ok((
        axum::http::StatusCode::CREATED,
        Json(service.create_template(request)?),
    ))
}

async fn apply_template(
    State(service): State<StrategyService>,
    Path(template_id): Path<String>,
    Json(request): Json<ApplyTemplateRequest>,
) -> Result<
    (
        axum::http::StatusCode,
        Json<shared_domain::strategy::Strategy>,
    ),
    StrategyError,
> {
    Ok((
        axum::http::StatusCode::CREATED,
        Json(service.apply_template(&template_id, request)?),
    ))
}
