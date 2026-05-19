# Martingale Diagnostics Contract Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix Claude branch `feature/full-v1` so the martingale diagnostics changes satisfy the existing worker contract and can proceed to full verification.

**Architecture:** Preserve the new rejection diagnostics feature, but remove hard-coded fake zero drawdown/return values from production source because the contract explicitly forbids `max_drawdown_pct: 0.0` placeholders in worker output paths. Use explicit `Option<f64>` diagnostics or a named failed-screening sample constructor instead of numeric placeholders that look like real metrics.

**Tech Stack:** Rust `backtest-worker`, Node verification contract.

---

## Verified Failure

On branch/worktree `/home/bumblebee/Project/grid_binance/.worktrees/full-v1` at commit `1e1ba3d fix: 问题描述 修复马丁诊断分支编译错误`, Rust focused tests pass, but the Node worker contract fails:

```bash
node tests/verification/backtest_worker_contract.test.mjs
```

Actual failing test:

```text
✖ backtest worker persists real portfolio Top 3 from outputs
AssertionError [ERR_ASSERTION]: The input was expected to not match the regular expression /max_drawdown_pct:\s*0\.0/
```

Offending production source:

```rust
Err(_) => {
    let sample = CandidateRejectionSample {
        candidate_id: overridden.candidate_id.clone(),
        symbol: symbol.to_owned(),
        direction_mode: direction_mode.to_owned(),
        total_return_pct: 0.0,
        max_drawdown_pct: 0.0,
        trade_count: 0,
        survival_valid: false,
    };
    (backtest_engine::martingale::scoring::CandidateScore {
        survival_valid: false,
        rank_score: 0.0,
        raw_score: 0.0,
        rejection_reasons: vec!["screening_failed".to_owned()],
    }, sample)
}
```

The contract exists because prior placeholder metrics caused misleading portfolio/backtest outputs. Diagnostics must not reintroduce placeholder-looking `max_drawdown_pct: 0.0` in production source.

---

### Task 1: Remove hard-coded metric placeholders from diagnostics

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Change diagnostic metric fields to optional values**

Change `CandidateRejectionSample` from:

```rust
struct CandidateRejectionSample {
    candidate_id: String,
    symbol: String,
    direction_mode: String,
    total_return_pct: f64,
    max_drawdown_pct: f64,
    trade_count: usize,
    survival_valid: bool,
}
```

to:

```rust
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
```

- [ ] **Step 2: Populate successful screening samples with `Some(...)`**

In the `Ok(ref metrics)` branch, set:

```rust
total_return_pct: Some(metrics.metrics.total_return_pct),
max_drawdown_pct: Some(metrics.metrics.max_drawdown_pct),
trade_count: metrics.metrics.trade_count as usize,
survival_valid: s.survival_valid,
rejection_reason: None,
```

- [ ] **Step 3: Populate failed screening samples without fake zero metrics**

In the `Err(_)` branch, set:

```rust
total_return_pct: None,
max_drawdown_pct: None,
trade_count: 0,
survival_valid: false,
rejection_reason: Some("screening_failed".to_owned()),
```

Do not use `max_drawdown_pct: 0.0` anywhere in production source.

- [ ] **Step 4: Update diagnostics aggregation for options**

Update `CandidateRejectionDiagnostics::from_samples()`:

```rust
let negative_return_count = samples
    .iter()
    .filter(|s| s.total_return_pct.map(|value| value <= 0.0).unwrap_or(false))
    .count();
let drawdown_rejected_count = samples
    .iter()
    .filter(|s| s.total_return_pct.map(|value| value > 0.0).unwrap_or(false) && !s.survival_valid)
    .count();
let zero_trade_count = samples.iter().filter(|s| s.trade_count == 0).count();
```

Sort `best_by_return` by treating `None` as lowest:

```rust
best_by_return.sort_by(|a, b| {
    b.total_return_pct
        .unwrap_or(f64::NEG_INFINITY)
        .total_cmp(&a.total_return_pct.unwrap_or(f64::NEG_INFINITY))
});
```

Sort `lowest_drawdown` by putting `None` last:

```rust
lowest_drawdown.sort_by(|a, b| {
    a.max_drawdown_pct
        .unwrap_or(f64::INFINITY)
        .total_cmp(&b.max_drawdown_pct.unwrap_or(f64::INFINITY))
});
```

- [ ] **Step 5: Update test helpers**

Update `candidate_rejection_sample_for_tests(...)` so numeric inputs are wrapped with `Some(...)` and `rejection_reason: None`.

Add one focused test for failed screening samples:

```rust
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
    assert_eq!(diagnostics.best_by_return[0].rejection_reason.as_deref(), Some("screening_failed"));
    assert!(diagnostics.best_by_return[0].max_drawdown_pct.is_none());
}
```

- [ ] **Step 6: Verify focused tests and contract**

Run:

```bash
cargo test -p backtest-worker zero_selection_error_includes_candidate_rejection_diagnostics -- --nocapture
cargo test -p backtest-worker screening_failed_rejection_sample_does_not_fake_zero_drawdown -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected:

- Both Rust tests run at least 1 test and pass.
- Node contract passes.

- [ ] **Step 7: Commit**

```bash
git add apps/backtest-worker/src/main.rs
git commit -m "fix: 问题描述 移除马丁诊断占位回撤值"
```

---

### Task 2: Re-run full verification gate

**Files:**
- No code changes unless tests fail.

- [ ] **Step 1: Run all required commands**

Run:

```bash
cargo test -p backtest-engine -- --nocapture
cargo test -p backtest-worker -- --nocapture
cargo test -p api-server martingale -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
pnpm --filter web exec next build --webpack
```

Expected: all commands exit `0`.

- [ ] **Step 2: Report evidence**

Claude must report:

- Commit hash.
- Focused test output summary.
- Full verification output summary.

---

## Do Not Do

- Do not weaken or delete `assert.doesNotMatch(worker, /max_drawdown_pct:\s*0\.0/)` from `tests/verification/backtest_worker_contract.test.mjs`.
- Do not fabricate numeric metrics for screening failures.
- Do not merge to `main`; reviewer will merge after independent verification.
- Do not deploy Docker services; reviewer will deploy after merge.
