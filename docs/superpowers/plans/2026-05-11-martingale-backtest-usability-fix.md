# Martingale Backtest Usability Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make martingale backtesting usable end-to-end: 20-symbol whitelist, automatic recent walk-forward windows, parameter presets/search spaces, compatible readonly market data, visible progress, and readable real results.

**Architecture:** Keep the existing API/task/worker pipeline. Add compatibility inside the market-data adapter, enrich worker task events/summaries, and make the web console derive UI from real task/candidate data instead of placeholders.

**Tech Stack:** Rust (`backtest-engine`, `backtest-worker`, `shared-db`, `api-server`), Next.js/React web components, Docker Compose, PostgreSQL task storage, readonly SQLite market data.

---

### Task 1: Raise symbol quota to 20

**Files:**
- Modify: `apps/api-server/src/services/backtest_service.rs`
- Modify: `apps/web/components/backtest/backtest-wizard.tsx`
- Modify: `tests/verification/backtest_console_contract.test.mjs`

- [ ] Add tests asserting default quota and UI copy allow 20 symbols.
- [ ] Change default max symbol quota from 5 to 20.
- [ ] Add frontend validation message for 20 symbols.
- [ ] Run API and web contract tests.

### Task 2: Add automatic recent walk-forward windows

**Files:**
- Modify: `apps/web/components/backtest/backtest-wizard.tsx`
- Modify: `apps/web/components/backtest/time-split-editor.tsx`
- Modify: `tests/verification/backtest_console_contract.test.mjs`

- [ ] Add tests asserting time mode is automatic by default and date pickers are advanced override only.
- [ ] Derive default end date from current date and use recent 365-day split.
- [ ] Include auto time metadata in payload.
- [ ] Keep manual date inputs behind an override select.

### Task 3: Add parameter presets/search-space UX

**Files:**
- Modify: `apps/web/components/backtest/backtest-wizard.tsx`
- Modify: `apps/web/components/backtest/martingale-parameter-editor.tsx`
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `apps/backtest-engine/src/search.rs`
- Modify: tests in `apps/backtest-engine/tests/search_scoring_time_splits.rs`, `apps/backtest-worker/src/main.rs`

- [ ] Add tests for conservative/balanced/aggressive presets and search arrays.
- [ ] Add preset selector and parameter ranges in UI.
- [ ] Serialize search space arrays to task config.
- [ ] Make worker use arrays for spacing, first order, multiplier, TP, leverage, max legs.

### Task 4: Support discord_c2im SQLite schema

**Files:**
- Modify: `apps/backtest-engine/src/sqlite_market_data.rs`
- Modify: tests in `apps/backtest-engine/src/sqlite_market_data.rs`
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] Add tests for legacy schema: `klines(symbol, market_type, timeframe, open_time, ...)`.
- [ ] Detect canonical vs discord_c2im schema.
- [ ] Load symbols from `market_universe` or `distinct klines.symbol`.
- [ ] Load klines using `timeframe/open_time` mapping.
- [ ] Treat missing agg_trades as candle-only refinement instead of failing.

### Task 5: Improve progress and results display

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `apps/web/components/backtest/backtest-console.tsx`
- Modify: `apps/web/components/backtest/backtest-task-list.tsx`
- Modify: `apps/web/components/backtest/backtest-result-table.tsx`
- Modify: `tests/verification/backtest_console_contract.test.mjs`

- [ ] Add tests for Chinese status/stage/error/result labels.
- [ ] Update worker task summary/events at every stage.
- [ ] Normalize failed/running/succeeded task progress from task summary.
- [ ] Show readable candidate metrics and parameter values.
- [ ] Remove remaining placeholder wording.

### Task 6: Verify with a real small backtest

**Files:**
- No production file unless defects found.

- [ ] Run unit/contract tests.
- [ ] Rebuild/restart only `api-server`, `web`, `backtest-worker`.
- [ ] Create a small backtest task using real DB and <=2 symbols.
- [ ] Poll until succeeded/failed.
- [ ] Verify at least one candidate is visible with metrics, or fix root cause.
