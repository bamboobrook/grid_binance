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
    let session_token = register_and_login(&app, "exchange-save@example.com").await;

    let response =
        save_credentials(&app, Some(&session_token), "demo-key", "demo-secret", true).await;

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
    let session_token = register_and_login(&app, "exchange-search@example.com").await;
    let sync = save_credentials(&app, Some(&session_token), "demo-key", "demo-secret", true).await;
    assert_eq!(sync.status(), StatusCode::OK);

    let response = search_symbols(&app, Some(&session_token), "btc fut").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["items"].as_array().expect("items").len(), 1);
    assert_eq!(body["items"][0]["symbol"], "BTCUSDT");
    assert_eq!(body["items"][0]["market"], "futures");
}

#[tokio::test]
async fn hedge_mode_validation_flags_mismatch_between_expectation_and_account_state() {
    let app = app();
    let session_token = register_and_login(&app, "exchange-hedge@example.com").await;

    let response = save_credentials(
        &app,
        Some(&session_token),
        "demo-key-oneway",
        "demo-secret",
        true,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["check"]["can_read_spot"], true);
    assert_eq!(body["check"]["can_read_futures"], true);
    assert_eq!(body["check"]["hedge_mode_ok"], false);
}

#[tokio::test]
async fn empty_api_credentials_are_rejected() {
    let app = app();
    let session_token = register_and_login(&app, "exchange-empty-creds@example.com").await;

    let response = save_credentials(&app, Some(&session_token), "", "", true).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await["error"],
        "api_key and api_secret are required"
    );
}

#[tokio::test]
async fn empty_symbol_query_is_rejected() {
    let app = app();
    let session_token = register_and_login(&app, "exchange-empty-query@example.com").await;
    let sync = save_credentials(&app, Some(&session_token), "demo-key", "demo-secret", true).await;
    assert_eq!(sync.status(), StatusCode::OK);

    let response = search_symbols(&app, Some(&session_token), "   ").await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response_json(response).await["error"], "query is required");
}

#[tokio::test]
async fn unauthenticated_requests_to_exchange_api_are_rejected() {
    let app = app();

    let save_response = save_credentials(&app, None, "demo-key", "demo-secret", true).await;
    assert_eq!(save_response.status(), StatusCode::UNAUTHORIZED);

    let search_response = search_symbols(&app, None, "btc").await;
    assert_eq!(search_response.status(), StatusCode::UNAUTHORIZED);
}

async fn save_credentials(
    app: &axum::Router,
    session_token: Option<&str>,
    api_key: &str,
    api_secret: &str,
    expected_hedge_mode: bool,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method("POST")
        .uri("/exchange/binance/credentials")
        .header("content-type", "application/json");
    if let Some(session_token) = session_token {
        request = request.header("authorization", format!("Bearer {session_token}"));
    }

    app.clone()
        .oneshot(
            request
                .body(Body::from(
                    json!({
                        "api_key": api_key,
                        "api_secret": api_secret,
                        "expected_hedge_mode": expected_hedge_mode,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn search_symbols(
    app: &axum::Router,
    session_token: Option<&str>,
    query: &str,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method("POST")
        .uri("/exchange/binance/symbols/search")
        .header("content-type", "application/json");
    if let Some(session_token) = session_token {
        request = request.header("authorization", format!("Bearer {session_token}"));
    }

    app.clone()
        .oneshot(
            request
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

async fn register_and_login(app: &axum::Router, email: &str) -> String {
    let register = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "password": "pass1234",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(register.status(), StatusCode::CREATED);
    let verification_code = response_json(register).await["verification_code"]
        .as_str()
        .expect("verification code")
        .to_owned();

    let verify = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/verify-email")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "code": verification_code,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(verify.status(), StatusCode::OK);

    let login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "password": "pass1234",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::OK);

    response_json(login).await["session_token"]
        .as_str()
        .expect("session token")
        .to_owned()
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    serde_json::from_slice(&bytes).expect("valid json")
}
