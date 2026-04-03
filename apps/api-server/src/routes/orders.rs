use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};

use crate::{
    routes::auth_guard::require_user_session,
    services::auth_service::AuthService,
    services::strategy_service::{StrategyError, StrategyRuntimeResponse, StrategyService},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/strategies/{strategy_id}/orders",
        get(list_strategy_runtime),
    )
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
