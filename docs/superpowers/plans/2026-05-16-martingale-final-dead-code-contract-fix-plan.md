# Martingale Final Dead-Code Contract Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this focused cleanup task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove leftover unused worker helpers and brittle grep contracts before merge/deploy, while preserving the now-wired profit-first staged search and real Portfolio Top 3 persistence.

**Architecture:** Do not change strategy behavior. Only remove dead code and update contract tests so they assert real execution-path behavior instead of unused symbol presence.

**Tech Stack:** Rust `backtest-worker`, Node verification tests.

---

## Verified State

Fresh checks after Claude's second completion:

- `cargo test -p backtest-engine -- --nocapture` passed.
- `cargo test -p backtest-worker -- --nocapture` passed, but emitted unused warnings:
  - unused imports: `MartingaleDirection`, `MartingaleRiskLimits`, `MartingaleStrategyConfig`
  - unused functions: `relax_drawdown_limit`, `reject_negative_return`, `search_space_for_symbol`, `apply_task_overrides`, `spawn_status_cancel_watcher`
- `cargo test -p api-server martingale_auto_search -- --nocapture` passed.
- Node verification tests passed.
- `pnpm --filter web exec next build --webpack` passed.
- Worktree is clean.

This is close, but not merge-ready because `tests/verification/backtest_worker_contract.test.mjs` still asserts `relax_drawdown_limit` / `reject_negative_return|positive_return`, while those functions are unused. That is a brittle grep contract, not a real behavior check.

---

## Task 1: Remove Dead Worker Helpers

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] **Step 1: Remove unused imports**

In `apps/backtest-worker/src/main.rs`, remove unused imports from the `shared_domain::martingale` import list:

```rust
MartingaleDirection
MartingaleRiskLimits
MartingaleStrategyConfig
```

- [ ] **Step 2: Remove unused worker helper functions**

Delete these unused functions from `apps/backtest-worker/src/main.rs`:

```rust
fn relax_drawdown_limit(...)
fn reject_negative_return(...)
fn search_space_for_symbol(...)
fn apply_task_overrides(...)
fn spawn_status_cancel_watcher(...)
```

Do not remove `apply_task_overrides_to_candidate`; it is still used by staged search.

- [ ] **Step 3: Update brittle worker contract assertions**

In `tests/verification/backtest_worker_contract.test.mjs`, replace:

```js
assert.match(worker, /relax_drawdown_limit/);
assert.match(worker, /reject_negative_return|positive_return/);
```

with assertions for the real path:

```js
assert.match(worker, /drawdown_limit_sequence\(&task\.config\.risk_profile\)/);
assert.match(worker, /score\.survival_valid/);
assert.match(worker, /total_return_pct\s*<=\s*0\.0/);
```

Keep the existing assertions that `process_task()` calls `run_profit_first_staged_search(&market_context, ...)` and that the old main `random_search` path is absent.

- [ ] **Step 4: Verify cleanup**

Run:

```bash
cargo test -p backtest-worker -- --nocapture 2>&1 | tee /tmp/backtest-worker-final.log
node tests/verification/backtest_worker_contract.test.mjs
```

Expected:

- Rust tests pass.
- Node worker contract passes.
- `/tmp/backtest-worker-final.log` does not contain these warnings:
  - `function relax_drawdown_limit is never used`
  - `function reject_negative_return is never used`
  - `function search_space_for_symbol is never used`
  - `function apply_task_overrides is never used`
  - `function spawn_status_cancel_watcher is never used`

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-worker/src/main.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "fix: 修复思路 清理马丁回测Worker死代码契约"
```

---

## Task 2: Final Merge Gate Verification

**Files:**
- No source change unless verification exposes a defect.

- [ ] **Step 1: Run full required verification**

```bash
cargo test -p backtest-engine -- --nocapture
cargo test -p backtest-worker -- --nocapture
cargo test -p api-server martingale_auto_search -- --nocapture
node tests/verification/martingale_backtest_rebuild_contract.test.mjs
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
pnpm --filter web exec next build --webpack
```

Expected: all pass.

- [ ] **Step 2: Check clean status**

```bash
git status --short --branch
```

Expected: clean working tree.

---

## Handoff Rule

After Claude completes this plan, ask Codex to re-check. Do not merge/deploy until Codex confirms the worker warning cleanup and full verification output.
