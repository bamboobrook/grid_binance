# Martingale Claude Completion Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Claude's Task 5–12 implementation truly execute in the real worker path instead of only passing grep-style contracts.

**Architecture:** Keep the current API/task/worker flow. Wire the already-added profit-first staged search into `process_task()`, persist real portfolio Top 3 from real outputs, remove unused long/short dead code or make it honest, and clean the worktree before merge.

**Tech Stack:** Rust workspace, backtest-worker, backtest-engine, API server, Next.js contract tests.

---

## Verified Problems

Fresh checks after Claude completion showed:

- `cargo test -p backtest-engine -- --nocapture` passed.
- `cargo test -p backtest-worker -- --nocapture` passed, but emitted warnings that core new functions are unused:
  - `run_profit_first_staged_search` is never used.
  - `build_portfolio_top3` and `PortfolioArtifact` are unused imports.
  - `build_long_short_config`, `combine_leg_results`, `merge_equity_curves_by_timestamp`, `max_drawdown_from_curve` are unused.
- Node contracts passed, but they only verify symbol presence and did not catch unused implementation.
- `pnpm --filter web exec next build --webpack` passed.
- Worktree still has uncommitted changes in:
  - `apps/api-server/src/services/martingale_publish_service.rs`
  - `apps/backtest-engine/src/sqlite_market_data.rs`
  - `crates/shared-db/src/backtest.rs`

Do not merge or deploy until all tasks below are complete.

---

## Task 1: Wire Profit-First Staged Search Into Worker

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] **Step 1: Add failing contract assertions**

Add to `tests/verification/backtest_worker_contract.test.mjs`:

```js
assert.match(worker, /run_profit_first_staged_search\(\s*&market_context,/);
assert.doesNotMatch(worker, /let random_candidates = apply_task_overrides\(\s*random_search\(/);
```

Run:

```bash
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: FAIL before implementation because `process_task()` still uses the old random-search path.

- [ ] **Step 2: Replace old search block in `process_task()`**

In `apps/backtest-worker/src/main.rs`, replace the current `random_search` + `search_candidates_with_drawdown_relaxation` block with per-symbol calls to `run_profit_first_staged_search()`.

Use this shape:

```rust
let mut screened = Vec::new();
let mut evaluated_count = 0usize;
let drawdown_limits = drawdown_limit_sequence(&task.config.risk_profile);
let first_drawdown_limit = drawdown_limits.first().copied().unwrap_or(25.0);

for symbol in &task.config.symbols {
    respect_pause_or_cancel(poller, &task.task_id).await?;
    for drawdown_limit_pct in &drawdown_limits {
        let scoring = scoring_config_from_task(&task.config, *drawdown_limit_pct);
        let candidates = run_profit_first_staged_search(
            &market_context,
            symbol,
            &task.config,
            &scoring,
            *drawdown_limit_pct,
        )
        .await?;
        evaluated_count += candidates.len();
        let valid: Vec<EvaluatedCandidateWithDrawdown> = candidates
            .into_iter()
            .filter(|candidate| candidate.score.survival_valid)
            .map(|candidate| EvaluatedCandidateWithDrawdown {
                candidate,
                used_drawdown_limit_pct: *drawdown_limit_pct,
                risk_relaxed: *drawdown_limit_pct > first_drawdown_limit,
            })
            .collect();
        if !valid.is_empty() {
            screened.extend(valid);
            break;
        }
    }
}
```

Then save with:

```rust
poller.save_candidates_and_artifacts(&task.task_id, evaluated_count, &outputs).await?;
```

- [ ] **Step 3: Remove obsolete unused helpers**

After wiring the staged path, remove old unused helpers if they remain unused. `cargo test -p backtest-worker -- --nocapture` must not warn that `run_profit_first_staged_search` is unused.

- [ ] **Step 4: Verify and commit**

Run:

```bash
cargo test -p backtest-worker -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Commit:

```bash
git add apps/backtest-worker/src/main.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "fix: 修复思路 接入收益优先分阶段搜索执行路径"
```

---

## Task 2: Persist Real Portfolio Top 3

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `apps/backtest-engine/src/portfolio_search.rs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] **Step 1: Replace placeholder portfolio mapping**

Delete the current placeholder `to_portfolio_candidates()` that sets `max_drawdown_pct: 0.0`.

Add:

```rust
fn portfolio_candidates_from_outputs(outputs: &[CandidateOutput]) -> Vec<backtest_engine::portfolio_search::EvaluatedCandidate> {
    outputs
        .iter()
        .filter_map(|output| {
            let config = serde_json::from_value(output.config.clone()).ok()?;
            Some(backtest_engine::portfolio_search::EvaluatedCandidate {
                candidate: SearchCandidate {
                    candidate_id: output.candidate_id.clone(),
                    config,
                },
                score: output.score,
                return_pct: output.total_return_pct,
                max_drawdown_pct: output.max_drawdown_pct,
                survival_passed: output.total_return_pct > 0.0
                    && output.max_drawdown_pct <= output.used_drawdown_limit_pct,
            })
        })
        .collect()
}
```

- [ ] **Step 2: Build and persist portfolio Top 3 after output selection**

After `select_top_outputs_per_symbol(...)` in `process_task()`:

```rust
let portfolio_inputs = portfolio_candidates_from_outputs(&outputs);
let max_portfolio_drawdown_pct = drawdown_limit_sequence(&task.config.risk_profile)
    .first()
    .copied()
    .unwrap_or(25.0);
let portfolio_top3 = build_portfolio_top3(&portfolio_inputs, max_portfolio_drawdown_pct);
```

Write an artifact and task summary containing:

```json
{
  "portfolio_top_n": 3,
  "portfolio_top3": [...],
  "portfolio_top3_artifact_path": "..."
}
```

If `TaskPoller` has no summary helper, add one that calls `BacktestRepository::update_task_summary`.

- [ ] **Step 3: Strengthen contract test**

Add to `tests/verification/backtest_worker_contract.test.mjs`:

```js
assert.match(worker, /portfolio_candidates_from_outputs/);
assert.match(worker, /update_task_summary\(/);
assert.match(worker, /portfolio_top3_artifact_path/);
assert.doesNotMatch(worker, /max_drawdown_pct:\s*0\.0/);
```

- [ ] **Step 4: Verify and commit**

Run:

```bash
cargo test -p backtest-engine -- --nocapture
cargo test -p backtest-worker -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Commit:

```bash
git add apps/backtest-worker/src/main.rs apps/backtest-engine/src/portfolio_search.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "fix: 修复思路 持久化真实组合Top3结果"
```

---

## Task 3: Remove or Honestly Integrate Long/Short Dead Code

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] **Step 1: Remove unused manual combiner helpers**

Unless they are actually called from `process_task()`, remove these worker helpers:

```rust
build_long_short_config
combine_leg_results
merge_equity_curves_by_timestamp
max_drawdown_from_curve
```

The current engine can already simulate multiple strategies in one portfolio config; do not leave fake worker-side combiner code unused.

- [ ] **Step 2: Make contract honest**

In `tests/verification/backtest_worker_contract.test.mjs`, assert the real behavior instead of unused helper names:

```js
assert.match(worker, /directions_from_mode/);
assert.match(worker, /long_and_short/);
assert.match(worker, /Long, Short/);
```

- [ ] **Step 3: Verify and commit**

Run:

```bash
cargo test -p backtest-worker -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: no dead-code warnings for long/short helper functions.

Commit:

```bash
git add apps/backtest-worker/src/main.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "fix: 修复思路 移除未接入的多空双腿死代码"
```

---

## Task 4: Resolve Uncommitted Worktree Changes

**Files:**
- Inspect: `apps/api-server/src/services/martingale_publish_service.rs`
- Inspect: `apps/backtest-engine/src/sqlite_market_data.rs`
- Inspect: `crates/shared-db/src/backtest.rs`

- [ ] **Step 1: Inspect diff**

Run:

```bash
git diff -- apps/api-server/src/services/martingale_publish_service.rs apps/backtest-engine/src/sqlite_market_data.rs crates/shared-db/src/backtest.rs
```

- [ ] **Step 2: Revert formatting-only diffs**

If changes are only rustfmt/formatting, revert them:

```bash
git checkout -- apps/api-server/src/services/martingale_publish_service.rs apps/backtest-engine/src/sqlite_market_data.rs crates/shared-db/src/backtest.rs
```

- [ ] **Step 3: Commit only if behavior is required**

If any behavior change is required for the feature, commit separately:

```bash
git add apps/api-server/src/services/martingale_publish_service.rs apps/backtest-engine/src/sqlite_market_data.rs crates/shared-db/src/backtest.rs
git commit -m "fix: 修复思路 收口马丁回测遗留改动"
```

- [ ] **Step 4: Verify clean tree**

```bash
git status --short --branch
```

Expected: no uncommitted files.

---

## Task 5: Final Verification

**Files:**
- No source change unless verification exposes a defect.

- [ ] **Step 1: Run Rust tests**

```bash
cargo test -p backtest-engine -- --nocapture
cargo test -p backtest-worker -- --nocapture
cargo test -p api-server martingale_auto_search -- --nocapture
```

Expected: all pass; `backtest-worker` must not warn that staged search, portfolio Top 3, or long/short helper functions are unused.

- [ ] **Step 2: Run Node contracts**

```bash
node tests/verification/martingale_backtest_rebuild_contract.test.mjs
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
```

Expected: all pass.

- [ ] **Step 3: Run frontend build**

```bash
pnpm --filter web exec next build --webpack
```

Expected: exit code 0.

- [ ] **Step 4: Check status**

```bash
git status --short --branch
git log --oneline --decorate -12
```

Expected: clean working tree and fix commits after `cb1481f`.

---

## Handoff Rule

Do not merge, deploy, restart services, or run live backtest validation until this fix plan is completed and independently reviewed.
