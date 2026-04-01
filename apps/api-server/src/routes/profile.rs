use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};

use crate::{
    routes::auth_guard::require_user_session,
    services::auth_service::{
        AuthError, AuthService, ChangePasswordRequest, ChangePasswordResponse, ProfileResponse,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/profile", get(read_profile))
        .route("/profile/password/change", post(change_password))
}

async fn read_profile(
    State(service): State<AuthService>,
    headers: HeaderMap,
) -> Result<Json<ProfileResponse>, AuthError> {
    let session = require_user_session(&service, &headers)?;
    Ok(Json(service.profile(&session.email)?))
}

async fn change_password(
    State(service): State<AuthService>,
    headers: HeaderMap,
    Json(request): Json<ChangePasswordRequest>,
) -> Result<Json<ChangePasswordResponse>, AuthError> {
    let session = require_user_session(&service, &headers)?;
    Ok(Json(service.change_password(&session.email, request)?))
}
