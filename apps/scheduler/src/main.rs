use axum::{http::header, response::IntoResponse, routing::get, Router};
use scheduler::jobs::symbol_sync::{spawn_hourly_symbol_sync_job, SymbolSyncRuntimeState};
use std::{
    io::{Error as IoError, ErrorKind},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::net::TcpListener;

const DEFAULT_PORT: u16 = 8082;
const SERVICE_NAME: &str = "scheduler";
const DEFAULT_SYMBOL_SYNC_INTERVAL_SECS: u64 = 60 * 60;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _database_url = required_env("DATABASE_URL")?;
    let _redis_url = required_env("REDIS_URL")?;

    let symbol_sync_state = Arc::new(Mutex::new(SymbolSyncRuntimeState::default()));
    let _symbol_sync_handle = spawn_hourly_symbol_sync_job(
        Duration::from_secs(configured_symbol_sync_interval_secs()),
        symbol_sync_state,
    );

    let listener = TcpListener::bind(("0.0.0.0", configured_port(DEFAULT_PORT))).await?;
    let app = Router::new().route("/healthz", get(healthz));

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

fn configured_port(default_port: u16) -> u16 {
    parse_port(std::env::var("PORT").ok(), default_port)
}

fn configured_symbol_sync_interval_secs() -> u64 {
    std::env::var("SYMBOL_SYNC_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_SYMBOL_SYNC_INTERVAL_SECS)
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
    use super::{
        configured_symbol_sync_interval_secs, health_payload, parse_port, required_env,
        DEFAULT_PORT, DEFAULT_SYMBOL_SYNC_INTERVAL_SECS, SERVICE_NAME,
    };

    #[test]
    fn health_payload_mentions_service_name() {
        let payload = health_payload(SERVICE_NAME);

        assert!(payload.contains("service_up"));
        assert!(payload.contains("scheduler"));
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
    fn symbol_sync_interval_uses_hourly_default_and_accepts_override() {
        std::env::remove_var("SYMBOL_SYNC_INTERVAL_SECS");
        assert_eq!(
            configured_symbol_sync_interval_secs(),
            DEFAULT_SYMBOL_SYNC_INTERVAL_SECS
        );

        std::env::set_var("SYMBOL_SYNC_INTERVAL_SECS", "90");
        assert_eq!(configured_symbol_sync_interval_secs(), 90);
        std::env::remove_var("SYMBOL_SYNC_INTERVAL_SECS");
    }
}
