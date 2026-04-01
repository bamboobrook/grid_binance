use axum::{
    extract::State,
    http::{header, HeaderMap},
    response::IntoResponse,
    routing::get,
    Router,
};

use crate::{
    routes::auth_guard::require_user_session,
    services::{analytics_service::AnalyticsService, auth_service::AuthError, auth_service::AuthService},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/exports/analytics.csv", get(export_analytics_csv))
}

async fn export_analytics_csv(
    State(auth): State<AuthService>,
    State(service): State<AnalyticsService>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AuthError> {
    require_user_session(&auth, &headers)?;
    Ok((
        [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
        service.export_csv(),
    ))
}
