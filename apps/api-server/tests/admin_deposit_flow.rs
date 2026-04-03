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
    thread::sleep,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
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
        .any(|record| record["tx_hash"] == "tx-ambiguous-manual"
            && record["review_reason"] == "ambiguous_match"));

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
    assert_eq!(
        response_json(rejected).await["deposit_status"],
        "manual_rejected"
    );
}

#[tokio::test]

async fn abnormal_deposit_processing_fails_when_audit_write_fails() {
    let server = ApiServerHarness::start("deposit-audit");
    let user_token = register_and_login_via_http(&server, "member@example.com", "pass1234");
    let admin_token = register_privileged_admin_and_login_via_http(&server, "admin@example.com");

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
        Some(&admin_token),
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
        Some(&admin_token),
        Some(json!({
            "chain": "BSC",
            "tx_hash": "tx-wrong-asset-audit",
            "decision": "reject",
            "order_id": null,
            "processed_at": "2026-04-01T00:06:00Z"
        })),
    );
    assert_eq!(reject_status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());
}

async fn register_admin_and_login(app: &axum::Router) -> String {
    register_privileged_admin_and_login(app, "admin@example.com").await
}

async fn register_privileged_admin_and_login(app: &axum::Router, email: &str) -> String {
    register_and_verify(app, email, "pass1234").await;
    let session_token = login_and_get_token(app, email, "pass1234").await;
    let enabled = enable_totp(app, email, &session_token).await;
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

fn register_privileged_admin_and_login_via_http(server: &ApiServerHarness, email: &str) -> String {
    let session_token = register_and_login_via_http(server, email, "pass1234");
    let (enable_status, enable_body) = http_json(
        "POST",
        &format!("{}/security/totp/enable", server.base_url()),
        Some(&session_token),
        Some(json!({ "email": email })),
    );
    assert_eq!(enable_status, StatusCode::OK.as_u16());
    let totp_code = enable_body["code"].as_str().expect("totp code");

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
