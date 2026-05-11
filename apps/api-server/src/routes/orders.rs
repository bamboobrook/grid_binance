use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use std::collections::HashMap;

use crate::{
    routes::auth_guard::require_user_session,
    services::auth_service::AuthService,
    services::strategy_service::{
        BatchStrategyRuntimeResponse, StrategyError, StrategyRuntimeResponse, StrategyService,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/strategies/{strategy_id}/orders",
            get(list_strategy_runtime),
        )
        .route(
            "/strategies/batch/runtimes",
            get(batch_list_strategy_runtimes),
        )
}

#[derive(Debug, serde::Deserialize)]
struct BatchRuntimeQuery {
    ids: String,
}

async fn batch_list_strategy_runtimes(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Query(query): Query<BatchRuntimeQuery>,
) -> Result<Json<BatchStrategyRuntimeResponse>, StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    let strategy_ids: Vec<String> = query.ids.split(',').map(|s| s.to_string()).collect();
    Ok(Json(service.batch_get_strategy_runtimes(
        &session.email,
        &strategy_ids,
    )?))
}

async fn list_strategy_runtime(
    State(auth): State<AuthService>,
    State(service): State<StrategyService>,
    headers: HeaderMap,
    Path(strategy_id): Path<String>,
) -> Result<Json<StrategyRuntimeResponse>, StrategyError> {
    let session = require_user_session(&auth, &headers).map_err(StrategyError::from)?;
    Ok(Json(
        service.get_strategy_runtime(&session.email, &strategy_id)?,
    ))
}
