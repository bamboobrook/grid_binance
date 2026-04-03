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
async fn operator_admin_cannot_update_plan_config_and_existing_defaults_remain_in_effect() {
    let app = app_with_state(
        AppState::from_shared_db(SharedDb::ephemeral().expect("db")).expect("state"),
    );
    let admin_token = register_admin_and_login(&app).await;
    let user_token = register_and_login(&app, "priced@example.com", "pass1234").await;

    let forbidden = upsert_plan(
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
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let listed = list_plans(&app, &admin_token).await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    let monthly = listed_body["plans"]
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
        .any(|price| price["chain"] == "BSC"
            && price["asset"] == "USDT"
            && price["amount"] == "20.00000000"));

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
    assert_eq!(order_body["amount"], "20.00000000");

    let matched = match_order(
        &app,
        &admin_token,
        "BSC",
        "USDT",
        order_body["address"].as_str().expect("address"),
        "20.00000000",
        "tx-priced",
        "2026-04-01T00:01:00Z",
    )
    .await;
    assert_eq!(matched.status(), StatusCode::OK);
    let matched_body = response_json(matched).await;
    assert_eq!(matched_body["matched"], true);
    assert_eq!(matched_body["active_until"], "2026-05-01T00:01:00Z");
}

#[tokio::test]
async fn forbidden_plan_update_does_not_partially_persist_plan_or_prices() {
    let app = app_with_state(
        AppState::from_shared_db(SharedDb::ephemeral().expect("db")).expect("state"),
    );
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
    assert_eq!(invalid.status(), StatusCode::FORBIDDEN);

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
        .any(|price| price["chain"] == "BSC"
            && price["asset"] == "USDT"
            && price["amount"] == "20.00000000"));
    assert!(!monthly["prices"]
        .as_array()
        .expect("prices")
        .iter()
        .any(|price| price["amount"] == "88.00000000"));
}

#[tokio::test]
async fn operator_admin_cannot_mutate_address_pools_but_can_review_current_pool_state() {
    let app = app_with_state(
        AppState::from_shared_db(SharedDb::ephemeral().expect("db")).expect("state"),
    );
    let admin_token = register_admin_and_login(&app).await;

    let disabled = upsert_address_pool(
        &app,
        &admin_token,
        json!({
            "chain": "BSC",
            "address": "bsc-addr-1",
            "is_enabled": false
        }),
    )
    .await;
    assert_eq!(disabled.status(), StatusCode::FORBIDDEN);

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
    assert_eq!(added.status(), StatusCode::FORBIDDEN);

    let listed = list_address_pools(&app, &admin_token).await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    assert!(listed_body["addresses"]
        .as_array()
        .expect("addresses")
        .iter()
        .any(|entry| entry["address"] == "bsc-addr-1" && entry["is_enabled"] == true));
    assert!(!listed_body["addresses"]
        .as_array()
        .expect("addresses")
        .iter()
        .any(|entry| entry["address"] == "bsc-extra-1"));

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
    assert_eq!(response_json(first).await["address"], "bsc-addr-1");

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
    let second_body = response_json(second).await;
    assert_eq!(second_body["address"], "bsc-addr-2");
    assert_eq!(second_body["queue_position"], Value::Null);
}

#[tokio::test]

async fn super_admin_plan_and_address_pool_updates_fail_when_audit_write_fails() {
    let server = ApiServerHarness::start("address-pool-audit");
    let super_admin_token =
        register_privileged_admin_and_login_via_http(&server, "super-admin@example.com");

    server.break_audit_table();

    let (plan_status, _) = http_json(
        "POST",
        &format!("{}/admin/memberships/plans", server.base_url()),
        Some(&super_admin_token),
        Some(json!({
            "code": "monthly",
            "name": "Monthly Plus",
            "duration_days": 45,
            "is_active": true,
            "prices": [
                { "chain": "BSC", "asset": "USDT", "amount": "21.50000000" },
                { "chain": "ETH", "asset": "USDT", "amount": "22.50000000" }
            ]
        })),
    );
    assert_eq!(plan_status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());
    let (plans_status, plans_body) = http_json(
        "GET",
        &format!("{}/admin/memberships/plans", server.base_url()),
        Some(&super_admin_token),
        None,
    );
    assert_eq!(plans_status, StatusCode::OK.as_u16());
    let monthly = plans_body["plans"]
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
        .any(|price| price["chain"] == "BSC"
            && price["asset"] == "USDT"
            && price["amount"] == "20.00000000"));

    let (pool_status, _) = http_json(
        "POST",
        &format!("{}/admin/address-pools", server.base_url()),
        Some(&super_admin_token),
        Some(json!({
            "chain": "BSC",
            "address": "bsc-extra-1",
            "is_enabled": true
        })),
    );
    assert_eq!(pool_status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());
    let (pools_status, pools_body) = http_json(
        "GET",
        &format!("{}/admin/address-pools", server.base_url()),
        Some(&super_admin_token),
        None,
    );
    assert_eq!(pools_status, StatusCode::OK.as_u16());
    assert!(!pools_body["addresses"]
        .as_array()
        .expect("addresses")
        .iter()
        .any(|entry| entry["chain"] == "BSC" && entry["address"] == "bsc-extra-1"));

    let (system_status, _) = http_json(
        "POST",
        &format!("{}/admin/system", server.base_url()),
        Some(&super_admin_token),
        Some(json!({
            "eth_confirmations": 6,
            "bsc_confirmations": 7,
            "sol_confirmations": 8
        })),
    );
    assert_eq!(system_status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());
    let (system_read_status, system_body) = http_json(
        "GET",
        &format!("{}/admin/system", server.base_url()),
        Some(&super_admin_token),
        None,
    );
    assert_eq!(system_read_status, StatusCode::OK.as_u16());
    assert_eq!(system_body["eth_confirmations"], 12);
    assert_eq!(system_body["bsc_confirmations"], 12);
    assert_eq!(system_body["sol_confirmations"], 12);
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
                .body(Body::from(json!({ "email": email }).to_string()))
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

async fn upsert_plan(
    app: &axum::Router,
    session_token: &str,
    payload: Value,
) -> axum::response::Response {
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
