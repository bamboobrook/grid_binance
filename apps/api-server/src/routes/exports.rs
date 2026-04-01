use axum::{extract::State, http::header, response::IntoResponse, routing::get, Router};

use crate::{services::analytics_service::AnalyticsService, AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/exports/analytics.csv", get(export_analytics_csv))
}

async fn export_analytics_csv(State(service): State<AnalyticsService>) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
        service.export_csv(),
    )
}
