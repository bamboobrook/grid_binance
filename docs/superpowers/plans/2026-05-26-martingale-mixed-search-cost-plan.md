# Martingale Mixed Search And Cost Accuracy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make martingale backtests use 0.045% fee, portfolio Top3 only, add mixed search across long/short/long+short, then run four validation tasks for flyingkid.

**Architecture:** Keep the kline engine as the single execution source of truth. Add mixed-mode orchestration in `backtest-worker` so each symbol contributes candidates from all three direction families into one portfolio pool. Keep frontend/API display focused on Top3 portfolios.

**Tech Stack:** Rust workspace (`backtest-engine`, `backtest-worker`), Next.js frontend contract checks, Postgres-backed task queue, Docker Compose deployment.

---

### Task 1: Cost Model Contract

**Files:**
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Test: `apps/backtest-engine/src/martingale/kline_engine.rs`

- [ ] Add/adjust unit test proving fee is 0.045% of notional and slippage is charged separately on entry and exit.
- [ ] Change default fee bps to `4.5` while preserving default slippage bps `2.0`.
- [ ] Run `cargo test -p backtest-engine martingale::kline_engine`.

### Task 2: Portfolio Top3 Only Contract

**Files:**
- Modify: `apps/web/components/backtest/backtest-console.tsx`
- Modify: `apps/backtest-worker/src/main.rs`
- Test: `tests/verification/backtest_console_contract.test.mjs`

- [ ] Update frontend result selection to prefer `portfolio_top3` and not display Top10 by default.
- [ ] Ensure worker summary keeps `portfolio_top3`; `portfolio_top10` may remain as artifact for compatibility but UI must not use it.
- [ ] Run `node --test tests/verification/backtest_console_contract.test.mjs`.

### Task 3: Mixed Search Mode

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `apps/backtest-engine/src/search.rs` if needed for direction aliases only
- Test: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] Add `mixed_best` task mode: for each symbol, run long, short, and long_short searches; merge positive/survival candidates into the shared portfolio pool.
- [ ] Keep long_short task behavior unchanged for existing validation.
- [ ] Ensure mixed candidates preserve real direction mode and leverage for publish mapping.
- [ ] Run `node --test tests/verification/backtest_worker_contract.test.mjs` and `cargo check -p backtest-worker`.

### Task 4: Deploy And Validation Tasks

**Files:**
- No source file changes expected after Task 3.

- [ ] Build affected services.
- [ ] Restart `api-server`, `web`, and `backtest-worker` only.
- [ ] Delete only previous flyingkid validation-livefix tasks for this rerun.
- [ ] Create four tasks: 7 long_short, 7 mixed_best, 18 long_short, 18 mixed_best.
- [ ] Monitor until all succeed and verify candidate counts plus `portfolio_top3` exist.
