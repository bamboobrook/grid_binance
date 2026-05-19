# Martingale Zero Valid Candidates Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the real deployed `long_short` martingale smoke so it produces usable candidates/portfolios instead of only failing with `no martingale candidates selected`.

**Architecture:** Keep the previous safety fix that prevents false success. Add diagnostics at the candidate rejection boundary, persist a compact rejection report, then adjust the search/scoring path based on evidence so the standard BTC/ETH balanced `long_short` smoke finds valid positive candidates within the configured drawdown limit.

**Tech Stack:** Rust (`backtest-worker`, `backtest-engine`), PostgreSQL task summaries/events, Docker smoke on host port `8080`, existing 1m market data.

---

## Verified Current State

After Claude's latest fix was merged and deployed:

- Merge commit: `7252df3 merge: 修复思路 合并马丁空候选烟测修复`
- Local verification passed:
  - `cargo test -p backtest-worker zero_selected_candidates_is_not_reported_as_success -- --nocapture`
  - `cargo test -p backtest-worker selection_keeps_best_positive_candidates_when_survival_filter_is_empty -- --nocapture`
  - `cargo test -p backtest-worker long_short_task_produces_long_and_short_candidates_via_intelligent_search -- --nocapture`
  - `cargo test -p backtest-engine -- --nocapture`
  - `cargo test -p backtest-worker -- --nocapture`
  - `cargo test -p api-server martingale -- --nocapture`
  - `node tests/verification/backtest_worker_contract.test.mjs`
  - `node tests/verification/backtest_console_contract.test.mjs`
  - `pnpm --filter web exec next build --webpack`
- Deployed services healthy:
  - `api-server` healthy
  - `backtest-worker` up
  - `web` healthy
  - `nginx` healthy
  - `/nginx-health` returned `ok`
  - `/api/healthz` returned `service_up{service="api-server"} 1`

## Real Smoke Failure

Created real task with exact payload:

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

Task id: `bt_1779164786670265471`

Observed result:

```sql
select task_id,status,started_at,completed_at,error_message
from backtest_tasks
where task_id='bt_1779164786670265471';
```

Actual:

```text
status = failed
error_message = no martingale candidates selected: direction_mode=long_short symbols=BTCUSDT,ETHUSDT screened_count=40 selected_count=0 risk_profile=balanced
```

Task events:

```text
running    {"worker":"backtest-worker"}
heartbeat  {"stage":"market_data_opening"}
heartbeat  {"stage":"search_started"}
failed     {"error":"no martingale candidates selected: direction_mode=long_short symbols=BTCUSDT,ETHUSDT screened_count=40 selected_count=0 risk_profile=balanced"}
```

Candidate count:

```sql
select count(*) from backtest_candidate_summaries where task_id='bt_1779164786670265471';
```

Actual: `0`.

This is no longer a false-success bug, but it still fails the product requirement: the default balanced BTC/ETH `long_short` smoke must produce enough usable candidates/portfolio results for the UI.

## Root Cause Questions To Answer With Evidence

Do not tune parameters blindly. First add diagnostics to answer:

1. Are generated `long_short` candidates losing money, exceeding drawdown, having too few trades, or being rejected for another scoring reason?
2. Does `CandidateScore` expose enough rejection reason data, or do we need worker-side classification from `return_pct`, `max_drawdown_pct`, `trade_count`, and `survival_valid`?
3. Does the fallback helper fail because all candidates have negative return, all exceed the drawdown limit, or because `score.total_return_pct`/`score.max_drawdown_pct` fields are not populated as expected?
4. Does `direction_mode=long_short` combine long and short inside one candidate in a way that doubles exposure or distorts drawdown, causing every candidate to fail?
5. Is the smoke search space too narrow for balanced BTC/ETH long_short, and should default smoke/auto-search use wider ranges instead of a single parameter vector?

---

### Task 1: Persist compact rejection diagnostics for zero-selection failures

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] **Step 1: Add a focused unit test for rejection diagnostics shape**

Add inside `apps/backtest-worker/src/main.rs` tests:

```rust
#[test]
fn zero_selection_error_includes_candidate_rejection_diagnostics() {
    let config = WorkerTaskConfig {
        strategy_type: "martingale".to_owned(),
        symbols: vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()],
        direction_mode: Some("long_short".to_owned()),
        risk_profile: "balanced".to_owned(),
        ..WorkerTaskConfig::default_for_tests()
    };

    let diagnostics = CandidateRejectionDiagnostics::from_scores(vec![
        candidate_rejection_sample_for_tests("loss", -2.0, 10.0, 50, false),
        candidate_rejection_sample_for_tests("drawdown", 8.0, 31.0, 120, false),
        candidate_rejection_sample_for_tests("valid", 6.0, 18.0, 80, true),
    ]);

    assert_eq!(diagnostics.total, 3);
    assert_eq!(diagnostics.negative_return_count, 1);
    assert_eq!(diagnostics.drawdown_rejected_count, 1);
    assert_eq!(diagnostics.survival_valid_count, 1);
    assert_eq!(diagnostics.best_by_return[0].candidate_id, "drawdown");

    let error = zero_selection_error(&config, 3, 0, &diagnostics);
    assert!(error.contains("no martingale candidates selected"));
    assert!(error.contains("negative_return=1"));
    assert!(error.contains("drawdown_rejected=1"));
    assert!(error.contains("survival_valid=1"));
}
```

Add helper used only in tests:

```rust
#[cfg(test)]
fn candidate_rejection_sample_for_tests(
    candidate_id: &str,
    total_return_pct: f64,
    max_drawdown_pct: f64,
    trade_count: usize,
    survival_valid: bool,
) -> CandidateRejectionSample {
    CandidateRejectionSample {
        candidate_id: candidate_id.to_owned(),
        total_return_pct,
        max_drawdown_pct,
        trade_count,
        survival_valid,
        direction_mode: "long_short".to_owned(),
        symbol: "BTCUSDT".to_owned(),
    }
}
```

- [ ] **Step 2: Implement diagnostic structs**

Add near selection helpers in `apps/backtest-worker/src/main.rs`:

```rust
#[derive(Debug, Clone, serde::Serialize)]
struct CandidateRejectionSample {
    candidate_id: String,
    symbol: String,
    direction_mode: String,
    total_return_pct: f64,
    max_drawdown_pct: f64,
    trade_count: usize,
    survival_valid: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct CandidateRejectionDiagnostics {
    total: usize,
    survival_valid_count: usize,
    negative_return_count: usize,
    drawdown_rejected_count: usize,
    zero_trade_count: usize,
    best_by_return: Vec<CandidateRejectionSample>,
    lowest_drawdown: Vec<CandidateRejectionSample>,
}
```

Implement:

```rust
impl CandidateRejectionDiagnostics {
    fn from_scores(samples: Vec<CandidateRejectionSample>) -> Self {
        let total = samples.len();
        let survival_valid_count = samples.iter().filter(|s| s.survival_valid).count();
        let negative_return_count = samples.iter().filter(|s| s.total_return_pct <= 0.0).count();
        let drawdown_rejected_count = samples
            .iter()
            .filter(|s| s.total_return_pct > 0.0 && !s.survival_valid)
            .count();
        let zero_trade_count = samples.iter().filter(|s| s.trade_count == 0).count();

        let mut best_by_return = samples.clone();
        best_by_return.sort_by(|a, b| b.total_return_pct.total_cmp(&a.total_return_pct));
        best_by_return.truncate(5);

        let mut lowest_drawdown = samples;
        lowest_drawdown.sort_by(|a, b| a.max_drawdown_pct.total_cmp(&b.max_drawdown_pct));
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
```

Add:

```rust
fn zero_selection_error(
    config: &WorkerTaskConfig,
    screened_count: usize,
    selected_count: usize,
    diagnostics: &CandidateRejectionDiagnostics,
) -> String {
    format!(
        "no martingale candidates selected: direction_mode={} symbols={} screened_count={} selected_count={} risk_profile={} negative_return={} drawdown_rejected={} zero_trade={} survival_valid={}",
        config.direction_mode.as_deref().unwrap_or("long"),
        config.symbols.join(","),
        screened_count,
        selected_count,
        config.risk_profile,
        diagnostics.negative_return_count,
        diagnostics.drawdown_rejected_count,
        diagnostics.zero_trade_count,
        diagnostics.survival_valid_count,
    )
}
```

- [ ] **Step 3: Capture diagnostics in the production loop**

In `process_task()`, while iterating generated/evaluated candidates, collect compact `CandidateRejectionSample` for every evaluated candidate before filtering. Include:

- candidate id;
- symbol;
- direction mode;
- total return pct;
- max drawdown pct;
- trade count;
- survival_valid.

When selected outputs are empty, update task summary before returning error:

```rust
poller
    .update_task_summary_fragment(
        &task.task_id,
        json!({
            "stage": "failed",
            "stage_label": "失败",
            "progress_pct": 100,
            "rejection_diagnostics": diagnostics,
        }),
    )
    .await?;
return Err(zero_selection_error(&task.config, evaluated_count, 0, &diagnostics));
```

If there is no `update_task_summary_fragment`, add a small method to `TaskPoller` that delegates to `repo.update_task_summary()` just like `heartbeat()` does. Keep it private to worker.

- [ ] **Step 4: Add source contract**

Append to `tests/verification/backtest_worker_contract.test.mjs`:

```js
test("worker records rejection diagnostics when martingale selection is empty", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /CandidateRejectionDiagnostics/);
  assert.match(worker, /rejection_diagnostics/);
  assert.match(worker, /negative_return_count/);
  assert.match(worker, /drawdown_rejected_count/);
  assert.match(worker, /best_by_return/);
  assert.match(worker, /lowest_drawdown/);
});
```

- [ ] **Step 5: Verify**

Run:

```bash
cargo test -p backtest-worker zero_selection_error_includes_candidate_rejection_diagnostics -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-worker/src/main.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "fix: 问题描述 记录马丁空候选拒绝诊断"
```

---

### Task 2: Use diagnostics to fix the actual BTC/ETH `long_short` smoke path

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify if evidence points there: `apps/backtest-engine/src/search.rs`
- Modify if evidence points there: `apps/backtest-engine/src/intelligent_search.rs`

- [ ] **Step 1: Run one diagnostic smoke before tuning**

Deploy Task 1 only, then run the exact smoke payload again. Query:

```sql
select status, error_message, jsonb_pretty(summary->'rejection_diagnostics')
from backtest_tasks
where task_id='<DIAGNOSTIC_TASK_ID>';
```

Record the actual counts in the Claude completion note:

- `negative_return_count`
- `drawdown_rejected_count`
- `zero_trade_count`
- `survival_valid_count`
- top `best_by_return` rows
- top `lowest_drawdown` rows

- [ ] **Step 2: Pick the fix based on evidence**

Use exactly one of these branches; do not combine blindly.

**Branch A: all or most candidates have positive return but exceed drawdown.**

Fix fallback to keep candidates within the user risk limit and improve search breadth around lower drawdown:

- Ensure `select_candidates_or_best_fallback_for_task()` filters `max_drawdown_pct <= drawdown_limit_pct`.
- Increase generated candidate breadth for `long_short` when user passes a single narrow value, by adding near-neighbor expansion in `search_space_from_staged()` only for auto-search/smoke:
  - `spacing_bps`: include `80, 120, 160, 200` around `120`;
  - `take_profit_bps`: include `40, 60, 80` around `60`;
  - `tail_stop_bps`: include `2000, 2500, 3000` only if it does not violate drawdown semantics;
  - keep `leverage=[2]` if explicitly set.

Add test:

```rust
#[test]
fn long_short_single_point_search_expands_safe_neighbors() {
    let config = WorkerTaskConfig {
        direction_mode: Some("long_short".to_owned()),
        search_space: serde_json::json!({
            "leverage": [2],
            "spacing_bps": [120],
            "take_profit_bps": [60],
            "long_short_weight_pct": [[60, 40]]
        }),
        ..WorkerTaskConfig::default_for_tests()
    };
    let staged = backtest_engine::search::StagedMartingaleSearchSpace::for_profile("balanced", "long_short");
    let space = search_space_from_staged(&staged, "BTCUSDT", &config);
    assert!(space.step_bps.contains(&80));
    assert!(space.step_bps.contains(&120));
    assert!(space.step_bps.contains(&160));
    assert!(space.take_profit_bps.contains(&40));
    assert!(space.take_profit_bps.contains(&60));
    assert!(space.take_profit_bps.contains(&80));
}
```

**Branch B: all or most candidates have negative return.**

Do not persist negative candidates. Expand the search space toward less aggressive/less fee-heavy configurations:

- Include wider `spacing_bps` neighbors: `120, 180, 240, 300`.
- Include lower multiplier neighbors: `1.1, 1.2, 1.25`.
- Include `max_legs`: `2, 3, 4`.
- Keep `take_profit_bps`: `60, 80, 100`.

Add test asserting expanded values are present.

**Branch C: candidates have zero trades.**

Investigate entry/spacing logic and market data range. Add a test proving generated trigger levels can fire on the loaded 1m data. Do not tune scoring until zero-trade cause is fixed.

**Branch D: `survival_valid_count > 0` but selected remains zero.**

Fix selection plumbing: the valid candidates are being dropped after scoring. Add a test around `select_refinement_candidates_with_drawdown_metadata()` proving valid `long_short` candidates survive ranking and per-symbol quotas.

- [ ] **Step 3: Verify selected branch with focused tests**

Run the new branch-specific test plus:

```bash
cargo test -p backtest-worker zero_selected_candidates_is_not_reported_as_success -- --nocapture
cargo test -p backtest-worker selection_keeps_best_positive_candidates_when_survival_filter_is_empty -- --nocapture
cargo test -p backtest-worker long_short_task_produces_long_and_short_candidates_via_intelligent_search -- --nocapture
cargo test -p backtest-worker -- --nocapture
```

Expected: all PASS.

- [ ] **Step 4: Commit**

Use a commit message matching the actual branch, for example:

```bash
git add apps/backtest-worker/src/main.rs apps/backtest-engine/src/search.rs apps/backtest-engine/src/intelligent_search.rs
git commit -m "fix: 修复思路 扩展马丁多空搜索避免空候选"
```

---

### Task 3: Final Docker smoke must produce usable results

**Files:**
- No code changes unless the smoke fails and evidence points back to Task 2.

- [ ] **Step 1: Run full local verification**

Run:

```bash
cargo test -p backtest-engine -- --nocapture
cargo test -p backtest-worker -- --nocapture
cargo test -p api-server martingale -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
pnpm --filter web exec next build --webpack
```

Expected: all PASS. Note any warnings separately; do not treat `0 tests` filtered binaries as proof by themselves.

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

Expected: build exits `0`, services healthy.

- [ ] **Step 3: Re-run exact BTC/ETH balanced long_short smoke**

Use the exact payload from this plan. Wait until terminal state.

Expected: `status=succeeded` with non-empty persisted candidates and portfolio results.

- [ ] **Step 4: SQL acceptance**

Run:

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

Then:

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

- [ ] **Step 5: Commit verification notes only if files changed**

If this plan was updated with diagnostic evidence, commit it:

```bash
git add docs/superpowers/plans/2026-05-19-martingale-zero-valid-candidates-diagnostics-plan.md
git commit -m "docs: 复现路径 记录马丁零有效候选诊断计划"
```

---

## Reviewer Handoff Checklist

Claude must report:

- Diagnostic smoke task id and `rejection_diagnostics` counts.
- Which Task 2 branch was selected and why.
- Commit hashes for diagnostic and fix commits.
- Final smoke task id.
- SQL acceptance output proving non-empty candidates and portfolio results.

## Do Not Do

- Do not fabricate placeholder candidates.
- Do not allow negative-return fallback candidates.
- Do not silently exceed the user's max drawdown/risk limit.
- Do not remove the previous false-success guard.
- Do not touch unrelated host port `3000`; this app is exposed through host `8080`.
