use axum::{extract::State, http::header, response::IntoResponse, routing::get, Router};
use backtest_engine::market_data::KlineBar;
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use rust_decimal::prelude::ToPrimitive;
use shared_binance::{
    parse_account_update_message, parse_user_data_message, BinanceAccountUpdate, BinanceClient,
    BinanceUserDataStream, CredentialCipher, SymbolMetadata,
};
use shared_db::{MartingalePortfolioRecord, NotificationLogRecord, SharedDb, StoredStrategy};
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleMarginMode, MartingaleMarketKind,
    MartingalePortfolioConfig, MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel,
    MartingaleStopLossModel, MartingaleStrategyConfig, MartingaleTakeProfitModel,
};
use shared_domain::strategy::Decimal;
use shared_domain::strategy::{
    FuturesMarginMode, GridGeneration, GridLevel, PostTriggerAction, ReferencePriceSource,
    RuntimeControls, Strategy, StrategyAmountMode, StrategyRuntime, StrategyRuntimeEvent,
    StrategyRuntimeOrder, StrategyRuntimePhase, StrategyRuntimePosition, StrategyStatus,
    StrategyType,
};
use shared_events::{MarketTick, NotificationKind};
use std::sync::{LazyLock, OnceLock};
use std::{
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
    io::{Error as IoError, ErrorKind},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::TcpListener, task::JoinHandle, time::sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use trading_engine::martingale_budget::apply_global_budget_allocations;
use trading_engine::{
    execution_effects::persist_execution_effects,
    execution_sync::{apply_execution_update, recompute_strategy_positions},
    martingale_candle::{complete_bars, LiveCandleBucket, MINUTE_MS},
    martingale_exit::martingale_strategy_drawdown_pct,
    martingale_recovery::{recover_martingale_runtime, MartingaleRecoveryInput, RecoveryPosition},
    martingale_runtime::{
        FuturesExchangeSettings, FuturesSymbolSettings, MartingaleRuntime, MartingaleRuntimeConfig,
        MartingaleRuntimeContext, MartingaleRuntimeOrder,
    },
    order_sync::{sync_strategy_orders, OrderQuantizationRules},
    strategy_runtime::StrategyRuntimeEngine,
    trade_sync::sync_strategy_trades,
};

const DEFAULT_PORT: u16 = 8081;
const SERVICE_NAME: &str = "trading-engine";
const DEFAULT_RECONCILE_INTERVAL_SECS: u64 = 5;
const BINANCE_EXCHANGE: &str = "binance";
const MARTINGALE_CONFIG_NOTE_PREFIX: &str = "martingale_strategy_config_json=";

static LIVE_TICK_QUEUE: OnceLock<Arc<Mutex<Vec<MarketTick>>>> = OnceLock::new();
static TICK_SUBSCRIBER_STARTED: OnceLock<()> = OnceLock::new();
static LATEST_MARKET_TICKS: OnceLock<Mutex<HashMap<String, MarketTick>>> = OnceLock::new();

/// 进程级 indicator feeds 持久化：跨 reconcile tick 保留 IndicatorRuntimeContext。
/// key = portfolio_id, value = 该 portfolio 的指标状态（ATR/ADX 增量缓存）。
/// 每次 reconcile：从 feeds 取出 → set_indicator_context → reconcile → indicator_context_clone → 存回。
static INDICATOR_FEEDS: OnceLock<
    Mutex<HashMap<String, backtest_engine::martingale::indicator_runtime::IndicatorRuntimeContext>>,
> = OnceLock::new();
static MARTINGALE_CANDLE_FEEDS: OnceLock<
    Mutex<HashMap<String, HashMap<String, LiveCandleBucket>>>,
> = OnceLock::new();

fn indicator_feeds() -> &'static Mutex<
    HashMap<String, backtest_engine::martingale::indicator_runtime::IndicatorRuntimeContext>,
> {
    INDICATOR_FEEDS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn martingale_candle_feeds() -> &'static Mutex<HashMap<String, HashMap<String, LiveCandleBucket>>> {
    MARTINGALE_CANDLE_FEEDS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 进程级 portfolio equity peak 跟踪：跨 reconcile tick 保留每个 portfolio 的
/// historical peak equity (quote)。用于计算组合 drawdown 百分比
/// (parity port of backtest guard `kline_engine.rs:142-146`).
/// key = portfolio_id, value = 历史最高 equity。
static MARTINGALE_PORTFOLIO_EQUITY_PEAKS: OnceLock<Mutex<HashMap<String, Decimal>>> = OnceLock::new();

fn martingale_portfolio_equity_peaks() -> &'static Mutex<HashMap<String, Decimal>> {
    MARTINGALE_PORTFOLIO_EQUITY_PEAKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 计算一个 running portfolio 的当前 drawdown 百分比 (peak→current equity).
///
/// equity = budget + Σ realized_pnl + Σ unrealized_pnl
/// - budget: portfolio 分配预算 (config.portfolio_budget_quote).
/// - realized: Σ over the portfolio's strategies of Σ fill.realized_pnl.
/// - unrealized: Σ over open positions of (latest_price - avg_entry) * qty * dir_sign,
///   latest_price 取自本 cycle 的 market_ticks (fallback average_entry_price).
///
/// 维护进程级 peak; drawdown_pct = (peak - current) / peak * 100 (peak > 0).
/// 返回 None 仅当 budget <= 0 或无策略可计算 (理论上不应发生在 running portfolio).
fn portfolio_drawdown_pct_for(
    db: &SharedDb,
    portfolio: &MartingalePortfolioRecord,
    config: &MartingaleRuntimeConfig,
    market_ticks: &[MarketTick],
) -> Option<f64> {
    let budget = config.portfolio_budget_quote;
    if budget <= Decimal::ZERO {
        return None;
    }

    let owner = &portfolio.owner;
    let mut realized = Decimal::ZERO;
    let mut unrealized = Decimal::ZERO;

    for strategy_config in &config.portfolio.strategies {
        let strategy_id = &strategy_config.strategy_id;
        let strategy = match db.find_strategy(owner, strategy_id) {
            Ok(Some(strategy)) => strategy,
            _ => continue,
        };
        // realized = Σ fill.realized_pnl (all-time; runtime.fills never cleared)
        for fill in &strategy.runtime.fills {
            if let Some(pnl) = fill.realized_pnl {
                realized += pnl;
            }
        }
        // unrealized over open positions
        let dir_sign = match strategy_config.direction {
            MartingaleDirection::Long => Decimal::ONE,
            MartingaleDirection::Short => Decimal::NEGATIVE_ONE,
        };
        for position in &strategy.runtime.positions {
            if position.quantity == Decimal::ZERO {
                continue;
            }
            // latest tick this cycle for the strategy symbol, else fall back to avg entry
            let latest_price = market_ticks
                .iter()
                .find(|tick| tick.symbol == strategy_config.symbol)
                .map(|tick| tick.price)
                .unwrap_or(position.average_entry_price);
            // Parity with backtest unrealized_pnl (kline_engine.rs:1464-1476):
            // subtract entry+exit costs so live portfolio drawdown matches the
            // backtest (fires slightly earlier / tighter).
            let gross_pnl =
                (latest_price - position.average_entry_price) * position.quantity * dir_sign;
            let costs = (position.quantity * position.average_entry_price
                + position.quantity * latest_price)
                * Decimal::from_f64_retain(
                    backtest_engine::martingale::kline_engine::DEFAULT_FEE_BPS
                        + backtest_engine::martingale::kline_engine::DEFAULT_SLIPPAGE_BPS,
                )
                .unwrap_or(Decimal::ZERO)
                / Decimal::from(10_000);
            unrealized += gross_pnl - costs;
        }
    }

    let current_equity = budget + realized + unrealized;

    let mut peaks = martingale_portfolio_equity_peaks()
        .lock()
        .expect("portfolio equity peaks poisoned");
    let peak_entry = peaks
        .entry(portfolio.portfolio_id.clone())
        .or_insert_with(|| current_equity);
    if current_equity > *peak_entry {
        *peak_entry = current_equity;
    }
    let peak = *peak_entry;
    drop(peaks);

    if peak > Decimal::ZERO {
        let drawdown = (peak - current_equity) / peak;
        // drawdown as a fraction (peak→current); *100 for percent.
        drawdown.to_f64().map(|v| v * 100.0)
    } else {
        None
    }
}

fn save_indicator_context(
    portfolio_id: &str,
    ctx: &backtest_engine::martingale::indicator_runtime::IndicatorRuntimeContext,
) {
    let mut feeds = indicator_feeds().lock().expect("indicator feeds poisoned");
    feeds.insert(portfolio_id.to_owned(), ctx.clone());
}

fn completed_martingale_indicator_bars(
    portfolio_id: &str,
    market_ticks: &[MarketTick],
) -> Vec<KlineBar> {
    let mut feeds = martingale_candle_feeds()
        .lock()
        .expect("martingale candle feeds poisoned");
    let by_symbol = feeds.entry(portfolio_id.to_owned()).or_default();
    complete_bars(by_symbol, market_ticks, MINUTE_MS)
}

/// Per-strategy locks to prevent concurrent reads-modify-writes between reconcile
/// and user-data-stream callbacks. Without this, updates from one path may silently
/// overwrite updates from the other.
static STRATEGY_LOCKS: LazyLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn strategy_lock(strategy_id: &str) -> Arc<Mutex<()>> {
    let mut locks = STRATEGY_LOCKS.lock().expect("strategy locks poisoned");
    locks
        .entry(strategy_id.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

fn live_tick_queue() -> &'static Arc<Mutex<Vec<MarketTick>>> {
    LIVE_TICK_QUEUE.get_or_init(|| Arc::new(Mutex::new(Vec::new())))
}

fn latest_market_ticks() -> &'static Mutex<HashMap<String, MarketTick>> {
    LATEST_MARKET_TICKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn start_market_tick_subscriber(db: &SharedDb) {
    TICK_SUBSCRIBER_STARTED.get_or_init(|| {
        let redis = db.redis().clone();
        let queue = live_tick_queue().clone();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            if let Err(error) = redis.run_market_tick_subscriber(tx).await {
                eprintln!("trading-engine market tick subscriber failed: {error}");
            }
        });
        tokio::spawn(async move {
            while let Some(tick) = rx.recv().await {
                if let Ok(mut guard) = queue.lock() {
                    guard.push(tick);
                }
            }
        });
    });
}

fn drain_live_ticks(limit: usize) -> Vec<MarketTick> {
    let mut guard = live_tick_queue().lock().expect("live tick queue poisoned");
    let count = limit.min(guard.len());
    guard.drain(0..count).collect()
}

fn remember_latest_market_ticks(ticks: &[MarketTick]) {
    if ticks.is_empty() {
        return;
    }
    let mut latest = latest_market_ticks()
        .lock()
        .expect("latest market ticks poisoned");
    for tick in ticks {
        if tick.price <= Decimal::ZERO {
            continue;
        }
        let key = market_tick_key(&tick.symbol, &tick.market);
        let replace = latest
            .get(&key)
            .map(|existing| tick.event_time_ms >= existing.event_time_ms)
            .unwrap_or(true);
        if replace {
            latest.insert(key, tick.clone());
        }
    }
}

fn latest_market_tick(symbol: &str, market: &str) -> Option<MarketTick> {
    latest_market_ticks()
        .lock()
        .expect("latest market ticks poisoned")
        .get(&market_tick_key(symbol, market))
        .cloned()
}

fn market_tick_key(symbol: &str, market: &str) -> String {
    format!(
        "{}:{}",
        symbol.trim().to_ascii_uppercase(),
        market.trim().to_ascii_lowercase()
    )
}

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
    if live_mode_enabled() {
        start_market_tick_subscriber(&db);
    }
    let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
    let db_for_loop = db.clone();
    let metrics_for_loop = metrics.clone();
    tokio::spawn(async move {
        loop {
            let db = db_for_loop.clone();
            let metrics = metrics_for_loop.clone();
            let result = tokio::task::spawn_blocking(move || {
                run_engine_iteration(|| reconcile_once(&db, &metrics), || sync_user_streams(&db))
            })
            .await;
            if let Err(join_error) = result {
                eprintln!("trading-engine reconcile panic: {join_error}");
                let mut guard = metrics_for_loop.lock().expect("metrics poisoned");
                guard.reconcile_failures_total += 1;
                guard.last_reconcile_at = Some(Utc::now().timestamp());
            } else if result.unwrap().is_err() {
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

fn reconcile_once(
    db: &SharedDb,
    metrics: &Arc<Mutex<RuntimeMetrics>>,
) -> Result<(), shared_db::SharedDbError> {
    let mut active = 0usize;
    let mut error_paused = 0usize;
    let live_mode = live_mode_enabled();
    let cipher = if live_mode {
        Some(credential_cipher()?)
    } else {
        None
    };
    let market_ticks = if live_mode {
        let mut ticks = drain_live_ticks(256);
        ticks.extend(db.drain_market_ticks(256)?);
        remember_latest_market_ticks(&ticks);
        ticks
    } else {
        db.drain_market_ticks(256)?
    };
    reconcile_running_martingale_portfolios(db, cipher.as_ref(), &market_ticks)?;
    for mut strategy in db.list_all_strategies()? {
        let strategy_lock_arc = strategy_lock(&strategy.id);
        let _strategy_guard = strategy_lock_arc.lock().expect("strategy lock poisoned");
        let mut dirty = false;
        if let Some(cipher) = cipher.as_ref() {
            dirty |= sync_live_orders(db, &mut strategy, cipher)?;
        }
        dirty |= apply_market_ticks(db, &mut strategy, &market_ticks)?;
        match strategy.status {
            StrategyStatus::Running => {
                let revision = strategy
                    .active_revision
                    .clone()
                    .unwrap_or_else(|| strategy.draft_revision.clone());
                if let Err(error) = StrategyRuntimeEngine::new(
                    &strategy.id,
                    strategy.market,
                    strategy.mode,
                    revision,
                ) {
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

fn reconcile_running_martingale_portfolios(
    db: &SharedDb,
    _cipher: Option<&CredentialCipher>,
    market_ticks: &[MarketTick],
) -> Result<(), shared_db::SharedDbError> {
    for portfolio in db.backtest_repo().list_running_martingale_portfolios()? {
        let mut persisted_ctx = {
            let feeds = indicator_feeds().lock().expect("indicator feeds poisoned");
            feeds
                .get(&portfolio.portfolio_id)
                .cloned()
                .unwrap_or_default()
        };
        let completed_bars =
            completed_martingale_indicator_bars(&portfolio.portfolio_id, market_ticks);
        if !completed_bars.is_empty() {
            for bar in &completed_bars {
                persisted_ctx.push_bar(bar);
            }
            save_indicator_context(&portfolio.portfolio_id, &persisted_ctx);
        }

        if portfolio
            .risk_summary
            .get("live_executor_started")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            reconcile_martingale_executor_strategies(db, &portfolio, &persisted_ctx, market_ticks)?;
            continue;
        }
        // Verify exchange preconfigure readiness; do NOT mutate exchange
        // settings from the executor loop.
        let preconfigure_status = portfolio
            .risk_summary
            .get("exchange_preconfigure")
            .and_then(|v| v.get("status"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if preconfigure_status != "ready" {
            eprintln!(
                "trading-engine portfolio {} exchange_preconfigure status is '{}'; blocking",
                portfolio.portfolio_id, preconfigure_status
            );
            continue;
        }
        let _settings = futures_settings_from_portfolio(&portfolio)?;
        let config = martingale_runtime_config_from_portfolio(&portfolio)?;
        let portfolio_drawdown_pct =
            portfolio_drawdown_pct_for(db, &portfolio, &config, market_ticks);

        // Per-strategy start_cycle + order generation
        let strategies_config = portfolio
            .config
            .get("portfolio_config")
            .and_then(|config| config.get("strategies"))
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut total_orders: usize = 0;
        let mut cycle_results: Vec<serde_json::Value> = Vec::new();

        let mut computed_orders: Vec<(
            MartingaleStrategyConfig,
            Decimal,
            Vec<MartingaleRuntimeOrder>,
        )> = Vec::new();

        for strategy_config in &strategies_config {
            let strategy_id = strategy_config
                .get("strategy_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            if strategy_id.is_empty() {
                continue;
            }
            // Anchor price: prefer config anchor_price, fallback to reference_price
            let anchor_price = strategy_anchor_price(strategy_config, market_ticks)
                .filter(|v| *v > rust_decimal::Decimal::ZERO)
                .unwrap_or(rust_decimal::Decimal::ZERO);
            if anchor_price <= rust_decimal::Decimal::ZERO {
                cycle_results.push(serde_json::json!({
                    "strategy_id": strategy_id,
                    "anchor_price": "0",
                    "order_count": 0,
                    "status": "blocked",
                    "error": "no valid positive anchor_price or reference_price in strategy config",
                }));
                continue;
            }

            let mut runtime = MartingaleRuntime::new(config.clone())
                .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;

            // 实盘 ATR 闭环：注入持久化的 indicator_context（跨 tick 保持 ATR/ADX 增量缓存）。
            runtime.set_indicator_context(persisted_ctx.clone());

            match runtime.start_cycle_with_futures_preflight(
                &_settings,
                strategy_id,
                anchor_price,
                martingale_runtime_context_for_strategy(
                    db,
                    &portfolio.owner,
                    strategy_id,
                    StrategyStatus::Running,
                    portfolio_drawdown_pct,
                )?,
            ) {
                Ok(()) => {
                    let orders: Vec<MartingaleRuntimeOrder> = runtime.orders().to_vec();
                    let count = orders.len();
                    total_orders += count;
                    if let Some(runtime_strategy_config) = config
                        .portfolio
                        .strategies
                        .iter()
                        .find(|strategy| strategy.strategy_id == strategy_id)
                        .cloned()
                    {
                        computed_orders.push((runtime_strategy_config, anchor_price, orders));
                    }
                    cycle_results.push(serde_json::json!({
                        "strategy_id": strategy_id,
                        "anchor_price": anchor_price.normalize().to_string(),
                        "order_count": count,
                        "status": "ok",
                    }));
                }
                Err(error) => {
                    cycle_results.push(serde_json::json!({
                        "strategy_id": strategy_id,
                        "anchor_price": anchor_price.normalize().to_string(),
                        "order_count": 0,
                        "status": "error",
                        "error": error.to_string(),
                    }));
                }
            }
            persisted_ctx = runtime.indicator_context_clone();
        }

        // 实盘 ATR 闭环：保存更新后的 indicator_context 回 INDICATOR_FEEDS
        save_indicator_context(&portfolio.portfolio_id, &persisted_ctx);

        // Persist computed orders to real MartingaleGrid executor strategies.
        // The portfolio config's strategy_id is the executor Strategy.id; creating
        // it here keeps live order submission, REST backfill, and user-stream fill
        // handling on the existing Strategy/order_sync path.
        let mut executor_strategy_count = 0usize;
        for (strategy_config, anchor_price, orders) in &computed_orders {
            if orders.is_empty() {
                continue;
            }
            let runtime_orders = martingale_runtime_orders_to_strategy_orders(orders);
            let strategy_id = &strategy_config.strategy_id;
            if db.find_strategy(&portfolio.owner, strategy_id)?.is_some() {
                let strategy = executor_strategy_from_martingale_config(
                    &portfolio.owner,
                    &portfolio,
                    strategy_config,
                    *anchor_price,
                    runtime_orders,
                )?;
                db.update_strategy(&strategy)?;
            } else {
                let sequence_id = db.next_sequence("strategy")?;
                let strategy = executor_strategy_from_martingale_config(
                    &portfolio.owner,
                    &portfolio,
                    strategy_config,
                    *anchor_price,
                    runtime_orders,
                )?;
                db.insert_strategy(&StoredStrategy {
                    sequence_id,
                    strategy,
                })?;
            }
            executor_strategy_count += 1;
        }

        let started = total_orders > 0;
        let summary = serde_json::json!({
            "live_executor_state": if started { "started" } else { "blocked" },
            "live_executor_started": started,
            "live_executor_ready": started,
            "strategy_count": strategies_config.len(),
            "order_count": total_orders,
            "executor_strategy_count": executor_strategy_count,
            "cycle_results": cycle_results,
        });
        db.backtest_repo()
            .upsert_martingale_live_snapshot(&portfolio, summary)?;
        // Also write back to risk_summary so the skip check works next iteration
        // and live-stats can fallback when Strategy runtime is empty.
        let mut risk_summary = portfolio.risk_summary.clone();
        risk_summary["live_executor_started"] = serde_json::json!(started);
        risk_summary["live_executor_state"] =
            serde_json::json!(if started { "started" } else { "blocked" });
        risk_summary["order_count"] = serde_json::json!(total_orders);
        risk_summary["executor_strategy_count"] = serde_json::json!(executor_strategy_count);
        risk_summary["strategy_count"] = serde_json::json!(strategies_config.len());
        risk_summary["cycle_results"] = serde_json::json!(cycle_results);
        db.backtest_repo()
            .update_martingale_portfolio_risk_summary(
                &portfolio.owner,
                &portfolio.portfolio_id,
                risk_summary,
            )?;
    }
    Ok(())
}

#[allow(dead_code)]

fn reconcile_martingale_executor_strategies(
    db: &SharedDb,
    portfolio: &MartingalePortfolioRecord,
    indicator_ctx: &backtest_engine::martingale::indicator_runtime::IndicatorRuntimeContext,
    market_ticks: &[MarketTick],
) -> Result<(), shared_db::SharedDbError> {
    let config = martingale_runtime_config_from_portfolio(portfolio)?;
    let settings = futures_settings_from_portfolio(portfolio)?;
    let portfolio_drawdown_pct =
        portfolio_drawdown_pct_for(db, portfolio, &config, market_ticks);
    for strategy_config in &config.portfolio.strategies {
        let Some(mut strategy) =
            db.find_strategy(&portfolio.owner, &strategy_config.strategy_id)?
        else {
            continue;
        };
        if strategy.strategy_type != StrategyType::MartingaleGrid
            || strategy.status != StrategyStatus::Running
        {
            continue;
        }
        let mut filled_legs = strategy
            .runtime
            .orders
            .iter()
            .filter(|order| {
                order.order_id.starts_with("mg-")
                    && order.order_id.contains("-leg-")
                    && order.status == "Filled"
            })
            .filter_map(|order| order.level_index)
            .collect::<Vec<_>>();
        if filled_legs.is_empty() {
            continue;
        }
        filled_legs.sort_unstable();
        filled_legs.dedup();

        let Some(anchor_price) = martingale_executor_anchor_price(&strategy) else {
            continue;
        };
        let mut runtime = MartingaleRuntime::new(config.clone())
            .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
        runtime.set_indicator_context(indicator_ctx.clone());
        runtime
            .start_cycle_with_futures_preflight(
                &settings,
                &strategy_config.strategy_id,
                anchor_price,
                martingale_runtime_context_for_strategy(
                    db,
                    &portfolio.owner,
                    &strategy_config.strategy_id,
                    strategy.status,
                    portfolio_drawdown_pct,
                )?,
            )
            .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;

        let mut blocked_error = None;
        for leg_index in filled_legs {
            if let Err(error) = runtime.mark_leg_filled(
                &strategy_config.strategy_id,
                strategy_config.direction,
                leg_index,
            ) {
                blocked_error = Some(error.to_string());
                break;
            }
        }

        let existing_order_ids = strategy
            .runtime
            .orders
            .iter()
            .map(|order| order.order_id.clone())
            .collect::<HashSet<_>>();
        let existing_leg_indexes = strategy
            .runtime
            .orders
            .iter()
            .filter(|order| order.order_id.starts_with("mg-") && order.order_id.contains("-leg-"))
            .filter(|order| order.status != "Canceled")
            .filter_map(|order| order.level_index)
            .collect::<HashSet<_>>();
        let mut added = 0usize;
        for order in runtime.orders() {
            if existing_order_ids.contains(&order.client_order_id) {
                continue;
            }
            if existing_leg_indexes.contains(&order.leg_index) {
                continue;
            }
            let next_orders = vec![order.clone()];
            strategy
                .runtime
                .orders
                .extend(martingale_runtime_orders_to_strategy_orders(&next_orders));
            added += 1;
        }
        if let Some(error) = blocked_error.as_ref() {
            strategy.runtime.events.push(StrategyRuntimeEvent {
                event_type: "martingale_safety_leg_blocked".to_string(),
                detail: error.to_string(),
                price: None,
                created_at: Utc::now(),
            });
        }
        if added > 0 {
            strategy.runtime.events.push(StrategyRuntimeEvent {
                event_type: "martingale_safety_legs_generated".to_string(),
                detail: format!("generated {added} martingale safety leg orders"),
                price: None,
                created_at: Utc::now(),
            });
        }
        if added > 0 || blocked_error.is_some() {
            db.update_strategy(&strategy)?;
        }
    }
    Ok(())
}

fn martingale_executor_anchor_price(strategy: &Strategy) -> Option<Decimal> {
    strategy
        .runtime
        .orders
        .iter()
        .find(|order| order.order_id.starts_with("mg-") && order.level_index == Some(0))
        .and_then(|order| order.price)
        .or_else(|| {
            strategy
                .active_revision
                .as_ref()
                .or(Some(&strategy.draft_revision))
                .and_then(|revision| revision.reference_price)
        })
        .filter(|price| *price > Decimal::ZERO)
}

fn martingale_runtime_context_for_strategy(
    db: &SharedDb,
    owner: &str,
    strategy_id: &str,
    fallback_status: StrategyStatus,
    portfolio_drawdown_pct: Option<f64>,
) -> Result<MartingaleRuntimeContext, shared_db::SharedDbError> {
    let strategy = db.find_strategy(owner, strategy_id)?;
    Ok(MartingaleRuntimeContext {
        now_ms: Some(Utc::now().timestamp_millis()),
        strategy_status: strategy
            .as_ref()
            .map(|strategy| strategy.status)
            .unwrap_or(fallback_status),
        last_cycle_closed_at_ms: strategy
            .as_ref()
            .and_then(last_martingale_cycle_closed_at_ms),
        portfolio_drawdown_pct,
        ..MartingaleRuntimeContext::default()
    })
}

fn last_martingale_cycle_closed_at_ms(strategy: &Strategy) -> Option<i64> {
    strategy
        .runtime
        .events
        .iter()
        .rev()
        .find(|event| {
            matches!(
                event.event_type.as_str(),
                "martingale_take_profit_stop"
                    | "martingale_strategy_drawdown_stop"
                    | "martingale_cycle_closed"
                    | "martingale_runtime_stopped"
            )
        })
        .map(|event| event.created_at.timestamp_millis())
}

fn martingale_runtime_orders_to_strategy_orders(
    orders: &[MartingaleRuntimeOrder],
) -> Vec<StrategyRuntimeOrder> {
    orders
        .iter()
        .map(|order| StrategyRuntimeOrder {
            order_id: order.client_order_id.clone(),
            exchange_order_id: order.exchange_order_id.clone(),
            level_index: Some(order.leg_index),
            side: order.side.clone(),
            order_type: "Limit".to_string(),
            price: Some(order.price),
            quantity: order.quantity,
            status: "Working".to_string(),
        })
        .collect()
}

fn executor_strategy_from_martingale_config(
    owner: &str,
    portfolio: &MartingalePortfolioRecord,
    strategy_config: &MartingaleStrategyConfig,
    anchor_price: Decimal,
    runtime_orders: Vec<StrategyRuntimeOrder>,
) -> Result<Strategy, shared_db::SharedDbError> {
    let mode = match strategy_config.direction {
        MartingaleDirection::Long => shared_domain::strategy::StrategyMode::FuturesLong,
        MartingaleDirection::Short => shared_domain::strategy::StrategyMode::FuturesShort,
    };
    let market = match strategy_config.market {
        MartingaleMarketKind::Spot => shared_domain::strategy::StrategyMarket::Spot,
        MartingaleMarketKind::UsdMFutures => shared_domain::strategy::StrategyMarket::FuturesUsdM,
    };
    let margin_mode = strategy_config.margin_mode.map(|mode| match mode {
        MartingaleMarginMode::Isolated => shared_domain::strategy::FuturesMarginMode::Isolated,
        MartingaleMarginMode::Cross => FuturesMarginMode::Cross,
    });
    let budget = strategy_planned_budget_quote(strategy_config)
        .filter(|value| *value > Decimal::ZERO)
        .unwrap_or_else(|| Decimal::ONE);
    let grid_spacing_bps = match strategy_config.spacing {
        MartingaleSpacingModel::FixedPercent { step_bps } => step_bps,
        MartingaleSpacingModel::Atr { .. } => 0,
        MartingaleSpacingModel::CustomSequence { ref steps_bps } => {
            steps_bps.first().copied().unwrap_or_default()
        }
        MartingaleSpacingModel::Multiplier { first_step_bps, .. } => first_step_bps,
        MartingaleSpacingModel::Mixed { ref phases } => phases
            .iter()
            .find_map(|phase| match phase {
                MartingaleSpacingModel::FixedPercent { step_bps } => Some(*step_bps),
                MartingaleSpacingModel::Multiplier { first_step_bps, .. } => Some(*first_step_bps),
                MartingaleSpacingModel::CustomSequence { steps_bps } => steps_bps.first().copied(),
                _ => None,
            })
            .unwrap_or_default(),
    };
    let revision = shared_domain::strategy::StrategyRevision {
        revision_id: format!("{}-executor", strategy_config.strategy_id),
        version: 1,
        strategy_type: StrategyType::MartingaleGrid,
        generation: GridGeneration::Custom,
        levels: vec![GridLevel {
            level_index: 0,
            entry_price: anchor_price,
            quantity: Decimal::ONE,
            take_profit_bps: take_profit_bps_for_revision(&strategy_config.take_profit),
            trailing_bps: None,
        }],
        amount_mode: StrategyAmountMode::Quote,
        futures_margin_mode: margin_mode,
        leverage: strategy_config.leverage,
        reference_price_source: ReferencePriceSource::Manual,
        reference_price: Some(anchor_price),
        overall_take_profit_bps: None,
        overall_stop_loss_bps: None,
        post_trigger_action: PostTriggerAction::Stop,
    };
    Ok(Strategy {
        id: strategy_config.strategy_id.clone(),
        owner_email: owner.to_owned(),
        name: format!("{} {}", portfolio.name, strategy_config.strategy_id),
        symbol: strategy_config.symbol.clone(),
        budget: budget.normalize().to_string(),
        grid_spacing_bps,
        status: StrategyStatus::Running,
        source_template_id: Some(portfolio.portfolio_id.clone()),
        membership_ready: true,
        exchange_ready: true,
        permissions_ready: true,
        withdrawals_disabled: true,
        hedge_mode_ready: portfolio.direction == "long_short",
        symbol_ready: true,
        filters_ready: true,
        margin_ready: true,
        conflict_ready: true,
        balance_ready: true,
        strategy_type: StrategyType::MartingaleGrid,
        market,
        mode,
        runtime_phase: StrategyRuntimePhase::Draft,
        runtime_controls: RuntimeControls {},
        draft_revision: revision.clone(),
        active_revision: Some(revision),
        runtime: StrategyRuntime {
            orders: runtime_orders,
            ..StrategyRuntime::default()
        },
        tags: vec![
            "martingale-portfolio-executor".to_string(),
            portfolio.portfolio_id.clone(),
        ],
        notes: notes_with_martingale_config(
            format!(
                "Auto-created executor for martingale portfolio {} from {}",
                portfolio.portfolio_id, portfolio.source_task_id
            ),
            strategy_config,
        )?,
        archived_at: None,
    })
}

fn notes_with_martingale_config(
    base_note: String,
    strategy_config: &MartingaleStrategyConfig,
) -> Result<String, shared_db::SharedDbError> {
    let encoded = serde_json::to_string(strategy_config)
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    Ok(format!(
        "{base_note}\n{MARTINGALE_CONFIG_NOTE_PREFIX}{encoded}"
    ))
}

fn martingale_config_from_strategy_notes(strategy: &Strategy) -> Option<MartingaleStrategyConfig> {
    strategy.notes.lines().find_map(|line| {
        let payload = line.strip_prefix(MARTINGALE_CONFIG_NOTE_PREFIX)?;
        serde_json::from_str::<MartingaleStrategyConfig>(payload).ok()
    })
}

fn take_profit_bps_for_revision(take_profit: &MartingaleTakeProfitModel) -> u32 {
    match take_profit {
        MartingaleTakeProfitModel::Percent { bps } => *bps,
        MartingaleTakeProfitModel::Trailing { activation_bps, .. } => *activation_bps,
        MartingaleTakeProfitModel::Mixed { phases } => phases
            .iter()
            .find_map(|phase| match phase {
                MartingaleTakeProfitModel::Percent { bps } => Some(*bps),
                _ => None,
            })
            .unwrap_or(100),
        MartingaleTakeProfitModel::Amount { .. } | MartingaleTakeProfitModel::Atr { .. } => 100,
    }
}

fn ensure_futures_exchange_settings(
    db: &SharedDb,
    portfolio: &MartingalePortfolioRecord,
    cipher: &CredentialCipher,
    settings: &FuturesExchangeSettings,
) -> Result<(), shared_db::SharedDbError> {
    let Some(credentials) = db.find_exchange_credentials(&portfolio.owner, BINANCE_EXCHANGE)?
    else {
        return Ok(());
    };
    let (api_key, api_secret) = cipher
        .decrypt(&credentials.encrypted_secret)
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    let client = BinanceClient::new(api_key, api_secret);
    client
        .set_usdm_position_mode(settings.hedge_mode)
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    for (symbol, symbol_settings) in &settings.symbols {
        let margin_type = match symbol_settings.margin_mode {
            MartingaleMarginMode::Isolated => "isolated",
            MartingaleMarginMode::Cross => "cross",
        };
        client
            .set_usdm_margin_type(symbol, margin_type)
            .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
        client
            .set_usdm_leverage(symbol, symbol_settings.leverage)
            .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    }
    Ok(())
}

fn strategy_anchor_price(
    strategy_config: &serde_json::Value,
    market_ticks: &[MarketTick],
) -> Option<Decimal> {
    strategy_config
        .get("anchor_price")
        .and_then(decimal_from_json)
        .or_else(|| {
            strategy_config
                .get("reference_price")
                .and_then(decimal_from_json)
        })
        .filter(|value| *value > Decimal::ZERO)
        .or_else(|| {
            let symbol = strategy_config.get("symbol")?.as_str()?;
            let batch_tick = market_ticks
                .iter()
                .rev()
                .find(|tick| {
                    tick.symbol.eq_ignore_ascii_case(symbol)
                        && (tick.market == "usdm"
                            || tick.market == "futures"
                            || tick.market == "usd_m_futures")
                        && tick.price > Decimal::ZERO
                })
                .map(|tick| tick.price);
            batch_tick.or_else(|| {
                latest_market_tick(symbol, "usdm")
                    .or_else(|| latest_market_tick(symbol, "usd_m_futures"))
                    .or_else(|| latest_market_tick(symbol, "futures"))
                    .map(|tick| tick.price)
            })
        })
}

fn futures_settings_from_portfolio(
    portfolio: &MartingalePortfolioRecord,
) -> Result<FuturesExchangeSettings, shared_db::SharedDbError> {
    let mut symbols: HashMap<String, FuturesSymbolSettings> = HashMap::new();
    let strategies = portfolio
        .config
        .get("portfolio_config")
        .and_then(|config| config.get("strategies"))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| shared_db::SharedDbError::new("portfolio strategies are required"))?;
    for strategy in strategies {
        if strategy.get("market").and_then(serde_json::Value::as_str) != Some("usd_m_futures") {
            continue;
        }
        let symbol = strategy
            .get("symbol")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| shared_db::SharedDbError::new("symbol is required"))?;
        let leverage = strategy
            .get("leverage")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| shared_db::SharedDbError::new("leverage is required"))?
            as u32;
        let margin_mode = match strategy
            .get("margin_mode")
            .and_then(serde_json::Value::as_str)
        {
            Some("isolated") => MartingaleMarginMode::Isolated,
            Some("cross") => MartingaleMarginMode::Cross,
            _ => return Err(shared_db::SharedDbError::new("margin_mode is required")),
        };
        if let Some(existing) = symbols.get_mut(symbol) {
            if existing.margin_mode != margin_mode {
                return Err(shared_db::SharedDbError::new(format!(
                    "{symbol} long/short strategies must share margin mode"
                )));
            }
            existing.leverage = existing.leverage.max(leverage);
            continue;
        }
        symbols.insert(
            symbol.to_owned(),
            FuturesSymbolSettings {
                margin_mode,
                leverage,
            },
        );
    }
    Ok(FuturesExchangeSettings {
        hedge_mode: portfolio.direction == "long_short",
        symbols,
    })
}

fn sync_live_orders(
    db: &SharedDb,
    strategy: &mut Strategy,
    cipher: &CredentialCipher,
) -> Result<bool, shared_db::SharedDbError> {
    if !matches!(
        strategy.status,
        StrategyStatus::Running
            | StrategyStatus::Paused
            | StrategyStatus::Stopping
            | StrategyStatus::Stopped
            | StrategyStatus::ErrorPaused
    ) {
        return Ok(false);
    }
    let Some(account) = db.find_exchange_account(&strategy.owner_email, BINANCE_EXCHANGE)? else {
        return Ok(false);
    };
    if !account.is_active {
        return Ok(false);
    }
    let Some(credentials) =
        db.find_exchange_credentials(&strategy.owner_email, BINANCE_EXCHANGE)?
    else {
        return Ok(false);
    };
    let (api_key, api_secret) = cipher
        .decrypt(&credentials.encrypted_secret)
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    let client = shared_binance::BinanceClient::new(api_key, api_secret);
    if strategy.strategy_type == StrategyType::MartingaleGrid {
        let settings = martingale_futures_settings_from_client(strategy, &client)?;
        sync_martingale_production_start(strategy, settings)?;
    }
    let quantization = strategy_quantization_rules(db, strategy)?
        .or_else(|| strategy_quantization_rules_from_exchange(strategy, &client));
    let result = sync_strategy_orders(strategy, &client, quantization.as_ref());
    let trade_result = sync_strategy_trades(db, strategy, &client)?;
    let positions_before_recompute = strategy.runtime.positions.clone();
    if strategy.strategy_type == StrategyType::MartingaleGrid {
        recompute_strategy_positions(strategy);
    }
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
    if result.fatal > 0 {
        apply_live_sync_failure(db, strategy, result.fatal, Utc::now())?;
        strategy.status = StrategyStatus::ErrorPaused;
    } else if result.failed > 0 {
        strategy.runtime.events.push(StrategyRuntimeEvent {
            event_type: "live_order_sync_retryable_failure".to_string(),
            detail: format!("{} live order sync actions will retry", result.failed),
            price: None,
            created_at: Utc::now(),
        });
    }
    if let Some(error) = result.last_error.as_deref() {
        strategy.runtime.events.push(StrategyRuntimeEvent {
            event_type: "live_order_sync_error_detail".to_string(),
            detail: error.to_string(),
            price: None,
            created_at: Utc::now(),
        });
    }
    Ok(result.submitted > 0
        || result.canceled > 0
        || result.failed > 0
        || result.fatal > 0
        || trade_result.new_fills > 0
        || positions_before_recompute != strategy.runtime.positions)
}

fn strategy_quantization_rules(
    db: &SharedDb,
    strategy: &Strategy,
) -> Result<Option<OrderQuantizationRules>, shared_db::SharedDbError> {
    let market_key = match strategy.market {
        shared_domain::strategy::StrategyMarket::Spot => "spot",
        shared_domain::strategy::StrategyMarket::FuturesUsdM => "usdm",
        shared_domain::strategy::StrategyMarket::FuturesCoinM => "coinm",
    };
    let symbol = db
        .list_exchange_symbols(&strategy.owner_email, BINANCE_EXCHANGE)?
        .into_iter()
        .find(|record| {
            record.market == market_key && record.symbol.eq_ignore_ascii_case(&strategy.symbol)
        });
    let Some(symbol) = symbol else {
        return Ok(None);
    };
    Ok(Some(order_quantization_rules_from_metadata(
        &symbol.metadata,
    )))
}

fn strategy_quantization_rules_from_exchange(
    strategy: &Strategy,
    client: &BinanceClient,
) -> Option<OrderQuantizationRules> {
    let symbols = match strategy.market {
        shared_domain::strategy::StrategyMarket::Spot => client.spot_symbols_strict().ok()?,
        shared_domain::strategy::StrategyMarket::FuturesUsdM => {
            client.usdm_symbols_strict().ok()?
        }
        shared_domain::strategy::StrategyMarket::FuturesCoinM => {
            client.coinm_symbols_strict().ok()?
        }
    };
    symbols
        .into_iter()
        .find(|symbol| symbol.symbol.eq_ignore_ascii_case(&strategy.symbol))
        .map(|symbol| order_quantization_rules_from_symbol(&symbol))
}

fn order_quantization_rules_from_symbol(symbol: &SymbolMetadata) -> OrderQuantizationRules {
    OrderQuantizationRules {
        price_tick_size: symbol.filters.price_tick_size.parse().ok(),
        quantity_step_size: symbol.filters.quantity_step_size.parse().ok(),
        min_quantity: symbol.filters.min_quantity.parse().ok(),
        min_notional: symbol.filters.min_notional.parse().ok(),
        client_order_id_max_len: 36,
    }
}

fn order_quantization_rules_from_metadata(metadata: &serde_json::Value) -> OrderQuantizationRules {
    let price_tick_size = metadata
        .get("filters")
        .and_then(|filters| filters.get("price_tick_size"))
        .and_then(|value| value.as_str())
        .and_then(|value| value.parse::<rust_decimal::Decimal>().ok());
    let quantity_step_size = metadata
        .get("filters")
        .and_then(|filters| filters.get("quantity_step_size"))
        .and_then(|value| value.as_str())
        .and_then(|value| value.parse::<rust_decimal::Decimal>().ok());
    let min_quantity = metadata
        .get("filters")
        .and_then(|filters| filters.get("min_quantity"))
        .and_then(|value| value.as_str())
        .and_then(|value| value.parse::<rust_decimal::Decimal>().ok());
    let min_notional = metadata
        .get("filters")
        .and_then(|filters| filters.get("min_notional"))
        .and_then(|value| value.as_str())
        .and_then(|value| value.parse::<rust_decimal::Decimal>().ok());
    OrderQuantizationRules {
        price_tick_size,
        quantity_step_size,
        min_quantity,
        min_notional,
        client_order_id_max_len: 36,
    }
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

fn sync_martingale_production_start(
    strategy: &mut Strategy,
    settings: FuturesExchangeSettings,
) -> Result<bool, shared_db::SharedDbError> {
    if strategy.status != StrategyStatus::Running || martingale_has_runtime_orders(strategy) {
        return Ok(false);
    }

    let config = martingale_runtime_config_from_strategy(strategy)?;
    let mut runtime = MartingaleRuntime::new(config)
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    let recovery_report = recover_martingale_runtime(
        &mut runtime,
        MartingaleRecoveryInput {
            positions: strategy
                .runtime
                .positions
                .iter()
                .map(|position| RecoveryPosition {
                    symbol: strategy.symbol.clone(),
                    quantity: position.quantity,
                    entry_price: position.average_entry_price,
                })
                .collect(),
            open_orders: Vec::new(),
            trades: Vec::new(),
        },
    );
    if !recovery_report.complete {
        return Err(shared_db::SharedDbError::new(
            "martingale recovery incomplete blocks live start",
        ));
    }
    let anchor_price = martingale_anchor_price(strategy)?;
    runtime
        .start_cycle_with_futures_preflight(
            &settings,
            &strategy.id,
            anchor_price,
            MartingaleRuntimeContext {
                now_ms: Some(Utc::now().timestamp_millis()),
                last_cycle_closed_at_ms: last_martingale_cycle_closed_at_ms(strategy),
                strategy_status: strategy.status,
                ..MartingaleRuntimeContext::default()
            },
        )
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;

    for order in runtime.orders() {
        strategy.runtime.orders.push(StrategyRuntimeOrder {
            order_id: order.client_order_id.clone(),
            exchange_order_id: order.exchange_order_id.clone(),
            level_index: Some(order.leg_index),
            side: if order.side.eq_ignore_ascii_case("BUY") {
                "Buy".to_string()
            } else {
                "Sell".to_string()
            },
            order_type: "Limit".to_string(),
            price: Some(order.price),
            quantity: order.quantity,
            status: "Working".to_string(),
        });
    }
    strategy.runtime.events.push(StrategyRuntimeEvent {
        event_type: "martingale_runtime_started".to_string(),
        detail: "martingale runtime started through futures preflight".to_string(),
        price: Some(anchor_price),
        created_at: Utc::now(),
    });
    Ok(true)
}

fn martingale_runtime_config_from_strategy(
    strategy: &Strategy,
) -> Result<MartingaleRuntimeConfig, shared_db::SharedDbError> {
    let revision = strategy
        .active_revision
        .as_ref()
        .unwrap_or(&strategy.draft_revision);
    if let Some(mut strategy_config) = martingale_config_from_strategy_notes(strategy) {
        strategy_config.strategy_id = strategy.id.clone();
        strategy_config.symbol = strategy.symbol.clone();
        strategy_config.direction = match strategy.mode {
            shared_domain::strategy::StrategyMode::FuturesShort => MartingaleDirection::Short,
            _ => MartingaleDirection::Long,
        };
        let portfolio_budget_quote = strategy_planned_budget_quote(&strategy_config)
            .filter(|value| *value > Decimal::ZERO)
            .unwrap_or_else(|| strategy_budget(strategy));
        return Ok(MartingaleRuntimeConfig {
            portfolio_id: strategy
                .source_template_id
                .clone()
                .unwrap_or_else(|| strategy.id.clone()),
            strategy_instance_id: revision.revision_id.clone(),
            portfolio: MartingalePortfolioConfig {
                direction_mode: strategy_config.direction_mode,
                strategies: vec![strategy_config],
                risk_limits: MartingaleRiskLimits::default(),
            },
            portfolio_budget_quote,
            exchange_min_notional: rust_decimal::Decimal::ZERO,
        });
    }
    let margin_mode = revision
        .futures_margin_mode
        .map(|mode| match mode {
            shared_domain::strategy::FuturesMarginMode::Isolated => MartingaleMarginMode::Isolated,
            shared_domain::strategy::FuturesMarginMode::Cross => MartingaleMarginMode::Cross,
        })
        .unwrap_or(MartingaleMarginMode::Cross);
    let leverage = revision.leverage.unwrap_or(1);
    let direction = match strategy.mode {
        shared_domain::strategy::StrategyMode::FuturesShort => MartingaleDirection::Short,
        _ => MartingaleDirection::Long,
    };
    let market = match strategy.market {
        shared_domain::strategy::StrategyMarket::Spot => MartingaleMarketKind::Spot,
        shared_domain::strategy::StrategyMarket::FuturesUsdM => MartingaleMarketKind::UsdMFutures,
        shared_domain::strategy::StrategyMarket::FuturesCoinM => {
            return Err(shared_db::SharedDbError::new(
                "martingale live runtime only supports spot and USDT-M futures",
            ));
        }
    };
    let strategy_config = MartingaleStrategyConfig {
        strategy_id: strategy.id.clone(),
        symbol: strategy.symbol.clone(),
        market,
        direction,
        direction_mode: match direction {
            MartingaleDirection::Long => MartingaleDirectionMode::LongOnly,
            MartingaleDirection::Short => MartingaleDirectionMode::ShortOnly,
        },
        margin_mode: (market == MartingaleMarketKind::UsdMFutures).then_some(margin_mode),
        leverage: (market == MartingaleMarketKind::UsdMFutures).then_some(leverage),
        spacing: MartingaleSpacingModel::FixedPercent {
            step_bps: strategy.grid_spacing_bps,
        },
        sizing: MartingaleSizingModel::Multiplier {
            first_order_quote: strategy_budget(strategy),
            multiplier: rust_decimal::Decimal::ONE,
            max_legs: 3,
        },
        take_profit: MartingaleTakeProfitModel::Percent { bps: 100 },
        stop_loss: None,
        indicators: Vec::new(),
        entry_triggers: Vec::new(),
        risk_limits: MartingaleRiskLimits::default(),
    };
    Ok(MartingaleRuntimeConfig {
        portfolio_id: strategy.id.clone(),
        strategy_instance_id: revision.revision_id.clone(),
        portfolio: MartingalePortfolioConfig {
            direction_mode: strategy_config.direction_mode,
            strategies: vec![strategy_config],
            risk_limits: MartingaleRiskLimits::default(),
        },
        portfolio_budget_quote: strategy_budget(strategy),
        exchange_min_notional: rust_decimal::Decimal::ZERO,
    })
}

fn martingale_runtime_config_from_portfolio(
    portfolio: &MartingalePortfolioRecord,
) -> Result<MartingaleRuntimeConfig, shared_db::SharedDbError> {
    let config_value = portfolio
        .config
        .get("portfolio_config")
        .cloned()
        .ok_or_else(|| shared_db::SharedDbError::new("portfolio_config is required"))?;
    let mut config: MartingalePortfolioConfig = serde_json::from_value(config_value.clone())
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    apply_portfolio_weight_scaling(&mut config, &config_value)?;
    config.validate().map_err(shared_db::SharedDbError::new)?;
    let portfolio_budget_quote = config
        .risk_limits
        .max_global_budget_quote
        .filter(|value| *value > Decimal::ZERO)
        .unwrap_or_else(|| {
            config
                .strategies
                .iter()
                .filter_map(strategy_planned_budget_quote)
                .reduce(|acc, value| acc + value)
                .unwrap_or_else(|| Decimal::new(10000, 0))
        });

    Ok(MartingaleRuntimeConfig {
        portfolio_id: portfolio.portfolio_id.clone(),
        strategy_instance_id: "portfolio".to_owned(),
        portfolio: config,
        portfolio_budget_quote,
        exchange_min_notional: Decimal::ZERO,
    })
}

fn apply_portfolio_weight_scaling(
    config: &mut MartingalePortfolioConfig,
    config_value: &serde_json::Value,
) -> Result<(), shared_db::SharedDbError> {
    let weights = portfolio_weight_factors(config_value)?;
    apply_global_budget_allocations(config, &weights);
    Ok(())
}

fn portfolio_weight_factors(
    config_value: &serde_json::Value,
) -> Result<HashMap<String, Decimal>, shared_db::SharedDbError> {
    let mut weights = HashMap::new();
    let Some(strategies) = config_value
        .get("strategies")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(weights);
    };
    for strategy in strategies {
        let Some(strategy_id) = strategy
            .get("strategy_id")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let Some(weight_pct) = strategy
            .get("portfolio_weight_pct")
            .or_else(|| strategy.get("weight_pct"))
            .and_then(decimal_from_json)
        else {
            continue;
        };
        if weight_pct <= Decimal::ZERO {
            return Err(shared_db::SharedDbError::new(format!(
                "portfolio weight for {strategy_id} must be positive"
            )));
        }
        weights.insert(strategy_id.to_owned(), weight_pct / Decimal::new(100, 0));
    }
    Ok(weights)
}

fn strategy_planned_budget_quote(strategy: &MartingaleStrategyConfig) -> Option<Decimal> {
    // Planned MARGIN capital, computed through the SAME path as the runtime's
    // margin-exposure accounting (compute_leg_notionals -> Decimal -> /leverage,
    // per leg) so the budget and the exposure sum match exactly with no
    // Decimal rounding drift. Margin = notional / leverage for futures and
    // = notional for spot. See backtest_engine::martingale::capital.
    let leverage = match strategy.market {
        MartingaleMarketKind::Spot => Decimal::ONE,
        MartingaleMarketKind::UsdMFutures => {
            Decimal::from(strategy.leverage.unwrap_or(1).max(1))
        }
    };
    let notionals = backtest_engine::martingale::rules::compute_leg_notionals(
        &strategy.sizing,
        f64::MAX,
        backtest_engine::martingale::capital::DEFAULT_EXCHANGE_MIN_NOTIONAL,
    )
    .ok()?;
    let mut total = Decimal::ZERO;
    for notional in notionals {
        let notional_dec = Decimal::try_from(notional)
            .map(|value| value.normalize())
            .ok()?;
        total += notional_dec / leverage;
    }
    Some(total)
}

fn decimal_from_json(value: &serde_json::Value) -> Option<Decimal> {
    value
        .as_str()
        .or_else(|| value.as_i64().map(|_| ""))
        .and_then(|text| {
            if text.is_empty() {
                None
            } else {
                text.parse::<Decimal>().ok()
            }
        })
        .or_else(|| {
            value
                .as_f64()
                .and_then(|number| Decimal::try_from(number).ok())
        })
}

fn martingale_futures_settings_from_client(
    strategy: &Strategy,
    client: &impl MartingaleExchangeSettingsSource,
) -> Result<FuturesExchangeSettings, shared_db::SharedDbError> {
    client.martingale_futures_settings(strategy)
}

fn sync_martingale_production_start_from_exchange(
    strategy: &mut Strategy,
    source: &impl MartingaleExchangeSettingsSource,
) -> Result<bool, shared_db::SharedDbError> {
    let settings = martingale_futures_settings_from_client(strategy, source)?;
    sync_martingale_production_start(strategy, settings)
}

trait MartingaleExchangeSettingsSource {
    fn martingale_futures_settings(
        &self,
        strategy: &Strategy,
    ) -> Result<FuturesExchangeSettings, shared_db::SharedDbError>;
}

impl MartingaleExchangeSettingsSource for BinanceClient {
    fn martingale_futures_settings(
        &self,
        strategy: &Strategy,
    ) -> Result<FuturesExchangeSettings, shared_db::SharedDbError> {
        if !matches!(
            strategy.market,
            shared_domain::strategy::StrategyMarket::FuturesUsdM
        ) {
            return Ok(FuturesExchangeSettings {
                hedge_mode: false,
                symbols: HashMap::new(),
            });
        }
        let hedge_mode = self
            .read_usdm_position_mode()
            .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
        let symbol_settings = self
            .read_usdm_symbol_settings(&strategy.symbol)
            .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
        let margin_mode = match symbol_settings.margin_type.as_deref() {
            Some("isolated") => MartingaleMarginMode::Isolated,
            _ => MartingaleMarginMode::Cross,
        };
        let leverage = symbol_settings.leverage.unwrap_or(1);
        let mut symbols = HashMap::new();
        symbols.insert(
            strategy.symbol.clone(),
            FuturesSymbolSettings {
                margin_mode,
                leverage,
            },
        );
        Ok(FuturesExchangeSettings {
            hedge_mode,
            symbols,
        })
    }
}

fn martingale_anchor_price(
    strategy: &Strategy,
) -> Result<rust_decimal::Decimal, shared_db::SharedDbError> {
    strategy
        .active_revision
        .as_ref()
        .unwrap_or(&strategy.draft_revision)
        .reference_price
        .or_else(|| {
            strategy
                .active_revision
                .as_ref()
                .unwrap_or(&strategy.draft_revision)
                .levels
                .first()
                .map(|level| level.entry_price)
        })
        .ok_or_else(|| shared_db::SharedDbError::new("martingale start requires anchor price"))
}

fn strategy_budget(strategy: &Strategy) -> rust_decimal::Decimal {
    strategy
        .budget
        .parse::<rust_decimal::Decimal>()
        .ok()
        .filter(|value| *value > rust_decimal::Decimal::ZERO)
        .unwrap_or(rust_decimal::Decimal::ONE)
}

fn martingale_has_runtime_orders(strategy: &Strategy) -> bool {
    strategy
        .runtime
        .orders
        .iter()
        .any(|order| order.order_id.starts_with("mg-") && order.order_id.contains("-leg-"))
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

    if strategy.strategy_type == StrategyType::MartingaleGrid {
        return apply_martingale_market_ticks(db, strategy, &relevant);
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
                &format!(
                    "{} reached overall take profit on {}.",
                    strategy.name, strategy.symbol
                ),
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
                &format!(
                    "{} reached overall stop loss on {}.",
                    strategy.name, strategy.symbol
                ),
                serde_json::json!({
                    "strategy_id": strategy.id,
                    "trigger_price": event.price.map(|price| price.to_string()).unwrap_or_default(),
                }),
                event.created_at,
            )?;
        }
    }

    let next_runtime = engine.snapshot().clone();
    let overall_close_requested = snapshot
        .events
        .iter()
        .skip(before_event_count)
        .any(|event| {
            event.event_type.starts_with("overall_take_profit")
                || event.event_type.starts_with("overall_stop_loss")
        });
    if next_runtime != strategy.runtime {
        strategy.runtime = next_runtime;
        changed = true;
    }
    if overall_close_requested && strategy.status == StrategyStatus::Running {
        strategy.status = StrategyStatus::Stopping;
        changed = true;
    } else if !engine.is_running() && strategy.status == StrategyStatus::Running {
        strategy.status = StrategyStatus::Stopped;
        changed = true;
    }

    Ok(changed)
}

fn apply_martingale_market_ticks(
    db: &SharedDb,
    strategy: &mut Strategy,
    market_ticks: &[&MarketTick],
) -> Result<bool, shared_db::SharedDbError> {
    let mut changed = false;
    let strategy_config = martingale_strategy_config_from_live_strategy(strategy)?;
    for tick in market_ticks {
        if martingale_close_already_requested(strategy) {
            break;
        }
        let Some(position) = martingale_position_for_strategy(strategy) else {
            continue;
        };
        // strategy.runtime.fills is all-time (never cleared on cycle close) and carries no
        // per-cycle marker, so summing all fills would mix in past cycles' PnL/fees. At
        // SL-evaluation time the current cycle is open and losing (no TP has fired), so
        // current-cycle realized PnL is ~0. Fall back to the position-based approximation:
        // realized = 0, entry_fees = entry notional * (DEFAULT_FEE_BPS + DEFAULT_SLIPPAGE_BPS)
        // to match backtest `entry_cost_quote` exactly (fee + slippage on entry notional).
        // The dominant SL term is the leverage-amplified unrealized loss.
        let realized_pnl = Decimal::ZERO;
        let entry_fees = position.quantity.abs()
            * position.average_entry_price
            * Decimal::from_f64_retain(
                backtest_engine::martingale::kline_engine::DEFAULT_FEE_BPS
                    + backtest_engine::martingale::kline_engine::DEFAULT_SLIPPAGE_BPS,
            )
            .unwrap_or(Decimal::ZERO)
            / Decimal::from(10_000_u32);
        let Some(exit) = martingale_exit_signal(
            &strategy_config,
            &position,
            tick.price,
            realized_pnl,
            entry_fees,
        ) else {
            continue;
        };
        request_martingale_close(strategy, &position, tick.price, &exit);
        strategy.status = StrategyStatus::Stopping;
        changed = true;
        persist_runtime_notification(
            db,
            strategy,
            if exit.event_type == "martingale_take_profit_stop" {
                NotificationKind::OverallTakeProfitTriggered
            } else {
                NotificationKind::OverallStopLossTriggered
            },
            if exit.event_type == "martingale_take_profit_stop" {
                "Martingale take profit reached"
            } else {
                "Martingale stop loss reached"
            },
            &format!(
                "{} triggered {} on {}.",
                strategy.name, exit.label, strategy.symbol
            ),
            serde_json::json!({
                "strategy_id": strategy.id,
                "trigger_price": tick.price.to_string(),
                "reason": exit.event_type,
            }),
            Utc::now(),
        )?;
    }
    Ok(changed)
}

struct MartingaleExitSignal {
    event_type: &'static str,
    label: &'static str,
    threshold_price: Decimal,
}

fn martingale_exit_signal(
    config: &MartingaleStrategyConfig,
    position: &StrategyRuntimePosition,
    current_price: Decimal,
    realized_pnl: Decimal,
    entry_fees: Decimal,
) -> Option<MartingaleExitSignal> {
    if current_price <= Decimal::ZERO || position.average_entry_price <= Decimal::ZERO {
        return None;
    }
    let is_long = config.direction == MartingaleDirection::Long;
    if let Some(threshold_price) =
        martingale_percent_take_profit_price(config, position.average_entry_price)
    {
        let triggered = if is_long {
            current_price >= threshold_price
        } else {
            current_price <= threshold_price
        };
        if triggered {
            return Some(MartingaleExitSignal {
                event_type: "martingale_take_profit_stop",
                label: "take profit",
                threshold_price,
            });
        }
    }
    if let Some(dd) = martingale_strategy_drawdown_pct(
        config,
        position.quantity,
        position.average_entry_price,
        current_price,
        realized_pnl,
        entry_fees,
    ) {
        let threshold = match &config.stop_loss {
            Some(MartingaleStopLossModel::StrategyDrawdownPct { pct_bps }) => {
                *pct_bps as f64 / 100.0
            }
            _ => 0.0,
        };
        if dd >= threshold {
            return Some(MartingaleExitSignal {
                event_type: "martingale_strategy_drawdown_stop",
                label: "strategy drawdown stop",
                threshold_price: current_price,
            });
        }
    }
    None
}

fn martingale_percent_take_profit_price(
    config: &MartingaleStrategyConfig,
    average_entry: Decimal,
) -> Option<Decimal> {
    let bps = match &config.take_profit {
        MartingaleTakeProfitModel::Percent { bps } => *bps,
        _ => return None,
    };
    let offset = average_entry * Decimal::from(bps) / Decimal::from(10_000_u32);
    Some(match config.direction {
        MartingaleDirection::Long => average_entry + offset,
        MartingaleDirection::Short => average_entry - offset,
    })
    .filter(|price| *price > Decimal::ZERO)
}

fn martingale_close_already_requested(strategy: &Strategy) -> bool {
    strategy.runtime.orders.iter().any(|order| {
        (order.order_id.starts_with("cl-")
            || order.order_id.starts_with("close-")
            || order.order_id.contains("-stop-close-"))
            && matches!(
                order.status.as_str(),
                "ClosingRequested" | "Placed" | "PartiallyFilled"
            )
    })
}

fn martingale_position_for_strategy(strategy: &Strategy) -> Option<StrategyRuntimePosition> {
    let mode = strategy.mode;
    strategy
        .runtime
        .positions
        .iter()
        .find(|position| position.mode == mode)
        .cloned()
        .or_else(|| strategy.runtime.positions.first().cloned())
}

fn request_martingale_close(
    strategy: &mut Strategy,
    position: &StrategyRuntimePosition,
    current_price: Decimal,
    exit: &MartingaleExitSignal,
) {
    let position_index = strategy
        .runtime
        .positions
        .iter()
        .position(|candidate| {
            candidate.mode == position.mode
                && candidate.market == position.market
                && candidate.quantity == position.quantity
                && candidate.average_entry_price == position.average_entry_price
        })
        .unwrap_or(0);
    let order_id = martingale_close_client_order_id(&strategy.id, position_index);
    if strategy.runtime.orders.iter().any(|order| {
        (order.order_id == order_id
            || order.order_id.starts_with("close-")
            || order.order_id.contains("-stop-close-"))
            && order
                .side
                .eq_ignore_ascii_case(close_side_for_runtime_position(position))
            && matches!(
                order.status.as_str(),
                "ClosingRequested" | "Placed" | "PartiallyFilled"
            )
    }) {
        return;
    }
    strategy.runtime.orders.push(StrategyRuntimeOrder {
        order_id,
        exchange_order_id: None,
        level_index: None,
        side: close_side_for_runtime_position(position).to_string(),
        order_type: "Market".to_string(),
        price: None,
        quantity: position.quantity.abs(),
        status: "ClosingRequested".to_string(),
    });
    strategy.runtime.events.push(StrategyRuntimeEvent {
        event_type: exit.event_type.to_string(),
        detail: format!(
            "{} triggered at {}; threshold_price={}",
            exit.label,
            current_price.normalize(),
            exit.threshold_price.normalize()
        ),
        price: Some(current_price),
        created_at: Utc::now(),
    });
}

fn martingale_close_client_order_id(strategy_id: &str, position_index: usize) -> String {
    let mut hasher = DefaultHasher::new();
    strategy_id.hash(&mut hasher);
    position_index.hash(&mut hasher);
    format!("cl-{hash:016x}-{position_index}", hash = hasher.finish())
}

fn close_side_for_runtime_position(position: &StrategyRuntimePosition) -> &'static str {
    match position.mode {
        shared_domain::strategy::StrategyMode::FuturesShort
        | shared_domain::strategy::StrategyMode::SpotSellOnly => "Buy",
        _ => "Sell",
    }
}

fn martingale_strategy_config_from_live_strategy(
    strategy: &Strategy,
) -> Result<MartingaleStrategyConfig, shared_db::SharedDbError> {
    let config = martingale_runtime_config_from_strategy(strategy)?;
    config
        .portfolio
        .strategies
        .into_iter()
        .find(|candidate| candidate.strategy_id == strategy.id)
        .ok_or_else(|| shared_db::SharedDbError::new("martingale strategy config not found"))
}

fn strategy_market_code(market: shared_domain::strategy::StrategyMarket) -> &'static str {
    match market {
        shared_domain::strategy::StrategyMarket::Spot => "spot",
        shared_domain::strategy::StrategyMarket::FuturesUsdM => "usdm",
        shared_domain::strategy::StrategyMarket::FuturesCoinM => "coinm",
    }
}

fn telegram_bot_token() -> Option<&'static str> {
    static TOKEN: OnceLock<Option<String>> = OnceLock::new();
    TOKEN
        .get_or_init(|| {
            std::env::var("TELEGRAM_BOT_TOKEN")
                .ok()
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        })
        .as_deref()
}

fn telegram_api_base_url() -> &'static str {
    static URL: OnceLock<String> = OnceLock::new();
    URL.get_or_init(|| {
        std::env::var("TELEGRAM_API_BASE_URL")
            .ok()
            .map(|v| v.trim().trim_end_matches('/').to_owned())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "https://api.telegram.org".to_string())
    })
}

fn telegram_http_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(5))
            .build()
    })
}

fn send_telegram_message(chat_id: &str, text: &str) -> Result<(), String> {
    let Some(token) = telegram_bot_token() else {
        return Err("TELEGRAM_BOT_TOKEN not configured".to_string());
    };
    telegram_http_agent()
        .post(&format!(
            "{}/bot{}/sendMessage",
            telegram_api_base_url(),
            token
        ))
        .send_json(ureq::json!({
            "chat_id": chat_id,
            "text": text,
        }))
        .map(|_| ())
        .map_err(|e| format!("telegram send failed: {}", e))
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
    let binding = db
        .find_telegram_binding(&strategy.owner_email)
        .ok()
        .flatten();
    let telegram_delivered = match (binding.as_ref(), telegram_bot_token()) {
        (Some(binding), Some(_token)) => {
            let text = format!("<b>{}</b>\n{}", title, body);
            match send_telegram_message(&binding.telegram_chat_id, &text) {
                Ok(()) => true,
                Err(err) => {
                    eprintln!(
                        "trading-engine telegram send failed for {}: {}",
                        strategy.owner_email, err
                    );
                    false
                }
            }
        }
        _ => false,
    };

    let mut record_payload = payload.clone();
    if let Some(object) = record_payload.as_object_mut() {
        object.insert(
            "telegram_delivered".to_string(),
            serde_json::json!(telegram_delivered),
        );
    }
    db.insert_notification_log(&NotificationLogRecord {
        user_email: strategy.owner_email.clone(),
        channel: "in_app".to_string(),
        template_key: Some(format!("{:?}", kind)),
        title: title.to_string(),
        body: body.to_string(),
        status: "delivered".to_string(),
        payload: record_payload.clone(),
        created_at,
        delivered_at: Some(created_at),
    })?;

    let telegram_status = if telegram_delivered {
        "delivered"
    } else {
        "failed"
    };
    db.insert_notification_log(&NotificationLogRecord {
        user_email: strategy.owner_email.clone(),
        channel: "telegram".to_string(),
        template_key: Some(format!("{:?}", kind)),
        title: title.to_string(),
        body: body.to_string(),
        status: telegram_status.to_string(),
        payload: record_payload,
        created_at,
        delivered_at: if telegram_delivered {
            Some(created_at)
        } else {
            None
        },
    })?;

    Ok(())
}

fn live_mode_enabled() -> bool {
    std::env::var("BINANCE_LIVE_MODE")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on" | "live"
            )
        })
        .unwrap_or(false)
}

fn credential_cipher() -> Result<CredentialCipher, shared_db::SharedDbError> {
    CredentialCipher::from_env("EXCHANGE_CREDENTIALS_MASTER_KEY")
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))
}

fn sync_user_streams(db: &SharedDb) -> Result<(), shared_db::SharedDbError> {
    static HANDLES: std::sync::OnceLock<Mutex<HashMap<String, JoinHandle<()>>>> =
        std::sync::OnceLock::new();
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
            eprintln!(
                "trading-engine user stream {} {} failed: {error}",
                email, market
            );
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn run_user_stream_once(
    db: &SharedDb,
    email: &str,
    market: &str,
) -> Result<(), shared_db::SharedDbError> {
    let cipher = credential_cipher()?;
    let Some(credentials) = db.find_exchange_credentials(email, BINANCE_EXCHANGE)? else {
        return Ok(());
    };
    let (api_key, api_secret) = cipher
        .decrypt(&credentials.encrypted_secret)
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    let client = BinanceClient::new(api_key.clone(), api_secret);
    // REST reconciliation after stream (re)connect
    if market == "usdm" {
        run_user_stream_rest_backfill(db, email, &client, market)?;
    }
    let stream = client
        .start_user_data_stream(market)
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    if let Some(keepalive_key) = user_stream_keepalive_key(&stream) {
        let keepalive_client = client.clone();
        let keepalive_market = market.to_string();
        let keepalive_email = email.to_string();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30 * 60));
            loop {
                interval.tick().await;
                if let Err(error) =
                    keepalive_client.keepalive_user_data_stream(&keepalive_market, &keepalive_key)
                {
                    eprintln!(
                        "trading-engine user stream keepalive {} {} failed: {}",
                        keepalive_email, keepalive_market, error
                    );
                    break;
                }
            }
        });
    }
    let (socket, _) = connect_async(stream.websocket_url.as_str())
        .await
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    let (mut write, mut read) = socket.split();
    if let Some(subscribe_request) = stream.subscribe_request.as_ref() {
        write
            .send(Message::Text(subscribe_request.clone().into()))
            .await
            .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    }

    while let Some(message) = read.next().await {
        let message = message.map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
        let payload = match message {
            Message::Text(text) => text.to_string(),
            Message::Binary(bytes) => String::from_utf8(bytes.to_vec())
                .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?,
            Message::Ping(payload) => {
                write
                    .send(Message::Pong(payload))
                    .await
                    .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
                continue;
            }
            Message::Pong(_) => continue,
            Message::Close(_) => break,
            _ => continue,
        };
        if let Some(update) = parse_user_data_message(market, &payload) {
            apply_execution_update_for_user(db, email, &client, &update)?;
        }
        if let Some(account_update) = parse_account_update_message(market, &payload) {
            apply_account_update_for_user(db, email, &account_update)?;
        }
    }
    Ok(())
}

fn user_stream_keepalive_key(stream: &BinanceUserDataStream) -> Option<String> {
    let key = stream.listen_key.trim();
    (!key.is_empty()).then(|| key.to_string())
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
    if strategy.strategy_type == StrategyType::MartingaleGrid {
        recompute_strategy_positions(strategy);
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

/// Run REST reconciliation after stream (re)connect.
/// Backfills openOrders, userTrades, account and balance positions.
fn run_user_stream_rest_backfill(
    db: &SharedDb,
    email: &str,
    client: &BinanceClient,
    market: &str,
) -> Result<(), shared_db::SharedDbError> {
    if market != "usdm" {
        return Ok(());
    }
    // Reconcile open orders by client_order_id
    for mut strategy in db.list_strategies(email)? {
        if !matches!(
            strategy.market,
            shared_domain::strategy::StrategyMarket::FuturesUsdM
        ) {
            continue;
        }
        if let Ok(orders) = client.open_orders(market, &strategy.symbol) {
            for order in &orders {
                if let Some(ref client_id) = order.client_order_id {
                    for runtime_order in &mut strategy.runtime.orders {
                        if runtime_order.order_id == *client_id
                            && runtime_order.exchange_order_id.is_none()
                        {
                            runtime_order.exchange_order_id = Some(order.order_id.clone());
                            if runtime_order.status != "Filled" {
                                runtime_order.status = "Placed".to_string();
                            }
                            break;
                        }
                    }
                }
            }
            db.update_strategy(&strategy)?;
        }
    }
    // Reconcile user trades idempotently --- write structured trade history
    // with real commission from Binance userTrades.commission (not price*0.001).
    for mut strategy in db.list_strategies(email)? {
        if !matches!(
            strategy.market,
            shared_domain::strategy::StrategyMarket::FuturesUsdM
        ) {
            continue;
        }
        if let Ok(trades) = client.user_trades(market, &strategy.symbol, 100) {
            for trade in &trades {
                // Persist to exchange_account_trade_history idempotently by trade_id.
                let existing = db
                    .list_exchange_trade_history(email)?
                    .iter()
                    .any(|t| t.trade_id == trade.trade_id);
                if !existing {
                    let _ =
                        db.insert_exchange_trade_history(&shared_db::ExchangeTradeHistoryRecord {
                            trade_id: trade.trade_id.clone(),
                            user_email: email.to_string(),
                            exchange: "binance".to_string(),
                            symbol: strategy.symbol.clone(),
                            side: trade.side.clone(),
                            quantity: trade.quantity.clone(),
                            price: trade.price.clone(),
                            fee_amount: trade.fee_amount.clone(),
                            fee_asset: trade.fee_asset.clone(),
                            traded_at: chrono::DateTime::from_timestamp_millis(trade.traded_at_ms)
                                .unwrap_or(Utc::now()),
                        });
                }
                let trade_id = format!(
                    "{}_{}",
                    trade.order_id.as_deref().unwrap_or(""),
                    trade.trade_id
                );
                if strategy
                    .runtime
                    .events
                    .iter()
                    .any(|ev| ev.event_type == "trade" && ev.detail.contains(&trade_id))
                {
                    continue;
                }
                strategy.runtime.events.push(StrategyRuntimeEvent {
                    event_type: "trade".to_string(),
                    detail: format!(
                        "backfilled trade {trade_id} price={} qty={}",
                        trade.price, trade.quantity
                    ),
                    price: trade.price.parse::<rust_decimal::Decimal>().ok(),
                    created_at: Utc::now(),
                });
            }
            db.update_strategy(&strategy)?;
        }
    }
    // Reconcile account positions (create if missing).
    // Collect total_unrealized at function scope so it can be merged with income
    // into a single account profit snapshot.
    let mut total_unrealized: rust_decimal::Decimal = rust_decimal::Decimal::ZERO;
    if let Ok(account) = client.read_usdm_account_v3() {
        let mut seen_positions: HashSet<(String, String)> = HashSet::new();
        for pos in &account.positions {
            let side = pos
                .position_side
                .clone()
                .unwrap_or_else(|| "long".to_string());
            let key = (pos.symbol.to_ascii_uppercase(), side.clone());
            if seen_positions.insert(key) {
                let unrealized: rust_decimal::Decimal =
                    pos.unrealized_pnl.parse().unwrap_or_default();
                total_unrealized += unrealized;
            }
        }
        for mut strategy in db.list_strategies(email)? {
            let mut dirty = false;
            for pos in &account.positions {
                if !strategy.symbol.eq_ignore_ascii_case(&pos.symbol) {
                    continue;
                }
                let amount: rust_decimal::Decimal = pos.position_amount.parse().unwrap_or_default();
                if amount.is_zero() {
                    continue;
                }
                if strategy.runtime.positions.is_empty() {
                    strategy.runtime.positions.push(StrategyRuntimePosition {
                        market: shared_domain::strategy::StrategyMarket::FuturesUsdM,
                        mode: if pos.position_side.as_deref() == Some("short") {
                            shared_domain::strategy::StrategyMode::FuturesShort
                        } else {
                            shared_domain::strategy::StrategyMode::FuturesLong
                        },
                        quantity: amount,
                        average_entry_price: pos.entry_price.parse().unwrap_or_default(),
                    });
                    dirty = true;
                    continue;
                }
                // Hedge mode: a single symbol may carry both LONG and SHORT
                // positions. Only update the runtime position whose direction
                // matches this exchange position; otherwise a SHORT position's
                // (negative) amount would overwrite the LONG runtime position
                // and vice versa, corrupting state on every stream reconnect.
                let side_is_short = pos.position_side.as_deref() == Some("short");
                for runtime_pos in &mut strategy.runtime.positions {
                    let runtime_is_short = matches!(
                        runtime_pos.mode,
                        shared_domain::strategy::StrategyMode::FuturesShort
                    );
                    if side_is_short != runtime_is_short {
                        continue;
                    }
                    runtime_pos.quantity = amount;
                    if let Ok(entry) = pos.entry_price.parse::<rust_decimal::Decimal>() {
                        runtime_pos.average_entry_price = entry;
                    }
                    dirty = true;
                }
            }
            if dirty {
                db.update_strategy(&strategy)?;
            }
        }
    }
    // Reconcile income: FUNDING_FEE, COMMISSION, REALIZED_PNL --- collect totals
    // then write a single merged account snapshot containing realized, fees, and funding.
    let mut total_funding: rust_decimal::Decimal = rust_decimal::Decimal::ZERO;
    let mut total_commission: rust_decimal::Decimal = rust_decimal::Decimal::ZERO;
    let mut total_realized: rust_decimal::Decimal = rust_decimal::Decimal::ZERO;
    for mut strategy in db.list_strategies(email)? {
        if !matches!(
            strategy.market,
            shared_domain::strategy::StrategyMarket::FuturesUsdM
        ) {
            continue;
        }
        if let Ok(incomes) = client.read_usdm_income(&strategy.symbol, None, 100) {
            for income in &incomes {
                let income_type = income
                    .get("incomeType")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let income_id = income.get("tranId").and_then(|v| v.as_i64()).unwrap_or(0);
                let income_uid = format!("income_{income_type}_{income_id}");
                if strategy
                    .runtime
                    .events
                    .iter()
                    .any(|ev| ev.event_type == "income" && ev.detail.contains(&income_uid))
                {
                    continue;
                }
                let amount: rust_decimal::Decimal = income
                    .get("income")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0")
                    .parse()
                    .unwrap_or_default();
                match income_type {
                    "COMMISSION" => total_commission += amount,
                    "FUNDING_FEE" => total_funding += amount,
                    "REALIZED_PNL" => total_realized += amount,
                    _ => {}
                }
                strategy.runtime.events.push(StrategyRuntimeEvent {
                    event_type: "income".to_string(),
                    detail: format!(
                        "backfilled {} income={} asset={} id={}",
                        income_uid,
                        amount,
                        income.get("asset").and_then(|v| v.as_str()).unwrap_or(""),
                        income_id,
                    ),
                    price: Some(amount),
                    created_at: Utc::now(),
                });
            }
            db.update_strategy(&strategy)?;
        }
    }
    // Write a single merged account snapshot with all fields populated
    // (realized from income, unrealized from account_v3, fees/funding from income).
    let needs_snapshot = total_realized != rust_decimal::Decimal::ZERO
        || total_commission != rust_decimal::Decimal::ZERO
        || total_funding != rust_decimal::Decimal::ZERO
        || total_unrealized != rust_decimal::Decimal::ZERO;
    if needs_snapshot {
        let _ = db.insert_account_profit_snapshot(&shared_db::AccountProfitSnapshotRecord {
            user_email: email.to_string(),
            exchange: "binance".to_string(),
            realized_pnl: total_realized.to_string(),
            unrealized_pnl: total_unrealized.to_string(),
            fees: total_commission.to_string(),
            funding: Some(total_funding.to_string()),
            captured_at: Utc::now(),
        });
    }
    // Reconcile balances --- persist to exchange_wallet_snapshots (structured table).
    if let Ok(balances) = client.read_usdm_account_v3_balance() {
        let mut wallet_balances: serde_json::Map<String, serde_json::Value> =
            serde_json::Map::new();
        for balance in &balances {
            wallet_balances.insert(balance.asset.clone(), serde_json::json!(balance.balance));
        }
        let _ = db.insert_exchange_wallet_snapshot(&shared_db::ExchangeWalletSnapshotRecord {
            user_email: email.to_string(),
            exchange: "binance".to_string(),
            wallet_type: "futures".to_string(),
            balances: serde_json::Value::Object(wallet_balances),
            captured_at: Utc::now(),
        });
    }

    // Persist last_rest_reconcile_at timestamp across all strategies
    for mut strategy in db.list_strategies(email)? {
        let ts = Utc::now().to_rfc3339();
        if !strategy
            .runtime
            .events
            .iter()
            .any(|ev| ev.event_type == "last_rest_reconcile_at")
        {
            strategy.runtime.events.push(StrategyRuntimeEvent {
                event_type: "last_rest_reconcile_at".to_string(),
                detail: ts,
                price: None,
                created_at: Utc::now(),
            });
        } else {
            for ev in &mut strategy.runtime.events {
                if ev.event_type == "last_rest_reconcile_at" {
                    ev.detail = ts.clone();
                    ev.created_at = Utc::now();
                }
            }
        }
        db.update_strategy(&strategy)?;
    }

    Ok(())
}

/// Apply ACCOUNT_UPDATE event. Creates position snapshots when none exist,
/// and writes structured wallet and profit snapshots.
/// Deduplicates (symbol, positionSide) to avoid double-counting unrealized_pnl
/// when the same exchange position maps to multiple strategies.
fn apply_account_update_for_user(
    db: &SharedDb,
    email: &str,
    update: &BinanceAccountUpdate,
) -> Result<(), shared_db::SharedDbError> {
    let mut seen_positions: HashSet<(String, String)> = HashSet::new();
    let mut total_unrealized: rust_decimal::Decimal = rust_decimal::Decimal::ZERO;
    let mut wallet_balances: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();

    for pos_update in &update.positions {
        let side = pos_update
            .position_side
            .clone()
            .unwrap_or_else(|| "long".to_string());
        let key = (pos_update.symbol.to_ascii_uppercase(), side.clone());
        if seen_positions.insert(key) {
            let unrealized: rust_decimal::Decimal =
                pos_update.unrealized_pnl.parse().unwrap_or_default();
            total_unrealized += unrealized;
        }
    }

    for mut strategy in db.list_strategies(email)? {
        let strategy_lock_arc = strategy_lock(&strategy.id);
        let _strategy_guard = strategy_lock_arc.lock().expect("strategy lock poisoned");
        let mut dirty = false;
        for pos_update in &update.positions {
            if !strategy.symbol.eq_ignore_ascii_case(&pos_update.symbol) {
                continue;
            }
            let amount: rust_decimal::Decimal =
                pos_update.position_amount.parse().unwrap_or_default();
            let entry: rust_decimal::Decimal = pos_update.entry_price.parse().unwrap_or_default();
            let mode = match pos_update.position_side.as_deref() {
                Some("short") | Some("SHORT") => {
                    shared_domain::strategy::StrategyMode::FuturesShort
                }
                _ => shared_domain::strategy::StrategyMode::FuturesLong,
            };

            let pos_index = strategy.runtime.positions.iter().position(|rp| {
                rp.market == shared_domain::strategy::StrategyMarket::FuturesUsdM && rp.mode == mode
            });

            if amount.is_zero() {
                if let Some(idx) = pos_index {
                    strategy.runtime.positions[idx].quantity = rust_decimal::Decimal::ZERO;
                    dirty = true;
                }
                continue;
            }

            match pos_index {
                Some(idx) => {
                    strategy.runtime.positions[idx].quantity = amount;
                    if entry > rust_decimal::Decimal::ZERO {
                        strategy.runtime.positions[idx].average_entry_price = entry;
                    }
                    dirty = true;
                }
                None => {
                    strategy.runtime.positions.push(StrategyRuntimePosition {
                        market: shared_domain::strategy::StrategyMarket::FuturesUsdM,
                        mode,
                        quantity: amount,
                        average_entry_price: entry,
                    });
                    dirty = true;
                }
            }
        }
        // Collect wallet balances from ACCOUNT_UPDATE
        for balance_update in &update.balances {
            wallet_balances.insert(
                balance_update.asset.clone(),
                serde_json::json!(balance_update.wallet_balance),
            );
        }
        // Persist last_stream_event_at timestamp
        let ts = Utc::now().to_rfc3339();
        if !strategy
            .runtime
            .events
            .iter()
            .any(|ev| ev.event_type == "last_stream_event_at")
        {
            strategy.runtime.events.push(StrategyRuntimeEvent {
                event_type: "last_stream_event_at".to_string(),
                detail: ts,
                price: None,
                created_at: Utc::now(),
            });
            dirty = true;
        } else {
            for ev in &mut strategy.runtime.events {
                if ev.event_type == "last_stream_event_at" {
                    ev.detail = ts.clone();
                    ev.created_at = Utc::now();
                    dirty = true;
                }
            }
        }

        if dirty {
            db.update_strategy(&strategy)?;
        }
    }

    // Write structured wallet snapshot
    if !wallet_balances.is_empty() {
        let _ = db.insert_exchange_wallet_snapshot(&shared_db::ExchangeWalletSnapshotRecord {
            user_email: email.to_string(),
            exchange: "binance".to_string(),
            wallet_type: "futures".to_string(),
            balances: serde_json::Value::Object(wallet_balances),
            captured_at: Utc::now(),
        });
    }

    // Write account profit snapshot if we have unrealized data
    if total_unrealized != rust_decimal::Decimal::ZERO {
        let _ = db.insert_account_profit_snapshot(&shared_db::AccountProfitSnapshotRecord {
            user_email: email.to_string(),
            exchange: "binance".to_string(),
            realized_pnl: "0".to_string(),
            unrealized_pnl: total_unrealized.to_string(),
            fees: "0".to_string(),
            funding: Some("0".to_string()),
            captured_at: Utc::now(),
        });
    }

    Ok(())
}

fn apply_execution_update_for_user(
    db: &SharedDb,
    email: &str,
    client: &BinanceClient,
    update: &shared_binance::BinanceExecutionUpdate,
) -> Result<(), shared_db::SharedDbError> {
    for mut strategy in db.list_strategies(email)? {
        let strategy_lock_arc = strategy_lock(&strategy.id);
        let _strategy_guard = strategy_lock_arc.lock().expect("strategy lock poisoned");
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
    value
        .and_then(|port| port.parse().ok())
        .unwrap_or(default_port)
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
    use super::{
        apply_live_sync_failure, health_payload, reconcile_once, RuntimeMetrics, DEFAULT_PORT,
        SERVICE_NAME,
    };
    use chrono::Utc;
    use rust_decimal::Decimal;
    use shared_binance::BinanceUserDataStream;
    use shared_db::{
        MartingalePortfolioItemRecord, MartingalePortfolioRecord, SharedDb, StoredStrategy,
    };
    use shared_domain::strategy::{
        GridGeneration, GridLevel, PostTriggerAction, ReferencePriceSource, RuntimeControls,
        Strategy, StrategyAmountMode, StrategyMarket, StrategyMode, StrategyRevision,
        StrategyRuntime, StrategyRuntimePhase, StrategyRuntimePosition, StrategyStatus,
        StrategyType,
    };
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

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
        assert_eq!(
            super::parse_port(Some("not-a-port".to_string()), DEFAULT_PORT),
            DEFAULT_PORT
        );
    }

    #[test]
    fn user_stream_keys_empty_when_live_mode_disabled() {
        let keys = super::desired_user_stream_keys(false, &[("alice@example.com", "spot,usdm")]);
        assert!(keys.is_empty());
    }

    #[test]
    fn user_stream_keys_expand_active_accounts_by_market_scope() {
        let keys = super::desired_user_stream_keys(
            true,
            &[
                ("alice@example.com", "spot,usdm"),
                ("bob@example.com", "coinm"),
            ],
        );
        assert_eq!(
            keys,
            vec![
                "alice@example.com:spot".to_string(),
                "alice@example.com:usdm".to_string(),
                "bob@example.com:coinm".to_string(),
            ]
        );
    }

    #[test]
    fn spot_ws_api_stream_skips_keepalive() {
        let stream = BinanceUserDataStream {
            market: "spot".to_string(),
            listen_key: String::new(),
            websocket_url: "wss://ws-api.binance.com:443/ws-api/v3".to_string(),
            subscribe_request: Some(
                "{\"method\":\"userDataStream.subscribe.signature\"}".to_string(),
            ),
        };

        assert_eq!(super::user_stream_keepalive_key(&stream), None);
    }

    #[test]
    fn futures_listen_key_stream_keeps_keepalive() {
        let stream = BinanceUserDataStream {
            market: "usdm".to_string(),
            listen_key: "listen-key-123".to_string(),
            websocket_url: "wss://fstream.binance.com/ws/listen-key-123".to_string(),
            subscribe_request: None,
        };

        assert_eq!(
            super::user_stream_keepalive_key(&stream),
            Some("listen-key-123".to_string())
        );
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
        strategy
            .runtime
            .orders
            .push(shared_domain::strategy::StrategyRuntimeOrder {
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
        )
        .expect("apply effects");

        assert!(changed);
        let trades = db
            .list_exchange_trade_history("engine@example.com")
            .unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].trade_id, "123");
    }

    #[test]
    fn reconcile_updates_metrics_for_running_and_error_paused_strategies() {
        let db = SharedDb::ephemeral().expect("db");
        db.insert_strategy(&StoredStrategy {
            sequence_id: 1,
            strategy: strategy("run", StrategyStatus::Running),
        })
        .unwrap();
        db.insert_strategy(&StoredStrategy {
            sequence_id: 2,
            strategy: strategy("err", StrategyStatus::ErrorPaused),
        })
        .unwrap();
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
        })
        .unwrap();
        std::env::remove_var("TELEGRAM_BOT_TOKEN");
        let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
        let mut running = strategy("notify-tg-fail", StrategyStatus::Running);
        running.runtime.positions = vec![StrategyRuntimePosition {
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotClassic,
            quantity: Decimal::new(1, 0),
            average_entry_price: Decimal::new(100, 0),
        }];
        running.active_revision = Some(shared_domain::strategy::StrategyRevision {
            revision_id: "notify-revision".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
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
        db.insert_strategy(&StoredStrategy {
            sequence_id: 1,
            strategy: running,
        })
        .expect("strategy");
        db.enqueue_market_tick(&shared_events::MarketTick {
            symbol: "BTCUSDT".to_string(),
            market: "spot".to_string(),
            price: Decimal::new(101, 0),
            event_time_ms: 1_000,
        })
        .expect("tick");

        reconcile_once(&db, &metrics).expect("reconcile");

        let notifications = db
            .list_notification_logs("engine@example.com", 10)
            .expect("notifications");
        assert!(notifications
            .iter()
            .any(|record| record.channel == "telegram"
                && record.template_key.as_deref() == Some("OverallTakeProfitTriggered")
                && record.status == "failed"));
    }

    #[test]
    fn reconcile_persists_overall_take_profit_notification() {
        let db = SharedDb::ephemeral().expect("db");
        let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
        let mut running = strategy("notify-tp", StrategyStatus::Running);
        running.runtime.positions = vec![StrategyRuntimePosition {
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotClassic,
            quantity: Decimal::new(1, 0),
            average_entry_price: Decimal::new(100, 0),
        }];
        running.active_revision = Some(shared_domain::strategy::StrategyRevision {
            revision_id: "notify-revision".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
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
        db.insert_strategy(&StoredStrategy {
            sequence_id: 1,
            strategy: running,
        })
        .expect("strategy");
        db.enqueue_market_tick(&shared_events::MarketTick {
            symbol: "BTCUSDT".to_string(),
            market: "spot".to_string(),
            price: Decimal::new(101, 0),
            event_time_ms: 1_000,
        })
        .expect("tick");

        reconcile_once(&db, &metrics).expect("reconcile");

        let notifications = db
            .list_notification_logs("engine@example.com", 10)
            .expect("notifications");
        assert!(notifications
            .iter()
            .any(|record| record.template_key.as_deref() == Some("OverallTakeProfitTriggered")));
    }

    #[test]
    fn reconcile_moves_overall_take_profit_into_stopping_without_local_flattening() {
        let db = SharedDb::ephemeral().expect("db");
        let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
        let mut running = strategy("stopping-tp", StrategyStatus::Running);
        running.runtime.positions = vec![StrategyRuntimePosition {
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotClassic,
            quantity: Decimal::new(1, 0),
            average_entry_price: Decimal::new(100, 0),
        }];
        running.runtime.orders = vec![shared_domain::strategy::StrategyRuntimeOrder {
            order_id: "stopping-tp-tp-0".to_string(),
            exchange_order_id: Some("live-tp-1".to_string()),
            level_index: Some(0),
            side: "Sell".to_string(),
            order_type: "Limit".to_string(),
            price: Some(Decimal::new(101, 0)),
            quantity: Decimal::new(1, 0),
            status: "Placed".to_string(),
        }];
        running.active_revision = Some(shared_domain::strategy::StrategyRevision {
            revision_id: "notify-revision".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
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
        db.insert_strategy(&StoredStrategy {
            sequence_id: 1,
            strategy: running,
        })
        .expect("strategy");
        db.enqueue_market_tick(&shared_events::MarketTick {
            symbol: "BTCUSDT".to_string(),
            market: "spot".to_string(),
            price: Decimal::new(101, 0),
            event_time_ms: 1_000,
        })
        .expect("tick");

        reconcile_once(&db, &metrics).expect("reconcile");

        let stored = db
            .find_strategy("engine@example.com", "stopping-tp")
            .expect("find")
            .expect("strategy");
        assert_eq!(stored.status, StrategyStatus::Stopping);
        assert_eq!(stored.runtime.positions.len(), 1);
        assert!(stored.runtime.fills.is_empty());
        assert!(stored
            .runtime
            .events
            .iter()
            .any(|event| event.event_type == "overall_take_profit_stop"));
    }

    #[test]
    fn reconcile_moves_overall_take_profit_into_stopping_for_short_position_with_buy_exit() {
        let db = SharedDb::ephemeral().expect("db");
        let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
        let mut running = strategy("stopping-tp-short", StrategyStatus::Running);
        running.mode = StrategyMode::FuturesShort;
        running.runtime.positions = vec![StrategyRuntimePosition {
            market: StrategyMarket::FuturesUsdM,
            mode: StrategyMode::FuturesShort,
            quantity: Decimal::new(1, 0),
            average_entry_price: Decimal::new(100, 0),
        }];
        running.runtime.orders = vec![shared_domain::strategy::StrategyRuntimeOrder {
            order_id: "stopping-tp-short-tp-0".to_string(),
            exchange_order_id: Some("live-tp-short-1".to_string()),
            level_index: Some(0),
            side: "Buy".to_string(),
            order_type: "Limit".to_string(),
            price: Some(Decimal::new(99, 0)),
            quantity: Decimal::new(1, 0),
            status: "Placed".to_string(),
        }];
        running.active_revision = Some(shared_domain::strategy::StrategyRevision {
            revision_id: "short-revision".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
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
        db.insert_strategy(&StoredStrategy {
            sequence_id: 1,
            strategy: running,
        })
        .expect("strategy");
        db.enqueue_market_tick(&shared_events::MarketTick {
            symbol: "BTCUSDT".to_string(),
            market: "spot".to_string(),
            price: Decimal::new(99, 0),
            event_time_ms: 1_000,
        })
        .expect("tick");

        reconcile_once(&db, &metrics).expect("reconcile");

        let stored = db
            .find_strategy("engine@example.com", "stopping-tp-short")
            .expect("find")
            .expect("strategy");
        assert_eq!(stored.status, StrategyStatus::Stopping);
        assert_eq!(stored.runtime.positions.len(), 1);
        assert!(stored
            .runtime
            .events
            .iter()
            .any(|event| event.event_type == "overall_take_profit_stop"));
    }

    #[test]
    fn reconcile_failure_auto_pauses_and_emits_runtime_error_notification() {
        let db = SharedDb::ephemeral().expect("db");
        let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
        let mut broken = strategy("broken", StrategyStatus::Running);
        broken.active_revision = Some(shared_domain::strategy::StrategyRevision {
            revision_id: "broken-revision".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
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
        db.insert_strategy(&StoredStrategy {
            sequence_id: 1,
            strategy: broken,
        })
        .expect("strategy");

        reconcile_once(&db, &metrics).expect("reconcile");

        let stored = db
            .find_strategy("engine@example.com", "broken")
            .expect("find")
            .expect("strategy");
        assert_eq!(stored.status, StrategyStatus::ErrorPaused);
        let notifications = db
            .list_notification_logs("engine@example.com", 10)
            .expect("notifications");
        assert!(notifications
            .iter()
            .any(|record| record.template_key.as_deref() == Some("RuntimeError")));
    }

    #[test]
    fn live_order_sync_failure_auto_pauses_and_emits_runtime_error_notification() {
        let db = SharedDb::ephemeral().expect("db");
        let mut running = strategy("live-sync-fail", StrategyStatus::Running);
        let created_at = Utc::now();

        let changed =
            apply_live_sync_failure(&db, &mut running, 2, created_at).expect("live sync failure");

        assert!(changed);
        assert_eq!(running.status, StrategyStatus::ErrorPaused);
        assert!(running
            .runtime
            .events
            .iter()
            .any(|event| event.event_type == "live_order_sync_failed"));
        let notifications = db
            .list_notification_logs("engine@example.com", 10)
            .expect("notifications");
        assert!(notifications
            .iter()
            .any(|record| record.template_key.as_deref() == Some("RuntimeError")));
    }

    #[test]
    fn reconcile_consumes_market_ticks_for_running_strategy() {
        let db = SharedDb::ephemeral().expect("db");
        let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
        let mut running = strategy("tick", StrategyStatus::Running);
        running.runtime.positions = vec![StrategyRuntimePosition {
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotClassic,
            quantity: Decimal::new(1, 0),
            average_entry_price: Decimal::new(100, 0),
        }];
        running.active_revision = Some(shared_domain::strategy::StrategyRevision {
            revision_id: "tick-revision".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
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
        db.insert_strategy(&StoredStrategy {
            sequence_id: 1,
            strategy: running,
        })
        .expect("strategy");
        db.enqueue_market_tick(&shared_events::MarketTick {
            symbol: "BTCUSDT".to_string(),
            market: "spot".to_string(),
            price: Decimal::new(101, 0),
            event_time_ms: 1_000,
        })
        .expect("tick");

        reconcile_once(&db, &metrics).expect("reconcile");

        let stored = db
            .find_strategy("engine@example.com", "tick")
            .expect("find")
            .expect("strategy");
        assert!(stored
            .runtime
            .events
            .iter()
            .any(|event| event.event_type.contains("overall_take_profit")));
    }

    #[test]
    fn martingale_production_start_uses_futures_preflight_entrypoint() {
        let mut running = strategy("martingale-live", StrategyStatus::Running);
        running.strategy_type = StrategyType::MartingaleGrid;
        running.market = StrategyMarket::FuturesUsdM;
        running.mode = StrategyMode::FuturesLong;
        running.draft_revision.strategy_type = StrategyType::MartingaleGrid;
        running.draft_revision.futures_margin_mode =
            Some(shared_domain::strategy::FuturesMarginMode::Cross);
        running.draft_revision.leverage = Some(3);
        running.draft_revision.reference_price = Some(Decimal::new(100, 0));
        running.active_revision = Some(running.draft_revision.clone());

        struct FakeSettingsSource {
            settings: trading_engine::martingale_runtime::FuturesExchangeSettings,
        }

        impl super::MartingaleExchangeSettingsSource for FakeSettingsSource {
            fn martingale_futures_settings(
                &self,
                _strategy: &Strategy,
            ) -> Result<
                trading_engine::martingale_runtime::FuturesExchangeSettings,
                shared_db::SharedDbError,
            > {
                Ok(self.settings.clone())
            }
        }

        let rejected = super::sync_martingale_production_start_from_exchange(
            &mut running.clone(),
            &FakeSettingsSource {
                settings: trading_engine::martingale_runtime::FuturesExchangeSettings {
                    hedge_mode: true,
                    symbols: HashMap::from([(
                        "BTCUSDT".to_string(),
                        trading_engine::martingale_runtime::FuturesSymbolSettings {
                            margin_mode: shared_domain::martingale::MartingaleMarginMode::Isolated,
                            leverage: 5,
                        },
                    )]),
                },
            },
        )
        .expect_err("production adapter must reject failed futures preflight");
        assert!(
            rejected.to_string().contains("margin mode")
                || rejected.to_string().contains("leverage")
        );

        let changed = super::sync_martingale_production_start_from_exchange(
            &mut running,
            &FakeSettingsSource {
                settings: trading_engine::martingale_runtime::FuturesExchangeSettings {
                    hedge_mode: true,
                    symbols: HashMap::from([(
                        "BTCUSDT".to_string(),
                        trading_engine::martingale_runtime::FuturesSymbolSettings {
                            margin_mode: shared_domain::martingale::MartingaleMarginMode::Cross,
                            leverage: 3,
                        },
                    )]),
                },
            },
        )
        .expect("production adapter should start through preflight entrypoint");

        assert!(changed);
        assert!(running
            .runtime
            .orders
            .iter()
            .any(|order| order.order_id.starts_with("mg-")));
        assert!(running
            .runtime
            .events
            .iter()
            .any(|event| event.event_type == "martingale_runtime_started"));
    }

    #[test]
    fn martingale_portfolio_config_maps_backtest_parameters_to_runtime() {
        let now = Utc::now();
        let portfolio_config = serde_json::json!({
            "portfolio_config": {
                "direction_mode": "long_and_short",
                "risk_limits": {},
                "strategies": [
                    {
                        "strategy_id": "btc-long",
                        "symbol": "BTCUSDT",
                        "market": "usd_m_futures",
                        "direction": "long",
                        "direction_mode": "long_and_short",
                        "margin_mode": "isolated",
                        "leverage": 3,
                        "spacing": { "fixed_percent": { "step_bps": 100 } },
                        "sizing": { "multiplier": { "first_order_quote": "10", "multiplier": "2", "max_legs": 4 } },
                        "take_profit": { "percent": { "bps": 80 } },
                        "stop_loss": { "strategy_drawdown_pct": { "pct_bps": 2000 } },
                        "indicators": [],
                        "entry_triggers": [],
                        "risk_limits": {}
                    },
                    {
                        "strategy_id": "btc-short",
                        "symbol": "BTCUSDT",
                        "market": "usd_m_futures",
                        "direction": "short",
                        "direction_mode": "long_and_short",
                        "margin_mode": "isolated",
                        "leverage": 3,
                        "spacing": { "fixed_percent": { "step_bps": 140 } },
                        "sizing": { "budget_scaled": { "first_order_quote": "8", "multiplier": "1.6", "max_legs": 3, "max_budget_quote": "60" } },
                        "take_profit": { "percent": { "bps": 100 } },
                        "stop_loss": { "strategy_drawdown_pct": { "pct_bps": 1800 } },
                        "indicators": [],
                        "entry_triggers": [],
                        "risk_limits": {}
                    }
                ]
            }
        });
        let portfolio = MartingalePortfolioRecord {
            portfolio_id: "mp-test".to_owned(),
            owner: "engine@example.com".to_owned(),
            name: "test".to_owned(),
            status: "running".to_owned(),
            source_task_id: "task".to_owned(),
            market: "usd_m_futures".to_owned(),
            direction: "long_short".to_owned(),
            risk_profile: "balanced".to_owned(),
            total_weight_pct: Decimal::new(100, 0),
            config: portfolio_config.clone(),
            risk_summary: serde_json::json!({}),
            created_at: now,
            updated_at: now,
            items: vec![MartingalePortfolioItemRecord {
                strategy_instance_id: "msi-test".to_owned(),
                portfolio_id: "mp-test".to_owned(),
                candidate_id: "bc-test".to_owned(),
                symbol: "BTCUSDT".to_owned(),
                weight_pct: Decimal::new(100, 0),
                leverage: 3,
                enabled: true,
                status: "running".to_owned(),
                parameter_snapshot: portfolio_config,
                metrics_snapshot: serde_json::json!({}),
                created_at: now,
                updated_at: now,
            }],
        };

        let config = super::martingale_runtime_config_from_portfolio(&portfolio)
            .expect("portfolio config should map to runtime");
        assert_eq!(config.portfolio.strategies.len(), 2);
        assert_eq!(config.portfolio_budget_quote, Decimal::new(6376, 2));

        let mut runtime = trading_engine::martingale_runtime::MartingaleRuntime::new(config)
            .expect("runtime should accept mapped config");
        let settings = trading_engine::martingale_runtime::FuturesExchangeSettings {
            hedge_mode: true,
            symbols: HashMap::from([(
                "BTCUSDT".to_owned(),
                trading_engine::martingale_runtime::FuturesSymbolSettings {
                    margin_mode: shared_domain::martingale::MartingaleMarginMode::Isolated,
                    leverage: 3,
                },
            )]),
        };
        runtime
            .start_cycle_with_futures_preflight(
                &settings,
                "btc-long",
                Decimal::new(100, 0),
                trading_engine::martingale_runtime::MartingaleRuntimeContext::default(),
            )
            .expect("long side should start");
        runtime
            .start_cycle(
                "btc-short",
                Decimal::new(100, 0),
                trading_engine::martingale_runtime::MartingaleRuntimeContext::default(),
            )
            .expect("short side should start after preflight");

        assert!(runtime.orders().iter().any(|order| {
            order.strategy_id == "btc-long"
                && order.side == "BUY"
                && order.notional_quote == Decimal::new(10, 0)
        }));
        assert!(runtime.orders().iter().any(|order| {
            order.strategy_id == "btc-short"
                && order.side == "SELL"
                && order.notional_quote == Decimal::new(8, 0)
        }));
    }

    #[test]
    fn martingale_portfolio_executor_strategy_preserves_live_order_sync_path() {
        let now = Utc::now();
        let portfolio = MartingalePortfolioRecord {
            portfolio_id: "mp-executor".to_owned(),
            owner: "engine@example.com".to_owned(),
            name: "executor portfolio".to_owned(),
            status: "running".to_owned(),
            source_task_id: "task".to_owned(),
            market: "usd_m_futures".to_owned(),
            direction: "long_short".to_owned(),
            risk_profile: "balanced".to_owned(),
            total_weight_pct: Decimal::new(100, 0),
            config: serde_json::json!({}),
            risk_summary: serde_json::json!({}),
            created_at: now,
            updated_at: now,
            items: Vec::new(),
        };
        let strategy_config = serde_json::from_value::<shared_domain::martingale::MartingaleStrategyConfig>(
            serde_json::json!({
                "strategy_id": "staged-executor-long",
                "symbol": "BTCUSDT",
                "market": "usd_m_futures",
                "direction": "long",
                "direction_mode": "long_and_short",
                "margin_mode": "isolated",
                "leverage": 3,
                "spacing": { "fixed_percent": { "step_bps": 100 } },
                "sizing": { "multiplier": { "first_order_quote": "10", "multiplier": "2", "max_legs": 3 } },
                "take_profit": { "percent": { "bps": 80 } },
                "stop_loss": { "strategy_drawdown_pct": { "pct_bps": 2000 } },
                "indicators": [],
                "entry_triggers": [],
                "risk_limits": {}
            }),
        )
        .expect("strategy config");
        let mut runtime = trading_engine::martingale_runtime::MartingaleRuntime::new(
            trading_engine::martingale_runtime::MartingaleRuntimeConfig {
                portfolio_id: portfolio.portfolio_id.clone(),
                strategy_instance_id: "portfolio".to_owned(),
                portfolio: shared_domain::martingale::MartingalePortfolioConfig {
                    direction_mode:
                        shared_domain::martingale::MartingaleDirectionMode::LongAndShort,
                    strategies: vec![strategy_config.clone()],
                    risk_limits: shared_domain::martingale::MartingaleRiskLimits::default(),
                },
                portfolio_budget_quote: Decimal::new(70, 0),
                exchange_min_notional: Decimal::ZERO,
            },
        )
        .expect("runtime");
        let settings = trading_engine::martingale_runtime::FuturesExchangeSettings {
            hedge_mode: true,
            symbols: HashMap::from([(
                "BTCUSDT".to_owned(),
                trading_engine::martingale_runtime::FuturesSymbolSettings {
                    margin_mode: shared_domain::martingale::MartingaleMarginMode::Isolated,
                    leverage: 3,
                },
            )]),
        };
        runtime
            .start_cycle_with_futures_preflight(
                &settings,
                "staged-executor-long",
                Decimal::new(100, 0),
                trading_engine::martingale_runtime::MartingaleRuntimeContext::default(),
            )
            .expect("cycle");
        let runtime_orders = super::martingale_runtime_orders_to_strategy_orders(runtime.orders());
        let executor = super::executor_strategy_from_martingale_config(
            &portfolio.owner,
            &portfolio,
            &strategy_config,
            Decimal::new(100, 0),
            runtime_orders,
        )
        .expect("executor strategy");

        assert_eq!(executor.id, "staged-executor-long");
        assert_eq!(executor.strategy_type, StrategyType::MartingaleGrid);
        assert_eq!(executor.market, StrategyMarket::FuturesUsdM);
        assert_eq!(executor.mode, StrategyMode::FuturesLong);
        assert_eq!(executor.source_template_id.as_deref(), Some("mp-executor"));
        assert_eq!(executor.runtime.orders.len(), 1);
        assert!(executor.runtime.orders[0].order_id.starts_with("mg-"));
        assert!(executor.runtime.orders[0].order_id.contains("-leg-"));
        assert!(executor.runtime.orders[0].order_id.len() <= 36);

        let restored = super::martingale_runtime_config_from_strategy(&executor)
            .expect("executor should preserve original martingale config")
            .portfolio
            .strategies
            .remove(0);
        assert_eq!(restored.spacing, strategy_config.spacing);
        assert_eq!(restored.sizing, strategy_config.sizing);
        assert_eq!(restored.take_profit, strategy_config.take_profit);
        assert_eq!(restored.stop_loss, strategy_config.stop_loss);
        assert_eq!(restored.entry_triggers, strategy_config.entry_triggers);
    }

    #[test]
    fn martingale_percent_take_profit_requests_market_close() {
        let mut running = strategy("martingale-tp", StrategyStatus::Running);
        running.strategy_type = StrategyType::MartingaleGrid;
        running.market = StrategyMarket::FuturesUsdM;
        running.mode = StrategyMode::FuturesLong;
        running.runtime.positions.push(StrategyRuntimePosition {
            market: StrategyMarket::FuturesUsdM,
            mode: StrategyMode::FuturesLong,
            quantity: Decimal::new(1, 0),
            average_entry_price: Decimal::new(100, 0),
        });
        let config = super::martingale_runtime_config_from_strategy(&running)
            .expect("strategy config")
            .portfolio
            .strategies
            .remove(0);
        let signal = super::martingale_exit_signal(
            &config,
            &running.runtime.positions[0],
            Decimal::new(102, 0),
            Decimal::ZERO,
            Decimal::ZERO,
        )
        .expect("tp should trigger");
        assert_eq!(signal.event_type, "martingale_take_profit_stop");

        let position = running.runtime.positions[0].clone();
        super::request_martingale_close(&mut running, &position, Decimal::new(102, 0), &signal);

        assert!(running.runtime.orders.iter().any(|order| {
            order.status == "ClosingRequested"
                && order.side == "Sell"
                && order.order_type == "Market"
        }));
    }

    #[test]
    fn martingale_executor_notes_restore_atr_indicator_and_cooldown_config() {
        let mut executor = strategy("staged-executor-atr", StrategyStatus::Running);
        executor.strategy_type = StrategyType::MartingaleGrid;
        executor.market = StrategyMarket::FuturesUsdM;
        executor.mode = StrategyMode::FuturesLong;
        executor.source_template_id = Some("mp-lp".to_owned());
        executor.active_revision = Some(shared_domain::strategy::StrategyRevision {
            revision_id: "staged-executor-atr-rev".to_owned(),
            version: 1,
            strategy_type: StrategyType::MartingaleGrid,
            generation: GridGeneration::Custom,
            levels: Vec::new(),
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: Some(shared_domain::strategy::FuturesMarginMode::Isolated),
            leverage: Some(10),
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: Some(Decimal::new(100, 0)),
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        });
        let original = serde_json::from_value::<shared_domain::martingale::MartingaleStrategyConfig>(
            serde_json::json!({
                "strategy_id": "staged-executor-atr",
                "symbol": "BTCUSDT",
                "market": "usd_m_futures",
                "direction": "long",
                "direction_mode": "long_and_short",
                "margin_mode": "isolated",
                "leverage": 10,
                "spacing": { "atr": { "multiplier": "2", "min_step_bps": 0, "max_step_bps": 30000 } },
                "sizing": { "multiplier": { "first_order_quote": "50", "multiplier": "2.4", "max_legs": 9 } },
                "take_profit": { "percent": { "bps": 300 } },
                "stop_loss": { "strategy_drawdown_pct": { "pct_bps": 600 } },
                "indicators": [{ "atr": { "period": 21 } }, { "adx": { "period": 14 } }],
                "entry_triggers": [
                    { "cooldown": { "seconds": 21600 } },
                    { "indicator_expression": { "expression": "adx(14) > 30" } }
                ],
                "risk_limits": {}
            }),
        )
        .expect("martingale strategy config");
        executor.notes =
            super::notes_with_martingale_config("executor".to_owned(), &original).expect("notes");

        let restored = super::martingale_runtime_config_from_strategy(&executor)
            .expect("runtime config")
            .portfolio
            .strategies
            .remove(0);

        assert_eq!(restored.spacing, original.spacing);
        assert_eq!(restored.sizing, original.sizing);
        assert_eq!(restored.take_profit, original.take_profit);
        assert_eq!(restored.stop_loss, original.stop_loss);
        assert_eq!(restored.indicators, original.indicators);
        assert_eq!(restored.entry_triggers, original.entry_triggers);
    }

    #[test]
    fn martingale_runtime_cooldown_blocks_live_reentry_until_elapsed() {
        let config = serde_json::from_value::<shared_domain::martingale::MartingaleStrategyConfig>(
            serde_json::json!({
                "strategy_id": "cooldown-live",
                "symbol": "BTCUSDT",
                "market": "usd_m_futures",
                "direction": "long",
                "direction_mode": "long_only",
                "margin_mode": "isolated",
                "leverage": 3,
                "spacing": { "fixed_percent": { "step_bps": 100 } },
                "sizing": { "multiplier": { "first_order_quote": "10", "multiplier": "2", "max_legs": 3 } },
                "take_profit": { "percent": { "bps": 100 } },
                "stop_loss": { "strategy_drawdown_pct": { "pct_bps": 2000 } },
                "indicators": [],
                "entry_triggers": [{ "cooldown": { "seconds": 60 } }],
                "risk_limits": {}
            }),
        )
        .expect("strategy config");
        let settings = trading_engine::martingale_runtime::FuturesExchangeSettings {
            hedge_mode: false,
            symbols: HashMap::from([(
                "BTCUSDT".to_owned(),
                trading_engine::martingale_runtime::FuturesSymbolSettings {
                    margin_mode: shared_domain::martingale::MartingaleMarginMode::Isolated,
                    leverage: 3,
                },
            )]),
        };
        let runtime_config = trading_engine::martingale_runtime::MartingaleRuntimeConfig {
            portfolio_id: "mp-cooldown".to_owned(),
            strategy_instance_id: "portfolio".to_owned(),
            portfolio: shared_domain::martingale::MartingalePortfolioConfig {
                direction_mode: shared_domain::martingale::MartingaleDirectionMode::LongOnly,
                strategies: vec![config],
                risk_limits: shared_domain::martingale::MartingaleRiskLimits::default(),
            },
            portfolio_budget_quote: Decimal::new(70, 0),
            exchange_min_notional: Decimal::ZERO,
        };

        let mut blocked =
            trading_engine::martingale_runtime::MartingaleRuntime::new(runtime_config.clone())
                .expect("runtime");
        let blocked_result = blocked.start_cycle_with_futures_preflight(
            &settings,
            "cooldown-live",
            Decimal::new(100, 0),
            trading_engine::martingale_runtime::MartingaleRuntimeContext {
                now_ms: Some(59_000),
                last_cycle_closed_at_ms: Some(0),
                ..Default::default()
            },
        );
        assert!(blocked_result
            .expect_err("cooldown should block")
            .to_string()
            .contains("entry triggers"));

        let mut allowed =
            trading_engine::martingale_runtime::MartingaleRuntime::new(runtime_config)
                .expect("runtime");
        allowed
            .start_cycle_with_futures_preflight(
                &settings,
                "cooldown-live",
                Decimal::new(100, 0),
                trading_engine::martingale_runtime::MartingaleRuntimeContext {
                    now_ms: Some(60_000),
                    last_cycle_closed_at_ms: Some(0),
                    ..Default::default()
                },
            )
            .expect("cooldown should allow after elapsed window");
    }

    #[test]
    fn martingale_strategy_drawdown_requests_market_close() {
        let mut running = strategy("martingale-sl", StrategyStatus::Running);
        running.strategy_type = StrategyType::MartingaleGrid;
        running.market = StrategyMarket::FuturesUsdM;
        running.mode = StrategyMode::FuturesLong;
        running.runtime.positions.push(StrategyRuntimePosition {
            market: StrategyMarket::FuturesUsdM,
            mode: StrategyMode::FuturesLong,
            quantity: Decimal::new(1, 0),
            average_entry_price: Decimal::new(100, 0),
        });
        let mut config = super::martingale_runtime_config_from_strategy(&running)
            .expect("strategy config")
            .portfolio
            .strategies
            .remove(0);
        config.take_profit =
            shared_domain::martingale::MartingaleTakeProfitModel::Percent { bps: 9_000 };
        config.stop_loss = Some(
            shared_domain::martingale::MartingaleStopLossModel::StrategyDrawdownPct {
                pct_bps: 2_000,
            },
        );

        let signal = super::martingale_exit_signal(
            &config,
            &running.runtime.positions[0],
            Decimal::new(79, 0),
            Decimal::ZERO,
            Decimal::ZERO,
        )
        .expect("strategy drawdown should trigger");
        assert_eq!(signal.event_type, "martingale_strategy_drawdown_stop");

        let position = running.runtime.positions[0].clone();
        super::request_martingale_close(&mut running, &position, Decimal::new(79, 0), &signal);
        assert!(running.runtime.orders.iter().any(|order| {
            order.status == "ClosingRequested"
                && order.side == "Sell"
                && order.order_type == "Market"
        }));
    }

    #[test]
    fn martingale_executor_reconcile_generates_next_safety_leg_after_fill() {
        let db = SharedDb::ephemeral().expect("db");
        let now = Utc::now();
        let portfolio_config = serde_json::json!({
            "portfolio_config": {
                "direction_mode": "long_only",
                "risk_limits": {},
                "strategies": [
                    {
                        "strategy_id": "staged-reconcile-long",
                        "symbol": "BTCUSDT",
                        "market": "usd_m_futures",
                        "direction": "long",
                        "direction_mode": "long_only",
                        "margin_mode": "isolated",
                        "leverage": 3,
                        "spacing": { "fixed_percent": { "step_bps": 100 } },
                        "sizing": { "multiplier": { "first_order_quote": "11", "multiplier": "1", "max_legs": 2 } },
                        "take_profit": { "percent": { "bps": 80 } },
                        "stop_loss": { "strategy_drawdown_pct": { "pct_bps": 2000 } },
                        "indicators": [],
                        "entry_triggers": [],
                        "risk_limits": {}
                    }
                ]
            }
        });
        let portfolio = MartingalePortfolioRecord {
            portfolio_id: "mp-reconcile".to_owned(),
            owner: "engine@example.com".to_owned(),
            name: "reconcile portfolio".to_owned(),
            status: "running".to_owned(),
            source_task_id: "task".to_owned(),
            market: "usd_m_futures".to_owned(),
            direction: "long".to_owned(),
            risk_profile: "balanced".to_owned(),
            total_weight_pct: Decimal::new(100, 0),
            config: portfolio_config.clone(),
            risk_summary: serde_json::json!({}),
            created_at: now,
            updated_at: now,
            items: vec![MartingalePortfolioItemRecord {
                strategy_instance_id: "msi-reconcile".to_owned(),
                portfolio_id: "mp-reconcile".to_owned(),
                candidate_id: "bc-reconcile".to_owned(),
                symbol: "BTCUSDT".to_owned(),
                weight_pct: Decimal::new(100, 0),
                leverage: 3,
                enabled: true,
                status: "running".to_owned(),
                parameter_snapshot: portfolio_config,
                metrics_snapshot: serde_json::json!({}),
                created_at: now,
                updated_at: now,
            }],
        };
        let config = super::martingale_runtime_config_from_portfolio(&portfolio)
            .expect("portfolio config should map to runtime");
        let strategy_config = config.portfolio.strategies[0].clone();
        let settings =
            super::futures_settings_from_portfolio(&portfolio).expect("futures settings");
        let mut runtime =
            trading_engine::martingale_runtime::MartingaleRuntime::new(config).expect("runtime");
        runtime
            .start_cycle_with_futures_preflight(
                &settings,
                "staged-reconcile-long",
                Decimal::new(100, 0),
                trading_engine::martingale_runtime::MartingaleRuntimeContext::default(),
            )
            .expect("cycle");
        let mut runtime_orders =
            super::martingale_runtime_orders_to_strategy_orders(runtime.orders());
        assert_eq!(runtime_orders.len(), 1);
        runtime_orders[0].status = "Filled".to_owned();
        let first_leg_id = runtime_orders[0].order_id.clone();
        let executor = super::executor_strategy_from_martingale_config(
            &portfolio.owner,
            &portfolio,
            &strategy_config,
            Decimal::new(100, 0),
            runtime_orders,
        )
        .expect("executor strategy");
        db.insert_strategy(&StoredStrategy {
            sequence_id: 1,
            strategy: executor,
        })
        .expect("strategy");

        super::reconcile_martingale_executor_strategies(
            &db,
            &portfolio,
            &backtest_engine::martingale::indicator_runtime::IndicatorRuntimeContext::default(),
            &[],
        )
        .expect("reconcile");

        let stored = db
            .find_strategy("engine@example.com", "staged-reconcile-long")
            .expect("find")
            .expect("strategy");
        assert!(stored
            .runtime
            .orders
            .iter()
            .any(|order| order.order_id == first_leg_id && order.status == "Filled"));
        let safety_leg = stored
            .runtime
            .orders
            .iter()
            .find(|order| order.level_index == Some(1))
            .expect("next safety leg");
        assert_eq!(safety_leg.status, "Working");
        assert_eq!(safety_leg.side, "BUY");
        assert!(safety_leg.price.expect("price") < Decimal::new(100, 0));
        assert!(stored
            .runtime
            .events
            .iter()
            .any(|event| event.event_type == "martingale_safety_legs_generated"));
    }

    #[test]
    fn martingale_portfolio_weight_caps_budget_without_scaling_order_size() {
        let now = Utc::now();
        let portfolio_config = serde_json::json!({
            "portfolio_config": {
                "direction_mode": "long_only",
                "risk_limits": { "max_global_budget_quote": "100" },
                "strategies": [
                    {
                        "strategy_id": "weighted-btc",
                        "symbol": "BTCUSDT",
                        "market": "usd_m_futures",
                        "direction": "long",
                        "direction_mode": "long_only",
                        "margin_mode": "isolated",
                        "leverage": 3,
                        "portfolio_weight_pct": "25",
                        "spacing": { "fixed_percent": { "step_bps": 100 } },
                        "sizing": { "multiplier": { "first_order_quote": "11", "multiplier": "2", "max_legs": 4 } },
                        "take_profit": { "percent": { "bps": 80 } },
                        "stop_loss": { "strategy_drawdown_pct": { "pct_bps": 2000 } },
                        "indicators": [],
                        "entry_triggers": [],
                        "risk_limits": {}
                    }
                ]
            }
        });
        let portfolio = MartingalePortfolioRecord {
            portfolio_id: "mp-weighted".to_owned(),
            owner: "engine@example.com".to_owned(),
            name: "weighted".to_owned(),
            status: "running".to_owned(),
            source_task_id: "task".to_owned(),
            market: "usd_m_futures".to_owned(),
            direction: "long".to_owned(),
            risk_profile: "conservative".to_owned(),
            total_weight_pct: Decimal::new(100, 0),
            config: portfolio_config.clone(),
            risk_summary: serde_json::json!({}),
            created_at: now,
            updated_at: now,
            items: vec![MartingalePortfolioItemRecord {
                strategy_instance_id: "msi-weighted".to_owned(),
                portfolio_id: "mp-weighted".to_owned(),
                candidate_id: "bc-weighted".to_owned(),
                symbol: "BTCUSDT".to_owned(),
                weight_pct: Decimal::new(25, 0),
                leverage: 3,
                enabled: true,
                status: "running".to_owned(),
                parameter_snapshot: portfolio_config.clone(),
                metrics_snapshot: serde_json::json!({}),
                created_at: now,
                updated_at: now,
            }],
        };

        let config = super::martingale_runtime_config_from_portfolio(&portfolio)
            .expect("weighted config should map to runtime");
        assert_eq!(config.portfolio_budget_quote, Decimal::new(100, 0));
        let weighted_strategy = &config.portfolio.strategies[0];
        assert_eq!(
            super::strategy_planned_budget_quote(weighted_strategy),
            Some(Decimal::new(55, 0))
        );
        assert_eq!(
            weighted_strategy.risk_limits.max_strategy_budget_quote,
            Some(Decimal::new(25, 0))
        );
        let mut runtime = trading_engine::martingale_runtime::MartingaleRuntime::new(config)
            .expect("runtime should accept weighted config");
        let settings = trading_engine::martingale_runtime::FuturesExchangeSettings {
            hedge_mode: false,
            symbols: HashMap::from([(
                "BTCUSDT".to_owned(),
                trading_engine::martingale_runtime::FuturesSymbolSettings {
                    margin_mode: shared_domain::martingale::MartingaleMarginMode::Isolated,
                    leverage: 3,
                },
            )]),
        };
        runtime
            .start_cycle_with_futures_preflight(
                &settings,
                "weighted-btc",
                Decimal::new(100, 0),
                trading_engine::martingale_runtime::MartingaleRuntimeContext::default(),
            )
            .expect("weighted strategy should start");

        assert_eq!(runtime.orders()[0].notional_quote, Decimal::new(11, 0));
    }

    #[test]
    fn martingale_weight_cap_keeps_first_order_when_cap_is_below_first_order() {
        let now = Utc::now();
        let portfolio_config = serde_json::json!({
            "portfolio_config": {
                "direction_mode": "long_only",
                "risk_limits": { "max_global_budget_quote": "100" },
                "strategies": [
                    {
                        "strategy_id": "tiny-weight-btc",
                        "symbol": "BTCUSDT",
                        "market": "usd_m_futures",
                        "direction": "long",
                        "direction_mode": "long_only",
                        "margin_mode": "isolated",
                        "leverage": 3,
                        "portfolio_weight_pct": "5",
                        "spacing": { "fixed_percent": { "step_bps": 100 } },
                        "sizing": { "multiplier": { "first_order_quote": "11", "multiplier": "2", "max_legs": 4 } },
                        "take_profit": { "percent": { "bps": 80 } },
                        "stop_loss": { "strategy_drawdown_pct": { "pct_bps": 2000 } },
                        "indicators": [],
                        "entry_triggers": [],
                        "risk_limits": {}
                    }
                ]
            }
        });
        let portfolio = MartingalePortfolioRecord {
            portfolio_id: "mp-tiny-weight".to_owned(),
            owner: "engine@example.com".to_owned(),
            name: "tiny weighted".to_owned(),
            status: "running".to_owned(),
            source_task_id: "task".to_owned(),
            market: "usd_m_futures".to_owned(),
            direction: "long".to_owned(),
            risk_profile: "conservative".to_owned(),
            total_weight_pct: Decimal::new(100, 0),
            config: portfolio_config.clone(),
            risk_summary: serde_json::json!({}),
            created_at: now,
            updated_at: now,
            items: vec![MartingalePortfolioItemRecord {
                strategy_instance_id: "msi-tiny-weight".to_owned(),
                portfolio_id: "mp-tiny-weight".to_owned(),
                candidate_id: "bc-tiny-weight".to_owned(),
                symbol: "BTCUSDT".to_owned(),
                weight_pct: Decimal::new(5, 0),
                leverage: 3,
                enabled: true,
                status: "running".to_owned(),
                parameter_snapshot: portfolio_config.clone(),
                metrics_snapshot: serde_json::json!({}),
                created_at: now,
                updated_at: now,
            }],
        };

        let config = super::martingale_runtime_config_from_portfolio(&portfolio)
            .expect("tiny weighted config should map to runtime");
        let weighted_strategy = &config.portfolio.strategies[0];
        assert_eq!(
            super::strategy_planned_budget_quote(weighted_strategy),
            Some(Decimal::new(55, 0))
        );
        // Cap is MARGIN units: global_budget(100) * weight_factor(0.05) = 5,
        // floored at the first leg's MARGIN (foq 11 / leverage 3 ~= 3.67), so
        // effective = max(5, 3.67) = 5 — NOT the first leg's NOTIONAL (11).
        assert_eq!(
            weighted_strategy.risk_limits.max_strategy_budget_quote,
            Some(Decimal::new(5, 0))
        );
        let mut runtime = trading_engine::martingale_runtime::MartingaleRuntime::new(config)
            .expect("runtime should accept tiny weighted config");
        let settings = trading_engine::martingale_runtime::FuturesExchangeSettings {
            hedge_mode: false,
            symbols: HashMap::from([(
                "BTCUSDT".to_owned(),
                trading_engine::martingale_runtime::FuturesSymbolSettings {
                    margin_mode: shared_domain::martingale::MartingaleMarginMode::Isolated,
                    leverage: 3,
                },
            )]),
        };
        runtime
            .start_cycle_with_futures_preflight(
                &settings,
                "tiny-weight-btc",
                Decimal::new(100, 0),
                trading_engine::martingale_runtime::MartingaleRuntimeContext::default(),
            )
            .expect("first order should not be scaled below exchange minimum");

        assert_eq!(runtime.orders()[0].notional_quote, Decimal::new(11, 0));
    }

    #[test]
    fn budget_blocked_safety_leg_not_persisted_as_working_order() {
        // Strategy: leverage 3, first_order 10, multiplier 2, max_legs 2.
        // Leg notionals [10, 20]; margins [10/3, 20/3] (~3.33, 6.67).
        // max_strategy_budget_quote = 8 => leg0 (3.33) fits, but leg1 would
        // push cumulative margin to ~10 > 8, so the safety leg is budget-
        // blocked and must NOT be persisted as a Working order.
        let config = serde_json::from_value::<shared_domain::martingale::MartingaleStrategyConfig>(
            serde_json::json!({
                "strategy_id": "budget-blocked-btc",
                "symbol": "BTCUSDT",
                "market": "usd_m_futures",
                "direction": "long",
                "direction_mode": "long_only",
                "margin_mode": "isolated",
                "leverage": 3,
                "spacing": { "fixed_percent": { "step_bps": 100 } },
                "sizing": { "multiplier": { "first_order_quote": "10", "multiplier": "2", "max_legs": 2 } },
                "take_profit": { "percent": { "bps": 100 } },
                "stop_loss": { "strategy_drawdown_pct": { "pct_bps": 2000 } },
                "indicators": [],
                "entry_triggers": [],
                "risk_limits": { "max_strategy_budget_quote": "8" }
            }),
        )
        .expect("strategy config");
        let settings = trading_engine::martingale_runtime::FuturesExchangeSettings {
            hedge_mode: false,
            symbols: HashMap::from([(
                "BTCUSDT".to_owned(),
                trading_engine::martingale_runtime::FuturesSymbolSettings {
                    margin_mode: shared_domain::martingale::MartingaleMarginMode::Isolated,
                    leverage: 3,
                },
            )]),
        };
        let runtime_config = trading_engine::martingale_runtime::MartingaleRuntimeConfig {
            portfolio_id: "mp-blocked".to_owned(),
            strategy_instance_id: "portfolio".to_owned(),
            portfolio: shared_domain::martingale::MartingalePortfolioConfig {
                direction_mode: shared_domain::martingale::MartingaleDirectionMode::LongOnly,
                strategies: vec![config],
                risk_limits: shared_domain::martingale::MartingaleRiskLimits::default(),
            },
            portfolio_budget_quote: Decimal::new(1000, 0),
            exchange_min_notional: Decimal::ZERO,
        };
        let mut runtime =
            trading_engine::martingale_runtime::MartingaleRuntime::new(runtime_config)
                .expect("runtime");
        runtime
            .start_cycle_with_futures_preflight(
                &settings,
                "budget-blocked-btc",
                Decimal::new(100, 0),
                trading_engine::martingale_runtime::MartingaleRuntimeContext::default(),
            )
            .expect("leg0 placed");
        assert_eq!(runtime.orders().len(), 1);
        assert_eq!(runtime.orders()[0].leg_index, 0);

        // Marking leg0 filled attempts leg1, which the strategy budget blocks.
        let blocked = runtime.mark_leg_filled(
            "budget-blocked-btc",
            shared_domain::martingale::MartingaleDirection::Long,
            0,
        );
        assert!(blocked.is_err(), "leg1 should be blocked by strategy budget");
        // The blocked safety leg must NOT be persisted as a Working order.
        assert!(
            runtime.orders().iter().all(|order| order.leg_index == 0),
            "no leg1 working order after budget block: {:?}",
            runtime.orders()
        );
        assert_eq!(runtime.orders().len(), 1);
    }

    #[test]
    fn martingale_portfolio_uses_global_budget_cap_when_present() {
        let now = Utc::now();
        let portfolio_config = serde_json::json!({
            "portfolio_config": {
                "direction_mode": "long_only",
                "risk_limits": { "max_global_budget_quote": "2000" },
                "strategies": [
                    {
                        "strategy_id": "capped-btc",
                        "symbol": "BTCUSDT",
                        "market": "usd_m_futures",
                        "direction": "long",
                        "direction_mode": "long_only",
                        "margin_mode": "isolated",
                        "leverage": 3,
                        "spacing": { "fixed_percent": { "step_bps": 100 } },
                        "sizing": { "multiplier": { "first_order_quote": "100", "multiplier": "2", "max_legs": 8 } },
                        "take_profit": { "percent": { "bps": 80 } },
                        "stop_loss": { "strategy_drawdown_pct": { "pct_bps": 2000 } },
                        "indicators": [],
                        "entry_triggers": [],
                        "risk_limits": {}
                    }
                ]
            }
        });
        let portfolio = MartingalePortfolioRecord {
            portfolio_id: "mp-capped".to_owned(),
            owner: "engine@example.com".to_owned(),
            name: "capped".to_owned(),
            status: "running".to_owned(),
            source_task_id: "task".to_owned(),
            market: "usd_m_futures".to_owned(),
            direction: "long".to_owned(),
            risk_profile: "conservative".to_owned(),
            total_weight_pct: Decimal::new(100, 0),
            config: portfolio_config.clone(),
            risk_summary: serde_json::json!({}),
            created_at: now,
            updated_at: now,
            items: vec![MartingalePortfolioItemRecord {
                strategy_instance_id: "msi-capped".to_owned(),
                portfolio_id: "mp-capped".to_owned(),
                candidate_id: "bc-capped".to_owned(),
                symbol: "BTCUSDT".to_owned(),
                weight_pct: Decimal::new(100, 0),
                leverage: 3,
                enabled: true,
                status: "running".to_owned(),
                parameter_snapshot: portfolio_config,
                metrics_snapshot: serde_json::json!({}),
                created_at: now,
                updated_at: now,
            }],
        };

        let config = super::martingale_runtime_config_from_portfolio(&portfolio)
            .expect("capped config should map to runtime");
        assert_eq!(config.portfolio_budget_quote, Decimal::new(2000, 0));
        let capped_strategy = &config.portfolio.strategies[0];
        assert_eq!(
            super::strategy_planned_budget_quote(capped_strategy),
            Some(Decimal::new(8500, 0))
        );
        assert_eq!(
            capped_strategy.risk_limits.max_strategy_budget_quote,
            Some(Decimal::new(2000, 0))
        );
    }

    #[test]
    fn martingale_anchor_price_uses_latest_usdm_tick_when_config_has_no_reference() {
        let strategy_config = serde_json::json!({ "symbol": "BTCUSDT" });
        let ticks = vec![
            shared_events::MarketTick {
                symbol: "ETHUSDT".to_owned(),
                market: "usdm".to_owned(),
                price: Decimal::new(2000, 0),
                event_time_ms: 1,
            },
            shared_events::MarketTick {
                symbol: "BTCUSDT".to_owned(),
                market: "spot".to_owned(),
                price: Decimal::new(99, 0),
                event_time_ms: 2,
            },
            shared_events::MarketTick {
                symbol: "BTCUSDT".to_owned(),
                market: "usdm".to_owned(),
                price: Decimal::new(101, 0),
                event_time_ms: 3,
            },
        ];

        assert_eq!(
            super::strategy_anchor_price(&strategy_config, &ticks),
            Some(Decimal::new(101, 0))
        );
    }

    #[test]
    fn martingale_production_start_blocks_unattributed_recovered_position() {
        let mut running = strategy("martingale-restart", StrategyStatus::Running);
        running.strategy_type = StrategyType::MartingaleGrid;
        running.market = StrategyMarket::FuturesUsdM;
        running.mode = StrategyMode::FuturesLong;
        running.draft_revision.strategy_type = StrategyType::MartingaleGrid;
        running.draft_revision.futures_margin_mode =
            Some(shared_domain::strategy::FuturesMarginMode::Cross);
        running.draft_revision.leverage = Some(3);
        running.draft_revision.reference_price = Some(Decimal::new(100, 0));
        running.active_revision = Some(running.draft_revision.clone());
        running.runtime.positions.push(StrategyRuntimePosition {
            market: StrategyMarket::FuturesUsdM,
            mode: StrategyMode::FuturesLong,
            quantity: Decimal::new(1, 0),
            average_entry_price: Decimal::new(100, 0),
        });

        let error = super::sync_martingale_production_start(
            &mut running,
            trading_engine::martingale_runtime::FuturesExchangeSettings {
                hedge_mode: true,
                symbols: HashMap::from([(
                    "BTCUSDT".to_string(),
                    trading_engine::martingale_runtime::FuturesSymbolSettings {
                        margin_mode: shared_domain::martingale::MartingaleMarginMode::Cross,
                        leverage: 3,
                    },
                )]),
            },
        )
        .expect_err("unattributed recovered position must block live start");

        assert!(error.to_string().contains("recovery incomplete"));
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
            strategy_type: StrategyType::OrdinaryGrid,
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotClassic,
            runtime_phase: StrategyRuntimePhase::Draft,
            runtime_controls: RuntimeControls {},
            draft_revision: revision(),
            active_revision: Some(revision()),
            runtime: StrategyRuntime::default(),
            tags: Vec::new(),
            notes: String::new(),
            archived_at: None,
        }
    }

    fn revision() -> StrategyRevision {
        StrategyRevision {
            revision_id: "rev-1".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
            levels: vec![GridLevel {
                level_index: 0,
                entry_price: Decimal::new(100, 0),
                quantity: Decimal::new(1, 0),
                take_profit_bps: 100,
                trailing_bps: None,
            }],
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        }
    }
}
