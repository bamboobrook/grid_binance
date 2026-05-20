# Martingale Search Portfolio Final Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make martingale backtest produce trustworthy per-symbol Top10 candidates and real cross-symbol portfolio Top3 with long_short dual-leg search.

**Architecture:** Fix the backend first: lock failing tests, preserve per-symbol candidate pools, enforce cross-symbol portfolio priority and single-symbol weight cap, then verify through Docker smoke. Keep UI unchanged unless API fields are missing from summaries.

**Tech Stack:** Rust workspace, Node verification tests, Docker Compose, existing `/backtest/tasks` APIs.

---

### Task 1: Lock regressions with tests

**Files:**
- Modify: `apps/backtest-engine/src/portfolio_search.rs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] Add portfolio tests proving Top1 must be cross-symbol when two eligible symbols exist.
- [ ] Add portfolio tests proving single-symbol allocation cap is enforced at 80%.
- [ ] Add worker contract tests proving per-symbol selection is not globally truncated.
- [ ] Run focused tests and confirm new tests fail before implementation.

### Task 2: Preserve per-symbol candidates

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] Inspect `select_top_outputs_per_symbol`, `display_outputs`, `portfolio_pool_outputs`, and `save_candidates_and_artifacts`.
- [ ] Ensure BTC high-score rows cannot globally squeeze out ETH rows.
- [ ] Persist/display TopN per symbol.
- [ ] Add summary diagnostics: searched symbols, display symbols, pool symbols, eligible symbols, dropped symbols.
- [ ] Run worker contract tests.

### Task 3: Fix portfolio construction

**Files:**
- Modify: `apps/backtest-engine/src/portfolio_search.rs`

- [ ] Generate cross-symbol portfolios before same-symbol portfolios.
- [ ] Rank diversified portfolios ahead of same-symbol portfolios when at least two eligible symbols exist.
- [ ] Enforce per-symbol allocation cap at 80%.
- [ ] Keep same-symbol multi-strategy only as lower-priority fallback.
- [ ] Ensure portfolio curves and drawdowns are computed from weighted member curves.
- [ ] Run portfolio unit tests.

### Task 4: Verify long_short output contract

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] Ensure `long_short` outputs include `long_short_legs.long` and `.short`.
- [ ] Ensure planned margin and leverage fields are copied into candidate summaries.
- [ ] Ensure annualized return, equity curve, drawdown curve, and trade preview survive persistence.
- [ ] Run Node contract test and focused worker tests.

### Task 5: Build, deploy, smoke, and push

**Files:**
- Modify/create only verification scripts if needed.

- [ ] Run focused Rust and Node tests.
- [ ] Build `api-server`, `backtest-worker`, `web` images.
- [ ] Recreate only `api-server`, `backtest-worker`, `web`.
- [ ] Run BTC+ETH `long_short` balanced smoke.
- [ ] Verify candidates include both symbols and Top1 portfolio is cross-symbol with per-symbol weight <= 80%.
- [ ] Commit changes in logical commits with required Chinese Git log content.
- [ ] Push to remote and report commit hashes plus smoke task id.
