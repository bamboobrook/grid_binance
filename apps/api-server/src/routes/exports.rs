use axum::{
    extract::State,
    http::{header, HeaderMap},
    response::IntoResponse,
    routing::get,
    Router,
};

use crate::{
    routes::auth_guard::require_user_session,
    services::{
        analytics_service::AnalyticsService, auth_service::AuthError, auth_service::AuthService,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/exports/orders.csv", get(export_orders_csv))
        .route("/exports/fills.csv", get(export_fills_csv))
        .route("/exports/strategy-stats.csv", get(export_strategy_stats_csv))
        .route("/exports/payments.csv", get(export_payments_csv))
}

async fn export_orders_csv(
    State(auth): State<AuthService>,
    State(service): State<AnalyticsService>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AuthError> {
    let session = require_user_session(&auth, &headers)?;
    csv_response(
        service
            .export_orders_csv(&session.email)
            .map_err(AuthError::storage)?,
    )
}

async fn export_fills_csv(
    State(auth): State<AuthService>,
    State(service): State<AnalyticsService>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AuthError> {
    let session = require_user_session(&auth, &headers)?;
    csv_response(
        service
            .export_fills_csv(&session.email)
            .map_err(AuthError::storage)?,
    )
}

async fn export_strategy_stats_csv(
    State(auth): State<AuthService>,
    State(service): State<AnalyticsService>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AuthError> {
    let session = require_user_session(&auth, &headers)?;
    csv_response(
        service
            .export_strategy_stats_csv(&session.email)
            .map_err(AuthError::storage)?,
    )
}

async fn export_payments_csv(
    State(auth): State<AuthService>,
    State(service): State<AnalyticsService>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AuthError> {
    let session = require_user_session(&auth, &headers)?;
    csv_response(
        service
            .export_payments_csv(&session.email)
            .map_err(AuthError::storage)?,
    )
}

fn csv_response(body: String) -> Result<impl IntoResponse, AuthError> {
    Ok(([(header::CONTENT_TYPE, "text/csv; charset=utf-8")], body))
}
