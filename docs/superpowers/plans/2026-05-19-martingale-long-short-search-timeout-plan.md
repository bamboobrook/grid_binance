# Martingale Long/Short Search Timeout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the deployed BTC/ETH `long_short` martingale smoke that now hangs in `search_started` after the search-space expansion, instead of completing with usable candidates/portfolios.

**Architecture:** Keep existing false-success and rejection-diagnostics safeguards. Add bounded search limits, progress heartbeats, and per-symbol/per-stage timeout protection so production tasks cannot remain indefinitely `running`; then tune the expanded `long_short` search space to produce candidates within a predictable smoke runtime.

**Tech Stack:** Rust `backtest-worker`, `backtest-engine` search/scoring, PostgreSQL task events/summary, Docker smoke on host `8080`.

---

## Verified Failure Evidence

After merging and deploying Claude commit `af632a1 fix: 问题描述 移除马丁诊断占位回撤值` via merge commit `ccaf885 merge: 修复思路 合并马丁诊断合同修复`:

- Local verification passed before deploy:
  - `cargo test -p backtest-engine -- --nocapture`
  - `cargo test -p backtest-worker -- --nocapture`
  - `cargo test -p api-server martingale -- --nocapture`
  - `node tests/verification/backtest_worker_contract.test.mjs`
  - `node tests/verification/backtest_console_contract.test.mjs`
  - `pnpm --filter web exec next build --webpack`
- Docker build and service restart succeeded.
- Health checks passed:
  - `api-server` healthy
  - `backtest-worker` up
  - `web` healthy
  - `nginx` healthy
  - `/nginx-health` returned `ok`
  - `/api/healthz` returned `service_up{service="api-server"} 1`

Created exact smoke task:

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

Task id: `bt_1779171820555207563`

After 5+ minutes:

```sql
select task_id,status,started_at,now() - started_at as runtime,updated_at,completed_at,error_message,jsonb_pretty(summary)
from backtest_tasks
where task_id='bt_1779171820555207563';
```

Actual:

```text
status = running
runtime > 00:05:31
updated_at = 2026-05-19 06:23:43.744904+00
completed_at = null
error_message = null
summary = {"stage":"search_started","stage_label":"参数搜索中","progress_pct":30}
```

Events:

```text
running    {"worker":"backtest-worker"}
heartbeat  {"stage":"market_data_opening"}
heartbeat  {"stage":"search_started"}
```

Candidate count:

```sql
select count(*) from backtest_candidate_summaries where task_id='bt_1779171820555207563';
```

Actual: `0`.

Process evidence:

```text
/usr/local/bin/backtest-worker  runtime 09:05  CPU ~64.9%  MEM ~1.1%
```

Worker logs show only startup, no per-symbol/per-round progress:

```text
backtest-worker starting: max_threads=2, poll_ms=5000, artifact_root=..., market_data_db_configured=true, database_url_configured=true, redis_url_configured=true
```

## Root Cause Hypothesis

The last search expansion likely made `run_profit_first_staged_search()` evaluate too many long/short combined candidates over the full 2023-to-last-month 1m dataset without timeout/progress checkpoints. The worker remains CPU-bound inside search/refinement and cannot update task progress or terminate within smoke expectations.

Do not simply increase smoke timeout. Product requirement is an interactive backtest system; user must see progress and the system must not hang indefinitely.

---

### Task 1: Add bounded search-count diagnostics before changing tuning

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add a pure test for search estimate**

Add inside `#[cfg(test)] mod tests`:

```rust
#[test]
fn long_short_smoke_search_estimate_is_bounded() {
    let config = WorkerTaskConfig {
        symbols: vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()],
        direction_mode: Some("long_short".to_owned()),
        risk_profile: "balanced".to_owned(),
        random_candidates: 16,
        intelligent_rounds: 1,
        search_space: serde_json::json!({
            "leverage": [2],
            "spacing_bps": [120],
            "order_multiplier": [1.25],
            "max_legs": [3],
            "take_profit_bps": [60],
            "tail_stop_bps": [2000],
            "long_short_weight_pct": [[60, 40], [50, 50]]
        }),
        ..WorkerTaskConfig::default()
    };

    let estimate = estimate_staged_search_work_for_task(&config);
    assert!(estimate.generated_candidates_per_symbol <= 64, "too many generated candidates per symbol: {:?}", estimate);
    assert!(estimate.max_screenings_per_symbol <= 64, "too many screenings per symbol: {:?}", estimate);
}
```

- [ ] **Step 2: Implement `SearchWorkEstimate`**

Add near search helpers:

```rust
#[derive(Debug, Clone, PartialEq)]
struct SearchWorkEstimate {
    generated_candidates_per_symbol: usize,
    max_screenings_per_symbol: usize,
}

fn estimate_staged_search_work_for_task(config: &WorkerTaskConfig) -> SearchWorkEstimate {
    let direction_mode = config.direction_mode.as_deref().unwrap_or("long");
    let staged = StagedMartingaleSearchSpace::for_profile(&config.risk_profile, direction_mode);
    let space = search_space_from_staged(&staged, "BTCUSDT", config);
    let direction_count = space.directions.len().max(1);
    let weight_count = staged.long_short_weight_pct.len().max(1);
    let generated = space.leverage.len().max(1)
        * space.step_bps.len().max(1)
        * space.multiplier.len().max(1)
        * space.max_legs.len().max(1)
        * space.take_profit_bps.len().max(1)
        * direction_count
        * if direction_mode == "long_short" || direction_mode == "long_and_short" { weight_count } else { 1 };
    let cap = config.random_candidates.max(1) * config.intelligent_rounds.max(1);
    SearchWorkEstimate {
        generated_candidates_per_symbol: generated,
        max_screenings_per_symbol: generated.min(cap.max(1)),
    }
}
```

If real `SearchSpace` field names differ, adapt names while preserving the test contract.

- [ ] **Step 3: Run the test and inspect estimate**

Run:

```bash
cargo test -p backtest-worker long_short_smoke_search_estimate_is_bounded -- --nocapture
```

Expected before tuning may FAIL and print a huge estimate. Use the estimate to guide Task 2.

- [ ] **Step 4: Commit only if this diagnostic compiles and passes after Task 2 tuning**

Do not commit a permanently failing test.

---

### Task 2: Bound the expanded long_short search space and screenings

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify if needed: `apps/backtest-engine/src/search.rs`

- [ ] **Step 1: Reduce/guard neighbor expansion**

Find the code added around `run_profit_first_staged_search()` / `search_space_from_staged()` for long_short single-point expansion. Adjust it so the exact smoke payload does not explode into hundreds/thousands of combinations.

Required policy:

- If user supplies a single explicit value, expand at most one dimension aggressively at a time.
- For exact smoke payload, generated per-symbol candidates must be <= 64.
- Keep `random_candidates` as a hard upper bound for K-line screenings per round unless explicitly configured higher by the task.
- Do not expand `tail_stop_bps` and `take_profit_bps` and `spacing_bps` multiplicatively without a cap.

Suggested minimal expansion for exact smoke:

```rust
spacing_bps: [120, 180, 240]
take_profit_bps: [60, 80]
order_multiplier: [1.15, 1.25]
max_legs: [3]
tail_stop_bps: [2000]
long_short_weight_pct: [[60, 40], [50, 50]]
leverage: [2]
```

This yields `3 * 2 * 2 * 1 * 1 * 2 = 24` per symbol before caps.

- [ ] **Step 2: Enforce cap before expensive screening**

Ensure `run_profit_first_staged_search()` uses `task.random_candidates.max(1)` as a hard cap before expensive `run_candidate_kline_screening()` calls.

If current `intelligent_search()` can call the screening closure more times than `random_candidates * intelligent_rounds`, wrap candidate generation/truncation before invoking it or configure `max_candidates` correctly.

Add a test:

```rust
#[test]
fn long_short_smoke_search_uses_random_candidates_as_screening_cap() {
    let config = WorkerTaskConfig {
        symbols: vec!["BTCUSDT".to_owned()],
        direction_mode: Some("long_short".to_owned()),
        risk_profile: "balanced".to_owned(),
        random_candidates: 16,
        intelligent_rounds: 1,
        search_space: serde_json::json!({
            "leverage": [2],
            "spacing_bps": [120],
            "order_multiplier": [1.25],
            "max_legs": [3],
            "take_profit_bps": [60],
            "tail_stop_bps": [2000],
            "long_short_weight_pct": [[60, 40], [50, 50]]
        }),
        ..WorkerTaskConfig::default()
    };

    let estimate = estimate_staged_search_work_for_task(&config);
    assert!(estimate.generated_candidates_per_symbol <= 64, "estimate: {:?}", estimate);
    assert!(estimate.max_screenings_per_symbol <= 16, "screenings must respect random_candidates: {:?}", estimate);
}
```

- [ ] **Step 3: Add progress heartbeat around per-symbol search**

In `process_task()`, before each symbol/drawdown search, write a heartbeat with useful payload if existing `heartbeat()` supports only stage string then add a small `heartbeat_with_payload()` method.

Required event payload should include:

```json
{
  "stage": "search_symbol",
  "symbol": "BTCUSDT",
  "drawdown_limit_pct": 25.0,
  "estimated_screenings": 16
}
```

If adding a new method is too much, at minimum update summary fields before entering search:

```json
{
  "stage": "search_symbol",
  "stage_label": "搜索 BTCUSDT 参数中",
  "progress_pct": 35,
  "current_symbol": "BTCUSDT",
  "estimated_screenings": 16
}
```

- [ ] **Step 4: Add timeout guard for per-symbol search**

Use `tokio::time::timeout` or a synchronous deadline check, depending on current function shape. Required behavior:

- Default per-symbol search timeout: 120 seconds.
- If exceeded, task fails with error containing `martingale search timed out`, symbol, direction_mode, and estimated screenings.
- Do not leave task `running` forever.

Add a focused test for the pure timeout error formatter if async timeout is hard to unit test:

```rust
#[test]
fn martingale_search_timeout_error_is_actionable() {
    let error = martingale_search_timeout_error("BTCUSDT", "long_short", 16, 120);
    assert!(error.contains("martingale search timed out"));
    assert!(error.contains("BTCUSDT"));
    assert!(error.contains("long_short"));
    assert!(error.contains("estimated_screenings=16"));
}
```

- [ ] **Step 5: Verify focused tests**

Run:

```bash
cargo test -p backtest-worker long_short_smoke_search_estimate_is_bounded -- --nocapture
cargo test -p backtest-worker long_short_smoke_search_uses_random_candidates_as_screening_cap -- --nocapture
cargo test -p backtest-worker martingale_search_timeout_error_is_actionable -- --nocapture
cargo test -p backtest-worker -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-worker/src/main.rs apps/backtest-engine/src/search.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "fix: 修复思路 限制马丁多空搜索耗时"
```

---

### Task 3: Final Docker smoke runtime acceptance

**Files:**
- No source changes unless smoke fails.

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

Expected: all exit `0`.

- [ ] **Step 2: Build/restart Docker services**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  build api-server backtest-worker web

docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  up -d --no-deps api-server backtest-worker web
```

Expected: services healthy.

- [ ] **Step 3: Run exact smoke payload**

Use the exact BTC/ETH balanced `long_short` payload from this plan.

Runtime acceptance:

- Task must reach `succeeded` or actionable `failed` within 180 seconds.
- It must not remain `running` at `search_started` with no heartbeat update.

Functional success acceptance if `succeeded`:

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

Portfolio acceptance:

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

If task fails, acceptance requires:

- `status = failed`
- `error_message` contains either `no martingale candidates selected` with `rejection_diagnostics`, or `martingale search timed out` with symbol/direction/estimate.
- No indefinite `running` task.

---

## Reviewer Handoff Checklist

Claude must report:

- Search estimate before and after tuning.
- Commit hash.
- Full local verification output summary.
- Docker smoke task id and runtime.
- SQL acceptance output.

## Do Not Do

- Do not remove false-success guard.
- Do not fabricate placeholder candidates.
- Do not let tasks run indefinitely without progress events.
- Do not simply increase smoke wait time as the fix.
- Do not touch unrelated host port `3000`; this app is exposed through host `8080`.
