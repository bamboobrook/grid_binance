# Martingale Parallel Search Performance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make martingale long_short candidate screening and refinement use CPU parallelism controlled by `BACKTEST_WORKER_MAX_THREADS`, then deploy and verify a faster 7-symbol search.

**Architecture:** Keep candidate generation deterministic and single-threaded, then evaluate generated candidates in bounded parallel chunks. Each worker thread only reads `MarketDataContext` and returns an indexed result; the caller reorders by original index before score sorting, preserving deterministic TopN behavior.

**Tech Stack:** Rust 2021, `backtest-worker`, standard library scoped threads, Docker Compose, existing cargo/node verification tests.

---

## File Map

- Modify: `apps/backtest-worker/src/main.rs`
  - Add bounded parallel candidate-screening helpers.
  - Thread `WorkerConfig.max_threads` into `run_profit_first_staged_search()` and `run_long_short_staged_search()`.
  - Replace sequential long_short coarse/fine screening loops with bounded parallel evaluation.
  - Add tests for deterministic parallel ordering and thread-count clamping.
- Modify: `deploy/docker/docker-compose.yml`
  - Change default `BACKTEST_WORKER_MAX_THREADS` from `2` to `24` for local high-performance backtests.
- No change: `apps/backtest-engine/*`
  - Do not change strategy semantics, scoring formula, fee logic, leverage principal calculation, or portfolio construction in this plan.

---

### Task 1: Add Deterministic Parallel Screening Helper Tests

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add helper test functions inside `mod tests`**

Place these near the existing backtest-worker tests, before the current long_short search tests:

```rust
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

    fn sample_parallel_score(candidate_id: &str) -> backtest_engine::martingale::scoring::CandidateScore {
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
```

- [ ] **Step 2: Add failing deterministic order test**

Add this test in `mod tests`:

```rust
    #[test]
    fn parallel_candidate_screening_preserves_input_order() {
        let candidates = (0..8)
            .map(|index| sample_candidate_for_parallel_test(&format!("candidate-{index}")))
            .collect::<Vec<_>>();

        let evaluated = screen_candidates_bounded_parallel(
            candidates,
            4,
            |candidate| {
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
            },
        );

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
```

- [ ] **Step 3: Add failing chunk-size clamp test**

Add this test in `mod tests`:

```rust
    #[test]
    fn bounded_parallelism_clamps_zero_to_one() {
        assert_eq!(bounded_parallel_width(0), 1);
        assert_eq!(bounded_parallel_width(1), 1);
        assert_eq!(bounded_parallel_width(24), 24);
    }
```

- [ ] **Step 4: Run tests and verify they fail before implementation**

Run:

```bash
cargo test -p backtest-worker parallel_candidate_screening_preserves_input_order -- --nocapture
cargo test -p backtest-worker bounded_parallelism_clamps_zero_to_one -- --nocapture
```

Expected: both fail to compile because `screen_candidates_bounded_parallel` and `bounded_parallel_width` are not defined yet.

---

### Task 2: Implement Bounded Parallel Screening Helper

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add bounded thread helper functions**

Add these functions near `martingale_search_timeout_error()` so production and tests can use them:

```rust
fn bounded_parallel_width(max_threads: usize) -> usize {
    max_threads.max(1)
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
        return candidates
            .into_iter()
            .map(evaluator)
            .collect::<Vec<_>>();
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
```

- [ ] **Step 2: Run new helper tests**

Run:

```bash
cargo test -p backtest-worker parallel_candidate_screening_preserves_input_order -- --nocapture
cargo test -p backtest-worker bounded_parallelism_clamps_zero_to_one -- --nocapture
```

Expected: both pass.

- [ ] **Step 3: Commit helper and tests**

Run:

```bash
git add apps/backtest-worker/src/main.rs
git commit -m "test: 修复思路 覆盖马丁候选并行筛选顺序"
```

---

### Task 3: Thread Worker max_threads Into Search Flow

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Change `run_profit_first_staged_search` signature**

Replace:

```rust
fn run_profit_first_staged_search(
    context: &MarketDataContext,
    symbol: &str,
    task: &WorkerTaskConfig,
    scoring: &ScoringConfig,
    _drawdown_limit_pct: f64,
) -> Result<(Vec<EvaluatedCandidate>, Vec<CandidateRejectionSample>), String> {
```

With:

```rust
fn run_profit_first_staged_search(
    context: &MarketDataContext,
    symbol: &str,
    task: &WorkerTaskConfig,
    scoring: &ScoringConfig,
    _drawdown_limit_pct: f64,
    max_threads: usize,
) -> Result<(Vec<EvaluatedCandidate>, Vec<CandidateRejectionSample>), String> {
```

- [ ] **Step 2: Pass `max_threads` into long_short path**

Replace:

```rust
        return run_long_short_staged_search(context, symbol, task, &coarse_space, scoring);
```

With:

```rust
        return run_long_short_staged_search(
            context,
            symbol,
            task,
            &coarse_space,
            scoring,
            max_threads,
        );
```

- [ ] **Step 3: Change `run_long_short_staged_search` signature**

Replace:

```rust
fn run_long_short_staged_search(
    context: &MarketDataContext,
    symbol: &str,
    task: &WorkerTaskConfig,
    staged: &StagedMartingaleSearchSpace,
    scoring: &ScoringConfig,
) -> Result<(Vec<EvaluatedCandidate>, Vec<CandidateRejectionSample>), String> {
```

With:

```rust
fn run_long_short_staged_search(
    context: &MarketDataContext,
    symbol: &str,
    task: &WorkerTaskConfig,
    staged: &StagedMartingaleSearchSpace,
    scoring: &ScoringConfig,
    max_threads: usize,
) -> Result<(Vec<EvaluatedCandidate>, Vec<CandidateRejectionSample>), String> {
```

- [ ] **Step 4: Update process_task call site**

Find the call inside `process_task`:

```rust
            let (candidates, rejection_samples) = run_profit_first_staged_search(
                &market_context,
                symbol,
                &task.config,
                &scoring,
                *drawdown_limit_pct,
            )?;
```

Replace with:

```rust
            let (candidates, rejection_samples) = run_profit_first_staged_search(
                &market_context,
                symbol,
                &task.config,
                &scoring,
                *drawdown_limit_pct,
                config.max_threads,
            )?;
```

- [ ] **Step 5: Run compile check**

Run:

```bash
cargo test -p backtest-worker bounded_parallelism_clamps_zero_to_one -- --nocapture
```

Expected: pass. If Rust reports any missing argument errors in tests, update those test call sites to pass `1` for `max_threads`.

---

### Task 4: Parallelize long_short Coarse Screening

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Extract repeated long_short evaluator function**

Add this function before `run_long_short_staged_search`:

```rust
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
            let score = score_candidate(metrics, scoring);
            let sample = CandidateRejectionSample {
                candidate_id: overridden.candidate_id.clone(),
                symbol: symbol.to_owned(),
                direction_mode: direction_mode.to_owned(),
                total_return_pct: Some(metrics.metrics.total_return_pct),
                max_drawdown_pct: Some(metrics.metrics.max_drawdown_pct),
                trade_count: metrics.metrics.trade_count as usize,
                survival_valid: score.survival_valid,
                rejection_reason: None,
            };
            (score, sample)
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
```

- [ ] **Step 2: Replace coarse sequential loop**

Inside `run_long_short_staged_search`, replace this entire block:

```rust
    let mut evaluated = Vec::new();
    let mut rejection_samples = Vec::new();
    let start = std::time::Instant::now();
    let timeout_secs: u64 = 600;

    for (idx, candidate) in candidates.into_iter().enumerate() {
        if idx > 0 && idx % 5 == 0 {
            if start.elapsed().as_secs() > timeout_secs {
                return Err(martingale_search_timeout_error(
                    symbol,
                    direction_mode,
                    candidate_count,
                    timeout_secs,
                ));
            }
        }
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
        rejection_samples.push(sample);
        evaluated.push(EvaluatedCandidate {
            candidate: overridden,
            score,
        });
    }
```

With:

```rust
    let start = std::time::Instant::now();
    let timeout_secs: u64 = 600;
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
    let mut evaluated = Vec::with_capacity(coarse_results.len());
    let mut rejection_samples = Vec::with_capacity(coarse_results.len());
    for (candidate, sample) in coarse_results {
        evaluated.push(candidate);
        rejection_samples.push(sample);
    }
```

- [ ] **Step 3: Run long_short tests**

Run:

```bash
cargo test -p backtest-worker long_short_candidate_selection_prioritizes_profit_potential_within_budget -- --nocapture
cargo test -p backtest-worker long_short_smoke_payload_expands_to_diverse_dual_leg_candidates -- --nocapture
```

Expected: both pass.

- [ ] **Step 4: Commit coarse parallelization**

Run:

```bash
git add apps/backtest-worker/src/main.rs
git commit -m "fix: 修复思路 并行执行马丁双向粗筛"
```

---

### Task 5: Parallelize long_short Fine Refinement

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Replace fine sequential inner loop**

Inside `run_long_short_staged_search`, replace the current survivor/fine loop body:

```rust
    let mut refined = Vec::new();
    for survivor in &survivors {
        let fine_space = long_short_fine_space_around_candidate(&survivor.candidate);
        let fine_task = task_with_long_short_refinement_space(task, &fine_space);
        let fine_candidates = generate_long_short_candidates_for_task(symbol, &fine_task, staged)?;
        for (fine_index, mut fine_candidate) in fine_candidates.into_iter().enumerate() {
            fine_candidate.candidate_id = format!(
                "{}-fine-{fine_index}-{}",
                survivor.candidate.candidate_id, fine_candidate.candidate_id
            );
            let overridden = apply_task_overrides_to_candidate(fine_candidate, task);
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
            rejection_samples.push(sample);
            refined.push(EvaluatedCandidate {
                candidate: overridden,
                score,
            });
        }
    }
```

With this chunked parallel implementation:

```rust
    let mut refined = Vec::new();
    for survivor in &survivors {
        let fine_space = long_short_fine_space_around_candidate(&survivor.candidate);
        let fine_task = task_with_long_short_refinement_space(task, &fine_space);
        let mut fine_candidates = generate_long_short_candidates_for_task(symbol, &fine_task, staged)?;
        for (fine_index, fine_candidate) in fine_candidates.iter_mut().enumerate() {
            fine_candidate.candidate_id = format!(
                "{}-fine-{fine_index}-{}",
                survivor.candidate.candidate_id, fine_candidate.candidate_id
            );
        }
        let fine_results = screen_candidates_bounded_parallel(
            fine_candidates,
            max_threads,
            |candidate| {
                evaluate_long_short_candidate_for_screening(
                    candidate,
                    context,
                    task,
                    symbol,
                    direction_mode,
                    scoring,
                )
            },
        );
        for (candidate, sample) in fine_results {
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
```

- [ ] **Step 2: Run search tests**

Run:

```bash
cargo test -p backtest-worker long_short_candidate_selection_prioritizes_profit_potential_within_budget -- --nocapture
cargo test -p backtest-worker long_short_smoke_payload_expands_to_diverse_dual_leg_candidates -- --nocapture
cargo test -p backtest-worker explicit_long_short_search_budget_is_respected_for_wide_multisymbol_runs -- --nocapture
```

Expected: all pass.

- [ ] **Step 3: Commit fine parallelization**

Run:

```bash
git add apps/backtest-worker/src/main.rs
git commit -m "fix: 修复思路 并行执行马丁双向精筛"
```

---

### Task 6: Raise Docker Default Thread Budget

**Files:**
- Modify: `deploy/docker/docker-compose.yml`

- [ ] **Step 1: Update default thread count**

Replace this line:

```yaml
      BACKTEST_WORKER_MAX_THREADS: ${BACKTEST_WORKER_MAX_THREADS:-2}
```

With:

```yaml
      BACKTEST_WORKER_MAX_THREADS: ${BACKTEST_WORKER_MAX_THREADS:-24}
```

- [ ] **Step 2: Run static verification**

Run:

```bash
rg -n "BACKTEST_WORKER_MAX_THREADS" deploy/docker/docker-compose.yml apps/backtest-worker/src/main.rs
```

Expected output includes the compose default `24` and the Rust env parser.

- [ ] **Step 3: Commit Docker thread budget**

Run:

```bash
git add deploy/docker/docker-compose.yml
git commit -m "fix: 修复思路 提升马丁回测本机线程预算"
```

---

### Task 7: Full Verification, Deploy, and 7-Symbol Smoke

**Files:**
- No code changes unless verification reveals failures.

- [ ] **Step 1: Run required local verification**

Run:

```bash
cargo test -p backtest-worker parallel_candidate_screening_preserves_input_order -- --nocapture
cargo test -p backtest-worker bounded_parallelism_clamps_zero_to_one -- --nocapture
cargo test -p backtest-worker long_short_candidate_selection_prioritizes_profit_potential_within_budget -- --nocapture
cargo test -p backtest-worker long_short_smoke_payload_expands_to_diverse_dual_leg_candidates -- --nocapture
cargo test -p backtest-worker explicit_long_short_search_budget_is_respected_for_wide_multisymbol_runs -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: all tests pass.

- [ ] **Step 2: Build and deploy worker**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  build backtest-worker

docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  up -d --no-deps --force-recreate backtest-worker
```

Expected: image builds and `grid-binance-backtest-worker-1` starts.

- [ ] **Step 3: Confirm worker thread env**

Run:

```bash
docker inspect grid-binance-backtest-worker-1 --format '{{json .Config.Env}}' | rg 'BACKTEST_WORKER_MAX_THREADS=24'
```

Expected: output contains `BACKTEST_WORKER_MAX_THREADS=24` unless `.env` intentionally overrides it.

- [ ] **Step 4: Create 7-symbol validation task**

Use the existing authenticated token from `/tmp/grid_binance_7sym3_token.txt`. Run:

```bash
TOKEN=$(cat /tmp/grid_binance_7sym3_token.txt)
cat > /tmp/grid_binance_7sym_parallel_payload.json <<'JSON'
{
  "strategy_type": "martingale",
  "symbols": ["BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT"],
  "market": "usd_m_futures",
  "direction_mode": "long_short",
  "risk_profile": "balanced",
  "interval": "1m",
  "random_candidates": 36,
  "intelligent_rounds": 2,
  "per_symbol_top_n": 10,
  "portfolio_top_n": 3,
  "random_seed": 20260520,
  "search_mode": "staged",
  "execution_model": "conservative_futures_isolated",
  "search_space": {
    "leverage": [2, 3, 4, 5, 6],
    "spacing_bps": [60, 80, 120, 160, 220, 300],
    "order_multiplier": [1.15, 1.25, 1.4, 1.6, 2.0],
    "max_legs": [3, 4, 5, 6],
    "take_profit_bps": [50, 70, 100, 130, 180],
    "tail_stop_bps": [1200, 1800, 2400, 3000, 3600],
    "long_short_weight_pct": [[80, 20], [70, 30], [60, 40], [50, 50], [40, 60]]
  }
}
JSON
docker cp /tmp/grid_binance_7sym_parallel_payload.json grid-binance-nginx-1:/tmp/7sym_parallel_payload.json
docker exec grid-binance-nginx-1 sh -lc "wget -q -O - --header='authorization: Bearer $TOKEN' --header='content-type: application/json' --post-file=/tmp/7sym_parallel_payload.json http://api-server:8080/backtest/tasks" | tee /tmp/grid_binance_7sym_parallel_create.json
jq -r '.task_id // .id // empty' /tmp/grid_binance_7sym_parallel_create.json > /tmp/grid_binance_7sym_parallel_task_id.txt
cat /tmp/grid_binance_7sym_parallel_task_id.txt
```

Expected: prints a new task id like `bt_...`.

- [ ] **Step 5: Verify CPU utilization while running**

Run after the task starts:

```bash
docker stats --no-stream grid-binance-backtest-worker-1
```

Expected: CPU is clearly above single-core usage, target at least `800%+` during candidate screening.

- [ ] **Step 6: Poll task to completion or meaningful progress**

Run:

```bash
TASK_ID=$(cat /tmp/grid_binance_7sym_parallel_task_id.txt)
TOKEN=$(cat /tmp/grid_binance_7sym3_token.txt)
for i in $(seq 1 120); do
  RESP=$(docker exec grid-binance-nginx-1 wget -q -O - --header="authorization: Bearer $TOKEN" http://api-server:8080/backtest/tasks/$TASK_ID)
  echo "poll=$i status=$(echo "$RESP" | jq -r .status) stage=$(echo "$RESP" | jq -r '.summary.stage // ""') current=$(echo "$RESP" | jq -r '.summary.current_symbol // ""') eligible=$(echo "$RESP" | jq -r '.summary.eligible_candidate_count // 0') symbols=$(echo "$RESP" | jq -c '.summary.eligible_symbols // []') portfolios=$(echo "$RESP" | jq '.summary.portfolio_top3|length')"
  STATUS=$(echo "$RESP" | jq -r .status)
  if [ "$STATUS" = "completed" ] || [ "$STATUS" = "succeeded" ] || [ "$STATUS" = "failed" ] || [ "$STATUS" = "cancelled" ]; then
    break
  fi
  sleep 15
done
```

Expected: task moves past the first symbol faster than the previous single-core run and eventually produces candidates/portfolios or a clear failure message.

- [ ] **Step 7: Extract Top3 result summary**

Run:

```bash
TASK_ID=$(cat /tmp/grid_binance_7sym_parallel_task_id.txt)
TOKEN=$(cat /tmp/grid_binance_7sym3_token.txt)
docker exec grid-binance-nginx-1 wget -q -O /tmp/7sym_parallel_task.json --header="authorization: Bearer $TOKEN" http://api-server:8080/backtest/tasks/$TASK_ID
docker cp grid-binance-nginx-1:/tmp/7sym_parallel_task.json /tmp/grid_binance_7sym_parallel_task_final.json
jq '{task_id,status,error_message,searched_symbols:.summary.searched_symbols,eligible_symbols:.summary.eligible_symbols,eligible_candidate_count:.summary.eligible_candidate_count,portfolio_count:(.summary.portfolio_top3|length),top3:(.summary.portfolio_top3|map({rank:.portfolio_rank,return_pct,annualized_return_pct,max_drawdown_pct,member_count,portfolio_unique_symbol_count,portfolio_symbols,members}))}' /tmp/grid_binance_7sym_parallel_task_final.json
```

Expected: top3 is present for successful task; if not, diagnostics explain rejected candidates.

- [ ] **Step 8: Push all commits**

Run:

```bash
git push origin main
git status --short --branch
```

Expected: push succeeds and working tree is clean except intentionally untracked local temp files outside repo.

---

## Self-Review Against Spec

- CPU parallelism: Tasks 2, 4, 5 implement bounded parallel candidate screening.
- `BACKTEST_WORKER_MAX_THREADS`: Tasks 3 and 6 wire and raise the setting.
- Accuracy: Plan avoids changing engine/scoring/portfolio semantics.
- Determinism: Task 1 tests input-order preservation; Task 2 preserves index order before scoring.
- Resource control: bounded width uses env-configured max threads.
- Verification: Task 7 includes unit, contract, deploy, CPU utilization, and 7-symbol smoke validation.
- GPU: explicitly out of scope per spec.
