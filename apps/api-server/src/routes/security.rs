use axum::{extract::State, routing::post, Json, Router};

use crate::services::auth_service::{
    AuthError, AuthService, EnableTotpRequest, EnableTotpResponse,
};

pub fn router() -> Router<AuthService> {
    Router::new().route("/security/totp/enable", post(enable_totp))
}

async fn enable_totp(
    State(service): State<AuthService>,
    Json(request): Json<EnableTotpRequest>,
) -> Result<Json<EnableTotpResponse>, AuthError> {
    Ok(Json(service.enable_totp(request)?))
}
