use axum::{
    extract::State,
    http::header,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use billing_chain_listener::processor::{process_observed_transfer, ListenerMatchResult, ObservedChainTransfer};
use shared_db::SharedDb;
use std::io::{Error as IoError, ErrorKind};
use tokio::net::TcpListener;

const DEFAULT_PORT: u16 = 8084;
const SERVICE_NAME: &str = "billing-chain-listener";

#[derive(Clone)]
struct ListenerState {
    db: SharedDb,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = required_env("DATABASE_URL")?;
    let redis_url = required_env("REDIS_URL")?;
    let db = SharedDb::connect(&database_url, &redis_url)?;
    let listener = TcpListener::bind(("0.0.0.0", configured_port(DEFAULT_PORT))).await?;
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/internal/observed-transfers", post(ingest_transfer))
        .with_state(ListenerState { db });

    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        health_payload(SERVICE_NAME),
    )
}

async fn ingest_transfer(
    State(state): State<ListenerState>,
    Json(request): Json<ObservedChainTransfer>,
) -> Result<Json<ListenerMatchResult>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    process_observed_transfer(&state.db, request)
        .map(Json)
        .map_err(|error| {
            (
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": error.to_string() })),
            )
        })
}

fn configured_port(default_port: u16) -> u16 {
    parse_port(std::env::var("PORT").ok(), default_port)
}

fn required_env(name: &str) -> Result<String, IoError> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| IoError::new(ErrorKind::InvalidInput, format!("{name} is required")))
}

fn parse_port(value: Option<String>, default_port: u16) -> u16 {
    value
        .and_then(|port| port.parse().ok())
        .unwrap_or(default_port)
}

fn health_payload(service_name: &str) -> String {
    format!(
        "# HELP service_up Service health probe status.\n# TYPE service_up gauge\nservice_up{{service=\"{service_name}\"}} 1\n"
    )
}

#[cfg(test)]
mod tests {
    use super::{configured_port, health_payload, parse_port, required_env, DEFAULT_PORT, SERVICE_NAME};

    #[test]
    fn health_payload_mentions_service_name() {
        let payload = health_payload(SERVICE_NAME);

        assert!(payload.contains("service_up"));
        assert!(payload.contains("billing-chain-listener"));
    }

    #[test]
    fn parse_port_falls_back_when_value_is_missing_or_invalid() {
        assert_eq!(parse_port(None, DEFAULT_PORT), DEFAULT_PORT);
        assert_eq!(
            parse_port(Some("not-a-port".to_string()), DEFAULT_PORT),
            DEFAULT_PORT
        );
    }

    #[test]
    fn required_env_requires_runtime_storage_urls() {
        std::env::remove_var("REDIS_URL");
        assert!(required_env("REDIS_URL").is_err());

        std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379/0");
        assert!(required_env("REDIS_URL").is_ok());
        std::env::remove_var("REDIS_URL");
    }

    #[test]
    fn configured_port_uses_default_when_missing() {
        assert_eq!(configured_port(DEFAULT_PORT), DEFAULT_PORT);
    }
}
