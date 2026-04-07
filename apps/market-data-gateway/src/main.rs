use axum::{extract::State, http::header, response::IntoResponse, routing::get, Router};
use market_data_gateway::{
    binance_ws::{run_market_stream, GatewayRuntime},
    subscriptions::{active_symbol_subscriptions, market_stream_plans, SymbolActivity},
};
use shared_db::SharedDb;
use shared_domain::strategy::{StrategyMarket, StrategyStatus};
use std::{
    io::{Error as IoError, ErrorKind},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::TcpListener, task::JoinHandle, time::sleep};

const DEFAULT_PORT: u16 = 8083;
const SERVICE_NAME: &str = "market-data-gateway";
const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 5;

#[derive(Debug, Clone, Default)]
struct GatewayMetrics {
    active_symbol_count: usize,
    refresh_failures_total: u64,
    refresh_runs_total: u64,
    reconnect_count: u32,
}

#[derive(Clone)]
struct GatewayState {
    metrics: Arc<Mutex<GatewayMetrics>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = required_env("DATABASE_URL")?;
    let redis_url = required_env("REDIS_URL")?;
    let db = SharedDb::connect(&database_url, &redis_url)?;
    let metrics = Arc::new(Mutex::new(GatewayMetrics::default()));
    let metrics_for_loop = metrics.clone();
    let runtime = Arc::new(Mutex::new(GatewayRuntime::new(&[])));
    let runtime_for_loop = runtime.clone();
    let live_mode = live_mode_enabled();

    tokio::spawn(async move {
        let mut live_handles: Vec<JoinHandle<()>> = Vec::new();
        let mut live_signature: Vec<String> = Vec::new();
        loop {
            match refresh_subscriptions(&db, &runtime_for_loop, &metrics_for_loop) {
                Ok(activities) if live_mode => {
                    let plans = market_stream_plans(&activities);
                    let signature = plans
                        .iter()
                        .map(|plan| format!("{}:{}", plan.market, plan.url))
                        .collect::<Vec<_>>();
                    let restart_required =
                        should_restart_streams(&live_handles, &live_signature, &signature);
                    let signature_changed = signature != live_signature;
                    if restart_required {
                        if !signature_changed {
                            let reconnects = {
                                let mut guard =
                                    runtime_for_loop.lock().expect("gateway runtime poisoned");
                                guard.reconnect(&activities);
                                guard.reconnect_count()
                            };
                            metrics_for_loop
                                .lock()
                                .expect("metrics poisoned")
                                .reconnect_count = reconnects;
                        }
                        for handle in live_handles.drain(..) {
                            handle.abort();
                        }
                        for plan in plans {
                            let runtime = runtime_for_loop.clone();
                            let db = db.clone();
                            live_handles.push(tokio::spawn(async move {
                                if let Err(error) = run_market_stream(&plan, runtime, db).await {
                                    eprintln!(
                                        "market-data-gateway live stream {} failed: {error}",
                                        plan.market
                                    );
                                }
                            }));
                        }
                        live_signature = signature;
                    }
                }
                Ok(_) => {}
                Err(_) => {
                    let mut guard = metrics_for_loop.lock().expect("metrics poisoned");
                    guard.refresh_failures_total += 1;
                }
            }
            sleep(Duration::from_secs(configured_refresh_interval_secs())).await;
        }
    });

    let listener = TcpListener::bind(("0.0.0.0", configured_port(DEFAULT_PORT))).await?;
    let app = Router::new()
        .route("/healthz", get(healthz))
        .with_state(GatewayState { metrics });

    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz(State(state): State<GatewayState>) -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        health_payload(SERVICE_NAME, &state.metrics),
    )
}

fn refresh_subscriptions(
    db: &SharedDb,
    runtime: &Arc<Mutex<GatewayRuntime>>,
    metrics: &Arc<Mutex<GatewayMetrics>>,
) -> Result<Vec<SymbolActivity>, shared_db::SharedDbError> {
    let strategies = db.list_all_strategies()?;
    let activities = strategies
        .into_iter()
        .map(|strategy| {
            SymbolActivity::new_with_market(
                strategy.symbol,
                market_code(strategy.market),
                strategy.status == StrategyStatus::Running,
            )
        })
        .collect::<Vec<_>>();
    let desired = active_symbol_subscriptions(&activities);
    let active_count = desired.len();

    {
        let mut guard = runtime.lock().expect("gateway runtime poisoned");
        if guard.subscriptions() != desired.as_slice() {
            if guard.subscriptions().is_empty() && active_count > 0 {
                *guard = GatewayRuntime::new(&activities);
            } else {
                guard.reconnect(&activities);
            }
        }

        let mut metrics_guard = metrics.lock().expect("metrics poisoned");
        metrics_guard.active_symbol_count = active_count;
        metrics_guard.reconnect_count = guard.reconnect_count();
        metrics_guard.refresh_runs_total += 1;
    }

    Ok(activities)
}

fn should_restart_streams(
    handles: &[JoinHandle<()>],
    current_signature: &[String],
    next_signature: &[String],
) -> bool {
    current_signature != next_signature || handles.iter().any(|handle| handle.is_finished())
}

fn market_code(market: StrategyMarket) -> &'static str {
    match market {
        StrategyMarket::Spot => "spot",
        StrategyMarket::FuturesUsdM => "usdm",
        StrategyMarket::FuturesCoinM => "coinm",
    }
}

fn live_mode_enabled() -> bool {
    std::env::var("BINANCE_LIVE_MODE")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn configured_refresh_interval_secs() -> u64 {
    std::env::var("MARKET_DATA_REFRESH_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_REFRESH_INTERVAL_SECS)
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
    value
        .and_then(|port| port.parse().ok())
        .unwrap_or(default_port)
}

fn health_payload(service_name: &str, metrics: &Arc<Mutex<GatewayMetrics>>) -> String {
    let guard = metrics.lock().expect("metrics poisoned");
    format!(
        "# HELP service_up Service health probe status.\n# TYPE service_up gauge\nservice_up{{service=\"{service_name}\"}} 1\n# HELP market_data_gateway_active_symbols Active symbols tracked by the subscription loop.\n# TYPE market_data_gateway_active_symbols gauge\nmarket_data_gateway_active_symbols {active}\n# HELP market_data_gateway_refresh_runs_total Subscription refresh loop executions.\n# TYPE market_data_gateway_refresh_runs_total counter\nmarket_data_gateway_refresh_runs_total {runs}\n# HELP market_data_gateway_refresh_failures_total Subscription refresh loop failures.\n# TYPE market_data_gateway_refresh_failures_total counter\nmarket_data_gateway_refresh_failures_total {failures}\n# HELP market_data_gateway_reconnect_count Reconnect count observed by runtime.\n# TYPE market_data_gateway_reconnect_count gauge\nmarket_data_gateway_reconnect_count {reconnects}\n",
        active = guard.active_symbol_count,
        runs = guard.refresh_runs_total,
        failures = guard.refresh_failures_total,
        reconnects = guard.reconnect_count,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        health_payload, parse_port, required_env, should_restart_streams, GatewayMetrics,
        DEFAULT_PORT, SERVICE_NAME,
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn health_payload_mentions_service_name() {
        let metrics = Arc::new(Mutex::new(GatewayMetrics::default()));
        let payload = health_payload(SERVICE_NAME, &metrics);
        assert!(payload.contains("service_up"));
        assert!(payload.contains("market_data_gateway_active_symbols"));
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
        std::env::remove_var("DATABASE_URL");
        assert!(required_env("DATABASE_URL").is_err());
        std::env::set_var("DATABASE_URL", "postgres://grid:secret@localhost/grid");
        assert!(required_env("DATABASE_URL").is_ok());
        std::env::remove_var("DATABASE_URL");
    }

    #[tokio::test]
    async fn finished_live_stream_handle_triggers_restart_even_when_signature_is_unchanged() {
        let handle = tokio::spawn(async {});
        tokio::task::yield_now().await;
        assert!(handle.is_finished());

        let current = vec!["spot:wss://stream.binance.com/ws/btcusdt@trade".to_string()];
        let next = vec!["spot:wss://stream.binance.com/ws/btcusdt@trade".to_string()];
        let handles = vec![handle];

        assert!(should_restart_streams(&handles, &current, &next));
    }
}
