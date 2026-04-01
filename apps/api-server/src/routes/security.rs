use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};

use crate::routes::auth_guard::require_user_session;
use crate::services::auth_service::{
    AuthError, AuthService, EnableTotpRequest, EnableTotpResponse,
};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/security/totp/enable", post(enable_totp))
}

async fn enable_totp(
    State(service): State<AuthService>,
    headers: HeaderMap,
    Json(request): Json<EnableTotpRequest>,
) -> Result<Json<EnableTotpResponse>, AuthError> {
    let session = require_user_session(&service, &headers)?;
    let session_token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| AuthError::unauthorized("valid session token required"))?;
    if session.email != request.email.trim().to_lowercase() {
        return Err(AuthError::forbidden("session does not match user"));
    }
    Ok(Json(service.enable_totp(request, session_token)?))
}
