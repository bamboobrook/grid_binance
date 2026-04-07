mod routes {
    pub mod admin_address_pools;
    pub mod admin_audit;
    pub mod admin_deposits;
    pub mod admin_memberships;
    pub mod admin_strategies;
    pub mod admin_sweeps;
    pub mod admin_system;
    pub mod admin_templates;
    pub mod admin_users;
    pub mod analytics;
    pub mod auth;
    pub mod auth_guard;
    pub mod billing;
    pub mod exchange;
    pub mod exports;
    pub mod membership;
    pub mod orders;
    pub mod profile;
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
    db: SharedDb,
    exchange: ExchangeService,
    membership: MembershipService,
    strategy: StrategyService,
    telegram: TelegramService,
}

impl AppState {
    pub fn ephemeral() -> Result<Self, SharedDbError> {
        Self::from_shared_db(SharedDb::ephemeral()?)
    }

    pub fn persistent(
        database_url: impl AsRef<str>,
        redis_url: impl AsRef<str>,
    ) -> Result<Self, SharedDbError> {
        let db = SharedDb::connect(database_url, redis_url)?;
        Ok(Self {
            analytics: AnalyticsService::new(db.clone()),
            auth: AuthService::new_strict(db.clone())
                .map_err(|error| SharedDbError::new(error.to_string()))?,
            db: db.clone(),
            exchange: ExchangeService::new_strict(db.clone())?,
            membership: MembershipService::new(db.clone()),
            strategy: StrategyService::new(db.clone()),
            telegram: TelegramService::new_strict(db)?,
        })
    }

    pub fn from_shared_db(db: SharedDb) -> Result<Self, SharedDbError> {
        Ok(Self {
            analytics: AnalyticsService::new(db.clone()),
            auth: AuthService::new_capture(db.clone()),
            db: db.clone(),
            exchange: ExchangeService::new(db.clone()),
            membership: MembershipService::new(db.clone()),
            strategy: StrategyService::new(db.clone()),
            telegram: TelegramService::new(db),
        })
    }
}

impl FromRef<AppState> for SharedDb {
    fn from_ref(input: &AppState) -> Self {
        input.db.clone()
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
    app_with_state(AppState::ephemeral().expect("test app state should initialize"))
}

pub fn app_with_persistent_state(
    database_url: impl AsRef<str>,
    redis_url: impl AsRef<str>,
) -> Result<Router, AppBuildError> {
    let db = SharedDb::connect(database_url, redis_url).map_err(AppBuildError::from)?;
    let auth = if std::env::var("APP_ENV").ok().as_deref() == Some("test") {
        AuthService::new_capture(db.clone())
    } else {
        AuthService::new_strict(db.clone()).map_err(AppBuildError::from)?
    };
    let state = AppState {
        analytics: AnalyticsService::new(db.clone()),
        auth,
        db: db.clone(),
        exchange: ExchangeService::new_strict(db.clone()).map_err(AppBuildError::from)?,
        membership: MembershipService::new(db.clone()),
        strategy: StrategyService::new(db.clone()),
        telegram: TelegramService::new_strict(db).map_err(AppBuildError::from)?,
    };
    Ok(app_with_state(state))
}

pub fn app_with_state(state: AppState) -> Router {
    Router::new()
        .merge(routes::admin_address_pools::router())
        .merge(routes::admin_audit::router())
        .merge(routes::admin_deposits::router())
        .merge(routes::admin_memberships::router())
        .merge(routes::admin_strategies::router())
        .merge(routes::admin_sweeps::router())
        .merge(routes::admin_system::router())
        .merge(routes::admin_templates::router())
        .merge(routes::admin_users::router())
        .merge(routes::analytics::router())
        .merge(routes::auth::router())
        .merge(routes::billing::router())
        .merge(routes::exchange::router())
        .merge(routes::exports::router())
        .merge(routes::membership::router())
        .merge(routes::orders::router())
        .merge(routes::profile::router())
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
    use std::sync::{Mutex, OnceLock};

    #[test]
    fn app_state_reuses_ephemeral_auth_data_across_service_rebuilds() {
        let db = shared_db::SharedDb::ephemeral().expect("ephemeral db");
        let first = AppState::from_shared_db(db.clone()).expect("open first app state");
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
                code: registered.verification_code.expect("verification code"),
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

        let reopened = AppState::from_shared_db(db).expect("reopen app state");
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

        let router = app_with_persistent_state(
            "postgres://grid:secret@localhost/grid",
            "redis://localhost:6379/0",
        );

        assert!(router.is_err());
    }

    #[test]
    fn persistent_router_requires_telegram_bot_secret_env() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("ADMIN_EMAILS", "admin@example.com");
        std::env::set_var("SESSION_TOKEN_SECRET", "grid-binance-dev-session-secret");
        std::env::remove_var("TELEGRAM_BOT_BIND_SECRET");

        let router = app_with_persistent_state(
            "postgres://grid:secret@localhost/grid",
            "redis://localhost:6379/0",
        );

        assert!(router.is_err());
    }

    #[test]
    fn persistent_router_rejects_invalid_database_url() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("ADMIN_EMAILS", "admin@example.com");
        std::env::set_var("SESSION_TOKEN_SECRET", "grid-binance-dev-session-secret");

        let router = app_with_persistent_state("not-a-postgres-url", "redis://localhost:6379/0");

        assert!(router.is_err());
    }

    #[test]
    fn persistent_router_rejects_invalid_redis_url() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("ADMIN_EMAILS", "admin@example.com");
        std::env::set_var("SESSION_TOKEN_SECRET", "grid-binance-dev-session-secret");

        let router =
            app_with_persistent_state("postgres://grid:secret@localhost/grid", "not-a-redis-url");

        assert!(router.is_err());
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }
}
