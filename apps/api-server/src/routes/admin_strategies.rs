use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};
use serde::Serialize;
use shared_db::SharedDb;
use shared_domain::strategy::Strategy;

use crate::{
    routes::auth_guard::require_admin_session,
    services::auth_service::{AuthError, AuthService},
    AppState,
};

#[derive(Debug, Serialize)]
pub struct AdminStrategyListResponse {
    pub items: Vec<Strategy>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/admin/strategies", get(list_strategies))
}

async fn list_strategies(
    State(auth): State<AuthService>,
    State(db): State<SharedDb>,
    headers: HeaderMap,
) -> Result<Json<AdminStrategyListResponse>, AuthError> {
    require_admin_session(&auth, &headers)?;
    let items = db
        .list_all_strategies()
        .map_err(|error| AuthError::storage(error))?;
    Ok(Json(AdminStrategyListResponse { items }))
}
