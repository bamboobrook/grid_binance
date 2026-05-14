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
        metrics::{EquityPoint, MartingaleBacktestEvent, MartingaleBacktestResult},
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
    "1m".to_owned()
}

fn default_per_symbol_top_n() -> usize {
    10
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

    let ranked = select_refinement_candidates_per_symbol(
        intelligent.candidates,
        task.config.symbols.len().max(1) * task.config.per_symbol_top_n.max(1),
        task.config.per_symbol_top_n.max(1),
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
        let visible_summary = visible_backtest_summary(
            &refined,
            &evaluated.candidate,
            &market_context,
            &task.config,
        );
        let rows = vec![json!({
            "task_id": task.task_id,
            "candidate_id": evaluated.candidate.candidate_id,
            "rank": index + 1,
            "kline_score": evaluated.score.rank_score,
            "trade_metrics": {
                "total_return_pct": refined.metrics.total_return_pct,
                "max_drawdown_pct": refined.metrics.max_drawdown_pct,
                "trade_count": refined.metrics.trade_count,
            },
        })];
        respect_pause_or_cancel(poller, &task.task_id).await?;
        let manifest = write_task_json_artifact(
            &config.artifact_root,
            &task.task_id,
            &evaluated.candidate.candidate_id,
            "summary",
            &rows,
        )?;
        verify_artifact(&manifest)?;
        let refined_score = backtest_engine::martingale::scoring::score_candidate(
            &refined,
            &scoring_config_from_task(&task.config),
        );
        outputs.push(CandidateOutput {
            candidate_id: evaluated.candidate.candidate_id,
            rank: index + 1,
            score: refined_score.rank_score,
            config: serde_json::to_value(&evaluated.candidate.config)
                .map_err(|error| format!("serialize candidate config: {error}"))?,
            summary: visible_summary,
            artifact_path: manifest.path.display().to_string(),
            checksum_sha256: manifest.checksum_sha256,
            used_trade_refinement,
            total_return_pct: refined.metrics.total_return_pct,
            max_drawdown_pct: refined.metrics.max_drawdown_pct,
            trade_count: refined.metrics.trade_count,
        });
        respect_pause_or_cancel(poller, &task.task_id).await?;
    }

    let outputs = select_top_outputs_per_symbol(
        outputs,
        task.config.per_symbol_top_n.max(1),
        &task.config.risk_profile,
        scoring_drawdown_limit(&task.config),
    );
    respect_pause_or_cancel(poller, &task.task_id).await?;
    poller
        .save_candidates_and_artifacts(&task.task_id, random_candidates.len(), &outputs)
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
    use std::collections::{BTreeMap, BTreeSet};

    candidates.sort_by(|left, right| {
        right
            .score
            .survival_valid
            .cmp(&left.score.survival_valid)
            .then_with(|| right.score.rank_score.total_cmp(&left.score.rank_score))
    });

    let mut selected = Vec::new();
    let mut selected_counts = BTreeMap::<String, usize>::new();

    for candidate in candidates
        .iter()
        .filter(|candidate| candidate.score.survival_valid)
    {
        let symbol = search_candidate_symbol(&candidate.candidate)
            .unwrap_or_else(|| candidate.candidate.candidate_id.clone());
        let count = selected_counts.entry(symbol).or_default();
        if *count >= per_symbol_top_n {
            continue;
        }
        *count += 1;
        selected.push(candidate.clone());
    }

    let selected_symbols = selected_counts.keys().cloned().collect::<BTreeSet<_>>();
    let mut selected_ids = selected
        .iter()
        .map(|candidate| candidate.candidate.candidate_id.clone())
        .collect::<BTreeSet<_>>();

    for candidate in candidates.iter() {
        if selected.len() >= min_total && !selected.is_empty() {
            break;
        }
        let symbol = search_candidate_symbol(&candidate.candidate)
            .unwrap_or_else(|| candidate.candidate.candidate_id.clone());
        if selected_symbols.contains(&symbol) {
            continue;
        }
        if selected_ids.insert(candidate.candidate.candidate_id.clone()) {
            selected.push(candidate.clone());
        }
    }

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
    outputs: Vec<CandidateOutput>,
    per_symbol_top_n: usize,
    risk_profile: &str,
    max_drawdown_pct: Option<f64>,
) -> Vec<CandidateOutput> {
    use std::collections::BTreeMap;

    let mut outputs = prefer_outputs_within_drawdown_limit(outputs, max_drawdown_pct);
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
            let parameter_rank_for_symbol = {
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
            let total_margin_budget_quote = output_total_margin_budget_quote(&output);
            let take_profit_bps = output_take_profit_bps(&output);
            let trailing_take_profit_bps = output_trailing_take_profit_bps(&output);
            let direction = output_direction(&output);
            let strategy_legs = output_strategy_legs(&output);
            let overfit_flag = output_overfit_flag(&output);
            let risk_summary_human = output_risk_summary_human(&output, risk_profile);
            let display_score = output_display_score(&output);

            output.rank = index + 1;
            output.summary = merge_json_objects(
                output.summary,
                json!({
                    "symbol": symbol,
                    "direction": direction,
                    "strategy_legs": strategy_legs,
                    "parameter_rank_for_symbol": parameter_rank_for_symbol,
                    "recommended_weight_pct": recommended_weight_pct,
                    "recommended_leverage": recommended_leverage,
                    "risk_profile": risk_profile,
                    "portfolio_group_key": portfolio_group_key,
                    "spacing_bps": spacing_bps,
                    "first_order_quote": first_order_quote,
                    "order_multiplier": order_multiplier,
                    "max_legs": max_legs,
                    "total_margin_budget_quote": total_margin_budget_quote,
                    "take_profit_bps": take_profit_bps,
                    "trailing_take_profit_bps": trailing_take_profit_bps,
                    "total_return_pct": output.total_return_pct,
                    "max_drawdown_pct": output.max_drawdown_pct,
                    "score": display_score,
                    "rank_score": output.score,
                    "overfit_flag": overfit_flag,
                    "risk_summary_human": risk_summary_human,
                    "drawdown_limit_pct": max_drawdown_pct,
                    "drawdown_limit_satisfied": max_drawdown_pct
                        .map(|limit| output.max_drawdown_pct <= limit)
                        .unwrap_or(true),
                    "artifact_path": output.artifact_path,
                }),
            );
            output
        })
        .collect()
}

fn prefer_outputs_within_drawdown_limit(
    outputs: Vec<CandidateOutput>,
    max_drawdown_pct: Option<f64>,
) -> Vec<CandidateOutput> {
    let Some(limit) = max_drawdown_pct.filter(|value| value.is_finite() && *value > 0.0) else {
        return outputs;
    };
    outputs
        .into_iter()
        .filter(|output| output.max_drawdown_pct <= limit)
        .collect()
}

fn output_display_score(output: &CandidateOutput) -> f64 {
    let return_component = (output.total_return_pct / 80.0).clamp(-1.0, 1.0) * 35.0;
    let drawdown_component = (1.0 - (output.max_drawdown_pct / 60.0).clamp(0.0, 1.0)) * 35.0;
    let trade_component = (output.trade_count as f64 / 300.0).clamp(0.0, 1.0) * 15.0;
    let survival_component = if output
        .summary
        .get("survival_passed")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        15.0
    } else {
        0.0
    };

    (return_component + drawdown_component + trade_component + survival_component).clamp(0.0, 100.0)
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

fn visible_backtest_summary(
    result: &MartingaleBacktestResult,
    candidate: &SearchCandidate,
    market_context: &MarketDataContext,
    task_config: &WorkerTaskConfig,
) -> Value {
    let equity_curve = sampled_equity_curve(&result.equity_curve, 360);
    let symbols = candidate_symbols(candidate);
    let matching_bars = market_context
        .bars
        .iter()
        .filter(|bar| symbols.contains(&bar.symbol.trim().to_uppercase()))
        .collect::<Vec<_>>();
    let matching_trades = market_context
        .trades
        .iter()
        .filter(|trade| symbols.contains(&trade.symbol.trim().to_uppercase()))
        .collect::<Vec<_>>();

    let annualized_return = annualized_return_pct(
        result.metrics.total_return_pct,
        task_config.start_ms,
        task_config.end_ms,
    );
    let backtest_years = backtest_years(task_config.start_ms, task_config.end_ms);

    json!({
        "equity_curve": equity_curve,
        "drawdown_curve": equity_curve,
        "trade_events": trade_event_rows(&result.events, 500),
        "sampled_trade_events": trade_event_rows(&result.events, 80),
        "annualized_return_pct": annualized_return,
        "backtest_years": backtest_years,
        "stop_loss_events": stop_loss_event_rows(&result.events, 100),
        "stop_count": result.metrics.stop_count,
        "max_capital_used_quote": result.metrics.max_capital_used_quote,
        "survival_passed": result.metrics.survival_passed,
        "rejection_reasons": result.rejection_reasons,
        "data_quality_score": result.metrics.data_quality_score,
        "data_coverage": {
            "interval": task_config.interval,
            "requested_start_ms": task_config.start_ms,
            "requested_end_ms": task_config.end_ms,
            "first_bar_ms": matching_bars.iter().map(|bar| bar.open_time_ms).min(),
            "last_bar_ms": matching_bars.iter().map(|bar| bar.open_time_ms).max(),
            "bar_count": matching_bars.len(),
            "agg_trade_count": matching_trades.len(),
            "used_full_minute_coverage": task_config.interval == "1m" && matching_bars.len() > 100_000,
        },
    })
}

fn sampled_equity_curve(points: &[EquityPoint], max_points: usize) -> Vec<Value> {
    if points.is_empty() || max_points == 0 {
        return Vec::new();
    }
    let step = (points.len() as f64 / max_points as f64).ceil().max(1.0) as usize;
    let mut peak = f64::MIN;
    let mut rows = Vec::new();
    for (index, point) in points.iter().enumerate() {
        peak = peak.max(point.equity_quote);
        let is_last = index + 1 == points.len();
        if index % step != 0 && !is_last {
            continue;
        }
        let drawdown = if peak > 0.0 {
            ((point.equity_quote - peak) / peak).min(0.0)
        } else {
            0.0
        };
        rows.push(json!({
            "ts": point.timestamp_ms,
            "equity": point.equity_quote,
            "drawdown": drawdown,
        }));
    }
    rows
}

fn trade_event_rows(events: &[MartingaleBacktestEvent], limit: usize) -> Vec<Value> {
    let matched = events
        .iter()
        .filter(|event| {
            matches!(
                event.event_type.as_str(),
                "entry"
                    | "safety_order"
                    | "take_profit"
                    | "stop_loss"
                    | "symbol_stop_loss"
                    | "global_stop_loss"
            )
        })
        .collect::<Vec<_>>();
    let sampled = sample_evenly(&matched, limit);
    sampled
        .into_iter()
        .map(|event| {
            json!({
                "ts": event.timestamp_ms,
                "type": event.event_type,
                "symbol": event.symbol,
                "strategy_instance_id": event.strategy_instance_id,
                "cycle_id": event.cycle_id,
                "detail": event.detail,
            })
        })
        .collect()
}

fn sample_evenly<'a, T>(items: &'a [&'a T], limit: usize) -> Vec<&'a T> {
    if limit == 0 || items.is_empty() {
        return Vec::new();
    }
    if items.len() <= limit {
        return items.to_vec();
    }
    if limit == 1 {
        return vec![items[0]];
    }
    let last_index = items.len() - 1;
    let slots = limit - 1;
    let mut indexes = std::collections::BTreeSet::new();
    for slot in 0..limit {
        indexes.insert((slot * last_index + slots / 2) / slots);
    }
    indexes.into_iter().map(|index| items[index]).collect()
}

fn annualized_return_pct(total_return_pct: f64, start_ms: i64, end_ms: i64) -> Option<f64> {
    let years = backtest_years(start_ms, end_ms)?;
    if years <= 0.0 || !total_return_pct.is_finite() {
        return None;
    }
    let growth = 1.0 + total_return_pct / 100.0;
    if growth <= 0.0 || !growth.is_finite() {
        return None;
    }
    let annualized = (growth.powf(1.0 / years) - 1.0) * 100.0;
    annualized.is_finite().then_some(annualized)
}

fn backtest_years(start_ms: i64, end_ms: i64) -> Option<f64> {
    if start_ms <= 0 || end_ms <= start_ms {
        return None;
    }
    let duration_ms = (end_ms - start_ms) as f64;
    let year_ms = 365.25 * 24.0 * 60.0 * 60.0 * 1000.0;
    Some(duration_ms / year_ms)
}

fn stop_loss_event_rows(events: &[MartingaleBacktestEvent], limit: usize) -> Vec<Value> {
    events
        .iter()
        .filter(|event| event.event_type.contains("stop_loss"))
        .take(limit)
        .map(|event| {
            json!({
                "ts": event.timestamp_ms,
                "symbol": event.symbol,
                "reason": event.event_type,
                "loss_pct": 0.0,
                "detail": event.detail,
            })
        })
        .collect()
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
    let Some(strategies) = output.config.get("strategies").and_then(Value::as_array) else {
        return Value::Null;
    };
    let directions = strategies
        .iter()
        .filter_map(|strategy| strategy.get("direction").and_then(Value::as_str))
        .collect::<std::collections::BTreeSet<_>>();
    if directions.len() > 1 {
        return Value::String("long_and_short".to_owned());
    }
    directions
        .into_iter()
        .next()
        .map(|direction| Value::String(direction.to_owned()))
        .unwrap_or(Value::Null)
}

fn output_strategy_legs(output: &CandidateOutput) -> Vec<Value> {
    output
        .config
        .get("strategies")
        .and_then(Value::as_array)
        .map(|strategies| {
            strategies
                .iter()
                .map(|strategy| {
                    json!({
                        "direction": strategy.get("direction").cloned().unwrap_or(Value::Null),
                        "spacing_bps": strategy_value_at_raw(strategy, &["spacing", "fixed_percent", "step_bps"]).cloned().unwrap_or(Value::Null),
                        "take_profit_bps": strategy_value_at_raw(strategy, &["take_profit", "percent", "bps"]).cloned().unwrap_or(Value::Null),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn strategy_value_at_raw<'a>(strategy: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut value = strategy;
    for key in path {
        value = value.get(*key)?;
    }
    Some(value)
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

fn output_total_margin_budget_quote(output: &CandidateOutput) -> Value {
    let first_order_quote = value_to_f64(&output_first_order_quote(output));
    let multiplier = value_to_f64(&output_order_multiplier(output));
    let max_legs = output_max_legs(output).as_u64();
    match (first_order_quote, multiplier, max_legs) {
        (Some(first), Some(multiplier), Some(max_legs)) => {
            let mut total = 0.0;
            let mut current = first;
            for _ in 0..max_legs {
                total += current;
                current *= multiplier;
            }
            json!(total)
        }
        _ => Value::Null,
    }
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

fn value_to_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
        .filter(|value| value.is_finite())
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
    format!(
        "{} 风险档，收益 {:.2}%，最大回撤 {:.2}%，{}。",
        risk_profile, output.total_return_pct, output.max_drawdown_pct, overfit
    )
}

fn strategy_value_at<'a>(output: &'a CandidateOutput, path: &[&str]) -> Option<&'a Value> {
    let mut value = output_strategy(output)?;
    for key in path {
        value = value.get(*key)?;
    }
    Some(value)
}

fn search_space_from_task(config: &WorkerTaskConfig) -> SearchSpace {
    let direction_mode = direction_mode_from_task(config.direction_mode.as_deref());
    SearchSpace {
        symbols: config.symbols.clone(),
        direction_mode,
        directions: directions_from_mode(config.direction_mode.as_deref()),
        market: market_kind(config.market.as_deref()),
        margin_mode: margin_mode(config.margin_mode.as_deref()),
        step_bps: search_space_u32(config, "spacing_bps")
            .or_else(|| template_u32(config, &["spacing", "step_bps"]).map(|value| vec![value]))
            .unwrap_or_else(|| vec![25, 50, 100]),
        first_order_quote: template_decimal(config, &["sizing", "first_order_quote"])
            .map(|value| vec![value])
            .or_else(|| search_space_decimal(config, "first_order_quote"))
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
        dynamic_allocation_enabled: config.dynamic_allocation_enabled.unwrap_or(
            direction_mode == shared_domain::martingale::MartingaleDirectionMode::LongAndShort,
        ),
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
    let limit = scoring_drawdown_limit(config).unwrap_or_else(|| {
        scoring_config_for_risk_profile(&config.risk_profile).max_global_drawdown_pct
    });
    let mut candidates = vec![limit * 0.64, limit * 0.8, limit, limit * 1.2];
    candidates.retain(|value| value.is_finite() && *value > 0.0);
    candidates.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
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
    let mut scoring = scoring_config_for_risk_profile(&config.risk_profile);
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

fn scoring_config_for_risk_profile(risk_profile: &str) -> ScoringConfig {
    let mut scoring = ScoringConfig::default();
    match risk_profile {
        "conservative" => {
            scoring.max_global_drawdown_pct = 12.0;
            scoring.max_strategy_drawdown_pct = 10.0;
            scoring.max_stop_count = 1;
            scoring.min_trade_count = 80;
            scoring.weight_return = 0.55;
            scoring.weight_calmar = 1.20;
            scoring.weight_sortino = 0.80;
            scoring.weight_drawdown = 2.60;
            scoring.weight_stop_frequency = 1.20;
            scoring.weight_capital_utilization = 0.10;
            scoring.weight_trade_stability = 0.45;
        }
        "aggressive" => {
            scoring.max_global_drawdown_pct = 45.0;
            scoring.max_strategy_drawdown_pct = 45.0;
            scoring.max_stop_count = 8;
            scoring.min_trade_count = 50;
            scoring.weight_return = 1.60;
            scoring.weight_calmar = 0.55;
            scoring.weight_sortino = 0.35;
            scoring.weight_drawdown = 0.60;
            scoring.weight_stop_frequency = 0.35;
            scoring.weight_capital_utilization = 0.45;
            scoring.weight_trade_stability = 0.20;
        }
        _ => {
            scoring.max_global_drawdown_pct = 25.0;
            scoring.max_strategy_drawdown_pct = 22.0;
            scoring.max_stop_count = 3;
            scoring.min_trade_count = 70;
            scoring.weight_return = 1.00;
            scoring.weight_calmar = 0.90;
            scoring.weight_sortino = 0.55;
            scoring.weight_drawdown = 1.25;
            scoring.weight_stop_frequency = 0.70;
            scoring.weight_capital_utilization = 0.25;
            scoring.weight_trade_stability = 0.30;
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
    use shared_domain::martingale::MartingaleDirectionMode::{LongAndShort, LongOnly, ShortOnly};
    match mode {
        Some("long_only") => LongOnly,
        Some("short_only") => ShortOnly,
        Some("long_and_short") => LongAndShort,
        _ => LongAndShort,
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

fn scoring_drawdown_limit(config: &WorkerTaskConfig) -> Option<f64> {
    config
        .scoring
        .as_ref()
        .and_then(|value| value.get("max_drawdown_pct"))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn leverage_values(range: Option<[u32; 2]>) -> Vec<u32> {
    let Some([left, right]) = range else {
        return (2..=10).collect();
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
        screened_count: usize,
        outputs: &[CandidateOutput],
    ) -> Result<(), String> {
        self.repo
            .append_task_event(
                task_id,
                "screening_completed",
                json!({ "screened_count": screened_count, "selected_count": outputs.len() }),
            )
            .map_err(|error| format!("append screening event: {error}"))?;
        for output in outputs {
            self.repo
                .save_candidate_with_artifact(
                    NewBacktestCandidateRecord {
                        task_id: task_id.to_owned(),
                        status: "ready".to_owned(),
                        rank: output.rank as i32,
                        config: output.config.clone(),
                        summary: merge_json_objects(
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
                    },
                    "summary",
                    output.artifact_path.clone(),
                    json!({
                        "checksum_sha256": output.checksum_sha256,
                        "source_candidate_id": output.candidate_id,
                    }),
                )
                .map_err(|error| format!("save candidate artifact bundle: {error}"))?;
        }
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
    use std::path::PathBuf;

    use shared_domain::martingale::{
        MartingaleDirection, MartingaleDirectionMode, MartingalePortfolioConfig,
        MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel,
        MartingaleStrategyConfig, MartingaleTakeProfitModel,
    };

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

    fn evaluated_candidate(symbol: &str, id: &str, score: f64) -> EvaluatedCandidate {
        evaluated_candidate_with_validity(symbol, id, score, true)
    }

    fn evaluated_candidate_with_validity(
        symbol: &str,
        id: &str,
        score: f64,
        survival_valid: bool,
    ) -> EvaluatedCandidate {
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
                survival_valid,
                rank_score: score,
                raw_score: score,
                rejection_reasons: if survival_valid {
                    Vec::new()
                } else {
                    vec!["global_drawdown_exceeded".to_owned()]
                },
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
    fn candidate_outputs_keep_top_ten_per_symbol_and_enrich_summary() {
        let mut outputs = Vec::new();
        for index in 1..=11 {
            outputs.push(candidate_output(
                "BTCUSDT",
                &format!("btc-{index}"),
                index,
                100.0 - index as f64,
                3,
            ));
        }
        outputs.push(candidate_output("ETHUSDT", "eth-1", 1, 30.0, 2));

        let selected =
            select_top_outputs_per_symbol(outputs, default_per_symbol_top_n(), "balanced", None);

        assert_eq!(default_per_symbol_top_n(), 10);
        assert_eq!(selected.len(), 11);
        assert!(selected
            .iter()
            .any(|output| output.candidate_id == "btc-10"));
        assert!(!selected
            .iter()
            .any(|output| output.candidate_id == "btc-11"));

        let btc_first = selected
            .iter()
            .find(|output| output.candidate_id == "btc-1")
            .unwrap();
        assert_eq!(btc_first.summary["symbol"], "BTCUSDT");
        assert_eq!(btc_first.summary["parameter_rank_for_symbol"], 1);
        assert_eq!(btc_first.summary["recommended_weight_pct"], 10.0);
        assert_eq!(btc_first.summary["recommended_leverage"], 3);
        assert_eq!(btc_first.summary["total_margin_budget_quote"], 81.25);
        assert_eq!(btc_first.summary["risk_profile"], "balanced");
    }

    #[test]
    fn refinement_selection_prioritizes_drawdown_valid_candidates_over_high_return_invalid() {
        let high_return_invalid = evaluated_candidate_with_validity(
            "BTCUSDT",
            "high-return-invalid",
            1.0e12 + 95.0,
            false,
        );
        let lower_return_valid =
            evaluated_candidate_with_validity("BTCUSDT", "lower-return-valid", 1.0e12 + 10.0, true);

        let selected = select_refinement_candidates_per_symbol(
            vec![high_return_invalid, lower_return_valid],
            1,
            1,
        );

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].candidate.candidate_id, "lower-return-valid");
        assert!(selected[0].score.survival_valid);
    }

    #[test]
    fn output_selection_returns_empty_when_all_refined_candidates_exceed_drawdown_limit() {
        let mut high_risk = candidate_output("BTCUSDT", "high-risk", 1, 99.0, 3);
        high_risk.max_drawdown_pct = 35.0;

        let selected = select_top_outputs_per_symbol(vec![high_risk], 10, "balanced", Some(20.0));

        assert!(selected.is_empty());
    }

    #[test]
    fn output_selection_marks_drawdown_limit_status() {
        let mut output = candidate_output("BTCUSDT", "valid", 1, 9.0, 3);
        output.max_drawdown_pct = 18.0;

        let selected = select_top_outputs_per_symbol(vec![output], 1, "balanced", Some(20.0));

        assert_eq!(selected[0].summary["drawdown_limit_pct"], 20.0);
        assert_eq!(selected[0].summary["drawdown_limit_satisfied"], true);
    }

    #[test]
    fn output_selection_prefers_candidates_within_drawdown_limit_when_available() {
        let mut high_risk = candidate_output("BTCUSDT", "high-risk", 1, 95.0, 3);
        high_risk.max_drawdown_pct = 45.0;
        let low_risk = candidate_output("BTCUSDT", "low-risk", 2, 10.0, 3);
        let outputs = vec![high_risk, low_risk];

        let selected = select_top_outputs_per_symbol(outputs, 10, "balanced", Some(20.0));

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].candidate_id, "low-risk");
        assert!(selected[0].max_drawdown_pct <= 20.0);
    }

    #[test]
    fn long_and_short_task_search_space_uses_combined_direction_mode() {
        let config: WorkerTaskConfig = serde_json::from_value(serde_json::json!({
            "symbols": ["BTCUSDT"],
            "random_seed": 7,
            "random_candidates": 12,
            "intelligent_rounds": 3,
            "top_n": 10,
            "direction_mode": "long_and_short"
        }))
        .expect("config");

        let space = search_space_from_task(&config);

        assert_eq!(
            space.direction_mode,
            shared_domain::martingale::MartingaleDirectionMode::LongAndShort
        );
        assert_eq!(space.directions.len(), 2);
    }

    #[test]
    fn dynamic_long_short_defaults_generate_short_stop_candidates_around_drawdown_limit() {
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
    fn risk_profiles_apply_distinct_scoring_limits_and_weights() {
        let conservative = WorkerTaskConfig {
            risk_profile: "conservative".to_owned(),
            ..WorkerTaskConfig::default()
        };
        let balanced = WorkerTaskConfig {
            risk_profile: "balanced".to_owned(),
            ..WorkerTaskConfig::default()
        };
        let aggressive = WorkerTaskConfig {
            risk_profile: "aggressive".to_owned(),
            ..WorkerTaskConfig::default()
        };

        let conservative_scoring = scoring_config_from_task(&conservative);
        let balanced_scoring = scoring_config_from_task(&balanced);
        let aggressive_scoring = scoring_config_from_task(&aggressive);

        assert!(
            conservative_scoring.max_global_drawdown_pct < balanced_scoring.max_global_drawdown_pct
        );
        assert!(
            balanced_scoring.max_global_drawdown_pct < aggressive_scoring.max_global_drawdown_pct
        );
        assert!(conservative_scoring.weight_drawdown > balanced_scoring.weight_drawdown);
        assert!(balanced_scoring.weight_drawdown > aggressive_scoring.weight_drawdown);
        assert!(aggressive_scoring.weight_return > balanced_scoring.weight_return);
        assert!(balanced_scoring.weight_return > conservative_scoring.weight_return);
    }

    #[test]
    fn futures_default_search_tests_every_leverage_from_two_to_ten() {
        let config = WorkerTaskConfig {
            market: Some("usd_m_futures".to_owned()),
            leverage_range: None,
            ..WorkerTaskConfig::default()
        };

        let space = search_space_from_task(&config);

        assert_eq!(space.leverage, (2..=10).collect::<Vec<_>>());
    }

    #[test]
    fn preset_search_space_keeps_first_order_fixed_from_template() {
        let config: WorkerTaskConfig = serde_json::from_value(serde_json::json!({
            "symbols": ["BTCUSDT"],
            "random_seed": 7,
            "random_candidates": 12,
            "intelligent_rounds": 3,
            "top_n": 10,
            "market": "usd_m_futures",
            "martingale_template": {
                "sizing": {
                    "first_order_quote": 10
                },
                "search_space": {
                    "first_order_quote": [8, 10, 15],
                    "spacing_bps": [80, 120],
                    "order_multiplier": [1.4, 2],
                    "take_profit_bps": [80, 100],
                    "max_legs": [4, 6]
                }
            }
        }))
        .expect("worker task config");

        let space = search_space_from_task(&config);

        assert_eq!(space.first_order_quote, vec![Decimal::new(10, 0)]);
    }

    #[test]
    fn selected_outputs_include_ui_required_summary_fields() {
        let mut output = candidate_output("BTCUSDT", "btc-1", 1, 1_000_000_090.0, 3);
        output.summary = serde_json::json!({
            "equity_curve": [{"ts": 1_000, "equity": 1_000.0, "drawdown": 0.0}],
            "drawdown_curve": [{"ts": 1_000, "equity": 1_000.0, "drawdown": 0.0}],
            "trade_events": [{"ts": 1_000, "type": "entry", "symbol": "BTCUSDT", "detail": "notional_quote=10"}],
            "sampled_trade_events": [{"ts": 1_000, "type": "entry", "symbol": "BTCUSDT", "detail": "notional_quote=10"}],
            "data_coverage": {"interval": "1m", "bar_count": 1},
            "annualized_return_pct": 12.3,
            "backtest_years": 1.0
        });
        let outputs = vec![output];

        let selected = select_top_outputs_per_symbol(outputs, 5, "balanced", None);
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
            "parameter_rank_for_symbol",
            "risk_profile",
            "total_return_pct",
            "max_drawdown_pct",
            "score",
            "overfit_flag",
            "risk_summary_human",
            "equity_curve",
            "drawdown_curve",
            "trade_events",
            "sampled_trade_events",
            "data_coverage",
            "annualized_return_pct",
            "backtest_years",
        ] {
            assert!(
                summary.contains_key(field),
                "missing summary field: {field}"
            );
        }
        assert_eq!(summary["artifact_path"], output.artifact_path);
        assert_eq!(summary["equity_curve"].as_array().unwrap().len(), 1);
        let ui_score = summary["score"].as_f64().unwrap();
        assert!(
            (0.0..=100.0).contains(&ui_score),
            "score shown to users must be a 0-100 rating, got {ui_score}"
        );
        assert_eq!(summary["rank_score"].as_f64().unwrap(), output.score);
    }

    #[test]
    fn trade_event_sampling_spans_entire_backtest_window() {
        let events = (0..1_000)
            .map(|index| MartingaleBacktestEvent {
                timestamp_ms: index * 60_000,
                event_type: "entry".to_owned(),
                symbol: "BTCUSDT".to_owned(),
                strategy_instance_id: "btc-long".to_owned(),
                cycle_id: Some(format!("cycle-{index}")),
                detail: format!("notional_quote={index}"),
            })
            .collect::<Vec<_>>();

        let sampled = trade_event_rows(&events, 80);

        assert_eq!(sampled.len(), 80);
        assert_eq!(sampled.first().unwrap()["ts"], 0);
        assert_eq!(sampled.last().unwrap()["ts"], 999 * 60_000);
    }

    #[test]
    fn annualized_return_uses_backtest_duration() {
        let annualized = annualized_return_pct(100.0, 1_672_531_200_000, 1_704_067_199_999)
            .expect("valid annualized return");

        assert!(annualized > 100.0);
        assert!(annualized < 101.0);
    }

    #[test]
    fn worker_defaults_to_one_minute_full_coverage_interval() {
        let config = WorkerTaskConfig::default();

        assert_eq!(config.interval, "1m");
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
        assert_eq!(space.first_order_quote, vec![Decimal::new(15, 0)]);
        assert_eq!(
            space.multiplier,
            vec![Decimal::new(14, 1), Decimal::new(2, 0)]
        );
        assert_eq!(space.take_profit_bps, vec![80, 90]);
        assert_eq!(space.leverage, vec![2, 3, 4]);
        assert_eq!(space.max_legs, vec![6, 8]);
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
