use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};

use crate::routes::auth_guard::{require_session_email, require_user_session};
use crate::services::auth_service::{
    AuthError, AuthService, DisableTotpRequest, DisableTotpResponse, EnableTotpRequest,
    EnableTotpResponse,
};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/security/totp/enable", post(enable_totp))
        .route("/security/totp/disable", post(disable_totp))
}

async fn enable_totp(
    State(service): State<AuthService>,
    headers: HeaderMap,
    Json(request): Json<EnableTotpRequest>,
) -> Result<Json<EnableTotpResponse>, AuthError> {
    let session = require_user_session(&service, &headers)?;
    require_session_email(&session, &request.email)?;
    Ok(Json(service.enable_totp(request, &session.email)?))
}

async fn disable_totp(
    State(service): State<AuthService>,
    headers: HeaderMap,
    Json(request): Json<DisableTotpRequest>,
) -> Result<Json<DisableTotpResponse>, AuthError> {
    let session = require_user_session(&service, &headers)?;
    require_session_email(&session, &request.email)?;
    Ok(Json(service.disable_totp(request, &session.email)?))
}
