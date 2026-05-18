# Martingale Post-Merge Smoke Fix Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the runtime issues discovered during post-merge Docker smoke tests so Martingale auto-search tasks can complete successfully and expose usable `long_short`, annualized return, charts, trades, and true portfolio details.

**Architecture:** Keep the already-merged Claude implementation as the base. Add regression tests for the exact smoke failures, then patch API normalization, worker config deserialization, worker DB summary size, annualized return calculation, and candidate direction summaries. Do not rely only on string contract tests; verify with one real Docker smoke task.

**Tech Stack:** Rust (`api-server`, `backtest-engine`, `backtest-worker`), Docker Compose runtime, existing backtest API.

---

## Smoke Evidence From Review

Post-merge deployment and smoke testing found these failures:

1. Task `bt_1779094068929427503` failed immediately:
   - Error: `invalid backtest task config ... missing field random_seed`
2. Task `bt_1779094148114025721` failed immediately after adding `random_seed`:
   - Error: `invalid backtest task config ... missing field random_candidates`
3. Task `bt_1779094448441626336` ran to completion stage but failed when saving summary:
   - Error: `update completed summary: error returned from database: total size of jsonb array elements exceeds the maximum of 268435455 bytes`
   - Root cause: Worker writes full 1m `equity_curve`, `drawdown_curve`, and/or trade arrays into DB task summary / candidate summary instead of keeping full data in artifacts and only storing previews in DB.
4. Task `bt_1779094995861065671` succeeded after manual local patch, but exposed remaining result-quality gaps:
   - First candidate `direction` displayed as `long` despite request being `long_short`.
   - `annualized_return_pct` was `null` even though the equity curve had valid timestamps.
   - Candidate and portfolio curves existed after previewing; trades were zero for that tiny smoke, which is acceptable only if no trades occurred.

The local experimental fixes were reverted by commits:

- `c646a5a` reverted annualized/direction patch.
- `6b79278` reverted DB summary preview patch.
- `764f170` reverted worker default config patch.

Claude should reimplement these fixes cleanly with tests.

---

## Task 1: Regression Tests for Worker Config Defaults

**Files:**
- Modify: `apps/api-server/tests/martingale_backtest_flow.rs`
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add API normalization assertions**

In `martingale_auto_search_normalizes_profit_first_contract`, assert normalized martingale config includes Worker-required fields:

```rust
assert_eq!(normalized["random_seed"], 1);
assert_eq!(normalized["random_candidates"], 16);
assert_eq!(normalized["intelligent_rounds"], 1);
assert_eq!(normalized["top_n"], 10);
```

- [ ] **Step 2: Add worker deserialization test**

In `apps/backtest-worker/src/main.rs` tests, add:

```rust
#[test]
fn worker_task_config_deserializes_missing_search_counts_with_defaults() {
    let config: WorkerTaskConfig = serde_json::from_value(json!({
        "symbols": ["BTCUSDT", "ETHUSDT"],
        "risk_profile": "balanced",
        "direction_mode": "long_short",
        "start_ms": 1672531200000_i64,
        "end_ms": 1673308800000_i64
    })).expect("worker config");

    assert_eq!(config.random_seed, 1);
    assert_eq!(config.random_candidates, 16);
    assert_eq!(config.intelligent_rounds, 1);
    assert_eq!(config.top_n, 10);
    assert_eq!(config.per_symbol_top_n, 10);
    assert_eq!(config.portfolio_top_n, 3);
}
```

- [ ] **Step 3: Run tests and verify failure first**

```bash
cargo test -p api-server martingale_auto_search_normalizes_profit_first_contract -- --nocapture
cargo test -p backtest-worker worker_task_config_deserializes_missing_search_counts_with_defaults -- --nocapture
```

Expected before implementation: at least one failure.

---

## Task 2: Fix API Normalization and Worker Defaults

**Files:**
- Modify: `apps/api-server/src/services/backtest_service.rs`
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add API defaults**

In `normalize_martingale_auto_search_config`, insert defaults only if user did not provide values:

```rust
object.entry("random_seed".to_owned()).or_insert_with(|| Value::Number(1.into()));
object.entry("random_candidates".to_owned()).or_insert_with(|| Value::Number(16.into()));
object.entry("intelligent_rounds".to_owned()).or_insert_with(|| Value::Number(1.into()));
object.entry("top_n".to_owned()).or_insert_with(|| Value::Number(10.into()));
```

Do not override explicit user-provided values.

- [ ] **Step 2: Add serde defaults to WorkerTaskConfig**

In `WorkerTaskConfig`:

```rust
#[serde(default = "default_random_seed")]
random_seed: u64,
#[serde(default = "default_random_candidates")]
random_candidates: usize,
#[serde(default = "default_intelligent_rounds")]
intelligent_rounds: usize,
#[serde(default = "default_top_n")]
top_n: usize,
```

Add functions:

```rust
fn default_random_seed() -> u64 { 1 }
fn default_random_candidates() -> usize { 16 }
fn default_intelligent_rounds() -> usize { 1 }
fn default_top_n() -> usize { 10 }
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p api-server martingale_auto_search_normalizes_profit_first_contract -- --nocapture
cargo test -p backtest-worker worker_task_config_deserializes_missing_search_counts_with_defaults -- --nocapture
```

- [ ] **Step 4: Commit**

```bash
git add apps/api-server/src/services/backtest_service.rs apps/api-server/tests/martingale_backtest_flow.rs apps/backtest-worker/src/main.rs
git commit -m "fix: 修复思路 补齐马丁回测任务Worker默认配置"
```

---

## Task 3: Prevent Oversized DB JSON Summaries

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add preview helper test**

Add test:

```rust
#[test]
fn sampled_preview_caps_large_series_and_keeps_edges() {
    let values = (0..1_000).collect::<Vec<_>>();
    let preview = sampled_preview(&values, 10);

    assert_eq!(preview.len(), 10);
    assert_eq!(preview.first().copied(), Some(0));
    assert_eq!(preview.last().copied(), Some(999));
}
```

- [ ] **Step 2: Add preview helper**

Add helper near existing JSON helpers:

```rust
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
```

- [ ] **Step 3: Candidate DB summary must store previews only**

In `save_candidates_and_artifacts`, replace full DB summary arrays:

```rust
"equity_curve": sampled_preview(&output.equity_curve, 500),
"drawdown_curve": sampled_preview(&output.drawdown_curve, 500),
"trades_preview": sampled_preview(&output.trades_preview, 100),
```

Full candidate data should remain in the artifact written by `write_task_json_artifact`.

- [ ] **Step 4: Portfolio DB summary must store previews only**

Keep full `portfolio_full_rows` in artifact, but use preview rows in task summary:

```rust
let portfolio_full_rows = portfolio_top3.top3.iter().enumerate().map(|(rank, portfolio)| {
    portfolio_summary_json(rank, portfolio, portfolio_top3.eligible_candidate_count, false)
}).collect::<Vec<Value>>();
let portfolio_rows = portfolio_top3.top3.iter().enumerate().map(|(rank, portfolio)| {
    portfolio_summary_json(rank, portfolio, portfolio_top3.eligible_candidate_count, true)
}).collect::<Vec<Value>>();
```

`portfolio_summary_json(..., preview_only=true)` should cap:

- `equity_curve` to 500 points.
- `drawdown_curve` to 500 points.
- `trades_preview` to 100 rows.

`portfolio_summary_json(..., preview_only=false)` should keep full artifact content.

- [ ] **Step 5: Run tests**

```bash
cargo test -p backtest-worker sampled_preview_caps_large_series_and_keeps_edges -- --nocapture
cargo test -p backtest-worker -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-worker/src/main.rs
git commit -m "fix: 修复思路 限制马丁回测DB摘要体积"
```

---

## Task 4: Annualized Return and Long+Short Direction Summary

**Files:**
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add annualized return test**

Add or extend a kline-engine test that runs at least two timestamps and asserts:

```rust
assert!(result.metrics.annualized_return_pct.is_some());
```

The test should use an existing profitable fixture if possible. If fixture returns zero return but valid dates, it is still acceptable as long as annualized is `Some(0.0)` rather than `None`.

- [ ] **Step 2: Compute annualized in kline result**

In `run_kline_screening`, after final equity and total return are computed:

```rust
let backtest_days = match (equity_curve.first(), equity_curve.last()) {
    (Some(first), Some(last)) if last.timestamp_ms > first.timestamp_ms => {
        (last.timestamp_ms - first.timestamp_ms) as f64 / 86_400_000.0
    }
    _ => 0.0,
};
let annualized_return_pct = calculate_annualized_return_pct(
    budget_quote,
    final_equity_quote,
    backtest_days,
);
```

Then set:

```rust
annualized_return_pct,
```

instead of `None`.

- [ ] **Step 3: Fix candidate direction summary**

In `output_direction`, if `output.config.direction_mode` is `LongAndShort`, `long_and_short`, or `long_short`, return JSON string `long_short` before falling back to first strategy direction.

Example:

```rust
if let Some(direction_mode) = output.config.get("direction_mode").and_then(Value::as_str) {
    if direction_mode.eq_ignore_ascii_case("long_and_short")
        || direction_mode.eq_ignore_ascii_case("long_short")
    {
        return Value::String("long_short".to_owned());
    }
}
```

- [ ] **Step 4: Add worker unit assertion**

Add a test or extend `selected_outputs_include_ui_required_summary_fields` so a candidate config with `direction_mode=LongAndShort` produces summary direction `long_short`.

- [ ] **Step 5: Run tests**

```bash
cargo test -p backtest-engine -- --nocapture
cargo test -p backtest-worker -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-engine/src/martingale/kline_engine.rs apps/backtest-worker/src/main.rs
git commit -m "fix: 修复思路 补齐马丁年化收益与多空方向摘要"
```

---

## Task 5: Docker Smoke Verification

**Files:**
- No planned source changes unless smoke fails.

- [ ] **Step 1: Build and restart affected services**

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  build api-server backtest-worker

docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  up -d --no-deps api-server backtest-worker
```

- [ ] **Step 2: Create smoke task through real API**

Use a new user and payload with minimal high-level fields only:

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

Do not manually include `random_seed` or `random_candidates`; the test must prove defaults work.

- [ ] **Step 3: Verify smoke result**

The task must end `succeeded`, not `failed`.

Validate:

- `candidate_len > 0`.
- At least one candidate summary `direction == "long_short"` for a `long_short` task.
- Candidate `annualized_return_pct` is not null when equity curve has valid time span.
- Candidate `equity_curve.length > 0` and `drawdown_curve.length > 0`.
- `portfolio_top3.length > 0`.
- First portfolio `member_count >= 2`.
- Portfolio allocation sum equals 100.
- Portfolio `equity_curve.length > 0` and `drawdown_curve.length > 0`.
- No DB jsonb size error in worker logs.

- [ ] **Step 4: Report evidence**

Claude must report:

- Commit hashes.
- Exact test commands and pass/fail status.
- Docker smoke task id.
- Candidate count.
- Candidate direction values.
- Candidate annualized return presence.
- Portfolio Top3 member counts and allocation sums.
- Any remaining limitation, especially if trades are zero because the tiny smoke produced no fills.

