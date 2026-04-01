use std::{collections::HashSet, env, sync::Arc};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use shared_auth::{
    email_code::{issue_email_code, verify_email_code},
    password::{hash_password, verify_password},
    session_token::{issue_session_token, verify_session_token, SessionClaims},
    totp::{current_code, generate_secret, verify_code},
};
use shared_db::{AuditLogRecord, AuthUserRecord, SharedDb};

const DEFAULT_ADMIN_EMAILS: [&str; 1] = ["admin@example.com"];
const DEFAULT_SESSION_TOKEN_SECRET: &str = "grid-binance-dev-session-secret";

#[derive(Clone)]
pub struct AuthService {
    db: SharedDb,
    config: Arc<AuthConfig>,
}

struct AuthConfig {
    admin_emails: HashSet<String>,
    session_token_secret: String,
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

#[derive(Debug, Deserialize)]
pub struct DisableTotpRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct DisableTotpResponse {
    pub disabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct ChangePasswordResponse {
    pub password_changed: bool,
}

#[derive(Debug, Serialize)]
pub struct ProfileResponse {
    pub email: String,
    pub email_verified: bool,
    pub totp_enabled: bool,
    pub admin_totp_required: bool,
    pub admin_access_granted: bool,
}

impl AuthService {
    pub fn new(db: SharedDb) -> Self {
        Self::from_config(db, AuthConfig::from_env())
    }

    pub fn new_strict(db: SharedDb) -> Result<Self, AuthConfigError> {
        Ok(Self::from_config(db, AuthConfig::from_env_strict()?))
    }

    pub fn register(
        &self,
        request: RegisterUserRequest,
    ) -> Result<RegisterUserResponse, AuthError> {
        let email = normalize_email(&request.email);
        if email.is_empty() {
            return Err(AuthError::bad_request("email and password are required"));
        }
        validate_password(&request.password)?;

        if self
            .db
            .find_auth_user(&email)
            .map_err(AuthError::storage)?
            .is_some()
        {
            return Err(AuthError::conflict("user already exists"));
        }

        let user_id = self
            .db
            .next_sequence("auth_user_id")
            .map_err(AuthError::storage)?;
        let verification_seed = self
            .db
            .next_sequence("auth_seed")
            .map_err(AuthError::storage)?;
        let verification_code = issue_email_code(verification_seed);

        self.db
            .insert_auth_user(&AuthUserRecord {
                user_id,
                email: email.clone(),
                password_hash: hash_password(&request.password),
                email_verified: false,
                verification_code: Some(verification_code.clone()),
                reset_code: None,
                totp_secret: None,
            })
            .map_err(AuthError::storage)?;

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
        let user = self
            .db
            .find_auth_user(&email)
            .map_err(AuthError::storage)?
            .ok_or_else(|| AuthError::not_found("user not found"))?;

        let expected = user
            .verification_code
            .clone()
            .ok_or_else(|| AuthError::bad_request("email already verified"))?;

        if !verify_email_code(&expected, &request.code) {
            return Err(AuthError::unauthorized("invalid verification code"));
        }

        self.db
            .update_auth_email_verification(&email, true, None)
            .map_err(AuthError::storage)?;
        self.emit_security_audit(
            &email,
            "auth.email_verified",
            json!({ "email_verified": true }),
        )?;

        Ok(VerifyEmailResponse { verified: true })
    }

    pub fn login(&self, request: LoginRequest) -> Result<LoginResponse, AuthError> {
        let email = normalize_email(&request.email);
        let user = self
            .db
            .find_auth_user(&email)
            .map_err(AuthError::storage)?
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

        let is_admin = self.config.admin_emails.contains(&email) && user.totp_secret.is_some();
        let sid = self
            .db
            .next_sequence("auth_seed")
            .map_err(AuthError::storage)?;
        let session_token = issue_session_token(
            &self.config.session_token_secret,
            &SessionClaims {
                email: email.clone(),
                is_admin,
                sid,
            },
        )
        .map_err(|_| AuthError::unauthorized("valid session token required"))?;
        self.db
            .insert_auth_session(&session_token, &email, sid)
            .map_err(AuthError::storage)?;

        Ok(LoginResponse { session_token })
    }

    pub fn request_password_reset(
        &self,
        request: PasswordResetRequest,
    ) -> Result<PasswordResetRequestResponse, AuthError> {
        let email = normalize_email(&request.email);
        self.db
            .find_auth_user(&email)
            .map_err(AuthError::storage)?
            .ok_or_else(|| AuthError::not_found("user not found"))?;
        let reset_seed = self
            .db
            .next_sequence("auth_seed")
            .map_err(AuthError::storage)?;
        let reset_code = issue_email_code(reset_seed);

        self.db
            .set_auth_reset_code(&email, Some(&reset_code))
            .map_err(AuthError::storage)?;
        self.emit_security_audit(
            &email,
            "auth.password_reset_requested",
            json!({ "reset_requested": true }),
        )?;

        Ok(PasswordResetRequestResponse { reset_code })
    }

    pub fn confirm_password_reset(
        &self,
        request: PasswordResetConfirmRequest,
    ) -> Result<PasswordResetConfirmResponse, AuthError> {
        let email = normalize_email(&request.email);
        let user = self
            .db
            .find_auth_user(&email)
            .map_err(AuthError::storage)?
            .ok_or_else(|| AuthError::not_found("user not found"))?;

        let expected = user
            .reset_code
            .clone()
            .ok_or_else(|| AuthError::bad_request("password reset was not requested"))?;

        if !verify_email_code(&expected, &request.code) {
            return Err(AuthError::unauthorized("invalid reset code"));
        }

        validate_password(&request.new_password)?;
        self.db
            .update_auth_password(&email, &hash_password(&request.new_password))
            .map_err(AuthError::storage)?;
        self.emit_security_audit(
            &email,
            "auth.password_reset_confirmed",
            json!({ "reset_completed": true }),
        )?;

        Ok(PasswordResetConfirmResponse {
            password_reset: true,
        })
    }

    pub fn enable_totp(
        &self,
        request: EnableTotpRequest,
        session_email: &str,
    ) -> Result<EnableTotpResponse, AuthError> {
        let email = normalize_email(&request.email);
        if normalize_email(session_email) != email {
            return Err(AuthError::forbidden("session does not match user"));
        }

        let user = self
            .db
            .find_auth_user(&email)
            .map_err(AuthError::storage)?
            .ok_or_else(|| AuthError::not_found("user not found"))?;
        let totp_seed = self
            .db
            .next_sequence("auth_seed")
            .map_err(AuthError::storage)?;
        let secret = generate_secret(totp_seed);
        let code = current_code(&secret);

        if !user.email_verified {
            return Err(AuthError::unauthorized("email not verified"));
        }

        self.db
            .set_auth_totp_secret(&email, Some(&secret))
            .map_err(AuthError::storage)?;
        self.emit_security_audit(
            &email,
            "security.totp_enabled",
            json!({ "totp_enabled": true }),
        )?;

        Ok(EnableTotpResponse { secret, code })
    }

    pub fn disable_totp(
        &self,
        request: DisableTotpRequest,
        session_email: &str,
    ) -> Result<DisableTotpResponse, AuthError> {
        let email = normalize_email(&request.email);
        if normalize_email(session_email) != email {
            return Err(AuthError::forbidden("session does not match user"));
        }

        let user = self
            .db
            .find_auth_user(&email)
            .map_err(AuthError::storage)?
            .ok_or_else(|| AuthError::not_found("user not found"))?;
        if user.totp_secret.is_none() {
            return Err(AuthError::bad_request("totp is not enabled"));
        }

        self.db
            .set_auth_totp_secret(&email, None)
            .map_err(AuthError::storage)?;
        self.emit_security_audit(
            &email,
            "security.totp_disabled",
            json!({ "totp_enabled": false }),
        )?;

        Ok(DisableTotpResponse { disabled: true })
    }

    pub fn change_password(
        &self,
        session_email: &str,
        request: ChangePasswordRequest,
    ) -> Result<ChangePasswordResponse, AuthError> {
        let email = normalize_email(session_email);
        let user = self
            .db
            .find_auth_user(&email)
            .map_err(AuthError::storage)?
            .ok_or_else(|| AuthError::not_found("user not found"))?;

        if !verify_password(&request.current_password, &user.password_hash) {
            return Err(AuthError::unauthorized("invalid credentials"));
        }

        validate_password(&request.new_password)?;
        self.db
            .update_auth_password(&email, &hash_password(&request.new_password))
            .map_err(AuthError::storage)?;
        self.emit_security_audit(
            &email,
            "profile.password_changed",
            json!({ "password_changed": true }),
        )?;

        Ok(ChangePasswordResponse {
            password_changed: true,
        })
    }

    pub fn profile(
        &self,
        session_email: &str,
        admin_access_granted: bool,
    ) -> Result<ProfileResponse, AuthError> {
        let email = normalize_email(session_email);
        let user = self
            .db
            .find_auth_user(&email)
            .map_err(AuthError::storage)?
            .ok_or_else(|| AuthError::not_found("user not found"))?;
        let admin_totp_required = self.config.admin_emails.contains(&email);
        let totp_enabled = user.totp_secret.is_some();

        Ok(ProfileResponse {
            email: user.email,
            email_verified: user.email_verified,
            totp_enabled,
            admin_totp_required,
            admin_access_granted,
        })
    }

    pub fn session_claims(&self, session_token: &str) -> Result<SessionClaims, AuthError> {
        let claims = verify_session_token(&self.config.session_token_secret, session_token)
            .map_err(|_| AuthError::unauthorized("valid session token required"))?;
        let email = normalize_email(&claims.email);
        if email.is_empty() {
            return Err(AuthError::unauthorized("valid session token required"));
        }

        let session_email = self
            .db
            .find_auth_session_email(session_token)
            .map_err(AuthError::storage)?
            .ok_or_else(|| AuthError::unauthorized("valid session token required"))?;

        if session_email != email {
            return Err(AuthError::unauthorized("valid session token required"));
        }

        let totp_enabled = self
            .db
            .find_auth_user(&email)
            .map_err(AuthError::storage)?
            .map(|user| user.totp_secret.is_some())
            .unwrap_or(false);

        Ok(SessionClaims {
            email: email.clone(),
            is_admin: claims.is_admin && self.config.admin_emails.contains(&email) && totp_enabled,
            sid: claims.sid,
        })
    }

    fn from_config(db: SharedDb, config: AuthConfig) -> Self {
        Self {
            db,
            config: Arc::new(config),
        }
    }

    fn emit_security_audit(
        &self,
        actor_email: &str,
        action: &str,
        payload: serde_json::Value,
    ) -> Result<(), AuthError> {
        self.db
            .insert_audit_log(&AuditLogRecord {
                actor_email: actor_email.to_owned(),
                action: action.to_owned(),
                target_type: "user".to_owned(),
                target_id: actor_email.to_owned(),
                payload,
                created_at: Utc::now(),
            })
            .map_err(AuthError::storage)
    }
}

impl Default for AuthService {
    fn default() -> Self {
        Self::new(SharedDb::ephemeral().expect("ephemeral auth db should initialize"))
    }
}

#[derive(Debug)]
pub struct AuthError {
    pub(crate) status: StatusCode,
    pub(crate) message: &'static str,
}

#[derive(Debug)]
pub struct AuthConfigError {
    message: String,
}

impl AuthConfigError {
    fn missing(name: &'static str) -> Self {
        Self {
            message: format!("{name} must be set for persistent runtime auth"),
        }
    }
}

impl std::fmt::Display for AuthConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AuthConfigError {}

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

    pub(crate) fn storage(_error: shared_db::SharedDbError) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal storage error",
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

    fn from_env_strict() -> Result<Self, AuthConfigError> {
        Ok(Self {
            admin_emails: load_admin_emails_strict()?,
            session_token_secret: required_env("SESSION_TOKEN_SECRET")?,
        })
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

fn load_admin_emails_strict() -> Result<HashSet<String>, AuthConfigError> {
    let emails = required_env("ADMIN_EMAILS")?
        .split(',')
        .map(normalize_email)
        .filter(|email| !email.is_empty())
        .collect::<HashSet<_>>();

    if emails.is_empty() {
        Err(AuthConfigError::missing("ADMIN_EMAILS"))
    } else {
        Ok(emails)
    }
}

fn required_env(name: &'static str) -> Result<String, AuthConfigError> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AuthConfigError::missing(name))
}

fn validate_password(password: &str) -> Result<(), AuthError> {
    if password.is_empty() {
        return Err(AuthError::bad_request("email and password are required"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AuthService, LoginRequest, RegisterUserRequest, VerifyEmailRequest};
    use shared_db::SharedDb;

    #[test]
    fn auth_state_survives_service_restart() {
        let db = SharedDb::ephemeral().expect("ephemeral db");
        let service = AuthService::new(db.clone());

        let registered = service
            .register(RegisterUserRequest {
                email: "user@example.com".to_string(),
                password: "secret".to_string(),
            })
            .expect("register user");

        service
            .verify_email(VerifyEmailRequest {
                email: "user@example.com".to_string(),
                code: registered.verification_code,
            })
            .expect("verify email");

        let login = service
            .login(LoginRequest {
                email: "user@example.com".to_string(),
                password: "secret".to_string(),
                totp_code: None,
            })
            .expect("login succeeds");

        let reopened = AuthService::new(db);
        let claims = reopened
            .session_claims(&login.session_token)
            .expect("session still valid");

        assert_eq!(claims.email, "user@example.com");
    }
}
