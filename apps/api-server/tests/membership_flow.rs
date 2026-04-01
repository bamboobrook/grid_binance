use api_server::app;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

mod support;

use support::register_and_login;

#[tokio::test]
async fn assigns_rotating_addresses_and_requires_exact_amount_match_before_activation() {
    let app = app();
    let alice_token = register_and_login(&app, "alice@example.com", "pass1234").await;
    let bob_token = register_and_login(&app, "bob@example.com", "pass1234").await;
    let admin_token = register_and_login(&app, "admin@example.com", "pass1234").await;

    let first_order = create_order(
        &app,
        &alice_token,
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
        &bob_token,
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

    let pending = membership_status(
        &app,
        &alice_token,
        "alice@example.com",
        "2026-04-01T00:10:00Z",
    )
    .await;
    assert_eq!(pending.status(), StatusCode::OK);
    assert_eq!(response_json(pending).await["status"], "Pending");

    let mismatch = match_order(
        &app,
        &admin_token,
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
        &admin_token,
        "BSC",
        "bsc-addr-1",
        "25.00000000",
        "tx-exact",
        "2026-04-01T00:14:59Z",
    )
    .await;
    assert_eq!(exact_match.status(), StatusCode::OK);
    let exact_match_body = response_json(exact_match).await;
    assert_eq!(exact_match_body["matched"], true);
    assert_eq!(exact_match_body["membership_status"], "Active");

    let active = membership_status(
        &app,
        &alice_token,
        "alice@example.com",
        "2026-04-15T00:00:00Z",
    )
    .await;
    assert_eq!(active.status(), StatusCode::OK);
    assert_eq!(response_json(active).await["status"], "Active");
}

#[tokio::test]
async fn membership_transitions_from_active_to_grace_to_expired() {
    let app = app();
    let user_token = register_and_login(&app, "grace@example.com", "pass1234").await;
    let admin_token = register_and_login(&app, "admin@example.com", "pass1234").await;

    let order = create_order(
        &app,
        &user_token,
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
        &admin_token,
        "BSC",
        "bsc-addr-1",
        "12.50000000",
        "tx-grace",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(matched.status(), StatusCode::OK);
    assert_eq!(response_json(matched).await["matched"], true);

    let active = membership_status(
        &app,
        &user_token,
        "grace@example.com",
        "2026-04-30T23:59:59Z",
    )
    .await;
    assert_eq!(active.status(), StatusCode::OK);
    assert_eq!(response_json(active).await["status"], "Active");

    let grace = membership_status(
        &app,
        &user_token,
        "grace@example.com",
        "2026-05-02T00:00:00Z",
    )
    .await;
    assert_eq!(grace.status(), StatusCode::OK);
    assert_eq!(response_json(grace).await["status"], "Grace");

    let expired = membership_status(
        &app,
        &user_token,
        "grace@example.com",
        "2026-05-05T00:00:00Z",
    )
    .await;
    assert_eq!(expired.status(), StatusCode::OK);
    assert_eq!(response_json(expired).await["status"], "Expired");
}

#[tokio::test]
async fn admin_override_can_freeze_and_revoke_membership() {
    let app = app();
    let admin_token = register_and_login(&app, "admin@example.com", "pass1234").await;

    let order = create_order(
        &app,
        &admin_token,
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
        &admin_token,
        "BSC",
        "bsc-addr-1",
        "8.00000000",
        "tx-admin",
        "2026-04-01T00:01:00Z",
    )
    .await;
    assert_eq!(matched.status(), StatusCode::OK);
    assert_eq!(response_json(matched).await["membership_status"], "Active");

    let frozen = override_membership(&app, &admin_token, "admin@example.com", Some("Frozen")).await;
    assert_eq!(frozen.status(), StatusCode::OK);
    let frozen_body = response_json(frozen).await;
    assert_eq!(frozen_body["status"], "Frozen");
    assert_eq!(frozen_body["override_status"], "Frozen");

    let cleared = override_membership(&app, &admin_token, "admin@example.com", None).await;
    assert_eq!(cleared.status(), StatusCode::OK);
    let cleared_body = response_json(cleared).await;
    assert_eq!(cleared_body["status"], "Active");
    assert_eq!(cleared_body["override_status"], Value::Null);

    let revoked =
        override_membership(&app, &admin_token, "admin@example.com", Some("Revoked")).await;
    assert_eq!(revoked.status(), StatusCode::OK);
    let revoked_body = response_json(revoked).await;
    assert_eq!(revoked_body["status"], "Revoked");
    assert_eq!(revoked_body["override_status"], "Revoked");
}

#[tokio::test]
async fn address_pool_rejects_new_order_when_all_leases_are_still_active() {
    let app = app();

    for (email, expected_address) in [
        ("lease-1@example.com", "bsc-addr-1"),
        ("lease-2@example.com", "bsc-addr-2"),
        ("lease-3@example.com", "bsc-addr-3"),
    ] {
        let user_token = register_and_login(&app, email, "pass1234").await;
        let response = create_order(
            &app,
            &user_token,
            email,
            "BSC",
            "grid-pro",
            "5.00000000",
            "2026-04-01T00:00:00Z",
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response_json(response).await["address"], expected_address);
    }

    let user_token = register_and_login(&app, "lease-4@example.com", "pass1234").await;
    let exhausted = create_order(
        &app,
        &user_token,
        "lease-4@example.com",
        "BSC",
        "grid-pro",
        "5.00000000",
        "2026-04-01T00:10:00Z",
    )
    .await;
    assert_eq!(exhausted.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(exhausted).await["error"],
        "no address available"
    );
}

#[tokio::test]
async fn expired_order_cannot_be_activated() {
    let app = app();
    let user_token = register_and_login(&app, "expired@example.com", "pass1234").await;
    let admin_token = register_and_login(&app, "admin@example.com", "pass1234").await;

    let order = create_order(
        &app,
        &user_token,
        "expired@example.com",
        "BSC",
        "grid-pro",
        "9.00000000",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(order.status(), StatusCode::CREATED);

    let expired_match = match_order(
        &app,
        &admin_token,
        "BSC",
        "bsc-addr-1",
        "9.00000000",
        "tx-expired",
        "2026-04-01T00:15:01Z",
    )
    .await;
    assert_eq!(expired_match.status(), StatusCode::OK);
    let expired_match_body = response_json(expired_match).await;
    assert_eq!(expired_match_body["matched"], false);
    assert_eq!(expired_match_body["reason"], "order_expired");

    let status = membership_status(
        &app,
        &user_token,
        "expired@example.com",
        "2026-04-01T00:16:00Z",
    )
    .await;
    assert_eq!(status.status(), StatusCode::OK);
    assert_eq!(response_json(status).await["status"], "Pending");
}

#[tokio::test]
async fn duplicate_tx_hash_cannot_activate_a_second_order() {
    let app = app();
    let first_user_token = register_and_login(&app, "dup-1@example.com", "pass1234").await;
    let second_user_token = register_and_login(&app, "dup-2@example.com", "pass1234").await;
    let admin_token = register_and_login(&app, "admin@example.com", "pass1234").await;

    let first_order = create_order(
        &app,
        &first_user_token,
        "dup-1@example.com",
        "BSC",
        "grid-pro",
        "6.00000000",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(first_order.status(), StatusCode::CREATED);

    let first_match = match_order(
        &app,
        &admin_token,
        "BSC",
        "bsc-addr-1",
        "6.00000000",
        "tx-duplicate",
        "2026-04-01T00:05:00Z",
    )
    .await;
    assert_eq!(first_match.status(), StatusCode::OK);
    assert_eq!(response_json(first_match).await["matched"], true);

    let second_order = create_order(
        &app,
        &second_user_token,
        "dup-2@example.com",
        "BSC",
        "grid-pro",
        "7.00000000",
        "2026-04-01T00:05:00Z",
    )
    .await;
    assert_eq!(second_order.status(), StatusCode::CREATED);

    let duplicate_match = match_order(
        &app,
        &admin_token,
        "BSC",
        "bsc-addr-2",
        "7.00000000",
        "tx-duplicate",
        "2026-04-01T00:06:00Z",
    )
    .await;
    assert_eq!(duplicate_match.status(), StatusCode::OK);
    let duplicate_match_body = response_json(duplicate_match).await;
    assert_eq!(duplicate_match_body["matched"], false);
    assert_eq!(duplicate_match_body["reason"], "duplicate_transaction");

    let second_status = membership_status(
        &app,
        &second_user_token,
        "dup-2@example.com",
        "2026-04-01T00:06:00Z",
    )
    .await;
    assert_eq!(second_status.status(), StatusCode::OK);
    assert_eq!(response_json(second_status).await["status"], "Pending");
}

#[tokio::test]
async fn ambiguous_same_address_and_amount_conflict_is_rejected() {
    let app = app();
    let old_token = register_and_login(&app, "ambiguous-old@example.com", "pass1234").await;
    let fill_1_token = register_and_login(&app, "ambiguous-fill-1@example.com", "pass1234").await;
    let fill_2_token = register_and_login(&app, "ambiguous-fill-2@example.com", "pass1234").await;
    let replacement_token = register_and_login(&app, "ambiguous-new@example.com", "pass1234").await;
    let admin_token = register_and_login(&app, "admin@example.com", "pass1234").await;

    for (email, user_token) in [
        ("ambiguous-old@example.com", &old_token),
        ("ambiguous-fill-1@example.com", &fill_1_token),
        ("ambiguous-fill-2@example.com", &fill_2_token),
    ] {
        let response = create_order(
            &app,
            user_token,
            email,
            "BSC",
            "grid-pro",
            if email == "ambiguous-old@example.com" {
                "11.00000000"
            } else {
                "4.00000000"
            },
            "2026-04-01T00:00:00Z",
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    let replacement = create_order(
        &app,
        &replacement_token,
        "ambiguous-new@example.com",
        "BSC",
        "grid-pro",
        "11.00000000",
        "2026-04-01T00:15:00Z",
    )
    .await;
    assert_eq!(replacement.status(), StatusCode::CREATED);
    assert_eq!(response_json(replacement).await["address"], "bsc-addr-1");

    let ambiguous_match = match_order(
        &app,
        &admin_token,
        "BSC",
        "bsc-addr-1",
        "11.00000000",
        "tx-ambiguous",
        "2026-04-01T00:15:00Z",
    )
    .await;
    assert_eq!(ambiguous_match.status(), StatusCode::OK);
    let ambiguous_match_body = response_json(ambiguous_match).await;
    assert_eq!(ambiguous_match_body["matched"], false);
    assert_eq!(ambiguous_match_body["reason"], "ambiguous_match");

    let old_status = membership_status(
        &app,
        &old_token,
        "ambiguous-old@example.com",
        "2026-04-01T00:15:00Z",
    )
    .await;
    assert_eq!(old_status.status(), StatusCode::OK);
    assert_eq!(response_json(old_status).await["status"], "Pending");

    let new_status = membership_status(
        &app,
        &replacement_token,
        "ambiguous-new@example.com",
        "2026-04-01T00:15:00Z",
    )
    .await;
    assert_eq!(new_status.status(), StatusCode::OK);
    assert_eq!(response_json(new_status).await["status"], "Pending");
}

#[tokio::test]
async fn anonymous_user_cannot_override_membership() {
    let app = app();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/membership/admin/override")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "admin@example.com",
                        "status": "Frozen",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

async fn create_order(
    app: &axum::Router,
    session_token: &str,
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
                .header("authorization", format!("Bearer {session_token}"))
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
    session_token: &str,
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
                .header("authorization", format!("Bearer {session_token}"))
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

async fn membership_status(
    app: &axum::Router,
    session_token: &str,
    email: &str,
    at: &str,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/membership/status")
                .header("authorization", format!("Bearer {session_token}"))
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
    session_token: &str,
    email: &str,
    status: Option<&str>,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/membership/admin/override")
                .header("authorization", format!("Bearer {session_token}"))
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
