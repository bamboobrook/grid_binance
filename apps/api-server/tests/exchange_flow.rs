use api_server::app;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn save_credentials_test_connection_and_sync_symbols() {
    let app = app();

    let response = save_credentials(&app, "demo-key", "demo-secret", true).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["check"]["can_read_spot"], true);
    assert_eq!(body["check"]["can_read_futures"], true);
    assert_eq!(body["check"]["hedge_mode_ok"], true);
    assert_eq!(body["synced_symbols"], 4);
}

#[tokio::test]
async fn fuzzy_search_matches_symbol_and_market_keywords() {
    let app = app();
    let sync = save_credentials(&app, "demo-key", "demo-secret", true).await;
    assert_eq!(sync.status(), StatusCode::OK);

    let response = search_symbols(&app, "btc fut").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["items"].as_array().expect("items").len(), 1);
    assert_eq!(body["items"][0]["symbol"], "BTCUSDT");
    assert_eq!(body["items"][0]["market"], "futures");
}

#[tokio::test]
async fn hedge_mode_validation_flags_account_without_dual_side_position() {
    let app = app();

    let response = save_credentials(&app, "demo-key", "demo-secret", false).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["check"]["can_read_spot"], true);
    assert_eq!(body["check"]["can_read_futures"], true);
    assert_eq!(body["check"]["hedge_mode_ok"], false);
}

async fn save_credentials(
    app: &axum::Router,
    api_key: &str,
    api_secret: &str,
    hedge_mode_enabled: bool,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/exchange/binance/credentials")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "api_key": api_key,
                        "api_secret": api_secret,
                        "hedge_mode_enabled": hedge_mode_enabled,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn search_symbols(app: &axum::Router, query: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/exchange/binance/symbols/search")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "query": query,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    serde_json::from_slice(&bytes).expect("valid json")
}
