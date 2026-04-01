mod routes {
    pub mod admin_templates;
    pub mod auth;
    pub mod billing;
    pub mod exchange;
    pub mod membership;
    pub mod security;
    pub mod strategies;
}

mod services {
    pub mod auth_service;
    pub mod exchange_service;
    pub mod membership_service;
    pub mod strategy_service;
}

use axum::{extract::FromRef, Router};
use services::{
    auth_service::AuthService, exchange_service::ExchangeService,
    membership_service::MembershipService, strategy_service::StrategyService,
};

#[derive(Clone, Default)]
pub struct AppState {
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
        .merge(routes::auth::router())
        .merge(routes::billing::router())
        .merge(routes::exchange::router())
        .merge(routes::membership::router())
        .merge(routes::security::router())
        .merge(routes::strategies::router())
        .with_state(AppState::default())
}
