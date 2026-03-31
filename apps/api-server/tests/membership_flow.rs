use api_server::app;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn assigns_rotating_addresses_and_requires_exact_amount_match_before_activation() {
    let app = app();

    let first_order = create_order(
        &app,
        "alice@example.com",
        "BSC",
        "grid-pro",
        "25.00000000",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(first_order.status(), StatusCode::CREATED);
    let first_order_body = response_json(first_order).await;
    assert_eq!(first_order_body["address"], "bsc-addr-1");

    let second_order = create_order(
        &app,
        "bob@example.com",
        "BSC",
        "grid-pro",
        "30.00000000",
        "2026-04-01T00:05:00Z",
    )
    .await;
    assert_eq!(second_order.status(), StatusCode::CREATED);
    let second_order_body = response_json(second_order).await;
    assert_eq!(second_order_body["address"], "bsc-addr-2");

    let pending = membership_status(&app, "alice@example.com", "2026-04-01T00:10:00Z").await;
    assert_eq!(pending.status(), StatusCode::OK);
    assert_eq!(response_json(pending).await["status"], "Pending");

    let mismatch = match_order(
        &app,
        "BSC",
        "bsc-addr-1",
        "24.99999999",
        "tx-mismatch",
        "2026-04-01T00:15:00Z",
    )
    .await;
    assert_eq!(mismatch.status(), StatusCode::OK);
    let mismatch_body = response_json(mismatch).await;
    assert_eq!(mismatch_body["matched"], false);
    assert_eq!(mismatch_body["reason"], "exact_amount_required");

    let exact_match = match_order(
        &app,
        "BSC",
        "bsc-addr-1",
        "25.00000000",
        "tx-exact",
        "2026-04-01T00:20:00Z",
    )
    .await;
    assert_eq!(exact_match.status(), StatusCode::OK);
    let exact_match_body = response_json(exact_match).await;
    assert_eq!(exact_match_body["matched"], true);
    assert_eq!(exact_match_body["membership_status"], "Active");

    let active = membership_status(&app, "alice@example.com", "2026-04-15T00:00:00Z").await;
    assert_eq!(active.status(), StatusCode::OK);
    assert_eq!(response_json(active).await["status"], "Active");
}

#[tokio::test]
async fn membership_transitions_from_active_to_grace_to_expired() {
    let app = app();

    let order = create_order(
        &app,
        "grace@example.com",
        "BSC",
        "grid-pro",
        "12.50000000",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(order.status(), StatusCode::CREATED);

    let matched = match_order(
        &app,
        "BSC",
        "bsc-addr-1",
        "12.50000000",
        "tx-grace",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(matched.status(), StatusCode::OK);
    assert_eq!(response_json(matched).await["matched"], true);

    let active = membership_status(&app, "grace@example.com", "2026-04-30T23:59:59Z").await;
    assert_eq!(active.status(), StatusCode::OK);
    assert_eq!(response_json(active).await["status"], "Active");

    let grace = membership_status(&app, "grace@example.com", "2026-05-02T00:00:00Z").await;
    assert_eq!(grace.status(), StatusCode::OK);
    assert_eq!(response_json(grace).await["status"], "Grace");

    let expired = membership_status(&app, "grace@example.com", "2026-05-05T00:00:00Z").await;
    assert_eq!(expired.status(), StatusCode::OK);
    assert_eq!(response_json(expired).await["status"], "Expired");
}

#[tokio::test]
async fn admin_override_can_freeze_and_revoke_membership() {
    let app = app();

    let order = create_order(
        &app,
        "admin@example.com",
        "BSC",
        "grid-pro",
        "8.00000000",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(order.status(), StatusCode::CREATED);

    let matched = match_order(
        &app,
        "BSC",
        "bsc-addr-1",
        "8.00000000",
        "tx-admin",
        "2026-04-01T00:01:00Z",
    )
    .await;
    assert_eq!(matched.status(), StatusCode::OK);
    assert_eq!(response_json(matched).await["membership_status"], "Active");

    let frozen = override_membership(&app, "admin@example.com", Some("Frozen")).await;
    assert_eq!(frozen.status(), StatusCode::OK);
    let frozen_body = response_json(frozen).await;
    assert_eq!(frozen_body["status"], "Frozen");
    assert_eq!(frozen_body["override_status"], "Frozen");

    let cleared = override_membership(&app, "admin@example.com", None).await;
    assert_eq!(cleared.status(), StatusCode::OK);
    let cleared_body = response_json(cleared).await;
    assert_eq!(cleared_body["status"], "Active");
    assert_eq!(cleared_body["override_status"], Value::Null);

    let revoked = override_membership(&app, "admin@example.com", Some("Revoked")).await;
    assert_eq!(revoked.status(), StatusCode::OK);
    let revoked_body = response_json(revoked).await;
    assert_eq!(revoked_body["status"], "Revoked");
    assert_eq!(revoked_body["override_status"], "Revoked");
}

async fn create_order(
    app: &axum::Router,
    email: &str,
    chain: &str,
    plan_code: &str,
    amount: &str,
    requested_at: &str,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/billing/orders")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "chain": chain,
                        "plan_code": plan_code,
                        "amount": amount,
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
    chain: &str,
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
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "chain": chain,
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

async fn membership_status(app: &axum::Router, email: &str, at: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/membership/status")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "at": at,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn override_membership(
    app: &axum::Router,
    email: &str,
    status: Option<&str>,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/membership/admin/override")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "status": status,
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
