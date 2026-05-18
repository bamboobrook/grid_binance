# Martingale Claude Smoke Fix Follow-up Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to complete these cleanup tasks. Do not start new feature work.

**Goal:** Make Claude's smoke-fix branch mergeable by committing all intended changes, preserving repository docs, and adding the missing regression coverage required by the smoke-fix plan.

**Current Reviewer Findings:**

- Branch checked: `feature/full-v1` at `a3f66ed0694e16592e20c6a8c25d9c35aa2d53ba`.
- `feature/full-v1` has uncommitted changes:
  - `apps/api-server/src/services/martingale_publish_service.rs`
  - `apps/api-server/tests/martingale_backtest_flow.rs`
- `git diff --stat main..feature/full-v1` shows the branch would delete the docs added on `main`, including:
  - `docs/superpowers/specs/2026-05-18-martingale-backtest-result-completeness-fix-design.md`
  - `docs/superpowers/plans/2026-05-18-martingale-backtest-result-completeness-fix-plan.md`
  - `docs/superpowers/plans/2026-05-18-martingale-claude-result-completeness-review-fix-plan.md`
  - `docs/superpowers/plans/2026-05-18-martingale-post-merge-smoke-fix-plan.md`
- Verification command `cargo test -p backtest-worker sampled_preview_caps_large_series_and_keeps_edges -- --nocapture` ran **0 tests**, so the required DB-summary preview regression test is missing or named differently.
- The broader tests passed in the reviewer run, but the branch is not mergeable while it has uncommitted changes and missing required regression coverage.

---

## Task 1: Preserve Main Documentation Before Merge

**Files:**
- Restore/preserve docs under `docs/superpowers/specs/`
- Restore/preserve docs under `docs/superpowers/plans/`

- [ ] **Step 1: Rebase or merge current main into `feature/full-v1`**

Run from `/home/bumblebee/Project/grid_binance/.worktrees/full-v1`:

```bash
git fetch origin
git merge main
```

If conflicts occur, preserve the main-branch docs listed above. Do not delete Superpowers spec/plan files.

- [ ] **Step 2: Verify docs are not deleted**

Run:

```bash
git diff --name-status main..HEAD | grep 'docs/superpowers' || true
```

Expected: no `D` entries for the four 2026-05-18 Martingale spec/plan files.

---

## Task 2: Commit or Revert Uncommitted Changes Intentionally

**Files:**
- `apps/api-server/src/services/martingale_publish_service.rs`
- `apps/api-server/tests/martingale_backtest_flow.rs`

- [ ] **Step 1: Inspect current uncommitted diff**

Run:

```bash
git diff -- apps/api-server/src/services/martingale_publish_service.rs apps/api-server/tests/martingale_backtest_flow.rs
```

Current reviewer-observed diff changes paused portfolio conflict handling and updates test expected messages from `leverage conflict` to `symbol conflict`.

- [ ] **Step 2: Decide whether this diff belongs to the smoke fix**

If it is required for currently failing tests, commit it with a message containing `修复思路`:

```bash
git add apps/api-server/src/services/martingale_publish_service.rs apps/api-server/tests/martingale_backtest_flow.rs
git commit -m "fix: 修复思路 收口马丁组合冲突校验断言"
```

If it is unrelated to the smoke-fix scope, revert only those uncommitted changes:

```bash
git restore apps/api-server/src/services/martingale_publish_service.rs apps/api-server/tests/martingale_backtest_flow.rs
```

Do not leave the branch dirty.

---

## Task 3: Add Missing DB Summary Preview Regression Test

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add exact required test**

Add this test inside the existing `#[cfg(test)] mod tests` in `apps/backtest-worker/src/main.rs`:

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

If the production helper has a different name, either rename it to `sampled_preview` or add a thin wrapper with that name. The reviewer will run this exact test name.

- [ ] **Step 2: Run the exact test**

```bash
cargo test -p backtest-worker sampled_preview_caps_large_series_and_keeps_edges -- --nocapture
```

Expected: `running 1 test` and `test ... ok`. `running 0 tests` is not acceptable.

- [ ] **Step 3: Commit**

```bash
git add apps/backtest-worker/src/main.rs
git commit -m "test: 问题描述 锁定马丁回测摘要采样上限"
```

---

## Task 4: Run Required Verification

Run from `/home/bumblebee/Project/grid_binance/.worktrees/full-v1`:

```bash
cargo test -p api-server martingale_auto_search_normalizes_profit_first_contract -- --nocapture
cargo test -p backtest-worker worker_task_config_deserializes_missing_search_counts_with_defaults -- --nocapture
cargo test -p backtest-worker sampled_preview_caps_large_series_and_keeps_edges -- --nocapture
cargo test -p backtest-engine -- --nocapture
cargo test -p backtest-worker -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
pnpm --filter web exec next build --webpack
```

All commands must exit 0.

---

## Task 5: Final Handoff Requirements

Before handing back to reviewer, report:

- Latest `feature/full-v1` commit hash.
- `git status --short --branch` output showing clean worktree.
- Confirmation that no `docs/superpowers/...2026-05-18...` files are deleted relative to main.
- Exact output summary for the three focused tests:
  - `martingale_auto_search_normalizes_profit_first_contract`
  - `worker_task_config_deserializes_missing_search_counts_with_defaults`
  - `sampled_preview_caps_large_series_and_keeps_edges`
- Whether the uncommitted publish-service diff was committed or reverted, and why.

