use api_server::{app_with_state, AppState};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};

use shared_db::SharedDb;
use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Barrier},
    thread::{self, sleep},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tower::ServiceExt;

mod support;

use support::{login_and_get_token, register_and_login, register_and_verify};

const MANUAL_CREDIT_CONFIRMATION: &str = "MANUAL_CREDIT_MEMBERSHIP";

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

    let missing_confirmation = process_abnormal_deposit(
        &app,
        &admin_token,
        "BSC",
        "tx-wrong-asset",
        "credit_membership",
        Some(order_id),
        None,
        Some("operator reviewed wrong-asset transfer and validated order ownership"),
        "2026-04-01T00:06:00Z",
    )
    .await;
    assert_eq!(missing_confirmation.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(missing_confirmation).await["error"],
        "manual credit confirmation is required"
    );

    let missing_justification = process_abnormal_deposit(
        &app,
        &admin_token,
        "BSC",
        "tx-wrong-asset",
        "credit_membership",
        Some(order_id),
        Some(MANUAL_CREDIT_CONFIRMATION),
        Some("   "),
        "2026-04-01T00:06:30Z",
    )
    .await;
    assert_eq!(missing_justification.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(missing_justification).await["error"],
        "manual credit justification is required"
    );

    let credited = process_abnormal_deposit(
        &app,
        &admin_token,
        "BSC",
        "tx-wrong-asset",
        "credit_membership",
        Some(order_id),
        Some(MANUAL_CREDIT_CONFIRMATION),
        Some("operator reviewed wrong-asset transfer and validated order ownership"),
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
    assert_eq!(
        credited_audit.payload["confirmation"],
        MANUAL_CREDIT_CONFIRMATION
    );
    assert_eq!(
        credited_audit.payload["justification"],
        "operator reviewed wrong-asset transfer and validated order ownership"
    );

    let notifications = db.list_notification_logs("member@example.com", 10).expect("notification logs");
    let deposit_notice = notifications
        .iter()
        .find(|record| record.template_key.as_deref() == Some("DepositConfirmed") && record.channel == "in_app")
        .expect("deposit confirmation notification");
    assert_eq!(deposit_notice.title, "Deposit confirmed");
    assert_eq!(deposit_notice.payload["event"]["payload"]["order_id"], order_id.to_string());
    assert_eq!(deposit_notice.payload["event"]["payload"]["tx_hash"], "tx-wrong-asset");
}

#[tokio::test]
async fn operator_admin_can_reject_abnormal_transfer_but_cannot_create_sweep_jobs() {
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
        None,
        None,
        "2026-04-01T00:06:00Z",
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::OK);
    assert_eq!(
        response_json(rejected).await["deposit_status"],
        "manual_rejected"
    );

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
    assert_eq!(sweep.status(), StatusCode::FORBIDDEN);

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
    assert_eq!(
        rejected_audit.payload["after_summary"],
        "manual_rejected reject"
    );
    assert!(!audit_logs
        .iter()
        .any(|record| record.action == "treasury.sweep_requested"));
}

#[tokio::test]
async fn exact_match_does_not_persist_payment_when_audit_write_fails() {
    let server = ApiServerHarness::start("exact-match-audit");
    let user_token = register_and_login_via_http(&server, "member@example.com", "pass1234");
    let super_admin_token =
        register_privileged_admin_and_login_via_http(&server, "super-admin@example.com");

    let (order_status, order_body) = http_json(
        "POST",
        &format!("{}/billing/orders", server.base_url()),
        Some(&user_token),
        Some(json!({
            "email": "member@example.com",
            "chain": "BSC",
            "asset": "USDT",
            "plan_code": "monthly",
            "requested_at": "2026-04-01T00:00:00Z"
        })),
    );
    assert_eq!(order_status, StatusCode::CREATED.as_u16());
    let order_id = order_body["order_id"].as_u64().expect("order id");
    let order_address = order_body["address"].as_str().expect("order address");
    let database_url = server.runtime.database_url();
    let redis_url = server.runtime.redis_url();
    let observed_address = order_address.to_string();
    tokio::task::spawn_blocking(move || {
        let db = SharedDb::connect(&database_url, &redis_url).expect("persistent db");
        db.upsert_deposit_transaction(&shared_db::DepositTransactionRecord {
            tx_hash: "tx-exact-audit-fail".to_string(),
            chain: "BSC".to_string(),
            asset: "USDT".to_string(),
            address: observed_address,
            amount: "20.00000000".to_string(),
            observed_at: "2026-04-01T00:05:00Z".parse().expect("observed_at"),
            order_id: Some(order_id),
            status: "confirming".to_string(),
            review_reason: Some("awaiting_confirmations".to_string()),
            processed_at: None,
            matched_order_id: None,
        }).expect("confirming deposit");
    }).await.expect("seed confirming deposit");

    server.break_audit_table();

    let (match_status, match_body) = http_json(
        "POST",
        &format!("{}/billing/orders/match", server.base_url()),
        Some(&super_admin_token),
        Some(json!({
            "chain": "BSC",
            "asset": "USDT",
            "address": order_address,
            "amount": "20.00000000",
            "tx_hash": "tx-exact-audit-fail",
            "confirmations": 12,
            "observed_at": "2026-04-01T00:05:00Z"
        })),
    );
    assert_eq!(match_status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());
    assert!(match_body["error"].is_string());

    let (status_status, status_body) = http_json(
        "POST",
        &format!("{}/membership/status", server.base_url()),
        Some(&user_token),
        Some(json!({
            "email": "member@example.com",
            "at": "2026-04-01T00:06:00Z"
        })),
    );
    assert_eq!(status_status, StatusCode::OK.as_u16());
    assert_eq!(status_body["status"], "Pending");

    let (deposits_status, deposits_body) = http_json(
        "GET",
        &format!(
            "{}/admin/deposits?at={}",
            server.base_url(),
            "2026-04-01T00:06:00Z"
        ),
        Some(&super_admin_token),
        None,
    );
    assert_eq!(deposits_status, StatusCode::OK.as_u16());
    let abnormal = deposits_body["abnormal_deposits"]
        .as_array()
        .expect("abnormal deposits")
        .iter()
        .find(|record| record["tx_hash"] == "tx-exact-audit-fail")
        .expect("confirming deposit");
    assert_eq!(abnormal["status"], "confirming");
    assert_eq!(abnormal["review_reason"], "awaiting_confirmations");

    let (orders_status, orders_body) = http_json(
        "GET",
        &format!(
            "{}/admin/deposits?at={}",
            server.base_url(),
            "2026-04-01T00:06:30Z"
        ),
        Some(&super_admin_token),
        None,
    );
    assert_eq!(orders_status, StatusCode::OK.as_u16());
    let order = orders_body["orders"]
        .as_array()
        .expect("orders")
        .iter()
        .find(|record| record["order_id"] == order_id)
        .expect("order listed");
    assert_eq!(order["status"], "pending");
}

#[tokio::test]
async fn super_admin_sweep_validation_rejects_unsupported_chain_asset_blank_from_address_and_non_pool_source(
) {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let super_admin_token =
        register_privileged_admin_and_login(&app, "super-admin@example.com").await;

    let unsupported_chain = create_sweep_job(
        &app,
        &super_admin_token,
        "TRON",
        "USDT",
        "tron-treasury-1",
        "2026-04-01T00:10:00Z",
        vec![json!({
            "from_address": "tron-addr-1",
            "amount": "42.00000000",
        })],
    )
    .await;
    assert_eq!(unsupported_chain.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(unsupported_chain).await["error"],
        "unsupported chain"
    );

    let unsupported_asset = create_sweep_job(
        &app,
        &super_admin_token,
        "ETH",
        "DAI",
        "eth-treasury-1",
        "2026-04-01T00:11:00Z",
        vec![json!({
            "from_address": "eth-addr-1",
            "amount": "42.00000000",
        })],
    )
    .await;
    assert_eq!(unsupported_asset.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(unsupported_asset).await["error"],
        "unsupported asset"
    );

    let blank_from_address = create_sweep_job(
        &app,
        &super_admin_token,
        "ETH",
        "USDC",
        "eth-treasury-1",
        "2026-04-01T00:12:00Z",
        vec![json!({
            "from_address": "   ",
            "amount": "42.00000000",
        })],
    )
    .await;
    assert_eq!(blank_from_address.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(blank_from_address).await["error"],
        "transfer from_address is required"
    );

    let non_pool_source = create_sweep_job(
        &app,
        &super_admin_token,
        "BSC",
        "USDT",
        "bsc-treasury-1",
        "2026-04-01T00:13:00Z",
        vec![json!({
            "from_address": "bsc-foreign-1",
            "amount": "42.00000000",
        })],
    )
    .await;
    assert_eq!(non_pool_source.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(non_pool_source).await["error"],
        "transfer from_address must belong to the address pool"
    );

    assert!(
        db.list_sweep_jobs().expect("sweep jobs").is_empty(),
        "invalid sweep requests must not persist"
    );
}

#[tokio::test]
async fn super_admin_sweep_creates_pending_job_without_fake_tx_hashes() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let super_admin_token =
        register_privileged_admin_and_login(&app, "super-admin@example.com").await;

    let created = create_sweep_job(
        &app,
        &super_admin_token,
        "BSC",
        "USDT",
        "bsc-treasury-1",
        "2026-04-01T00:14:00Z",
        vec![json!({
            "from_address": "bsc-addr-1",
            "amount": "20.00000000",
        })],
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body = response_json(created).await;
    assert_eq!(created_body["status"], "pending");

    let listed = list_sweeps(&app, &super_admin_token).await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    let job = listed_body["jobs"]
        .as_array()
        .expect("jobs")
        .iter()
        .find(|record| record["chain"] == "BSC" && record["asset"] == "USDT")
        .expect("sweep job listed");
    assert_eq!(job["status"], "pending");
    assert!(job["completed_at"].is_null());
    assert_eq!(job["transfers"][0]["from_address"], "bsc-addr-1");
    assert_eq!(job["transfers"][0]["to_address"], "bsc-treasury-1");
    assert!(job["transfers"][0]["tx_hash"].is_null());

    let stored = db.list_sweep_jobs().expect("sweep jobs");
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].status, "pending");
    assert!(stored[0].completed_at.is_none());
    assert_eq!(stored[0].transfers.len(), 1);
    assert!(stored[0].transfers[0].tx_hash.is_none());
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
    let unrelated_token = register_and_login(&app, "unrelated-order@example.com", "pass1234").await;
    let unrelated_order = create_order(
        &app,
        &unrelated_token,
        "unrelated-order@example.com",
        "BSC",
        "USDT",
        "quarterly",
        "2026-04-01T00:02:00Z",
    )
    .await;
    assert_eq!(unrelated_order.status(), StatusCode::CREATED);
    let unrelated_order_id = response_json(unrelated_order).await["order_id"]
        .as_u64()
        .expect("unrelated order id");
    let pending_token = register_and_login(&app, "pending-order@example.com", "pass1234").await;
    let pending_order = create_order(
        &app,
        &pending_token,
        "pending-order@example.com",
        "BSC",
        "USDT",
        "monthly",
        "2026-04-01T00:30:00Z",
    )
    .await;
    assert_eq!(pending_order.status(), StatusCode::CREATED);
    let pending_order_id = response_json(pending_order).await["order_id"]
        .as_u64()
        .expect("pending order id");

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
    assert_eq!(
        expired_match_body["deposit_status"],
        "manual_review_required"
    );

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
    let deposits = listed_body["abnormal_deposits"]
        .as_array()
        .expect("deposits");
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

    let unrelated_credit = process_abnormal_deposit(
        &app,
        &admin_token,
        "BSC",
        "tx-order-not-found",
        "credit_membership",
        Some(unrelated_order_id),
        Some(MANUAL_CREDIT_CONFIRMATION),
        Some("attempted to bind orphan transfer to unrelated quarterly order"),
        "2026-04-01T01:10:30Z",
    )
    .await;
    assert_eq!(unrelated_credit.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(unrelated_credit).await["error"],
        "manual credit target order is inconsistent with deposit context"
    );

    let credited = process_abnormal_deposit(
        &app,
        &admin_token,
        "BSC",
        "tx-order-not-found",
        "credit_membership",
        Some(expired_order_id),
        Some(MANUAL_CREDIT_CONFIRMATION),
        Some("attempted to bind orphan transfer to an already expired order assignment"),
        "2026-04-01T01:11:00Z",
    )
    .await;
    assert_eq!(credited.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(credited).await["error"],
        "manual credit target order is inconsistent with deposit context"
    );

    let credited = process_abnormal_deposit(
        &app,
        &admin_token,
        "BSC",
        "tx-order-not-found",
        "credit_membership",
        Some(pending_order_id),
        Some(MANUAL_CREDIT_CONFIRMATION),
        Some(
            "manual review attempted to bind orphan transfer to a different pending order address",
        ),
        "2026-04-01T01:11:30Z",
    )
    .await;
    assert_eq!(credited.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(credited).await["error"],
        "manual credit target order is inconsistent with deposit context"
    );
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
    db.insert_billing_order(&shared_db::BillingOrderRecord {
        order_id: 1003,
        email: "amb-c@example.com".to_string(),
        chain: "BSC".to_string(),
        asset: "USDT".to_string(),
        plan_code: "monthly".to_string(),
        amount: "20.00000000".to_string(),
        requested_at: "2026-04-01T00:02:00Z".parse().expect("time"),
        assignment: Some(shared_chain::assignment::AddressAssignment {
            chain: "BSC".to_string(),
            address: "different-address".to_string(),
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
        .any(|record| record["tx_hash"] == "tx-ambiguous-manual"
            && record["review_reason"] == "ambiguous_match"));

    let unrelated_credit = process_abnormal_deposit(
        &app,
        &admin_token,
        "BSC",
        "tx-ambiguous-manual",
        "credit_membership",
        Some(1003),
        Some(MANUAL_CREDIT_CONFIRMATION),
        Some("attempted to bind ambiguous transfer to order on a different address"),
        "2026-04-01T00:10:30Z",
    )
    .await;
    assert_eq!(unrelated_credit.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(unrelated_credit).await["error"],
        "manual credit target order is inconsistent with deposit context"
    );

    let rejected = process_abnormal_deposit(
        &app,
        &admin_token,
        "BSC",
        "tx-ambiguous-manual",
        "reject",
        None,
        None,
        None,
        "2026-04-01T00:11:00Z",
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::OK);
    assert_eq!(
        response_json(rejected).await["deposit_status"],
        "manual_rejected"
    );
}

#[tokio::test]

async fn abnormal_deposit_processing_fails_when_audit_write_fails() {
    let server = ApiServerHarness::start("deposit-audit");
    let user_token = register_and_login_via_http(&server, "member@example.com", "pass1234");
    let super_admin_token =
        register_privileged_admin_and_login_via_http(&server, "super-admin@example.com");

    let (order_status, order_body) = http_json(
        "POST",
        &format!("{}/billing/orders", server.base_url()),
        Some(&user_token),
        Some(json!({
            "email": "member@example.com",
            "chain": "BSC",
            "asset": "USDT",
            "plan_code": "monthly",
            "requested_at": "2026-04-01T00:00:00Z"
        })),
    );
    assert_eq!(order_status, StatusCode::CREATED.as_u16());

    let order_address = order_body["address"].as_str().expect("address");
    let (match_status, _) = http_json(
        "POST",
        &format!("{}/billing/orders/match", server.base_url()),
        Some(&super_admin_token),
        Some(json!({
            "chain": "BSC",
            "asset": "USDC",
            "address": order_address,
            "amount": "20.00000000",
            "tx_hash": "tx-wrong-asset-audit",
            "observed_at": "2026-04-01T00:05:00Z"
        })),
    );
    assert_eq!(match_status, StatusCode::OK.as_u16());

    server.break_audit_table();

    let (reject_status, _) = http_json(
        "POST",
        &format!("{}/admin/deposits/process", server.base_url()),
        Some(&super_admin_token),
        Some(json!({
            "chain": "BSC",
            "tx_hash": "tx-wrong-asset-audit",
            "decision": "credit_membership",
            "order_id": order_body["order_id"],
            "confirmation": MANUAL_CREDIT_CONFIRMATION,
            "justification": "audit write failure should abort manual credit mutation",
            "processed_at": "2026-04-01T00:06:00Z"
        })),
    );
    assert_eq!(reject_status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());

    let (status_status, status_body) = http_json(
        "POST",
        &format!("{}/membership/status", server.base_url()),
        Some(&user_token),
        Some(json!({
            "email": "member@example.com",
            "at": "2026-04-01T00:07:00Z"
        })),
    );
    assert_eq!(status_status, StatusCode::OK.as_u16());
    assert_eq!(status_body["status"], "Pending");

    let (deposits_status, deposits_body) = http_json(
        "GET",
        &format!(
            "{}/admin/deposits?at={}",
            server.base_url(),
            "2026-04-01T00:07:00Z"
        ),
        Some(&super_admin_token),
        None,
    );
    assert_eq!(deposits_status, StatusCode::OK.as_u16());
    let deposit = deposits_body["abnormal_deposits"]
        .as_array()
        .expect("abnormal deposits")
        .iter()
        .find(|record| record["tx_hash"] == "tx-wrong-asset-audit")
        .expect("tx still listed");
    assert_eq!(deposit["status"], "manual_review_required");
    assert_eq!(deposit["review_reason"], "wrong_asset");

    let (sweep_status, _) = http_json(
        "POST",
        &format!("{}/admin/sweeps", server.base_url()),
        Some(&super_admin_token),
        Some(json!({
            "chain": "BSC",
            "asset": "USDT",
            "treasury_address": "bsc-treasury-1",
            "requested_at": "2026-04-01T00:08:00Z",
            "transfers": [
                {
                    "from_address": "bsc-addr-1",
                    "amount": "20.00000000"
                }
            ]
        })),
    );
    assert_eq!(sweep_status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());
    let (sweeps_read_status, sweeps_body) = http_json(
        "GET",
        &format!("{}/admin/sweeps", server.base_url()),
        Some(&super_admin_token),
        None,
    );
    assert_eq!(sweeps_read_status, StatusCode::OK.as_u16());
    assert_eq!(
        sweeps_body["jobs"].as_array().expect("jobs").len(),
        0,
        "failed sweep request must not persist"
    );
}

#[test]
fn concurrent_manual_abnormal_processing_allows_only_one_outcome() {
    let server = ApiServerHarness::start("manual-claim");
    let user_token = register_and_login_via_http(&server, "member@example.com", "pass1234");
    let credit_admin_token =
        register_privileged_admin_and_login_via_http(&server, "admin@example.com");
    let reject_admin_token =
        register_privileged_admin_and_login_via_http(&server, "super-admin@example.com");

    let (order_status, order_body) = http_json(
        "POST",
        &format!("{}/billing/orders", server.base_url()),
        Some(&user_token),
        Some(json!({
            "email": "member@example.com",
            "chain": "BSC",
            "asset": "USDT",
            "plan_code": "monthly",
            "requested_at": "2026-04-01T00:00:00Z"
        })),
    );
    assert_eq!(order_status, StatusCode::CREATED.as_u16());

    let order_id = order_body["order_id"].as_u64().expect("order id");
    let order_address = order_body["address"].as_str().expect("address");

    let (match_status, match_body) = http_json(
        "POST",
        &format!("{}/billing/orders/match", server.base_url()),
        Some(&credit_admin_token),
        Some(json!({
            "chain": "BSC",
            "asset": "USDC",
            "address": order_address,
            "amount": "20.00000000",
            "tx_hash": "tx-manual-race",
            "observed_at": "2026-04-01T00:05:00Z"
        })),
    );
    assert_eq!(match_status, StatusCode::OK.as_u16());
    assert_eq!(match_body["deposit_status"], "manual_review_required");

    let barrier = Arc::new(Barrier::new(2));
    let process_url = format!("{}/admin/deposits/process", server.base_url());
    let credit_barrier = barrier.clone();
    let reject_barrier = barrier.clone();
    let credit_token = credit_admin_token.clone();
    let reject_token = reject_admin_token.clone();
    let credit_url = process_url.clone();
    let reject_url = process_url;

    let credit_handle = thread::spawn(move || {
        credit_barrier.wait();
        http_json(
            "POST",
            &credit_url,
            Some(&credit_token),
            Some(json!({
                "chain": "BSC",
                "tx_hash": "tx-manual-race",
                "decision": "credit_membership",
                "order_id": order_id,
                "confirmation": MANUAL_CREDIT_CONFIRMATION,
                "justification": "credit path raced against reject path",
                "processed_at": "2026-04-01T00:06:00Z"
            })),
        )
    });
    let reject_handle = thread::spawn(move || {
        reject_barrier.wait();
        http_json(
            "POST",
            &reject_url,
            Some(&reject_token),
            Some(json!({
                "chain": "BSC",
                "tx_hash": "tx-manual-race",
                "decision": "reject",
                "processed_at": "2026-04-01T00:06:00Z"
            })),
        )
    });

    let credit_result = credit_handle.join().expect("credit join");
    let reject_result = reject_handle.join().expect("reject join");
    let statuses = [credit_result.0, reject_result.0];
    assert_eq!(
        statuses
            .iter()
            .filter(|status| **status == StatusCode::OK.as_u16())
            .count(),
        1
    );
    assert_eq!(
        statuses
            .iter()
            .filter(|status| **status == StatusCode::BAD_REQUEST.as_u16())
            .count(),
        1
    );

    let (deposits_status, deposits_body) = http_json(
        "GET",
        &format!(
            "{}/admin/deposits?at={}",
            server.base_url(),
            "2026-04-01T00:07:00Z"
        ),
        Some(&reject_admin_token),
        None,
    );
    assert_eq!(deposits_status, StatusCode::OK.as_u16());
    let deposit = deposits_body["abnormal_deposits"]
        .as_array()
        .expect("abnormal deposits")
        .iter()
        .find(|record| record["tx_hash"] == "tx-manual-race")
        .expect("deposit listed");
    assert!(deposit["status"] == "manual_approved" || deposit["status"] == "manual_rejected");
}

async fn register_admin_and_login(app: &axum::Router) -> String {
    register_privileged_admin_and_login(app, "admin@example.com").await
}

async fn register_privileged_admin_and_login(app: &axum::Router, email: &str) -> String {
    register_and_verify(app, email, "pass1234").await;
    let enabled = bootstrap_admin_totp(app, email, "pass1234").await;
    let totp_code = enabled["code"].as_str().expect("totp code");
    login_with_totp(app, email, "pass1234", totp_code).await
}

struct PersistentRuntimeHarness {
    project_name: String,
    override_file: PathBuf,
    postgres_port: u16,
    redis_port: u16,
}

impl PersistentRuntimeHarness {
    fn start(prefix: &str) -> Self {
        let workspace_root = workspace_root();
        let postgres_port = pick_unused_port();
        let redis_port = pick_unused_port();
        let project_name = format!(
            "{prefix}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("unix time")
                .as_nanos()
        );
        let override_file = std::env::temp_dir().join(format!("{project_name}.yml"));
        fs::write(
            &override_file,
            format!(
                "services:
  postgres:
    ports:
      - \"{postgres_port}:5432\"

  redis:
    ports:
      - \"{redis_port}:6379\"
"
            ),
        )
        .expect("write compose override");
        run_command(
            Command::new("docker")
                .arg("compose")
                .arg("-p")
                .arg(&project_name)
                .arg("--env-file")
                .arg(workspace_root.join(".env.example"))
                .arg("-f")
                .arg(workspace_root.join("deploy/docker/docker-compose.yml"))
                .arg("-f")
                .arg(&override_file)
                .arg("up")
                .arg("-d")
                .arg("--wait")
                .arg("postgres")
                .arg("redis"),
            "start persistent runtime",
        );

        Self {
            project_name,
            override_file,
            postgres_port,
            redis_port,
        }
    }

    fn database_url(&self) -> String {
        format!(
            "postgres://postgres:postgres@127.0.0.1:{}/grid_binance",
            self.postgres_port
        )
    }

    fn redis_url(&self) -> String {
        format!("redis://127.0.0.1:{}/0", self.redis_port)
    }

    fn break_audit_table(&self) {
        run_command(
            Command::new("docker")
                .arg("exec")
                .arg(format!("{}-postgres-1", self.project_name))
                .arg("psql")
                .arg("-U")
                .arg("postgres")
                .arg("-d")
                .arg("grid_binance")
                .arg("-c")
                .arg("ALTER TABLE audit_logs RENAME TO audit_logs_disabled"),
            "break audit table",
        );
    }
}

impl Drop for PersistentRuntimeHarness {
    fn drop(&mut self) {
        let workspace_root = workspace_root();
        let _ = Command::new("docker")
            .arg("compose")
            .arg("-p")
            .arg(&self.project_name)
            .arg("--env-file")
            .arg(workspace_root.join(".env.example"))
            .arg("-f")
            .arg(workspace_root.join("deploy/docker/docker-compose.yml"))
            .arg("-f")
            .arg(&self.override_file)
            .arg("down")
            .arg("-v")
            .status();
        let _ = fs::remove_file(&self.override_file);
    }
}

async fn bootstrap_admin_totp(app: &axum::Router, email: &str, password: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/admin-bootstrap")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "email": email, "password": password }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await
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
                        "confirmations": 12,
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
    confirmation: Option<&str>,
    justification: Option<&str>,
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
                        "confirmation": confirmation,
                        "justification": justification,
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

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    serde_json::from_slice(&bytes).expect("valid json")
}

struct ApiServerHarness {
    runtime: PersistentRuntimeHarness,
    port: u16,
    child: Child,
}

impl ApiServerHarness {
    fn start(prefix: &str) -> Self {
        let runtime = PersistentRuntimeHarness::start(prefix);
        let port = pick_unused_port();
        let mut child = Command::new("bash");
        child
            .arg("-lc")
            .arg("source \"$HOME/.cargo/env\" && cargo run -p api-server")
            .current_dir(workspace_root())
            .env("DATABASE_URL", runtime.database_url())
            .env("REDIS_URL", runtime.redis_url())
            .env("SESSION_TOKEN_SECRET", "grid-binance-dev-session-secret")
            .env("APP_ENV", "test")
            .env("AUTH_EMAIL_DELIVERY", "capture")
            .env("ADMIN_EMAILS", "admin@example.com")
            .env("SUPER_ADMIN_EMAILS", "super-admin@example.com")
            .env(
                "EXCHANGE_CREDENTIALS_MASTER_KEY",
                "grid-binance-dev-exchange-secret",
            )
            .env(
                "TELEGRAM_BOT_BIND_SECRET",
                "grid-binance-dev-telegram-secret",
            )
            .env("PORT", port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = child.spawn().expect("start api-server");
        let harness = Self {
            runtime,
            port,
            child,
        };
        harness.wait_until_ready();
        harness
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn break_audit_table(&self) {
        self.runtime.break_audit_table();
    }

    fn wait_until_ready(&self) {
        for _ in 0..120 {
            let output = Command::new("curl")
                .arg("-sS")
                .arg("-o")
                .arg("/dev/null")
                .arg("-w")
                .arg("%{http_code}")
                .arg(format!("{}/healthz", self.base_url()))
                .output();
            if let Ok(output) = output {
                if output.status.success()
                    && String::from_utf8_lossy(&output.stdout).trim() == "200"
                {
                    return;
                }
            }
            sleep(Duration::from_millis(500));
        }
        panic!("api-server did not become ready");
    }
}

impl Drop for ApiServerHarness {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn register_and_login_via_http(server: &ApiServerHarness, email: &str, password: &str) -> String {
    let (register_status, register_body) = http_json(
        "POST",
        &format!("{}/auth/register", server.base_url()),
        None,
        Some(json!({ "email": email, "password": password })),
    );
    assert_eq!(register_status, StatusCode::CREATED.as_u16());
    let verification_code = register_body["verification_code"]
        .as_str()
        .expect("verification code");

    let (verify_status, _) = http_json(
        "POST",
        &format!("{}/auth/verify-email", server.base_url()),
        None,
        Some(json!({ "email": email, "code": verification_code })),
    );
    assert_eq!(verify_status, StatusCode::OK.as_u16());

    let (login_status, login_body) = http_json(
        "POST",
        &format!("{}/auth/login", server.base_url()),
        None,
        Some(json!({ "email": email, "password": password })),
    );
    assert_eq!(login_status, StatusCode::OK.as_u16());
    login_body["session_token"]
        .as_str()
        .expect("session token")
        .to_owned()
}

fn register_and_verify_via_http(server: &ApiServerHarness, email: &str, password: &str) {
    let (register_status, register_body) = http_json(
        "POST",
        &format!("{}/auth/register", server.base_url()),
        None,
        Some(json!({ "email": email, "password": password })),
    );
    assert_eq!(register_status, StatusCode::CREATED.as_u16());
    let verification_code = register_body["verification_code"].as_str().expect("verification code").to_owned();
    let (verify_status, _) = http_json(
        "POST",
        &format!("{}/auth/verify-email", server.base_url()),
        None,
        Some(json!({ "email": email, "code": verification_code })),
    );
    assert_eq!(verify_status, StatusCode::OK.as_u16());
}

fn register_privileged_admin_and_login_via_http(server: &ApiServerHarness, email: &str) -> String {
    register_and_verify_via_http(server, email, "pass1234");
    let (bootstrap_status, bootstrap_body) = http_json(
        "POST",
        &format!("{}/auth/admin-bootstrap", server.base_url()),
        None,
        Some(json!({ "email": email, "password": "pass1234" })),
    );
    assert_eq!(bootstrap_status, StatusCode::OK.as_u16());
    let totp_code = bootstrap_body["code"].as_str().expect("totp code");

    let (login_status, login_body) = http_json(
        "POST",
        &format!("{}/auth/login", server.base_url()),
        None,
        Some(json!({
            "email": email,
            "password": "pass1234",
            "totp_code": totp_code,
        })),
    );
    assert_eq!(login_status, StatusCode::OK.as_u16());
    login_body["session_token"]
        .as_str()
        .expect("session token")
        .to_owned()
}

fn http_json(
    method: &str,
    url: &str,
    bearer_token: Option<&str>,
    payload: Option<Value>,
) -> (u16, Value) {
    let mut command = Command::new("curl");
    command.arg("-sS").arg("-X").arg(method).arg(url);
    if let Some(token) = bearer_token {
        command
            .arg("-H")
            .arg(format!("authorization: Bearer {token}"));
    }
    if payload.is_some() {
        command.arg("-H").arg("content-type: application/json");
    }
    if let Some(payload) = payload {
        command.arg("-d").arg(payload.to_string());
    }
    command.arg("-w").arg("\n%{http_code}");
    let output = command.output().expect("execute curl");
    assert!(
        output.status.success(),
        "curl failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("curl stdout utf8");
    let (body, status) = stdout.rsplit_once('\n').expect("curl status line");
    let status = status.trim().parse::<u16>().expect("http status");
    let body = body.trim();
    let json = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(body).expect("valid json body")
    };
    (status, json)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn pick_unused_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind random port")
        .local_addr()
        .expect("local addr")
        .port()
}

fn run_command(command: &mut Command, context: &str) {
    let output = command.output().expect(context);
    assert!(
        output.status.success(),
        "{context} failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
