use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};

use crate::{
    routes::auth_guard::require_user_session,
    services::{
        analytics_service::AnalyticsService, auth_service::AuthError, auth_service::AuthService,
    },
    AppState,
};
use shared_domain::analytics::AnalyticsReport;

pub fn router() -> Router<AppState> {
    Router::new().route("/analytics", get(get_analytics))
}

async fn get_analytics(
    State(auth): State<AuthService>,
    State(service): State<AnalyticsService>,
    headers: HeaderMap,
) -> Result<Json<AnalyticsReport>, AuthError> {
    require_user_session(&auth, &headers)?;
    Ok(Json(service.report()))
}
