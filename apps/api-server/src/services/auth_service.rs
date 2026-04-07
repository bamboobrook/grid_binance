use std::{
    collections::HashMap,
    env,
    io::{BufRead, BufReader, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::{Arc, OnceLock},
    time::Duration as StdDuration,
};

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

const DEFAULT_OPERATOR_ADMIN_EMAILS: [&str; 1] = ["admin@example.com"];
const DEFAULT_SUPER_ADMIN_EMAILS: [&str; 1] = ["super-admin@example.com"];
const DEFAULT_SESSION_TOKEN_SECRET: &str = "grid-binance-dev-session-secret";
const DEFAULT_AUTH_EMAIL_SMTP_PORT: u16 = 25;

#[derive(Clone)]
pub struct AuthService {
    db: SharedDb,
    config: Arc<AuthConfig>,
}

struct AuthConfig {
    admin_roles: HashMap<String, AdminRole>,
    session_token_secret: String,
    email_delivery: AuthEmailDelivery,
}

enum AuthEmailDelivery {
    Capture,
    Http(HttpEmailConfig),
    Smtp(SmtpEmailConfig),
}

struct HttpEmailConfig {
    url: String,
    bearer_token: Option<String>,
    from: String,
}

struct SmtpEmailConfig {
    host: String,
    port: u16,
    helo_name: String,
    from: String,
}

struct EmailMessage {
    to: String,
    subject: String,
    body: String,
}

#[derive(Debug, Clone, Copy)]
enum EmailCodeKind {
    Verification,
    PasswordReset,
}

#[derive(Debug)]
struct EmailDeliveryError {
    message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdminRole {
    SuperAdmin,
    OperatorAdmin,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminPermissions {
    pub can_manage_memberships: bool,
    pub can_manage_plans: bool,
    pub can_manage_address_pools: bool,
    pub can_manage_templates: bool,
    pub can_manage_sweeps: bool,
    pub can_manage_system: bool,
}

#[derive(Debug, Deserialize)]
pub struct RegisterUserRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterUserResponse {
    pub user_id: u64,
    pub code_delivery: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_code: Option<String>,
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
    pub code_delivery: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_code: Option<String>,
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
pub struct AdminTotpBootstrapRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AdminTotpBootstrapResponse {
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
    pub admin_role: Option<AdminRole>,
    pub admin_permissions: Option<AdminPermissions>,
}

impl AuthService {
    pub fn new(db: SharedDb) -> Self {
        Self::from_config(db, AuthConfig::from_env())
    }

    pub fn new_capture(db: SharedDb) -> Self {
        Self::from_config(db, AuthConfig::capture())
    }

    pub fn new_strict(db: SharedDb) -> Result<Self, AuthConfigError> {
        Ok(Self::from_config(db, AuthConfig::from_env_strict()?))
    }

    fn send_email_code(
        &self,
        email: &str,
        code: &str,
        kind: EmailCodeKind,
    ) -> Result<(), AuthError> {
        let message = EmailMessage {
            to: email.to_owned(),
            subject: kind.subject().to_owned(),
            body: kind.body(code),
        };
        self.config
            .email_delivery
            .send(message)
            .map_err(|_| AuthError::internal(kind.delivery_failure_message()))
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

        if let Some(existing) = self
            .db
            .find_auth_user(&email)
            .map_err(AuthError::storage)?
        {
            let verification_seed = self
                .db
                .next_sequence("auth_seed")
                .map_err(AuthError::storage)?;
            let verification_code = issue_email_code(verification_seed);
            self.send_email_code(&email, &verification_code, EmailCodeKind::Verification)?;
            self.db
                .update_auth_email_verification(
                    &email,
                    existing.email_verified,
                    Some(&verification_code),
                )
                .map_err(AuthError::storage)?;

            return Ok(RegisterUserResponse {
                user_id: existing.user_id,
                code_delivery: "email",
                verification_code: self
                    .config
                    .email_delivery
                    .capture_code(Some(verification_code)),
            });
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

        self.send_email_code(&email, &verification_code, EmailCodeKind::Verification)?;

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
            code_delivery: "email",
            verification_code: self.config.email_delivery.capture_code(Some(verification_code)),
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
            .ok_or_else(|| AuthError::unauthorized("invalid verification code"))?;

        let expected = user
            .verification_code
            .clone()
            .ok_or_else(|| AuthError::unauthorized("invalid verification code"))?;

        if !verify_email_code(&expected, &request.code) {
            return Err(AuthError::unauthorized("invalid verification code"));
        }

        self.db
            .update_auth_email_verification_with_audit(
                &email,
                true,
                None,
                &security_audit(
                    &email,
                    "auth.email_verified",
                    json!({ "email_verified": true }),
                ),
            )
            .map_err(AuthError::storage)?;

        Ok(VerifyEmailResponse { verified: true })
    }

    pub fn login(&self, request: LoginRequest) -> Result<LoginResponse, AuthError> {
        let email = normalize_email(&request.email);
        let user = self
            .db
            .find_auth_user(&email)
            .map_err(AuthError::storage)?
            .ok_or_else(|| AuthError::unauthorized("invalid credentials"))?;

        if !user.email_verified {
            return Err(AuthError::unauthorized("invalid credentials"));
        }

        if !verify_password(&request.password, &user.password_hash) {
            return Err(AuthError::unauthorized("invalid credentials"));
        }

        let admin_role = resolved_admin_role(&self.config, &email);
        if admin_role.is_some() && user.totp_secret.is_none() {
            return Err(AuthError::unauthorized("admin totp setup required"));
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

        if let Some(role) = admin_role {
            let _ = self.db.upsert_admin_user(&email, role.as_str(), true);
        }
        let is_admin = admin_role.is_some() && user.totp_secret.is_some();
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
        let Some(_) = self.db
            .find_auth_user(&email)
            .map_err(AuthError::storage)?
        else {
            return Ok(PasswordResetRequestResponse {
                code_delivery: "email",
                reset_code: None,
            });
        };
        let reset_seed = self
            .db
            .next_sequence("auth_seed")
            .map_err(AuthError::storage)?;
        let reset_code = issue_email_code(reset_seed);

        self.db
            .set_auth_reset_code_with_audit(
                &email,
                Some(&reset_code),
                &security_audit(
                    &email,
                    "auth.password_reset_requested",
                    json!({ "reset_requested": true }),
                ),
            )
            .map_err(AuthError::storage)?;

        self.send_email_code(&email, &reset_code, EmailCodeKind::PasswordReset)?;

        Ok(PasswordResetRequestResponse {
            code_delivery: "email",
            reset_code: self.config.email_delivery.capture_code(Some(reset_code)),
        })
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
            .ok_or_else(|| AuthError::unauthorized("invalid reset code"))?;

        let expected = user
            .reset_code
            .clone()
            .ok_or_else(|| AuthError::unauthorized("invalid reset code"))?;

        if !verify_email_code(&expected, &request.code) {
            return Err(AuthError::unauthorized("invalid reset code"));
        }

        validate_password(&request.new_password)?;
        self.db
            .update_auth_password_with_audit(
                &email,
                &hash_password(&request.new_password),
                true,
                &security_audit(
                    &email,
                    "auth.password_reset_confirmed",
                    json!({ "reset_completed": true }),
                ),
            )
            .map_err(AuthError::storage)?;

        Ok(PasswordResetConfirmResponse {
            password_reset: true,
        })
    }

    pub fn bootstrap_admin_totp(
        &self,
        request: AdminTotpBootstrapRequest,
    ) -> Result<AdminTotpBootstrapResponse, AuthError> {
        let email = normalize_email(&request.email);
        let user = self
            .db
            .find_auth_user(&email)
            .map_err(AuthError::storage)?
            .ok_or_else(|| AuthError::unauthorized("admin totp bootstrap requires a verified admin account"))?;
        let admin_role = resolved_admin_role(&self.config, &email)
            .ok_or_else(|| AuthError::forbidden("admin totp bootstrap is only available for configured admin accounts"))?;

        if !user.email_verified {
            return Err(AuthError::unauthorized("admin totp bootstrap requires a verified admin account"));
        }
        if !verify_password(&request.password, &user.password_hash) {
            return Err(AuthError::unauthorized("invalid credentials"));
        }
        if user.totp_secret.is_some() {
            return Err(AuthError::conflict("admin totp is already enabled"));
        }

        let totp_seed = self
            .db
            .next_sequence("auth_seed")
            .map_err(AuthError::storage)?;
        let secret = generate_secret(totp_seed);
        let code = current_code(&secret);

        self.db
            .set_auth_totp_secret_with_audit(
                &email,
                Some(&secret),
                false,
                &security_audit(
                    &email,
                    "security.admin_totp_bootstrapped",
                    json!({
                        "admin_role": admin_role.as_str(),
                        "totp_enabled": true,
                    }),
                ),
            )
            .map_err(AuthError::storage)?;

        Ok(AdminTotpBootstrapResponse { secret, code })
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
            .set_auth_totp_secret_with_audit(
                &email,
                Some(&secret),
                false,
                &security_audit(
                    &email,
                    "security.totp_enabled",
                    json!({ "totp_enabled": true }),
                ),
            )
            .map_err(AuthError::storage)?;

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
            .set_auth_totp_secret_with_audit(
                &email,
                None,
                true,
                &security_audit(
                    &email,
                    "security.totp_disabled",
                    json!({ "totp_enabled": false }),
                ),
            )
            .map_err(AuthError::storage)?;

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
            .update_auth_password_with_audit(
                &email,
                &hash_password(&request.new_password),
                true,
                &security_audit(
                    &email,
                    "profile.password_changed",
                    json!({ "password_changed": true }),
                ),
            )
            .map_err(AuthError::storage)?;

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
        let resolved_role = resolved_admin_role(&self.config, &email);
        let admin_totp_required = resolved_role.is_some();
        let totp_enabled = user.totp_secret.is_some();
        let admin_role = admin_access_granted.then_some(resolved_role).flatten();

        Ok(ProfileResponse {
            email: user.email,
            email_verified: user.email_verified,
            totp_enabled,
            admin_totp_required,
            admin_access_granted,
            admin_role,
            admin_permissions: admin_role.map(AdminPermissions::for_role),
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
            is_admin: claims.is_admin
                && resolved_admin_role(&self.config, &email).is_some()
                && totp_enabled,
            sid: claims.sid,
        })
    }

    fn from_config(db: SharedDb, config: AuthConfig) -> Self {
        Self {
            db,
            config: Arc::new(config),
        }
    }

    pub fn admin_role_for_email(&self, email: &str) -> Option<AdminRole> {
        resolved_admin_role(&self.config, email)
    }
}

impl AdminRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SuperAdmin => "super_admin",
            Self::OperatorAdmin => "operator_admin",
        }
    }
}

impl AdminPermissions {
    fn for_role(role: AdminRole) -> Self {
        match role {
            AdminRole::SuperAdmin => Self {
                can_manage_memberships: true,
                can_manage_plans: true,
                can_manage_address_pools: true,
                can_manage_templates: true,
                can_manage_sweeps: true,
                can_manage_system: true,
            },
            AdminRole::OperatorAdmin => Self {
                can_manage_memberships: false,
                can_manage_plans: false,
                can_manage_address_pools: false,
                can_manage_templates: false,
                can_manage_sweeps: false,
                can_manage_system: false,
            },
        }
    }
}

impl EmailCodeKind {
    fn subject(self) -> &'static str {
        match self {
            Self::Verification => "Grid Binance verification code",
            Self::PasswordReset => "Grid Binance password reset code",
        }
    }

    fn body(self, code: &str) -> String {
        match self {
            Self::Verification => format!(
                "Your Grid Binance verification code is {code}.

Enter this code on the verify email page before your first login.
If you did not request this account, you can ignore this email."
            ),
            Self::PasswordReset => format!(
                "Your Grid Binance password reset code is {code}.

Enter this code on the password reset page to choose a new password.
If you did not request a reset, you can ignore this email."
            ),
        }
    }

    fn delivery_failure_message(self) -> &'static str {
        match self {
            Self::Verification => "verification email delivery failed",
            Self::PasswordReset => "password reset email delivery failed",
        }
    }
}

impl EmailDeliveryError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for EmailDeliveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for EmailDeliveryError {}

impl AuthEmailDelivery {
    fn send(&self, message: EmailMessage) -> Result<(), EmailDeliveryError> {
        match self {
            Self::Capture => Ok(()),
            Self::Http(config) => send_http_email(config, &message),
            Self::Smtp(config) => send_smtp_email(config, &message),
        }
    }

    fn capture_code(&self, code: Option<String>) -> Option<String> {
        match self {
            Self::Capture => code,
            Self::Http(_) | Self::Smtp(_) => None,
        }
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

    fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
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

    pub(crate) fn internal(message: &'static str) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
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

fn security_audit(actor_email: &str, action: &str, payload: serde_json::Value) -> AuditLogRecord {
    AuditLogRecord {
        actor_email: actor_email.to_owned(),
        action: action.to_owned(),
        target_type: "user".to_owned(),
        target_id: actor_email.to_owned(),
        payload,
        created_at: Utc::now(),
    }
}

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

fn resolved_admin_role(config: &AuthConfig, email: &str) -> Option<AdminRole> {
    let email = normalize_email(email);
    config.admin_roles.get(&email).copied().or_else(|| {
        if DEFAULT_SUPER_ADMIN_EMAILS
            .iter()
            .any(|candidate| normalize_email(candidate) == email)
        {
            Some(AdminRole::SuperAdmin)
        } else if DEFAULT_OPERATOR_ADMIN_EMAILS
            .iter()
            .any(|candidate| normalize_email(candidate) == email)
        {
            Some(AdminRole::OperatorAdmin)
        } else {
            None
        }
    })
}

impl AuthConfig {
    fn capture() -> Self {
        Self {
            admin_roles: load_admin_roles(),
            session_token_secret: env::var("SESSION_TOKEN_SECRET")
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| DEFAULT_SESSION_TOKEN_SECRET.to_owned()),
            email_delivery: AuthEmailDelivery::Capture,
        }
    }

    fn from_env() -> Self {
        Self {
            admin_roles: load_admin_roles(),
            session_token_secret: env::var("SESSION_TOKEN_SECRET")
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| DEFAULT_SESSION_TOKEN_SECRET.to_owned()),
            email_delivery: load_auth_email_delivery(false)
                .unwrap_or_else(|error| panic!("{error}")),
        }
    }

    fn from_env_strict() -> Result<Self, AuthConfigError> {
        Ok(Self {
            admin_roles: load_admin_roles_strict()?,
            session_token_secret: required_env("SESSION_TOKEN_SECRET")?,
            email_delivery: load_auth_email_delivery(true)?,
        })
    }
}

fn load_auth_email_delivery(strict_runtime: bool) -> Result<AuthEmailDelivery, AuthConfigError> {
    let mode = optional_env("AUTH_EMAIL_DELIVERY");
    let Some(mode) = mode.as_deref() else {
        if strict_runtime {
            return Err(AuthConfigError::missing("AUTH_EMAIL_DELIVERY"));
        }
        return Ok(AuthEmailDelivery::Capture);
    };

    match mode {
        "capture" if !strict_runtime => Ok(AuthEmailDelivery::Capture),
        "capture" => Err(AuthConfigError::message(
            "AUTH_EMAIL_DELIVERY must be smtp or http for persistent runtime auth",
        )),
        "http" => Ok(AuthEmailDelivery::Http(HttpEmailConfig {
            url: required_env("AUTH_EMAIL_HTTP_URL")?,
            bearer_token: optional_env("AUTH_EMAIL_HTTP_BEARER_TOKEN"),
            from: required_env("AUTH_EMAIL_FROM")?,
        })),
        "smtp" => Ok(AuthEmailDelivery::Smtp(SmtpEmailConfig {
            host: required_env("AUTH_EMAIL_SMTP_HOST")?,
            port: optional_env("AUTH_EMAIL_SMTP_PORT")
                .map(|value| {
                    value.parse::<u16>().map_err(|_| {
                        AuthConfigError::message(
                            "AUTH_EMAIL_SMTP_PORT must be a valid u16 port number",
                        )
                    })
                })
                .transpose()?
                .unwrap_or(DEFAULT_AUTH_EMAIL_SMTP_PORT),
            helo_name: optional_env("AUTH_EMAIL_SMTP_HELO_NAME")
                .unwrap_or_else(|| "localhost".to_owned()),
            from: required_env("AUTH_EMAIL_FROM")?,
        })),
        _ => Err(AuthConfigError::message(
            "AUTH_EMAIL_DELIVERY must be one of: capture, smtp, http",
        )),
    }
}

fn send_http_email(
    config: &HttpEmailConfig,
    message: &EmailMessage,
) -> Result<(), EmailDeliveryError> {
    let request = auth_email_http_client().post(&config.url);
    let request = if let Some(token) = &config.bearer_token {
        request.set("authorization", &format!("Bearer {token}"))
    } else {
        request
    };

    request
        .set("content-type", "application/json")
        .send_json(ureq::json!({
            "from": config.from,
            "to": message.to,
            "subject": message.subject,
            "text": message.body,
        }))
        .map_err(|error| EmailDeliveryError::new(format!("http email delivery failed: {error}")))?;

    Ok(())
}

fn send_smtp_email(
    config: &SmtpEmailConfig,
    message: &EmailMessage,
) -> Result<(), EmailDeliveryError> {
    let address = (config.host.as_str(), config.port)
        .to_socket_addrs()
        .map_err(|error| {
            EmailDeliveryError::new(format!("smtp address resolution failed: {error}"))
        })?
        .next()
        .ok_or_else(|| EmailDeliveryError::new("smtp address resolution returned no addresses"))?;
    let mut stream = TcpStream::connect_timeout(&address, StdDuration::from_secs(5))
        .map_err(|error| EmailDeliveryError::new(format!("smtp connect failed: {error}")))?;
    stream
        .set_read_timeout(Some(StdDuration::from_secs(5)))
        .map_err(|error| EmailDeliveryError::new(format!("smtp read timeout failed: {error}")))?;
    stream
        .set_write_timeout(Some(StdDuration::from_secs(5)))
        .map_err(|error| EmailDeliveryError::new(format!("smtp write timeout failed: {error}")))?;
    let mut reader = BufReader::new(
        stream
            .try_clone()
            .map_err(|error| EmailDeliveryError::new(format!("smtp clone failed: {error}")))?,
    );

    smtp_expect_code(&mut reader, 220)?;
    smtp_write_command(
        &mut stream,
        &mut reader,
        &format!("HELO {}", config.helo_name),
        250,
    )?;
    smtp_write_command(
        &mut stream,
        &mut reader,
        &format!("MAIL FROM:<{}>", config.from),
        250,
    )?;
    smtp_write_command(
        &mut stream,
        &mut reader,
        &format!("RCPT TO:<{}>", message.to),
        250,
    )?;
    smtp_write_command(&mut stream, &mut reader, "DATA", 354)?;
    smtp_write_message(
        &mut stream,
        &config.from,
        &message.to,
        &message.subject,
        &message.body,
    )?;
    smtp_expect_code(&mut reader, 250)?;
    smtp_write_command(&mut stream, &mut reader, "QUIT", 221)?;

    Ok(())
}

fn smtp_write_command(
    stream: &mut TcpStream,
    reader: &mut BufReader<TcpStream>,
    command: &str,
    expected_code: u16,
) -> Result<(), EmailDeliveryError> {
    stream
        .write_all(command.as_bytes())
        .map_err(|error| EmailDeliveryError::new(format!("smtp write failed: {error}")))?;
    stream
        .write_all(b"\r\n")
        .map_err(|error| EmailDeliveryError::new(format!("smtp write failed: {error}")))?;
    stream
        .flush()
        .map_err(|error| EmailDeliveryError::new(format!("smtp flush failed: {error}")))?;
    smtp_expect_code(reader, expected_code)
}

fn smtp_write_message(
    stream: &mut TcpStream,
    from: &str,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), EmailDeliveryError> {
    let escaped_body = body
        .lines()
        .map(|line| {
            if line.starts_with('.') {
                format!(".{line}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n");
    let payload = format!(
        "From: {from}\r\nTo: {to}\r\nSubject: {subject}\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\n{escaped_body}\r\n.\r\n"
    );
    stream
        .write_all(payload.as_bytes())
        .map_err(|error| EmailDeliveryError::new(format!("smtp message write failed: {error}")))?;
    stream
        .flush()
        .map_err(|error| EmailDeliveryError::new(format!("smtp flush failed: {error}")))
}

fn smtp_expect_code(
    reader: &mut BufReader<TcpStream>,
    expected_code: u16,
) -> Result<(), EmailDeliveryError> {
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|error| EmailDeliveryError::new(format!("smtp read failed: {error}")))?;
        if bytes == 0 {
            return Err(EmailDeliveryError::new(
                "smtp connection closed unexpectedly",
            ));
        }
        let code = line
            .get(0..3)
            .ok_or_else(|| EmailDeliveryError::new("smtp response missing status code"))?
            .parse::<u16>()
            .map_err(|_| EmailDeliveryError::new("smtp response contained invalid status code"))?;
        let has_more = line.as_bytes().get(3) == Some(&b'-');
        if !has_more {
            if code == expected_code {
                return Ok(());
            }
            return Err(EmailDeliveryError::new(format!(
                "smtp expected status {expected_code} but received {code}",
            )));
        }
    }
}

fn auth_email_http_client() -> &'static ureq::Agent {
    static CLIENT: OnceLock<ureq::Agent> = OnceLock::new();
    CLIENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout(StdDuration::from_secs(5))
            .build()
    })
}

fn load_admin_roles() -> HashMap<String, AdminRole> {
    let mut roles = HashMap::new();

    for email in DEFAULT_OPERATOR_ADMIN_EMAILS
        .into_iter()
        .map(normalize_email)
    {
        roles.insert(email, AdminRole::OperatorAdmin);
    }
    for email in DEFAULT_SUPER_ADMIN_EMAILS.into_iter().map(normalize_email) {
        roles.insert(email, AdminRole::SuperAdmin);
    }

    for email in env::var("ADMIN_EMAILS")
        .unwrap_or_default()
        .split(',')
        .map(normalize_email)
        .filter(|email| !email.is_empty())
    {
        roles.insert(email, AdminRole::OperatorAdmin);
    }
    for email in env::var("SUPER_ADMIN_EMAILS")
        .unwrap_or_default()
        .split(',')
        .map(normalize_email)
        .filter(|email| !email.is_empty())
    {
        roles.insert(email, AdminRole::SuperAdmin);
    }

    roles
}

fn load_admin_roles_strict() -> Result<HashMap<String, AdminRole>, AuthConfigError> {
    let mut roles = HashMap::new();

    for email in required_env("ADMIN_EMAILS")?
        .split(',')
        .map(normalize_email)
        .filter(|email| !email.is_empty())
    {
        roles.insert(email, AdminRole::OperatorAdmin);
    }
    for email in env::var("SUPER_ADMIN_EMAILS")
        .unwrap_or_default()
        .split(',')
        .map(normalize_email)
        .filter(|email| !email.is_empty())
    {
        roles.insert(email, AdminRole::SuperAdmin);
    }

    if roles.is_empty() {
        Err(AuthConfigError::missing("ADMIN_EMAILS"))
    } else {
        Ok(roles)
    }
}

fn optional_env(name: &'static str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn required_env(name: &'static str) -> Result<String, AuthConfigError> {
    optional_env(name).ok_or_else(|| AuthConfigError::missing(name))
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
                code: registered.verification_code.expect("verification code"),
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
