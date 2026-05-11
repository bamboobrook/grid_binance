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
    intelligent_search::{intelligent_search, IntelligentSearchConfig},
    market_data::{AggTrade, KlineBar, MarketDataSource},
    martingale::{
        kline_engine::run_kline_screening, metrics::MartingaleBacktestResult,
        scoring::ScoringConfig, trade_engine::run_trade_refinement,
    },
    search::{random_search, SearchCandidate, SearchSpace},
    sqlite_market_data::SqliteMarketDataSource,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use shared_db::{BacktestRepository, NewBacktestCandidateRecord, SharedDb};

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
            interval: default_interval(),
            start_ms: 0,
            end_ms: 0,
        }
    }
}

fn default_interval() -> String {
    "1h".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CandidateOutput {
    candidate_id: String,
    rank: usize,
    score: f64,
    config: serde_json::Value,
    artifact_path: String,
    checksum_sha256: String,
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
    let random_candidates = random_search(
        &search_space,
        task.config.random_candidates,
        task.config.random_seed,
    )?;
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
            scoring: ScoringConfig::default(),
        },
        Some(cancel.as_ref()),
        |candidate| run_candidate_kline_screening(candidate, &market_context),
    )?;
    cancel.store(true, Ordering::SeqCst);
    let _ = cancel_watcher.join();
    respect_pause_or_cancel(poller, &task.task_id).await?;

    let mut ranked = intelligent.candidates;
    ranked.sort_by(|left, right| right.score.rank_score.total_cmp(&left.score.rank_score));
    let mut outputs = Vec::new();

    for (index, evaluated) in ranked
        .into_iter()
        .take(task.config.top_n.max(1))
        .enumerate()
    {
        poller
            .heartbeat(
                &task.task_id,
                &format!("trade_refinement_top_{}", index + 1),
            )
            .await?;
        let refined = run_candidate_trade_refinement(&evaluated.candidate, &market_context)?;
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
        outputs.push(CandidateOutput {
            candidate_id: evaluated.candidate.candidate_id,
            rank: index + 1,
            score: evaluated.score.rank_score,
            config: serde_json::to_value(&evaluated.candidate.config)
                .map_err(|error| format!("serialize candidate config: {error}"))?,
            artifact_path: manifest.path.display().to_string(),
            checksum_sha256: manifest.checksum_sha256,
        });
        respect_pause_or_cancel(poller, &task.task_id).await?;
    }

    respect_pause_or_cancel(poller, &task.task_id).await?;
    poller
        .save_candidates_and_artifacts(&task.task_id, random_candidates.len(), &outputs)
        .await?;
    respect_pause_or_cancel(poller, &task.task_id).await?;
    poller.mark_completed(&task.task_id).await?;
    Ok(())
}

fn search_space_from_task(config: &WorkerTaskConfig) -> SearchSpace {
    SearchSpace {
        symbols: config.symbols.clone(),
        directions: vec![
            shared_domain::martingale::MartingaleDirection::Long,
            shared_domain::martingale::MartingaleDirection::Short,
        ],
        step_bps: vec![25, 50, 100],
        first_order_quote: vec![Decimal::new(100, 0), Decimal::new(250, 0)],
        take_profit_bps: vec![30, 60, 100],
        leverage: vec![1, 2, 3],
        max_legs: vec![3, 5, 7],
    }
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
            if symbol_trades.is_empty() {
                return Err(format!(
                    "no aggTrades for {normalized} range={}..{}; trade refinement requires成交级数据",
                    config.start_ms, config.end_ms
                ));
            }
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
        return Err(format!(
            "candidate {} has no matching aggTrades",
            candidate.candidate_id
        ));
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
                        summary: json!({ "score": output.score }),
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
            .transition_task(task_id, "succeeded")
            .map_err(|error| format!("mark task completed: {error}"))?;
        self.repo
            .append_task_event(task_id, "completed", json!({}))
            .map_err(|error| format!("append completed event: {error}"))?;
        Ok(())
    }

    async fn mark_failed(&self, task_id: &str, error: &str) -> Result<(), String> {
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
    use std::path::PathBuf;

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
            interval: "1h".to_owned(),
            start_ms: 1,
            end_ms: 2_000,
        };

        let error = MarketDataContext::load(&source, &config).unwrap_err();
        assert!(error.contains("no aggTrades"));
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
