use std::{
    collections::{HashMap, HashSet},
    env,
    sync::{Arc, Mutex},
};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use shared_auth::{
    email_code::{issue_email_code, verify_email_code},
    password::{hash_password, verify_password},
    session_token::{issue_session_token, verify_session_token, SessionClaims},
    totp::{current_code, generate_secret, verify_code},
};

const DEFAULT_ADMIN_EMAILS: [&str; 1] = ["admin@example.com"];
const DEFAULT_SESSION_TOKEN_SECRET: &str = "grid-binance-dev-session-secret";

#[derive(Clone)]
pub struct AuthService {
    inner: Arc<Mutex<AuthState>>,
    config: Arc<AuthConfig>,
}

#[derive(Default)]
struct AuthState {
    next_user_id: u64,
    next_seed: u64,
    users: HashMap<String, UserRecord>,
    sessions: HashMap<String, String>,
}

struct AuthConfig {
    admin_emails: HashSet<String>,
    session_token_secret: String,
}

struct UserRecord {
    password_hash: String,
    email_verified: bool,
    verification_code: Option<String>,
    reset_code: Option<String>,
    totp_secret: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterUserRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterUserResponse {
    pub user_id: u64,
    pub verification_code: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailRequest {
    pub email: String,
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyEmailResponse {
    pub verified: bool,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    pub totp_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub session_token: String,
}

#[derive(Debug, Deserialize)]
pub struct PasswordResetRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct PasswordResetRequestResponse {
    pub reset_code: String,
}

#[derive(Debug, Deserialize)]
pub struct PasswordResetConfirmRequest {
    pub email: String,
    pub code: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct PasswordResetConfirmResponse {
    pub password_reset: bool,
}

#[derive(Debug, Deserialize)]
pub struct EnableTotpRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct EnableTotpResponse {
    pub secret: String,
    pub code: String,
}

impl AuthService {
    pub fn register(
        &self,
        request: RegisterUserRequest,
    ) -> Result<RegisterUserResponse, AuthError> {
        let email = normalize_email(&request.email);
        if email.is_empty() {
            return Err(AuthError::bad_request("email and password are required"));
        }
        validate_password(&request.password)?;

        let mut inner = self.inner.lock().expect("auth state poisoned");
        if inner.users.contains_key(&email) {
            return Err(AuthError::conflict("user already exists"));
        }

        inner.next_user_id += 1;
        inner.next_seed += 1;

        let user_id = inner.next_user_id;
        let verification_code = issue_email_code(inner.next_seed);

        inner.users.insert(
            email,
            UserRecord {
                password_hash: hash_password(&request.password),
                email_verified: false,
                verification_code: Some(verification_code.clone()),
                reset_code: None,
                totp_secret: None,
            },
        );

        Ok(RegisterUserResponse {
            user_id,
            verification_code,
        })
    }

    pub fn verify_email(
        &self,
        request: VerifyEmailRequest,
    ) -> Result<VerifyEmailResponse, AuthError> {
        let email = normalize_email(&request.email);
        let mut inner = self.inner.lock().expect("auth state poisoned");
        let user = inner
            .users
            .get_mut(&email)
            .ok_or_else(|| AuthError::not_found("user not found"))?;

        let expected = user
            .verification_code
            .clone()
            .ok_or_else(|| AuthError::bad_request("email already verified"))?;

        if !verify_email_code(&expected, &request.code) {
            return Err(AuthError::unauthorized("invalid verification code"));
        }

        user.email_verified = true;
        user.verification_code = None;

        Ok(VerifyEmailResponse { verified: true })
    }

    pub fn login(&self, request: LoginRequest) -> Result<LoginResponse, AuthError> {
        let email = normalize_email(&request.email);
        let mut inner = self.inner.lock().expect("auth state poisoned");
        let user = inner
            .users
            .get(&email)
            .ok_or_else(|| AuthError::not_found("user not found"))?;

        if !user.email_verified {
            return Err(AuthError::unauthorized("email not verified"));
        }

        if !verify_password(&request.password, &user.password_hash) {
            return Err(AuthError::unauthorized("invalid credentials"));
        }

        if let Some(secret) = &user.totp_secret {
            let code = request
                .totp_code
                .as_deref()
                .ok_or_else(|| AuthError::unauthorized("totp code required"))?;

            if !verify_code(secret, code) {
                return Err(AuthError::unauthorized("invalid totp code"));
            }
        }

        let is_admin = self.config.admin_emails.contains(&email);
        inner.next_seed += 1;
        let session_token = issue_session_token(
            &self.config.session_token_secret,
            &SessionClaims {
                email: email.clone(),
                is_admin,
                sid: inner.next_seed,
            },
        )
        .map_err(|_| AuthError::unauthorized("valid session token required"))?;
        inner.sessions.insert(session_token.clone(), email);

        Ok(LoginResponse { session_token })
    }

    pub fn request_password_reset(
        &self,
        request: PasswordResetRequest,
    ) -> Result<PasswordResetRequestResponse, AuthError> {
        let email = normalize_email(&request.email);
        let mut inner = self.inner.lock().expect("auth state poisoned");
        inner.next_seed += 1;
        let reset_code = issue_email_code(inner.next_seed);

        let user = inner
            .users
            .get_mut(&email)
            .ok_or_else(|| AuthError::not_found("user not found"))?;

        user.reset_code = Some(reset_code.clone());

        Ok(PasswordResetRequestResponse { reset_code })
    }

    pub fn confirm_password_reset(
        &self,
        request: PasswordResetConfirmRequest,
    ) -> Result<PasswordResetConfirmResponse, AuthError> {
        let email = normalize_email(&request.email);
        let mut inner = self.inner.lock().expect("auth state poisoned");
        let user = inner
            .users
            .get_mut(&email)
            .ok_or_else(|| AuthError::not_found("user not found"))?;

        let expected = user
            .reset_code
            .clone()
            .ok_or_else(|| AuthError::bad_request("password reset was not requested"))?;

        if !verify_email_code(&expected, &request.code) {
            return Err(AuthError::unauthorized("invalid reset code"));
        }

        validate_password(&request.new_password)?;
        user.password_hash = hash_password(&request.new_password);
        user.reset_code = None;

        Ok(PasswordResetConfirmResponse {
            password_reset: true,
        })
    }

    pub fn enable_totp(
        &self,
        request: EnableTotpRequest,
        session_token: &str,
    ) -> Result<EnableTotpResponse, AuthError> {
        let email = normalize_email(&request.email);
        let session_claims = self.session_claims(session_token)?;
        if session_claims.email != email {
            return Err(AuthError::forbidden("session does not match user"));
        }

        let mut inner = self.inner.lock().expect("auth state poisoned");
        inner.next_seed += 1;
        let secret = generate_secret(inner.next_seed);
        let code = current_code(&secret);

        let user = inner
            .users
            .get_mut(&email)
            .ok_or_else(|| AuthError::not_found("user not found"))?;

        if !user.email_verified {
            return Err(AuthError::unauthorized("email not verified"));
        }

        user.totp_secret = Some(secret.clone());

        Ok(EnableTotpResponse { secret, code })
    }

    pub fn session_claims(&self, session_token: &str) -> Result<SessionClaims, AuthError> {
        let claims = verify_session_token(&self.config.session_token_secret, session_token)
            .map_err(|_| AuthError::unauthorized("valid session token required"))?;
        let email = normalize_email(&claims.email);
        if email.is_empty() {
            return Err(AuthError::unauthorized("valid session token required"));
        }

        let inner = self.inner.lock().expect("auth state poisoned");
        let session_email = inner
            .sessions
            .get(session_token)
            .ok_or_else(|| AuthError::unauthorized("valid session token required"))?;

        if session_email != &email {
            return Err(AuthError::unauthorized("valid session token required"));
        }

        Ok(SessionClaims {
            email: email.clone(),
            is_admin: claims.is_admin && self.config.admin_emails.contains(&email),
            sid: claims.sid,
        })
    }
}

impl Default for AuthService {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(AuthState::default())),
            config: Arc::new(AuthConfig::from_env()),
        }
    }
}

#[derive(Debug)]
pub struct AuthError {
    pub(crate) status: StatusCode,
    pub(crate) message: &'static str,
}

impl AuthError {
    pub(crate) fn bad_request(message: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message,
        }
    }

    pub(crate) fn conflict(message: &'static str) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message,
        }
    }

    pub(crate) fn not_found(message: &'static str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message,
        }
    }

    pub(crate) fn unauthorized(message: &'static str) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message,
        }
    }

    pub(crate) fn forbidden(message: &'static str) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message,
        }
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.message.to_owned(),
            }),
        )
            .into_response()
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

impl AuthConfig {
    fn from_env() -> Self {
        Self {
            admin_emails: load_admin_emails(),
            session_token_secret: env::var("SESSION_TOKEN_SECRET")
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| DEFAULT_SESSION_TOKEN_SECRET.to_owned()),
        }
    }
}

fn load_admin_emails() -> HashSet<String> {
    let configured = env::var("ADMIN_EMAILS").ok();
    let emails = configured
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(normalize_email)
        .filter(|email| !email.is_empty())
        .collect::<HashSet<_>>();

    if emails.is_empty() {
        DEFAULT_ADMIN_EMAILS
            .into_iter()
            .map(normalize_email)
            .collect()
    } else {
        emails
    }
}

fn validate_password(password: &str) -> Result<(), AuthError> {
    if password.is_empty() {
        return Err(AuthError::bad_request("email and password are required"));
    }

    Ok(())
}
