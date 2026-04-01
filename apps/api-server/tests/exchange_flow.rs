use api_server::{app, app_with_state, AppState};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use shared_db::SharedDb;
use tower::ServiceExt;

#[tokio::test]
async fn save_credentials_persists_masked_account_health_and_three_market_symbol_metadata() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db).expect("app state"));
    let session_token = register_and_login(&app, "exchange-save@example.com").await;

    let response = save_credentials(
        &app,
        Some(&session_token),
        "demo-key-1234",
        "demo-secret",
        true,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["account"]["exchange"], "binance");
    assert_eq!(body["account"]["api_key_masked"], "demo****1234");
    assert_eq!(body["account"]["connection_status"], "healthy");
    assert_eq!(body["account"]["sync_status"], "success");
    assert_eq!(body["account"]["validation"]["can_read_spot"], true);
    assert_eq!(body["account"]["validation"]["can_read_usdm"], true);
    assert_eq!(body["account"]["validation"]["can_read_coinm"], true);
    assert_eq!(body["account"]["validation"]["hedge_mode_ok"], true);
    assert_eq!(body["account"]["validation"]["permissions_ok"], true);
    assert_eq!(body["account"]["symbol_counts"]["spot"], 2);
    assert_eq!(body["account"]["symbol_counts"]["usdm"], 2);
    assert_eq!(body["account"]["symbol_counts"]["coinm"], 2);
    assert_eq!(body["synced_symbols"], 6);
    assert!(body["account"]["last_checked_at"].is_string());
    assert!(body["account"]["last_synced_at"].is_string());
}

#[tokio::test]
async fn one_user_only_has_one_binance_account_and_updates_replace_the_masked_read_model() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db).expect("app state"));
    let session_token = register_and_login(&app, "exchange-single-account@example.com").await;

    let first = save_credentials(
        &app,
        Some(&session_token),
        "demo-key-1234",
        "demo-secret",
        true,
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);

    let second = save_credentials(
        &app,
        Some(&session_token),
        "next-key-5678",
        "demo-secret",
        true,
    )
    .await;

    assert_eq!(second.status(), StatusCode::OK);
    let second_body = response_json(second).await;
    assert_eq!(second_body["account"]["api_key_masked"], "next****5678");
    assert_eq!(second_body["synced_symbols"], 6);

    let read = read_account(&app, Some(&session_token)).await;
    assert_eq!(read.status(), StatusCode::OK);
    let read_body = response_json(read).await;
    assert_eq!(read_body["account"]["api_key_masked"], "next****5678");
    assert_eq!(read_body["account"]["symbol_counts"]["spot"], 2);
    assert_eq!(read_body["account"]["symbol_counts"]["usdm"], 2);
    assert_eq!(read_body["account"]["symbol_counts"]["coinm"], 2);
}

#[tokio::test]
async fn fuzzy_search_uses_persisted_symbol_metadata_after_service_rebuild() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let first_app = app_with_state(AppState::from_shared_db(db.clone()).expect("first app"));
    let session_token = register_and_login(&first_app, "exchange-search@example.com").await;
    let sync = save_credentials(
        &first_app,
        Some(&session_token),
        "demo-key-1234",
        "demo-secret",
        true,
    )
    .await;
    assert_eq!(sync.status(), StatusCode::OK);

    let rebuilt_app = app_with_state(AppState::from_shared_db(db).expect("rebuilt app"));

    let account = read_account(&rebuilt_app, Some(&session_token)).await;
    assert_eq!(account.status(), StatusCode::OK);
    let account_body = response_json(account).await;
    assert_eq!(account_body["account"]["api_key_masked"], "demo****1234");
    assert_eq!(account_body["account"]["connection_status"], "healthy");

    let response = search_symbols(&rebuilt_app, Some(&session_token), "btc coin delivery").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["items"].as_array().expect("items").len(), 1);
    assert_eq!(body["items"][0]["symbol"], "BTCUSD_PERP");
    assert_eq!(body["items"][0]["market"], "coinm");
}

#[tokio::test]
async fn hedge_mode_validation_flags_mismatch_between_expectation_and_account_state() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("first app"));
    let session_token = register_and_login(&app, "exchange-hedge@example.com").await;

    let response = save_credentials(
        &app,
        Some(&session_token),
        "demo-key-oneway-1234",
        "demo-secret",
        true,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["account"]["validation"]["can_read_spot"], true);
    assert_eq!(body["account"]["validation"]["can_read_usdm"], true);
    assert_eq!(body["account"]["validation"]["can_read_coinm"], true);
    assert_eq!(body["account"]["validation"]["hedge_mode_ok"], false);
    assert_eq!(body["account"]["connection_status"], "degraded");
    assert_eq!(body["account"]["sync_status"], "success");

    let rebuilt = app_with_state(AppState::from_shared_db(db).expect("rebuilt app"));
    let read = read_account(&rebuilt, Some(&session_token)).await;
    assert_eq!(read.status(), StatusCode::OK);
    let read_body = response_json(read).await;
    assert_eq!(read_body["account"]["validation"]["hedge_mode_ok"], false);
    assert_eq!(read_body["account"]["connection_status"], "degraded");
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
    let sync = save_credentials(
        &app,
        Some(&session_token),
        "demo-key-1234",
        "demo-secret",
        true,
    )
    .await;
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

    let read_response = read_account(&app, None).await;
    assert_eq!(read_response.status(), StatusCode::UNAUTHORIZED);

    let search_response = search_symbols(&app, None, "btc").await;
    assert_eq!(search_response.status(), StatusCode::UNAUTHORIZED);
}

async fn read_account(app: &axum::Router, session_token: Option<&str>) -> axum::response::Response {
    let mut request = Request::builder()
        .method("GET")
        .uri("/exchange/binance/account");
    if let Some(session_token) = session_token {
        request = request.header("authorization", format!("Bearer {session_token}"));
    }

    app.clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
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
