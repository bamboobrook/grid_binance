use std::{
    env,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use backtest_engine::{
    artifacts::{verify_artifact, write_task_json_artifact},
    intelligent_search::{intelligent_search, EvaluatedCandidate, IntelligentSearchConfig},
    market_data::{AggTrade, KlineBar, MarketDataSource},
    martingale::{
        kline_engine::run_kline_screening,
        metrics::MartingaleBacktestResult,
        portfolio_optimizer::{self, OptimizerCandidate, OptimizerConfig},
        scoring::ScoringConfig,
        trade_engine::run_trade_refinement,
    },
    search::{random_search, SearchCandidate, SearchSpace},
    sqlite_market_data::SqliteMarketDataSource,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared_db::{BacktestRepository, NewBacktestCandidateRecord, SharedDb};
use shared_domain::martingale::{
    MartingaleEntryTrigger, MartingaleIndicatorConfig, MartingaleMarginMode, MartingaleMarketKind,
};

const DEFAULT_MAX_THREADS: usize = 2;
const DEFAULT_POLL_MS: u64 = 5_000;
const DEFAULT_TOP_N: usize = 3;
const DYNAMIC_PER_SYMBOL_TOP_N: usize = 10;
const PORTFOLIO_TOP_N: usize = 10;

#[derive(Debug, Clone)]
struct WorkerConfig {
    database_url: String,
    redis_url: String,
    artifact_root: PathBuf,
    market_data_db_path: Option<PathBuf>,
    max_threads: usize,
    poll_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BacktestTask {
    task_id: String,
    owner: String,
    priority: i64,
    config: WorkerTaskConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkerTaskConfig {
    symbols: Vec<String>,
    random_seed: u64,
    random_candidates: usize,
    intelligent_rounds: usize,
    top_n: usize,
    #[serde(default = "default_per_symbol_top_n")]
    per_symbol_top_n: usize,
    #[serde(default = "default_risk_profile")]
    risk_profile: String,
    #[serde(default)]
    market: Option<String>,
    #[serde(default)]
    margin_mode: Option<String>,
    #[serde(default)]
    direction_mode: Option<String>,
    #[serde(default)]
    leverage_range: Option<[u32; 2]>,
    #[serde(default)]
    martingale_template: Option<Value>,
    #[serde(default)]
    scoring: Option<Value>,
    #[serde(default)]
    dynamic_allocation_enabled: Option<bool>,
    #[serde(default)]
    short_stop_drawdown_pct_candidates: Option<Vec<f64>>,
    #[serde(default)]
    short_atr_stop_multiplier_candidates: Option<Vec<f64>>,
    #[serde(default)]
    allocation_cooldown_hours_candidates: Option<Vec<u32>>,
    #[serde(default = "default_interval")]
    interval: String,
    #[serde(default)]
    start_ms: i64,
    #[serde(default)]
    end_ms: i64,
}

impl Default for WorkerTaskConfig {
    fn default() -> Self {
        Self {
            symbols: vec!["BTCUSDT".to_owned()],
            random_seed: 1,
            random_candidates: 16,
            intelligent_rounds: 2,
            top_n: DEFAULT_TOP_N,
            per_symbol_top_n: default_per_symbol_top_n(),
            risk_profile: default_risk_profile(),
            market: None,
            margin_mode: None,
            direction_mode: None,
            leverage_range: None,
            martingale_template: None,
            scoring: None,
            dynamic_allocation_enabled: None,
            short_stop_drawdown_pct_candidates: None,
            short_atr_stop_multiplier_candidates: None,
            allocation_cooldown_hours_candidates: None,
            interval: default_interval(),
            start_ms: 0,
            end_ms: 0,
        }
    }
}

fn default_interval() -> String {
    "1h".to_owned()
}

fn default_per_symbol_top_n() -> usize {
    5
}

fn effective_per_symbol_top_n(config: &WorkerTaskConfig) -> usize {
    let requested_direction_mode = direction_mode_from_task(config.direction_mode.as_deref());
    let dynamic_allocation_enabled = config.dynamic_allocation_enabled.unwrap_or(
        requested_direction_mode
            == shared_domain::martingale::MartingaleDirectionMode::LongAndShort,
    );
    if dynamic_allocation_enabled && config.per_symbol_top_n == default_per_symbol_top_n() {
        DYNAMIC_PER_SYMBOL_TOP_N
    } else {
        config.per_symbol_top_n.max(1)
    }
}

fn default_risk_profile() -> String {
    "balanced".to_owned()
}

fn stage_label(stage: &str) -> &'static str {
    match stage {
        "market_data_opening" => "打开行情库",
        "search_started" => "参数搜索中",
        "trade_refinement_top_1" => "精测 Top 1",
        "trade_refinement_top_2" => "精测 Top 2",
        "trade_refinement_top_3" => "精测 Top 3",
        _ if stage.starts_with("trade_refinement_top_") => "候选精测中",
        _ => "运行中",
    }
}

fn stage_progress(stage: &str) -> u32 {
    match stage {
        "market_data_opening" => 10,
        "search_started" => 30,
        "trade_refinement_top_1" => 65,
        "trade_refinement_top_2" => 75,
        "trade_refinement_top_3" => 85,
        _ if stage.starts_with("trade_refinement_top_") => 80,
        _ => 50,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CandidateOutput {
    candidate_id: String,
    rank: usize,
    score: f64,
    config: serde_json::Value,
    summary: serde_json::Value,
    artifact_path: String,
    checksum_sha256: String,
    used_trade_refinement: bool,
    total_return_pct: f64,
    max_drawdown_pct: f64,
    trade_count: u64,
}

#[derive(Debug, Clone)]
struct PortfolioOptimizationOutput {
    candidates: Vec<Value>,
    warning: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let config = WorkerConfig::from_env()?;
    eprintln!(
        "backtest-worker starting: max_threads={}, poll_ms={}, artifact_root={}, market_data_db_configured={}, database_url_configured={}, redis_url_configured={}",
        config.max_threads,
        config.poll_ms,
        config.artifact_root.display(),
        config.market_data_db_path.is_some(),
        !config.database_url.is_empty(),
        !config.redis_url.is_empty()
    );

    let poller = TaskPoller::new(config.clone());
    loop {
        match poller.poll_next_queued_by_priority().await? {
            Some(claimed) => {
                let task_id = claimed.task_id().to_owned();
                match claimed.into_task() {
                    Ok(task) => {
                        if let Err(error) = process_task(&config, &poller, task).await {
                            if let Err(mark_error) = poller.mark_failed(&task_id, &error).await {
                                eprintln!(
                                    "backtest task failed and mark_failed also failed: task_id={task_id} error={error} mark_error={mark_error}"
                                );
                            }
                            eprintln!("backtest task failed: {error}");
                        }
                    }
                    Err(error) => {
                        let _ = poller.heartbeat(&task_id, "config_error").await;
                        if let Err(mark_error) = poller.mark_failed(&task_id, &error).await {
                            eprintln!(
                                "backtest task config invalid and mark_failed also failed: task_id={task_id} error={error} mark_error={mark_error}"
                            );
                        }
                        eprintln!("backtest task config invalid: task_id={task_id} error={error}");
                    }
                }
            }
            None => tokio::time::sleep(Duration::from_millis(config.poll_ms)).await,
        }
    }
}

async fn process_task(
    config: &WorkerConfig,
    poller: &TaskPoller,
    task: BacktestTask,
) -> Result<(), String> {
    poller.mark_running(&task.task_id).await?;
    poller
        .heartbeat(&task.task_id, "market_data_opening")
        .await?;
    let market_data = config.open_market_data()?;
    let market_context = MarketDataContext::load(&market_data, &task.config)?;
    poller.heartbeat(&task.task_id, "search_started").await?;

    let search_space = search_space_from_task(&task.config);
    let random_candidates = apply_task_overrides(
        random_search(
            &search_space,
            task.config.random_candidates,
            task.config.random_seed,
        )?,
        &task.config,
    );
    respect_pause_or_cancel(poller, &task.task_id).await?;

    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_watcher = spawn_status_cancel_watcher(
        poller.clone(),
        task.task_id.clone(),
        config.poll_ms,
        cancel.clone(),
    );
    let intelligent = intelligent_search(
        &search_space,
        &IntelligentSearchConfig {
            seed: task.config.random_seed,
            random_round_size: task.config.random_candidates.max(1),
            max_rounds: task.config.intelligent_rounds.max(1),
            max_candidates: task.config.random_candidates.max(1)
                * task.config.intelligent_rounds.max(1),
            survivor_percentile: 0.25,
            timeout: None,
            scoring: scoring_config_from_task(&task.config),
        },
        Some(cancel.as_ref()),
        |candidate| {
            let candidate = apply_task_overrides_to_candidate(candidate.clone(), &task.config);
            run_candidate_kline_screening(&candidate, &market_context)
        },
    )?;
    cancel.store(true, Ordering::SeqCst);
    let _ = cancel_watcher.join();
    respect_pause_or_cancel(poller, &task.task_id).await?;

    let per_symbol_top_n = effective_per_symbol_top_n(&task.config);
    let ranked = select_refinement_candidates_per_symbol(
        intelligent.candidates,
        task.config.symbols.len().max(1) * per_symbol_top_n,
        per_symbol_top_n,
    );
    let mut outputs = Vec::new();

    for (index, evaluated) in ranked.into_iter().enumerate() {
        poller
            .heartbeat(
                &task.task_id,
                &format!("trade_refinement_top_{}", index + 1),
            )
            .await?;
        let refined = run_candidate_trade_refinement(&evaluated.candidate, &market_context)?;
        let used_trade_refinement =
            !trades_for_candidate(&evaluated.candidate, &market_context.trades).is_empty();
        let candidate_id = evaluated.candidate.candidate_id.clone();
        outputs.push(CandidateOutput {
            candidate_id: candidate_id.clone(),
            rank: index + 1,
            score: evaluated.score.rank_score,
            config: serde_json::to_value(&evaluated.candidate.config)
                .map_err(|error| format!("serialize candidate config: {error}"))?,
            summary: result_summary_fields(&refined),
            artifact_path: config
                .artifact_root
                .join(&task.task_id)
                .join(format!("{candidate_id}-summary.jsonl"))
                .display()
                .to_string(),
            checksum_sha256: String::new(),
            used_trade_refinement,
            total_return_pct: refined.metrics.total_return_pct,
            max_drawdown_pct: refined.metrics.max_drawdown_pct,
            trade_count: refined.metrics.trade_count,
        });
        respect_pause_or_cancel(poller, &task.task_id).await?;
    }

    let outputs = select_top_outputs_per_symbol(
        outputs,
        per_symbol_top_n,
        &task.config.risk_profile,
        scoring_config_from_task(&task.config).max_global_drawdown_pct,
    );
    respect_pause_or_cancel(poller, &task.task_id).await?;
    let portfolio_optimization = optimize_output_portfolios(&outputs, &task.config);
    poller
        .save_candidates_and_artifacts(
            &task.task_id,
            &config.artifact_root,
            random_candidates.len(),
            &outputs,
            &portfolio_optimization,
        )
        .await?;
    respect_pause_or_cancel(poller, &task.task_id).await?;
    poller.mark_completed(&task.task_id).await?;
    Ok(())
}

fn select_refinement_candidates_per_symbol(
    mut candidates: Vec<EvaluatedCandidate>,
    min_total: usize,
    per_symbol_top_n: usize,
) -> Vec<EvaluatedCandidate> {
    use std::collections::BTreeMap;

    candidates.sort_by(|left, right| right.score.rank_score.total_cmp(&left.score.rank_score));

    let mut selected = Vec::new();
    let mut selected_counts = BTreeMap::<String, usize>::new();

    for candidate in candidates.iter() {
        let symbol = search_candidate_symbol(&candidate.candidate)
            .unwrap_or_else(|| candidate.candidate.candidate_id.clone());
        let count = selected_counts.entry(symbol).or_default();
        if *count >= per_symbol_top_n {
            continue;
        }
        *count += 1;
        selected.push(candidate.clone());
    }

    if selected.len() >= min_total {
        return selected;
    }

    let mut selected_ids = selected
        .iter()
        .map(|candidate| candidate.candidate.candidate_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    for candidate in candidates {
        if selected.len() >= min_total {
            break;
        }
        if selected_ids.insert(candidate.candidate.candidate_id.clone()) {
            selected.push(candidate);
        }
    }

    selected
}

fn select_top_outputs_per_symbol(
    mut outputs: Vec<CandidateOutput>,
    per_symbol_top_n: usize,
    risk_profile: &str,
    max_drawdown_limit_pct: f64,
) -> Vec<CandidateOutput> {
    use std::collections::BTreeMap;

    outputs.sort_by(|left, right| right.score.total_cmp(&left.score));

    let mut selected = Vec::new();
    let mut selected_counts = BTreeMap::<String, usize>::new();

    for output in outputs {
        let symbol = output_symbol(&output).unwrap_or_else(|| output.candidate_id.clone());
        let count = selected_counts.entry(symbol).or_default();
        if *count >= per_symbol_top_n {
            continue;
        }
        *count += 1;
        selected.push(output);
    }

    let total_selected_by_symbol = selected.iter().fold(BTreeMap::new(), |mut counts, output| {
        let symbol = output_symbol(output).unwrap_or_else(|| output.candidate_id.clone());
        *counts.entry(symbol).or_insert(0usize) += 1;
        counts
    });
    let mut rank_by_symbol = BTreeMap::<String, usize>::new();

    selected
        .into_iter()
        .enumerate()
        .map(|(index, mut output)| {
            let symbol = output_symbol(&output).unwrap_or_else(|| output.candidate_id.clone());
            let per_symbol_rank = {
                let rank = rank_by_symbol.entry(symbol.clone()).or_insert(0usize);
                *rank += 1;
                *rank
            };
            let selected_for_symbol = total_selected_by_symbol
                .get(&symbol)
                .copied()
                .unwrap_or(1)
                .max(1);
            let recommended_weight_pct = 100.0 / selected_for_symbol as f64;
            let recommended_leverage = output_leverage(&output).unwrap_or(1);
            let portfolio_group_key = output_portfolio_group_key(&output);
            let spacing_bps = output_spacing_bps(&output);
            let first_order_quote = output_first_order_quote(&output);
            let order_multiplier = output_order_multiplier(&output);
            let max_legs = output_max_legs(&output);
            let take_profit_bps = output_take_profit_bps(&output);
            let trailing_take_profit_bps = output_trailing_take_profit_bps(&output);
            let direction = output_direction(&output);
            let overfit_flag = output_overfit_flag(&output);
            let risk_summary_human = output_risk_summary_human(&output, risk_profile);
            let allocation_curve = output_summary_value_or(&output, "allocation_curve", json!([]));
            let regime_timeline = output_summary_value_or(&output, "regime_timeline", json!([]));
            let cost_summary = output_summary_value_or(
                &output,
                "cost_summary",
                json!({
                    "fee_quote": 0.0,
                    "slippage_quote": 0.0,
                    "stop_loss_quote": 0.0,
                    "forced_exit_quote": 0.0,
                }),
            );
            let rebalance_count = output_summary_value_or(&output, "rebalance_count", json!(0));
            let forced_exit_count = output_summary_value_or(&output, "forced_exit_count", json!(0));
            let average_allocation_hold_hours =
                output_summary_value_or(&output, "average_allocation_hold_hours", Value::Null);

            output.rank = index + 1;
            output.summary = merge_json_objects(
                output.summary,
                json!({
                    "symbol": symbol,
                    "direction": direction,
                    "per_symbol_rank": per_symbol_rank,
                    "portfolio_top_n": PORTFOLIO_TOP_N,
                    "recommended_weight_pct": recommended_weight_pct,
                    "recommended_leverage": recommended_leverage,
                    "risk_profile": risk_profile,
                    "max_drawdown_limit_pct": max_drawdown_limit_pct,
                    "dynamic_allocation_rules": dynamic_allocation_rules(),
                    "allocation_curve": allocation_curve,
                    "regime_timeline": regime_timeline,
                    "cost_summary": cost_summary,
                    "rebalance_count": rebalance_count,
                    "forced_exit_count": forced_exit_count,
                    "average_allocation_hold_hours": average_allocation_hold_hours,
                    "portfolio_group_key": portfolio_group_key,
                    "spacing_bps": spacing_bps,
                    "first_order_quote": first_order_quote,
                    "order_multiplier": order_multiplier,
                    "max_legs": max_legs,
                    "take_profit_bps": take_profit_bps,
                    "trailing_take_profit_bps": trailing_take_profit_bps,
                    "total_return_pct": output.total_return_pct,
                    "max_drawdown_pct": output.max_drawdown_pct,
                    "score": output.score,
                    "overfit_flag": overfit_flag,
                    "risk_summary_human": risk_summary_human,
                    "artifact_path": output.artifact_path,
                }),
            );
            output
        })
        .collect()
}

fn merge_json_objects(base: Value, patch: Value) -> Value {
    let mut merged = match base {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    if let Value::Object(patch_map) = patch {
        for (key, value) in patch_map {
            merged.insert(key, value);
        }
    }
    Value::Object(merged)
}

fn result_summary_fields(result: &MartingaleBacktestResult) -> Value {
    let cost_burden_quote = result.cost_summary.fee_quote
        + result.cost_summary.slippage_quote
        + result.cost_summary.stop_loss_quote
        + result.cost_summary.forced_exit_quote;
    json!({
        "equity_curve": result.equity_curve,
        "allocation_curve": result.allocation_curve,
        "regime_timeline": result.regime_timeline,
        "cost_summary": result.cost_summary,
        "rebalance_count": result.rebalance_count,
        "forced_exit_count": result.forced_exit_count,
        "average_allocation_hold_hours": result.average_allocation_hold_hours,
        "stop_loss_count_human": format!("止损次数 {} 次", result.metrics.stop_count),
        "forced_exit_count_human": format!("强制退出 {} 次", result.forced_exit_count),
        "cost_burden_quote": cost_burden_quote,
        "cost_burden_human": format!("成本负担 {:.2} U", cost_burden_quote),
    })
}

fn dynamic_allocation_rules() -> Value {
    json!({
        "timeframes": ["4h", "1d"],
        "btc_filter": true,
        "funding_rate_used": false,
        "weight_buckets": [[100, 0], [80, 20], [60, 40], [50, 50], [40, 60], [20, 80], [0, 100]],
        "cooldown_hours": 16,
        "existing_position_policy": "tiered_pause_cancel_force_exit",
    })
}

fn output_summary_value_or(output: &CandidateOutput, key: &str, default: Value) -> Value {
    output.summary.get(key).cloned().unwrap_or(default)
}

fn output_strategy(output: &CandidateOutput) -> Option<&Value> {
    output
        .config
        .get("strategies")
        .and_then(Value::as_array)
        .and_then(|strategies| strategies.first())
}

fn output_symbol(output: &CandidateOutput) -> Option<String> {
    output_strategy(output)
        .and_then(|strategy| strategy.get("symbol"))
        .and_then(Value::as_str)
        .map(|symbol| symbol.trim().to_uppercase())
        .filter(|symbol| !symbol.is_empty())
}

fn output_direction(output: &CandidateOutput) -> Value {
    output_strategy(output)
        .and_then(|strategy| strategy.get("direction"))
        .cloned()
        .unwrap_or(Value::Null)
}

fn output_leverage(output: &CandidateOutput) -> Option<u32> {
    output_strategy(output)
        .and_then(|strategy| strategy.get("leverage"))
        .and_then(Value::as_u64)
        .map(|value| value as u32)
}

fn output_portfolio_group_key(output: &CandidateOutput) -> String {
    output_symbol(output).unwrap_or_else(|| output.candidate_id.clone())
}

fn output_spacing_bps(output: &CandidateOutput) -> Value {
    strategy_value_at(output, &["spacing", "fixed_percent", "step_bps"])
        .cloned()
        .unwrap_or(Value::Null)
}

fn output_first_order_quote(output: &CandidateOutput) -> Value {
    strategy_value_at(output, &["sizing", "multiplier", "first_order_quote"])
        .cloned()
        .unwrap_or(Value::Null)
}

fn output_order_multiplier(output: &CandidateOutput) -> Value {
    strategy_value_at(output, &["sizing", "multiplier", "multiplier"])
        .cloned()
        .unwrap_or(Value::Null)
}

fn output_max_legs(output: &CandidateOutput) -> Value {
    strategy_value_at(output, &["sizing", "multiplier", "max_legs"])
        .cloned()
        .unwrap_or(Value::Null)
}

fn output_take_profit_bps(output: &CandidateOutput) -> Value {
    strategy_value_at(output, &["take_profit", "percent", "bps"])
        .cloned()
        .unwrap_or(Value::Null)
}

fn output_trailing_take_profit_bps(output: &CandidateOutput) -> Value {
    strategy_value_at(output, &["take_profit", "trailing", "callback_bps"])
        .cloned()
        .unwrap_or(Value::Null)
}

fn output_overfit_flag(output: &CandidateOutput) -> bool {
    output.trade_count < 5 || output.max_drawdown_pct > 50.0 || !output.score.is_finite()
}

fn output_risk_summary_human(output: &CandidateOutput, risk_profile: &str) -> String {
    let overfit = if output_overfit_flag(output) {
        "需复核过拟合风险"
    } else {
        "过拟合风险未触发"
    };
    let stop_loss_count = output
        .summary
        .get("stop_loss_count_human")
        .and_then(Value::as_str)
        .unwrap_or("止损次数 0 次");
    let forced_exit_count = output
        .summary
        .get("forced_exit_count_human")
        .and_then(Value::as_str)
        .unwrap_or("强制退出 0 次");
    let cost_burden = output
        .summary
        .get("cost_burden_human")
        .and_then(Value::as_str)
        .unwrap_or("成本负担 0.00 U");
    format!(
        "{} 风险档，收益 {:.2}%，最大回撤 {:.2}%，{}，{}，{}，{}。",
        risk_profile,
        output.total_return_pct,
        output.max_drawdown_pct,
        stop_loss_count,
        forced_exit_count,
        cost_burden,
        overfit
    )
}

fn optimize_output_portfolios(
    outputs: &[CandidateOutput],
    config: &WorkerTaskConfig,
) -> PortfolioOptimizationOutput {
    let optimizer_config = optimizer_config_from_task(config);
    if !optimizer_config.max_drawdown_pct.is_finite() || optimizer_config.max_drawdown_pct < 0.0 {
        return PortfolioOptimizationOutput {
            candidates: Vec::new(),
            warning: Some(format!(
                "portfolio optimizer skipped: invalid max_drawdown_pct {}",
                optimizer_config.max_drawdown_pct
            )),
        };
    }

    let candidates = outputs
        .iter()
        .filter_map(|output| {
            let equity_curve = output_equity_curve(output);
            if equity_curve.is_empty() {
                return None;
            }
            Some(OptimizerCandidate::new(
                output.candidate_id.clone(),
                output_symbol(output)?,
                output.total_return_pct,
                output.max_drawdown_pct,
                equity_curve,
            ))
        })
        .collect::<Vec<_>>();

    match portfolio_optimizer::optimize_portfolios(&candidates, &optimizer_config, PORTFOLIO_TOP_N)
    {
        Ok(portfolios) => PortfolioOptimizationOutput {
            candidates: portfolios
                .into_iter()
                .enumerate()
                .map(|(index, portfolio)| {
                    json!({
                        "rank": index + 1,
                        "items": portfolio.items.into_iter().map(|item| json!({
                            "candidate_id": item.candidate_id,
                            "symbol": item.symbol,
                            "weight_pct": item.weight_pct,
                        })).collect::<Vec<_>>(),
                        "total_return_pct": portfolio.total_return_pct,
                        "max_drawdown_pct": portfolio.max_drawdown_pct,
                        "return_drawdown_ratio": portfolio.return_drawdown_ratio,
                    })
                })
                .collect(),
            warning: None,
        },
        Err(error) => PortfolioOptimizationOutput {
            candidates: Vec::new(),
            warning: Some(format!("portfolio optimizer skipped: {error}")),
        },
    }
}

fn candidate_summary_artifact_row(task_id: &str, output: &CandidateOutput) -> Value {
    merge_json_objects(
        json!({
            "task_id": task_id,
            "candidate_id": output.candidate_id,
            "rank": output.rank,
            "kline_score": output.score,
            "trade_metrics": {
                "total_return_pct": output.total_return_pct,
                "max_drawdown_pct": output.max_drawdown_pct,
                "trade_count": output.trade_count,
            },
        }),
        output.summary.clone(),
    )
}

fn task_portfolio_summary(portfolio_optimization: &PortfolioOptimizationOutput) -> Value {
    let mut summary = json!({
        "portfolio_top_n": PORTFOLIO_TOP_N,
        "portfolio_candidates": portfolio_optimization.candidates,
    });
    if let Some(warning) = &portfolio_optimization.warning {
        summary = merge_json_objects(
            summary,
            json!({
                "portfolio_optimizer_warning": warning,
                "warnings": [warning],
            }),
        );
    }
    summary
}

fn write_candidate_summary_artifact(
    artifact_root: &std::path::Path,
    task_id: &str,
    output: &CandidateOutput,
) -> Result<(String, String), String> {
    let rows = vec![candidate_summary_artifact_row(task_id, output)];
    let manifest = write_task_json_artifact(
        artifact_root,
        task_id,
        &output.candidate_id,
        "summary",
        &rows,
    )?;
    verify_artifact(&manifest)?;
    Ok((
        manifest.path.display().to_string(),
        manifest.checksum_sha256,
    ))
}

fn optimizer_config_from_task(config: &WorkerTaskConfig) -> OptimizerConfig {
    let max_drawdown_pct = scoring_config_from_task(config).max_global_drawdown_pct;
    match config.risk_profile.as_str() {
        "conservative" => OptimizerConfig::conservative(max_drawdown_pct),
        "aggressive" => OptimizerConfig::aggressive(max_drawdown_pct),
        _ => OptimizerConfig::balanced(max_drawdown_pct),
    }
}

fn output_equity_curve(output: &CandidateOutput) -> Vec<f64> {
    output
        .summary
        .get("equity_curve")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(equity_point_value).collect())
        .unwrap_or_default()
}

fn equity_point_value(value: &Value) -> Option<f64> {
    value.as_f64().or_else(|| {
        value.as_object().and_then(|object| {
            ["equity_quote", "equity", "capital", "value"]
                .iter()
                .find_map(|key| object.get(*key).and_then(Value::as_f64))
        })
    })
}

fn strategy_value_at<'a>(output: &'a CandidateOutput, path: &[&str]) -> Option<&'a Value> {
    let mut value = output_strategy(output)?;
    for key in path {
        value = value.get(*key)?;
    }
    Some(value)
}

fn search_space_from_task(config: &WorkerTaskConfig) -> SearchSpace {
    let requested_direction_mode = direction_mode_from_task(config.direction_mode.as_deref());
    let dynamic_allocation_enabled = config.dynamic_allocation_enabled.unwrap_or(
        requested_direction_mode
            == shared_domain::martingale::MartingaleDirectionMode::LongAndShort,
    );
    let direction_mode = if requested_direction_mode
        == shared_domain::martingale::MartingaleDirectionMode::LongAndShort
        && !dynamic_allocation_enabled
    {
        shared_domain::martingale::MartingaleDirectionMode::IndicatorSelected
    } else {
        requested_direction_mode
    };
    SearchSpace {
        symbols: config.symbols.clone(),
        direction_mode,
        directions: directions_from_mode(config.direction_mode.as_deref()),
        market: market_kind(config.market.as_deref()),
        margin_mode: margin_mode(config.margin_mode.as_deref()),
        step_bps: search_space_u32(config, "spacing_bps")
            .or_else(|| template_u32(config, &["spacing", "step_bps"]).map(|value| vec![value]))
            .unwrap_or_else(|| vec![25, 50, 100]),
        first_order_quote: search_space_decimal(config, "first_order_quote")
            .or_else(|| {
                template_decimal(config, &["sizing", "first_order_quote"]).map(|value| vec![value])
            })
            .unwrap_or_else(|| vec![Decimal::new(100, 0), Decimal::new(250, 0)]),
        multiplier: search_space_decimal(config, "order_multiplier")
            .or_else(|| {
                template_decimal(config, &["sizing", "multiplier"]).map(|value| vec![value])
            })
            .unwrap_or_else(|| vec![Decimal::new(15, 1)]),
        take_profit_bps: search_space_u32(config, "take_profit_bps")
            .or_else(|| template_u32(config, &["take_profit", "bps"]).map(|value| vec![value]))
            .unwrap_or_else(|| vec![30, 60, 100]),
        leverage: leverage_values(config.leverage_range),
        max_legs: search_space_u32(config, "max_legs")
            .or_else(|| template_u32(config, &["sizing", "max_legs"]).map(|value| vec![value]))
            .unwrap_or_else(|| vec![3, 5, 7]),
        dynamic_allocation_enabled,
        short_stop_drawdown_pct_candidates: config
            .short_stop_drawdown_pct_candidates
            .clone()
            .filter(|values| !values.is_empty())
            .unwrap_or_else(|| short_stop_drawdown_pct_candidates(config)),
        short_atr_stop_multiplier_candidates: config
            .short_atr_stop_multiplier_candidates
            .clone()
            .filter(|values| !values.is_empty())
            .unwrap_or_else(|| vec![1.5, 2.0, 2.5, 3.0]),
        allocation_cooldown_hours_candidates: config
            .allocation_cooldown_hours_candidates
            .clone()
            .filter(|values| !values.is_empty())
            .unwrap_or_else(|| allocation_cooldown_hours_candidates(&config.risk_profile)),
    }
}

fn short_stop_drawdown_pct_candidates(config: &WorkerTaskConfig) -> Vec<f64> {
    let limit = config
        .scoring
        .as_ref()
        .and_then(|value| value.get("max_drawdown_pct"))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(25.0);
    let mut candidates = vec![limit * 0.64, limit * 0.8, limit, limit * 1.2];
    candidates.retain(|value| value.is_finite() && *value > 0.0);
    candidates.sort_by(|left, right| left.total_cmp(right));
    candidates.dedup_by(|left, right| (*left - *right).abs() < f64::EPSILON);
    candidates
}

fn allocation_cooldown_hours_candidates(risk_profile: &str) -> Vec<u32> {
    match risk_profile {
        "conservative" => vec![24, 36, 48],
        "aggressive" => vec![6, 12, 18],
        _ => vec![12, 16, 24],
    }
}

fn scoring_config_from_task(config: &WorkerTaskConfig) -> ScoringConfig {
    let mut scoring = ScoringConfig::default();
    if let Some(value) = config.scoring.as_ref() {
        if let Some(max_drawdown_pct) = value.get("max_drawdown_pct").and_then(Value::as_f64) {
            scoring.max_global_drawdown_pct = max_drawdown_pct;
            scoring.max_strategy_drawdown_pct = max_drawdown_pct;
        }
        if let Some(max_stop_count) = value.get("max_stop_loss_count").and_then(Value::as_u64) {
            scoring.max_stop_count = max_stop_count;
        }
        if let Some(weights) = value.get("weights") {
            assign_f64(weights, "weight_return", &mut scoring.weight_return);
            assign_f64(weights, "weight_calmar", &mut scoring.weight_calmar);
            assign_f64(weights, "weight_sortino", &mut scoring.weight_sortino);
            assign_f64(weights, "weight_drawdown", &mut scoring.weight_drawdown);
            assign_f64(
                weights,
                "weight_stop_frequency",
                &mut scoring.weight_stop_frequency,
            );
            assign_f64(
                weights,
                "weight_capital_utilization",
                &mut scoring.weight_capital_utilization,
            );
            assign_f64(
                weights,
                "weight_trade_stability",
                &mut scoring.weight_trade_stability,
            );
        }
    }
    scoring
}

fn assign_f64(source: &Value, key: &str, target: &mut f64) {
    if let Some(value) = source.get(key).and_then(Value::as_f64) {
        if value.is_finite() {
            *target = value;
        }
    }
}

fn apply_task_overrides(
    candidates: Vec<SearchCandidate>,
    config: &WorkerTaskConfig,
) -> Vec<SearchCandidate> {
    candidates
        .into_iter()
        .map(|candidate| apply_task_overrides_to_candidate(candidate, config))
        .collect()
}

fn apply_task_overrides_to_candidate(
    mut candidate: SearchCandidate,
    config: &WorkerTaskConfig,
) -> SearchCandidate {
    let indicators = indicator_configs_from_task(config);
    let entry_triggers = entry_triggers_from_task(config);
    if indicators.is_empty() && entry_triggers.is_empty() {
        return candidate;
    }
    for strategy in &mut candidate.config.strategies {
        if !indicators.is_empty() {
            strategy.indicators = indicators.clone();
        }
        if !entry_triggers.is_empty() {
            strategy.entry_triggers = entry_triggers.clone();
        }
    }
    candidate
}

fn indicator_configs_from_task(config: &WorkerTaskConfig) -> Vec<MartingaleIndicatorConfig> {
    let Some(indicators) = config
        .martingale_template
        .as_ref()
        .and_then(|template| template.get("indicators"))
    else {
        return Vec::new();
    };
    serde_json::from_value(indicators.clone()).unwrap_or_default()
}

fn entry_triggers_from_task(config: &WorkerTaskConfig) -> Vec<MartingaleEntryTrigger> {
    let Some(entry_triggers) = config
        .martingale_template
        .as_ref()
        .and_then(|template| template.get("entry_triggers"))
    else {
        return Vec::new();
    };
    serde_json::from_value(entry_triggers.clone()).unwrap_or_default()
}

fn market_kind(value: Option<&str>) -> Option<MartingaleMarketKind> {
    match value {
        Some("spot") => Some(MartingaleMarketKind::Spot),
        Some("usd_m_futures") => Some(MartingaleMarketKind::UsdMFutures),
        _ => None,
    }
}

fn margin_mode(value: Option<&str>) -> Option<MartingaleMarginMode> {
    match value {
        Some("isolated") => Some(MartingaleMarginMode::Isolated),
        Some("cross") => Some(MartingaleMarginMode::Cross),
        _ => None,
    }
}

fn search_space_value<'a>(config: &'a WorkerTaskConfig, key: &str) -> Option<&'a Value> {
    config
        .martingale_template
        .as_ref()
        .and_then(|template| template.get("search_space"))
        .and_then(|space| space.get(key))
}

fn search_space_u32(config: &WorkerTaskConfig, key: &str) -> Option<Vec<u32>> {
    let values = search_space_value(config, key)?.as_array()?;
    let parsed: Vec<u32> = values
        .iter()
        .filter_map(Value::as_u64)
        .filter_map(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .collect();
    (!parsed.is_empty()).then_some(parsed)
}

fn search_space_decimal(config: &WorkerTaskConfig, key: &str) -> Option<Vec<Decimal>> {
    let values = search_space_value(config, key)?.as_array()?;
    let parsed: Vec<Decimal> = values
        .iter()
        .filter_map(decimal_from_value)
        .filter(|value| *value > Decimal::ZERO)
        .collect();
    (!parsed.is_empty()).then_some(parsed)
}

fn decimal_from_value(value: &Value) -> Option<Decimal> {
    if let Some(number) = value.as_i64() {
        return Some(Decimal::from(number));
    }
    value
        .as_str()
        .and_then(|text| text.parse::<Decimal>().ok())
        .or_else(|| value.to_string().parse::<Decimal>().ok())
}

fn direction_mode_from_task(
    mode: Option<&str>,
) -> shared_domain::martingale::MartingaleDirectionMode {
    use shared_domain::martingale::MartingaleDirectionMode::{
        IndicatorSelected, LongAndShort, LongOnly, ShortOnly,
    };
    match mode {
        Some("long_only") => LongOnly,
        Some("short_only") => ShortOnly,
        Some("long_and_short") => LongAndShort,
        _ => IndicatorSelected,
    }
}

fn directions_from_mode(mode: Option<&str>) -> Vec<shared_domain::martingale::MartingaleDirection> {
    use shared_domain::martingale::MartingaleDirection::{Long, Short};
    match mode {
        Some("long_only") => vec![Long],
        Some("short_only") => vec![Short],
        Some("long_and_short") => vec![Long, Short],
        _ => vec![Long, Short],
    }
}

fn leverage_values(range: Option<[u32; 2]>) -> Vec<u32> {
    let Some([left, right]) = range else {
        return vec![1, 2, 3];
    };
    let start = left.min(right).max(1);
    let end = left.max(right).max(start);
    (start..=end).collect()
}

fn template_value<'a>(config: &'a WorkerTaskConfig, path: &[&str]) -> Option<&'a Value> {
    let mut current = config.martingale_template.as_ref()?;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn template_u32(config: &WorkerTaskConfig, path: &[&str]) -> Option<u32> {
    template_value(config, path)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
}

fn template_decimal(config: &WorkerTaskConfig, path: &[&str]) -> Option<Decimal> {
    let value = template_value(config, path)?;
    decimal_from_value(value).filter(|value| *value > Decimal::ZERO)
}

#[derive(Debug, Clone)]
struct MarketDataContext {
    bars: Vec<KlineBar>,
    trades: Vec<AggTrade>,
}

impl MarketDataContext {
    fn load(source: &dyn MarketDataSource, config: &WorkerTaskConfig) -> Result<Self, String> {
        validate_time_range(config)?;
        let available_symbols = source.list_symbols()?;
        let available_set = available_symbols
            .iter()
            .map(|symbol| symbol.trim().to_uppercase())
            .collect::<std::collections::BTreeSet<_>>();
        let mut bars = Vec::new();
        let mut trades = Vec::new();
        for symbol in &config.symbols {
            let normalized = symbol.trim().to_uppercase();
            if !available_set.contains(&normalized) {
                return Err(format!(
                    "market data source does not contain requested symbol {normalized}"
                ));
            }
            let mut symbol_bars = source.load_klines(
                &normalized,
                config.start_ms,
                config.end_ms,
                &config.interval,
            )?;
            if symbol_bars.is_empty() {
                return Err(format!(
                    "no klines for {normalized} interval={} range={}..{}",
                    config.interval, config.start_ms, config.end_ms
                ));
            }
            let mut symbol_trades =
                source.load_agg_trades(&normalized, config.start_ms, config.end_ms)?;
            bars.append(&mut symbol_bars);
            trades.append(&mut symbol_trades);
        }
        bars.sort_by(|left, right| {
            left.open_time_ms
                .cmp(&right.open_time_ms)
                .then_with(|| left.symbol.cmp(&right.symbol))
        });
        trades.sort_by(|left, right| {
            left.trade_time_ms
                .cmp(&right.trade_time_ms)
                .then_with(|| left.symbol.cmp(&right.symbol))
        });
        Ok(Self { bars, trades })
    }
}

fn validate_time_range(config: &WorkerTaskConfig) -> Result<(), String> {
    if config.start_ms <= 0 || config.end_ms <= 0 || config.start_ms >= config.end_ms {
        return Err(
            "backtest worker requires positive start_ms/end_ms with start_ms < end_ms".to_owned(),
        );
    }
    if config.interval.trim().is_empty() {
        return Err("backtest interval cannot be empty".to_owned());
    }
    Ok(())
}

fn run_candidate_kline_screening(
    candidate: &SearchCandidate,
    market_context: &MarketDataContext,
) -> Result<MartingaleBacktestResult, String> {
    let bars = bars_for_candidate(candidate, &market_context.bars);
    if bars.is_empty() {
        return Err(format!(
            "candidate {} has no matching kline bars",
            candidate.candidate_id
        ));
    }
    run_kline_screening(candidate.config.clone(), &bars)
}

fn run_candidate_trade_refinement(
    candidate: &SearchCandidate,
    market_context: &MarketDataContext,
) -> Result<MartingaleBacktestResult, String> {
    let trades = trades_for_candidate(candidate, &market_context.trades);
    if trades.is_empty() {
        let bars = bars_for_candidate(candidate, &market_context.bars);
        if bars.is_empty() {
            return Err(format!(
                "candidate {} has no matching kline bars for candle-only refinement",
                candidate.candidate_id
            ));
        }
        return run_kline_screening(candidate.config.clone(), &bars);
    }
    run_trade_refinement(candidate.config.clone(), &trades)
}

fn bars_for_candidate(candidate: &SearchCandidate, bars: &[KlineBar]) -> Vec<KlineBar> {
    let symbols = candidate_symbols(candidate);
    bars.iter()
        .filter(|bar| symbols.contains(&bar.symbol.trim().to_uppercase()))
        .cloned()
        .collect()
}

fn trades_for_candidate(candidate: &SearchCandidate, trades: &[AggTrade]) -> Vec<AggTrade> {
    let symbols = candidate_symbols(candidate);
    trades
        .iter()
        .filter(|trade| symbols.contains(&trade.symbol.trim().to_uppercase()))
        .cloned()
        .collect()
}

fn candidate_symbols(candidate: &SearchCandidate) -> std::collections::BTreeSet<String> {
    candidate
        .config
        .strategies
        .iter()
        .map(|strategy| strategy.symbol.trim().to_uppercase())
        .collect()
}

fn search_candidate_symbol(candidate: &SearchCandidate) -> Option<String> {
    candidate
        .config
        .strategies
        .first()
        .map(|strategy| strategy.symbol.trim().to_uppercase())
        .filter(|symbol| !symbol.is_empty())
}

fn spawn_status_cancel_watcher(
    poller: TaskPoller,
    task_id: String,
    poll_ms: u64,
    cancel: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while !cancel.load(Ordering::SeqCst) {
            match poller.current_status_sync(&task_id) {
                Ok(Some(status))
                    if matches!(
                        status.as_str(),
                        "paused" | "cancelled" | "failed" | "succeeded"
                    ) =>
                {
                    cancel.store(true, Ordering::SeqCst);
                    break;
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => {
                    cancel.store(true, Ordering::SeqCst);
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(poll_ms.max(1)));
        }
    })
}

async fn respect_pause_or_cancel(poller: &TaskPoller, task_id: &str) -> Result<(), String> {
    match poller.current_status(task_id).await?.as_deref() {
        Some("cancelled") => Err(format!("task cancelled: {task_id}")),
        Some("failed") => Err(format!("task failed externally: {task_id}")),
        Some("succeeded") => Err(format!("task completed externally: {task_id}")),
        Some("paused") => {
            poller.heartbeat(task_id, "paused_wait").await?;
            loop {
                tokio::time::sleep(Duration::from_millis(poller.poll_ms())).await;
                match poller.current_status(task_id).await?.as_deref() {
                    Some("running") => return Ok(()),
                    Some("queued") => {
                        return Err(format!(
                            "task returned to queued while paused; reclaim required: {task_id}"
                        ));
                    }
                    Some("cancelled") => {
                        return Err(format!("task cancelled while paused: {task_id}"))
                    }
                    Some("failed") => return Err(format!("task failed while paused: {task_id}")),
                    Some("succeeded") => {
                        return Err(format!("task completed while paused: {task_id}"));
                    }
                    Some("paused") => {}
                    Some(other) => {
                        return Err(format!("unexpected task status {other}: {task_id}"))
                    }
                    None => return Err(format!("task disappeared while paused: {task_id}")),
                }
            }
        }
        _ => Ok(()),
    }
}

#[derive(Clone)]
struct TaskPoller {
    repo: BacktestRepository,
    poll_ms: u64,
}

impl TaskPoller {
    fn new(config: WorkerConfig) -> Self {
        let db = SharedDb::connect(&config.database_url, &config.redis_url)
            .expect("DATABASE_URL/REDIS_URL must open SharedDb for backtest-worker");
        let repo = db.backtest_repo();
        Self {
            repo,
            poll_ms: config.poll_ms,
        }
    }

    fn poll_ms(&self) -> u64 {
        self.poll_ms
    }

    async fn poll_next_queued_by_priority(&self) -> Result<Option<ClaimedTask>, String> {
        if let Ok(raw) = env::var("BACKTEST_WORKER_DEMO_TASK_JSON") {
            let task = serde_json::from_str(&raw)
                .map_err(|error| format!("parse BACKTEST_WORKER_DEMO_TASK_JSON: {error}"))?;
            return Ok(Some(ClaimedTask::Parsed(task)));
        }

        let claimed = self
            .repo
            .claim_next_queued_task()
            .map_err(|error| format!("claim queued task: {error}"))?
            .map(ClaimedTask::Record);
        Ok(claimed)
    }

    async fn mark_running(&self, task_id: &str) -> Result<(), String> {
        self.repo
            .append_task_event(task_id, "running", json!({ "worker": "backtest-worker" }))
            .map_err(|error| format!("append running event: {error}"))?;
        Ok(())
    }

    async fn heartbeat(&self, task_id: &str, stage: &str) -> Result<(), String> {
        self.repo
            .append_task_event(task_id, "heartbeat", json!({ "stage": stage }))
            .map_err(|error| format!("append heartbeat event: {error}"))?;
        self.repo
            .update_task_summary(
                task_id,
                json!({
                    "stage": stage,
                    "stage_label": stage_label(stage),
                    "progress_pct": stage_progress(stage),
                }),
            )
            .map_err(|error| format!("update task summary: {error}"))?;
        Ok(())
    }

    async fn current_status(&self, task_id: &str) -> Result<Option<String>, String> {
        self.current_status_sync(task_id)
    }

    fn current_status_sync(&self, task_id: &str) -> Result<Option<String>, String> {
        self.repo
            .find_task(task_id)
            .map_err(|error| format!("find task status: {error}"))
            .map(|task| task.map(|task| task.status))
    }

    async fn save_candidates_and_artifacts(
        &self,
        task_id: &str,
        artifact_root: &std::path::Path,
        screened_count: usize,
        outputs: &[CandidateOutput],
        portfolio_optimization: &PortfolioOptimizationOutput,
    ) -> Result<(), String> {
        self.repo
            .append_task_event(
                task_id,
                "screening_completed",
                json!({ "screened_count": screened_count, "selected_count": outputs.len() }),
            )
            .map_err(|error| format!("append screening event: {error}"))?;
        for output in outputs {
            let (artifact_path, checksum_sha256) =
                write_candidate_summary_artifact(artifact_root, task_id, output)?;
            self.repo
                .save_candidate_with_artifact(
                    NewBacktestCandidateRecord {
                        task_id: task_id.to_owned(),
                        status: "ready".to_owned(),
                        rank: output.rank as i32,
                        config: output.config.clone(),
                        summary: merge_json_objects(
                            merge_json_objects(
                                json!({
                                    "score": output.score,
                                    "search_mode": "智能搜索",
                                    "result_mode": if output.used_trade_refinement { "成交级精测" } else { "K线级回测" },
                                    "total_return_pct": output.total_return_pct,
                                    "max_drawdown_pct": output.max_drawdown_pct,
                                    "trade_count": output.trade_count,
                                }),
                                output.summary.clone(),
                            ),
                            json!({ "artifact_path": artifact_path }),
                        ),
                    },
                    "summary",
                    artifact_path,
                    json!({
                        "checksum_sha256": checksum_sha256,
                        "source_candidate_id": output.candidate_id,
                    }),
                )
                .map_err(|error| format!("save candidate artifact bundle: {error}"))?;
        }
        self.repo
            .update_task_summary(task_id, task_portfolio_summary(portfolio_optimization))
            .map_err(|error| format!("update portfolio candidate summary: {error}"))?;
        Ok(())
    }

    async fn mark_completed(&self, task_id: &str) -> Result<(), String> {
        self.repo
            .update_task_summary(
                task_id,
                json!({
                    "stage": "completed",
                    "stage_label": "已完成",
                    "progress_pct": 100,
                }),
            )
            .map_err(|error| format!("update completed summary: {error}"))?;
        self.repo
            .transition_task(task_id, "succeeded")
            .map_err(|error| format!("mark task completed: {error}"))?;
        self.repo
            .append_task_event(task_id, "completed", json!({}))
            .map_err(|error| format!("append completed event: {error}"))?;
        Ok(())
    }

    async fn mark_failed(&self, task_id: &str, error: &str) -> Result<(), String> {
        self.repo
            .update_task_summary(
                task_id,
                json!({
                    "stage": "failed",
                    "stage_label": "失败",
                    "progress_pct": 100,
                }),
            )
            .map_err(|error| format!("update failed summary: {error}"))?;
        self.repo
            .fail_task(task_id, error)
            .map_err(|error| format!("mark task failed: {error}"))?;
        self.repo
            .append_task_event(task_id, "failed", json!({ "error": error }))
            .map_err(|error| format!("append failed event: {error}"))?;
        Ok(())
    }
}

fn task_priority(config: &serde_json::Value, summary: &serde_json::Value) -> i64 {
    summary
        .get("priority")
        .and_then(serde_json::Value::as_i64)
        .or_else(|| config.get("priority").and_then(serde_json::Value::as_i64))
        .unwrap_or(0)
}

enum ClaimedTask {
    Parsed(BacktestTask),
    Record(shared_db::BacktestTaskRecord),
}

impl ClaimedTask {
    fn task_id(&self) -> &str {
        match self {
            Self::Parsed(task) => &task.task_id,
            Self::Record(record) => &record.task_id,
        }
    }

    fn into_task(self) -> Result<BacktestTask, String> {
        match self {
            Self::Parsed(task) => Ok(task),
            Self::Record(record) => {
                let priority = task_priority(&record.config, &record.summary);
                let config = serde_json::from_value(record.config.clone()).map_err(|error| {
                    format!(
                        "invalid backtest task config for {}: {error}",
                        record.task_id
                    )
                })?;
                Ok(BacktestTask {
                    task_id: record.task_id,
                    owner: record.owner,
                    priority,
                    config,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use backtest_engine::martingale::scoring::CandidateScore;
    use std::{fs, path::PathBuf};

    use shared_domain::martingale::{
        MartingaleDirection, MartingaleDirectionMode, MartingalePortfolioConfig,
        MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel,
        MartingaleStrategyConfig, MartingaleTakeProfitModel,
    };

    mod tempfile {
        use std::{
            env, fs, io,
            path::{Path, PathBuf},
            sync::atomic::{AtomicU64, Ordering},
            time::{SystemTime, UNIX_EPOCH},
        };

        static NEXT_ID: AtomicU64 = AtomicU64::new(0);

        pub struct TempDir {
            path: PathBuf,
        }

        impl TempDir {
            pub fn new() -> io::Result<Self> {
                for _ in 0..100 {
                    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
                    let now_nanos = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos();
                    let path = env::temp_dir().join(format!(
                        "backtest-worker-persistence-{}-{now_nanos}-{id}",
                        std::process::id()
                    ));
                    match fs::create_dir(&path) {
                        Ok(()) => return Ok(Self { path }),
                        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                        Err(error) => return Err(error),
                    }
                }

                Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "failed to create unique temporary directory",
                ))
            }

            pub fn path(&self) -> &Path {
                &self.path
            }
        }

        impl Drop for TempDir {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.path);
            }
        }
    }

    struct MemoryMarketDataSource {
        symbols: Vec<String>,
        bars: Vec<KlineBar>,
        trades: Vec<AggTrade>,
    }

    impl MarketDataSource for MemoryMarketDataSource {
        fn list_symbols(&self) -> Result<Vec<String>, String> {
            Ok(self.symbols.clone())
        }

        fn load_klines(
            &self,
            symbol: &str,
            start_ms: i64,
            end_ms: i64,
            _interval: &str,
        ) -> Result<Vec<KlineBar>, String> {
            Ok(self
                .bars
                .iter()
                .filter(|bar| {
                    bar.symbol == symbol
                        && bar.open_time_ms >= start_ms
                        && bar.open_time_ms <= end_ms
                })
                .cloned()
                .collect())
        }

        fn load_agg_trades(
            &self,
            symbol: &str,
            start_ms: i64,
            end_ms: i64,
        ) -> Result<Vec<AggTrade>, String> {
            Ok(self
                .trades
                .iter()
                .filter(|trade| {
                    trade.symbol == symbol
                        && trade.trade_time_ms >= start_ms
                        && trade.trade_time_ms <= end_ms
                })
                .cloned()
                .collect())
        }

        fn schema_fingerprint(&self) -> Result<String, String> {
            Ok("memory".to_owned())
        }
    }

    #[test]
    fn claimed_task_rejects_bad_config() {
        let record = shared_db::BacktestTaskRecord {
            task_id: "task-bad".to_owned(),
            owner: "user@example.com".to_owned(),
            status: "running".to_owned(),
            strategy_type: "martingale_grid".to_owned(),
            config: serde_json::json!({ "symbols": "BTCUSDT" }),
            summary: serde_json::json!({}),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            error_message: None,
        };

        assert!(ClaimedTask::Record(record).into_task().is_err());
    }

    fn candidate_output(
        symbol: &str,
        id: &str,
        rank: usize,
        score: f64,
        leverage: u32,
    ) -> CandidateOutput {
        CandidateOutput {
            candidate_id: id.to_owned(),
            rank,
            score,
            config: serde_json::json!({
                "strategies": [{
                    "symbol": symbol,
                    "leverage": leverage,
                    "spacing": { "fixed_percent": { "step_bps": 100 } },
                    "sizing": {
                        "multiplier": {
                            "first_order_quote": "10",
                            "multiplier": "1.5",
                            "max_legs": 4
                        }
                    },
                    "take_profit": { "percent": { "bps": 80 } }
                }]
            }),
            summary: serde_json::json!({}),
            artifact_path: format!("/tmp/{id}.json"),
            checksum_sha256: "sha256".to_owned(),
            used_trade_refinement: false,
            total_return_pct: score,
            max_drawdown_pct: 5.0,
            trade_count: 10,
        }
    }

    fn candidate_output_with_curve(
        symbol: &str,
        id: &str,
        score: f64,
        equity_curve: Value,
    ) -> CandidateOutput {
        let mut output = candidate_output(symbol, id, 1, score, 3);
        output.summary = serde_json::json!({ "equity_curve": equity_curve });
        output
    }

    fn worker_task_config_with_scoring(max_drawdown_pct: Value) -> WorkerTaskConfig {
        WorkerTaskConfig {
            scoring: Some(serde_json::json!({ "max_drawdown_pct": max_drawdown_pct })),
            ..WorkerTaskConfig::default()
        }
    }

    fn evaluated_candidate(symbol: &str, id: &str, score: f64) -> EvaluatedCandidate {
        let strategy = MartingaleStrategyConfig {
            strategy_id: id.to_owned(),
            symbol: symbol.to_owned(),
            market: MartingaleMarketKind::UsdMFutures,
            direction: MartingaleDirection::Long,
            direction_mode: MartingaleDirectionMode::LongOnly,
            margin_mode: Some(MartingaleMarginMode::Isolated),
            leverage: Some(2),
            spacing: MartingaleSpacingModel::FixedPercent { step_bps: 100 },
            sizing: MartingaleSizingModel::Multiplier {
                first_order_quote: Decimal::new(10, 0),
                multiplier: Decimal::new(2, 0),
                max_legs: 5,
            },
            take_profit: MartingaleTakeProfitModel::Percent { bps: 100 },
            stop_loss: None,
            indicators: Vec::new(),
            entry_triggers: Vec::new(),
            risk_limits: MartingaleRiskLimits::default(),
        };
        EvaluatedCandidate {
            candidate: SearchCandidate {
                candidate_id: id.to_owned(),
                config: MartingalePortfolioConfig {
                    direction_mode: MartingaleDirectionMode::LongOnly,
                    strategies: vec![strategy],
                    risk_limits: MartingaleRiskLimits::default(),
                },
            },
            score: CandidateScore {
                survival_valid: true,
                rank_score: score,
                raw_score: score,
                rejection_reasons: Vec::new(),
            },
        }
    }

    #[test]
    fn refinement_selection_keeps_top_five_per_symbol_before_global_cutoff() {
        let mut candidates = Vec::new();
        for index in 0..8 {
            candidates.push(evaluated_candidate(
                "ETHUSDT",
                &format!("eth-{index}"),
                100.0 - index as f64,
            ));
        }
        for index in 0..5 {
            candidates.push(evaluated_candidate(
                "BTCUSDT",
                &format!("btc-{index}"),
                50.0 - index as f64,
            ));
        }

        let selected = select_refinement_candidates_per_symbol(candidates, 10, 5);
        let eth_count = selected
            .iter()
            .filter(|candidate| {
                search_candidate_symbol(&candidate.candidate).as_deref() == Some("ETHUSDT")
            })
            .count();
        let btc_count = selected
            .iter()
            .filter(|candidate| {
                search_candidate_symbol(&candidate.candidate).as_deref() == Some("BTCUSDT")
            })
            .count();

        assert_eq!(selected.len(), 10);
        assert_eq!(eth_count, 5);
        assert_eq!(btc_count, 5);
    }

    #[test]
    fn candidate_outputs_keep_top_five_per_symbol_and_enrich_summary() {
        let outputs = vec![
            candidate_output("BTCUSDT", "btc-1", 1, 90.0, 3),
            candidate_output("BTCUSDT", "btc-2", 2, 80.0, 3),
            candidate_output("BTCUSDT", "btc-3", 3, 70.0, 3),
            candidate_output("BTCUSDT", "btc-4", 4, 60.0, 3),
            candidate_output("BTCUSDT", "btc-5", 5, 50.0, 3),
            candidate_output("BTCUSDT", "btc-6", 6, 40.0, 3),
            candidate_output("ETHUSDT", "eth-1", 1, 30.0, 2),
        ];

        let selected = select_top_outputs_per_symbol(outputs, 5, "balanced", 40.0);

        assert_eq!(selected.len(), 6);
        assert!(selected.iter().any(|output| output.candidate_id == "btc-5"));
        assert!(!selected.iter().any(|output| output.candidate_id == "btc-6"));

        let btc_first = selected
            .iter()
            .find(|output| output.candidate_id == "btc-1")
            .unwrap();
        assert_eq!(btc_first.summary["symbol"], "BTCUSDT");
        assert_eq!(btc_first.summary["per_symbol_rank"], 1);
        assert!(btc_first.summary.get("parameter_rank_for_symbol").is_none());
        assert_eq!(btc_first.summary["recommended_weight_pct"], 20.0);
        assert_eq!(btc_first.summary["recommended_leverage"], 3);
        assert_eq!(btc_first.summary["risk_profile"], "balanced");
    }

    #[test]
    fn selected_outputs_include_ui_required_summary_fields() {
        let outputs = vec![candidate_output("BTCUSDT", "btc-1", 1, 90.0, 3)];

        let selected = select_top_outputs_per_symbol(outputs, 5, "balanced", 40.0);
        let output = selected.first().unwrap();
        let summary = output.summary.as_object().unwrap();

        for field in [
            "symbol",
            "direction",
            "spacing_bps",
            "first_order_quote",
            "order_multiplier",
            "max_legs",
            "take_profit_bps",
            "trailing_take_profit_bps",
            "recommended_weight_pct",
            "recommended_leverage",
            "per_symbol_rank",
            "risk_profile",
            "total_return_pct",
            "max_drawdown_pct",
            "score",
            "overfit_flag",
            "risk_summary_human",
        ] {
            assert!(
                summary.contains_key(field),
                "missing summary field: {field}"
            );
        }
        assert!(summary.contains_key("artifact_path") || summary.contains_key("equity_curve"));
        assert_eq!(summary["artifact_path"], output.artifact_path);
        assert!(summary.get("allocation_curve").is_some());
        assert!(summary.get("regime_timeline").is_some());
        assert!(summary.get("cost_summary").is_some());
        assert_eq!(summary["per_symbol_rank"], 1);
        assert_eq!(summary["portfolio_top_n"], 10);
        assert!(summary.get("dynamic_allocation_rules").is_some());
        assert!(summary.get("max_drawdown_limit_pct").is_some());
    }

    #[tokio::test]
    async fn save_candidates_and_artifacts_persists_ui_contract_fields() {
        let db = SharedDb::ephemeral().expect("ephemeral shared db");
        let repo = db.backtest_repo();
        let task = repo
            .create_task(shared_db::NewBacktestTaskRecord {
                owner: "user@example.com".to_owned(),
                strategy_type: "martingale_grid".to_owned(),
                config: serde_json::json!({ "symbol": "BTCUSDT", "timeframe": "1h" }),
                summary: serde_json::json!({}),
            })
            .expect("create backtest task");
        let poller = TaskPoller {
            repo: repo.clone(),
            poll_ms: 1,
        };
        let artifact_temp_dir = tempfile::TempDir::new().expect("create artifact temp dir");
        let artifact_root = artifact_temp_dir.path().to_path_buf();

        let output = select_top_outputs_per_symbol(
            vec![candidate_output_with_curve(
                "BTCUSDT",
                "btc-1",
                90.0,
                serde_json::json!([
                    { "equity": 1000.0 },
                    { "equity_quote": 1010.0 }
                ]),
            )],
            5,
            "balanced",
            40.0,
        )
        .remove(0);
        let portfolio_optimization = PortfolioOptimizationOutput {
            candidates: vec![serde_json::json!({
                "rank": 1,
                "items": [{
                    "candidate_id": "btc-1",
                    "symbol": "BTCUSDT",
                    "weight_pct": 100.0,
                }],
                "total_return_pct": 90.0,
                "max_drawdown_pct": 5.0,
                "return_drawdown_ratio": 18.0,
            })],
            warning: None,
        };

        poller
            .save_candidates_and_artifacts(
                &task.task_id,
                &artifact_root,
                1,
                std::slice::from_ref(&output),
                &portfolio_optimization,
            )
            .await
            .expect("save candidates and artifacts");

        let candidates = repo
            .list_candidates(&task.task_id)
            .expect("list persisted candidates");
        assert_eq!(candidates.len(), 1);
        let candidate_summary = &candidates[0].summary;
        for field in [
            "equity_curve",
            "per_symbol_rank",
            "symbol",
            "direction",
            "spacing_bps",
            "first_order_quote",
            "order_multiplier",
            "max_legs",
            "take_profit_bps",
            "trailing_take_profit_bps",
            "recommended_weight_pct",
            "recommended_leverage",
            "risk_profile",
            "total_return_pct",
            "max_drawdown_pct",
            "score",
            "overfit_flag",
            "risk_summary_human",
        ] {
            assert!(
                candidate_summary.get(field).is_some(),
                "missing persisted candidate summary field: {field}"
            );
        }

        let task_after_save = repo
            .find_task(&task.task_id)
            .expect("find task after save")
            .expect("task after save exists");
        assert_eq!(task_after_save.summary["portfolio_top_n"], PORTFOLIO_TOP_N);
        assert_eq!(
            task_after_save.summary["portfolio_candidates"]
                .as_array()
                .expect("portfolio candidates array")
                .len(),
            1
        );

        poller
            .mark_completed(&task.task_id)
            .await
            .expect("mark task completed");
        let task_after_completed = repo
            .find_task(&task.task_id)
            .expect("find task after completed")
            .expect("task after completed exists");
        assert_eq!(
            task_after_completed.summary["portfolio_top_n"],
            PORTFOLIO_TOP_N
        );
        assert_eq!(
            task_after_completed.summary["portfolio_candidates"]
                .as_array()
                .expect("portfolio candidates survive completion")
                .len(),
            1
        );

        let artifact_path = candidate_summary["artifact_path"]
            .as_str()
            .expect("persisted artifact path");
        assert_ne!(artifact_path, output.artifact_path);
        assert!(
            std::path::Path::new(artifact_path).starts_with(&artifact_root),
            "artifact file should be inside test artifact root: {artifact_path}"
        );
        assert!(
            std::path::Path::new(artifact_path).exists(),
            "artifact file should exist: {artifact_path}"
        );
        let artifact_contents =
            fs::read_to_string(artifact_path).expect("read persisted artifact file");
        let artifact_row: Value = serde_json::from_str(
            artifact_contents
                .lines()
                .next()
                .expect("artifact JSONL summary row"),
        )
        .expect("parse persisted artifact JSONL row");
        for field in [
            "allocation_curve",
            "regime_timeline",
            "cost_summary",
            "rebalance_count",
            "forced_exit_count",
            "average_allocation_hold_hours",
            "dynamic_allocation_rules",
            "portfolio_top_n",
            "risk_summary_human",
            "max_drawdown_limit_pct",
        ] {
            assert!(
                artifact_row.get(field).is_some(),
                "missing persisted artifact row field: {field}"
            );
        }
    }

    #[test]
    fn candidate_summary_artifact_row_contains_ui_required_fields() {
        let output = select_top_outputs_per_symbol(
            vec![candidate_output_with_curve(
                "BTCUSDT",
                "btc-1",
                90.0,
                serde_json::json!([1000.0, 1010.0]),
            )],
            5,
            "balanced",
            40.0,
        )
        .remove(0);

        let row = candidate_summary_artifact_row("task-1", &output);

        for field in [
            "portfolio_top_n",
            "allocation_curve",
            "regime_timeline",
            "cost_summary",
            "rebalance_count",
            "forced_exit_count",
            "average_allocation_hold_hours",
            "dynamic_allocation_rules",
            "risk_summary_human",
            "per_symbol_rank",
            "equity_curve",
        ] {
            assert!(row.get(field).is_some(), "missing artifact field: {field}");
        }
        assert!(row.get("parameter_rank_for_symbol").is_none());
        assert_eq!(row["portfolio_top_n"], PORTFOLIO_TOP_N);
        assert_eq!(row["trade_metrics"]["total_return_pct"], 90.0);
    }

    #[test]
    fn output_equity_curve_reads_arrays_and_common_point_objects() {
        let numeric = candidate_output_with_curve(
            "BTCUSDT",
            "btc-1",
            90.0,
            serde_json::json!([1000.0, 990.0, 1015.0]),
        );
        let objects = candidate_output_with_curve(
            "ETHUSDT",
            "eth-1",
            80.0,
            serde_json::json!([
                { "equity": 1000.0 },
                { "capital": 995.0 },
                { "value": 1010.0 },
                { "equity_quote": 1020.0 }
            ]),
        );
        let missing = candidate_output("SOLUSDT", "sol-1", 1, 70.0, 2);

        assert_eq!(output_equity_curve(&numeric), vec![1000.0, 990.0, 1015.0]);
        assert_eq!(
            output_equity_curve(&objects),
            vec![1000.0, 995.0, 1010.0, 1020.0]
        );
        assert!(output_equity_curve(&missing).is_empty());
    }

    #[test]
    fn portfolio_optimizer_degrades_on_invalid_drawdown_limit() {
        let output = candidate_output_with_curve(
            "BTCUSDT",
            "btc-1",
            90.0,
            serde_json::json!([1000.0, 1010.0]),
        );
        let config = worker_task_config_with_scoring(serde_json::json!(-1.0));

        let optimized = optimize_output_portfolios(&[output], &config);
        let summary = task_portfolio_summary(&optimized);

        assert!(optimized.candidates.is_empty());
        assert!(optimized
            .warning
            .unwrap()
            .contains("invalid max_drawdown_pct"));
        assert_eq!(summary["portfolio_top_n"], PORTFOLIO_TOP_N);
        assert_eq!(summary["portfolio_candidates"].as_array().unwrap().len(), 0);
        assert!(summary["portfolio_optimizer_warning"]
            .as_str()
            .unwrap()
            .contains("invalid max_drawdown_pct"));
    }

    #[test]
    fn portfolio_optimizer_uses_real_curves_and_keeps_top10_rank_order() {
        let mut outputs = Vec::new();
        for index in 0..10 {
            outputs.push(candidate_output_with_curve(
                &format!("SYM{index}USDT"),
                &format!("candidate-{index}"),
                100.0 - index as f64,
                serde_json::json!([1000.0, 1005.0 + index as f64, 1010.0 + index as f64]),
            ));
        }
        let config = worker_task_config_with_scoring(serde_json::json!(40.0));

        let optimized = optimize_output_portfolios(&outputs, &config);
        let summary = task_portfolio_summary(&optimized);

        assert_eq!(optimized.warning, None);
        assert_eq!(optimized.candidates.len(), PORTFOLIO_TOP_N);
        for (index, candidate) in optimized.candidates.iter().enumerate() {
            assert_eq!(candidate["rank"], index + 1);
        }
        assert_eq!(summary["portfolio_top_n"], PORTFOLIO_TOP_N);
        assert_eq!(
            summary["portfolio_candidates"].as_array().unwrap().len(),
            PORTFOLIO_TOP_N
        );
    }

    #[test]
    fn portfolio_optimizer_skips_empty_and_irregular_curves_without_fabricating() {
        let missing = candidate_output("BTCUSDT", "btc-1", 1, 90.0, 3);
        let irregular_left = candidate_output_with_curve(
            "ETHUSDT",
            "eth-1",
            80.0,
            serde_json::json!([1000.0, 1005.0, 1010.0]),
        );
        let irregular_right = candidate_output_with_curve(
            "SOLUSDT",
            "sol-1",
            70.0,
            serde_json::json!([1000.0, 1002.0]),
        );
        let config = worker_task_config_with_scoring(serde_json::json!(40.0));

        let empty_result = optimize_output_portfolios(&[missing], &config);
        let irregular_result =
            optimize_output_portfolios(&[irregular_left, irregular_right], &config);

        assert!(empty_result.candidates.is_empty());
        assert_eq!(empty_result.warning, None);
        assert!(irregular_result.candidates.is_empty());
        assert_eq!(irregular_result.warning, None);
    }

    #[test]
    fn worker_requires_market_data_path_instead_of_synthetic_candidates() {
        let config = WorkerConfig {
            database_url: "postgres://example".to_owned(),
            redis_url: "redis://example".to_owned(),
            artifact_root: PathBuf::from("/tmp/backtest-artifacts"),
            market_data_db_path: None,
            max_threads: 1,
            poll_ms: 1,
        };

        let error = config.open_market_data().unwrap_err();
        assert!(error.contains("BACKTEST_MARKET_DATA_DB_PATH is required"));
        assert!(error.contains("refusing to generate synthetic"));
    }

    #[test]
    fn market_context_loads_only_requested_readonly_data_range() {
        let source = MemoryMarketDataSource {
            symbols: vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()],
            bars: vec![
                KlineBar {
                    symbol: "BTCUSDT".to_owned(),
                    open_time_ms: 1_000,
                    open: 100.0,
                    high: 101.0,
                    low: 99.0,
                    close: 100.5,
                    volume: 10.0,
                },
                KlineBar {
                    symbol: "ETHUSDT".to_owned(),
                    open_time_ms: 1_000,
                    open: 200.0,
                    high: 201.0,
                    low: 199.0,
                    close: 200.5,
                    volume: 10.0,
                },
            ],
            trades: vec![
                AggTrade {
                    symbol: "BTCUSDT".to_owned(),
                    trade_time_ms: 1_000,
                    price: 100.0,
                    quantity: 1.0,
                    is_buyer_maker: false,
                },
                AggTrade {
                    symbol: "ETHUSDT".to_owned(),
                    trade_time_ms: 1_000,
                    price: 200.0,
                    quantity: 1.0,
                    is_buyer_maker: false,
                },
            ],
        };
        let config = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned()],
            random_seed: 1,
            random_candidates: 1,
            intelligent_rounds: 1,
            top_n: 1,
            per_symbol_top_n: default_per_symbol_top_n(),
            risk_profile: default_risk_profile(),
            market: None,
            margin_mode: None,
            direction_mode: None,
            leverage_range: None,
            martingale_template: None,
            scoring: None,
            dynamic_allocation_enabled: None,
            short_stop_drawdown_pct_candidates: None,
            short_atr_stop_multiplier_candidates: None,
            allocation_cooldown_hours_candidates: None,
            interval: "1h".to_owned(),
            start_ms: 1,
            end_ms: 2_000,
        };

        let context = MarketDataContext::load(&source, &config).expect("market context");
        assert_eq!(context.bars.len(), 1);
        assert_eq!(context.bars[0].symbol, "BTCUSDT");
        assert_eq!(context.trades.len(), 1);
        assert_eq!(context.trades[0].symbol, "BTCUSDT");
    }

    #[test]
    fn market_context_rejects_missing_trade_refinement_data() {
        let source = MemoryMarketDataSource {
            symbols: vec!["BTCUSDT".to_owned()],
            bars: vec![KlineBar {
                symbol: "BTCUSDT".to_owned(),
                open_time_ms: 1_000,
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.5,
                volume: 10.0,
            }],
            trades: Vec::new(),
        };
        let config = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned()],
            random_seed: 1,
            random_candidates: 1,
            intelligent_rounds: 1,
            top_n: 1,
            per_symbol_top_n: default_per_symbol_top_n(),
            risk_profile: default_risk_profile(),
            market: None,
            margin_mode: None,
            direction_mode: None,
            leverage_range: None,
            martingale_template: None,
            scoring: None,
            dynamic_allocation_enabled: None,
            short_stop_drawdown_pct_candidates: None,
            short_atr_stop_multiplier_candidates: None,
            allocation_cooldown_hours_candidates: None,
            interval: "1h".to_owned(),
            start_ms: 1,
            end_ms: 2_000,
        };

        let context =
            MarketDataContext::load(&source, &config).expect("market context without trades");
        assert!(context.trades.is_empty());
    }

    #[test]
    fn search_space_uses_wizard_parameter_ranges() {
        let config: WorkerTaskConfig = serde_json::from_value(serde_json::json!({
            "symbols": ["BTCUSDT", "ETHUSDT"],
            "random_seed": 7,
            "random_candidates": 12,
            "intelligent_rounds": 3,
            "top_n": 5,
            "market": "usd_m_futures",
            "margin_mode": "isolated",
            "direction_mode": "short_only",
            "leverage_range": [2, 4],
            "martingale_template": {
                "spacing": { "model": "fixed_percent", "step_bps": 125 },
                "sizing": {
                    "model": "multiplier",
                    "first_order_quote": 15,
                    "multiplier": 2,
                    "max_legs": 8
                },
                "take_profit": { "model": "percent", "bps": 90 },
                "search_space": {
                    "spacing_bps": [75, 125],
                    "first_order_quote": [10, 15],
                    "order_multiplier": [1.4, 2],
                    "take_profit_bps": [80, 90],
                    "max_legs": [6, 8]
                }
            }
        }))
        .expect("worker task config");

        let space = search_space_from_task(&config);

        assert_eq!(space.symbols, vec!["BTCUSDT", "ETHUSDT"]);
        assert_eq!(
            space.directions,
            vec![shared_domain::martingale::MartingaleDirection::Short]
        );
        assert_eq!(space.market, Some(MartingaleMarketKind::UsdMFutures));
        assert_eq!(space.margin_mode, Some(MartingaleMarginMode::Isolated));
        assert_eq!(space.step_bps, vec![75, 125]);
        assert_eq!(
            space.first_order_quote,
            vec![Decimal::new(10, 0), Decimal::new(15, 0)]
        );
        assert_eq!(
            space.multiplier,
            vec![Decimal::new(14, 1), Decimal::new(2, 0)]
        );
        assert_eq!(space.take_profit_bps, vec![80, 90]);
        assert_eq!(space.leverage, vec![2, 3, 4]);
        assert_eq!(space.max_legs, vec![6, 8]);
    }

    #[test]
    fn default_task_config_keeps_single_direction_sampling_metadata() {
        let config = WorkerTaskConfig::default();

        let space = search_space_from_task(&config);

        assert_eq!(
            space.direction_mode,
            shared_domain::martingale::MartingaleDirectionMode::IndicatorSelected
        );
        assert_eq!(space.directions.len(), 2);
        assert!(!space.dynamic_allocation_enabled);
        assert!(space.leverage.contains(&1));
        assert_eq!(default_interval(), "1h");
        assert_eq!(default_per_symbol_top_n(), 5);
        assert_eq!(effective_per_symbol_top_n(&config), 5);
    }

    #[test]
    fn dynamic_long_short_defaults_generate_search_metadata_candidates() {
        let config: WorkerTaskConfig = serde_json::from_value(serde_json::json!({
            "symbols": ["BTCUSDT"],
            "random_seed": 7,
            "random_candidates": 12,
            "intelligent_rounds": 3,
            "top_n": 10,
            "risk_profile": "balanced",
            "direction_mode": "long_and_short",
            "scoring": {
                "max_drawdown_pct": 25.0
            }
        }))
        .expect("config");

        let space = search_space_from_task(&config);

        assert!(space.dynamic_allocation_enabled);
        assert_eq!(effective_per_symbol_top_n(&config), 10);
        assert_eq!(
            space.short_atr_stop_multiplier_candidates,
            vec![1.5, 2.0, 2.5, 3.0]
        );
        assert_eq!(space.allocation_cooldown_hours_candidates, vec![12, 16, 24]);
        assert!(space.short_stop_drawdown_pct_candidates.contains(&16.0));
        assert!(space.short_stop_drawdown_pct_candidates.contains(&20.0));
        assert!(space.short_stop_drawdown_pct_candidates.contains(&25.0));
        assert!(space.short_stop_drawdown_pct_candidates.contains(&30.0));
        assert!(!space.short_stop_drawdown_pct_candidates.contains(&12.5));
    }

    #[test]
    fn dynamic_allocation_false_long_short_uses_single_direction_search_mode() {
        let disabled: WorkerTaskConfig = serde_json::from_value(serde_json::json!({
            "symbols": ["BTCUSDT"],
            "random_seed": 7,
            "random_candidates": 4,
            "intelligent_rounds": 1,
            "top_n": 2,
            "direction_mode": "long_and_short",
            "dynamic_allocation_enabled": false,
            "market": "usd_m_futures"
        }))
        .expect("disabled config");
        let enabled: WorkerTaskConfig = serde_json::from_value(serde_json::json!({
            "symbols": ["BTCUSDT"],
            "random_seed": 7,
            "random_candidates": 4,
            "intelligent_rounds": 1,
            "top_n": 2,
            "direction_mode": "long_and_short",
            "dynamic_allocation_enabled": true,
            "market": "usd_m_futures"
        }))
        .expect("enabled config");

        let disabled_space = search_space_from_task(&disabled);
        let enabled_space = search_space_from_task(&enabled);
        let disabled_candidates = random_search(&disabled_space, 4, 7).expect("disabled search");
        let enabled_candidates = random_search(&enabled_space, 4, 7).expect("enabled search");

        assert_eq!(
            disabled_space.direction_mode,
            shared_domain::martingale::MartingaleDirectionMode::IndicatorSelected
        );
        assert!(!disabled_space.dynamic_allocation_enabled);
        assert!(disabled_candidates.iter().all(|candidate| {
            candidate.config.direction_mode
                != shared_domain::martingale::MartingaleDirectionMode::LongAndShort
                && candidate.config.strategies.len() == 1
        }));
        assert_eq!(
            enabled_space.direction_mode,
            shared_domain::martingale::MartingaleDirectionMode::LongAndShort
        );
        assert!(enabled_space.dynamic_allocation_enabled);
        assert!(enabled_candidates.iter().all(|candidate| {
            candidate.config.direction_mode
                == shared_domain::martingale::MartingaleDirectionMode::LongAndShort
                && candidate.config.strategies.len() == 2
        }));
    }

    #[test]
    fn worker_applies_wizard_scoring_and_indicator_overrides() {
        let config: WorkerTaskConfig = serde_json::from_value(serde_json::json!({
            "symbols": ["BTCUSDT"],
            "random_seed": 7,
            "random_candidates": 1,
            "intelligent_rounds": 1,
            "top_n": 1,
            "scoring": {
                "max_drawdown_pct": 12.5,
                "max_stop_loss_count": 2,
                "weights": {
                    "weight_return": 0.2,
                    "weight_calmar": 0.3,
                    "weight_sortino": 0.1,
                    "weight_drawdown": 0.4,
                    "weight_stop_frequency": 0.6,
                    "weight_capital_utilization": 0.7,
                    "weight_trade_stability": 0.8
                }
            },
            "martingale_template": {
                "indicators": [
                    { "atr": { "period": 21 } },
                    { "rsi": { "period": 14, "overbought": "68", "oversold": "32" } }
                ],
                "entry_triggers": [
                    { "indicator_expression": { "expression": "rsi(14) <= 68" } }
                ]
            }
        }))
        .expect("worker task config");

        let scoring = scoring_config_from_task(&config);
        assert_eq!(scoring.max_global_drawdown_pct, 12.5);
        assert_eq!(scoring.max_strategy_drawdown_pct, 12.5);
        assert_eq!(scoring.max_stop_count, 2);
        assert_eq!(scoring.weight_stop_frequency, 0.6);
        assert_eq!(scoring.weight_capital_utilization, 0.7);
        assert_eq!(scoring.weight_trade_stability, 0.8);

        let candidate = random_search(&search_space_from_task(&config), 1, 7)
            .expect("candidate")
            .remove(0);
        let candidate = apply_task_overrides_to_candidate(candidate, &config);
        let strategy = &candidate.config.strategies[0];
        assert!(matches!(
            strategy.indicators[0],
            MartingaleIndicatorConfig::Atr { period: 21 }
        ));
        assert!(matches!(
            strategy.indicators[1],
            MartingaleIndicatorConfig::Rsi { .. }
        ));
        assert!(matches!(
            strategy.entry_triggers[0],
            MartingaleEntryTrigger::IndicatorExpression { .. }
        ));
    }
}

impl WorkerConfig {
    fn from_env() -> Result<Self, String> {
        Ok(Self {
            database_url: required_env("DATABASE_URL")?,
            redis_url: required_env("REDIS_URL")?,
            artifact_root: PathBuf::from(required_env("BACKTEST_ARTIFACT_ROOT")?),
            market_data_db_path: optional_env_path("BACKTEST_MARKET_DATA_DB_PATH"),
            max_threads: parse_env_usize("BACKTEST_WORKER_MAX_THREADS", DEFAULT_MAX_THREADS)?,
            poll_ms: parse_env_u64("BACKTEST_WORKER_POLL_MS", DEFAULT_POLL_MS)?,
        })
    }

    fn open_market_data(&self) -> Result<SqliteMarketDataSource, String> {
        let path = self.market_data_db_path.as_ref().ok_or_else(|| {
            "BACKTEST_MARKET_DATA_DB_PATH is required for worker backtests; refusing to generate synthetic martingale candidates".to_owned()
        })?;
        SqliteMarketDataSource::open_readonly(path).map_err(|error| {
            format!(
                "open read-only market data database {}: {error}",
                path.display()
            )
        })
    }
}

fn optional_env_path(name: &str) -> Option<PathBuf> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("{name} is required"))
}

fn parse_env_usize(name: &str, default: usize) -> Result<usize, String> {
    match env::var(name) {
        Ok(value) => value
            .parse::<usize>()
            .map_err(|error| format!("invalid {name}: {error}"))
            .map(|value| value.max(1)),
        Err(_) => Ok(default),
    }
}

fn parse_env_u64(name: &str, default: u64) -> Result<u64, String> {
    match env::var(name) {
        Ok(value) => value
            .parse::<u64>()
            .map_err(|error| format!("invalid {name}: {error}"))
            .map(|value| value.max(1)),
        Err(_) => Ok(default),
    }
}
