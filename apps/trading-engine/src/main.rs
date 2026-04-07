use axum::{extract::State, http::header, response::IntoResponse, routing::get, Router};
use chrono::Utc;
use futures_util::StreamExt;
use shared_binance::{parse_user_data_message, BinanceClient, CredentialCipher};
use shared_db::{NotificationLogRecord, SharedDb};
use shared_events::{MarketTick, NotificationKind};
use shared_domain::strategy::{Strategy, StrategyRuntimeEvent, StrategyStatus};
use std::{
    collections::{HashMap, HashSet},
    io::{Error as IoError, ErrorKind},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::TcpListener, task::JoinHandle, time::sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use trading_engine::{execution_effects::persist_execution_effects, execution_sync::apply_execution_update, order_sync::sync_strategy_orders, strategy_runtime::StrategyRuntimeEngine, trade_sync::sync_strategy_trades};

const DEFAULT_PORT: u16 = 8081;
const SERVICE_NAME: &str = "trading-engine";
const DEFAULT_RECONCILE_INTERVAL_SECS: u64 = 5;
const BINANCE_EXCHANGE: &str = "binance";

#[derive(Debug, Clone, Default)]
struct RuntimeMetrics {
    active_strategies: usize,
    error_paused_strategies: usize,
    reconcile_failures_total: u64,
    reconcile_runs_total: u64,
    last_reconcile_at: Option<i64>,
}

#[derive(Clone)]
struct EngineState {
    metrics: Arc<Mutex<RuntimeMetrics>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = required_env("DATABASE_URL")?;
    let redis_url = required_env("REDIS_URL")?;
    let db = SharedDb::connect(&database_url, &redis_url)?;
    let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
    let db_for_loop = db.clone();
    let metrics_for_loop = metrics.clone();
    tokio::spawn(async move {
        loop {
            if run_engine_iteration(
                || reconcile_once(&db_for_loop, &metrics_for_loop),
                || sync_user_streams(&db_for_loop),
            )
            .is_err()
            {
                let mut guard = metrics_for_loop.lock().expect("metrics poisoned");
                guard.reconcile_failures_total += 1;
                guard.last_reconcile_at = Some(Utc::now().timestamp());
            }
            sleep(Duration::from_secs(configured_reconcile_interval_secs())).await;
        }
    });

    let listener = TcpListener::bind(("0.0.0.0", configured_port(DEFAULT_PORT))).await?;
    let app = Router::new()
        .route("/healthz", get(healthz))
        .with_state(EngineState { metrics });

    axum::serve(listener, app).await?;
    Ok(())
}

fn run_engine_iteration<F, G, E>(reconcile: F, sync_streams: G) -> Result<(), E>
where
    F: FnOnce() -> Result<(), E>,
    G: FnOnce() -> Result<(), E>,
{
    let reconcile_result = reconcile();
    let sync_result = sync_streams();
    match (reconcile_result, sync_result) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(_), Ok(_)) => Ok(()),
    }
}

async fn healthz(State(state): State<EngineState>) -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        health_payload(SERVICE_NAME, &state.metrics),
    )
}

fn reconcile_once(db: &SharedDb, metrics: &Arc<Mutex<RuntimeMetrics>>) -> Result<(), shared_db::SharedDbError> {
    let mut active = 0usize;
    let mut error_paused = 0usize;
    let live_mode = live_mode_enabled();
    let cipher = if live_mode { Some(credential_cipher()?) } else { None };
    let market_ticks = db.drain_market_ticks(256)?;
    for mut strategy in db.list_all_strategies()? {
        let mut dirty = false;
        if let Some(cipher) = cipher.as_ref() {
            dirty |= sync_live_orders(db, &mut strategy, cipher)?;
        }
        dirty |= apply_market_ticks(db, &mut strategy, &market_ticks)?;
        match strategy.status {
            StrategyStatus::Running => {
                let revision = strategy.active_revision.clone().unwrap_or_else(|| strategy.draft_revision.clone());
                if let Err(error) = StrategyRuntimeEngine::new(&strategy.id, strategy.market, strategy.mode, revision) {
                    strategy.status = StrategyStatus::ErrorPaused;
                    strategy.runtime.events.push(StrategyRuntimeEvent {
                        event_type: "runtime_reconcile_failed".to_string(),
                        detail: error.to_string(),
                        price: None,
                        created_at: Utc::now(),
                    });
                    persist_runtime_notification(
                        db,
                        &strategy,
                        NotificationKind::RuntimeError,
                        "Runtime failure",
                        &format!("{} failed runtime validation: {}", strategy.name, error),
                        serde_json::json!({ "strategy_id": strategy.id, "reason": error.to_string() }),
                        Utc::now(),
                    )?;
                    dirty = true;
                    error_paused += 1;
                } else {
                    active += 1;
                }
            }
            StrategyStatus::ErrorPaused => {
                error_paused += 1;
            }
            _ => {}
        }
        if dirty {
            db.update_strategy(&strategy)?;
        }
    }

    let mut guard = metrics.lock().expect("metrics poisoned");
    guard.active_strategies = active;
    guard.error_paused_strategies = error_paused;
    guard.reconcile_runs_total += 1;
    guard.last_reconcile_at = Some(Utc::now().timestamp());
    Ok(())
}


fn sync_live_orders(
    db: &SharedDb,
    strategy: &mut Strategy,
    cipher: &CredentialCipher,
) -> Result<bool, shared_db::SharedDbError> {
    if !matches!(
        strategy.status,
        StrategyStatus::Running | StrategyStatus::Paused | StrategyStatus::Stopped | StrategyStatus::ErrorPaused
    ) {
        return Ok(false);
    }
    let Some(account) = db.find_exchange_account(&strategy.owner_email, BINANCE_EXCHANGE)? else {
        return Ok(false);
    };
    if !account.is_active {
        return Ok(false);
    }
    let Some(credentials) = db.find_exchange_credentials(&strategy.owner_email, BINANCE_EXCHANGE)? else {
        return Ok(false);
    };
    let (api_key, api_secret) = cipher
        .decrypt(&credentials.encrypted_secret)
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    let client = shared_binance::BinanceClient::new(api_key, api_secret);
    let result = sync_strategy_orders(strategy, &client);
    let trade_result = sync_strategy_trades(db, strategy, &client)?;
    if result.submitted > 0 {
        strategy.runtime.events.push(StrategyRuntimeEvent {
            event_type: "live_orders_submitted".to_string(),
            detail: format!("submitted {} live orders", result.submitted),
            price: None,
            created_at: Utc::now(),
        });
    }
    if result.canceled > 0 {
        strategy.runtime.events.push(StrategyRuntimeEvent {
            event_type: "live_orders_canceled".to_string(),
            detail: format!("canceled {} live orders", result.canceled),
            price: None,
            created_at: Utc::now(),
        });
    }
    if trade_result.new_fills > 0 {
        strategy.runtime.events.push(StrategyRuntimeEvent {
            event_type: "live_trade_sync_completed".to_string(),
            detail: format!("recorded {} exchange fills", trade_result.new_fills),
            price: None,
            created_at: Utc::now(),
        });
    }
    if result.failed > 0 {
        apply_live_sync_failure(db, strategy, result.failed, Utc::now())?;
    }
    Ok(result.submitted > 0 || result.canceled > 0 || result.failed > 0 || trade_result.new_fills > 0)
}

fn apply_live_sync_failure(
    db: &SharedDb,
    strategy: &mut Strategy,
    failed: usize,
    created_at: chrono::DateTime<Utc>,
) -> Result<bool, shared_db::SharedDbError> {
    strategy.runtime.events.push(StrategyRuntimeEvent {
        event_type: "live_order_sync_failed".to_string(),
        detail: format!("{} live order sync actions failed", failed),
        price: None,
        created_at,
    });
    if matches!(strategy.status, StrategyStatus::Running) {
        strategy.status = StrategyStatus::ErrorPaused;
    }
    persist_runtime_notification(
        db,
        strategy,
        NotificationKind::RuntimeError,
        "Live order sync failed",
        &format!(
            "{} failed to reconcile {} live exchange actions on {}. Check API credentials, symbol permissions, and open orders before resuming.",
            strategy.name, failed, strategy.symbol
        ),
        serde_json::json!({
            "strategy_id": strategy.id,
            "failed_actions": failed,
            "reason": "live_order_sync_failed",
        }),
        created_at,
    )?;
    Ok(true)
}

fn apply_market_ticks(
    db: &SharedDb,
    strategy: &mut Strategy,
    market_ticks: &[MarketTick],
) -> Result<bool, shared_db::SharedDbError> {
    if strategy.status != StrategyStatus::Running {
        return Ok(false);
    }

    let relevant = market_ticks
        .iter()
        .filter(|tick| {
            tick.symbol.eq_ignore_ascii_case(&strategy.symbol)
                && tick.market == strategy_market_code(strategy.market)
        })
        .collect::<Vec<_>>();
    if relevant.is_empty() {
        return Ok(false);
    }

    let revision = strategy
        .active_revision
        .clone()
        .unwrap_or_else(|| strategy.draft_revision.clone());
    let mut engine = StrategyRuntimeEngine::from_runtime_snapshot(
        &strategy.id,
        strategy.market,
        strategy.mode,
        revision,
        strategy.runtime.clone(),
        true,
    )
    .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;

    let mut changed = false;
    let before_event_count = strategy.runtime.events.len();
    for tick in relevant {
        let events = engine
            .on_price(tick.price)
            .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
        if !events.is_empty() {
            changed = true;
        }
    }

    let snapshot = engine.snapshot().clone();
    for event in snapshot.events.iter().skip(before_event_count) {
        if event.event_type.starts_with("overall_take_profit") {
            persist_runtime_notification(
                db,
                strategy,
                NotificationKind::OverallTakeProfitTriggered,
                "Overall take profit reached",
                &format!("{} reached overall take profit on {}.", strategy.name, strategy.symbol),
                serde_json::json!({
                    "strategy_id": strategy.id,
                    "trigger_price": event.price.map(|price| price.to_string()).unwrap_or_default(),
                }),
                event.created_at,
            )?;
        }
        if event.event_type.starts_with("overall_stop_loss") {
            persist_runtime_notification(
                db,
                strategy,
                NotificationKind::OverallStopLossTriggered,
                "Overall stop loss reached",
                &format!("{} reached overall stop loss on {}.", strategy.name, strategy.symbol),
                serde_json::json!({
                    "strategy_id": strategy.id,
                    "trigger_price": event.price.map(|price| price.to_string()).unwrap_or_default(),
                }),
                event.created_at,
            )?;
        }
    }

    let next_runtime = engine.snapshot().clone();
    if next_runtime != strategy.runtime {
        strategy.runtime = next_runtime;
        changed = true;
    }
    if !engine.is_running() && strategy.status == StrategyStatus::Running {
        strategy.status = StrategyStatus::Stopped;
        changed = true;
    }

    Ok(changed)
}

fn strategy_market_code(market: shared_domain::strategy::StrategyMarket) -> &'static str {
    match market {
        shared_domain::strategy::StrategyMarket::Spot => "spot",
        shared_domain::strategy::StrategyMarket::FuturesUsdM => "usdm",
        shared_domain::strategy::StrategyMarket::FuturesCoinM => "coinm",
    }
}

fn persist_runtime_notification(
    db: &SharedDb,
    strategy: &Strategy,
    kind: NotificationKind,
    title: &str,
    body: &str,
    payload: serde_json::Value,
    created_at: chrono::DateTime<Utc>,
) -> Result<(), shared_db::SharedDbError> {
    let binding = db.find_telegram_binding(&strategy.owner_email)?;
    let telegram_delivered = match (binding.as_ref(), telegram_bot_token()) {
        (Some(binding), Some(token)) => send_telegram_message(&token, &binding.telegram_chat_id, title, body).is_ok(),
        _ => false,
    };
    let mut record_payload = payload.clone();
    if let Some(object) = record_payload.as_object_mut() {
        object.insert("telegram_delivered".to_string(), serde_json::json!(telegram_delivered));
    }
    db.insert_notification_log(&NotificationLogRecord {
        user_email: strategy.owner_email.clone(),
        channel: "in_app".to_string(),
        template_key: Some(format!("{:?}", kind)),
        title: title.to_string(),
        body: body.to_string(),
        status: "delivered".to_string(),
        payload: record_payload,
        created_at,
        delivered_at: Some(created_at),
    })?;
    if binding.is_some() {
        db.insert_notification_log(&NotificationLogRecord {
            user_email: strategy.owner_email.clone(),
            channel: "telegram".to_string(),
            template_key: Some(format!("{:?}", kind)),
            title: title.to_string(),
            body: body.to_string(),
            status: if telegram_delivered { "delivered" } else { "failed" }.to_string(),
            payload,
            created_at,
            delivered_at: telegram_delivered.then_some(created_at),
        })?;
    }
    Ok(())
}

fn telegram_bot_token() -> Option<String> {
    std::env::var("TELEGRAM_BOT_TOKEN")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn telegram_api_base_url() -> String {
    std::env::var("TELEGRAM_API_BASE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "https://api.telegram.org".to_string())
}

fn telegram_http_agent() -> &'static ureq::Agent {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT.get_or_init(|| ureq::AgentBuilder::new().timeout(Duration::from_secs(5)).build())
}

fn send_telegram_message(
    bot_token: &str,
    chat_id: &str,
    title: &str,
    body: &str,
) -> Result<(), shared_db::SharedDbError> {
    telegram_http_agent()
        .post(&format!("{}/bot{}/sendMessage", telegram_api_base_url(), bot_token))
        .send_json(ureq::json!({
            "chat_id": chat_id,
            "text": format!("{}\n{}", title, body),
        }))
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    Ok(())
}

fn live_mode_enabled() -> bool {
    std::env::var("BINANCE_LIVE_MODE")
        .ok()
        .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on" | "live"))
        .unwrap_or(false)
}

fn credential_cipher() -> Result<CredentialCipher, shared_db::SharedDbError> {
    CredentialCipher::from_env("EXCHANGE_CREDENTIALS_MASTER_KEY")
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))
}

fn sync_user_streams(db: &SharedDb) -> Result<(), shared_db::SharedDbError> {
    static HANDLES: std::sync::OnceLock<Mutex<HashMap<String, JoinHandle<()>>>> = std::sync::OnceLock::new();
    let handles = HANDLES.get_or_init(|| Mutex::new(HashMap::new()));
    let accounts = db.list_active_exchange_accounts(BINANCE_EXCHANGE)?;
    let desired = desired_user_stream_keys(
        live_mode_enabled(),
        &accounts
            .iter()
            .map(|account| (account.user_email.as_str(), account.market_scope.as_str()))
            .collect::<Vec<_>>(),
    );
    let desired_set = desired.iter().cloned().collect::<HashSet<_>>();

    let mut guard = handles.lock().expect("stream handles poisoned");
    let existing = guard.keys().cloned().collect::<Vec<_>>();
    for key in existing {
        let finished = guard.get(&key).is_some_and(|handle| handle.is_finished());
        if finished || !desired_set.contains(&key) {
            if let Some(handle) = guard.remove(&key) {
                handle.abort();
            }
        }
    }

    for account in accounts {
        for market in selected_markets_from_scope(&account.market_scope) {
            let key = format!("{}:{}", account.user_email.to_lowercase(), market);
            if guard.contains_key(&key) {
                continue;
            }
            let db = db.clone();
            let email = account.user_email.clone();
            guard.insert(
                key,
                tokio::spawn(async move {
                    user_stream_task(db, email, market).await;
                }),
            );
        }
    }
    Ok(())
}

async fn user_stream_task(db: SharedDb, email: String, market: String) {
    loop {
        if let Err(error) = run_user_stream_once(&db, &email, &market).await {
            eprintln!("trading-engine user stream {} {} failed: {error}", email, market);
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn run_user_stream_once(db: &SharedDb, email: &str, market: &str) -> Result<(), shared_db::SharedDbError> {
    let Some(credentials) = db.find_exchange_credentials(email, BINANCE_EXCHANGE)? else {
        return Ok(());
    };
    let cipher = credential_cipher()?;
    let (api_key, api_secret) = cipher
        .decrypt(&credentials.encrypted_secret)
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    let client = BinanceClient::new(api_key, api_secret);
    let stream = client
        .start_user_data_stream(market)
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    let keepalive_client = client.clone();
    let keepalive_market = market.to_string();
    let keepalive_email = email.to_string();
    let keepalive_key = stream.listen_key.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30 * 60));
        loop {
            interval.tick().await;
            if let Err(error) = keepalive_client.keepalive_user_data_stream(&keepalive_market, &keepalive_key) {
                eprintln!("trading-engine user stream keepalive {} {} failed: {}", keepalive_email, keepalive_market, error);
                break;
            }
        }
    });
    let (socket, _) = connect_async(stream.websocket_url)
        .await
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    let (_, mut read) = socket.split();

    while let Some(message) = read.next().await {
        let message = message.map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
        let payload = match message {
            Message::Text(text) => text.to_string(),
            Message::Binary(bytes) => String::from_utf8(bytes.to_vec())
                .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?,
            Message::Close(_) => break,
            _ => continue,
        };
        if let Some(update) = parse_user_data_message(market, &payload) {
            apply_execution_update_for_user(db, email, &client, &update)?;
        }
    }
    Ok(())
}

fn apply_execution_update_effects(
    db: &SharedDb,
    strategy: &mut Strategy,
    update: &shared_binance::BinanceExecutionUpdate,
) -> Result<bool, shared_db::SharedDbError> {
    let changed = apply_execution_update(strategy, update);
    if !changed {
        return Ok(false);
    }
    let effects = persist_execution_effects(db, strategy, update)?;
    strategy.runtime.events.push(StrategyRuntimeEvent {
        event_type: "execution_effects_persisted".to_string(),
        detail: format!("persisted {} execution-side trades", effects.new_trades),
        price: None,
        created_at: Utc::now(),
    });
    Ok(true)
}

fn apply_execution_update_for_user(
    db: &SharedDb,
    email: &str,
    client: &BinanceClient,
    update: &shared_binance::BinanceExecutionUpdate,
) -> Result<(), shared_db::SharedDbError> {
    for mut strategy in db.list_strategies(email)? {
        let changed = apply_execution_update_effects(db, &mut strategy, update)?;
        if changed {
            let _ = sync_strategy_trades(db, &mut strategy, client)?;
            db.update_strategy(&strategy)?;
        }
    }
    Ok(())
}

fn desired_user_stream_keys(live_enabled: bool, accounts: &[(&str, &str)]) -> Vec<String> {
    if !live_enabled {
        return Vec::new();
    }
    let mut keys = Vec::new();
    for (email, scope) in accounts {
        for market in selected_markets_from_scope(scope) {
            keys.push(format!("{}:{}", email.to_lowercase(), market));
        }
    }
    keys.sort();
    keys
}

fn selected_markets_from_scope(scope: &str) -> Vec<String> {
    let mut markets = scope
        .split(',')
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "spot" | "usdm" | "coinm"))
        .collect::<Vec<_>>();
    if markets.is_empty() {
        markets.push("spot".to_string());
    }
    markets.sort();
    markets.dedup();
    markets
}

fn configured_reconcile_interval_secs() -> u64 {
    std::env::var("TRADING_ENGINE_RECONCILE_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_RECONCILE_INTERVAL_SECS)
}

fn configured_port(default_port: u16) -> u16 {
    parse_port(std::env::var("PORT").ok(), default_port)
}

fn required_env(name: &str) -> Result<String, IoError> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| IoError::new(ErrorKind::InvalidInput, format!("{name} is required")))
}

fn parse_port(value: Option<String>, default_port: u16) -> u16 {
    value.and_then(|port| port.parse().ok()).unwrap_or(default_port)
}

fn health_payload(service_name: &str, metrics: &Arc<Mutex<RuntimeMetrics>>) -> String {
    let guard = metrics.lock().expect("metrics poisoned");
    format!(
        "# HELP service_up Service health probe status.\n# TYPE service_up gauge\nservice_up{{service=\"{service_name}\"}} 1\n# HELP trading_engine_active_strategies Running strategies seen by the reconcile loop.\n# TYPE trading_engine_active_strategies gauge\ntrading_engine_active_strategies {active}\n# HELP trading_engine_error_paused_strategies Error-paused strategies seen by the reconcile loop.\n# TYPE trading_engine_error_paused_strategies gauge\ntrading_engine_error_paused_strategies {error_paused}\n# HELP trading_engine_reconcile_runs_total Reconcile loop executions.\n# TYPE trading_engine_reconcile_runs_total counter\ntrading_engine_reconcile_runs_total {runs}\n# HELP trading_engine_reconcile_failures_total Reconcile loop failures.\n# TYPE trading_engine_reconcile_failures_total counter\ntrading_engine_reconcile_failures_total {failures}\n",
        active = guard.active_strategies,
        error_paused = guard.error_paused_strategies,
        runs = guard.reconcile_runs_total,
        failures = guard.reconcile_failures_total,
    )
}

#[cfg(test)]
mod tests {
    use super::{apply_live_sync_failure, health_payload, reconcile_once, RuntimeMetrics, DEFAULT_PORT, SERVICE_NAME};
    use shared_db::{SharedDb, StoredStrategy};
    use shared_domain::strategy::{
        GridGeneration, GridLevel, PostTriggerAction, Strategy, StrategyAmountMode, StrategyMarket,
        StrategyMode,
        StrategyRevision, StrategyRuntime, StrategyStatus,
    };
    use chrono::Utc;
    use rust_decimal::Decimal;
    use std::sync::{Arc, Mutex};

    #[test]
    fn engine_iteration_calls_stream_sync_on_successful_reconcile() {
        let reconcile_calls = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let sync_calls = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let reconcile_calls_clone = reconcile_calls.clone();
        let sync_calls_clone = sync_calls.clone();

        let result = super::run_engine_iteration(
            move || {
                *reconcile_calls_clone.lock().unwrap() += 1;
                Ok::<(), &'static str>(())
            },
            move || {
                *sync_calls_clone.lock().unwrap() += 1;
                Ok::<(), &'static str>(())
            },
        );

        assert!(result.is_ok());
        assert_eq!(*reconcile_calls.lock().unwrap(), 1);
        assert_eq!(*sync_calls.lock().unwrap(), 1);
    }

    #[test]
    fn engine_iteration_still_calls_stream_sync_when_reconcile_fails() {
        let sync_calls = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let sync_calls_clone = sync_calls.clone();

        let result = super::run_engine_iteration(
            || Err::<(), &'static str>("reconcile failed"),
            move || {
                *sync_calls_clone.lock().unwrap() += 1;
                Ok::<(), &'static str>(())
            },
        );

        assert_eq!(result, Err("reconcile failed"));
        assert_eq!(*sync_calls.lock().unwrap(), 1);
    }

    #[test]
    fn health_payload_mentions_service_name() {
        let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
        let payload = health_payload(SERVICE_NAME, &metrics);
        assert!(payload.contains("service_up"));
        assert!(payload.contains("trading_engine_active_strategies"));
    }

    #[test]
    fn parse_port_falls_back_when_value_is_missing_or_invalid() {
        assert_eq!(super::parse_port(None, DEFAULT_PORT), DEFAULT_PORT);
        assert_eq!(super::parse_port(Some("not-a-port".to_string()), DEFAULT_PORT), DEFAULT_PORT);
    }

    #[test]
    fn user_stream_keys_empty_when_live_mode_disabled() {
        let keys = super::desired_user_stream_keys(false, &[("alice@example.com", "spot,usdm")]);
        assert!(keys.is_empty());
    }

    #[test]
    fn user_stream_keys_expand_active_accounts_by_market_scope() {
        let keys = super::desired_user_stream_keys(true, &[
            ("alice@example.com", "spot,usdm"),
            ("bob@example.com", "coinm"),
        ]);
        assert_eq!(keys, vec![
            "alice@example.com:spot".to_string(),
            "alice@example.com:usdm".to_string(),
            "bob@example.com:coinm".to_string(),
        ]);
    }

    #[test]
    fn required_env_requires_runtime_storage_urls() {
        std::env::remove_var("DATABASE_URL");
        assert!(super::required_env("DATABASE_URL").is_err());
        std::env::set_var("DATABASE_URL", "postgres://grid:secret@localhost/grid");
        assert!(super::required_env("DATABASE_URL").is_ok());
        std::env::remove_var("DATABASE_URL");
    }

    #[test]
    fn execution_update_effects_persist_trade_history_immediately() {
        let db = SharedDb::ephemeral().expect("db");
        let mut strategy = strategy("run", StrategyStatus::Running);
        strategy.owner_email = "engine@example.com".to_string();
        strategy.runtime.orders.push(shared_domain::strategy::StrategyRuntimeOrder {
            order_id: "run-order-0".to_string(),
            exchange_order_id: Some("555".to_string()),
            level_index: Some(0),
            side: "Buy".to_string(),
            order_type: "Limit".to_string(),
            price: Some(Decimal::new(100, 0)),
            quantity: Decimal::new(1, 0),
            status: "Placed".to_string(),
        });

        let changed = super::apply_execution_update_effects(
            &db,
            &mut strategy,
            &shared_binance::BinanceExecutionUpdate {
                market: "spot".to_string(),
                symbol: "BTCUSDT".to_string(),
                order_id: "555".to_string(),
                client_order_id: Some("run-order-0".to_string()),
                side: Some("BUY".to_string()),
                order_type: Some("LIMIT".to_string()),
                status: "FILLED".to_string(),
                execution_type: Some("TRADE".to_string()),
                order_price: Some("100".to_string()),
                last_fill_price: Some("100".to_string()),
                last_fill_quantity: Some("1".to_string()),
                cumulative_fill_quantity: Some("1".to_string()),
                fee_amount: Some("0.1".to_string()),
                fee_asset: Some("USDT".to_string()),
                position_side: None,
                trade_id: Some("123".to_string()),
                realized_profit: None,
                event_time_ms: 1_710_000,
            },
        ).expect("apply effects");

        assert!(changed);
        let trades = db.list_exchange_trade_history("engine@example.com").unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].trade_id, "123");
    }

    #[test]
    fn reconcile_updates_metrics_for_running_and_error_paused_strategies() {
        let db = SharedDb::ephemeral().expect("db");
        db.insert_strategy(&StoredStrategy { sequence_id: 1, strategy: strategy("run", StrategyStatus::Running) }).unwrap();
        db.insert_strategy(&StoredStrategy { sequence_id: 2, strategy: strategy("err", StrategyStatus::ErrorPaused) }).unwrap();
        let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
        reconcile_once(&db, &metrics).unwrap();
        let guard = metrics.lock().unwrap();
        assert_eq!(guard.active_strategies, 1);
        assert_eq!(guard.error_paused_strategies, 1);
        assert_eq!(guard.reconcile_runs_total, 1);
    }

    #[test]
    fn runtime_notifications_record_failed_telegram_log_when_binding_exists_without_bot_token() {
        let db = SharedDb::ephemeral().expect("db");
        db.upsert_telegram_binding(&shared_db::TelegramBindingRecord {
            user_email: "engine@example.com".to_string(),
            telegram_user_id: "tg-1".to_string(),
            telegram_chat_id: "chat-1".to_string(),
            bound_at: chrono::Utc::now(),
        }).unwrap();
        std::env::remove_var("TELEGRAM_BOT_TOKEN");
        let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
        let mut running = strategy("notify-tg-fail", StrategyStatus::Running);
        running.runtime.positions = vec![shared_domain::strategy::StrategyRuntimePosition {
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotClassic,
            quantity: Decimal::new(1, 0),
            average_entry_price: Decimal::new(100, 0),
        }];
        running.active_revision = Some(shared_domain::strategy::StrategyRevision {
            revision_id: "notify-revision".to_string(),
            version: 1,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            levels: vec![GridLevel {
                level_index: 0,
                entry_price: Decimal::new(100, 0),
                quantity: Decimal::new(1, 0),
                take_profit_bps: 100,
                trailing_bps: None,
            }],
            overall_take_profit_bps: Some(100),
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        });
        db.insert_strategy(&StoredStrategy { sequence_id: 1, strategy: running }).expect("strategy");
        db.enqueue_market_tick(&shared_events::MarketTick {
            symbol: "BTCUSDT".to_string(),
            market: "spot".to_string(),
            price: Decimal::new(101, 0),
            event_time_ms: 1_000,
        }).expect("tick");

        reconcile_once(&db, &metrics).expect("reconcile");

        let notifications = db.list_notification_logs("engine@example.com", 10).expect("notifications");
        assert!(notifications.iter().any(|record| record.channel == "telegram" && record.template_key.as_deref() == Some("OverallTakeProfitTriggered") && record.status == "failed"));
    }

    #[test]
    fn reconcile_persists_overall_take_profit_notification() {
        let db = SharedDb::ephemeral().expect("db");
        let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
        let mut running = strategy("notify-tp", StrategyStatus::Running);
        running.runtime.positions = vec![shared_domain::strategy::StrategyRuntimePosition {
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotClassic,
            quantity: Decimal::new(1, 0),
            average_entry_price: Decimal::new(100, 0),
        }];
        running.active_revision = Some(shared_domain::strategy::StrategyRevision {
            revision_id: "notify-revision".to_string(),
            version: 1,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            levels: vec![GridLevel {
                level_index: 0,
                entry_price: Decimal::new(100, 0),
                quantity: Decimal::new(1, 0),
                take_profit_bps: 100,
                trailing_bps: None,
            }],
            overall_take_profit_bps: Some(100),
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        });
        db.insert_strategy(&StoredStrategy { sequence_id: 1, strategy: running }).expect("strategy");
        db.enqueue_market_tick(&shared_events::MarketTick {
            symbol: "BTCUSDT".to_string(),
            market: "spot".to_string(),
            price: Decimal::new(101, 0),
            event_time_ms: 1_000,
        }).expect("tick");

        reconcile_once(&db, &metrics).expect("reconcile");

        let notifications = db.list_notification_logs("engine@example.com", 10).expect("notifications");
        assert!(notifications.iter().any(|record| record.template_key.as_deref() == Some("OverallTakeProfitTriggered")));
    }

    #[test]
    fn reconcile_failure_auto_pauses_and_emits_runtime_error_notification() {
        let db = SharedDb::ephemeral().expect("db");
        let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
        let mut broken = strategy("broken", StrategyStatus::Running);
        broken.active_revision = Some(shared_domain::strategy::StrategyRevision {
            revision_id: "broken-revision".to_string(),
            version: 1,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            levels: vec![GridLevel {
                level_index: 0,
                entry_price: Decimal::new(100, 0),
                quantity: Decimal::new(1, 0),
                take_profit_bps: 100,
                trailing_bps: Some(200),
            }],
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        });
        db.insert_strategy(&StoredStrategy { sequence_id: 1, strategy: broken }).expect("strategy");

        reconcile_once(&db, &metrics).expect("reconcile");

        let stored = db.find_strategy("engine@example.com", "broken").expect("find").expect("strategy");
        assert_eq!(stored.status, StrategyStatus::ErrorPaused);
        let notifications = db.list_notification_logs("engine@example.com", 10).expect("notifications");
        assert!(notifications.iter().any(|record| record.template_key.as_deref() == Some("RuntimeError")));
    }

    #[test]
    fn live_order_sync_failure_auto_pauses_and_emits_runtime_error_notification() {
        let db = SharedDb::ephemeral().expect("db");
        let mut running = strategy("live-sync-fail", StrategyStatus::Running);
        let created_at = Utc::now();

        let changed = apply_live_sync_failure(&db, &mut running, 2, created_at).expect("live sync failure");

        assert!(changed);
        assert_eq!(running.status, StrategyStatus::ErrorPaused);
        assert!(running.runtime.events.iter().any(|event| event.event_type == "live_order_sync_failed"));
        let notifications = db.list_notification_logs("engine@example.com", 10).expect("notifications");
        assert!(notifications.iter().any(|record| record.template_key.as_deref() == Some("RuntimeError")));
    }

    #[test]
    fn reconcile_consumes_market_ticks_for_running_strategy() {
        let db = SharedDb::ephemeral().expect("db");
        let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
        let mut running = strategy("tick", StrategyStatus::Running);
        running.runtime.positions = vec![shared_domain::strategy::StrategyRuntimePosition {
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotClassic,
            quantity: Decimal::new(1, 0),
            average_entry_price: Decimal::new(100, 0),
        }];
        running.active_revision = Some(shared_domain::strategy::StrategyRevision {
            revision_id: "tick-revision".to_string(),
            version: 1,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            levels: vec![GridLevel {
                level_index: 0,
                entry_price: Decimal::new(100, 0),
                quantity: Decimal::new(1, 0),
                take_profit_bps: 100,
                trailing_bps: None,
            }],
            overall_take_profit_bps: Some(100),
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        });
        db.insert_strategy(&StoredStrategy { sequence_id: 1, strategy: running }).expect("strategy");
        db.enqueue_market_tick(&shared_events::MarketTick {
            symbol: "BTCUSDT".to_string(),
            market: "spot".to_string(),
            price: Decimal::new(101, 0),
            event_time_ms: 1_000,
        }).expect("tick");

        reconcile_once(&db, &metrics).expect("reconcile");

        let stored = db.find_strategy("engine@example.com", "tick").expect("find").expect("strategy");
        assert!(stored.runtime.events.iter().any(|event| event.event_type.contains("overall_take_profit")));
    }

    fn strategy(id: &str, status: StrategyStatus) -> Strategy {
        Strategy {
            id: id.to_string(),
            owner_email: "engine@example.com".to_string(),
            name: id.to_string(),
            symbol: "BTCUSDT".to_string(),
            budget: "1000".to_string(),
            grid_spacing_bps: 100,
            status,
            source_template_id: None,
            membership_ready: true,
            exchange_ready: true,
            permissions_ready: true,
            withdrawals_disabled: true,
            hedge_mode_ready: true,
            symbol_ready: true,
            filters_ready: true,
            margin_ready: true,
            conflict_ready: true,
            balance_ready: true,
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotClassic,
            draft_revision: revision(),
            active_revision: Some(revision()),
            runtime: StrategyRuntime::default(),
            archived_at: None,
        }
    }

    fn revision() -> StrategyRevision {
        StrategyRevision {
            revision_id: "rev-1".to_string(),
            version: 1,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            levels: vec![GridLevel { level_index: 0, entry_price: Decimal::new(100,0), quantity: Decimal::new(1,0), take_profit_bps: 100, trailing_bps: None }],
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        }
    }
}
