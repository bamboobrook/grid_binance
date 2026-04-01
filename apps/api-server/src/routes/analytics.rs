use axum::{extract::State, routing::get, Json, Router};

use crate::{services::analytics_service::AnalyticsService, AppState};
use shared_domain::analytics::AnalyticsReport;

pub fn router() -> Router<AppState> {
    Router::new().route("/analytics", get(get_analytics))
}

async fn get_analytics(State(service): State<AnalyticsService>) -> Json<AnalyticsReport> {
    Json(service.report())
}
