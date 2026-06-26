use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};

use crate::{
    routes::auth_guard::require_user_session,
    services::{
        auth_service::AuthService, live_statistics_service::LiveStatisticsResponse,
        live_statistics_service::LiveStatisticsService,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/martingale-portfolios/{id}/live-stats",
        get(get_live_stats),
    )
}

async fn get_live_stats(
    State(auth): State<AuthService>,
    State(service): State<LiveStatisticsService>,
    headers: HeaderMap,
    Path(portfolio_id): Path<String>,
) -> Result<Json<LiveStatisticsResponse>, LiveStatsError> {
    let session =
        require_user_session(&auth, &headers).map_err(|_| LiveStatsError::Unauthorized)?;
    let email = session.email;

    let response = service
        .compute_portfolio_live_stats(&email, &portfolio_id)
        .map_err(|err| {
            let msg = err.to_string();
            if msg.contains("not found") || msg.contains("not owned") {
                LiveStatsError::NotFound(msg)
            } else {
                LiveStatsError::Storage(msg)
            }
        })?;

    Ok(Json(response))
}

#[derive(Debug)]
enum LiveStatsError {
    Unauthorized,
    NotFound(String),
    Storage(String),
}

impl IntoResponse for LiveStatsError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized").into_response(),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message).into_response(),
            Self::Storage(message) => (StatusCode::INTERNAL_SERVER_ERROR, message).into_response(),
        }
    }
}
