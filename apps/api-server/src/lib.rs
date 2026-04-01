use std::path::Path;

mod routes {
    pub mod admin_templates;
    pub mod analytics;
    pub mod auth;
    pub mod auth_guard;
    pub mod billing;
    pub mod exchange;
    pub mod exports;
    pub mod membership;
    pub mod security;
    pub mod strategies;
    pub mod telegram;
}

mod services {
    pub mod analytics_service;
    pub mod auth_service;
    pub mod exchange_service;
    pub mod membership_service;
    pub mod strategy_service;
    pub mod telegram_service;
}

use axum::{extract::FromRef, Router};
use services::{
    analytics_service::AnalyticsService,
    auth_service::{AuthConfigError, AuthService},
    exchange_service::ExchangeService,
    membership_service::MembershipService,
    strategy_service::StrategyService,
    telegram_service::TelegramService,
};
use shared_db::{SharedDb, SharedDbError};

#[derive(Clone)]
pub struct AppState {
    analytics: AnalyticsService,
    auth: AuthService,
    exchange: ExchangeService,
    membership: MembershipService,
    strategy: StrategyService,
    telegram: TelegramService,
}

impl AppState {
    pub fn in_memory() -> Result<Self, SharedDbError> {
        Self::from_shared_db(SharedDb::in_memory()?)
    }

    pub fn persistent(path: impl AsRef<Path>) -> Result<Self, SharedDbError> {
        Self::from_shared_db(SharedDb::open(path)?)
    }

    fn from_shared_db(db: SharedDb) -> Result<Self, SharedDbError> {
        Ok(Self {
            analytics: AnalyticsService::default(),
            auth: AuthService::new(db.clone()),
            exchange: ExchangeService::default(),
            membership: MembershipService::new(db.clone()),
            strategy: StrategyService::new(db),
            telegram: TelegramService::default(),
        })
    }
}

impl FromRef<AppState> for AuthService {
    fn from_ref(input: &AppState) -> Self {
        input.auth.clone()
    }
}

impl FromRef<AppState> for AnalyticsService {
    fn from_ref(input: &AppState) -> Self {
        input.analytics.clone()
    }
}

impl FromRef<AppState> for MembershipService {
    fn from_ref(input: &AppState) -> Self {
        input.membership.clone()
    }
}

impl FromRef<AppState> for ExchangeService {
    fn from_ref(input: &AppState) -> Self {
        input.exchange.clone()
    }
}

impl FromRef<AppState> for StrategyService {
    fn from_ref(input: &AppState) -> Self {
        input.strategy.clone()
    }
}

impl FromRef<AppState> for TelegramService {
    fn from_ref(input: &AppState) -> Self {
        input.telegram.clone()
    }
}

pub fn app() -> Router {
    app_with_state(AppState::in_memory().expect("test app state should initialize"))
}

pub fn app_with_persistent_state(path: impl AsRef<Path>) -> Result<Router, AppBuildError> {
    let db = SharedDb::open(path).map_err(AppBuildError::from)?;
    let state = AppState {
        analytics: AnalyticsService::default(),
        auth: AuthService::new_strict(db.clone()).map_err(AppBuildError::from)?,
        exchange: ExchangeService::default(),
        membership: MembershipService::new(db.clone()),
        strategy: StrategyService::new(db),
        telegram: TelegramService::default(),
    };
    Ok(app_with_state(state))
}

pub fn app_with_state(state: AppState) -> Router {
    Router::new()
        .merge(routes::admin_templates::router())
        .merge(routes::analytics::router())
        .merge(routes::auth::router())
        .merge(routes::billing::router())
        .merge(routes::exchange::router())
        .merge(routes::exports::router())
        .merge(routes::membership::router())
        .merge(routes::security::router())
        .merge(routes::strategies::router())
        .merge(routes::telegram::router())
        .with_state(state)
}

#[derive(Debug)]
pub enum AppBuildError {
    Storage(SharedDbError),
    AuthConfig(AuthConfigError),
}

impl From<SharedDbError> for AppBuildError {
    fn from(value: SharedDbError) -> Self {
        Self::Storage(value)
    }
}

impl From<AuthConfigError> for AppBuildError {
    fn from(value: AuthConfigError) -> Self {
        Self::AuthConfig(value)
    }
}

impl std::fmt::Display for AppBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(error) => write!(f, "{error}"),
            Self::AuthConfig(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for AppBuildError {}

#[cfg(test)]
mod tests {
    use super::{app_with_persistent_state, AppState};
    use crate::services::auth_service::{LoginRequest, RegisterUserRequest, VerifyEmailRequest};
    use std::{
        path::PathBuf,
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn persistent_app_state_reuses_file_backed_auth_data() {
        let db_path = temp_db_path("app-state");
        let first = AppState::persistent(&db_path).expect("open first app state");
        let registered = first
            .auth
            .register(RegisterUserRequest {
                email: "persisted@app.test".to_string(),
                password: "secret".to_string(),
            })
            .expect("register user");
        first
            .auth
            .verify_email(VerifyEmailRequest {
                email: "persisted@app.test".to_string(),
                code: registered.verification_code,
            })
            .expect("verify email");
        let session = first
            .auth
            .login(LoginRequest {
                email: "persisted@app.test".to_string(),
                password: "secret".to_string(),
                totp_code: None,
            })
            .expect("login");

        let reopened = AppState::persistent(&db_path).expect("reopen app state");
        let claims = reopened
            .auth
            .session_claims(&session.session_token)
            .expect("session still exists");

        assert_eq!(claims.email, "persisted@app.test");
    }

    #[test]
    fn persistent_router_requires_runtime_auth_env() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::remove_var("ADMIN_EMAILS");
        std::env::remove_var("SESSION_TOKEN_SECRET");

        let router = app_with_persistent_state(temp_db_path("strict-app"));

        assert!(router.is_err());
    }

    fn temp_db_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic")
            .as_nanos();
        std::env::temp_dir().join(format!("grid-binance-{label}-{nonce}.sqlite3"))
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }
}
