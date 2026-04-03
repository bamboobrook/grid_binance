use axum::{http::header, response::IntoResponse, routing::get, Router};
use std::io::{Error as IoError, ErrorKind};
use tokio::net::TcpListener;

const DEFAULT_PORT: u16 = 8080;
const SERVICE_NAME: &str = "api-server";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = configured_database_url()?;
    let redis_url = configured_redis_url()?;
    let listener = TcpListener::bind(("0.0.0.0", configured_port(DEFAULT_PORT))).await?;
    let app =
        Router::new()
            .route("/healthz", get(healthz))
            .merge(api_server::app_with_persistent_state(
                database_url,
                redis_url,
            )?);

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

fn configured_database_url() -> Result<String, IoError> {
    required_env("DATABASE_URL")
}

fn configured_redis_url() -> Result<String, IoError> {
    required_env("REDIS_URL")
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
        configured_database_url, configured_redis_url, health_payload, parse_port, required_env,
        DEFAULT_PORT, SERVICE_NAME,
    };

    #[test]
    fn health_payload_mentions_service_name() {
        let payload = health_payload(SERVICE_NAME);

        assert!(payload.contains("service_up"));
        assert!(payload.contains("api-server"));
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
    fn required_env_requires_non_empty_values() {
        std::env::remove_var("DATABASE_URL");
        assert!(required_env("DATABASE_URL").is_err());

        std::env::set_var("DATABASE_URL", "postgres://grid:secret@localhost/grid");
        assert_eq!(
            required_env("DATABASE_URL").expect("env should be returned"),
            "postgres://grid:secret@localhost/grid".to_string()
        );
        std::env::remove_var("DATABASE_URL");
    }

    #[test]
    fn configured_runtime_urls_read_expected_envs() {
        std::env::set_var("DATABASE_URL", "postgres://grid:secret@localhost/grid");
        std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379/0");

        assert_eq!(
            configured_database_url().expect("database url"),
            "postgres://grid:secret@localhost/grid".to_string()
        );
        assert_eq!(
            configured_redis_url().expect("redis url"),
            "redis://127.0.0.1:6379/0".to_string()
        );

        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("REDIS_URL");
    }
}
