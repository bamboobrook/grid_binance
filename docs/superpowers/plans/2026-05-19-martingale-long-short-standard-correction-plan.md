# Martingale Long/Short Standard Correction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Correct the latest Claude branch so `direction_mode=long_short` remains true dual-direction martingale search with the original risk standards, instead of drifting into single-direction candidates or relaxed drawdown thresholds.

**Architecture:** Revert the behavioral drift while preserving useful infrastructure: bounded search, progress/timeout, diagnostics. The optimizer must search better true `LongAndShort` parameter combinations, not “solve” long_short by inserting `LongOnly`/`ShortOnly` candidates or weakening the user's risk ceiling.

**Tech Stack:** Rust `backtest-worker`, `backtest-engine`, existing worker contracts, Docker smoke on host `8080`.

---

## Why This Plan Exists

User requirement is explicit:

- `long_short` means simultaneous dual-direction execution.
- Results must show true dual-direction candidates, not single-direction substitutes.
- Keep prior risk standards: balanced max drawdown target is 25%, conservative 20%, aggressive 30% unless the user explicitly changes it.
- Seek higher annualized return within risk constraints; do not lower standards to make weak results pass.

Latest Claude branch `feature/full-v1` at `cf9be19 fix: 修复思路 马丁多空搜索同时搜索单向候选` violates this direction.

## Verified Drift Evidence

In `/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/backtest-worker/src/main.rs`:

1. `run_long_short_staged_search()` now generates single-direction candidates for a `long_short` request:

```rust
let long_candidates = generate_staged_candidates_for_symbol(symbol, "long", &long_staged, 256)?;
let short_candidates = generate_staged_candidates_for_symbol(symbol, "short", &short_staged, 256)?;
all_candidates.extend(long_candidates);
all_candidates.extend(short_candidates);
all_candidates.extend(long_short_candidates);
```

This violates the standard: a `long_short` task must not persist or rank `LongOnly`/`ShortOnly` candidates as if they satisfy the request.

2. `long_short_drawdown_limit_sequence()` relaxes balanced drawdown to 40/50:

```rust
"balanced" => vec![40.0, 50.0]
```

This violates the risk standard. Balanced should target 25%, not 40%+.

3. Tests encode the wrong behavior:

```rust
fn long_short_drawdown_limits_are_relaxed()
```

This test should be removed/replaced with a risk-standard preservation test.

---

### Task 1: Lock true dual-direction candidate contract

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] **Step 1: Add Rust test forbidding single-direction candidates for long_short**

Add inside worker tests:

```rust
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
    let candidates = generate_long_short_candidates_for_task_for_tests("BTCUSDT", &task, &staged)
        .expect("candidates should generate");

    assert!(!candidates.is_empty());
    assert!(
        candidates.iter().all(|candidate| candidate.config.direction_mode == MartingaleDirectionMode::LongAndShort),
        "long_short request must only generate LongAndShort portfolio candidates"
    );
    assert!(
        candidates.iter().all(|candidate| {
            let has_long = candidate.config.strategies.iter().any(|s| s.direction == MartingaleDirection::Long);
            let has_short = candidate.config.strategies.iter().any(|s| s.direction == MartingaleDirection::Short);
            has_long && has_short
        }),
        "every long_short candidate must contain both long and short legs"
    );
}
```

If there is no helper, extract one from `run_long_short_staged_search()` that only generates/caps candidates without screening:

```rust
#[cfg(test)]
fn generate_long_short_candidates_for_task_for_tests(
    symbol: &str,
    task: &WorkerTaskConfig,
    staged: &StagedMartingaleSearchSpace,
) -> Result<Vec<SearchCandidate>, String> {
    generate_long_short_candidates_for_task(symbol, task, staged)
}
```

Production helper should be:

```rust
fn generate_long_short_candidates_for_task(
    symbol: &str,
    task: &WorkerTaskConfig,
    staged: &StagedMartingaleSearchSpace,
) -> Result<Vec<SearchCandidate>, String> {
    let effective_staged = apply_search_space_overrides_to_staged(staged, task);
    let cap = task.random_candidates.max(1) * task.intelligent_rounds.max(1);
    let candidates = backtest_engine::search::generate_staged_candidates_for_symbol(
        symbol,
        "long_short",
        &effective_staged,
        512,
    )?;
    Ok(interleave_candidates_by_spacing(candidates, cap))
}
```

- [ ] **Step 2: Add Node contract forbidding single-direction generation inside long_short path**

Append to `tests/verification/backtest_worker_contract.test.mjs`:

```js
test("long_short worker path does not substitute single-direction candidates", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  const fnMatch = worker.match(/fn run_long_short_staged_search[\s\S]*?\n}\n\nfn /);
  assert.ok(fnMatch, "run_long_short_staged_search should exist");
  const body = fnMatch[0];
  assert.doesNotMatch(body, /generate_staged_candidates_for_symbol\([^\)]*"long"/);
  assert.doesNotMatch(body, /generate_staged_candidates_for_symbol\([^\)]*"short"/);
  assert.doesNotMatch(body, /long_candidates/);
  assert.doesNotMatch(body, /short_candidates/);
});
```

- [ ] **Step 3: Remove single-direction generation**

In `run_long_short_staged_search()`, remove:

- `long_staged`
- `short_staged`
- `long_candidates`
- `short_candidates`
- `all_candidates.extend(long_candidates)`
- `all_candidates.extend(short_candidates)`

Keep only true `long_short` candidates.

- [ ] **Step 4: Verify focused tests**

Run:

```bash
cargo test -p backtest-worker long_short_search_does_not_generate_single_direction_substitutes -- --nocapture
cargo test -p backtest-worker long_short_task_produces_long_and_short_candidates_via_intelligent_search -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-worker/src/main.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "fix: 问题描述 禁止马丁多空回测退化为单向候选"
```

---

### Task 2: Restore risk profile drawdown standards

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Replace relaxed drawdown test**

Remove `long_short_drawdown_limits_are_relaxed()`.

Add:

```rust
#[test]
fn long_short_uses_configured_risk_profile_drawdown_limits() {
    assert_eq!(long_short_drawdown_limit_sequence("conservative")[0], 20.0);
    assert_eq!(long_short_drawdown_limit_sequence("balanced")[0], 25.0);
    assert_eq!(long_short_drawdown_limit_sequence("aggressive")[0], 30.0);
}
```

- [ ] **Step 2: Restore drawdown limit values**

Change `long_short_drawdown_limit_sequence()` to preserve the same first-pass risk standards:

```rust
fn long_short_drawdown_limit_sequence(risk_profile: &str) -> Vec<f64> {
    match risk_profile {
        "conservative" => vec![20.0, 25.0],
        "balanced" => vec![25.0, 30.0],
        "aggressive" => vec![30.0, 35.0],
        _ => vec![25.0, 30.0],
    }
}
```

The second value is only a controlled fallback; do not jump to 40/50/60.

- [ ] **Step 3: Verify**

Run:

```bash
cargo test -p backtest-worker long_short_uses_configured_risk_profile_drawdown_limits -- --nocapture
cargo test -p backtest-worker worker_applies_risk_profile_drawdown_and_wizard_overrides -- --nocapture
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/backtest-worker/src/main.rs
git commit -m "fix: 修复思路 恢复马丁多空风险回撤标准"
```

---

### Task 3: Search better true long_short parameters, not lower standards

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Keep lower-churn search, but only for true long_short candidates**

Keep useful lower-churn expansion, but ensure generated candidates remain `LongAndShort`:

- spacing can include wider values, e.g. `[120, 180, 240, 300, 420, 720]` if bounded by screening cap.
- multiplier can include lower values, e.g. `[1.10, 1.15, 1.20, 1.25]`.
- take profit can include `[60, 80, 100, 140, 200]`.
- max legs can include `[2, 3]`.
- weights can include provided `[[60,40],[50,50]]` plus optional conservative `[[70,30]]` only if still true dual-leg.

Do not generate `LongOnly` or `ShortOnly` candidates.

- [ ] **Step 2: Add test that screened candidates include true long_short lower-churn variety**

```rust
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
    let candidates = generate_long_short_candidates_for_task_for_tests("BTCUSDT", &task, &staged)
        .expect("candidates should generate");

    let spacings: std::collections::BTreeSet<u32> = candidates.iter()
        .filter_map(search_candidate_spacing_bps)
        .collect();
    assert!(spacings.iter().any(|value| *value >= 240), "should screen wider spacing candidates: {:?}", spacings);
    assert!(candidates.iter().all(|candidate| candidate.config.direction_mode == MartingaleDirectionMode::LongAndShort));
}
```

- [ ] **Step 3: Verify**

Run:

```bash
cargo test -p backtest-worker long_short_lower_churn_candidates_remain_dual_leg -- --nocapture
cargo test -p backtest-worker long_short_smoke_search_estimate_is_bounded -- --nocapture
cargo test -p backtest-worker long_short_smoke_search_uses_random_candidates_as_screening_cap -- --nocapture
cargo test -p backtest-worker -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/backtest-worker/src/main.rs
git commit -m "fix: 修复思路 搜索真实马丁多空低磨损参数"
```

---

### Task 4: Full verification and smoke acceptance

**Files:**
- No source changes unless tests fail.

- [ ] **Step 1: Full local verification**

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

- [ ] **Step 2: Docker deploy**

Build/restart only this project:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  build api-server backtest-worker web

docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  up -d --no-deps api-server backtest-worker web
```

- [ ] **Step 3: Exact smoke payload**

Run the BTC/ETH balanced `long_short` payload.

Acceptance if succeeded:

```sql
select count(*) as candidates,
       count(*) filter (where summary->>'direction' in ('long_short','LongShort','long+short')) as long_short_candidates,
       count(*) filter (where summary->>'direction' in ('long','short','LongOnly','ShortOnly')) as single_direction_candidates,
       count(*) filter (where (summary->>'annualized_return_pct')::numeric > 0) as positive_annualized_candidates,
       count(*) filter (where (summary->>'max_drawdown_pct')::numeric <= 30) as within_risk_candidates
from backtest_candidate_summaries
where task_id='<NEW_TASK_ID>';
```

Expected:

- `candidates >= 10`
- `long_short_candidates = candidates`
- `single_direction_candidates = 0`
- `positive_annualized_candidates >= 1`
- `within_risk_candidates = candidates` or any excluded candidate must not be selected for portfolio.

Portfolio acceptance:

```sql
select jsonb_array_length(summary->'portfolio_top3') as portfolio_count,
       jsonb_array_length((summary->'portfolio_top3'->0)->'members') as first_member_count,
       ((summary->'portfolio_top3'->0)->>'annualized_return_pct') as first_annualized,
       ((summary->'portfolio_top3'->0)->>'max_drawdown_pct') as first_drawdown
from backtest_tasks
where task_id='<NEW_TASK_ID>';
```

Expected:

- `portfolio_count >= 1`
- `first_member_count >= 2`
- `first_annualized > 0`
- `first_drawdown <= 30`

If failed, include `rejection_diagnostics`; failure is acceptable only if it preserves standards and explains why no true dual-direction positive candidates exist. Do not pass with single-direction substitutes.

---

## Do Not Do

- Do not generate `long` or `short` candidates inside a `long_short` request.
- Do not relax balanced drawdown to 40% or 50%.
- Do not optimize by returning single-direction results.
- Do not fabricate positive candidates.
- Do not remove timeout/progress diagnostics.
- Do not touch unrelated host port `3000`; this app is exposed through host `8080`.
