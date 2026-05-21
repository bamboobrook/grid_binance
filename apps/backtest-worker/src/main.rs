use std::{env, path::PathBuf, time::Duration};

#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

use backtest_engine::{
    artifacts::{verify_artifact, write_task_json_artifact},
    intelligent_search::{intelligent_search, EvaluatedCandidate, IntelligentSearchConfig},
    market_data::{AggTrade, KlineBar, MarketDataSource},
    martingale::{
        kline_engine::run_kline_screening, metrics::MartingaleBacktestResult,
        scoring::ScoringConfig, trade_engine::run_trade_refinement,
    },
    portfolio_search::{build_portfolio_top3, build_portfolio_top_n_v2},
    search::{
        drawdown_limit_sequence, fine_space_around, CoarseParameterPoint, SearchCandidate,
        SearchSpace, StagedMartingaleSearchSpace,
    },
    sqlite_market_data::SqliteMarketDataSource,
};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared_db::{
    BacktestCandidateRecord, BacktestRepository, NewBacktestCandidateRecord, SharedDb,
};
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleEntryTrigger,
    MartingaleIndicatorConfig, MartingaleMarginMode, MartingaleMarketKind, MartingaleSizingModel,
    MartingaleSpacingModel, MartingaleStopLossModel, MartingaleStrategyConfig,
    MartingaleTakeProfitModel,
};

const DEFAULT_MAX_THREADS: usize = 2;
const DEFAULT_POLL_MS: u64 = 5_000;
const DEFAULT_TOP_N: usize = 3;
const SCREENING_MAX_BARS_PER_SYMBOL: usize = 180_000;
const SCREENING_RECENT_BARS_PER_SYMBOL: usize = 120_000;
const SCREENING_EARLY_BARS_PER_SYMBOL: usize = 30_000;
const SCREENING_MIDDLE_BARS_PER_SYMBOL: usize = 30_000;

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
    #[serde(default = "default_random_seed")]
    random_seed: u64,
    #[serde(default = "default_random_candidates")]
    random_candidates: usize,
    #[serde(default = "default_intelligent_rounds")]
    intelligent_rounds: usize,
    #[serde(default = "default_top_n")]
    top_n: usize,
    #[serde(default = "default_per_symbol_top_n")]
    per_symbol_top_n: usize,
    #[serde(default = "default_portfolio_top_n")]
    portfolio_top_n: usize,
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
    search_space: Option<Value>,
    #[serde(default)]
    scoring: Option<Value>,
    #[serde(default)]
    extended_universe: Option<bool>,
    #[serde(default)]
    search_mode: Option<String>,
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
            portfolio_top_n: default_portfolio_top_n(),
            risk_profile: default_risk_profile(),
            market: None,
            margin_mode: None,
            direction_mode: None,
            leverage_range: None,
            martingale_template: None,
            search_space: None,
            scoring: None,
            extended_universe: None,
            search_mode: None,
            interval: default_interval(),
            start_ms: 0,
            end_ms: 0,
        }
    }
}

fn default_interval() -> String {
    "1h".to_owned()
}

fn default_random_seed() -> u64 {
    1
}
fn default_random_candidates() -> usize {
    16
}
fn default_intelligent_rounds() -> usize {
    1
}
fn default_top_n() -> usize {
    10
}
fn default_per_symbol_top_n() -> usize {
    10
}

fn default_portfolio_top_n() -> usize {
    3
}

fn default_risk_profile() -> String {
    "balanced".to_owned()
}

fn default_expanded_universe_symbols() -> Vec<String> {
    [
        "BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "DOGEUSDT", "XRPUSDT", "ADAUSDT", "ZECUSDT",
        "DASHUSDT", "NEARUSDT", "BCHUSDT", "LINKUSDT", "AVAXUSDT", "UNIUSDT", "FILUSDT", "DOTUSDT",
        "AAVEUSDT", "INJUSDT",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn effective_search_symbols(config: &WorkerTaskConfig) -> Vec<String> {
    // Explicit user-provided symbols always take priority.
    // A single-element vec!["BTCUSDT"] is treated as the Default and therefore
    // *not* explicit — it means the caller didn't supply symbols.
    let has_explicit_symbols = !config.symbols.is_empty()
        && !(config.symbols.len() == 1 && config.symbols[0] == "BTCUSDT");
    if has_explicit_symbols {
        return config.symbols.clone();
    }
    if config.extended_universe.unwrap_or(false) {
        return default_expanded_universe_symbols();
    }
    config.symbols.clone()
}

fn should_use_profit_optimized_v2(config: &WorkerTaskConfig) -> bool {
    config.extended_universe.unwrap_or(false)
        || config.search_mode.as_deref() == Some("profit_optimized_v2")
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

fn martingale_search_timeout_error(
    symbol: &str,
    direction_mode: &str,
    estimated_screenings: usize,
    timeout_secs: u64,
) -> String {
    format!(
        "martingale search timed out: symbol={} direction_mode={} estimated_screenings={} timeout_secs={}",
        symbol, direction_mode, estimated_screenings, timeout_secs
    )
}

fn bounded_parallel_width(max_threads: usize) -> usize {
    max_threads.max(1)
}

fn long_short_search_timeout_secs(
    task: &WorkerTaskConfig,
    coarse_candidate_count: usize,
    max_threads: usize,
) -> u64 {
    let survivor_count = long_short_survivor_limit(task).min(coarse_candidate_count);
    let fine_candidate_count = task.random_candidates.max(12);
    let estimated_screenings = coarse_candidate_count + survivor_count * fine_candidate_count;
    let threads = bounded_parallel_width(max_threads);
    let estimated_parallel_batches = (estimated_screenings + threads - 1) / threads;
    (estimated_parallel_batches as u64 * 90).clamp(600, 3_600)
}

fn screen_candidates_bounded_parallel<F>(
    candidates: Vec<SearchCandidate>,
    max_threads: usize,
    evaluator: F,
) -> Vec<(EvaluatedCandidate, CandidateRejectionSample)>
where
    F: Fn(SearchCandidate) -> (EvaluatedCandidate, CandidateRejectionSample) + Sync,
{
    let width = bounded_parallel_width(max_threads);
    if candidates.is_empty() {
        return Vec::new();
    }
    if width == 1 || candidates.len() == 1 {
        return candidates.into_iter().map(evaluator).collect::<Vec<_>>();
    }

    let indexed = candidates.into_iter().enumerate().collect::<Vec<_>>();
    let chunk_size = (indexed.len() + width - 1) / width;
    let mut indexed_results = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in indexed.chunks(chunk_size.max(1)) {
            let evaluator_ref = &evaluator;
            let chunk_items = chunk.to_vec();
            handles.push(scope.spawn(move || {
                chunk_items
                    .into_iter()
                    .map(|(index, candidate)| (index, evaluator_ref(candidate)))
                    .collect::<Vec<_>>()
            }));
        }

        let mut merged = Vec::new();
        for handle in handles {
            merged.extend(handle.join().expect("candidate screening thread panicked"));
        }
        merged
    });

    indexed_results.sort_by_key(|(index, _)| *index);
    indexed_results
        .into_iter()
        .map(|(_index, result)| result)
        .collect()
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
    used_drawdown_limit_pct: f64,
    risk_relaxed: bool,
    total_return_pct: f64,
    max_drawdown_pct: f64,
    trade_count: u64,
    annualized_return_pct: Option<f64>,
    return_drawdown_ratio: Option<f64>,
    planned_margin_quote: Option<f64>,
    max_leverage_used: Option<f64>,
    equity_curve: Vec<backtest_engine::martingale::metrics::EquityPoint>,
    drawdown_curve: Vec<backtest_engine::martingale::metrics::DrawdownPoint>,
    trades_preview: Vec<backtest_engine::martingale::metrics::MartingaleTradeDetail>,
}

#[derive(Debug, Clone, Serialize)]
struct CandidateRejectionSample {
    candidate_id: String,
    symbol: String,
    direction_mode: String,
    total_return_pct: Option<f64>,
    max_drawdown_pct: Option<f64>,
    trade_count: usize,
    survival_valid: bool,
    rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CandidateRejectionDiagnostics {
    total: usize,
    survival_valid_count: usize,
    negative_return_count: usize,
    drawdown_rejected_count: usize,
    zero_trade_count: usize,
    best_by_return: Vec<CandidateRejectionSample>,
    lowest_drawdown: Vec<CandidateRejectionSample>,
}

impl CandidateRejectionDiagnostics {
    fn from_samples(samples: Vec<CandidateRejectionSample>) -> Self {
        let total = samples.len();
        let survival_valid_count = samples.iter().filter(|s| s.survival_valid).count();
        let negative_return_count = samples
            .iter()
            .filter(|s| s.total_return_pct.map(|v| v <= 0.0).unwrap_or(false))
            .count();
        let drawdown_rejected_count = samples
            .iter()
            .filter(|s| s.total_return_pct.map(|v| v > 0.0).unwrap_or(false) && !s.survival_valid)
            .count();
        let zero_trade_count = samples.iter().filter(|s| s.trade_count == 0).count();

        let mut best_by_return = samples.clone();
        best_by_return.sort_by(|a, b| {
            b.total_return_pct
                .unwrap_or(f64::NEG_INFINITY)
                .total_cmp(&a.total_return_pct.unwrap_or(f64::NEG_INFINITY))
        });
        best_by_return.truncate(5);

        let mut lowest_drawdown = samples;
        lowest_drawdown.sort_by(|a, b| {
            a.max_drawdown_pct
                .unwrap_or(f64::INFINITY)
                .total_cmp(&b.max_drawdown_pct.unwrap_or(f64::INFINITY))
        });
        lowest_drawdown.truncate(5);

        Self {
            total,
            survival_valid_count,
            negative_return_count,
            drawdown_rejected_count,
            zero_trade_count,
            best_by_return,
            lowest_drawdown,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SearchWorkEstimate {
    generated_candidates_per_symbol: usize,
    max_screenings_per_symbol: usize,
}

fn estimate_staged_search_work_for_task(config: &WorkerTaskConfig) -> SearchWorkEstimate {
    let direction_mode = config.direction_mode.as_deref().unwrap_or("long");
    let staged = StagedMartingaleSearchSpace::for_profile(&config.risk_profile, direction_mode);

    // Apply user search_space overrides if present
    let leverage = search_space_u32(config, "leverage").unwrap_or_else(|| staged.leverage.clone());
    let spacing_bps =
        search_space_u32(config, "spacing_bps").unwrap_or_else(|| staged.spacing_bps.clone());
    let order_multiplier = search_space_f64(config, "order_multiplier")
        .unwrap_or_else(|| staged.order_multiplier.clone());
    let max_legs = search_space_u32(config, "max_legs").unwrap_or_else(|| staged.max_legs.clone());
    let take_profit_bps = search_space_u32(config, "take_profit_bps")
        .unwrap_or_else(|| staged.take_profit_bps.clone());
    let tail_stop_bps =
        search_space_u32(config, "tail_stop_bps").unwrap_or_else(|| staged.tail_stop_bps.clone());

    let weight_count = if direction_mode == "long_short" || direction_mode == "long_and_short" {
        search_space_long_short_weights(config)
            .map(|w| w.len())
            .unwrap_or_else(|| staged.long_short_weight_pct.len())
            .max(1)
    } else {
        1
    };

    let generated = leverage.len().max(1)
        * spacing_bps.len().max(1)
        * order_multiplier.len().max(1)
        * max_legs.len().max(1)
        * take_profit_bps.len().max(1)
        * tail_stop_bps.len().max(1)
        * weight_count;

    let requested_cap = config.random_candidates.max(1) * config.intelligent_rounds.max(1);
    let cap = if (direction_mode == "long_short" || direction_mode == "long_and_short")
        && config.search_space.is_none()
    {
        requested_cap
            .max(config.per_symbol_top_n.max(10) * 40)
            .min(700)
    } else {
        requested_cap
    };
    SearchWorkEstimate {
        generated_candidates_per_symbol: generated,
        max_screenings_per_symbol: generated.min(cap.max(1)),
    }
}

#[derive(Debug, Clone, PartialEq)]
struct EvaluatedCandidateWithDrawdown {
    candidate: EvaluatedCandidate,
    used_drawdown_limit_pct: f64,
    risk_relaxed: bool,
    screening_total_return_pct: Option<f64>,
    screening_max_drawdown_pct: Option<f64>,
    screening_trade_count: Option<usize>,
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

fn run_profit_first_staged_search(
    context: &MarketDataContext,
    symbol: &str,
    task: &WorkerTaskConfig,
    scoring: &ScoringConfig,
    _drawdown_limit_pct: f64,
    max_threads: usize,
) -> Result<(Vec<EvaluatedCandidate>, Vec<CandidateRejectionSample>), String> {
    let direction_mode = task.direction_mode.as_deref().unwrap_or("long");
    let coarse_space = if should_use_profit_optimized_v2(task) {
        StagedMartingaleSearchSpace::profit_optimized_v2(&task.risk_profile, direction_mode)
    } else {
        StagedMartingaleSearchSpace::for_profile(&task.risk_profile, direction_mode)
    };

    // When direction_mode is long_short, use generate_staged_candidates_for_symbol
    // which correctly builds LongAndShort candidates with both long and short legs.
    // intelligent_search only produces single-direction candidates because it picks
    // one direction from SearchSpace.directions at a time.
    if direction_mode == "long_short" || direction_mode == "long_and_short" {
        return run_long_short_staged_search(
            context,
            symbol,
            task,
            &coarse_space,
            scoring,
            max_threads,
        );
    }

    let search_space = search_space_from_staged(&coarse_space, symbol, task);
    let coarse_candidates = intelligent_search(
        &search_space,
        &IntelligentSearchConfig {
            seed: task.random_seed,
            random_round_size: task.random_candidates.max(1),
            max_rounds: task.intelligent_rounds.max(1),
            max_candidates: task.random_candidates.max(1) * task.intelligent_rounds.max(1),
            survivor_percentile: 0.25,
            timeout: None,
            scoring: scoring.clone(),
        },
        None,
        |candidate| {
            let overridden = apply_task_overrides_to_candidate(candidate.clone(), task);
            run_candidate_kline_screening(&overridden, context)
        },
    )?;

    let mut rejection_samples: Vec<CandidateRejectionSample> = coarse_candidates
        .candidates
        .iter()
        .map(|c| rejection_sample_from_evaluated(c, symbol, direction_mode))
        .collect();

    let survivors: Vec<_> = coarse_candidates
        .candidates
        .into_iter()
        .filter(|c| c.score.survival_valid)
        .take(24)
        .collect();

    let mut refined = Vec::new();
    for survivor in &survivors {
        let parameter_point = coarse_parameter_point_from_candidate(&survivor.candidate);
        let fine_space = fine_space_around(&parameter_point);
        let fine_search_space = search_space_from_staged(&fine_space, symbol, task);
        let fine_candidates = intelligent_search(
            &fine_search_space,
            &IntelligentSearchConfig {
                seed: task.random_seed.wrapping_add(1),
                random_round_size: task.random_candidates.max(1),
                max_rounds: 1,
                max_candidates: task.random_candidates.max(1),
                survivor_percentile: 0.25,
                timeout: None,
                scoring: scoring.clone(),
            },
            None,
            |candidate| {
                let overridden = apply_task_overrides_to_candidate(candidate.clone(), task);
                run_candidate_kline_screening(&overridden, context)
            },
        )?;
        for c in &fine_candidates.candidates {
            rejection_samples.push(rejection_sample_from_evaluated(c, symbol, direction_mode));
        }
        refined.extend(fine_candidates.candidates);
    }

    refined.sort_by(|a, b| b.score.rank_score.total_cmp(&a.score.rank_score));
    refined.truncate(task.per_symbol_top_n.max(10));
    Ok((refined, rejection_samples))
}

fn long_short_drawdown_limit_sequence(risk_profile: &str) -> Vec<f64> {
    // Preserve the same first-pass risk standards as single-direction search.
    // The second value is only a controlled fallback; do not jump to 40/50/60.
    match risk_profile {
        "conservative" => vec![20.0, 25.0],
        "balanced" => vec![25.0, 30.0],
        "aggressive" => vec![30.0, 35.0],
        _ => vec![25.0, 30.0],
    }
}

fn apply_search_space_overrides_to_staged(
    staged: &StagedMartingaleSearchSpace,
    task: &WorkerTaskConfig,
) -> StagedMartingaleSearchSpace {
    let direction_mode = task.direction_mode.as_deref().unwrap_or("long");
    let is_long_short = direction_mode == "long_short" || direction_mode == "long_and_short";

    let user_spacing = search_space_u32(task, "spacing_bps");
    let user_multiplier = search_space_f64(task, "order_multiplier");
    let user_max_legs = search_space_u32(task, "max_legs");
    let user_take_profit = search_space_u32(task, "take_profit_bps");

    // For long_short auto-search, user-provided values are anchors, not the whole
    // search universe. Expand around them so the system can discover better values.
    let spacing_bps = if let Some(ref vals) = user_spacing {
        if is_long_short && vals.len() <= 2 {
            let mut expanded: Vec<u32> = vals.clone();
            for &v in vals {
                for &neighbor in &[
                    v.saturating_sub(40),
                    v + 60,
                    v + 120,
                    v + 180,
                    v + 240,
                    v + 300,
                    v + 420,
                    v + 600,
                    v + 840,
                ] {
                    if neighbor >= 50 && neighbor <= 1200 && !expanded.contains(&neighbor) {
                        expanded.push(neighbor);
                    }
                }
            }
            expanded.sort();
            expanded.dedup();
            if expanded.len() < 8 {
                let defaults = &[80_u32, 120, 180, 240, 360, 480, 720, 960];
                for &d in defaults {
                    if !expanded.contains(&d) {
                        expanded.push(d);
                    }
                }
                expanded.sort();
            }
            expanded
        } else {
            vals.clone()
        }
    } else {
        staged.spacing_bps.clone()
    };

    let order_multiplier = if let Some(ref vals) = user_multiplier {
        if is_long_short && vals.len() <= 2 {
            let mut expanded: Vec<f64> = vals.clone();
            for &v in vals {
                for &neighbor in &[v - 0.20, v - 0.10, v - 0.05, v + 0.05, v + 0.15] {
                    if neighbor >= 1.05
                        && neighbor <= 2.0
                        && !expanded.iter().any(|e| (e - neighbor).abs() < 0.001)
                    {
                        expanded.push(neighbor);
                    }
                }
            }
            expanded.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            expanded.dedup_by(|a, b| (*a - *b).abs() < 0.001);
            if expanded.len() < 4 {
                expanded = staged.order_multiplier.clone();
            }
            expanded
        } else {
            vals.clone()
        }
    } else {
        staged.order_multiplier.clone()
    };

    let max_legs = if let Some(ref vals) = user_max_legs {
        if is_long_short && vals.len() <= 2 {
            let mut expanded: Vec<u32> = vals.clone();
            for &v in vals {
                for &neighbor in &[v.saturating_sub(1), v + 1, v + 2] {
                    if neighbor >= 2 && neighbor <= 8 && !expanded.contains(&neighbor) {
                        expanded.push(neighbor);
                    }
                }
            }
            expanded.sort();
            if expanded.len() < 3 {
                expanded = staged.max_legs.clone();
            }
            expanded
        } else {
            vals.clone()
        }
    } else {
        staged.max_legs.clone()
    };

    let take_profit_bps = if let Some(ref vals) = user_take_profit {
        if is_long_short && vals.len() <= 2 {
            let mut expanded: Vec<u32> = vals.clone();
            for &v in vals {
                for &neighbor in &[v.saturating_sub(10), v + 20, v + 40, v + 80, v + 140] {
                    if neighbor >= 40 && neighbor <= 500 && !expanded.contains(&neighbor) {
                        expanded.push(neighbor);
                    }
                }
            }
            expanded.sort();
            expanded.dedup();
            if expanded.len() < 6 {
                let defaults = &[50_u32, 60, 80, 100, 140, 200];
                for &d in defaults {
                    if !expanded.contains(&d) {
                        expanded.push(d);
                    }
                }
                expanded.sort();
            }
            expanded
        } else {
            vals.clone()
        }
    } else {
        staged.take_profit_bps.clone()
    };

    let tail_stop_bps = if let Some(ref vals) = search_space_u32(task, "tail_stop_bps") {
        if is_long_short && vals.len() <= 2 {
            let mut expanded: Vec<u32> = vals.clone();
            for &v in vals {
                for &neighbor in &[
                    v.saturating_sub(1400),
                    v.saturating_sub(1000),
                    v.saturating_sub(600),
                    v + 600,
                    v + 1200,
                ] {
                    if neighbor >= 400 && neighbor <= 4000 && !expanded.contains(&neighbor) {
                        expanded.push(neighbor);
                    }
                }
            }
            expanded.sort();
            expanded.dedup();
            if expanded.len() < 6 {
                let defaults = &[600_u32, 1000, 1400, 2000, 2600, 3200];
                for &d in defaults {
                    if !expanded.contains(&d) {
                        expanded.push(d);
                    }
                }
                expanded.sort();
            }
            expanded
        } else {
            vals.clone()
        }
    } else {
        staged.tail_stop_bps.clone()
    };

    let long_short_weight_pct = if let Some(ref vals) = search_space_long_short_weights(task) {
        if is_long_short && vals.len() <= 2 {
            let mut expanded: Vec<(u32, u32)> = vals.clone();
            for &(l, s) in vals.iter() {
                let neighbors = [
                    (l.saturating_add(10), s.saturating_sub(10)),
                    (l.saturating_sub(10), s.saturating_add(10)),
                    (l.saturating_add(20), s.saturating_sub(20)),
                    (l.saturating_sub(20), s.saturating_add(20)),
                ];
                for &(nl, ns) in &neighbors {
                    let pair = (nl, ns);
                    if !expanded.contains(&pair) && nl + ns == 100 {
                        expanded.push(pair);
                    }
                }
            }
            if expanded.len() < 5 {
                expanded = staged.long_short_weight_pct.clone();
            }
            expanded
        } else {
            vals.clone()
        }
    } else {
        staged.long_short_weight_pct.clone()
    };

    let leverage = if let Some(ref vals) = search_space_u32(task, "leverage") {
        if is_long_short && vals.len() <= 2 {
            let mut expanded: Vec<u32> = vals.clone();
            for &v in vals {
                for &neighbor in &[v + 1, v + 2, v + 3] {
                    if neighbor <= 10 && !expanded.contains(&neighbor) {
                        expanded.push(neighbor);
                    }
                }
            }
            expanded.sort();
            if expanded.len() < 3 {
                expanded = staged.leverage.clone();
            }
            expanded
        } else {
            vals.clone()
        }
    } else {
        staged.leverage.clone()
    };

    StagedMartingaleSearchSpace {
        leverage,
        spacing_bps,
        order_multiplier,
        max_legs,
        take_profit_bps,
        tail_stop_bps,
        long_short_weight_pct,
    }
}

fn generate_long_short_candidates_for_task(
    symbol: &str,
    task: &WorkerTaskConfig,
    staged: &StagedMartingaleSearchSpace,
) -> Result<Vec<SearchCandidate>, String> {
    use backtest_engine::search::generate_staged_candidates_for_symbol;

    let effective_staged = apply_search_space_overrides_to_staged(staged, task);
    let requested_cap = task.random_candidates.max(1) * task.intelligent_rounds.max(1);
    let cap = if task.search_space.is_some() {
        if requested_cap >= task.per_symbol_top_n.max(10) {
            requested_cap
                .max(task.per_symbol_top_n.max(10) * 8)
                .min(700)
        } else {
            requested_cap
        }
    } else {
        requested_cap
            .max(task.per_symbol_top_n.max(10) * 40)
            .min(700)
    };
    let candidates =
        generate_staged_candidates_for_symbol(symbol, "long_short", &effective_staged, 20_000)?;
    Ok(stratified_long_short_candidates(candidates, cap))
}

fn stratified_long_short_candidates(
    candidates: Vec<SearchCandidate>,
    limit: usize,
) -> Vec<SearchCandidate> {
    if candidates.len() <= limit {
        return candidates;
    }

    let mut globally_ranked = candidates.clone();
    globally_ranked.sort_by(|left, right| {
        candidate_profit_priority(right).total_cmp(&candidate_profit_priority(left))
    });

    let profit_quota = (limit / 3).max(1);
    let mut selected = Vec::with_capacity(limit);
    let mut selected_ids = std::collections::BTreeSet::<String>::new();
    for candidate in globally_ranked.into_iter().take(profit_quota) {
        selected_ids.insert(candidate.candidate_id.clone());
        selected.push(candidate);
    }

    let mut buckets: std::collections::BTreeMap<(u32, u32, u32), Vec<SearchCandidate>> =
        std::collections::BTreeMap::new();
    for candidate in candidates {
        let long = candidate
            .config
            .strategies
            .iter()
            .find(|s| s.direction == MartingaleDirection::Long);
        let short = candidate
            .config
            .strategies
            .iter()
            .find(|s| s.direction == MartingaleDirection::Short);
        let leverage_bucket = candidate
            .config
            .strategies
            .iter()
            .filter_map(|strategy| strategy.leverage)
            .max()
            .unwrap_or(1);
        let key = match (long, short) {
            (Some(long), Some(short)) => (
                leverage_bucket,
                strategy_spacing_bps(long).unwrap_or(0),
                strategy_spacing_bps(short).unwrap_or(0),
            ),
            _ => (leverage_bucket, 0, 0),
        };
        buckets.entry(key).or_default().push(candidate);
    }

    for bucket in buckets.values_mut() {
        bucket.sort_by(|left, right| {
            candidate_profit_priority(right).total_cmp(&candidate_profit_priority(left))
        });
    }

    let mut keys: Vec<_> = buckets.keys().copied().collect();
    keys.sort_by(|left, right| {
        let left_score = buckets
            .get(left)
            .and_then(|bucket| bucket.first())
            .map(candidate_profit_priority)
            .unwrap_or(0.0);
        let right_score = buckets
            .get(right)
            .and_then(|bucket| bucket.first())
            .map(candidate_profit_priority)
            .unwrap_or(0.0);
        right_score.total_cmp(&left_score)
    });
    let mut indices = std::collections::BTreeMap::<(u32, u32, u32), usize>::new();
    while selected.len() < limit {
        let mut added = false;
        for key in &keys {
            let index = indices.get(key).copied().unwrap_or(0);
            if let Some(bucket) = buckets.get(key) {
                if let Some(candidate) = bucket.get(index) {
                    indices.insert(*key, index + 1);
                    if !selected_ids.insert(candidate.candidate_id.clone()) {
                        added = true;
                        continue;
                    }
                    selected.push(candidate.clone());
                    added = true;
                    if selected.len() >= limit {
                        break;
                    }
                }
            }
        }
        if !added {
            break;
        }
    }
    selected
}

fn candidate_profit_priority(candidate: &SearchCandidate) -> f64 {
    let leverage = candidate
        .config
        .strategies
        .iter()
        .filter_map(|strategy| strategy.leverage)
        .max()
        .unwrap_or(1) as f64;
    let mut spacing_sum = 0.0;
    let mut take_profit_sum = 0.0;
    let mut multiplier_sum = 0.0;
    let mut max_legs_sum = 0.0;
    let mut tail_stop_sum = 0.0;
    let mut count = 0.0;
    for strategy in &candidate.config.strategies {
        count += 1.0;
        spacing_sum += strategy_spacing_bps(strategy).unwrap_or(100) as f64;
        take_profit_sum += strategy_take_profit_bps_value(strategy).unwrap_or(80) as f64;
        tail_stop_sum += strategy_tail_stop_bps(strategy).unwrap_or(2_500) as f64;
        if let MartingaleSizingModel::Multiplier {
            multiplier,
            max_legs,
            ..
        } = &strategy.sizing
        {
            multiplier_sum += multiplier.to_f64().unwrap_or(1.25);
            max_legs_sum += *max_legs as f64;
        }
    }
    let count = f64::max(count, 1.0);
    let avg_spacing = spacing_sum / count;
    let avg_take_profit = take_profit_sum / count;
    let avg_multiplier = multiplier_sum / count;
    let avg_max_legs = max_legs_sum / count;
    let avg_tail_stop = tail_stop_sum / count;

    leverage.min(6.0) * 3.0
        + avg_multiplier * 20.0
        + avg_max_legs * 8.0
        + avg_take_profit / 8.0
        + avg_tail_stop / 600.0
        - avg_spacing / 45.0
}

fn strategy_spacing_bps(strategy: &MartingaleStrategyConfig) -> Option<u32> {
    match &strategy.spacing {
        MartingaleSpacingModel::FixedPercent { step_bps } => Some(*step_bps),
        _ => None,
    }
}

fn strategy_take_profit_bps_value(strategy: &MartingaleStrategyConfig) -> Option<u32> {
    match &strategy.take_profit {
        MartingaleTakeProfitModel::Percent { bps } => Some(*bps),
        _ => None,
    }
}

fn strategy_tail_stop_bps(strategy: &MartingaleStrategyConfig) -> Option<u32> {
    match &strategy.stop_loss {
        Some(MartingaleStopLossModel::StrategyDrawdownPct { pct_bps }) => Some(*pct_bps),
        _ => None,
    }
}

#[cfg(test)]
fn strategy_take_profit_bps(strategy: &MartingaleStrategyConfig) -> Option<u32> {
    match &strategy.take_profit {
        MartingaleTakeProfitModel::Percent { bps } => Some(*bps),
        _ => None,
    }
}

fn evaluate_long_short_candidate_for_screening(
    candidate: SearchCandidate,
    context: &MarketDataContext,
    task: &WorkerTaskConfig,
    symbol: &str,
    direction_mode: &str,
    scoring: &ScoringConfig,
) -> (EvaluatedCandidate, CandidateRejectionSample) {
    use backtest_engine::martingale::scoring::score_candidate;

    let overridden = apply_task_overrides_to_candidate(candidate, task);
    let result = run_candidate_kline_screening(&overridden, context);
    let (score, sample) = match result {
        Ok(ref metrics) => {
            let s = score_candidate(metrics, scoring);
            let sample = CandidateRejectionSample {
                candidate_id: overridden.candidate_id.clone(),
                symbol: symbol.to_owned(),
                direction_mode: direction_mode.to_owned(),
                total_return_pct: Some(metrics.metrics.total_return_pct),
                max_drawdown_pct: Some(metrics.metrics.max_drawdown_pct),
                trade_count: metrics.metrics.trade_count as usize,
                survival_valid: s.survival_valid,
                rejection_reason: None,
            };
            (s, sample)
        }
        Err(_) => {
            let sample = CandidateRejectionSample {
                candidate_id: overridden.candidate_id.clone(),
                symbol: symbol.to_owned(),
                direction_mode: direction_mode.to_owned(),
                total_return_pct: None,
                max_drawdown_pct: None,
                trade_count: 0,
                survival_valid: false,
                rejection_reason: Some("screening_failed".to_owned()),
            };
            (
                backtest_engine::martingale::scoring::CandidateScore {
                    survival_valid: false,
                    rank_score: 0.0,
                    raw_score: 0.0,
                    rejection_reasons: vec!["screening_failed".to_owned()],
                },
                sample,
            )
        }
    };
    (
        EvaluatedCandidate {
            candidate: overridden,
            score,
        },
        sample,
    )
}

fn run_long_short_staged_search(
    context: &MarketDataContext,
    symbol: &str,
    task: &WorkerTaskConfig,
    staged: &StagedMartingaleSearchSpace,
    scoring: &ScoringConfig,
    max_threads: usize,
) -> Result<(Vec<EvaluatedCandidate>, Vec<CandidateRejectionSample>), String> {
    let direction_mode = task.direction_mode.as_deref().unwrap_or("long");

    // Apply user search_space overrides with lower-churn neighbor expansion
    // and cap candidates to random_candidates * intelligent_rounds.
    let candidates = generate_long_short_candidates_for_task(symbol, task, staged)?;
    let candidate_count = candidates.len();

    let start = std::time::Instant::now();
    let timeout_secs = long_short_search_timeout_secs(task, candidate_count, max_threads);

    let coarse_results = screen_candidates_bounded_parallel(candidates, max_threads, |candidate| {
        evaluate_long_short_candidate_for_screening(
            candidate,
            context,
            task,
            symbol,
            direction_mode,
            scoring,
        )
    });
    if start.elapsed().as_secs() > timeout_secs {
        return Err(martingale_search_timeout_error(
            symbol,
            direction_mode,
            candidate_count,
            timeout_secs,
        ));
    }

    let (mut evaluated, mut rejection_samples): (Vec<_>, Vec<_>) =
        coarse_results.into_iter().unzip();
    let mut screening_samples_by_id = rejection_samples
        .iter()
        .map(|sample| (sample.candidate_id.clone(), sample.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();

    sort_long_short_candidates_for_profile(
        &mut evaluated,
        &task.risk_profile,
        Some(&screening_samples_by_id),
    );
    let survivors: Vec<_> = evaluated
        .iter()
        .filter(|candidate| candidate.score.survival_valid)
        .take(profit_v2_survivor_limit(task))
        .cloned()
        .collect();

    let mut refined = Vec::new();
    for survivor in &survivors {
        let fine_space = long_short_fine_space_around_candidate(&survivor.candidate);
        let fine_task = task_with_long_short_refinement_space(task, &fine_space);
        let mut fine_candidates =
            generate_long_short_candidates_for_task(symbol, &fine_task, staged)?;
        for (fine_index, fine_candidate) in fine_candidates.iter_mut().enumerate() {
            fine_candidate.candidate_id = format!(
                "{}-fine-{fine_index}-{}",
                survivor.candidate.candidate_id, fine_candidate.candidate_id
            );
        }

        let fine_results =
            screen_candidates_bounded_parallel(fine_candidates, max_threads, |candidate| {
                evaluate_long_short_candidate_for_screening(
                    candidate,
                    context,
                    task,
                    symbol,
                    direction_mode,
                    scoring,
                )
            });

        for (candidate, sample) in fine_results {
            screening_samples_by_id.insert(sample.candidate_id.clone(), sample.clone());
            rejection_samples.push(sample);
            refined.push(candidate);
        }

        if start.elapsed().as_secs() > timeout_secs {
            return Err(martingale_search_timeout_error(
                symbol,
                direction_mode,
                candidate_count,
                timeout_secs,
            ));
        }
    }

    evaluated.extend(refined);
    sort_long_short_candidates_for_profile(
        &mut evaluated,
        &task.risk_profile,
        Some(&screening_samples_by_id),
    );
    let mut seen_ids = std::collections::BTreeSet::<String>::new();
    evaluated.retain(|candidate| seen_ids.insert(candidate.candidate.candidate_id.clone()));
    evaluated.truncate(task.per_symbol_top_n.max(20).min(80));
    Ok((evaluated, rejection_samples))
}

fn long_short_fine_space_around_candidate(
    candidate: &SearchCandidate,
) -> StagedMartingaleSearchSpace {
    let base = coarse_parameter_point_from_candidate(candidate);
    let mut fine = fine_space_around(&base);

    let long = candidate
        .config
        .strategies
        .iter()
        .find(|strategy| strategy.direction == MartingaleDirection::Long);
    let short = candidate
        .config
        .strategies
        .iter()
        .find(|strategy| strategy.direction == MartingaleDirection::Short);

    let mut spacing_bps = fine.spacing_bps.clone();
    if let Some(value) = long.and_then(strategy_spacing_bps) {
        push_u32_neighbors(&mut spacing_bps, value, &[40, 20], 50, 1200);
    }
    if let Some(value) = short.and_then(strategy_spacing_bps) {
        push_u32_neighbors(&mut spacing_bps, value, &[40, 20], 50, 1200);
    }
    spacing_bps.sort();
    spacing_bps.dedup();
    fine.spacing_bps = spacing_bps;

    let mut take_profit_bps = fine.take_profit_bps.clone();
    if let Some(value) = long.and_then(strategy_take_profit_bps_value) {
        push_u32_neighbors(&mut take_profit_bps, value, &[30, 15], 40, 500);
    }
    if let Some(value) = short.and_then(strategy_take_profit_bps_value) {
        push_u32_neighbors(&mut take_profit_bps, value, &[30, 15], 40, 500);
    }
    take_profit_bps.sort();
    take_profit_bps.dedup();
    fine.take_profit_bps = take_profit_bps;

    let mut tail_stop_bps = fine.tail_stop_bps.clone();
    if let Some(value) = long.and_then(strategy_tail_stop_bps) {
        push_u32_neighbors(&mut tail_stop_bps, value, &[400, 200], 400, 4000);
    }
    if let Some(value) = short.and_then(strategy_tail_stop_bps) {
        push_u32_neighbors(&mut tail_stop_bps, value, &[400, 200], 400, 4000);
    }
    tail_stop_bps.sort();
    tail_stop_bps.dedup();
    fine.tail_stop_bps = tail_stop_bps;

    fine
}

fn push_u32_neighbors(values: &mut Vec<u32>, center: u32, deltas: &[u32], min: u32, max: u32) {
    if center >= min && center <= max {
        values.push(center);
    }
    for delta in deltas {
        if let Some(lower) = center.checked_sub(*delta) {
            if lower >= min {
                values.push(lower);
            }
        }
        let upper = center.saturating_add(*delta);
        if upper <= max {
            values.push(upper);
        }
    }
}

fn task_with_long_short_refinement_space(
    task: &WorkerTaskConfig,
    fine_space: &StagedMartingaleSearchSpace,
) -> WorkerTaskConfig {
    let mut refined_task = task.clone();
    refined_task.search_space = Some(json!({
        "leverage": fine_space.leverage,
        "spacing_bps": fine_space.spacing_bps,
        "order_multiplier": fine_space.order_multiplier,
        "max_legs": fine_space.max_legs,
        "take_profit_bps": fine_space.take_profit_bps,
        "tail_stop_bps": fine_space.tail_stop_bps,
        "long_short_weight_pct": fine_space.long_short_weight_pct,
    }));
    refined_task.random_candidates = task.random_candidates.max(32);
    refined_task.intelligent_rounds = 1;
    refined_task
}

fn portfolio_candidates_from_outputs(
    outputs: &[CandidateOutput],
) -> Vec<backtest_engine::portfolio_search::EvaluatedCandidate> {
    outputs
        .iter()
        .filter_map(|output| {
            let config = serde_json::from_value(output.config.clone()).ok()?;
            let summary = &output.summary;
            let equity_curve: Vec<backtest_engine::martingale::metrics::EquityPoint> = summary
                .get("equity_curve")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let drawdown_curve: Vec<backtest_engine::martingale::metrics::DrawdownPoint> = summary
                .get("drawdown_curve")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let planned_margin_quote: f64 = output.planned_margin_quote.unwrap_or(0.0);
            let annualized_return_pct: Option<f64> = summary
                .get("annualized_return_pct")
                .and_then(|v| v.as_f64());
            let trades: Vec<backtest_engine::martingale::metrics::MartingaleTradeDetail> = summary
                .get("trades_preview")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            Some(backtest_engine::portfolio_search::EvaluatedCandidate {
                candidate: SearchCandidate {
                    candidate_id: output.candidate_id.clone(),
                    config,
                },
                score: output.score,
                return_pct: output.total_return_pct,
                max_drawdown_pct: output.max_drawdown_pct,
                survival_passed: output.total_return_pct > 0.0,
                planned_margin_quote,
                trade_count: output.trade_count,
                annualized_return_pct,
                equity_curve: if equity_curve.is_empty() {
                    output.equity_curve.clone()
                } else {
                    equity_curve
                },
                drawdown_curve: if drawdown_curve.is_empty() {
                    output.drawdown_curve.clone()
                } else {
                    drawdown_curve
                },
                trades: if trades.is_empty() {
                    output.trades_preview.clone()
                } else {
                    trades
                },
            })
        })
        .collect()
}

fn search_space_from_staged(
    staged: &StagedMartingaleSearchSpace,
    symbol: &str,
    task: &WorkerTaskConfig,
) -> SearchSpace {
    SearchSpace {
        symbols: vec![symbol.to_owned()],
        directions: directions_from_mode(task.direction_mode.as_deref()),
        market: market_kind(task.market.as_deref()),
        margin_mode: margin_mode(task.margin_mode.as_deref()),
        step_bps: search_space_u32(task, "spacing_bps")
            .unwrap_or_else(|| staged.spacing_bps.clone()),
        first_order_quote: search_space_decimal(task, "first_order_quote")
            .or_else(|| {
                template_decimal(task, &["sizing", "first_order_quote"]).map(|value| vec![value])
            })
            .unwrap_or_else(|| vec![Decimal::new(100, 0), Decimal::new(250, 0)]),
        multiplier: search_space_decimal(task, "order_multiplier").unwrap_or_else(|| {
            staged
                .order_multiplier
                .iter()
                .filter_map(|m| Decimal::from_f64_retain(*m))
                .collect()
        }),
        take_profit_bps: search_space_u32(task, "take_profit_bps")
            .unwrap_or_else(|| staged.take_profit_bps.clone()),
        leverage: search_space_u32(task, "leverage").unwrap_or_else(|| staged.leverage.clone()),
        max_legs: search_space_u32(task, "max_legs").unwrap_or_else(|| staged.max_legs.clone()),
    }
}

fn coarse_parameter_point_from_candidate(candidate: &SearchCandidate) -> CoarseParameterPoint {
    let strategy = candidate.config.strategies.first();
    let leverage = strategy.and_then(|s| s.leverage).unwrap_or(2);
    let spacing_bps = strategy
        .map(|s| match &s.spacing {
            MartingaleSpacingModel::FixedPercent { step_bps } => *step_bps,
            _ => 100,
        })
        .unwrap_or(100);
    let (order_multiplier, max_legs) = strategy
        .map(|s| match &s.sizing {
            MartingaleSizingModel::Multiplier {
                multiplier,
                max_legs,
                ..
            } => (multiplier.to_f64().unwrap_or(1.5), *max_legs),
            _ => (1.5, 5),
        })
        .unwrap_or((1.5, 5));
    let take_profit_bps = strategy
        .map(|s| match &s.take_profit {
            MartingaleTakeProfitModel::Percent { bps } => *bps,
            _ => 100,
        })
        .unwrap_or(100);

    let (long_weight_pct, short_weight_pct) =
        if candidate.config.direction_mode == MartingaleDirectionMode::LongAndShort {
            let long_first = candidate
                .config
                .strategies
                .iter()
                .find(|s| s.direction == MartingaleDirection::Long)
                .and_then(|s| match &s.sizing {
                    MartingaleSizingModel::Multiplier {
                        first_order_quote, ..
                    } => Some(first_order_quote.to_f64().unwrap_or(0.0)),
                    _ => None,
                })
                .unwrap_or(0.0);
            let short_first = candidate
                .config
                .strategies
                .iter()
                .find(|s| s.direction == MartingaleDirection::Short)
                .and_then(|s| match &s.sizing {
                    MartingaleSizingModel::Multiplier {
                        first_order_quote, ..
                    } => Some(first_order_quote.to_f64().unwrap_or(0.0)),
                    _ => None,
                })
                .unwrap_or(0.0);
            let total = long_first + short_first;
            if total > 0.0 {
                (
                    (long_first / total * 100.0) as u32,
                    (short_first / total * 100.0) as u32,
                )
            } else {
                (50, 50)
            }
        } else {
            (100, 0)
        };

    CoarseParameterPoint {
        leverage,
        spacing_bps,
        order_multiplier,
        max_legs,
        take_profit_bps,
        tail_stop_bps: 1800,
        long_weight_pct,
        short_weight_pct,
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
    let effective_symbols = effective_search_symbols(&task.config);
    let market_data = config.open_market_data()?;
    let market_context = MarketDataContext::load(&market_data, &task.config, &effective_symbols)?;
    poller.heartbeat(&task.task_id, "search_started").await?;

    let direction_mode = task.config.direction_mode.as_deref().unwrap_or("long");
    let is_long_short = direction_mode == "long_short" || direction_mode == "long_and_short";
    let drawdown_limits = if is_long_short {
        long_short_drawdown_limit_sequence(&task.config.risk_profile)
    } else {
        drawdown_limit_sequence(&task.config.risk_profile)
    };
    let first_drawdown_limit = drawdown_limits.first().copied().unwrap_or(25.0);
    let mut screened = Vec::new();
    let mut evaluated_count = 0usize;
    let mut all_rejection_samples: Vec<CandidateRejectionSample> = Vec::new();

    for symbol in &effective_symbols {
        respect_pause_or_cancel(poller, &task.task_id).await?;
        let estimate = estimate_staged_search_work_for_task(&task.config);
        poller
            .update_task_summary_fragment(
                &task.task_id,
                json!({
                    "stage": "search_symbol",
                    "stage_label": format!("搜索 {} 参数中", symbol),
                    "progress_pct": 35,
                    "current_symbol": symbol,
                    "estimated_screenings": estimate.max_screenings_per_symbol,
                }),
            )
            .await?;
        for drawdown_limit_pct in &drawdown_limits {
            let scoring = scoring_config_from_task(&task.config, *drawdown_limit_pct);
            let (candidates, rejection_samples) = run_profit_first_staged_search(
                &market_context,
                symbol,
                &task.config,
                &scoring,
                *drawdown_limit_pct,
                config.max_threads,
            )?;
            evaluated_count += candidates.len();
            let samples_by_id = rejection_samples
                .iter()
                .map(|sample| (sample.candidate_id.clone(), sample.clone()))
                .collect::<std::collections::BTreeMap<_, _>>();
            all_rejection_samples.extend(rejection_samples);
            let risk_relaxed = *drawdown_limit_pct > first_drawdown_limit;
            let valid = select_candidates_or_best_fallback_for_task(
                candidates,
                *drawdown_limit_pct,
                risk_relaxed,
                &samples_by_id,
                &task.config.risk_profile,
            );
            if !valid.is_empty() {
                screened.extend(valid);
                break;
            }
        }
    }
    respect_pause_or_cancel(poller, &task.task_id).await?;

    let ranked = select_refinement_candidates_with_drawdown_metadata(
        screened,
        task.config.symbols.len().max(1) * task.config.per_symbol_top_n.max(1),
        task.config.per_symbol_top_n.max(20),
        &task.config.risk_profile,
    );
    let mut outputs = Vec::new();

    for (index, evaluated_with_drawdown) in ranked.into_iter().enumerate() {
        let evaluated = evaluated_with_drawdown.candidate;
        poller
            .heartbeat(
                &task.task_id,
                &format!("trade_refinement_top_{}", index + 1),
            )
            .await?;
        let overridden_candidate =
            apply_task_overrides_to_candidate(evaluated.candidate.clone(), &task.config);
        let refined = run_candidate_trade_refinement(&overridden_candidate, &market_context)?;
        let used_trade_refinement =
            !trades_for_candidate(&evaluated.candidate, &market_context.trades).is_empty();
        let rows = vec![json!({
            "task_id": task.task_id,
            "candidate_id": evaluated.candidate.candidate_id,
            "rank": index + 1,
            "kline_score": evaluated.score.rank_score,
            "trade_metrics": {
                "total_return_pct": refined.metrics.total_return_pct,
                "max_drawdown_pct": refined.metrics.max_drawdown_pct,
                "used_drawdown_limit_pct": evaluated_with_drawdown.used_drawdown_limit_pct,
                "risk_relaxed": evaluated_with_drawdown.risk_relaxed,
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
        outputs.push(CandidateOutput {
            candidate_id: evaluated.candidate.candidate_id,
            rank: index + 1,
            score: evaluated.score.rank_score,
            config: serde_json::to_value(&overridden_candidate.config)
                .map_err(|error| format!("serialize candidate config: {error}"))?,
            summary: json!({}),
            artifact_path: manifest.path.display().to_string(),
            checksum_sha256: manifest.checksum_sha256,
            used_trade_refinement,
            used_drawdown_limit_pct: evaluated_with_drawdown.used_drawdown_limit_pct,
            risk_relaxed: evaluated_with_drawdown.risk_relaxed,
            total_return_pct: refined.metrics.total_return_pct,
            max_drawdown_pct: refined.metrics.max_drawdown_pct,
            trade_count: refined.metrics.trade_count,
            annualized_return_pct: refined.metrics.annualized_return_pct,
            return_drawdown_ratio: refined.metrics.return_drawdown_ratio,
            planned_margin_quote: refined.metrics.planned_margin_quote,
            max_leverage_used: refined.metrics.max_leverage_used,
            equity_curve: refined.equity_curve.clone(),
            drawdown_curve: refined.drawdown_curve.clone(),
            trades_preview: refined.trades.clone(),
        });
        respect_pause_or_cancel(poller, &task.task_id).await?;
    }

    let max_portfolio_drawdown_pct = if is_long_short {
        long_short_drawdown_limit_sequence(&task.config.risk_profile)
            .first()
            .copied()
            .unwrap_or(30.0)
    } else {
        drawdown_limit_sequence(&task.config.risk_profile)
            .first()
            .copied()
            .unwrap_or(25.0)
    };
    let portfolio_pool_outputs = if should_use_profit_optimized_v2(&task.config) {
        select_portfolio_pool_outputs_v2(outputs.clone(), max_portfolio_drawdown_pct, 10, 10, 5)
    } else {
        select_portfolio_pool_outputs(
            outputs.clone(),
            task.config.per_symbol_top_n.max(20),
            &task.config.risk_profile,
        )
    };
    let display_outputs = select_top_outputs_per_symbol(
        outputs,
        task.config.per_symbol_top_n.max(1),
        &task.config.risk_profile,
    );
    respect_pause_or_cancel(poller, &task.task_id).await?;
    let diagnostics = CandidateRejectionDiagnostics::from_samples(all_rejection_samples);
    if display_outputs.is_empty() {
        let _ = poller
            .update_task_summary_fragment(
                &task.task_id,
                json!({
                    "stage": "failed",
                    "stage_label": "失败",
                    "progress_pct": 100,
                    "rejection_diagnostics": diagnostics,
                }),
            )
            .await;
    }
    ensure_non_empty_selection_for_task(
        &task.config,
        display_outputs.len(),
        evaluated_count,
        &diagnostics,
    )?;
    let display_symbols = symbols_from_outputs(&display_outputs);
    let portfolio_pool_symbols = symbols_from_outputs(&portfolio_pool_outputs);
    let _persisted_candidates = poller
        .save_candidates_and_artifacts(&task.task_id, evaluated_count, &display_outputs)
        .await?;
    respect_pause_or_cancel(poller, &task.task_id).await?;

    let portfolio_candidates = portfolio_candidates_from_outputs(&portfolio_pool_outputs);
    let portfolio_top_n = if should_use_profit_optimized_v2(&task.config) {
        10
    } else {
        3
    };
    let portfolio_top3 = if should_use_profit_optimized_v2(&task.config) {
        build_portfolio_top_n_v2(
            &portfolio_candidates,
            max_portfolio_drawdown_pct,
            portfolio_top_n,
        )
    } else {
        build_portfolio_top3(&portfolio_candidates, max_portfolio_drawdown_pct)
    };
    let portfolio_full_rows = portfolio_top3.top3.iter().enumerate().map(|(rank, portfolio)| {
        let member_id_hash: String = portfolio.members.iter()
            .map(|m| m.candidate_id.as_str())
            .collect::<Vec<&str>>()
            .join("-");
        json!({
            "portfolio_id": format!("portfolio-{}-{}", rank + 1, member_id_hash),
            "portfolio_rank": rank + 1,
            "member_count": portfolio.member_count,
            "members": portfolio.members.iter().map(|m| json!({
                "candidate_id": m.candidate_id,
                "symbol": m.symbol,
                "direction": m.direction,
                "allocation_pct": m.allocation_pct,
                "return_pct": m.return_pct,
                "max_drawdown_pct": m.max_drawdown_pct,
                "annualized_return_pct": m.annualized_return_pct,
                "score": m.score,
                "trade_count": m.trade_count,
            })).collect::<Vec<Value>>(),
            "total_return_pct": portfolio.return_pct,
            "return_pct": portfolio.return_pct,
            "max_drawdown_pct": portfolio.max_drawdown_pct,
            "annualized_return_pct": portfolio.annualized_return_pct,
            "score": portfolio.score,
            "trade_count": portfolio.trade_count,
            "equity_curve": portfolio.equity_curve,
            "drawdown_curve": portfolio.drawdown_curve,
            "trades_preview": portfolio.trades_preview,
            "eligible_candidate_count": portfolio_top3.eligible_candidate_count,
            "eligible_symbols": portfolio_top3.eligible_symbols.clone(),
            "unique_eligible_symbol_count": portfolio_top3.unique_eligible_symbol_count,
            "portfolio_symbols": portfolio.members.iter().map(|m| m.symbol.clone()).collect::<Vec<_>>(),
            "portfolio_unique_symbol_count": portfolio.members.iter().map(|m| m.symbol.as_str()).collect::<std::collections::HashSet<_>>().len(),
        })
    }).collect::<Vec<Value>>();
    let portfolio_rows = portfolio_top3.top3.iter().enumerate().map(|(rank, portfolio)| {
        let member_id_hash: String = portfolio.members.iter()
            .map(|m| m.candidate_id.as_str())
            .collect::<Vec<&str>>()
            .join("-");
        json!({
            "portfolio_id": format!("portfolio-{}-{}", rank + 1, member_id_hash),
            "portfolio_rank": rank + 1,
            "member_count": portfolio.member_count,
            "members": portfolio.members.iter().map(|m| json!({
                "candidate_id": m.candidate_id,
                "symbol": m.symbol,
                "direction": m.direction,
                "allocation_pct": m.allocation_pct,
                "return_pct": m.return_pct,
                "max_drawdown_pct": m.max_drawdown_pct,
                "annualized_return_pct": m.annualized_return_pct,
                "score": m.score,
                "trade_count": m.trade_count,
            })).collect::<Vec<Value>>(),
            "total_return_pct": portfolio.return_pct,
            "return_pct": portfolio.return_pct,
            "max_drawdown_pct": portfolio.max_drawdown_pct,
            "annualized_return_pct": portfolio.annualized_return_pct,
            "score": portfolio.score,
            "trade_count": portfolio.trade_count,
            "equity_curve": sampled_preview(&portfolio.equity_curve, 500),
            "drawdown_curve": sampled_preview(&portfolio.drawdown_curve, 500),
            "trades_preview": sampled_preview(&portfolio.trades_preview, 100),
            "eligible_candidate_count": portfolio_top3.eligible_candidate_count,
            "eligible_symbols": portfolio_top3.eligible_symbols.clone(),
            "unique_eligible_symbol_count": portfolio_top3.unique_eligible_symbol_count,
            "portfolio_symbols": portfolio.members.iter().map(|m| m.symbol.clone()).collect::<Vec<_>>(),
            "portfolio_unique_symbol_count": portfolio.members.iter().map(|m| m.symbol.as_str()).collect::<std::collections::HashSet<_>>().len(),
        })
    }).collect::<Vec<Value>>();
    let portfolio_top10_rows: Vec<Value> = portfolio_top3
        .all_portfolios
        .as_ref()
        .map(|all| {
            all.iter().enumerate().map(|(rank, portfolio)| {
                let member_id_hash: String = portfolio.members.iter()
                    .map(|m| m.candidate_id.as_str())
                    .collect::<Vec<&str>>()
                    .join("-");
                json!({
                    "portfolio_id": format!("portfolio-{}-{}", rank + 1, member_id_hash),
                    "portfolio_rank": rank + 1,
                    "member_count": portfolio.member_count,
                    "members": portfolio.members.iter().map(|m| json!({
                        "candidate_id": m.candidate_id,
                        "symbol": m.symbol,
                        "direction": m.direction,
                        "allocation_pct": m.allocation_pct,
                        "return_pct": m.return_pct,
                        "max_drawdown_pct": m.max_drawdown_pct,
                        "annualized_return_pct": m.annualized_return_pct,
                        "score": m.score,
                        "trade_count": m.trade_count,
                    })).collect::<Vec<Value>>(),
                    "total_return_pct": portfolio.return_pct,
                    "return_pct": portfolio.return_pct,
                    "max_drawdown_pct": portfolio.max_drawdown_pct,
                    "annualized_return_pct": portfolio.annualized_return_pct,
                    "score": portfolio.score,
                    "trade_count": portfolio.trade_count,
                    "equity_curve": sampled_preview(&portfolio.equity_curve, 500),
                    "drawdown_curve": sampled_preview(&portfolio.drawdown_curve, 500),
                    "trades_preview": sampled_preview(&portfolio.trades_preview, 100),
                    "eligible_candidate_count": portfolio_top3.eligible_candidate_count,
                    "eligible_symbols": portfolio_top3.eligible_symbols.clone(),
                    "unique_eligible_symbol_count": portfolio_top3.unique_eligible_symbol_count,
                    "portfolio_symbols": portfolio.members.iter().map(|m| m.symbol.clone()).collect::<Vec<_>>(),
                    "portfolio_unique_symbol_count": portfolio.members.iter().map(|m| m.symbol.as_str()).collect::<std::collections::HashSet<_>>().len(),
                })
            }).collect()
        })
        .unwrap_or_default();
    let portfolio_top10_full_rows: Vec<Value> = portfolio_top3
        .all_portfolios
        .as_ref()
        .map(|all| {
            all.iter().enumerate().map(|(rank, portfolio)| {
                let member_id_hash: String = portfolio.members.iter()
                    .map(|m| m.candidate_id.as_str())
                    .collect::<Vec<&str>>()
                    .join("-");
                json!({
                    "portfolio_id": format!("portfolio-{}-{}", rank + 1, member_id_hash),
                    "portfolio_rank": rank + 1,
                    "member_count": portfolio.member_count,
                    "members": portfolio.members.iter().map(|m| json!({
                        "candidate_id": m.candidate_id,
                        "symbol": m.symbol,
                        "direction": m.direction,
                        "allocation_pct": m.allocation_pct,
                        "return_pct": m.return_pct,
                        "max_drawdown_pct": m.max_drawdown_pct,
                        "annualized_return_pct": m.annualized_return_pct,
                        "score": m.score,
                        "trade_count": m.trade_count,
                    })).collect::<Vec<Value>>(),
                    "total_return_pct": portfolio.return_pct,
                    "return_pct": portfolio.return_pct,
                    "max_drawdown_pct": portfolio.max_drawdown_pct,
                    "annualized_return_pct": portfolio.annualized_return_pct,
                    "score": portfolio.score,
                    "trade_count": portfolio.trade_count,
                    "equity_curve": portfolio.equity_curve,
                    "drawdown_curve": portfolio.drawdown_curve,
                    "trades_preview": portfolio.trades_preview,
                    "eligible_candidate_count": portfolio_top3.eligible_candidate_count,
                    "eligible_symbols": portfolio_top3.eligible_symbols.clone(),
                    "unique_eligible_symbol_count": portfolio_top3.unique_eligible_symbol_count,
                    "portfolio_symbols": portfolio.members.iter().map(|m| m.symbol.clone()).collect::<Vec<_>>(),
                    "portfolio_unique_symbol_count": portfolio.members.iter().map(|m| m.symbol.as_str()).collect::<std::collections::HashSet<_>>().len(),
                })
            }).collect()
        })
        .unwrap_or_default();
    let portfolio_manifest = write_task_json_artifact(
        &config.artifact_root,
        &task.task_id,
        "portfolio",
        "top3",
        &portfolio_full_rows,
    )?;
    verify_artifact(&portfolio_manifest)?;
    let portfolio_top10_manifest = if !portfolio_top10_full_rows.is_empty() {
        Some(write_task_json_artifact(
            &config.artifact_root,
            &task.task_id,
            "portfolio",
            "top10",
            &portfolio_top10_full_rows,
        )?)
    } else {
        None
    };

    poller
        .mark_completed(
            &task.task_id,
            json!({
                "portfolio_top_n": portfolio_top_n,
                "portfolio_top3": portfolio_rows,
                "portfolio_top10": portfolio_top10_rows,
                "expanded_universe_symbol_count": effective_symbols.len(),
                "portfolio_pool_candidate_count": portfolio_pool_outputs.len(),
                "portfolio_pool_note": "positive-return candidates include qualified, high-return and low-drawdown tiers; final portfolio still enforces hard drawdown limit",
                "portfolio_top3_artifact_path": portfolio_manifest.path.display().to_string(),
                "portfolio_top10_artifact_path": portfolio_top10_manifest.as_ref().map(|m| m.path.display().to_string()).unwrap_or_default(),
                "eligible_candidate_count": portfolio_top3.eligible_candidate_count,
                "searched_symbols": task.config.symbols.clone(),
                "display_symbols": display_symbols,
                "portfolio_pool_symbols": portfolio_pool_symbols,
                "eligible_symbols": portfolio_top3.eligible_symbols,
                "unique_eligible_symbol_count": portfolio_top3.unique_eligible_symbol_count,
                "eligible_candidates": portfolio_candidates.iter().map(|c| {
                    let (long_weight_pct, short_weight_pct) = if c.candidate.config.direction_mode == MartingaleDirectionMode::LongAndShort {
                        let long_first = c.candidate.config.strategies.iter()
                            .find(|s| s.direction == MartingaleDirection::Long)
                            .and_then(|s| match &s.sizing { MartingaleSizingModel::Multiplier { first_order_quote, .. } => Some(first_order_quote.to_f64().unwrap_or(0.0)), _ => None })
                            .unwrap_or(0.0);
                        let short_first = c.candidate.config.strategies.iter()
                            .find(|s| s.direction == MartingaleDirection::Short)
                            .and_then(|s| match &s.sizing { MartingaleSizingModel::Multiplier { first_order_quote, .. } => Some(first_order_quote.to_f64().unwrap_or(0.0)), _ => None })
                            .unwrap_or(0.0);
                        let total = long_first + short_first;
                        if total > 0.0 {
                            ((long_first / total * 100.0) as u32, (short_first / total * 100.0) as u32)
                        } else {
                            (50, 50)
                        }
                    } else {
                        (0, 0)
                    };
                    json!({
                        "candidate_id": c.candidate.candidate_id,
                        "symbol": c.candidate.config.strategies.first().map(|s| s.symbol.clone()).unwrap_or_default(),
                        "direction": format!("{:?}", c.candidate.config.direction_mode),
                        "long_short_legs": serde_json::to_value(&c.candidate.config).ok().as_ref().map(|v| long_short_leg_summary_from_config(v)).unwrap_or(json!({})),
                        "score": c.score,
                        "return_pct": c.return_pct,
                        "max_drawdown_pct": c.max_drawdown_pct,
                        "annualized_return_pct": c.annualized_return_pct,
                        "planned_margin_quote": c.planned_margin_quote,
                        "trade_count": c.trade_count,
                        "survival_passed": c.survival_passed,
                        "long_weight_pct": long_weight_pct,
                        "short_weight_pct": short_weight_pct,
                    })
                }).collect::<Vec<Value>>(),
            }),
        )
        .await?;
    Ok(())
}

#[cfg(test)]
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

#[cfg(test)]
fn search_candidates_with_drawdown_relaxation<F>(
    config: &WorkerTaskConfig,
    cancel: Option<&AtomicBool>,
    mut search_symbol: F,
) -> Result<Vec<EvaluatedCandidateWithDrawdown>, String>
where
    F: FnMut(&str, f64) -> Result<Vec<EvaluatedCandidate>, String>,
{
    let drawdown_limits = drawdown_limit_sequence(&config.risk_profile);
    let first_drawdown_limit = drawdown_limits.first().copied().unwrap_or(30.0);
    let mut screened = Vec::new();

    for symbol in &config.symbols {
        if cancel
            .map(|flag| flag.load(Ordering::SeqCst))
            .unwrap_or(false)
        {
            return Err("cancelled".to_string());
        }
        let mut selected_for_symbol = None;
        for drawdown_limit_pct in drawdown_limits.iter().copied() {
            let candidates = search_symbol(symbol, drawdown_limit_pct)?;
            let has_survival_valid = candidates
                .iter()
                .any(|candidate| candidate.score.survival_valid);
            selected_for_symbol = Some((drawdown_limit_pct, candidates));
            if has_survival_valid {
                break;
            }
        }
        let (used_drawdown_limit_pct, candidates) =
            selected_for_symbol.ok_or_else(|| "drawdown limit sequence is empty".to_string())?;
        let risk_relaxed = used_drawdown_limit_pct > first_drawdown_limit;
        screened.extend(candidates.into_iter().filter_map(|candidate| {
            if !candidate.score.survival_valid {
                return None;
            }
            Some(EvaluatedCandidateWithDrawdown {
                candidate,
                used_drawdown_limit_pct,
                risk_relaxed,
                screening_total_return_pct: None,
                screening_max_drawdown_pct: None,
                screening_trade_count: None,
            })
        }));
    }

    Ok(screened)
}

fn ensure_non_empty_selection_for_task(
    config: &WorkerTaskConfig,
    selected_count: usize,
    screened_count: usize,
    diagnostics: &CandidateRejectionDiagnostics,
) -> Result<(), String> {
    if selected_count > 0 {
        return Ok(());
    }
    Err(format!(
        "no martingale candidates selected: direction_mode={} symbols={} screened_count={} selected_count=0 risk_profile={} negative_return={} drawdown_rejected={} zero_trade={} survival_valid={}",
        config.direction_mode.as_deref().unwrap_or("long"),
        config.symbols.join(","),
        screened_count,
        config.risk_profile,
        diagnostics.negative_return_count,
        diagnostics.drawdown_rejected_count,
        diagnostics.zero_trade_count,
        diagnostics.survival_valid_count,
    ))
}

fn select_candidates_or_best_fallback_for_task(
    candidates: Vec<EvaluatedCandidate>,
    drawdown_limit_pct: f64,
    risk_relaxed: bool,
    samples_by_id: &std::collections::BTreeMap<String, CandidateRejectionSample>,
    risk_profile: &str,
) -> Vec<EvaluatedCandidateWithDrawdown> {
    let mut valid: Vec<_> = candidates
        .iter()
        .filter(|candidate| candidate.score.survival_valid)
        .cloned()
        .map(|candidate| {
            evaluated_with_drawdown(candidate, drawdown_limit_pct, risk_relaxed, samples_by_id)
        })
        .collect();
    if !valid.is_empty() {
        sort_evaluated_with_drawdown_for_profile(&mut valid, risk_profile);
        return valid;
    }

    let mut fallback: Vec<_> = candidates
        .into_iter()
        .filter(|candidate| candidate.score.rank_score > 0.0)
        .collect();
    fallback.sort_by(|a, b| b.score.rank_score.total_cmp(&a.score.rank_score));
    fallback
        .into_iter()
        .take(10)
        .map(|candidate| {
            evaluated_with_drawdown(candidate, drawdown_limit_pct, true, samples_by_id)
        })
        .collect()
}

fn evaluated_with_drawdown(
    candidate: EvaluatedCandidate,
    drawdown_limit_pct: f64,
    risk_relaxed: bool,
    samples_by_id: &std::collections::BTreeMap<String, CandidateRejectionSample>,
) -> EvaluatedCandidateWithDrawdown {
    let sample = samples_by_id.get(&candidate.candidate.candidate_id);
    EvaluatedCandidateWithDrawdown {
        candidate,
        used_drawdown_limit_pct: drawdown_limit_pct,
        risk_relaxed,
        screening_total_return_pct: sample.and_then(|sample| sample.total_return_pct),
        screening_max_drawdown_pct: sample.and_then(|sample| sample.max_drawdown_pct),
        screening_trade_count: sample.map(|sample| sample.trade_count),
    }
}

fn sort_evaluated_with_drawdown_for_profile(
    candidates: &mut [EvaluatedCandidateWithDrawdown],
    risk_profile: &str,
) {
    if risk_profile.eq_ignore_ascii_case("aggressive") {
        candidates.sort_by(|left, right| {
            evaluated_profit_score(right)
                .total_cmp(&evaluated_profit_score(left))
                .then_with(|| {
                    right
                        .candidate
                        .score
                        .rank_score
                        .total_cmp(&left.candidate.score.rank_score)
                })
        });
    } else {
        candidates.sort_by(|left, right| {
            right
                .candidate
                .score
                .rank_score
                .total_cmp(&left.candidate.score.rank_score)
                .then_with(|| {
                    evaluated_profit_score(right).total_cmp(&evaluated_profit_score(left))
                })
        });
    }
}

fn evaluated_profit_score(candidate: &EvaluatedCandidateWithDrawdown) -> f64 {
    let total_return = candidate
        .screening_total_return_pct
        .unwrap_or(candidate.candidate.score.raw_score);
    if total_return <= 0.0 {
        return f64::NEG_INFINITY;
    }
    let drawdown = candidate
        .screening_max_drawdown_pct
        .unwrap_or(candidate.used_drawdown_limit_pct)
        .max(1.0);
    let trade_bonus = candidate.screening_trade_count.unwrap_or(0).min(300) as f64 / 300.0;
    total_return + (total_return / drawdown) * 10.0 - drawdown * 0.15 + trade_bonus * 3.0
}

fn rejection_sample_from_evaluated(
    evaluated: &EvaluatedCandidate,
    symbol: &str,
    direction_mode: &str,
) -> CandidateRejectionSample {
    let has_negative_return = evaluated
        .score
        .rejection_reasons
        .iter()
        .any(|r| r.contains("negative_return"));
    let has_zero_trades = evaluated
        .score
        .rejection_reasons
        .iter()
        .any(|r| r.contains("insufficient_data_quality") || r.contains("screening_failed"));
    let rejection_reason = if evaluated.score.rejection_reasons.is_empty() {
        None
    } else {
        Some(evaluated.score.rejection_reasons.join("; "))
    };
    CandidateRejectionSample {
        candidate_id: evaluated.candidate.candidate_id.clone(),
        symbol: symbol.to_owned(),
        direction_mode: direction_mode.to_owned(),
        total_return_pct: if has_negative_return {
            None
        } else {
            Some(evaluated.score.raw_score)
        },
        max_drawdown_pct: None,
        trade_count: if has_zero_trades { 0 } else { 1 },
        survival_valid: evaluated.score.survival_valid,
        rejection_reason,
    }
}

fn select_refinement_candidates_with_drawdown_metadata(
    mut candidates: Vec<EvaluatedCandidateWithDrawdown>,
    _min_total: usize,
    per_symbol_top_n: usize,
    risk_profile: &str,
) -> Vec<EvaluatedCandidateWithDrawdown> {
    use std::collections::BTreeMap;

    sort_evaluated_with_drawdown_for_profile(&mut candidates, risk_profile);

    let mut selected = Vec::new();
    let mut selected_counts = BTreeMap::<String, usize>::new();

    for candidate in candidates.iter() {
        let symbol = search_candidate_symbol(&candidate.candidate.candidate)
            .unwrap_or_else(|| candidate.candidate.candidate.candidate_id.clone());
        let count = selected_counts.entry(symbol).or_default();
        if *count >= per_symbol_top_n {
            continue;
        }
        *count += 1;
        selected.push(candidate.clone());
    }

    selected
}

fn long_short_survivor_limit(task: &WorkerTaskConfig) -> usize {
    task.per_symbol_top_n.max(20).min(48)
}

fn profit_v2_survivor_limit(task: &WorkerTaskConfig) -> usize {
    if should_use_profit_optimized_v2(task) {
        task.per_symbol_top_n.max(20).min(80)
    } else {
        long_short_survivor_limit(task)
    }
}

fn sort_long_short_candidates_for_profile(
    candidates: &mut [EvaluatedCandidate],
    risk_profile: &str,
    samples_by_id: Option<&std::collections::BTreeMap<String, CandidateRejectionSample>>,
) {
    if risk_profile.eq_ignore_ascii_case("aggressive") {
        candidates.sort_by(|left, right| {
            let left_profit_score = aggressive_screening_profit_score(left, samples_by_id);
            let right_profit_score = aggressive_screening_profit_score(right, samples_by_id);
            right_profit_score
                .total_cmp(&left_profit_score)
                .then_with(|| right.score.rank_score.total_cmp(&left.score.rank_score))
                .then_with(|| {
                    right
                        .candidate
                        .candidate_id
                        .cmp(&left.candidate.candidate_id)
                })
        });
    } else {
        candidates.sort_by(|left, right| right.score.rank_score.total_cmp(&left.score.rank_score));
    }
}

fn aggressive_screening_profit_score(
    candidate: &EvaluatedCandidate,
    samples_by_id: Option<&std::collections::BTreeMap<String, CandidateRejectionSample>>,
) -> f64 {
    if !candidate.score.survival_valid {
        return f64::NEG_INFINITY;
    }
    let Some(sample) =
        samples_by_id.and_then(|samples| samples.get(&candidate.candidate.candidate_id))
    else {
        return candidate.score.rank_score;
    };
    let total_return = sample.total_return_pct.unwrap_or(0.0);
    if total_return <= 0.0 {
        return f64::NEG_INFINITY;
    }
    let drawdown = sample.max_drawdown_pct.unwrap_or(100.0).max(1.0);
    total_return + (total_return / drawdown) * 8.0 - drawdown * 0.2
}

fn output_profit_score(output: &CandidateOutput) -> f64 {
    let annualized = output
        .annualized_return_pct
        .unwrap_or(output.total_return_pct);
    if annualized <= 0.0 {
        return f64::NEG_INFINITY;
    }
    let drawdown = output.max_drawdown_pct.max(1.0);
    annualized + (annualized / drawdown) * 10.0 - drawdown * 0.15
}

fn select_portfolio_pool_outputs_v2(
    outputs: Vec<CandidateOutput>,
    drawdown_limit_pct: f64,
    qualified_top_n: usize,
    growth_top_n: usize,
    low_drawdown_top_n: usize,
) -> Vec<CandidateOutput> {
    let mut by_symbol: std::collections::BTreeMap<String, Vec<CandidateOutput>> =
        std::collections::BTreeMap::new();
    for output in outputs {
        if output.total_return_pct <= 0.0 {
            continue;
        }
        let symbol = output_symbol(&output).unwrap_or_else(|| output.candidate_id.clone());
        by_symbol.entry(symbol).or_default().push(output);
    }

    let mut selected: Vec<CandidateOutput> = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for (_symbol, mut items) in by_symbol {
        // Tier 1: qualified (within drawdown limit)
        items.sort_by(|a, b| output_profit_score(b).total_cmp(&output_profit_score(a)));
        for output in items
            .iter()
            .filter(|o| o.max_drawdown_pct <= drawdown_limit_pct)
            .take(qualified_top_n)
        {
            if seen.insert(output.candidate_id.clone()) {
                selected.push(output.clone());
            }
        }

        // Tier 2: high return (may exceed drawdown limit)
        items.sort_by(|a, b| {
            b.annualized_return_pct
                .unwrap_or(b.total_return_pct)
                .total_cmp(&a.annualized_return_pct.unwrap_or(a.total_return_pct))
        });
        for output in items.iter().take(growth_top_n) {
            if seen.insert(output.candidate_id.clone()) {
                selected.push(output.clone());
            }
        }

        // Tier 3: low drawdown stabilizers
        items.sort_by(|a, b| a.max_drawdown_pct.total_cmp(&b.max_drawdown_pct));
        for output in items.iter().take(low_drawdown_top_n) {
            if seen.insert(output.candidate_id.clone()) {
                selected.push(output.clone());
            }
        }
    }
    selected
}

fn select_portfolio_pool_outputs(
    mut outputs: Vec<CandidateOutput>,
    per_symbol_top_n: usize,
    risk_profile: &str,
) -> Vec<CandidateOutput> {
    use std::collections::BTreeMap;

    outputs.retain(|output| output.total_return_pct > 0.0 && !output.equity_curve.is_empty());
    sort_outputs_for_profile(&mut outputs, risk_profile);

    let mut selected = Vec::new();
    let mut selected_counts = BTreeMap::<String, usize>::new();
    let mut selected_signatures = std::collections::BTreeSet::<String>::new();

    for output in outputs {
        let symbol = output_symbol(&output).unwrap_or_else(|| output.candidate_id.clone());
        let signature = output_parameter_signature(&output);
        if !selected_signatures.insert(signature) {
            continue;
        }
        let count = selected_counts.entry(symbol).or_default();
        if *count >= per_symbol_top_n {
            continue;
        }
        *count += 1;
        selected.push(output);
    }

    selected
}

fn select_top_outputs_per_symbol(
    mut outputs: Vec<CandidateOutput>,
    per_symbol_top_n: usize,
    risk_profile: &str,
) -> Vec<CandidateOutput> {
    use std::collections::BTreeMap;

    outputs.retain(|output| {
        if output.total_return_pct <= 0.0 {
            return false;
        }
        if output.max_drawdown_pct > output.used_drawdown_limit_pct {
            return false;
        }
        true
    });

    sort_outputs_for_profile(&mut outputs, risk_profile);

    let mut selected = Vec::new();
    let mut selected_counts = BTreeMap::<String, usize>::new();
    let mut selected_signatures = std::collections::BTreeSet::<String>::new();

    for output in outputs {
        let symbol = output_symbol(&output).unwrap_or_else(|| output.candidate_id.clone());
        let signature = output_parameter_signature(&output);
        if !selected_signatures.insert(signature) {
            continue;
        }
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
            let take_profit_bps = output_take_profit_bps(&output);
            let trailing_take_profit_bps = output_trailing_take_profit_bps(&output);
            let direction = output_direction(&output);
            let overfit_flag = output_overfit_flag(&output);
            let risk_summary_human = output_risk_summary_human(&output, risk_profile);

            let direction_mode = output.config.get("direction_mode").cloned().unwrap_or(Value::Null);
            let long_short_legs = long_short_leg_summary_from_config(&output.config);

            output.rank = index + 1;
            output.summary = merge_json_objects(
                output.summary,
                json!({
                    "source_candidate_id": output.candidate_id,
                    "symbol": symbol,
                    "direction": direction,
                    "direction_mode": direction_mode,
                    "long_short_legs": long_short_legs,
                    "parameter_rank_for_symbol": parameter_rank_for_symbol,
                    "recommended_weight_pct": recommended_weight_pct,
                    "recommended_leverage": recommended_leverage,
                    "max_leverage_used": output.max_leverage_used.unwrap_or(recommended_leverage as f64),
                    "planned_margin_quote": output.planned_margin_quote,
                    "risk_profile": risk_profile,
                    "portfolio_group_key": portfolio_group_key,
                    "spacing_bps": spacing_bps,
                    "first_order_quote": first_order_quote,
                    "order_multiplier": order_multiplier,
                    "max_legs": max_legs,
                    "take_profit_bps": take_profit_bps,
                    "trailing_take_profit_bps": trailing_take_profit_bps,
                    "return_pct": output.total_return_pct,
                    "total_return_pct": output.total_return_pct,
                    "max_drawdown_pct": output.max_drawdown_pct,
                    "annualized_return_pct": output.annualized_return_pct,
                    "used_drawdown_limit_pct": output.used_drawdown_limit_pct,
                    "risk_relaxed": output.risk_relaxed,
                    "score": output.score,
                    "overfit_flag": overfit_flag,
                    "risk_summary_human": risk_summary_human,
                    "artifact_path": output.artifact_path,
                    "equity_curve": sampled_preview(&output.equity_curve, 500),
                    "drawdown_curve": sampled_preview(&output.drawdown_curve, 500),
                    "trades_preview": sampled_preview(&output.trades_preview, 100),
                }),
            );
            output
        })
        .collect()
}

fn sort_outputs_for_profile(outputs: &mut [CandidateOutput], risk_profile: &str) {
    if risk_profile.eq_ignore_ascii_case("aggressive") {
        outputs.sort_by(|left, right| {
            let left_annualized = left.annualized_return_pct.unwrap_or(left.total_return_pct);
            let right_annualized = right
                .annualized_return_pct
                .unwrap_or(right.total_return_pct);
            right_annualized
                .total_cmp(&left_annualized)
                .then_with(|| {
                    right
                        .return_drawdown_ratio
                        .unwrap_or(0.0)
                        .total_cmp(&left.return_drawdown_ratio.unwrap_or(0.0))
                })
                .then_with(|| left.max_drawdown_pct.total_cmp(&right.max_drawdown_pct))
        });
    } else {
        outputs.sort_by(|left, right| right.score.total_cmp(&left.score));
    }
}

fn symbols_from_outputs(outputs: &[CandidateOutput]) -> Vec<String> {
    outputs
        .iter()
        .filter_map(output_symbol)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn sampled_preview<T: Clone>(items: &[T], max_items: usize) -> Vec<T> {
    if max_items == 0 || items.is_empty() {
        return Vec::new();
    }
    if items.len() <= max_items {
        return items.to_vec();
    }
    let last_index = items.len() - 1;
    (0..max_items)
        .map(|index| {
            let source_index = index * last_index / (max_items - 1);
            items[source_index].clone()
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

fn merge_json_objects_mut(base: &mut Value, patch: Value) {
    if let Value::Object(patch_map) = patch {
        if let Value::Object(base_map) = base {
            for (key, value) in patch_map {
                base_map.insert(key, value);
            }
        }
    }
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
    if let Some(direction_mode) = output.config.get("direction_mode").and_then(Value::as_str) {
        if direction_mode.eq_ignore_ascii_case("long_and_short")
            || direction_mode.eq_ignore_ascii_case("long_short")
        {
            return Value::String("long_short".to_owned());
        }
    }
    output_strategy(output)
        .and_then(|strategy| strategy.get("direction"))
        .cloned()
        .unwrap_or(Value::Null)
}

fn output_leverage(output: &CandidateOutput) -> Option<u32> {
    output
        .config
        .get("strategies")
        .and_then(Value::as_array)
        .map(|strategies| {
            strategies
                .iter()
                .filter_map(|strategy| {
                    strategy
                        .get("leverage")
                        .and_then(Value::as_u64)
                        .map(|v| v as u32)
                })
                .max()
                .unwrap_or(1)
        })
}

fn output_parameter_signature(output: &CandidateOutput) -> String {
    serde_json::to_string(&output.config).unwrap_or_else(|_| output.candidate_id.clone())
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

fn long_short_leg_summary_from_config(config: &Value) -> Value {
    let Some(strategies) = config.get("strategies").and_then(|v| v.as_array()) else {
        return json!({});
    };

    let mut result = serde_json::Map::new();
    for strategy in strategies {
        let direction = strategy
            .get("direction")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_lowercase();
        let key = if direction.contains("long") {
            "long"
        } else if direction.contains("short") {
            "short"
        } else {
            continue;
        };
        let spacing_key = strategy
            .get("spacing")
            .and_then(|v| v.as_object())
            .map(|obj| obj.keys().next().cloned().unwrap_or_default())
            .unwrap_or_default();
        let sizing_key = strategy
            .get("sizing")
            .and_then(|v| v.as_object())
            .map(|obj| obj.keys().next().cloned().unwrap_or_default())
            .unwrap_or_default();
        let take_profit_key = strategy
            .get("take_profit")
            .and_then(|v| v.as_object())
            .map(|obj| obj.keys().next().cloned().unwrap_or_default())
            .unwrap_or_default();
        result.insert(key.to_owned(), json!({
            "first_order_quote": strategy.pointer(&format!("/sizing/{sizing_key}/first_order_quote")).cloned().unwrap_or(Value::Null),
            "order_multiplier": strategy.pointer(&format!("/sizing/{sizing_key}/multiplier")).cloned().unwrap_or(Value::Null),
            "max_legs": strategy.pointer(&format!("/sizing/{sizing_key}/max_legs")).cloned().unwrap_or(Value::Null),
            "spacing_bps": strategy.pointer(&format!("/spacing/{spacing_key}/step_bps")).or_else(|| strategy.pointer(&format!("/spacing/{spacing_key}/first_step_bps"))).cloned().unwrap_or(Value::Null),
            "take_profit_bps": strategy.pointer(&format!("/take_profit/{take_profit_key}/bps")).cloned().unwrap_or(Value::Null),
            "stop_loss_bps": strategy.pointer("/stop_loss/strategy_drawdown_pct/pct_bps").cloned().unwrap_or(Value::Null),
            "leverage": strategy.get("leverage").cloned().unwrap_or(Value::Null),
        }));
    }
    Value::Object(result)
}

#[cfg(test)]
fn search_space_from_task(config: &WorkerTaskConfig) -> SearchSpace {
    SearchSpace {
        symbols: config.symbols.clone(),
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
    }
}

fn scoring_config_from_task(config: &WorkerTaskConfig, max_drawdown_pct: f64) -> ScoringConfig {
    let mut scoring = ScoringConfig::default();
    scoring.max_global_drawdown_pct = max_drawdown_pct;
    scoring.max_strategy_drawdown_pct = max_drawdown_pct;
    if let Some(value) = config.scoring.as_ref() {
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
        .search_space
        .as_ref()
        .and_then(|space| space.get(key))
        .or_else(|| {
            config
                .martingale_template
                .as_ref()
                .and_then(|template| template.get("search_space"))
                .and_then(|space| space.get(key))
        })
}

fn search_space_f64(config: &WorkerTaskConfig, key: &str) -> Option<Vec<f64>> {
    let values = search_space_value(config, key)?.as_array()?;
    let parsed: Vec<f64> = values
        .iter()
        .filter_map(|v| v.as_f64())
        .filter(|v| *v > 0.0)
        .collect();
    (!parsed.is_empty()).then_some(parsed)
}

fn search_space_long_short_weights(config: &WorkerTaskConfig) -> Option<Vec<(u32, u32)>> {
    let values = search_space_value(config, "long_short_weight_pct")?.as_array()?;
    let parsed: Vec<(u32, u32)> = values
        .iter()
        .filter_map(|v| v.as_array())
        .filter_map(|arr| {
            let first = arr
                .first()
                .and_then(|v| v.as_u64())
                .and_then(|v| u32::try_from(v).ok())?;
            let second = arr
                .get(1)
                .and_then(|v| v.as_u64())
                .and_then(|v| u32::try_from(v).ok())?;
            Some((first, second))
        })
        .collect();
    (!parsed.is_empty()).then_some(parsed)
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

fn directions_from_mode(mode: Option<&str>) -> Vec<shared_domain::martingale::MartingaleDirection> {
    use shared_domain::martingale::MartingaleDirection::{Long, Short};
    match mode {
        Some("long_only") => vec![Long],
        Some("short_only") => vec![Short],
        Some("long_and_short") => vec![Long, Short],
        _ => vec![Long, Short],
    }
}

#[cfg(test)]
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

#[cfg(test)]
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
    fn load(
        source: &dyn MarketDataSource,
        config: &WorkerTaskConfig,
        symbols: &[String],
    ) -> Result<Self, String> {
        validate_time_range(config)?;
        let available_symbols = source.list_symbols()?;
        let available_set = available_symbols
            .iter()
            .map(|symbol| symbol.trim().to_uppercase())
            .collect::<std::collections::BTreeSet<_>>();
        let mut bars = Vec::new();
        let mut trades = Vec::new();
        for symbol in symbols {
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
    if config.interval != "1m" {
        eprintln!(
            "WARNING: interval '{}' is not 1m; backtest accuracy may be reduced",
            config.interval
        );
    }
    Ok(())
}

fn run_candidate_kline_screening(
    candidate: &SearchCandidate,
    market_context: &MarketDataContext,
) -> Result<MartingaleBacktestResult, String> {
    let bars = screening_bars_for_candidate(candidate, &market_context.bars);
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

fn screening_bars_for_candidate(candidate: &SearchCandidate, bars: &[KlineBar]) -> Vec<KlineBar> {
    let full_bars = bars_for_candidate(candidate, bars);
    representative_screening_bars(&full_bars)
}

fn representative_screening_bars(bars: &[KlineBar]) -> Vec<KlineBar> {
    if bars.len() <= SCREENING_MAX_BARS_PER_SYMBOL {
        return bars.to_vec();
    }

    let mut selected = Vec::with_capacity(SCREENING_MAX_BARS_PER_SYMBOL);
    selected.extend(bars.iter().take(SCREENING_EARLY_BARS_PER_SYMBOL).cloned());

    let middle_start = bars.len().saturating_sub(SCREENING_MIDDLE_BARS_PER_SYMBOL) / 2;
    selected.extend(
        bars.iter()
            .skip(middle_start)
            .take(SCREENING_MIDDLE_BARS_PER_SYMBOL)
            .cloned(),
    );

    let recent_start = bars.len().saturating_sub(SCREENING_RECENT_BARS_PER_SYMBOL);
    selected.extend(bars.iter().skip(recent_start).cloned());
    selected.sort_by_key(|bar| bar.open_time_ms);
    selected.dedup_by_key(|bar| bar.open_time_ms);
    selected
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

    async fn update_task_summary_fragment(
        &self,
        task_id: &str,
        fragment: Value,
    ) -> Result<(), String> {
        self.repo
            .update_task_summary(task_id, fragment)
            .map_err(|error| format!("update task summary fragment: {error}"))?;
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
    ) -> Result<Vec<BacktestCandidateRecord>, String> {
        self.repo
            .append_task_event(
                task_id,
                "screening_completed",
                json!({ "screened_count": screened_count, "selected_count": outputs.len() }),
            )
            .map_err(|error| format!("append screening event: {error}"))?;
        let mut persisted = Vec::with_capacity(outputs.len());
        for output in outputs {
            let (record, _artifact) = self
                .repo
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
                                "used_drawdown_limit_pct": output.used_drawdown_limit_pct,
                                "risk_relaxed": output.risk_relaxed,
                                "trade_count": output.trade_count,
                                "annualized_return_pct": output.annualized_return_pct,
                                "return_drawdown_ratio": output.return_drawdown_ratio,
                                "planned_margin_quote": output.planned_margin_quote,
                                "max_leverage_used": output.max_leverage_used,
                                "equity_curve": sampled_preview(&output.equity_curve, 500),
                                "drawdown_curve": sampled_preview(&output.drawdown_curve, 500),
                                "trades_preview": sampled_preview(&output.trades_preview, 100),
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
            persisted.push(record);
        }
        Ok(persisted)
    }

    async fn mark_completed(&self, task_id: &str, extra_summary: Value) -> Result<(), String> {
        let mut base = json!({
            "stage": "completed",
            "stage_label": "已完成",
            "progress_pct": 100,
        });
        merge_json_objects_mut(&mut base, extra_summary);
        self.repo
            .update_task_summary(task_id, base)
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
    use backtest_engine::search::random_search;
    use std::path::PathBuf;

    use shared_domain::martingale::{
        MartingaleDirection, MartingaleDirectionMode, MartingalePortfolioConfig,
        MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel,
        MartingaleStopLossModel, MartingaleStrategyConfig, MartingaleTakeProfitModel,
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
                "direction_mode": "long_only",
                "strategies": [{
                    "symbol": symbol,
                    "leverage": leverage,
                    "spacing": { "fixed_percent": { "step_bps": 100 + rank as u32 } },
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
            used_drawdown_limit_pct: 25.0,
            risk_relaxed: false,
            total_return_pct: score,
            max_drawdown_pct: 5.0,
            trade_count: 10,
            annualized_return_pct: Some(score / 2.0),
            return_drawdown_ratio: Some(score / 5.0),
            planned_margin_quote: Some(150.0),
            max_leverage_used: Some(leverage as f64),
            equity_curve: Vec::new(),
            drawdown_curve: Vec::new(),
            trades_preview: Vec::new(),
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
    fn drawdown_metadata_is_applied_independently_per_symbol() {
        let mut btc = evaluated_candidate("BTCUSDT", "btc-1", 90.0);
        let mut eth = evaluated_candidate("ETHUSDT", "eth-1", 80.0);
        btc.score.survival_valid = true;
        eth.score.survival_valid = true;

        let selected = select_refinement_candidates_with_drawdown_metadata(
            vec![
                EvaluatedCandidateWithDrawdown {
                    candidate: btc,
                    used_drawdown_limit_pct: 25.0,
                    risk_relaxed: false,
                    screening_total_return_pct: Some(90.0),
                    screening_max_drawdown_pct: Some(10.0),
                    screening_trade_count: Some(20),
                },
                EvaluatedCandidateWithDrawdown {
                    candidate: eth,
                    used_drawdown_limit_pct: 30.0,
                    risk_relaxed: true,
                    screening_total_return_pct: Some(80.0),
                    screening_max_drawdown_pct: Some(12.0),
                    screening_trade_count: Some(20),
                },
            ],
            2,
            1,
            "balanced",
        );

        let btc = selected
            .iter()
            .find(|candidate| {
                search_candidate_symbol(&candidate.candidate.candidate).as_deref()
                    == Some("BTCUSDT")
            })
            .unwrap();
        let eth = selected
            .iter()
            .find(|candidate| {
                search_candidate_symbol(&candidate.candidate.candidate).as_deref()
                    == Some("ETHUSDT")
            })
            .unwrap();

        assert_eq!(btc.used_drawdown_limit_pct, 25.0);
        assert!(!btc.risk_relaxed);
        assert_eq!(eth.used_drawdown_limit_pct, 30.0);
        assert!(eth.risk_relaxed);
    }

    #[test]
    fn drawdown_relaxation_stops_independently_for_each_symbol() {
        let config = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()],
            risk_profile: "balanced".to_owned(),
            ..WorkerTaskConfig::default()
        };
        let mut calls = Vec::new();

        let screened =
            search_candidates_with_drawdown_relaxation(&config, None, |symbol, limit| {
                calls.push((symbol.to_owned(), limit));
                let mut candidate =
                    evaluated_candidate(symbol, &format!("{symbol}-{limit}"), limit);
                candidate.score.survival_valid = symbol == "BTCUSDT" || limit >= 30.0;
                Ok(vec![candidate])
            })
            .unwrap();

        assert_eq!(
            calls,
            vec![
                ("BTCUSDT".to_owned(), 25.0),
                ("ETHUSDT".to_owned(), 25.0),
                ("ETHUSDT".to_owned(), 30.0),
            ]
        );
        let btc = screened
            .iter()
            .find(|candidate| {
                search_candidate_symbol(&candidate.candidate.candidate).as_deref()
                    == Some("BTCUSDT")
            })
            .unwrap();
        let eth = screened
            .iter()
            .find(|candidate| {
                search_candidate_symbol(&candidate.candidate.candidate).as_deref()
                    == Some("ETHUSDT")
            })
            .unwrap();

        assert_eq!(btc.used_drawdown_limit_pct, 25.0);
        assert!(!btc.risk_relaxed);
        assert_eq!(eth.used_drawdown_limit_pct, 30.0);
        assert!(eth.risk_relaxed);
    }

    #[test]
    fn all_invalid_symbol_produces_no_refinement_candidates() {
        let config = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned()],
            risk_profile: "balanced".to_owned(),
            ..WorkerTaskConfig::default()
        };

        let screened =
            search_candidates_with_drawdown_relaxation(&config, None, |symbol, limit| {
                let mut candidate =
                    evaluated_candidate(symbol, &format!("{symbol}-{limit}"), limit);
                candidate.score.survival_valid = false;
                candidate.score.rejection_reasons = vec!["global_drawdown_exceeded".to_owned()];
                Ok(vec![candidate])
            })
            .unwrap();
        let selected =
            select_refinement_candidates_with_drawdown_metadata(screened, 1, 1, "balanced");

        assert!(selected.is_empty());
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
        let mut btc_duplicate = candidate_output("BTCUSDT", "btc-duplicate", 1, 95.0, 3);
        btc_duplicate.config = candidate_output("BTCUSDT", "btc-1", 1, 90.0, 3).config;
        let outputs = vec![
            btc_duplicate,
            candidate_output("BTCUSDT", "btc-1", 1, 90.0, 3),
            candidate_output("BTCUSDT", "btc-2", 2, 80.0, 3),
            candidate_output("BTCUSDT", "btc-3", 3, 70.0, 3),
            candidate_output("BTCUSDT", "btc-4", 4, 60.0, 3),
            candidate_output("BTCUSDT", "btc-5", 5, 50.0, 3),
            candidate_output("BTCUSDT", "btc-6", 6, 40.0, 3),
            candidate_output("ETHUSDT", "eth-1", 1, 30.0, 2),
        ];

        let selected = select_top_outputs_per_symbol(outputs, 5, "balanced");

        assert_eq!(selected.len(), 6);
        assert!(selected
            .iter()
            .any(|output| output.candidate_id == "btc-duplicate"));
        assert!(!selected.iter().any(|output| output.candidate_id == "btc-1"));
        assert!(selected.iter().any(|output| output.candidate_id == "btc-5"));
        assert!(!selected.iter().any(|output| output.candidate_id == "btc-6"));

        let btc_first = selected
            .iter()
            .find(|output| output.candidate_id == "btc-duplicate")
            .unwrap();
        assert_eq!(btc_first.summary["symbol"], "BTCUSDT");
        assert_eq!(btc_first.summary["parameter_rank_for_symbol"], 1);
        assert_eq!(btc_first.summary["recommended_weight_pct"], 20.0);
        assert_eq!(btc_first.summary["recommended_leverage"], 3);
        assert_eq!(btc_first.summary["risk_profile"], "balanced");
        assert_eq!(btc_first.summary["source_candidate_id"], "btc-duplicate");
    }

    #[test]
    fn selected_outputs_include_ui_required_summary_fields() {
        let outputs = vec![candidate_output("BTCUSDT", "btc-1", 1, 90.0, 3)];

        let selected = select_top_outputs_per_symbol(outputs, 5, "balanced");
        let output = selected.first().unwrap();
        let summary = output.summary.as_object().unwrap();

        for field in [
            "symbol",
            "direction",
            "direction_mode",
            "long_short_legs",
            "spacing_bps",
            "first_order_quote",
            "order_multiplier",
            "max_legs",
            "take_profit_bps",
            "trailing_take_profit_bps",
            "recommended_weight_pct",
            "recommended_leverage",
            "max_leverage_used",
            "parameter_rank_for_symbol",
            "risk_profile",
            "return_pct",
            "total_return_pct",
            "max_drawdown_pct",
            "annualized_return_pct",
            "used_drawdown_limit_pct",
            "risk_relaxed",
            "score",
            "overfit_flag",
            "risk_summary_human",
            "equity_curve",
            "drawdown_curve",
            "trades_preview",
        ] {
            assert!(
                summary.contains_key(field),
                "missing summary field: {field}"
            );
        }
        assert!(summary.contains_key("artifact_path"));
        assert_eq!(summary["artifact_path"], output.artifact_path);
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
            portfolio_top_n: default_portfolio_top_n(),
            risk_profile: default_risk_profile(),
            market: None,
            margin_mode: None,
            direction_mode: None,
            leverage_range: None,
            martingale_template: None,
            search_space: None,
            scoring: None,
            extended_universe: None,
            search_mode: None,
            interval: "1h".to_owned(),
            start_ms: 1,
            end_ms: 2_000,
        };

        let context =
            MarketDataContext::load(&source, &config, &config.symbols).expect("market context");
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
            portfolio_top_n: default_portfolio_top_n(),
            risk_profile: default_risk_profile(),
            market: None,
            margin_mode: None,
            direction_mode: None,
            leverage_range: None,
            martingale_template: None,
            search_space: None,
            scoring: None,
            extended_universe: None,
            search_mode: None,
            interval: "1h".to_owned(),
            start_ms: 1,
            end_ms: 2_000,
        };

        let context = MarketDataContext::load(&source, &config, &config.symbols)
            .expect("market context without trades");
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
    fn staged_search_space_uses_task_search_space_overrides() {
        let mut config = WorkerTaskConfig::default();
        config.symbols = vec!["BTCUSDT".to_owned()];
        config.martingale_template = Some(json!({
            "search_space": {
                "spacing_bps": [77],
                "order_multiplier": ["1.7"],
                "max_legs": [4],
                "take_profit_bps": [88],
                "leverage": [6]
            }
        }));
        let staged = StagedMartingaleSearchSpace::for_profile("balanced", "long_only");
        let space = search_space_from_staged(&staged, "BTCUSDT", &config);
        assert_eq!(space.step_bps, vec![77]);
        assert_eq!(space.max_legs, vec![4]);
        assert_eq!(space.take_profit_bps, vec![88]);
        assert_eq!(space.leverage, vec![6]);
        assert_eq!(space.multiplier, vec![Decimal::new(17, 1)]);
    }

    #[test]
    fn worker_applies_risk_profile_drawdown_and_wizard_overrides() {
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

        let scoring = scoring_config_from_task(&config, 25.0);
        assert_eq!(scoring.max_global_drawdown_pct, 25.0);
        assert_eq!(scoring.max_strategy_drawdown_pct, 25.0);
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

    #[test]
    fn worker_task_config_deserializes_missing_search_counts_with_defaults() {
        let config: WorkerTaskConfig = serde_json::from_value(json!({
            "symbols": ["BTCUSDT", "ETHUSDT"],
            "risk_profile": "balanced",
            "direction_mode": "long_short",
            "start_ms": 1672531200000_i64,
            "end_ms": 1673308800000_i64
        }))
        .expect("worker config");

        assert_eq!(config.random_seed, 1);
        assert_eq!(config.random_candidates, 16);
        assert_eq!(config.intelligent_rounds, 1);
        assert_eq!(config.top_n, 10);
        assert_eq!(config.per_symbol_top_n, 10);
        assert_eq!(config.portfolio_top_n, 3);
    }

    #[test]
    fn sampled_preview_caps_large_series_and_keeps_edges() {
        let values = (0..1_000).collect::<Vec<_>>();
        let preview = sampled_preview(&values, 10);

        assert_eq!(preview.len(), 10);
        assert_eq!(preview.first().copied(), Some(0));
        assert_eq!(preview.last().copied(), Some(999));
    }

    #[test]
    fn long_short_task_produces_long_and_short_candidates_via_intelligent_search() {
        let config = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            random_seed: 7,
            random_candidates: 8,
            intelligent_rounds: 1,
            per_symbol_top_n: 10,
            top_n: 10,
            portfolio_top_n: 3,
            market: Some("usd_m_futures".to_owned()),
            margin_mode: Some("isolated".to_owned()),
            leverage_range: Some([2, 2]),
            martingale_template: Some(serde_json::json!({
                "search_space": {
                    "leverage": [2],
                    "spacing_bps": [120],
                    "order_multiplier": [1.25],
                    "max_legs": [3],
                    "take_profit_bps": [60],
                    "tail_stop_bps": [2000],
                    "long_short_weight_pct": [[60, 40], [50, 50]]
                }
            })),
            ..WorkerTaskConfig::default()
        };

        let staged = backtest_engine::search::StagedMartingaleSearchSpace::for_profile(
            &config.risk_profile,
            config.direction_mode.as_deref().unwrap(),
        );

        // When direction_mode is long_short, the worker must produce LongAndShort
        // candidates (with both long and short strategy legs), not separate
        // Long-only and Short-only candidates.
        let candidates = backtest_engine::search::generate_staged_candidates_for_symbol(
            "BTCUSDT",
            "long_short",
            &staged,
            20,
        )
        .expect("long_short candidates should generate");

        assert!(!candidates.is_empty());
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.config.direction_mode
                    == MartingaleDirectionMode::LongAndShort),
            "user-requested long_short search must not degrade to long_only/short_only candidates"
        );
        assert!(
            candidates.iter().all(|candidate| {
                let has_long = candidate
                    .config
                    .strategies
                    .iter()
                    .any(|strategy| strategy.direction == MartingaleDirection::Long);
                let has_short = candidate
                    .config
                    .strategies
                    .iter()
                    .any(|strategy| strategy.direction == MartingaleDirection::Short);
                has_long && has_short
            }),
            "each long_short portfolio candidate must include both long and short strategy legs"
        );
    }

    fn sample_candidate_for_parallel_test(id: &str) -> SearchCandidate {
        let strategy = MartingaleStrategyConfig {
            strategy_id: format!("strategy-{id}"),
            symbol: "BTCUSDT".to_owned(),
            market: MartingaleMarketKind::UsdMFutures,
            direction: MartingaleDirection::Long,
            direction_mode: MartingaleDirectionMode::LongOnly,
            margin_mode: Some(MartingaleMarginMode::Isolated),
            leverage: Some(2),
            spacing: MartingaleSpacingModel::FixedPercent { step_bps: 100 },
            sizing: MartingaleSizingModel::Multiplier {
                first_order_quote: Decimal::new(100, 0),
                multiplier: Decimal::new(15, 1),
                max_legs: 4,
            },
            take_profit: MartingaleTakeProfitModel::Percent { bps: 80 },
            stop_loss: Some(MartingaleStopLossModel::StrategyDrawdownPct { pct_bps: 2_500 }),
            indicators: Vec::new(),
            entry_triggers: Vec::new(),
            risk_limits: shared_domain::martingale::MartingaleRiskLimits::default(),
        };
        SearchCandidate {
            candidate_id: id.to_owned(),
            config: shared_domain::martingale::MartingalePortfolioConfig {
                direction_mode: MartingaleDirectionMode::LongOnly,
                strategies: vec![strategy],
                risk_limits: shared_domain::martingale::MartingaleRiskLimits::default(),
            },
        }
    }

    fn sample_parallel_score(
        candidate_id: &str,
    ) -> backtest_engine::martingale::scoring::CandidateScore {
        let raw_score = candidate_id
            .trim_start_matches("candidate-")
            .parse::<f64>()
            .unwrap_or(1.0);
        backtest_engine::martingale::scoring::CandidateScore {
            survival_valid: raw_score > 0.0,
            rank_score: raw_score,
            raw_score,
            rejection_reasons: Vec::new(),
        }
    }

    #[test]
    fn parallel_candidate_screening_preserves_input_order() {
        let candidates = (0..8)
            .map(|index| sample_candidate_for_parallel_test(&format!("candidate-{index}")))
            .collect::<Vec<_>>();

        let evaluated = screen_candidates_bounded_parallel(candidates, 4, |candidate| {
            let score = sample_parallel_score(&candidate.candidate_id);
            let sample = CandidateRejectionSample {
                candidate_id: candidate.candidate_id.clone(),
                symbol: "BTCUSDT".to_owned(),
                direction_mode: "long".to_owned(),
                total_return_pct: Some(score.rank_score),
                max_drawdown_pct: Some(1.0),
                trade_count: 1,
                survival_valid: score.survival_valid,
                rejection_reason: None,
            };
            (EvaluatedCandidate { candidate, score }, sample)
        });

        let ids = evaluated
            .into_iter()
            .map(|(candidate, _sample)| candidate.candidate.candidate_id)
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec![
                "candidate-0",
                "candidate-1",
                "candidate-2",
                "candidate-3",
                "candidate-4",
                "candidate-5",
                "candidate-6",
                "candidate-7"
            ]
        );
    }

    #[test]
    fn bounded_parallelism_clamps_zero_to_one() {
        assert_eq!(bounded_parallel_width(0), 1);
        assert_eq!(bounded_parallel_width(1), 1);
        assert_eq!(bounded_parallel_width(24), 24);
    }
    #[test]
    fn representative_screening_bars_keep_small_inputs_unchanged() {
        let bars = (0..10)
            .map(|index| KlineBar {
                symbol: "BTCUSDT".to_owned(),
                open_time_ms: index,
                open: 1.0,
                high: 1.0,
                low: 1.0,
                close: 1.0,
                volume: 1.0,
            })
            .collect::<Vec<_>>();

        let screened = representative_screening_bars(&bars);

        assert_eq!(screened, bars);
    }

    #[test]
    fn representative_screening_bars_sample_early_middle_and_recent_windows() {
        let total = SCREENING_MAX_BARS_PER_SYMBOL + 20_000;
        let bars = (0..total)
            .map(|index| KlineBar {
                symbol: "ADAUSDT".to_owned(),
                open_time_ms: index as i64,
                open: 1.0,
                high: 1.0,
                low: 1.0,
                close: 1.0,
                volume: 1.0,
            })
            .collect::<Vec<_>>();

        let screened = representative_screening_bars(&bars);

        assert!(screened.len() <= SCREENING_MAX_BARS_PER_SYMBOL);
        assert!(screened.iter().any(|bar| bar.open_time_ms == 0));
        assert!(screened
            .iter()
            .any(|bar| bar.open_time_ms == (total / 2) as i64));
        assert!(screened
            .iter()
            .any(|bar| bar.open_time_ms == (total - 1) as i64));
        assert!(screened
            .windows(2)
            .all(|window| window[0].open_time_ms < window[1].open_time_ms));
    }

    #[test]
    fn long_short_timeout_scales_with_refinement_budget() {
        let small = WorkerTaskConfig {
            random_candidates: 6,
            per_symbol_top_n: 10,
            ..WorkerTaskConfig::default()
        };
        let wide = WorkerTaskConfig {
            random_candidates: 36,
            per_symbol_top_n: 10,
            ..WorkerTaskConfig::default()
        };

        assert_eq!(long_short_search_timeout_secs(&small, 6, 24), 600);
        assert!(long_short_search_timeout_secs(&wide, 72, 24) > 600);
        assert!(long_short_search_timeout_secs(&wide, 72, 24) <= 3_600);
    }

    #[cfg(test)]
    fn evaluated_candidate_for_tests(
        candidate_id: &str,
        symbol: &str,
        direction_mode: MartingaleDirectionMode,
        leverage: u32,
        return_pct: f64,
        _max_drawdown_pct: f64,
        survival_valid: bool,
    ) -> EvaluatedCandidate {
        use backtest_engine::martingale::scoring::CandidateScore;
        use backtest_engine::search::SearchCandidate;
        use shared_domain::martingale::{MartingalePortfolioConfig, MartingaleRiskLimits};

        let strategies = match direction_mode {
            MartingaleDirectionMode::LongAndShort => vec![
                MartingaleStrategyConfig {
                    strategy_id: format!("{candidate_id}-long"),
                    symbol: symbol.to_owned(),
                    market: MartingaleMarketKind::UsdMFutures,
                    direction: MartingaleDirection::Long,
                    direction_mode,
                    margin_mode: Some(MartingaleMarginMode::Isolated),
                    leverage: Some(leverage),
                    spacing: MartingaleSpacingModel::FixedPercent { step_bps: 120 },
                    sizing: MartingaleSizingModel::Multiplier {
                        first_order_quote: Decimal::new(60, 0),
                        multiplier: Decimal::new(125, 2),
                        max_legs: 3,
                    },
                    take_profit: MartingaleTakeProfitModel::Percent { bps: 60 },
                    stop_loss: None,
                    indicators: vec![],
                    entry_triggers: vec![],
                    risk_limits: MartingaleRiskLimits::default(),
                },
                MartingaleStrategyConfig {
                    strategy_id: format!("{candidate_id}-short"),
                    symbol: symbol.to_owned(),
                    market: MartingaleMarketKind::UsdMFutures,
                    direction: MartingaleDirection::Short,
                    direction_mode,
                    margin_mode: Some(MartingaleMarginMode::Isolated),
                    leverage: Some(leverage),
                    spacing: MartingaleSpacingModel::FixedPercent { step_bps: 120 },
                    sizing: MartingaleSizingModel::Multiplier {
                        first_order_quote: Decimal::new(40, 0),
                        multiplier: Decimal::new(125, 2),
                        max_legs: 3,
                    },
                    take_profit: MartingaleTakeProfitModel::Percent { bps: 60 },
                    stop_loss: None,
                    indicators: vec![],
                    entry_triggers: vec![],
                    risk_limits: MartingaleRiskLimits::default(),
                },
            ],
            _ => vec![],
        };

        EvaluatedCandidate {
            candidate: SearchCandidate {
                candidate_id: candidate_id.to_owned(),
                config: MartingalePortfolioConfig {
                    direction_mode,
                    strategies,
                    risk_limits: MartingaleRiskLimits::default(),
                },
            },
            score: CandidateScore {
                survival_valid,
                rank_score: return_pct,
                raw_score: return_pct,
                rejection_reasons: if survival_valid {
                    vec![]
                } else {
                    vec!["survival_failed".to_owned()]
                },
            },
        }
    }

    fn candidate_rejection_sample_for_tests(
        candidate_id: &str,
        total_return_pct: f64,
        max_drawdown_pct: f64,
        trade_count: usize,
        survival_valid: bool,
    ) -> CandidateRejectionSample {
        CandidateRejectionSample {
            candidate_id: candidate_id.to_owned(),
            total_return_pct: Some(total_return_pct),
            max_drawdown_pct: Some(max_drawdown_pct),
            trade_count,
            survival_valid,
            direction_mode: "long_short".to_owned(),
            symbol: "BTCUSDT".to_owned(),
            rejection_reason: None,
        }
    }

    #[test]
    fn zero_selected_candidates_is_not_reported_as_success() {
        let config = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            per_symbol_top_n: 10,
            top_n: 10,
            ..WorkerTaskConfig::default()
        };

        let diagnostics = CandidateRejectionDiagnostics::from_samples(vec![]);
        let error = ensure_non_empty_selection_for_task(&config, 0, 2, &diagnostics)
            .expect_err("zero selected candidates must be an error");
        assert!(
            error.contains("no martingale candidates selected")
                && error.contains("screened_count=2")
                && error.contains("direction_mode=long_short"),
            "error should be actionable: {error}"
        );
    }

    #[test]
    fn zero_selection_error_includes_candidate_rejection_diagnostics() {
        let config = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            per_symbol_top_n: 10,
            top_n: 10,
            ..WorkerTaskConfig::default()
        };

        let diagnostics = CandidateRejectionDiagnostics::from_samples(vec![
            candidate_rejection_sample_for_tests("loss", -2.0, 10.0, 50, false),
            candidate_rejection_sample_for_tests("drawdown", 8.0, 31.0, 120, false),
            candidate_rejection_sample_for_tests("valid", 6.0, 18.0, 80, true),
        ]);

        assert_eq!(diagnostics.total, 3);
        assert_eq!(diagnostics.negative_return_count, 1);
        assert_eq!(diagnostics.drawdown_rejected_count, 1);
        assert_eq!(diagnostics.survival_valid_count, 1);
        assert_eq!(diagnostics.best_by_return[0].candidate_id, "drawdown");

        let error = ensure_non_empty_selection_for_task(&config, 0, 3, &diagnostics)
            .expect_err("should be error");
        assert!(error.contains("no martingale candidates selected"));
        assert!(error.contains("negative_return=1"));
        assert!(error.contains("drawdown_rejected=1"));
        assert!(error.contains("survival_valid=1"));
    }

    #[test]
    fn screening_failed_rejection_sample_does_not_fake_zero_drawdown() {
        let sample = CandidateRejectionSample {
            candidate_id: "failed".to_owned(),
            symbol: "BTCUSDT".to_owned(),
            direction_mode: "long_short".to_owned(),
            total_return_pct: None,
            max_drawdown_pct: None,
            trade_count: 0,
            survival_valid: false,
            rejection_reason: Some("screening_failed".to_owned()),
        };

        let diagnostics = CandidateRejectionDiagnostics::from_samples(vec![sample]);
        assert_eq!(diagnostics.negative_return_count, 0);
        assert_eq!(diagnostics.drawdown_rejected_count, 0);
        assert_eq!(diagnostics.zero_trade_count, 1);
        assert_eq!(
            diagnostics.best_by_return[0].rejection_reason.as_deref(),
            Some("screening_failed")
        );
        assert!(diagnostics.best_by_return[0].max_drawdown_pct.is_none());
    }

    #[test]
    fn long_short_search_does_not_generate_single_direction_substitutes() {
        let task = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            random_candidates: 24,
            intelligent_rounds: 1,
            search_space: Some(serde_json::json!({
                "leverage": [2],
                "spacing_bps": [120],
                "order_multiplier": [1.25],
                "max_legs": [3],
                "take_profit_bps": [60],
                "tail_stop_bps": [2000],
                "long_short_weight_pct": [[60, 40], [50, 50]]
            })),
            ..WorkerTaskConfig::default()
        };

        let staged = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");
        let candidates = generate_long_short_candidates_for_task("BTCUSDT", &task, &staged)
            .expect("candidates should generate");

        assert!(!candidates.is_empty());
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.config.direction_mode
                    == MartingaleDirectionMode::LongAndShort),
            "long_short request must only generate LongAndShort portfolio candidates"
        );
        assert!(
            candidates.iter().all(|candidate| {
                let has_long = candidate
                    .config
                    .strategies
                    .iter()
                    .any(|s| s.direction == MartingaleDirection::Long);
                let has_short = candidate
                    .config
                    .strategies
                    .iter()
                    .any(|s| s.direction == MartingaleDirection::Short);
                has_long && has_short
            }),
            "every long_short candidate must contain both long and short legs"
        );
    }

    #[test]
    fn long_short_lower_churn_candidates_remain_dual_leg() {
        let task = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            random_candidates: 24,
            intelligent_rounds: 1,
            search_space: Some(serde_json::json!({
                "leverage": [2],
                "spacing_bps": [120],
                "order_multiplier": [1.25],
                "max_legs": [3],
                "take_profit_bps": [60],
                "tail_stop_bps": [2000],
                "long_short_weight_pct": [[60, 40], [50, 50]]
            })),
            ..WorkerTaskConfig::default()
        };

        let staged = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");
        let candidates = generate_long_short_candidates_for_task("BTCUSDT", &task, &staged)
            .expect("candidates should generate");

        let spacings: std::collections::BTreeSet<u32> = candidates
            .iter()
            .filter_map(|c| {
                c.config.strategies.first().and_then(|s| match &s.spacing {
                    MartingaleSpacingModel::FixedPercent { step_bps } => Some(*step_bps),
                    _ => None,
                })
            })
            .collect();
        assert!(
            spacings.iter().any(|value| *value >= 240),
            "should screen wider spacing candidates: {:?}",
            spacings
        );
        assert!(candidates
            .iter()
            .all(|candidate| candidate.config.direction_mode
                == MartingaleDirectionMode::LongAndShort));
    }

    #[test]
    fn selection_keeps_best_positive_candidates_when_survival_filter_is_empty() {
        let candidates = vec![
            evaluated_candidate_for_tests(
                "bad-negative",
                "BTCUSDT",
                MartingaleDirectionMode::LongAndShort,
                2,
                -5.0,
                10.0,
                false,
            ),
            evaluated_candidate_for_tests(
                "best-positive-risk-relaxed",
                "BTCUSDT",
                MartingaleDirectionMode::LongAndShort,
                2,
                12.0,
                28.0,
                false,
            ),
            evaluated_candidate_for_tests(
                "second-positive-risk-relaxed",
                "BTCUSDT",
                MartingaleDirectionMode::LongAndShort,
                2,
                8.0,
                24.0,
                false,
            ),
        ];

        let samples_by_id = candidates
            .iter()
            .map(|candidate| {
                (
                    candidate.candidate.candidate_id.clone(),
                    CandidateRejectionSample {
                        candidate_id: candidate.candidate.candidate_id.clone(),
                        symbol: "BTCUSDT".to_owned(),
                        direction_mode: "long_short".to_owned(),
                        total_return_pct: Some(candidate.score.raw_score),
                        max_drawdown_pct: Some(20.0),
                        trade_count: 10,
                        survival_valid: candidate.score.survival_valid,
                        rejection_reason: None,
                    },
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let selected = select_candidates_or_best_fallback_for_task(
            candidates,
            25.0,
            true,
            &samples_by_id,
            "balanced",
        );

        assert_eq!(selected.len(), 2);
        assert_eq!(
            selected[0].candidate.candidate.candidate_id,
            "best-positive-risk-relaxed"
        );
        assert!(selected[0].risk_relaxed);
        assert_eq!(selected[0].used_drawdown_limit_pct, 25.0);
    }

    #[test]
    fn long_short_smoke_search_estimate_is_bounded() {
        let config = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            random_candidates: 16,
            intelligent_rounds: 1,
            search_space: Some(serde_json::json!({
                "leverage": [2],
                "spacing_bps": [120],
                "order_multiplier": [1.25],
                "max_legs": [3],
                "take_profit_bps": [60],
                "tail_stop_bps": [2000],
                "long_short_weight_pct": [[60, 40], [50, 50]]
            })),
            ..WorkerTaskConfig::default()
        };

        let estimate = estimate_staged_search_work_for_task(&config);
        assert!(
            estimate.generated_candidates_per_symbol <= 64,
            "too many generated candidates per symbol: {:?}",
            estimate
        );
        assert!(
            estimate.max_screenings_per_symbol <= 64,
            "too many screenings per symbol: {:?}",
            estimate
        );
    }

    #[test]
    fn long_short_smoke_search_uses_random_candidates_as_screening_cap() {
        let config = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            random_candidates: 16,
            intelligent_rounds: 1,
            search_space: Some(serde_json::json!({
                "leverage": [2],
                "spacing_bps": [120],
                "order_multiplier": [1.25],
                "max_legs": [3],
                "take_profit_bps": [60],
                "tail_stop_bps": [2000],
                "long_short_weight_pct": [[60, 40], [50, 50]]
            })),
            ..WorkerTaskConfig::default()
        };

        let estimate = estimate_staged_search_work_for_task(&config);
        assert!(
            estimate.generated_candidates_per_symbol <= 64,
            "estimate: {:?}",
            estimate
        );
        assert!(
            estimate.max_screenings_per_symbol <= 16,
            "screenings must respect random_candidates: {:?}",
            estimate
        );
    }

    #[test]
    fn martingale_search_timeout_error_is_actionable() {
        let error = martingale_search_timeout_error("BTCUSDT", "long_short", 16, 120);
        assert!(error.contains("martingale search timed out"));
        assert!(error.contains("BTCUSDT"));
        assert!(error.contains("long_short"));
        assert!(error.contains("estimated_screenings=16"));
    }

    #[test]
    fn long_short_uses_configured_risk_profile_drawdown_limits() {
        assert_eq!(long_short_drawdown_limit_sequence("conservative")[0], 20.0);
        assert_eq!(long_short_drawdown_limit_sequence("balanced")[0], 25.0);
        assert_eq!(long_short_drawdown_limit_sequence("aggressive")[0], 30.0);
    }

    #[test]
    fn lower_churn_expansion_adds_wider_spacing_neighbors() {
        let staged = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");
        let task = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            search_space: Some(serde_json::json!({
                "leverage": [2],
                "spacing_bps": [120],
                "order_multiplier": [1.25],
                "max_legs": [3],
                "take_profit_bps": [60],
                "tail_stop_bps": [2000],
                "long_short_weight_pct": [[60, 40], [50, 50]]
            })),
            ..WorkerTaskConfig::default()
        };
        let expanded = apply_search_space_overrides_to_staged(&staged, &task);
        assert!(
            expanded.spacing_bps.contains(&120),
            "should keep original 120"
        );
        assert!(
            expanded.spacing_bps.contains(&180),
            "should add 180: {:?}",
            expanded.spacing_bps
        );
        assert!(
            expanded.spacing_bps.contains(&300),
            "should add 300: {:?}",
            expanded.spacing_bps
        );
        assert!(
            expanded.spacing_bps.contains(&420),
            "should add 420: {:?}",
            expanded.spacing_bps
        );
        assert!(
            expanded.spacing_bps.contains(&720),
            "should add 720: {:?}",
            expanded.spacing_bps
        );
    }

    #[test]
    fn lower_churn_expansion_adds_lower_multiplier_neighbors() {
        let staged = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");
        let task = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            search_space: Some(serde_json::json!({
                "leverage": [2],
                "spacing_bps": [120],
                "order_multiplier": [1.4],
                "max_legs": [3],
                "take_profit_bps": [60],
                "tail_stop_bps": [2000],
                "long_short_weight_pct": [[60, 40]]
            })),
            ..WorkerTaskConfig::default()
        };
        let expanded = apply_search_space_overrides_to_staged(&staged, &task);
        assert!(
            expanded.order_multiplier.len() > 1,
            "order_multiplier should expand: {:?}",
            expanded.order_multiplier
        );
        assert!(
            expanded.order_multiplier.iter().any(|&m| m < 1.4),
            "should add lower multiplier: {:?}",
            expanded.order_multiplier
        );
    }

    #[test]
    fn lower_churn_expansion_adds_higher_take_profit() {
        let staged = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");
        let task = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            search_space: Some(serde_json::json!({
                "leverage": [2],
                "spacing_bps": [120],
                "order_multiplier": [1.25],
                "max_legs": [3],
                "take_profit_bps": [60],
                "tail_stop_bps": [2000],
                "long_short_weight_pct": [[60, 40]]
            })),
            ..WorkerTaskConfig::default()
        };
        let expanded = apply_search_space_overrides_to_staged(&staged, &task);
        assert!(
            expanded.take_profit_bps.contains(&60),
            "should keep original 60"
        );
        assert!(
            expanded.take_profit_bps.contains(&100),
            "should add 100: {:?}",
            expanded.take_profit_bps
        );
        assert!(
            expanded.take_profit_bps.contains(&140),
            "should add 140: {:?}",
            expanded.take_profit_bps
        );
        assert!(
            expanded.take_profit_bps.contains(&200),
            "should add 200: {:?}",
            expanded.take_profit_bps
        );
    }

    #[test]
    fn expanded_search_estimate_stays_bounded() {
        let staged = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");
        let task = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            random_candidates: 16,
            intelligent_rounds: 1,
            search_space: Some(serde_json::json!({
                "leverage": [2],
                "spacing_bps": [120],
                "order_multiplier": [1.25],
                "max_legs": [3],
                "take_profit_bps": [60],
                "tail_stop_bps": [2000],
                "long_short_weight_pct": [[60, 40], [50, 50]]
            })),
            ..WorkerTaskConfig::default()
        };
        let _expanded = apply_search_space_overrides_to_staged(&staged, &task);
        let estimate = estimate_staged_search_work_for_task(&task);
        // Even with expansion, screenings must respect random_candidates cap
        assert!(
            estimate.max_screenings_per_symbol <= 16,
            "screenings must respect cap: {:?}",
            estimate
        );
        // Generated candidates should be reasonable (not thousands)
        assert!(
            estimate.generated_candidates_per_symbol <= 1024,
            "generated should be bounded: {:?}",
            estimate
        );
    }

    #[test]
    fn explicit_long_short_search_budget_is_respected_for_wide_multisymbol_runs() {
        let config = WorkerTaskConfig {
            symbols: vec!["ADAUSDT".to_owned(), "BNBUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            random_candidates: 24,
            intelligent_rounds: 2,
            search_space: Some(serde_json::json!({
                "leverage": [2, 3, 4],
                "spacing_bps": [80, 120, 180],
                "order_multiplier": [1.15, 1.25, 1.4],
                "max_legs": [3, 4],
                "take_profit_bps": [50, 70, 100],
                "tail_stop_bps": [1800, 2400, 3000],
                "long_short_weight_pct": [[70, 30], [60, 40], [50, 50], [40, 60]]
            })),
            ..WorkerTaskConfig::default()
        };

        let estimate = estimate_staged_search_work_for_task(&config);
        assert_eq!(
            estimate.max_screenings_per_symbol, 48,
            "explicit budget should not be inflated: {:?}",
            estimate
        );
    }

    #[test]
    fn long_short_smoke_payload_expands_to_diverse_dual_leg_candidates() {
        let task = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            random_candidates: 16,
            intelligent_rounds: 1,
            per_symbol_top_n: 10,
            search_space: Some(serde_json::json!({
                "leverage": [2],
                "spacing_bps": [120],
                "order_multiplier": [1.25],
                "max_legs": [3],
                "take_profit_bps": [60],
                "tail_stop_bps": [2000],
                "long_short_weight_pct": [[60, 40], [50, 50]]
            })),
            ..WorkerTaskConfig::default()
        };

        let staged = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");
        let candidates = generate_long_short_candidates_for_task("BTCUSDT", &task, &staged)
            .expect("smoke candidates should generate");

        assert!(candidates.len() >= 16);
        assert!(candidates.iter().all(|candidate| {
            candidate.config.direction_mode == MartingaleDirectionMode::LongAndShort
                && candidate.config.strategies.len() == 2
        }));

        let spacing_pairs: std::collections::BTreeSet<(u32, u32)> = candidates
            .iter()
            .filter_map(|candidate| {
                let long = candidate
                    .config
                    .strategies
                    .iter()
                    .find(|s| s.direction == MartingaleDirection::Long)?;
                let short = candidate
                    .config
                    .strategies
                    .iter()
                    .find(|s| s.direction == MartingaleDirection::Short)?;
                match (&long.spacing, &short.spacing) {
                    (
                        MartingaleSpacingModel::FixedPercent {
                            step_bps: long_step,
                        },
                        MartingaleSpacingModel::FixedPercent {
                            step_bps: short_step,
                        },
                    ) => Some((*long_step, *short_step)),
                    _ => None,
                }
            })
            .collect();

        assert!(
            spacing_pairs.len() >= 8,
            "expected diverse long/short spacing pairs, got {spacing_pairs:?}"
        );
        assert!(spacing_pairs
            .iter()
            .any(|(long_step, short_step)| long_step != short_step));
    }

    #[test]
    fn long_short_candidate_selection_prioritizes_profit_potential_within_budget() {
        let task = WorkerTaskConfig {
            symbols: vec!["SOLUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            random_candidates: 6,
            intelligent_rounds: 1,
            per_symbol_top_n: 10,
            search_space: Some(serde_json::json!({
                "leverage": [2, 6],
                "spacing_bps": [80, 180],
                "order_multiplier": [1.15, 1.6],
                "max_legs": [3, 5],
                "take_profit_bps": [50, 120],
                "tail_stop_bps": [1800, 3600],
                "long_short_weight_pct": [[60, 40], [50, 50]]
            })),
            ..WorkerTaskConfig::default()
        };

        let staged = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");
        let candidates = generate_long_short_candidates_for_task("SOLUSDT", &task, &staged)
            .expect("profit-prioritized candidates should generate");

        assert_eq!(candidates.len(), 6);
        assert!(
            candidates.iter().any(|candidate| candidate
                .config
                .strategies
                .iter()
                .any(|strategy| strategy.leverage.unwrap_or(1) >= 6)),
            "high-leverage candidates should remain inside the explicit search budget"
        );
        assert!(
            candidates.iter().any(|candidate| candidate
                .config
                .strategies
                .iter()
                .any(|strategy| strategy_take_profit_bps(strategy).unwrap_or(0) >= 120)),
            "higher take-profit candidates should remain inside the explicit search budget"
        );
    }

    #[test]
    fn long_short_candidate_generation_preserves_risk_standard_and_dual_direction() {
        assert_eq!(
            long_short_drawdown_limit_sequence("balanced"),
            vec![25.0, 30.0]
        );

        let task = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            random_candidates: 16,
            intelligent_rounds: 1,
            per_symbol_top_n: 10,
            search_space: Some(serde_json::json!({
                "leverage": [2],
                "spacing_bps": [120],
                "order_multiplier": [1.25],
                "max_legs": [3],
                "take_profit_bps": [60],
                "tail_stop_bps": [2000],
                "long_short_weight_pct": [[60, 40], [50, 50]]
            })),
            ..WorkerTaskConfig::default()
        };

        let staged = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");
        let candidates = generate_long_short_candidates_for_task("BTCUSDT", &task, &staged)
            .expect("candidates should generate");

        assert!(
            candidates.len() >= 30,
            "expected enough candidates for top10 selection, got {}",
            candidates.len()
        );
        assert!(candidates
            .iter()
            .all(|candidate| candidate.config.direction_mode
                == MartingaleDirectionMode::LongAndShort));
        assert!(candidates.iter().all(|candidate| candidate
            .config
            .strategies
            .iter()
            .any(|s| s.direction == MartingaleDirection::Long)));
        assert!(candidates.iter().all(|candidate| candidate
            .config
            .strategies
            .iter()
            .any(|s| s.direction == MartingaleDirection::Short)));
    }

    #[test]
    fn long_short_balanced_auto_search_expands_all_key_dimensions() {
        let task = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            random_candidates: 16,
            intelligent_rounds: 1,
            per_symbol_top_n: 10,
            search_space: Some(serde_json::json!({
                "leverage": [2],
                "spacing_bps": [120],
                "order_multiplier": [1.25],
                "max_legs": [3],
                "take_profit_bps": [60],
                "tail_stop_bps": [2000],
                "long_short_weight_pct": [[60, 40], [50, 50]]
            })),
            ..WorkerTaskConfig::default()
        };

        let staged = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");
        let expanded = apply_search_space_overrides_to_staged(&staged, &task);

        assert!(
            expanded.leverage.len() >= 3,
            "must test multiple leverage values: {:?}",
            expanded.leverage
        );
        assert!(
            expanded.spacing_bps.len() >= 8,
            "must test broad spacing values: {:?}",
            expanded.spacing_bps
        );
        assert!(
            expanded.order_multiplier.len() >= 4,
            "must test multiple multipliers: {:?}",
            expanded.order_multiplier
        );
        assert!(
            expanded.max_legs.len() >= 3,
            "must test multiple max leg counts: {:?}",
            expanded.max_legs
        );
        assert!(
            expanded.take_profit_bps.len() >= 6,
            "must test broad TP values: {:?}",
            expanded.take_profit_bps
        );
        assert!(
            expanded.tail_stop_bps.len() >= 6,
            "must test broad stop-loss values: {:?}",
            expanded.tail_stop_bps
        );
        assert!(
            expanded.long_short_weight_pct.len() >= 5,
            "must test multiple long/short weights: {:?}",
            expanded.long_short_weight_pct
        );
    }

    #[test]
    fn expanded_universe_defaults_include_only_full_history_futures_symbols() {
        let symbols = default_expanded_universe_symbols();
        assert!(
            symbols.len() >= 18,
            "expected at least 18 symbols, got {symbols:?}"
        );
        for required in [
            "BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "DOGEUSDT", "XRPUSDT", "ADAUSDT",
            "ZECUSDT", "DASHUSDT", "NEARUSDT", "BCHUSDT", "LINKUSDT", "AVAXUSDT", "UNIUSDT",
            "FILUSDT", "DOTUSDT", "AAVEUSDT", "INJUSDT",
        ] {
            assert!(
                symbols.contains(&required.to_owned()),
                "missing {required}: {symbols:?}"
            );
        }
        for excluded in [
            "SUIUSDT",
            "1000PEPEUSDT",
            "ONDOUSDT",
            "TONUSDT",
            "WLDUSDT",
            "ENAUSDT",
        ] {
            assert!(
                !symbols.contains(&excluded.to_owned()),
                "short-history symbol should not be default: {excluded}"
            );
        }
    }

    fn candidate_output_fixture(
        id: &str,
        symbol: &str,
        total_return_pct: f64,
        max_drawdown_pct: f64,
        planned_margin_quote: f64,
    ) -> CandidateOutput {
        CandidateOutput {
            candidate_id: id.to_owned(),
            rank: 1,
            score: total_return_pct,
            config: serde_json::json!({
                "direction_mode": "long_only",
                "strategies": [{
                    "symbol": symbol,
                    "leverage": 3,
                    "spacing": { "fixed_percent": { "step_bps": 100 } },
                    "sizing": { "multiplier": { "first_order_quote": "10", "multiplier": "1.5", "max_legs": 4 } },
                    "take_profit": { "percent": { "bps": 80 } }
                }]
            }),
            summary: serde_json::json!({}),
            artifact_path: format!("/tmp/{id}.json"),
            checksum_sha256: "sha256".to_owned(),
            used_trade_refinement: false,
            used_drawdown_limit_pct: 25.0,
            risk_relaxed: false,
            total_return_pct,
            max_drawdown_pct,
            trade_count: 100,
            annualized_return_pct: Some(total_return_pct / 2.0),
            return_drawdown_ratio: if max_drawdown_pct > 0.0 {
                Some(total_return_pct / max_drawdown_pct)
            } else {
                None
            },
            planned_margin_quote: Some(planned_margin_quote),
            max_leverage_used: Some(3.0),
            equity_curve: vec![
                backtest_engine::martingale::metrics::EquityPoint {
                    timestamp_ms: 1,
                    equity_quote: planned_margin_quote,
                },
                backtest_engine::martingale::metrics::EquityPoint {
                    timestamp_ms: 2,
                    equity_quote: planned_margin_quote * (1.0 + total_return_pct / 100.0),
                },
            ],
            drawdown_curve: Vec::new(),
            trades_preview: Vec::new(),
        }
    }

    #[test]
    fn portfolio_pool_keeps_qualified_high_return_and_low_drawdown_tiers_per_symbol() {
        let outputs = vec![
            candidate_output_fixture("btc-safe", "BTCUSDT", 18.0, 8.0, 100.0),
            candidate_output_fixture("btc-growth", "BTCUSDT", 80.0, 42.0, 100.0),
            candidate_output_fixture("btc-loss", "BTCUSDT", -5.0, 4.0, 100.0),
            candidate_output_fixture("eth-safe", "ETHUSDT", 12.0, 6.0, 100.0),
            candidate_output_fixture("eth-growth", "ETHUSDT", 70.0, 38.0, 100.0),
        ];

        let pool = select_portfolio_pool_outputs_v2(outputs, 25.0, 10, 10, 5);
        let ids: std::collections::BTreeSet<_> =
            pool.iter().map(|o| o.candidate_id.as_str()).collect();

        assert!(ids.contains("btc-safe"));
        assert!(ids.contains("btc-growth"));
        assert!(ids.contains("eth-safe"));
        assert!(ids.contains("eth-growth"));
        assert!(!ids.contains("btc-loss"));
    }

    #[test]
    fn profit_optimized_v2_selection_keeps_tail_parameter_candidates() {
        let space = StagedMartingaleSearchSpace::profit_optimized_v2("aggressive", "long_short");
        let candidates = backtest_engine::search::generate_staged_candidates_for_symbol(
            "BTCUSDT",
            "long_short",
            &space,
            96,
        )
        .expect("v2 candidates should generate");

        // Verify tail coverage: wide spacing, high take-profit, high leverage
        let has_wide_spacing = candidates.iter().any(|c| {
            c.config
                .strategies
                .iter()
                .any(|s| strategy_spacing_bps(s).unwrap_or(0) >= 600)
        });
        let has_high_tp = candidates.iter().any(|c| {
            c.config
                .strategies
                .iter()
                .any(|s| strategy_take_profit_bps(s).unwrap_or(0) >= 300)
        });
        let has_high_leverage = candidates.iter().any(|c| {
            c.config
                .strategies
                .iter()
                .any(|s| s.leverage.unwrap_or(1) >= 10)
        });

        assert!(
            has_wide_spacing,
            "v2 must include wide spacing candidates (>=600)"
        );
        assert!(
            has_high_tp,
            "v2 must include high take-profit candidates (>=300)"
        );
        assert!(
            has_high_leverage,
            "v2 must include high leverage candidates (>=10)"
        );
    }

    fn build_portfolio_summary_for_test(
        pool_count: usize,
        universe_count: usize,
        top_n: usize,
        note: Option<&str>,
    ) -> serde_json::Value {
        let mut summary = serde_json::json!({
            "portfolio_pool_candidate_count": pool_count,
            "expanded_universe_symbol_count": universe_count,
            "portfolio_top_n": top_n,
        });
        if let Some(note_str) = note {
            summary["portfolio_pool_note"] = serde_json::Value::String(note_str.to_owned());
        }
        summary
    }

    #[test]
    fn extended_universe_summary_reports_portfolio_top10_and_pool_counts() {
        let summary = build_portfolio_summary_for_test(
            42,
            18,
            10,
            Some(
                "positive-return candidates include qualified, high-return and low-drawdown tiers",
            ),
        );

        assert_eq!(
            summary
                .get("portfolio_pool_candidate_count")
                .and_then(|v| v.as_u64()),
            Some(42)
        );
        assert_eq!(
            summary
                .get("expanded_universe_symbol_count")
                .and_then(|v| v.as_u64()),
            Some(18)
        );
        assert_eq!(
            summary.get("portfolio_top_n").and_then(|v| v.as_u64()),
            Some(10)
        );
        assert!(summary
            .get("portfolio_pool_note")
            .and_then(|v| v.as_str())
            .unwrap()
            .contains("high-return"));
    }

    #[test]
    fn explicit_symbols_are_not_replaced_by_expanded_universe() {
        let mut config = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()],
            ..WorkerTaskConfig::default()
        };
        config.extended_universe = Some(true);

        let effective = effective_search_symbols(&config);
        assert_eq!(effective, vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()]);
    }

    #[test]
    fn long_short_screening_samples_late_dimension_values_not_only_prefix() {
        let task = WorkerTaskConfig {
            symbols: vec!["BTCUSDT".to_owned()],
            direction_mode: Some("long_short".to_owned()),
            risk_profile: "balanced".to_owned(),
            random_candidates: 16,
            intelligent_rounds: 1,
            per_symbol_top_n: 10,
            search_space: Some(serde_json::json!({
                "leverage": [2],
                "spacing_bps": [120],
                "order_multiplier": [1.25],
                "max_legs": [3],
                "take_profit_bps": [60],
                "tail_stop_bps": [2000],
                "long_short_weight_pct": [[60, 40], [50, 50]]
            })),
            ..WorkerTaskConfig::default()
        };

        let staged = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");
        let candidates = generate_long_short_candidates_for_task("BTCUSDT", &task, &staged)
            .expect("candidates should generate");

        let mut spacings = std::collections::BTreeSet::new();
        let mut take_profits = std::collections::BTreeSet::new();
        let mut stops = std::collections::BTreeSet::new();

        for candidate in &candidates {
            let long = candidate
                .config
                .strategies
                .iter()
                .find(|s| s.direction == MartingaleDirection::Long)
                .unwrap();
            let short = candidate
                .config
                .strategies
                .iter()
                .find(|s| s.direction == MartingaleDirection::Short)
                .unwrap();
            spacings.insert((
                strategy_spacing_bps(long).unwrap(),
                strategy_spacing_bps(short).unwrap(),
            ));
            take_profits.insert((
                strategy_take_profit_bps(long).unwrap(),
                strategy_take_profit_bps(short).unwrap(),
            ));
            if let Some(MartingaleStopLossModel::StrategyDrawdownPct { pct_bps }) = &long.stop_loss
            {
                stops.insert(*pct_bps);
            }
        }

        assert!(
            spacings.len() >= 8,
            "expected broad long/short spacing pairs: {spacings:?}"
        );
        assert!(
            take_profits.len() >= 4,
            "expected broad TP pairs: {take_profits:?}"
        );
        assert!(
            stops.len() >= 3,
            "expected multiple stop-loss ranges: {stops:?}"
        );
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
