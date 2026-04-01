use api_server::{app_with_state, AppState};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use shared_db::SharedDb;
use tower::ServiceExt;

mod support;

use support::{login_and_get_token, register_and_login, register_and_verify};

#[tokio::test]
async fn admin_can_update_plan_config_and_new_orders_use_latest_price_and_duration() {
    let app = app_with_state(AppState::from_shared_db(SharedDb::ephemeral().expect("db")).expect("state"));
    let admin_token = register_admin_and_login(&app).await;
    let user_token = register_and_login(&app, "priced@example.com", "pass1234").await;

    let updated = upsert_plan(
        &app,
        &admin_token,
        json!({
            "code": "monthly",
            "name": "Monthly Plus",
            "duration_days": 45,
            "is_active": true,
            "prices": [
                { "chain": "BSC", "asset": "USDT", "amount": "21.50000000" },
                { "chain": "ETH", "asset": "USDT", "amount": "22.50000000" }
            ]
        }),
    )
    .await;
    assert_eq!(updated.status(), StatusCode::OK);
    let updated_body = response_json(updated).await;
    assert_eq!(updated_body["duration_days"], 45);
    assert_eq!(
        updated_body["prices"]
            .as_array()
            .expect("prices")
            .iter()
            .find(|price| price["chain"] == "BSC" && price["asset"] == "USDT")
            .expect("bsc price")["amount"],
        "21.50000000"
    );

    let listed = list_plans(&app, &admin_token).await;
    assert_eq!(listed.status(), StatusCode::OK);
    assert!(response_json(listed).await["plans"]
        .as_array()
        .expect("plans")
        .iter()
        .any(|plan| plan["code"] == "monthly" && plan["duration_days"] == 45));

    let order = create_order(
        &app,
        &user_token,
        "priced@example.com",
        "BSC",
        "USDT",
        "monthly",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(order.status(), StatusCode::CREATED);
    let order_body = response_json(order).await;
    assert_eq!(order_body["amount"], "21.50000000");

    let matched = match_order(
        &app,
        &admin_token,
        "BSC",
        "USDT",
        order_body["address"].as_str().expect("address"),
        "21.50000000",
        "tx-priced",
        "2026-04-01T00:01:00Z",
    )
    .await;
    assert_eq!(matched.status(), StatusCode::OK);
    let matched_body = response_json(matched).await;
    assert_eq!(matched_body["matched"], true);
    assert_eq!(matched_body["active_until"], "2026-05-16T00:01:00Z");
}

#[tokio::test]
async fn invalid_plan_update_does_not_partially_persist_plan_or_prices() {
    let app =
        app_with_state(AppState::from_shared_db(SharedDb::ephemeral().expect("db")).expect("state"));
    let admin_token = register_admin_and_login(&app).await;

    let seeded = list_plans(&app, &admin_token).await;
    assert_eq!(seeded.status(), StatusCode::OK);
    let before = response_json(seeded).await;
    let original = before["plans"]
        .as_array()
        .expect("plans")
        .iter()
        .find(|plan| plan["code"] == "monthly")
        .expect("monthly plan");
    assert_eq!(original["duration_days"], 30);

    let invalid = upsert_plan(
        &app,
        &admin_token,
        json!({
            "code": "monthly",
            "name": "Broken Monthly",
            "duration_days": 99,
            "is_active": true,
            "prices": [
                { "chain": "BSC", "asset": "USDT", "amount": "88.00000000" },
                { "chain": "TRON", "asset": "USDT", "amount": "77.00000000" }
            ]
        }),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

    let after_response = list_plans(&app, &admin_token).await;
    assert_eq!(after_response.status(), StatusCode::OK);
    let after = response_json(after_response).await;
    let monthly = after["plans"]
        .as_array()
        .expect("plans")
        .iter()
        .find(|plan| plan["code"] == "monthly")
        .expect("monthly plan");
    assert_eq!(monthly["name"], "Monthly");
    assert_eq!(monthly["duration_days"], 30);
    assert!(monthly["prices"]
        .as_array()
        .expect("prices")
        .iter()
        .any(|price| price["chain"] == "BSC" && price["asset"] == "USDT" && price["amount"] == "20.00000000"));
    assert!(!monthly["prices"]
        .as_array()
        .expect("prices")
        .iter()
        .any(|price| price["amount"] == "88.00000000"));
}

#[tokio::test]
async fn admin_can_expand_disable_and_list_address_pools() {
    let app = app_with_state(AppState::from_shared_db(SharedDb::ephemeral().expect("db")).expect("state"));
    let admin_token = register_admin_and_login(&app).await;

    for address in ["bsc-addr-1", "bsc-addr-2", "bsc-addr-3", "bsc-addr-4", "bsc-addr-5"] {
        let disabled = upsert_address_pool(
            &app,
            &admin_token,
            json!({
                "chain": "BSC",
                "address": address,
                "is_enabled": false
            }),
        )
        .await;
        assert_eq!(disabled.status(), StatusCode::OK);
    }

    let added = upsert_address_pool(
        &app,
        &admin_token,
        json!({
            "chain": "BSC",
            "address": "bsc-extra-1",
            "is_enabled": true
        }),
    )
    .await;
    assert_eq!(added.status(), StatusCode::OK);

    let added_second = upsert_address_pool(
        &app,
        &admin_token,
        json!({
            "chain": "BSC",
            "address": "bsc-extra-2",
            "is_enabled": false
        }),
    )
    .await;
    assert_eq!(added_second.status(), StatusCode::OK);

    let listed = list_address_pools(&app, &admin_token).await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    assert!(listed_body["addresses"]
        .as_array()
        .expect("addresses")
        .iter()
        .any(|entry| entry["address"] == "bsc-extra-1" && entry["is_enabled"] == true));
    assert!(listed_body["addresses"]
        .as_array()
        .expect("addresses")
        .iter()
        .any(|entry| entry["address"] == "bsc-extra-2" && entry["is_enabled"] == false));

    let first_token = register_and_login(&app, "pool-admin-1@example.com", "pass1234").await;
    let second_token = register_and_login(&app, "pool-admin-2@example.com", "pass1234").await;

    let first = create_order(
        &app,
        &first_token,
        "pool-admin-1@example.com",
        "BSC",
        "USDT",
        "monthly",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(first.status(), StatusCode::CREATED);
    assert_eq!(response_json(first).await["address"], "bsc-extra-1");

    let second = create_order(
        &app,
        &second_token,
        "pool-admin-2@example.com",
        "BSC",
        "USDT",
        "monthly",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(second.status(), StatusCode::CREATED);
    assert_eq!(response_json(second).await["queue_position"], 1);

    let enabled = upsert_address_pool(
        &app,
        &admin_token,
        json!({
            "chain": "BSC",
            "address": "bsc-extra-2",
            "is_enabled": true
        }),
    )
    .await;
    assert_eq!(enabled.status(), StatusCode::OK);
    let pools_after_enable = list_address_pools(&app, &admin_token).await;
    assert_eq!(pools_after_enable.status(), StatusCode::OK);
    let _ = response_json(pools_after_enable).await;

    let deposits = list_admin_deposits(&app, &admin_token, "2026-04-01T00:10:00Z").await;
    assert_eq!(deposits.status(), StatusCode::OK);
    let deposits_body = response_json(deposits).await;
    let promoted = deposits_body["orders"]
        .as_array()
        .expect("orders")
        .iter()
        .find(|order| order["email"] == "pool-admin-2@example.com")
        .expect("promoted order");
    assert_eq!(promoted["address"], "bsc-extra-2");
    assert_eq!(promoted["queue_position"], Value::Null);
}

async fn register_admin_and_login(app: &axum::Router) -> String {
    register_and_verify(app, "admin@example.com", "pass1234").await;
    let session_token = login_and_get_token(app, "admin@example.com", "pass1234").await;
    let enabled = enable_totp(app, "admin@example.com", &session_token).await;
    let totp_code = enabled["code"].as_str().expect("totp code");
    login_with_totp(app, "admin@example.com", "pass1234", totp_code).await
}

async fn enable_totp(app: &axum::Router, email: &str, session_token: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/security/totp/enable")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "email": email }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await
}

async fn login_with_totp(app: &axum::Router, email: &str, password: &str, totp_code: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "password": password,
                        "totp_code": totp_code,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await["session_token"]
        .as_str()
        .expect("session token")
        .to_owned()
}

async fn upsert_plan(app: &axum::Router, session_token: &str, payload: Value) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/memberships/plans")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn list_plans(app: &axum::Router, session_token: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin/memberships/plans")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn upsert_address_pool(
    app: &axum::Router,
    session_token: &str,
    payload: Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/address-pools")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn list_address_pools(app: &axum::Router, session_token: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin/address-pools")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn create_order(
    app: &axum::Router,
    session_token: &str,
    email: &str,
    chain: &str,
    asset: &str,
    plan_code: &str,
    requested_at: &str,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/billing/orders")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "chain": chain,
                        "asset": asset,
                        "plan_code": plan_code,
                        "requested_at": requested_at,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn match_order(
    app: &axum::Router,
    session_token: &str,
    chain: &str,
    asset: &str,
    address: &str,
    amount: &str,
    tx_hash: &str,
    observed_at: &str,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/billing/orders/match")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "chain": chain,
                        "asset": asset,
                        "address": address,
                        "amount": amount,
                        "tx_hash": tx_hash,
                        "observed_at": observed_at,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn list_admin_deposits(
    app: &axum::Router,
    session_token: &str,
    at: &str,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/admin/deposits?at={at}"))
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::empty())
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
