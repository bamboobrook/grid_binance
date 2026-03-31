mod routes {
    pub mod auth;
    pub mod billing;
    pub mod membership;
    pub mod security;
}

mod services {
    pub mod auth_service;
    pub mod membership_service;
}

use axum::{extract::FromRef, Router};
use services::{auth_service::AuthService, membership_service::MembershipService};

#[derive(Clone, Default)]
pub struct AppState {
    auth: AuthService,
    membership: MembershipService,
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

pub fn app() -> Router {
    Router::new()
        .merge(routes::auth::router())
        .merge(routes::billing::router())
        .merge(routes::membership::router())
        .merge(routes::security::router())
        .with_state(AppState::default())
}
