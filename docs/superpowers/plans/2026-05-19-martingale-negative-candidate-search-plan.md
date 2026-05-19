# Martingale Negative Candidate Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the BTC/ETH balanced `long_short` martingale smoke so it finds positive-return candidates and portfolio results instead of quickly failing with all screened candidates negative.

**Architecture:** Keep the bounded-search and timeout protections. Use the new rejection diagnostics to tune the default/auto `long_short` search breadth intelligently: when all candidates are negative, expand toward less fee-heavy/wider-spacing/lower-churn combinations while keeping screenings bounded and respecting user risk limits.

**Tech Stack:** Rust `backtest-worker`, `backtest-engine`, PostgreSQL smoke diagnostics, Docker deployment on host `8080`.

---

## Verified Current State

After merging and deploying `cad62f8 merge: 修复思路 合并马丁多空搜索耗时修复`:

- Local verification passed:
  - `cargo test -p backtest-worker long_short_smoke_search_estimate_is_bounded -- --nocapture`
  - `cargo test -p backtest-worker long_short_smoke_search_uses_random_candidates_as_screening_cap -- --nocapture`
  - `cargo test -p backtest-worker martingale_search_timeout_error_is_actionable -- --nocapture`
  - `cargo test -p backtest-worker -- --nocapture`
  - `node tests/verification/backtest_worker_contract.test.mjs`
  - `cargo test -p backtest-engine -- --nocapture`
  - `cargo test -p api-server martingale -- --nocapture`
  - `node tests/verification/backtest_console_contract.test.mjs`
  - `pnpm --filter web exec next build --webpack`
- Docker build/restart succeeded.
- Health checks passed.

## Real Smoke Failure

Exact smoke task:

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

Task id: `bt_1779174267838364515`

Result:

```text
status = failed
runtime ≈ 21s
error_message = no martingale candidates selected: direction_mode=long_short symbols=BTCUSDT,ETHUSDT screened_count=8 selected_count=0 risk_profile=balanced negative_return=8 drawdown_rejected=0 zero_trade=0 survival_valid=0
```

Summary diagnostics:

```json
{
  "stage": "failed",
  "current_symbol": "ETHUSDT",
  "estimated_screenings": 2,
  "rejection_diagnostics": {
    "total": 8,
    "negative_return_count": 8,
    "drawdown_rejected_count": 0,
    "zero_trade_count": 0,
    "survival_valid_count": 0,
    "best_by_return": [
      {
        "symbol": "BTCUSDT",
        "candidate_id": "staged-cand-1",
        "direction_mode": "long_short",
        "trade_count": 3466782,
        "total_return_pct": "negative"
      }
    ]
  }
}
```

Important interpretation:

- Timeout fixed: task terminates quickly.
- Diagnostics work: we can see all screened candidates are negative.
- Still fails product requirement: default balanced BTC/ETH `long_short` smoke produces no candidates/portfolio.
- `trade_count` is extremely high, suggesting churn/fee drag from too-tight spacing or too-frequent long+short cycling.

---

### Task 1: Add tests for negative-return search-space expansion

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add test for negative-candidate branch expansion**

Add inside worker tests:

```rust
#[test]
fn long_short_negative_return_search_includes_lower_churn_neighbors() {
    let config = WorkerTaskConfig {
        symbols: vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()],
        direction_mode: Some("long_short".to_owned()),
        risk_profile: "balanced".to_owned(),
        random_candidates: 24,
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

    let staged = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");
    let space = search_space_from_staged(&staged, "BTCUSDT", &config);

    assert!(space.step_bps.contains(&120));
    assert!(space.step_bps.contains(&180));
    assert!(space.step_bps.contains(&240));
    assert!(space.step_bps.contains(&300));
    assert!(space.multiplier.iter().any(|value| value.to_string() == "1.10" || value.to_string() == "1.1"));
    assert!(space.multiplier.iter().any(|value| value.to_string() == "1.20" || value.to_string() == "1.2"));
    assert!(space.multiplier.iter().any(|value| value.to_string() == "1.25"));
    assert!(space.take_profit_bps.contains(&60));
    assert!(space.take_profit_bps.contains(&80));
    assert!(space.take_profit_bps.contains(&100));
}
```

- [ ] **Step 2: Add test that bounded estimate still holds**

Update or add:

```rust
#[test]
fn long_short_negative_return_expansion_stays_bounded() {
    let config = WorkerTaskConfig {
        symbols: vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()],
        direction_mode: Some("long_short".to_owned()),
        risk_profile: "balanced".to_owned(),
        random_candidates: 24,
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
    assert!(estimate.generated_candidates_per_symbol <= 128, "estimate: {:?}", estimate);
    assert!(estimate.max_screenings_per_symbol <= 24, "estimate: {:?}", estimate);
}
```

- [ ] **Step 3: Run tests and confirm failure before implementation**

Run:

```bash
cargo test -p backtest-worker long_short_negative_return_search_includes_lower_churn_neighbors -- --nocapture
cargo test -p backtest-worker long_short_negative_return_expansion_stays_bounded -- --nocapture
```

Expected before implementation: first test likely FAILS because current smoke only screened 8 candidates and did not include enough lower-churn neighbors.

---

### Task 2: Tune bounded lower-churn long_short search

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify if needed: `apps/backtest-engine/src/search.rs`

- [ ] **Step 1: Expand lower-churn dimensions without exploding screenings**

For `direction_mode=long_short` and auto/profit-first staged search, when user provides a single tight value, include lower-churn neighbors:

- `spacing_bps`: `[120, 180, 240, 300]`
- `order_multiplier`: `[1.10, 1.20, 1.25]`
- `max_legs`: `[2, 3]` if current is `3`
- `take_profit_bps`: `[60, 80, 100]`
- `tail_stop_bps`: keep `[2000]` unless diagnostics show drawdown rejection, because current failure is negative return not drawdown.
- `long_short_weight_pct`: keep `[[60,40],[50,50]]` for bounded smoke.

Then cap K-line screenings using `random_candidates`. For this smoke, set normalized `random_candidates` to at least `24` for `long_short` if not explicitly set higher by the request/config normalization. Do not let generated combinations all run; choose deterministic top/broad samples up to the cap.

- [ ] **Step 2: Prefer lower-churn candidates in deterministic sample order**

If only the first generated candidates are screened, ensure ordering does not always screen tightest/highest-churn candidates first.

Required ordering for `long_short` smoke sample should include at least one candidate from each spacing bucket before duplicating similar tight configs:

```text
spacing 120, 180, 240, 300
```

Add a pure helper if needed:

```rust
fn interleave_candidates_by_spacing(candidates: Vec<SearchCandidate>, limit: usize) -> Vec<SearchCandidate>
```

Test:

```rust
#[test]
fn long_short_candidate_sampling_interleaves_spacing_buckets() {
    let candidates = generated_long_short_candidates_for_test_with_spacings(vec![120, 180, 240, 300], 2);
    let sampled = interleave_candidates_by_spacing(candidates, 4);
    let spacings: Vec<u32> = sampled.iter().filter_map(search_candidate_spacing_bps).collect();
    assert_eq!(spacings, vec![120, 180, 240, 300]);
}
```

If adding `generated_long_short_candidates_for_test_with_spacings` is too much, build minimal `SearchCandidate` values using existing test helpers. Keep the assertion exact.

- [ ] **Step 3: Verify focused tests**

Run:

```bash
cargo test -p backtest-worker long_short_negative_return_search_includes_lower_churn_neighbors -- --nocapture
cargo test -p backtest-worker long_short_negative_return_expansion_stays_bounded -- --nocapture
cargo test -p backtest-worker long_short_candidate_sampling_interleaves_spacing_buckets -- --nocapture
cargo test -p backtest-worker -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: all PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/backtest-worker/src/main.rs apps/backtest-engine/src/search.rs
git commit -m "fix: 修复思路 扩展马丁低磨损搜索候选"
```

---

### Task 3: Docker smoke acceptance must produce candidates

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

- [ ] **Step 3: Run exact smoke payload**

Use the exact BTC/ETH balanced `long_short` payload in this plan.

Expected:

- Task reaches terminal state within 180 seconds.
- Preferred: `succeeded` with non-empty candidates and portfolio results.
- Not acceptable: `failed` with `negative_return_count = total` again.

- [ ] **Step 4: SQL acceptance for success**

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

If still failed, include full `rejection_diagnostics` and do not claim completion.

---

## Reviewer Handoff Checklist

Claude must report:

- Search candidate estimate after tuning.
- Which spacings/multipliers were actually screened in smoke, if available.
- Final smoke task id and runtime.
- SQL acceptance output.

## Do Not Do

- Do not include negative-return fallback candidates.
- Do not remove timeout/progress safeguards.
- Do not fabricate candidates or metrics.
- Do not weaken max-drawdown constraints to make bad candidates pass.
- Do not touch unrelated host port `3000`; this app is exposed through host `8080`.
