# Martingale Claude Compile Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix Claude branch `feature/full-v1` so the latest martingale zero-valid-candidate diagnostics work compiles and can be re-verified.

**Architecture:** Do not change product behavior first. Repair the broken function signature/call-site mismatch and test helper warning, then rerun the previously requested focused tests before any broader tuning.

**Tech Stack:** Rust `backtest-worker`, Node verification contract.

---

## Verified Failure

On branch/worktree `/home/bumblebee/Project/grid_binance/.worktrees/full-v1` at commit `747722a fix: 修复思路 扩展马丁多空搜索避免空候选`, the first focused test fails to compile:

```bash
cargo test -p backtest-worker zero_selection_error_includes_candidate_rejection_diagnostics -- --nocapture
```

Actual error:

```text
error[E0061]: this function takes 3 arguments but 4 arguments were supplied
   --> apps/backtest-worker/src/main.rs:600:25
    |
600 |             let valid = select_candidates_or_best_fallback_for_task(
    |                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
...
604 |                 &rejection_samples,
    |                 ------------------ unexpected argument #4 of type `&Vec<CandidateRejectionSample>`

note: function defined here
   --> apps/backtest-worker/src/main.rs:954:4
    |
954 | fn select_candidates_or_best_fallback_for_task(
```

There is also a warning that should be cleaned while touching the test:

```text
warning: unused variable: `config`
    --> apps/backtest-worker/src/main.rs:2818:13
```

## Current Broken Code Shape

Call site currently passes 4 args:

```rust
let valid = select_candidates_or_best_fallback_for_task(
    candidates,
    *drawdown_limit_pct,
    risk_relaxed,
    &rejection_samples,
);
```

Function currently accepts 3 args:

```rust
fn select_candidates_or_best_fallback_for_task(
    candidates: Vec<EvaluatedCandidate>,
    drawdown_limit_pct: f64,
    risk_relaxed: bool,
) -> Vec<EvaluatedCandidateWithDrawdown> {
```

## Required Decision

Choose one consistent interface. Recommended: remove the unused fourth call argument unless diagnostics are actually used inside the helper.

Do **not** add unused parameters just to satisfy the call. If `rejection_samples` is not needed for fallback selection, keep diagnostics collection separate via `all_rejection_samples`.

---

### Task 1: Fix compile error without changing behavior

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Remove the extra argument at the call site**

Change:

```rust
let valid = select_candidates_or_best_fallback_for_task(
    candidates,
    *drawdown_limit_pct,
    risk_relaxed,
    &rejection_samples,
);
```

to:

```rust
let valid = select_candidates_or_best_fallback_for_task(
    candidates,
    *drawdown_limit_pct,
    risk_relaxed,
);
```

- [ ] **Step 2: Remove or use the unused test variable**

At `apps/backtest-worker/src/main.rs` around the test `selection_keeps_best_positive_candidates_when_survival_filter_is_empty`, remove the unused `let config = WorkerTaskConfig { ... };` block if it is not used by assertions.

If the test should assert `per_symbol_top_n`, then update the helper signature intentionally and use `&config`; otherwise remove the variable entirely.

- [ ] **Step 3: Run the previously failing focused test**

Run:

```bash
cargo test -p backtest-worker zero_selection_error_includes_candidate_rejection_diagnostics -- --nocapture
```

Expected:

- Compile succeeds.
- Exactly 1 focused test runs and passes.
- No new compile errors.

- [ ] **Step 4: Run the required martingale worker focused tests**

Run:

```bash
cargo test -p backtest-worker zero_selected_candidates_is_not_reported_as_success -- --nocapture
cargo test -p backtest-worker selection_keeps_best_positive_candidates_when_survival_filter_is_empty -- --nocapture
cargo test -p backtest-worker long_short_task_produces_long_and_short_candidates_via_intelligent_search -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected:

- Every `cargo test` command runs at least 1 test and passes.
- Node contract passes.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-worker/src/main.rs
git commit -m "fix: 问题描述 修复马丁诊断分支编译错误"
```

---

### Task 2: Re-run the full verification gate

**Files:**
- No code changes unless Task 2 reveals another compile/test failure.

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

Expected:

- All commands exit `0`.
- If warnings remain, list them in the handoff; do not hide them.

- [ ] **Step 2: Report verification evidence**

Claude must report:

- New commit hash.
- Output summary for each focused command in Task 1.
- Output summary for each full gate command in Task 2.

---

## Do Not Do

- Do not merge to `main`; reviewer will merge after independent verification.
- Do not deploy Docker services; reviewer will deploy after merge.
- Do not delete existing plan documents from `main` intentionally.
- Do not tune search/scoring further until the branch compiles and focused tests pass.
