use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};

use crate::{
    routes::auth_guard::require_user_session,
    services::{
        analytics_service::AnalyticsService, auth_service::AuthError, auth_service::AuthService,
    },
    AppState,
};
use shared_domain::analytics::{AnalyticsReport, StrategyProfitSummary};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/analytics", get(get_analytics))
        .route("/analytics/overview", get(get_analytics_overview))
        .route("/analytics/strategies", get(get_strategy_summaries))
}

async fn get_analytics(
    State(auth): State<AuthService>,
    State(service): State<AnalyticsService>,
    headers: HeaderMap,
) -> Result<Json<AnalyticsReport>, AuthError> {
    let session = require_user_session(&auth, &headers)?;
    Ok(Json(
        service
            .report_for_user(&session.email)
            .map_err(AuthError::storage)?,
    ))
}

async fn get_analytics_overview(
    State(auth): State<AuthService>,
    State(service): State<AnalyticsService>,
    headers: HeaderMap,
) -> Result<Json<AnalyticsReport>, AuthError> {
    let session = require_user_session(&auth, &headers)?;
    Ok(Json(
        service
            .overview_for_user(&session.email)
            .map_err(AuthError::storage)?,
    ))
}

async fn get_strategy_summaries(
    State(auth): State<AuthService>,
    State(service): State<AnalyticsService>,
    headers: HeaderMap,
) -> Result<Json<Vec<StrategyProfitSummary>>, AuthError> {
    let session = require_user_session(&auth, &headers)?;
    Ok(Json(
        service
            .strategy_summaries_for_user(&session.email)
            .map_err(AuthError::storage)?,
    ))
}
