use axum::{
    extract::State,
    http::header,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use billing_chain_listener::processor::{
    process_observed_transfer, promote_due_orders, ListenerMatchResult, ObservedChainTransfer,
    ProcessorError,
};
use shared_db::SharedDb;
use std::io::{Error as IoError, ErrorKind};
use tokio::net::TcpListener;
use tokio::time::{interval, Duration as TokioDuration};

const DEFAULT_PORT: u16 = 8084;
const SERVICE_NAME: &str = "billing-chain-listener";

#[derive(Clone)]
struct ListenerState {
    db: SharedDb,
    internal_token: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = required_env("DATABASE_URL")?;
    let redis_url = required_env("REDIS_URL")?;
    let internal_token = required_env("INTERNAL_SHARED_SECRET")?;
    let db = SharedDb::connect(&database_url, &redis_url)?;
    tokio::spawn(queue_promotion_loop(db.clone()));
    let listener = TcpListener::bind(("0.0.0.0", configured_port(DEFAULT_PORT))).await?;
    let app = build_router(ListenerState { db, internal_token });

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
    headers: axum::http::HeaderMap,
    Json(request): Json<ObservedChainTransfer>,
) -> Result<Json<ListenerMatchResult>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    authorize_internal_request(&headers, &state.internal_token)?;
    process_observed_transfer(&state.db, request)
        .map(|result| {
            let _ = promote_due_orders(&state.db, chrono::Utc::now());
            Json(result)
        })
        .map_err(map_processor_error)
}

async fn queue_promotion_loop(db: SharedDb) {
    let mut ticker = interval(TokioDuration::from_secs(30));
    loop {
        ticker.tick().await;
        let _ = promote_due_orders(&db, chrono::Utc::now());
    }
}

fn authorize_internal_request(
    headers: &axum::http::HeaderMap,
    expected_token: &str,
) -> Result<(), (axum::http::StatusCode, Json<serde_json::Value>)> {
    let supplied = headers
        .get("x-internal-token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or((
            axum::http::StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "internal token required" })),
        ))?;
    if supplied != expected_token {
        return Err((
            axum::http::StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "invalid internal token" })),
        ));
    }
    Ok(())
}

fn map_processor_error(error: ProcessorError) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    match error {
        ProcessorError::InvalidRequest(message) => (
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({ "error": message })),
        ),
        ProcessorError::Storage(storage) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": storage.to_string() })),
        ),
    }
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

fn build_router(state: ListenerState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/internal/observed-transfers", post(ingest_transfer))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::{
        build_router, configured_port, health_payload, parse_port, required_env, ListenerState,
        DEFAULT_PORT, SERVICE_NAME,
    };
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use serde_json::json;
    use shared_db::SharedDb;
    use tower::ServiceExt;

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

    #[tokio::test]
    async fn observed_transfer_endpoint_rejects_invalid_payload_with_422() {
        let app = build_router(ListenerState {
            db: SharedDb::ephemeral().expect("db"),
            internal_token: "secret".to_string(),
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/observed-transfers")
                    .header("x-internal-token", "secret")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "chain": "",
                            "asset": "USDT",
                            "address": "addr-1",
                            "amount": "1.00000000",
                            "tx_hash": "tx-1",
                            "observed_at": "2026-04-01T00:00:00Z"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = serde_json::from_slice::<serde_json::Value>(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body"),
        )
        .expect("json");
        assert_eq!(body["error"], "invalid chain");
    }
}
