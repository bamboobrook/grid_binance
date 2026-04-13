use axum::{
    extract::State,
    http::header,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use billing_chain_listener::{
    processor::{
        process_observed_transfer, promote_due_orders, ListenerMatchResult, ObservedChainTransfer,
        ProcessorError,
    },
    rpc::{
        collect_observed_transfers, parse_runtime_config, parse_sweep_executor_config,
        submit_sweep_transfer, sweep_transfer_confirmed, PollCursorState, RpcRuntimeConfig,
        SweepExecutorConfig,
    },
};
use reqwest::Client;
use serde_json::json;
use shared_db::{AuditLogRecord, SharedDb, SweepJobRecord, SweepTransferRecord};
use std::{
    io::{Error as IoError, ErrorKind},
    sync::{Arc, Mutex},
};
use tokio::{
    net::TcpListener,
    sync::Mutex as AsyncMutex,
    time::{interval, Duration as TokioDuration},
};

const DEFAULT_PORT: u16 = 8084;
const SERVICE_NAME: &str = "billing-chain-listener";
const DEFAULT_RPC_POLL_INTERVAL_SECS: u64 = 30;

#[derive(Debug, Clone, Default)]
struct ListenerMetrics {
    manual_review_total: usize,
    matched_total: u64,
    observed_transfers_total: u64,
    pool_enabled_addresses: usize,
    queue_promotions_total: u64,
    queued_orders: usize,
}

#[derive(Clone)]
struct ListenerState {
    db: SharedDb,
    internal_token: String,
    metrics: Arc<Mutex<ListenerMetrics>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = required_env("DATABASE_URL")?;
    let redis_url = required_env("REDIS_URL")?;
    let internal_token = required_env("INTERNAL_SHARED_SECRET")?;
    let db = SharedDb::connect(&database_url, &redis_url)?;
    let metrics = Arc::new(Mutex::new(ListenerMetrics::default()));
    refresh_listener_metrics(&db, &metrics, 0, 0, false)?;
    tokio::spawn(queue_promotion_loop(db.clone(), metrics.clone()));

    if let Ok(config) = parse_runtime_config() {
        tokio::spawn(rpc_polling_loop(
            db.clone(),
            metrics.clone(),
            Client::new(),
            config.clone(),
        ));
        if let Ok(executor) = parse_sweep_executor_config() {
            tokio::spawn(sweep_submission_loop(db.clone(), Client::new(), executor));
            tokio::spawn(sweep_confirmation_loop(db.clone(), Client::new(), config));
        } else {
            eprintln!("billing-chain-listener sweep execution disabled: SWEEP_EXECUTOR_URL not configured");
        }
    } else {
        eprintln!(
            "billing-chain-listener live rpc polling disabled: CHAIN_RPC_URL_ETH / CHAIN_RPC_URL_BSC / CHAIN_RPC_URL_SOL not fully configured"
        );
    }

    let listener = TcpListener::bind(("0.0.0.0", configured_port(DEFAULT_PORT))).await?;
    let app = build_router(ListenerState {
        db,
        internal_token,
        metrics,
    });

    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz(State(state): State<ListenerState>) -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        health_payload(SERVICE_NAME, &state.metrics),
    )
}

async fn ingest_transfer(
    State(state): State<ListenerState>,
    headers: axum::http::HeaderMap,
    Json(request): Json<ObservedChainTransfer>,
) -> Result<Json<ListenerMatchResult>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    authorize_internal_request(&headers, &state.internal_token)?;
    process_observed_transfer(&state.db, request)
        .map(|result| {
            let promoted = promote_due_orders(&state.db, chrono::Utc::now()).unwrap_or_default();
            let _ =
                refresh_listener_metrics(&state.db, &state.metrics, 1, promoted, result.matched);
            Json(result)
        })
        .map_err(map_processor_error)
}

async fn queue_promotion_loop(db: SharedDb, metrics: Arc<Mutex<ListenerMetrics>>) {
    let mut ticker = interval(TokioDuration::from_secs(30));
    loop {
        ticker.tick().await;
        let promoted = promote_due_orders(&db, chrono::Utc::now()).unwrap_or_default();
        let _ = refresh_listener_metrics(&db, &metrics, 0, promoted, false);
    }
}

async fn rpc_polling_loop(
    db: SharedDb,
    metrics: Arc<Mutex<ListenerMetrics>>,
    http: Client,
    config: RpcRuntimeConfig,
) {
    let state = Arc::new(AsyncMutex::new(PollCursorState::default()));
    let mut ticker = interval(TokioDuration::from_secs(configured_rpc_poll_interval_secs()));
    loop {
        ticker.tick().await;
        match collect_observed_transfers(&db, &http, &config, &state).await {
            Ok(transfers) => {
                for transfer in transfers {
                    match process_observed_transfer(&db, transfer) {
                        Ok(result) => {
                            let promoted =
                                promote_due_orders(&db, chrono::Utc::now()).unwrap_or_default();
                            let _ = refresh_listener_metrics(
                                &db,
                                &metrics,
                                1,
                                promoted,
                                result.matched,
                            );
                        }
                        Err(error) => {
                            eprintln!("billing-chain-listener failed to process observed transfer: {error}");
                        }
                    }
                }
                let _ = refresh_listener_metrics(&db, &metrics, 0, 0, false);
            }
            Err(error) => {
                eprintln!("billing-chain-listener live rpc polling failed: {error}");
            }
        }
    }
}

async fn sweep_submission_loop(db: SharedDb, http: Client, executor: SweepExecutorConfig) {
    let mut ticker = interval(TokioDuration::from_secs(30));
    loop {
        ticker.tick().await;
        process_sweep_submission_once(&db, &http, &executor).await;
    }
}

async fn process_sweep_submission_once(
    db: &SharedDb,
    http: &Client,
    executor: &SweepExecutorConfig,
) {
    let jobs = match db.list_sweep_jobs() {
        Ok(jobs) => jobs,
        Err(error) => {
            eprintln!("billing-chain-listener failed to list sweep jobs: {error}");
            return;
        }
    };
    for job in jobs.into_iter().filter(|job| job.status == "pending") {
        if !db
            .mark_sweep_job_submitting(job.sweep_job_id)
            .unwrap_or(false)
        {
            continue;
        }
        let mut submission_failed = None::<String>;
        let submitted_at = chrono::Utc::now();
        let mut submitted_any = false;
        for transfer in job
            .transfers
            .iter()
            .filter(|transfer| transfer.status == "pending")
        {
            match submit_sweep_transfer(
                http,
                executor,
                job.sweep_job_id,
                &job.chain,
                &job.asset,
                &transfer.from_address,
                &transfer.to_address,
                &transfer.amount,
            )
            .await
            {
                Ok(tx_hash) => {
                    let audit = sweep_transfer_audit(
                        "treasury.sweep_transfer_submitted",
                        &job,
                        transfer,
                        submitted_at,
                        "submitted",
                        Some(&tx_hash),
                        None,
                    );
                    if let Err(error) = db.mark_sweep_transfer_submitted_with_audit(
                        job.sweep_job_id,
                        &transfer.from_address,
                        &tx_hash,
                        submitted_at,
                        &audit,
                    ) {
                        submission_failed = Some(error.to_string());
                        break;
                    }
                    submitted_any = true;
                }
                Err(error) => {
                    let message = error.to_string();
                    let audit = sweep_transfer_audit(
                        "treasury.sweep_transfer_failed",
                        &job,
                        transfer,
                        submitted_at,
                        "failed",
                        transfer.tx_hash.as_deref(),
                        Some(&message),
                    );
                    match db.mark_sweep_transfer_failed_with_audit(
                        job.sweep_job_id,
                        &transfer.from_address,
                        submitted_at,
                        &message,
                        &audit,
                    ) {
                        Ok(_) => {
                            submission_failed = Some(message);
                        }
                        Err(storage_error) => {
                            submission_failed = Some(storage_error.to_string());
                        }
                    }
                    break;
                }
            }
        }
        if let Some(message) = submission_failed {
            let audit = sweep_job_audit(
                "treasury.sweep_job_failed",
                &job,
                submitted_at,
                "failed",
                Some(&message),
            );
            let _ = db.mark_sweep_job_failed_with_audit(
                job.sweep_job_id,
                submitted_at,
                &message,
                &audit,
            );
            continue;
        }
        if submitted_any {
            let audit = sweep_job_audit(
                "treasury.sweep_job_submitted",
                &job,
                submitted_at,
                "submitted",
                None,
            );
            let _ = db.mark_sweep_job_submitted_with_audit(job.sweep_job_id, submitted_at, &audit);
        } else {
            let message = "sweep job has no pending transfers";
            let audit = sweep_job_audit(
                "treasury.sweep_job_failed",
                &job,
                submitted_at,
                "failed",
                Some(message),
            );
            let _ = db.mark_sweep_job_failed_with_audit(
                job.sweep_job_id,
                submitted_at,
                message,
                &audit,
            );
        }
    }
}

async fn sweep_confirmation_loop(db: SharedDb, http: Client, config: RpcRuntimeConfig) {
    let mut ticker = interval(TokioDuration::from_secs(30));
    loop {
        ticker.tick().await;
        process_sweep_confirmation_once(&db, &http, &config).await;
    }
}

async fn process_sweep_confirmation_once(db: &SharedDb, http: &Client, config: &RpcRuntimeConfig) {
    let jobs = match db.list_sweep_jobs() {
        Ok(jobs) => jobs,
        Err(error) => {
            eprintln!("billing-chain-listener failed to list submitted sweeps: {error}");
            return;
        }
    };
    for job in jobs.into_iter().filter(|job| job.status == "submitted") {
        let mut all_confirmed = true;
        for transfer in job
            .transfers
            .iter()
            .filter(|transfer| transfer.status == "submitted")
        {
            let Some(tx_hash) = transfer.tx_hash.as_deref() else {
                all_confirmed = false;
                continue;
            };
            match sweep_transfer_confirmed(http, config, &job.chain, tx_hash).await {
                Ok(true) => {
                    let confirmed_at = chrono::Utc::now();
                    let audit = sweep_transfer_audit(
                        "treasury.sweep_transfer_confirmed",
                        &job,
                        transfer,
                        confirmed_at,
                        "confirmed",
                        Some(tx_hash),
                        None,
                    );
                    if let Err(error) = db.mark_sweep_transfer_confirmed_with_audit(
                        job.sweep_job_id,
                        &transfer.from_address,
                        confirmed_at,
                        &audit,
                    ) {
                        eprintln!(
                            "billing-chain-listener failed to persist confirmed sweep transfer {}: {}",
                            tx_hash, error
                        );
                        all_confirmed = false;
                    }
                }
                Ok(false) => {
                    all_confirmed = false;
                }
                Err(error) => {
                    eprintln!(
                        "billing-chain-listener failed to confirm sweep transfer {}: {}",
                        tx_hash, error
                    );
                    all_confirmed = false;
                }
            }
        }
        if all_confirmed {
            let refreshed = db.list_sweep_jobs().unwrap_or_default();
            if refreshed
                .iter()
                .find(|item| item.sweep_job_id == job.sweep_job_id)
                .is_some_and(|item| {
                    item.transfers
                        .iter()
                        .all(|transfer| transfer.status == "confirmed")
                })
            {
                let completed_at = chrono::Utc::now();
                let audit = sweep_job_audit(
                    "treasury.sweep_job_confirmed",
                    &job,
                    completed_at,
                    "confirmed",
                    None,
                );
                let _ =
                    db.mark_sweep_job_confirmed_with_audit(job.sweep_job_id, completed_at, &audit);
            }
        }
    }
}

fn sweep_transfer_audit(
    action: &str,
    job: &SweepJobRecord,
    transfer: &SweepTransferRecord,
    created_at: chrono::DateTime<chrono::Utc>,
    status: &str,
    tx_hash: Option<&str>,
    error_message: Option<&str>,
) -> AuditLogRecord {
    let mut payload = serde_json::Map::from_iter([
        ("chain".to_string(), json!(job.chain)),
        ("asset".to_string(), json!(job.asset)),
        ("sweep_job_id".to_string(), json!(job.sweep_job_id)),
        ("from_address".to_string(), json!(transfer.from_address)),
        ("to_address".to_string(), json!(transfer.to_address)),
        ("amount".to_string(), json!(transfer.amount)),
        ("status".to_string(), json!(status)),
    ]);
    if let Some(tx_hash) = tx_hash {
        payload.insert("tx_hash".to_string(), json!(tx_hash));
    }
    if let Some(error_message) = error_message {
        payload.insert("error_message".to_string(), json!(error_message));
    }
    AuditLogRecord {
        actor_email: SERVICE_NAME.to_string(),
        action: action.to_string(),
        target_type: "sweep_transfer".to_string(),
        target_id: format!("{}:{}", job.sweep_job_id, transfer.from_address),
        payload: serde_json::Value::Object(payload),
        created_at,
    }
}

fn sweep_job_audit(
    action: &str,
    job: &SweepJobRecord,
    created_at: chrono::DateTime<chrono::Utc>,
    status: &str,
    last_error: Option<&str>,
) -> AuditLogRecord {
    let mut payload = serde_json::Map::from_iter([
        ("chain".to_string(), json!(job.chain)),
        ("asset".to_string(), json!(job.asset)),
        ("sweep_job_id".to_string(), json!(job.sweep_job_id)),
        ("status".to_string(), json!(status)),
        ("transfer_count".to_string(), json!(job.transfers.len())),
    ]);
    if let Some(last_error) = last_error {
        payload.insert("last_error".to_string(), json!(last_error));
    }
    AuditLogRecord {
        actor_email: SERVICE_NAME.to_string(),
        action: action.to_string(),
        target_type: "sweep_job".to_string(),
        target_id: job.sweep_job_id.to_string(),
        payload: serde_json::Value::Object(payload),
        created_at,
    }
}

fn refresh_listener_metrics(
    db: &SharedDb,
    metrics: &Arc<Mutex<ListenerMetrics>>,
    observed_delta: u64,
    promoted: usize,
    matched: bool,
) -> Result<(), shared_db::SharedDbError> {
    let orders = db.list_billing_orders()?;
    let addresses = db.list_deposit_addresses()?;
    let deposits = db.list_deposit_transactions()?;
    let mut guard = metrics.lock().expect("listener metrics poisoned");
    guard.observed_transfers_total += observed_delta;
    if matched {
        guard.matched_total += 1;
    }
    guard.queue_promotions_total += promoted as u64;
    guard.pool_enabled_addresses = addresses.iter().filter(|record| record.is_enabled).count();
    guard.queued_orders = orders
        .iter()
        .filter(|order| order.status == "queued")
        .count();
    guard.manual_review_total = deposits
        .iter()
        .filter(|deposit| deposit.status == "manual_review_required")
        .count();
    Ok(())
}

fn authorize_internal_request(
    headers: &axum::http::HeaderMap,
    expected_token: &str,
) -> Result<(), (axum::http::StatusCode, Json<serde_json::Value>)> {
    let supplied = headers
        .get("x-internal-token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or((
            axum::http::StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "internal token required" })),
        ))?;
    if supplied != expected_token {
        return Err((
            axum::http::StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "invalid internal token" })),
        ));
    }
    Ok(())
}

fn map_processor_error(error: ProcessorError) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    match error {
        ProcessorError::InvalidRequest(message) => (
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({ "error": message })),
        ),
        ProcessorError::Storage(storage) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": storage.to_string() })),
        ),
    }
}

fn configured_port(default_port: u16) -> u16 {
    parse_port(std::env::var("PORT").ok(), default_port)
}

fn configured_rpc_poll_interval_secs() -> u64 {
    std::env::var("CHAIN_LISTENER_RPC_POLL_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_RPC_POLL_INTERVAL_SECS)
}

fn required_env(name: &str) -> Result<String, IoError> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| IoError::new(ErrorKind::InvalidInput, format!("{name} is required")))
}

fn parse_port(value: Option<String>, default_port: u16) -> u16 {
    value
        .and_then(|port| port.parse().ok())
        .unwrap_or(default_port)
}

fn health_payload(service_name: &str, metrics: &Arc<Mutex<ListenerMetrics>>) -> String {
    let guard = metrics.lock().expect("listener metrics poisoned");
    format!(
        "# HELP service_up Service health probe status.\n# TYPE service_up gauge\nservice_up{{service=\"{service_name}\"}} 1\n# HELP billing_listener_observed_transfers_total Observed chain transfers processed by the listener.\n# TYPE billing_listener_observed_transfers_total counter\nbilling_listener_observed_transfers_total {observed}\n# HELP billing_listener_matched_total Automatically matched deposits.\n# TYPE billing_listener_matched_total counter\nbilling_listener_matched_total {matched}\n# HELP billing_listener_queue_promotions_total Queue promotions after address release.\n# TYPE billing_listener_queue_promotions_total counter\nbilling_listener_queue_promotions_total {promotions}\n# HELP billing_listener_queued_orders Current queued billing orders.\n# TYPE billing_listener_queued_orders gauge\nbilling_listener_queued_orders {queued}\n# HELP billing_listener_pool_enabled_addresses Enabled addresses in the billing pool.\n# TYPE billing_listener_pool_enabled_addresses gauge\nbilling_listener_pool_enabled_addresses {addresses}\n# HELP billing_listener_manual_review_total Current manual review backlog.\n# TYPE billing_listener_manual_review_total gauge\nbilling_listener_manual_review_total {manual_review}\n",
        observed = guard.observed_transfers_total,
        matched = guard.matched_total,
        promotions = guard.queue_promotions_total,
        queued = guard.queued_orders,
        addresses = guard.pool_enabled_addresses,
        manual_review = guard.manual_review_total,
    )
}

fn build_router(state: ListenerState) -> Router {
    let router = Router::new().route("/healthz", get(healthz));
    let router = if internal_ingest_enabled() {
        router.route("/internal/observed-transfers", post(ingest_transfer))
    } else {
        router
    };
    router.with_state(state)
}

fn internal_ingest_enabled() -> bool {
    if std::env::var("APP_ENV")
        .ok()
        .map(|value| value.trim().eq_ignore_ascii_case("test"))
        .unwrap_or(false)
    {
        return true;
    }

    std::env::var("CHAIN_LISTENER_ALLOW_INTERNAL_INGEST")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{
        build_router, configured_port, health_payload, internal_ingest_enabled, parse_port,
        process_sweep_confirmation_once, process_sweep_submission_once, required_env,
        ListenerMetrics, ListenerState, RpcRuntimeConfig, SweepExecutorConfig, DEFAULT_PORT,
        SERVICE_NAME,
    };
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use shared_db::{SharedDb, SweepJobRecord, SweepTransferRecord};
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex, OnceLock},
    };
    use tower::ServiceExt;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    async fn spawn_json_server(router: axum::Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let address = listener.local_addr().expect("test server addr");
        tokio::spawn(async move {
            axum::serve(listener, router)
                .await
                .expect("serve test server");
        });
        format!("http://{address}")
    }

    fn sample_sweep_job(
        status: &str,
        transfer_status: &str,
        tx_hash: Option<&str>,
        submitted_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> SweepJobRecord {
        let requested_at = Utc.with_ymd_and_hms(2026, 4, 1, 0, 14, 0).single().unwrap();
        SweepJobRecord {
            sweep_job_id: 41,
            chain: "BSC".to_string(),
            asset: "USDT".to_string(),
            status: status.to_string(),
            requested_by: "super-admin@example.com".to_string(),
            requested_at,
            treasury_address: "bsc-treasury-1".to_string(),
            submitted_at,
            completed_at: None,
            failed_at: None,
            last_error: None,
            attempt_count: 0,
            transfers: vec![SweepTransferRecord {
                from_address: "bsc-addr-1".to_string(),
                to_address: "bsc-treasury-1".to_string(),
                amount: "20.00000000".to_string(),
                tx_hash: tx_hash.map(str::to_string),
                status: transfer_status.to_string(),
                submitted_at,
                confirmed_at: None,
                failed_at: None,
                error_message: None,
            }],
        }
    }

    #[tokio::test]
    async fn sweep_submission_records_submitted_audit_logs() {
        let db = SharedDb::ephemeral().expect("db");
        db.create_sweep_job(&sample_sweep_job("pending", "pending", None, None))
            .expect("create sweep job");
        let url = spawn_json_server(axum::Router::new().route(
            "/",
            axum::routing::post(|| async { axum::Json(json!({ "tx_hash": "0xsweep-submitted" })) }),
        ))
        .await;

        process_sweep_submission_once(
            &db,
            &reqwest::Client::new(),
            &SweepExecutorConfig {
                url,
                auth_token: None,
            },
        )
        .await;

        let jobs = db.list_sweep_jobs().expect("sweep jobs");
        assert_eq!(jobs[0].status, "submitted");
        assert_eq!(jobs[0].transfers[0].status, "submitted");
        assert_eq!(
            jobs[0].transfers[0].tx_hash.as_deref(),
            Some("0xsweep-submitted")
        );

        let audit_logs = db.list_audit_logs().expect("audit logs");
        assert!(audit_logs.iter().any(|record| {
            record.action == "treasury.sweep_transfer_submitted"
                && record.target_id == "41:bsc-addr-1"
                && record.payload["tx_hash"] == "0xsweep-submitted"
        }));
        assert!(audit_logs.iter().any(|record| {
            record.action == "treasury.sweep_job_submitted"
                && record.target_id == "41"
                && record.payload["status"] == "submitted"
        }));
    }

    #[tokio::test]
    async fn sweep_submission_failures_record_failed_audit_logs() {
        let db = SharedDb::ephemeral().expect("db");
        db.create_sweep_job(&sample_sweep_job("pending", "pending", None, None))
            .expect("create sweep job");
        let url = spawn_json_server(axum::Router::new().route(
            "/",
            axum::routing::post(|| async {
                (
                    StatusCode::BAD_GATEWAY,
                    axum::Json(json!({ "error": "executor offline" })),
                )
            }),
        ))
        .await;

        process_sweep_submission_once(
            &db,
            &reqwest::Client::new(),
            &SweepExecutorConfig {
                url,
                auth_token: None,
            },
        )
        .await;

        let jobs = db.list_sweep_jobs().expect("sweep jobs");
        assert_eq!(jobs[0].status, "failed");
        assert_eq!(jobs[0].transfers[0].status, "failed");
        assert!(jobs[0]
            .last_error
            .as_deref()
            .unwrap_or_default()
            .contains("502"));

        let audit_logs = db.list_audit_logs().expect("audit logs");
        assert!(audit_logs.iter().any(|record| {
            record.action == "treasury.sweep_transfer_failed"
                && record.target_id == "41:bsc-addr-1"
                && record.payload["error_message"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("502")
        }));
        assert!(audit_logs.iter().any(|record| {
            record.action == "treasury.sweep_job_failed"
                && record.target_id == "41"
                && record.payload["last_error"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("502")
        }));
    }

    #[tokio::test]
    async fn sweep_confirmation_records_confirmed_audit_logs() {
        let submitted_at = Utc.with_ymd_and_hms(2026, 4, 1, 0, 15, 0).single().unwrap();
        let db = SharedDb::ephemeral().expect("db");
        db.create_sweep_job(&sample_sweep_job(
            "submitted",
            "submitted",
            Some("0xsweep-submitted"),
            Some(submitted_at),
        ))
        .expect("create submitted sweep job");
        let url = spawn_json_server(axum::Router::new().route(
            "/",
            axum::routing::post(|| async {
                axum::Json(json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": { "status": "0x1" }
                }))
            }),
        ))
        .await;

        process_sweep_confirmation_once(
            &db,
            &reqwest::Client::new(),
            &RpcRuntimeConfig {
                eth_rpc_url: url.clone(),
                bsc_rpc_url: url.clone(),
                sol_rpc_url: url,
                token_registry: BTreeMap::new(),
                evm_initial_lookback_blocks: 64,
                sol_signature_limit: 50,
            },
        )
        .await;

        let jobs = db.list_sweep_jobs().expect("sweep jobs");
        assert_eq!(jobs[0].status, "confirmed");
        assert_eq!(jobs[0].transfers[0].status, "confirmed");
        assert!(jobs[0].completed_at.is_some());

        let audit_logs = db.list_audit_logs().expect("audit logs");
        assert!(audit_logs.iter().any(|record| {
            record.action == "treasury.sweep_transfer_confirmed"
                && record.target_id == "41:bsc-addr-1"
                && record.payload["tx_hash"] == "0xsweep-submitted"
        }));
        assert!(audit_logs.iter().any(|record| {
            record.action == "treasury.sweep_job_confirmed"
                && record.target_id == "41"
                && record.payload["status"] == "confirmed"
        }));
    }

    #[test]
    fn health_payload_mentions_service_name() {
        let payload = health_payload(
            SERVICE_NAME,
            &Arc::new(Mutex::new(ListenerMetrics::default())),
        );

        assert!(payload.contains("service_up"));
        assert!(payload.contains("billing_listener_queued_orders"));
    }

    #[test]
    fn parse_port_falls_back_when_value_is_missing_or_invalid() {
        assert_eq!(parse_port(None, DEFAULT_PORT), DEFAULT_PORT);
        assert_eq!(
            parse_port(Some("not-a-port".to_string()), DEFAULT_PORT),
            DEFAULT_PORT
        );
    }

    #[test]
    fn required_env_requires_runtime_storage_urls() {
        std::env::remove_var("REDIS_URL");
        assert!(required_env("REDIS_URL").is_err());

        std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379/0");
        assert!(required_env("REDIS_URL").is_ok());
        std::env::remove_var("REDIS_URL");
    }

    #[test]
    fn configured_port_uses_default_when_missing() {
        assert_eq!(configured_port(DEFAULT_PORT), DEFAULT_PORT);
    }

    #[test]
    fn internal_ingest_enabled_defaults_false_and_accepts_explicit_flag() {
        std::env::remove_var("APP_ENV");
        std::env::remove_var("CHAIN_LISTENER_ALLOW_INTERNAL_INGEST");
        assert!(!internal_ingest_enabled());

        std::env::set_var("CHAIN_LISTENER_ALLOW_INTERNAL_INGEST", "1");
        assert!(internal_ingest_enabled());
        std::env::remove_var("CHAIN_LISTENER_ALLOW_INTERNAL_INGEST");
    }

    #[tokio::test]
    async fn internal_observed_transfer_route_is_disabled_without_explicit_test_or_override() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::remove_var("APP_ENV");
        std::env::remove_var("CHAIN_LISTENER_ALLOW_INTERNAL_INGEST");
        let app = build_router(ListenerState {
            db: SharedDb::ephemeral().expect("db"),
            internal_token: "secret".to_string(),
            metrics: Arc::new(Mutex::new(ListenerMetrics::default())),
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/observed-transfers")
                    .header("x-internal-token", "secret")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn internal_observed_transfer_route_is_available_when_explicitly_enabled() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::remove_var("APP_ENV");
        std::env::set_var("CHAIN_LISTENER_ALLOW_INTERNAL_INGEST", "1");
        let app = build_router(ListenerState {
            db: SharedDb::ephemeral().expect("db"),
            internal_token: "secret".to_string(),
            metrics: Arc::new(Mutex::new(ListenerMetrics::default())),
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/observed-transfers")
                    .header("x-internal-token", "secret")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "chain": "",
                            "asset": "USDT",
                            "address": "addr-1",
                            "amount": "1.00000000",
                            "tx_hash": "tx-1",
                            "observed_at": "2026-04-01T00:00:00Z"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        std::env::remove_var("CHAIN_LISTENER_ALLOW_INTERNAL_INGEST");
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn observed_transfer_endpoint_rejects_invalid_payload_with_422() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("APP_ENV", "test");
        std::env::remove_var("CHAIN_LISTENER_ALLOW_INTERNAL_INGEST");
        let app = build_router(ListenerState {
            db: SharedDb::ephemeral().expect("db"),
            internal_token: "secret".to_string(),
            metrics: Arc::new(Mutex::new(ListenerMetrics::default())),
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/observed-transfers")
                    .header("x-internal-token", "secret")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "chain": "",
                            "asset": "USDT",
                            "address": "addr-1",
                            "amount": "1.00000000",
                            "tx_hash": "tx-1",
                            "observed_at": "2026-04-01T00:00:00Z"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        std::env::remove_var("APP_ENV");
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["error"], "invalid chain");
    }
}
