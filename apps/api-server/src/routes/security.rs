use axum::{
    extract::State,
    http::{header::AUTHORIZATION, HeaderMap},
    routing::post,
    Json, Router,
};

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
    let session_token = bearer_token(&headers)
        .ok_or_else(|| AuthError::unauthorized("valid session token required"))?;
    Ok(Json(service.enable_totp(request, session_token)?))
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
}
