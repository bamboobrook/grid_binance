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
async fn wrong_asset_transfer_requires_manual_review_and_admin_can_credit_membership() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let state = AppState::from_shared_db(db.clone()).expect("state");
    let app = api_server::app_with_state(state);
    let user_token = register_and_login(&app, "member@example.com", "pass1234").await;
    let admin_token = register_admin_and_login(&app).await;

    let order = create_order(
        &app,
        &user_token,
        "member@example.com",
        "BSC",
        "USDT",
        "monthly",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(order.status(), StatusCode::CREATED);
    let order_body = response_json(order).await;
    let order_id = order_body["order_id"].as_u64().expect("order id");

    let abnormal = match_order(
        &app,
        &admin_token,
        "BSC",
        "USDC",
        "bsc-addr-1",
        "20.00000000",
        "tx-wrong-asset",
        "2026-04-01T00:05:00Z",
    )
    .await;
    assert_eq!(abnormal.status(), StatusCode::OK);
    let abnormal_body = response_json(abnormal).await;
    assert_eq!(abnormal_body["matched"], false);
    assert_eq!(abnormal_body["reason"], "wrong_asset");
    assert_eq!(abnormal_body["deposit_status"], "manual_review_required");

    let listed = list_admin_deposits(&app, &admin_token, "2026-04-01T00:05:00Z").await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    let deposit = listed_body["abnormal_deposits"]
        .as_array()
        .expect("abnormal deposits")
        .iter()
        .find(|record| record["tx_hash"] == "tx-wrong-asset")
        .expect("tx listed");
    assert_eq!(deposit["review_reason"], "wrong_asset");
    assert_eq!(deposit["status"], "manual_review_required");

    let credited = process_abnormal_deposit(
        &app,
        &admin_token,
        "BSC",
        "tx-wrong-asset",
        "credit_membership",
        Some(order_id),
        "2026-04-01T00:06:00Z",
    )
    .await;
    assert_eq!(credited.status(), StatusCode::OK);
    let credited_body = response_json(credited).await;
    assert_eq!(credited_body["deposit_status"], "manual_approved");
    assert_eq!(credited_body["membership_status"], "Active");

    let status = membership_status(
        &app,
        &user_token,
        "member@example.com",
        "2026-04-01T00:07:00Z",
    )
    .await;
    assert_eq!(status.status(), StatusCode::OK);
    assert_eq!(response_json(status).await["status"], "Active");

    let audit_logs = db.list_audit_logs().expect("audit logs");
    let credited_audit = audit_logs
        .iter()
        .find(|record| record.action == "deposit.manual_credited")
        .expect("credited audit");
    assert_eq!(credited_audit.actor_email, "admin@example.com");
    assert_eq!(credited_audit.payload["session_role"], "operator_admin");
    assert!(credited_audit.payload["session_sid"].as_u64().is_some());
    assert_eq!(
        credited_audit.payload["before_summary"],
        "manual_review_required wrong_asset"
    );
    assert_eq!(
        credited_audit.payload["after_summary"],
        format!("manual_approved credit_membership order {order_id}")
    );
}

#[tokio::test]
async fn admin_can_reject_abnormal_transfer_and_create_audited_sweep_jobs() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let state = AppState::from_shared_db(db.clone()).expect("state");
    let app = app_with_state(state);
    let user_token = register_and_login(&app, "treasury@example.com", "pass1234").await;
    let admin_token = register_admin_and_login(&app).await;

    let order = create_order(
        &app,
        &user_token,
        "treasury@example.com",
        "ETH",
        "USDC",
        "quarterly",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(order.status(), StatusCode::CREATED);

    let abnormal = match_order(
        &app,
        &admin_token,
        "ETH",
        "USDC",
        "eth-addr-1",
        "53.99999999",
        "tx-underpaid",
        "2026-04-01T00:05:00Z",
    )
    .await;
    assert_eq!(abnormal.status(), StatusCode::OK);
    let abnormal_body = response_json(abnormal).await;
    assert_eq!(abnormal_body["matched"], false);
    assert_eq!(abnormal_body["reason"], "exact_amount_required");
    assert_eq!(abnormal_body["deposit_status"], "manual_review_required");

    let rejected = process_abnormal_deposit(
        &app,
        &admin_token,
        "ETH",
        "tx-underpaid",
        "reject",
        None,
        "2026-04-01T00:06:00Z",
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::OK);
    assert_eq!(response_json(rejected).await["deposit_status"], "manual_rejected");

    let sweep = create_sweep_job(
        &app,
        &admin_token,
        "ETH",
        "USDC",
        "eth-treasury-1",
        "2026-04-01T00:10:00Z",
        vec![
            json!({
                "from_address": "eth-addr-1",
                "amount": "42.00000000",
            }),
            json!({
                "from_address": "eth-addr-2",
                "amount": "18.50000000",
            }),
        ],
    )
    .await;
    assert_eq!(sweep.status(), StatusCode::CREATED);
    let sweep_body = response_json(sweep).await;
    assert_eq!(sweep_body["status"], "queued");
    assert_eq!(sweep_body["transfer_count"], 2);
    assert_eq!(sweep_body["requested_by"], "admin@example.com");

    let listed = list_sweeps(&app, &admin_token).await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    let jobs = listed_body["jobs"].as_array().expect("jobs");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["chain"], "ETH");
    assert_eq!(jobs[0]["asset"], "USDC");
    assert_eq!(jobs[0]["transfer_count"], 2);

    let audit_logs = db.list_audit_logs().expect("audit logs");
    let rejected_audit = audit_logs
        .iter()
        .find(|record| record.action == "deposit.manual_rejected")
        .expect("rejected audit");
    assert_eq!(rejected_audit.actor_email, "admin@example.com");
    assert_eq!(rejected_audit.payload["session_role"], "operator_admin");
    assert!(rejected_audit.payload["session_sid"].as_u64().is_some());
    assert_eq!(
        rejected_audit.payload["before_summary"],
        "manual_review_required exact_amount_required"
    );
    assert_eq!(rejected_audit.payload["after_summary"], "manual_rejected reject");
    assert!(audit_logs
        .iter()
        .any(|record| record.action == "treasury.sweep_requested"));
}

#[tokio::test]
async fn expired_and_unmatched_transfers_create_processable_manual_review_records() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let user_token = register_and_login(&app, "expired-manual@example.com", "pass1234").await;
    let admin_token = register_admin_and_login(&app).await;

    let expired_order = create_order(
        &app,
        &user_token,
        "expired-manual@example.com",
        "BSC",
        "USDT",
        "monthly",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(expired_order.status(), StatusCode::CREATED);
    let expired_body = response_json(expired_order).await;
    let expired_order_id = expired_body["order_id"].as_u64().expect("order id");

    let expired_match = match_order(
        &app,
        &admin_token,
        "BSC",
        "USDT",
        "bsc-addr-1",
        "20.00000000",
        "tx-expired-manual",
        "2026-04-01T01:00:01Z",
    )
    .await;
    assert_eq!(expired_match.status(), StatusCode::OK);
    let expired_match_body = response_json(expired_match).await;
    assert_eq!(expired_match_body["reason"], "order_expired");
    assert_eq!(expired_match_body["deposit_status"], "manual_review_required");

    let unmatched = match_order(
        &app,
        &admin_token,
        "BSC",
        "USDT",
        "unknown-address",
        "20.00000000",
        "tx-order-not-found",
        "2026-04-01T01:10:00Z",
    )
    .await;
    assert_eq!(unmatched.status(), StatusCode::OK);
    let unmatched_body = response_json(unmatched).await;
    assert_eq!(unmatched_body["reason"], "order_not_found");
    assert_eq!(unmatched_body["deposit_status"], "manual_review_required");

    let listed = list_admin_deposits(&app, &admin_token, "2026-04-01T01:10:00Z").await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    let deposits = listed_body["abnormal_deposits"].as_array().expect("deposits");
    assert!(deposits.iter().any(|record| {
        record["tx_hash"] == "tx-expired-manual"
            && record["review_reason"] == "order_expired"
            && record["status"] == "manual_review_required"
    }));
    assert!(deposits.iter().any(|record| {
        record["tx_hash"] == "tx-order-not-found"
            && record["review_reason"] == "order_not_found"
            && record["status"] == "manual_review_required"
    }));

    let credited = process_abnormal_deposit(
        &app,
        &admin_token,
        "BSC",
        "tx-order-not-found",
        "credit_membership",
        Some(expired_order_id),
        "2026-04-01T01:11:00Z",
    )
    .await;
    assert_eq!(credited.status(), StatusCode::OK);
    let credited_body = response_json(credited).await;
    assert_eq!(credited_body["deposit_status"], "manual_approved");
    assert_eq!(credited_body["order_id"], expired_order_id);
    assert_eq!(credited_body["membership_status"], "Active");
}

#[tokio::test]
async fn ambiguous_manual_review_records_can_be_processed() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let admin_token = register_admin_and_login(&app).await;

    for email in ["amb-a@example.com", "amb-b@example.com"] {
        register_and_verify(&app, email, "pass1234").await;
    }

    db.insert_billing_order(&shared_db::BillingOrderRecord {
        order_id: 1001,
        email: "amb-a@example.com".to_string(),
        chain: "BSC".to_string(),
        asset: "USDT".to_string(),
        plan_code: "monthly".to_string(),
        amount: "20.00000000".to_string(),
        requested_at: "2026-04-01T00:00:00Z".parse().expect("time"),
        assignment: Some(shared_chain::assignment::AddressAssignment {
            chain: "BSC".to_string(),
            address: "shared-ambiguous".to_string(),
            expires_at: "2026-04-01T02:00:00Z".parse().expect("time"),
        }),
        paid_at: None,
        tx_hash: None,
        status: "pending".to_string(),
        enqueued_at: None,
    })
    .expect("insert order");
    db.insert_billing_order(&shared_db::BillingOrderRecord {
        order_id: 1002,
        email: "amb-b@example.com".to_string(),
        chain: "BSC".to_string(),
        asset: "USDT".to_string(),
        plan_code: "monthly".to_string(),
        amount: "20.00000000".to_string(),
        requested_at: "2026-04-01T00:01:00Z".parse().expect("time"),
        assignment: Some(shared_chain::assignment::AddressAssignment {
            chain: "BSC".to_string(),
            address: "shared-ambiguous".to_string(),
            expires_at: "2026-04-01T02:00:00Z".parse().expect("time"),
        }),
        paid_at: None,
        tx_hash: None,
        status: "pending".to_string(),
        enqueued_at: None,
    })
    .expect("insert order");

    let ambiguous = match_order(
        &app,
        &admin_token,
        "BSC",
        "USDT",
        "shared-ambiguous",
        "20.00000000",
        "tx-ambiguous-manual",
        "2026-04-01T00:10:00Z",
    )
    .await;
    assert_eq!(ambiguous.status(), StatusCode::OK);
    let ambiguous_body = response_json(ambiguous).await;
    assert_eq!(ambiguous_body["reason"], "ambiguous_match");
    assert_eq!(ambiguous_body["deposit_status"], "manual_review_required");

    let listed = list_admin_deposits(&app, &admin_token, "2026-04-01T00:10:00Z").await;
    assert_eq!(listed.status(), StatusCode::OK);
    assert!(response_json(listed).await["abnormal_deposits"]
        .as_array()
        .expect("deposits")
        .iter()
        .any(|record| record["tx_hash"] == "tx-ambiguous-manual" && record["review_reason"] == "ambiguous_match"));

    let rejected = process_abnormal_deposit(
        &app,
        &admin_token,
        "BSC",
        "tx-ambiguous-manual",
        "reject",
        None,
        "2026-04-01T00:11:00Z",
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::OK);
    assert_eq!(response_json(rejected).await["deposit_status"], "manual_rejected");
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
                .body(Body::from(
                    json!({
                        "email": email,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await
}

async fn login_with_totp(
    app: &axum::Router,
    email: &str,
    password: &str,
    totp_code: &str,
) -> String {
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

async fn process_abnormal_deposit(
    app: &axum::Router,
    session_token: &str,
    chain: &str,
    tx_hash: &str,
    decision: &str,
    order_id: Option<u64>,
    processed_at: &str,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/deposits/process")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "chain": chain,
                        "tx_hash": tx_hash,
                        "decision": decision,
                        "order_id": order_id,
                        "processed_at": processed_at,
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

async fn create_sweep_job(
    app: &axum::Router,
    session_token: &str,
    chain: &str,
    asset: &str,
    treasury_address: &str,
    requested_at: &str,
    transfers: Vec<Value>,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/sweeps")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "chain": chain,
                        "asset": asset,
                        "treasury_address": treasury_address,
                        "requested_at": requested_at,
                        "transfers": transfers,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn list_sweeps(app: &axum::Router, session_token: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin/sweeps")
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
