mod routes {
    pub mod auth;
    pub mod security;
}

mod services {
    pub mod auth_service;
}

use axum::Router;
use services::auth_service::AuthService;

pub fn app() -> Router {
    Router::new()
        .merge(routes::auth::router())
        .merge(routes::security::router())
        .with_state(AuthService::default())
}
