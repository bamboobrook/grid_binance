use axum::{http::header, response::IntoResponse, routing::get, Router};
use tokio::net::TcpListener;

const DEFAULT_DB_PATH: &str = "data/api-server.sqlite3";
const DEFAULT_PORT: u16 = 8080;
const SERVICE_NAME: &str = "api-server";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(("0.0.0.0", configured_port(DEFAULT_PORT))).await?;
    let app = Router::new()
        .route("/healthz", get(healthz))
        .merge(api_server::app_with_persistent_state(configured_db_path(DEFAULT_DB_PATH))?);

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

fn configured_db_path(default_path: &str) -> String {
    std::env::var("APP_DB_PATH")
        .ok()
        .map(|path| path.trim().to_owned())
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| default_path.to_owned())
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
    use super::{configured_db_path, health_payload, parse_port, DEFAULT_DB_PATH, DEFAULT_PORT, SERVICE_NAME};

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
    fn configured_db_path_uses_env_or_default() {
        std::env::remove_var("APP_DB_PATH");
        assert_eq!(configured_db_path(DEFAULT_DB_PATH), DEFAULT_DB_PATH);

        std::env::set_var("APP_DB_PATH", "/tmp/runtime.sqlite3");
        assert_eq!(
            configured_db_path(DEFAULT_DB_PATH),
            "/tmp/runtime.sqlite3".to_string()
        );
        std::env::remove_var("APP_DB_PATH");
    }
}
