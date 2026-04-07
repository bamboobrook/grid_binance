use axum::{extract::State, http::StatusCode, routing::post, Json, Router};

use crate::services::auth_service::{
    AdminTotpBootstrapRequest, AdminTotpBootstrapResponse, AuthError, AuthService, LoginRequest,
    LoginResponse, PasswordResetConfirmRequest, PasswordResetConfirmResponse, PasswordResetRequest,
    PasswordResetRequestResponse, RegisterUserRequest, RegisterUserResponse, VerifyEmailRequest,
    VerifyEmailResponse,
};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/verify-email", post(verify_email))
        .route("/auth/login", post(login))
        .route("/auth/admin-bootstrap", post(admin_bootstrap))
        .route("/auth/password-reset/request", post(request_password_reset))
        .route("/auth/password-reset/confirm", post(confirm_password_reset))
}

async fn register(
    State(service): State<AuthService>,
    Json(request): Json<RegisterUserRequest>,
) -> Result<(StatusCode, Json<RegisterUserResponse>), AuthError> {
    let response = service.register(request)?;
    Ok((StatusCode::CREATED, Json(response)))
}

async fn verify_email(
    State(service): State<AuthService>,
    Json(request): Json<VerifyEmailRequest>,
) -> Result<Json<VerifyEmailResponse>, AuthError> {
    Ok(Json(service.verify_email(request)?))
}

async fn login(
    State(service): State<AuthService>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AuthError> {
    Ok(Json(service.login(request)?))
}

async fn admin_bootstrap(
    State(service): State<AuthService>,
    Json(request): Json<AdminTotpBootstrapRequest>,
) -> Result<Json<AdminTotpBootstrapResponse>, AuthError> {
    Ok(Json(service.bootstrap_admin_totp(request)?))
}

async fn request_password_reset(
    State(service): State<AuthService>,
    Json(request): Json<PasswordResetRequest>,
) -> Result<Json<PasswordResetRequestResponse>, AuthError> {
    Ok(Json(service.request_password_reset(request)?))
}

async fn confirm_password_reset(
    State(service): State<AuthService>,
    Json(request): Json<PasswordResetConfirmRequest>,
) -> Result<Json<PasswordResetConfirmResponse>, AuthError> {
    Ok(Json(service.confirm_password_reset(request)?))
}
