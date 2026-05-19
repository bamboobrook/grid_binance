# Martingale Empty Selection Smoke Follow-up Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the deployed martingale `long_short` smoke where a task evaluates candidates but persists zero candidates/zero portfolios while still marking the task `succeeded`.

**Architecture:** Keep Claude's previous direction-preservation work, but add a real worker-path regression for the post-filter selection stage. The worker must either persist enough valid candidates for the requested search or fail with a clear diagnostic; it must never report `succeeded` with `selected_count=0` for a normal two-symbol `long_short` smoke payload.

**Tech Stack:** Rust workspace (`backtest-worker`, `backtest-engine`, `api-server`), PostgreSQL/Docker smoke, existing market data DB, Node verification contracts.

---

## Verified Failure Evidence

After merging Claude commits into `main` and deploying Docker services on 2026-05-19:

- Merge commit: `d2245ef merge: 修复思路 合并马丁多空烟测修复`
- Services healthy after deploy:
  - `grid-binance-api-server-1` healthy
  - `grid-binance-backtest-worker-1` up
  - `grid-binance-web-1` healthy
  - `grid-binance-nginx-1` healthy
  - `http://127.0.0.1:8080/nginx-health` returned `ok`
  - `http://127.0.0.1:8080/api/healthz` returned `service_up{service="api-server"} 1`
- Real smoke task created with:
  ```json
  {
    "strategy_type": "martingale",
    "symbols": ["BTCUSDT", "ETHUSDT"],
    "direction": "long_short",
    "direction_mode": "long_short",
    "risk_profile": "balanced",
    "search_space": {
      "leverage": [2],
      "spacing_bps": [120],
      "order_multiplier": [1.25],
      "max_legs": [3],
      "take_profit_bps": [60],
      "tail_stop_bps": [2000],
      "long_short_weight_pct": [[60, 40], [50, 50]]
    }
  }
  ```
- Smoke task id: `bt_1779161968643214142`
- Task finished as `succeeded`, but no candidates were persisted:
  ```sql
  select task_id,status,started_at,completed_at,error_message
  from backtest_tasks
  where task_id='bt_1779161968643214142';
  ```
  Actual: `status=succeeded`, `error_message` empty.
- Candidate validation failed:
  ```sql
  select count(*) as candidates,
         count(*) filter (where summary->>'direction' in ('long_short','LongShort','long+short')) as long_short_candidates,
         count(*) filter (where summary->>'annualized_return_pct' is not null) as annualized_candidates,
         count(*) filter (where jsonb_array_length(coalesce(summary->'equity_curve','[]'::jsonb))>0
                           and jsonb_array_length(coalesce(summary->'drawdown_curve','[]'::jsonb))>0) as curve_candidates,
         count(*) filter (where (summary->>'recommended_leverage')::numeric >= 2
                           or (summary->>'max_leverage_used')::numeric >= 2) as leverage_candidates
  from backtest_candidate_summaries
  where task_id='bt_1779161968643214142';
  ```
  Actual: all counts are `0`.
- Portfolio validation failed:
  ```sql
  select jsonb_array_length(summary->'portfolio_top3') as portfolio_count,
         jsonb_array_length((summary->'portfolio_top3'->0)->'members') as first_member_count,
         ((summary->'portfolio_top3'->0)->>'annualized_return_pct') as first_annualized,
         jsonb_array_length(coalesce((summary->'portfolio_top3'->0)->'equity_curve','[]'::jsonb)) as first_equity_len,
         jsonb_array_length(coalesce((summary->'portfolio_top3'->0)->'drawdown_curve','[]'::jsonb)) as first_drawdown_len
  from backtest_tasks
  where task_id='bt_1779161968643214142';
  ```
  Actual: `portfolio_count=0`, no first member/curve.
- Task summary proves the worker knowingly completed empty:
  ```json
  {
    "stage": "completed",
    "stage_label": "已完成",
    "progress_pct": 100,
    "portfolio_top3": [],
    "portfolio_top_n": 3,
    "eligible_candidates": [],
    "eligible_candidate_count": 0,
    "portfolio_top3_artifact_path": "/var/lib/grid-binance/backtest-artifacts/bt_1779161968643214142/portfolio-top3.jsonl"
  }
  ```
- Task events show the failure boundary:
  ```text
  running             {"worker":"backtest-worker"}
  heartbeat           {"stage":"market_data_opening"}
  heartbeat           {"stage":"search_started"}
  screening_completed {"screened_count":40,"selected_count":0}
  completed           {}
  ```

## Root Cause Hypotheses To Verify

Do not implement until a failing test identifies the exact boundary.

1. `run_long_short_staged_search()`/`run_profit_first_staged_search()` can evaluate candidates but every candidate has `score.survival_valid=false`, so `process_task()` filters all out.
2. The fixed `long_short` generator may produce valid two-leg configs, but the scoring/drawdown filter is too strict for the smoke payload and lacks fallback/risk-relaxation, causing zero persisted candidates.
3. `select_refinement_candidates_with_drawdown_metadata()` may intentionally drop invalid candidates, but the worker then incorrectly treats empty selection as success.
4. `save_candidates_and_artifacts()` is only called with `outputs=[]`, so candidate APIs cannot show annualized/curves/leverage even though `screened_count=40` existed upstream.

## File Map

- Modify: `apps/backtest-worker/src/main.rs`
  - Add regression tests around `process_task` selection behavior.
  - Add diagnostics for zero-selection cases.
  - Ensure non-empty selected outputs or explicit failure.
- Modify if needed: `apps/backtest-engine/src/search.rs`
  - If candidate generation/scoring marks all smoke candidates invalid, adjust the staged candidate generation or scoring contract, not by weakening global production risk rules silently.
- Modify if needed: `apps/backtest-engine/src/intelligent_search.rs`
  - If survivor handling loses best positive-but-risk-relaxed candidates, add tested fallback selection.
- Modify: `tests/verification/backtest_worker_contract.test.mjs`
  - Add source-level contract that `selected_count=0` cannot be completed silently.
- Modify if needed: `apps/api-server/tests/martingale_backtest_flow.rs`
  - Only if API status/error semantics need coverage.

---

### Task 1: Add a failing worker test for empty selected outputs

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add a unit test for selection fallback/failure semantics**

Add this test inside the existing `#[cfg(test)] mod tests` in `apps/backtest-worker/src/main.rs`. Use existing helper constructors where present; if helper names differ, keep the assertions unchanged.

```rust
#[test]
fn zero_selected_candidates_is_not_reported_as_success() {
    let config = WorkerTaskConfig {
        strategy_type: "martingale".to_owned(),
        symbols: vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()],
        direction_mode: Some("long_short".to_owned()),
        risk_profile: "balanced".to_owned(),
        random_seed: 1,
        random_candidates: 16,
        intelligent_rounds: 1,
        per_symbol_top_n: 10,
        top_n: 10,
        portfolio_top_n: 3,
        search_mode: Some("staged".to_owned()),
        market: Some("usd_m_futures".to_owned()),
        margin_mode: Some("isolated".to_owned()),
        search_space: serde_json::json!({
            "leverage": [2],
            "spacing_bps": [120],
            "order_multiplier": [1.25],
            "max_legs": [3],
            "take_profit_bps": [60],
            "tail_stop_bps": [2000],
            "long_short_weight_pct": [[60, 40], [50, 50]]
        }),
        ..WorkerTaskConfig::default_for_tests()
    };

    let screened = search_candidates_with_drawdown_relaxation(&config, None, |_symbol, _drawdown_limit_pct| {
        Ok(vec![evaluated_candidate_for_tests(
            "invalid-long-short",
            "BTCUSDT",
            shared_domain::martingale::MartingaleDirectionMode::LongAndShort,
            2,
            10.0,
            99.0,
            false,
        )])
    })
    .expect("search should execute");

    assert!(screened.is_empty(), "test setup must reproduce zero selected candidates");

    let error = ensure_non_empty_selection_for_task(&config, screened.len(), 2)
        .expect_err("zero selected candidates must be an error, not a successful task");
    assert!(
        error.contains("no martingale candidates selected")
            && error.contains("screened_count=2")
            && error.contains("direction_mode=long_short"),
        "error should be actionable: {error}"
    );
}
```

Also add minimal test helpers if they do not already exist:

```rust
#[cfg(test)]
fn evaluated_candidate_for_tests(
    candidate_id: &str,
    symbol: &str,
    direction_mode: shared_domain::martingale::MartingaleDirectionMode,
    leverage: u32,
    return_pct: f64,
    max_drawdown_pct: f64,
    survival_valid: bool,
) -> backtest_engine::intelligent_search::EvaluatedCandidate {
    use backtest_engine::intelligent_search::{CandidateScore, EvaluatedCandidate};
    use backtest_engine::search::SearchCandidate;
    use rust_decimal::Decimal;
    use shared_domain::martingale::{
        MartingaleDirection, MartingaleMarginMode, MartingaleMarketKind, MartingalePortfolioConfig,
        MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel, MartingaleStrategyConfig,
        MartingaleTakeProfitModel,
    };

    let strategies = match direction_mode {
        shared_domain::martingale::MartingaleDirectionMode::LongAndShort => vec![
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
            rank_score: return_pct,
            total_return_pct: return_pct,
            annualized_return_pct: Some(return_pct),
            max_drawdown_pct,
            return_drawdown_ratio: if max_drawdown_pct > 0.0 { return_pct / max_drawdown_pct } else { 0.0 },
            survival_valid,
            trade_count: 10,
        },
    }
}
```

If local `CandidateScore` fields differ, adapt field names to the real struct while preserving semantics.

- [ ] **Step 2: Add `ensure_non_empty_selection_for_task()` as the tested seam**

Before implementation, add the test call only; it should fail to compile because `ensure_non_empty_selection_for_task()` does not exist. This is intentional.

- [ ] **Step 3: Run the failing test**

Run:

```bash
cargo test -p backtest-worker zero_selected_candidates_is_not_reported_as_success -- --nocapture
```

Expected before implementation: FAIL/compile error for missing `ensure_non_empty_selection_for_task`, or FAIL because empty selection is allowed.

- [ ] **Step 4: Implement the minimal helper**

In `apps/backtest-worker/src/main.rs`, add a pure helper near selection functions:

```rust
fn ensure_non_empty_selection_for_task(
    config: &WorkerTaskConfig,
    selected_count: usize,
    screened_count: usize,
) -> Result<(), String> {
    if selected_count > 0 {
        return Ok(());
    }
    Err(format!(
        "no martingale candidates selected: strategy_type={} direction_mode={} symbols={} screened_count={} selected_count=0 risk_profile={}",
        config.strategy_type,
        config.direction_mode.as_deref().unwrap_or("long"),
        config.symbols.join(","),
        screened_count,
        config.risk_profile,
    ))
}
```

Then call it in `process_task()` immediately after final selected/refined outputs are known and before `save_candidates_and_artifacts()` or `mark_completed()`.

- [ ] **Step 5: Re-run the focused test**

Run:

```bash
cargo test -p backtest-worker zero_selected_candidates_is_not_reported_as_success -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-worker/src/main.rs
git commit -m "fix: 问题描述 阻止马丁回测空候选误报成功"
```

---

### Task 2: Preserve best risk-relaxed candidates when strict survival filters empty everything

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify if needed: `apps/backtest-engine/src/intelligent_search.rs`

- [ ] **Step 1: Add a regression test for fallback candidate retention**

Add this test inside `apps/backtest-worker/src/main.rs` tests:

```rust
#[test]
fn selection_keeps_best_positive_candidates_when_survival_filter_is_empty() {
    let config = WorkerTaskConfig {
        strategy_type: "martingale".to_owned(),
        symbols: vec!["BTCUSDT".to_owned()],
        direction_mode: Some("long_short".to_owned()),
        risk_profile: "balanced".to_owned(),
        per_symbol_top_n: 10,
        top_n: 10,
        ..WorkerTaskConfig::default_for_tests()
    };

    let candidates = vec![
        evaluated_candidate_for_tests(
            "bad-negative", "BTCUSDT",
            shared_domain::martingale::MartingaleDirectionMode::LongAndShort,
            2, -5.0, 10.0, false,
        ),
        evaluated_candidate_for_tests(
            "best-positive-risk-relaxed", "BTCUSDT",
            shared_domain::martingale::MartingaleDirectionMode::LongAndShort,
            2, 12.0, 28.0, false,
        ),
        evaluated_candidate_for_tests(
            "second-positive-risk-relaxed", "BTCUSDT",
            shared_domain::martingale::MartingaleDirectionMode::LongAndShort,
            2, 8.0, 24.0, false,
        ),
    ];

    let selected = select_candidates_or_best_fallback_for_task(
        &config,
        candidates,
        25.0,
    );

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].candidate.candidate.candidate_id, "second-positive-risk-relaxed");
    assert!(selected[0].risk_relaxed);
    assert_eq!(selected[0].used_drawdown_limit_pct, 25.0);
}
```

Intent:

- Negative-return candidates must still be discarded.
- If strict survival leaves no candidates, the worker may keep the best positive candidate that is closest to the drawdown limit, rather than succeeding with zero results.
- For `balanced`, a candidate at 24% drawdown should be preferred over 28% when the configured limit is 25%.

- [ ] **Step 2: Run the test before implementation**

Run:

```bash
cargo test -p backtest-worker selection_keeps_best_positive_candidates_when_survival_filter_is_empty -- --nocapture
```

Expected: FAIL/compile error until helper exists.

- [ ] **Step 3: Implement fallback selection as a pure helper**

Add a helper near `search_candidates_with_drawdown_relaxation()`:

```rust
fn select_candidates_or_best_fallback_for_task(
    config: &WorkerTaskConfig,
    candidates: Vec<backtest_engine::intelligent_search::EvaluatedCandidate>,
    drawdown_limit_pct: f64,
) -> Vec<EvaluatedCandidateWithDrawdown> {
    let mut valid: Vec<_> = candidates
        .iter()
        .filter(|candidate| candidate.score.survival_valid)
        .cloned()
        .map(|candidate| EvaluatedCandidateWithDrawdown {
            candidate,
            used_drawdown_limit_pct: drawdown_limit_pct,
            risk_relaxed: false,
        })
        .collect();
    if !valid.is_empty() {
        return valid;
    }

    let mut fallback: Vec<_> = candidates
        .into_iter()
        .filter(|candidate| candidate.score.total_return_pct > 0.0)
        .filter(|candidate| candidate.score.max_drawdown_pct <= drawdown_limit_pct)
        .collect();
    fallback.sort_by(|a, b| {
        b.score
            .rank_score
            .total_cmp(&a.score.rank_score)
            .then_with(|| a.score.max_drawdown_pct.total_cmp(&b.score.max_drawdown_pct))
    });
    fallback
        .into_iter()
        .take(config.per_symbol_top_n.max(1))
        .map(|candidate| EvaluatedCandidateWithDrawdown {
            candidate,
            used_drawdown_limit_pct: drawdown_limit_pct,
            risk_relaxed: true,
        })
        .collect()
}
```

If the real `CandidateScore` names differ, adapt to actual fields. Keep the key behavior:

- never include negative-return candidates;
- never include candidates above the current drawdown limit in this fallback;
- mark fallback rows `risk_relaxed=true` so UI can warn that strict survival did not pass.

- [ ] **Step 4: Wire helper into the production search loop**

In `process_task()` where it currently does:

```rust
let valid: Vec<EvaluatedCandidateWithDrawdown> = candidates
    .into_iter()
    .filter(|candidate| candidate.score.survival_valid)
    ...
```

replace with the helper:

```rust
let valid = select_candidates_or_best_fallback_for_task(
    &task.config,
    candidates,
    *drawdown_limit_pct,
);
```

Keep the existing drawdown-limit sequence behavior: if a stricter limit finds fallback candidates, do not jump to wider limits unless that fallback is empty.

- [ ] **Step 5: Re-run tests**

Run:

```bash
cargo test -p backtest-worker selection_keeps_best_positive_candidates_when_survival_filter_is_empty -- --nocapture
cargo test -p backtest-worker zero_selected_candidates_is_not_reported_as_success -- --nocapture
cargo test -p backtest-worker -- --nocapture
```

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-worker/src/main.rs
git commit -m "fix: 修复思路 保留马丁风险内正收益候选"
```

---

### Task 3: Add source contract for non-empty completion semantics

**Files:**
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] **Step 1: Add contract test**

Append:

```js
test("worker cannot complete martingale tasks with zero selected candidates silently", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /ensure_non_empty_selection_for_task/);
  assert.match(worker, /no martingale candidates selected/);
  assert.match(worker, /screened_count/);
  assert.match(worker, /selected_count=0/);
  assert.match(worker, /select_candidates_or_best_fallback_for_task/);
});
```

- [ ] **Step 2: Run contract**

Run:

```bash
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/verification/backtest_worker_contract.test.mjs
git commit -m "test: 问题描述 锁定马丁空候选完成契约"
```

---

### Task 4: Docker smoke must prove non-empty candidates and portfolios

**Files:**
- No source changes unless Task 4 fails; if it fails, do not paper over with SQL-only changes—return to Tasks 1/2 root cause.

- [ ] **Step 1: Run focused local verification**

Run:

```bash
cargo test -p backtest-worker zero_selected_candidates_is_not_reported_as_success -- --nocapture
cargo test -p backtest-worker selection_keeps_best_positive_candidates_when_survival_filter_is_empty -- --nocapture
cargo test -p backtest-worker long_short_task_produces_long_and_short_candidates_via_intelligent_search -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: all PASS with at least one test executed in every `cargo test` command.

- [ ] **Step 2: Build and restart Docker services**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  build api-server backtest-worker web

docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  up -d --no-deps api-server backtest-worker web
```

Expected: build exits `0`, services become healthy.

- [ ] **Step 3: Re-run the exact smoke payload**

Create a new task with the exact JSON in the failure evidence. Wait until terminal status.

Expected terminal status:

- Prefer `succeeded` with non-empty candidates/portfolio.
- If no candidates can be found, status must be `failed` with `error_message` containing `no martingale candidates selected`; it must not be `succeeded` with empty candidates.

- [ ] **Step 4: Validate SQL success case**

If task succeeds, run:

```sql
select count(*) as candidates,
       count(*) filter (where summary->>'direction' in ('long_short','LongShort','long+short')) as long_short_candidates,
       count(*) filter (where summary->>'annualized_return_pct' is not null) as annualized_candidates,
       count(*) filter (where jsonb_array_length(coalesce(summary->'equity_curve','[]'::jsonb))>0
                         and jsonb_array_length(coalesce(summary->'drawdown_curve','[]'::jsonb))>0) as curve_candidates,
       count(*) filter (where (summary->>'recommended_leverage')::numeric >= 2
                         or (summary->>'max_leverage_used')::numeric >= 2) as leverage_candidates
from backtest_candidate_summaries
where task_id='<NEW_TASK_ID>';
```

Expected:

- `candidates >= 10`
- `long_short_candidates >= 1`
- `annualized_candidates = candidates`
- `curve_candidates = candidates`
- `leverage_candidates >= 1`

Then run:

```sql
select jsonb_array_length(summary->'portfolio_top3') as portfolio_count,
       jsonb_array_length((summary->'portfolio_top3'->0)->'members') as first_member_count,
       ((summary->'portfolio_top3'->0)->>'annualized_return_pct') as first_annualized,
       jsonb_array_length(coalesce((summary->'portfolio_top3'->0)->'equity_curve','[]'::jsonb)) as first_equity_len,
       jsonb_array_length(coalesce((summary->'portfolio_top3'->0)->'drawdown_curve','[]'::jsonb)) as first_drawdown_len
from backtest_tasks
where task_id='<NEW_TASK_ID>';
```

Expected:

- `portfolio_count >= 1`
- `first_member_count >= 2`
- `first_annualized` not null/empty
- `first_equity_len > 0`
- `first_drawdown_len > 0`

- [ ] **Step 5: Validate SQL failure case if no candidates exist**

If task fails, run:

```sql
select status, error_message, summary
from backtest_tasks
where task_id='<NEW_TASK_ID>';
```

Expected:

- `status = failed`
- `error_message` contains `no martingale candidates selected`
- `summary.stage` should not be `completed`

- [ ] **Step 6: Commit verification docs only if changed**

If this plan or smoke notes are updated, commit them:

```bash
git add docs/superpowers/plans/2026-05-19-martingale-empty-selection-smoke-followup-plan.md
git commit -m "docs: 复现路径 记录马丁空候选烟测修复计划"
```

---

## Reviewer Handoff Checklist

Claude must report:

- Commit hashes for Tasks 1-3.
- Output of each focused test in Task 4 Step 1, showing non-zero executed tests.
- New Docker smoke task id.
- Terminal status of the smoke task.
- SQL counts proving either:
  - success with `candidates >= 10`, `long_short_candidates >= 1`, curves/annualized/leverage present, and portfolio details present; or
  - failure with a clear `no martingale candidates selected` error, not a false success.

## Do Not Do

- Do not mark a martingale task `succeeded` when `selected_count=0`.
- Do not bypass candidate persistence by fabricating placeholder candidates.
- Do not include negative-return candidates in fallback selection.
- Do not weaken max-drawdown rules beyond the current drawdown limit in this fix.
- Do not touch unrelated host port `3000`; grid frontend is exposed on host `8080`.
