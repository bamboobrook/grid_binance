use api_server::{app, app_with_state, AppState};
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
async fn supports_all_three_chains_and_stablecoin_pricing_rules() {
    let app = app();
    let eth_token = register_and_login(&app, "eth@example.com", "pass1234").await;
    let bsc_token = register_and_login(&app, "bsc@example.com", "pass1234").await;
    let sol_token = register_and_login(&app, "sol@example.com", "pass1234").await;
    let admin_token = register_admin_and_login(&app).await;

    let eth_order = create_order(
        &app,
        &eth_token,
        "eth@example.com",
        "ETH",
        "USDT",
        "monthly",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(eth_order.status(), StatusCode::CREATED);
    let eth_order_body = response_json(eth_order).await;
    assert_eq!(eth_order_body["chain"], "ETH");
    assert_eq!(eth_order_body["asset"], "USDT");
    assert_eq!(eth_order_body["amount"], "20.00000000");
    assert_eq!(eth_order_body["address"], "eth-addr-1");
    assert_eq!(eth_order_body["queue_position"], Value::Null);

    let bsc_order = create_order(
        &app,
        &bsc_token,
        "bsc@example.com",
        "BSC",
        "USDC",
        "quarterly",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(bsc_order.status(), StatusCode::CREATED);
    let bsc_order_body = response_json(bsc_order).await;
    assert_eq!(bsc_order_body["chain"], "BSC");
    assert_eq!(bsc_order_body["asset"], "USDC");
    assert_eq!(bsc_order_body["amount"], "54.00000000");
    assert_eq!(bsc_order_body["address"], "bsc-addr-1");

    let sol_order = create_order(
        &app,
        &sol_token,
        "sol@example.com",
        "SOL",
        "USDT",
        "yearly",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(sol_order.status(), StatusCode::CREATED);
    let sol_order_body = response_json(sol_order).await;
    assert_eq!(sol_order_body["chain"], "SOL");
    assert_eq!(sol_order_body["asset"], "USDT");
    assert_eq!(sol_order_body["amount"], "180.00000000");
    assert_eq!(sol_order_body["address"], "sol-addr-1");

    let mismatch = match_order(
        &app,
        &admin_token,
        "BSC",
        "USDC",
        "bsc-addr-1",
        "53.99999999",
        "tx-exact-mismatch",
        "2026-04-01T00:10:00Z",
    )
    .await;
    assert_eq!(mismatch.status(), StatusCode::OK);
    let mismatch_body = response_json(mismatch).await;
    assert_eq!(mismatch_body["matched"], false);
    assert_eq!(mismatch_body["reason"], "exact_amount_required");
    assert_eq!(mismatch_body["deposit_status"], "manual_review_required");

    let exact_match = match_order(
        &app,
        &admin_token,
        "ETH",
        "USDT",
        "eth-addr-1",
        "20.00000000",
        "tx-eth-exact",
        "2026-04-01T00:10:00Z",
    )
    .await;
    assert_eq!(exact_match.status(), StatusCode::OK);
    let exact_match_body = response_json(exact_match).await;
    assert_eq!(exact_match_body["matched"], true);
    assert_eq!(exact_match_body["membership_status"], "Active");
    assert_eq!(exact_match_body["deposit_status"], "matched");
}

#[tokio::test]
async fn queues_orders_when_pool_is_exhausted_and_promotes_in_fifo_order() {
    let app = app();
    let admin_token = register_admin_and_login(&app).await;

    for index in 1..=5 {
        let email = format!("queue-{index}@example.com");
        let user_token = register_and_login(&app, &email, "pass1234").await;
        let response = create_order(
            &app,
            &user_token,
            &email,
            "BSC",
            "USDT",
            "monthly",
            "2026-04-01T00:00:00Z",
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response_json(response).await["address"],
            format!("bsc-addr-{index}")
        );
    }

    let queued_email = "queue-6@example.com";
    let queued_token = register_and_login(&app, queued_email, "pass1234").await;
    let queued = create_order(
        &app,
        &queued_token,
        queued_email,
        "BSC",
        "USDT",
        "monthly",
        "2026-04-01T00:01:00Z",
    )
    .await;
    assert_eq!(queued.status(), StatusCode::CREATED);
    let queued_body = response_json(queued).await;
    assert_eq!(queued_body["address"], Value::Null);
    assert_eq!(queued_body["queue_position"], 1);

    let deposits = list_admin_deposits(&app, &admin_token, "2026-04-01T01:01:00Z").await;
    assert_eq!(deposits.status(), StatusCode::OK);
    let deposits_body = response_json(deposits).await;
    let promoted = deposits_body["orders"]
        .as_array()
        .expect("orders array")
        .iter()
        .find(|order| order["email"] == queued_email)
        .expect("queued order promoted");
    assert_eq!(promoted["address"], "bsc-addr-1");
    assert_eq!(promoted["queue_position"], Value::Null);

    let next_email = "queue-7@example.com";
    let next_token = register_and_login(&app, next_email, "pass1234").await;
    let next_order = create_order(
        &app,
        &next_token,
        next_email,
        "BSC",
        "USDT",
        "monthly",
        "2026-04-01T01:01:00Z",
    )
    .await;
    assert_eq!(next_order.status(), StatusCode::CREATED);
    assert_eq!(response_json(next_order).await["address"], "bsc-addr-2");
}

#[tokio::test]
async fn allocation_uses_configured_pool_and_respects_disabled_addresses() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    for address in [
        "bsc-addr-1",
        "bsc-addr-2",
        "bsc-addr-3",
        "bsc-addr-4",
        "bsc-addr-5",
    ] {
        db.upsert_deposit_address(&shared_db::DepositAddressPoolRecord {
            chain: "BSC".to_string(),
            address: address.to_string(),
            is_enabled: false,
        })
        .expect("disable default address");
    }
    db.upsert_deposit_address(&shared_db::DepositAddressPoolRecord {
        chain: "BSC".to_string(),
        address: "bsc-custom-1".to_string(),
        is_enabled: true,
    })
    .expect("insert custom address");
    db.upsert_deposit_address(&shared_db::DepositAddressPoolRecord {
        chain: "BSC".to_string(),
        address: "bsc-custom-2".to_string(),
        is_enabled: true,
    })
    .expect("insert custom address");
    let app = app_with_state(AppState::from_shared_db(db).expect("state"));
    let first_token = register_and_login(&app, "pool-a@example.com", "pass1234").await;
    let second_token = register_and_login(&app, "pool-b@example.com", "pass1234").await;

    let first = create_order(
        &app,
        &first_token,
        "pool-a@example.com",
        "BSC",
        "USDT",
        "monthly",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(first.status(), StatusCode::CREATED);
    assert_eq!(response_json(first).await["address"], "bsc-custom-1");

    let second = create_order(
        &app,
        &second_token,
        "pool-b@example.com",
        "BSC",
        "USDT",
        "monthly",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(second.status(), StatusCode::CREATED);
    assert_eq!(response_json(second).await["address"], "bsc-custom-2");
}

#[tokio::test]
async fn membership_transitions_from_active_to_grace_to_expired_after_48_hours() {
    let app = app();
    let user_token = register_and_login(&app, "grace@example.com", "pass1234").await;
    let admin_token = register_admin_and_login(&app).await;

    let order = create_order(
        &app,
        &user_token,
        "grace@example.com",
        "BSC",
        "USDT",
        "monthly",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(order.status(), StatusCode::CREATED);

    let matched = match_order(
        &app,
        &admin_token,
        "BSC",
        "USDT",
        "bsc-addr-1",
        "20.00000000",
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
        "2026-05-02T23:59:59Z",
    )
    .await;
    assert_eq!(grace.status(), StatusCode::OK);
    assert_eq!(response_json(grace).await["status"], "Grace");

    let expired = membership_status(
        &app,
        &user_token,
        "grace@example.com",
        "2026-05-03T00:00:01Z",
    )
    .await;
    assert_eq!(expired.status(), StatusCode::OK);
    assert_eq!(response_json(expired).await["status"], "Expired");
}

#[tokio::test]
async fn operator_admin_cannot_override_membership_or_write_override_audit_logs() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let state = AppState::from_shared_db(db.clone()).expect("state");
    let app = app_with_state(state);
    let admin_token = register_admin_and_login(&app).await;

    let order = create_order(
        &app,
        &admin_token,
        "admin@example.com",
        "BSC",
        "USDT",
        "monthly",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(order.status(), StatusCode::CREATED);

    let matched = match_order(
        &app,
        &admin_token,
        "BSC",
        "USDT",
        "bsc-addr-1",
        "20.00000000",
        "tx-admin",
        "2026-04-01T00:01:00Z",
    )
    .await;
    assert_eq!(matched.status(), StatusCode::OK);
    assert_eq!(response_json(matched).await["membership_status"], "Active");

    let frozen = override_membership(&app, &admin_token, "admin@example.com", Some("Frozen")).await;
    assert_eq!(frozen.status(), StatusCode::FORBIDDEN);

    let cleared = override_membership(&app, &admin_token, "admin@example.com", None).await;
    assert_eq!(cleared.status(), StatusCode::FORBIDDEN);

    let revoked =
        override_membership(&app, &admin_token, "admin@example.com", Some("Revoked")).await;
    assert_eq!(revoked.status(), StatusCode::FORBIDDEN);

    let audit_logs = db.list_audit_logs().expect("audit logs");
    assert!(!audit_logs
        .iter()
        .any(|record| record.action == "membership.override_updated"));
}

#[tokio::test]
async fn admin_can_open_extend_and_unfreeze_membership_manually() {
    operator_admin_cannot_manually_open_extend_or_unfreeze_membership().await;
    manual_membership_actions_fail_when_audit_write_fails().await;
}

async fn operator_admin_cannot_manually_open_extend_or_unfreeze_membership() {
    let db = SharedDb::ephemeral().expect("ephemeral db");
    let app = app_with_state(AppState::from_shared_db(db.clone()).expect("state"));
    let user_token = register_and_login(&app, "manual@example.com", "pass1234").await;
    let admin_token = register_admin_and_login(&app).await;

    let opened = manage_membership(
        &app,
        &admin_token,
        "manual@example.com",
        "open",
        Some(30),
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(opened.status(), StatusCode::FORBIDDEN);

    let frozen =
        override_membership(&app, &admin_token, "manual@example.com", Some("Frozen")).await;
    assert_eq!(frozen.status(), StatusCode::FORBIDDEN);

    let unfrozen = manage_membership(
        &app,
        &admin_token,
        "manual@example.com",
        "unfreeze",
        None,
        "2026-04-10T00:00:00Z",
    )
    .await;
    assert_eq!(unfrozen.status(), StatusCode::FORBIDDEN);

    let extended = manage_membership(
        &app,
        &admin_token,
        "manual@example.com",
        "extend",
        Some(30),
        "2026-04-15T00:00:00Z",
    )
    .await;
    assert_eq!(extended.status(), StatusCode::FORBIDDEN);

    let status = membership_status(
        &app,
        &user_token,
        "manual@example.com",
        "2026-05-20T00:00:00Z",
    )
    .await;
    assert_eq!(status.status(), StatusCode::OK);
    assert_eq!(response_json(status).await["status"], "Pending");

    let audit_logs = db.list_audit_logs().expect("audit logs");
    assert!(!audit_logs
        .iter()
        .any(|record| record.action == "membership.manual_opened"));
    assert!(!audit_logs
        .iter()
        .any(|record| record.action == "membership.manual_extended"));
    assert!(!audit_logs
        .iter()
        .any(|record| record.action == "membership.manual_unfrozen"));
    assert!(!audit_logs
        .iter()
        .any(|record| record.action == "membership.override_updated"));
}

async fn manual_membership_actions_fail_when_audit_write_fails() {
    let server = ApiServerHarness::start("membership-audit");
    let super_admin_token =
        register_privileged_admin_and_login_via_http(&server, "super-admin@example.com");

    server.break_audit_table();

    let (opened_status, _) = http_json(
        "POST",
        &format!("{}/admin/memberships/manage", server.base_url()),
        Some(&super_admin_token),
        Some(json!({
            "email": "manual@example.com",
            "action": "open",
            "duration_days": 30,
            "at": "2026-04-01T00:00:00Z"
        })),
    );
    assert_eq!(opened_status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());

    let (extended_status, _) = http_json(
        "POST",
        &format!("{}/admin/memberships/manage", server.base_url()),
        Some(&super_admin_token),
        Some(json!({
            "email": "manual@example.com",
            "action": "extend",
            "duration_days": 30,
            "at": "2026-04-15T00:00:00Z"
        })),
    );
    assert_eq!(extended_status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());

    let (override_status, _) = http_json(
        "POST",
        &format!("{}/admin/memberships/override", server.base_url()),
        Some(&super_admin_token),
        Some(json!({
            "email": "manual@example.com",
            "status": "Frozen"
        })),
    );
    assert_eq!(override_status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());

    let (unfreeze_status, _) = http_json(
        "POST",
        &format!("{}/admin/memberships/manage", server.base_url()),
        Some(&super_admin_token),
        Some(json!({
            "email": "manual@example.com",
            "action": "unfreeze",
            "duration_days": null,
            "at": "2026-04-20T00:00:00Z"
        })),
    );
    assert_eq!(unfreeze_status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());
}

#[tokio::test]
async fn operator_admin_cannot_manually_extend_membership_during_grace() {
    let app = app();
    let user_token = register_and_login(&app, "grace-stack@example.com", "pass1234").await;
    let admin_token = register_admin_and_login(&app).await;

    let order = create_order(
        &app,
        &user_token,
        "grace-stack@example.com",
        "BSC",
        "USDT",
        "monthly",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(order.status(), StatusCode::CREATED);

    let matched = match_order(
        &app,
        &admin_token,
        "BSC",
        "USDT",
        "bsc-addr-1",
        "20.00000000",
        "tx-grace-stack",
        "2026-04-01T00:00:00Z",
    )
    .await;
    assert_eq!(matched.status(), StatusCode::OK);

    let extended = manage_membership(
        &app,
        &admin_token,
        "grace-stack@example.com",
        "extend",
        Some(10),
        "2026-05-02T12:00:00Z",
    )
    .await;
    assert_eq!(extended.status(), StatusCode::FORBIDDEN);

    let status = membership_status(
        &app,
        &user_token,
        "grace-stack@example.com",
        "2026-05-02T12:00:00Z",
    )
    .await;
    assert_eq!(status.status(), StatusCode::OK);
    assert_eq!(response_json(status).await["status"], "Grace");
}

#[tokio::test]
async fn anonymous_user_cannot_override_membership() {
    let app = app();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/memberships/override")
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
                .uri("/admin/memberships/override")
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

async fn manage_membership(
    app: &axum::Router,
    session_token: &str,
    email: &str,
    action: &str,
    duration_days: Option<i64>,
    at: &str,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/memberships/manage")
                .header("authorization", format!("Bearer {session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "action": action,
                        "duration_days": duration_days,
                        "at": at,
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
