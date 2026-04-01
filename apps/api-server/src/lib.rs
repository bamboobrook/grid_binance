mod routes {
    pub mod admin_templates;
    pub mod analytics;
    pub mod auth;
    pub mod billing;
    pub mod exchange;
    pub mod exports;
    pub mod membership;
    pub mod security;
    pub mod strategies;
}

mod services {
    pub mod analytics_service;
    pub mod auth_service;
    pub mod exchange_service;
    pub mod membership_service;
    pub mod strategy_service;
}

use axum::{extract::FromRef, Router};
use services::{
    analytics_service::AnalyticsService, auth_service::AuthService,
    exchange_service::ExchangeService, membership_service::MembershipService,
    strategy_service::StrategyService,
};

#[derive(Clone, Default)]
pub struct AppState {
    analytics: AnalyticsService,
    auth: AuthService,
    exchange: ExchangeService,
    membership: MembershipService,
    strategy: StrategyService,
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

pub fn app() -> Router {
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
        .with_state(AppState::default())
}
