# GLM Martingale Worktree Audit

Date: 2026-06-25
Remote repo: `/home/bumblebee/Project/grid_binance`
Purpose: classify the current dirty worktree after GLM's partial margin-capital parity work, and record the current runtime state before handing the next steps back to GLM.

## Safety State Observed

Read-only checks performed from the remote repo:

- `trading-engine` is not running.
- `api-server` is running and healthy.
- `backtest-worker` is running.
- Binance USDT-M probe showed:
  - `openOrderCount=0`
  - `nonzeroPositionCount=0`
  - `multiAssetsMargin=false`
  - `dualSidePosition=true`
- No formal live Martingale trading should be started from the current state.

Important: smoke executor strategies currently have local DB `Working` orders, even though Binance has no open orders. Starting `trading-engine` without resetting the smoke state may immediately retry those orders.

## Dirty Worktree Summary

`git diff --stat` reports 69 modified tracked files:

- 8779 insertions.
- 3096 deletions.
- Plus many untracked files.

The worktree is mixed. Do not make a single blind commit unless the unrelated UI/billing work is intentionally included. Split into categories first.

## Category A: GLM Margin-Capital Parity Fixes

These files are part of the current Martingale capital model repair and should be reviewed together:

- `apps/backtest-engine/src/martingale/capital.rs` (new)
- `apps/backtest-engine/src/martingale/mod.rs`
- `apps/backtest-engine/src/martingale/kline_engine.rs`
- `apps/backtest-engine/src/martingale/metrics.rs`
- `apps/backtest-engine/src/portfolio_search.rs`
- `apps/backtest-worker/src/main.rs`
- `apps/trading-engine/src/martingale_runtime.rs`
- `apps/trading-engine/src/main.rs`
- `apps/api-server/src/services/martingale_publish_service.rs`
- `crates/shared-domain/src/martingale.rs`
- `crates/shared-db/src/backtest.rs`
- `apps/web/lib/api-types.ts`
- `apps/web/components/backtest/backtest-console.tsx`
- `apps/web/components/backtest/backtest-result-table.tsx`
- `apps/web/components/backtest/live-portfolio-controls.tsx`
- `apps/web/components/backtest/exchange-preconfigure-panel.tsx`

Claimed by GLM and partly verified by status/logs:

- Added a capital model where futures `first_order_quote` is treated as order notional, and margin is `notional / leverage`.
- Added the hard test case: `first_order_quote=10`, `multiplier=2`, `max_legs=4`, `leverage=2` should produce planned notional `150` and planned margin `75`.
- Updated backtest metrics to use margin capital for principal/return/drawdown.
- Updated live runtime order generation to use notional for order quantity and margin for budget checks.
- Added publish/start preflight and long/short weight split.

Needs GLM to preserve:

- The new tests and all previous green test results.
- The exact capital semantics in docs and code comments, so this bug does not recur.

## Category B: Prior Martingale / Live / Data Infrastructure Changes

These appear related to the larger Martingale live parity project, but not all are necessarily GLM's latest margin fix:

- `apps/api-server/src/lib.rs`
- `apps/api-server/src/routes/backtest.rs`
- `apps/api-server/src/routes/backtest_route.rs` (untracked)
- `apps/api-server/src/routes/live_statistics.rs`
- `apps/api-server/src/services/live_statistics_service.rs`
- `apps/api-server/src/services/martingale_exchange_preconfigure_service.rs`
- `apps/backtest-engine/src/indicators.rs`
- `apps/backtest-engine/src/bin/reprice_martingale_candidates.rs` (untracked)
- `apps/backtest-engine/src/martingale/indicator_runtime.rs` (untracked)
- `apps/backtest-engine/src/search.rs`
- `apps/backtest-engine/src/sqlite_market_data.rs`
- `apps/backtest-engine/src/walk_forward.rs`
- `apps/backtest-engine/tests/search_scoring_time_splits.rs`
- `apps/market-data-gateway/src/main.rs`
- `apps/trading-engine/src/execution_sync.rs`
- `apps/trading-engine/src/order_sync.rs`
- `apps/trading-engine/src/statistics.rs`
- `apps/trading-engine/src/trade_sync.rs`
- `apps/trading-engine/tests/martingale_runtime.rs`
- `apps/trading-engine/tests/order_sync.rs`
- `crates/shared-binance/src/client.rs`
- `deploy/docker/docker-compose.yml`
- `.dockerignore`

These should be reviewed as a second commit or kept with the Martingale fix only if directly required by tests.

## Category C: Web / Billing / UI Work Not Specific To This Margin Fix

These files are likely unrelated to the GLM capital-model repair and should be isolated before committing the Martingale fix:

- `apps/web/app/[locale]/(public)/login/page.tsx`
- `apps/web/app/[locale]/(public)/register/page.tsx`
- `apps/web/app/[locale]/app/analytics/page.tsx`
- `apps/web/app/[locale]/app/billing/page.tsx`
- `apps/web/app/[locale]/app/dashboard/page.tsx`
- `apps/web/app/[locale]/app/exchange/page.tsx`
- `apps/web/app/[locale]/app/help/page.tsx`
- `apps/web/app/[locale]/app/notifications/page.tsx`
- `apps/web/app/[locale]/app/orders/page.tsx`
- `apps/web/app/[locale]/app/security/page.tsx`
- `apps/web/app/[locale]/app/strategies/[id]/page.tsx`
- `apps/web/app/[locale]/app/strategies/new/page.tsx`
- `apps/web/app/[locale]/app/strategies/page.tsx`
- `apps/web/app/[locale]/app/telegram/page.tsx`
- `apps/web/app/[locale]/page.tsx`
- `apps/web/app/api/user/exports/[kind]/route.ts`
- `apps/web/app/api/user/security/route.ts`
- `apps/web/app/api/user/strategies/[id]/route.ts`
- `apps/web/app/api/user/strategies/batch/route.ts`
- `apps/web/app/api/user/telegram/route.ts`
- `apps/web/app/[locale]/app/orders/[kind]/page.tsx` (untracked)
- `apps/web/app/[locale]/app/orders/order-data.ts` (untracked)
- `apps/web/app/[locale]/app/orders/order-tables.tsx` (untracked)
- `apps/web/app/api/ui/preferences/route.ts` (untracked)
- `apps/web/app/api/user/strategies/create-martingale/route.ts` (untracked)
- `apps/web/components/billing/membership-order-form.tsx`
- `apps/web/components/layout/mobile-bottom-nav.tsx`
- `apps/web/components/layout/sidebar.tsx`
- `apps/web/components/onboarding/empty-state-guide.tsx`
- `apps/web/components/shell/shell-preferences.tsx`
- `apps/web/components/shell/user-shell.tsx`
- `apps/web/components/strategies/strategy-inventory-table.tsx`
- `apps/web/components/strategies/strategy-visual-preview.tsx`
- `apps/web/components/strategies/strategy-workspace-form.tsx`
- `apps/web/components/strategies/stop-all-strategies-form.tsx` (untracked)
- `apps/web/components/ui/pnl-trend-chart.tsx`
- `apps/web/lib/api/help-articles.ts`
- `apps/web/lib/api/mock-data.ts`
- `apps/web/next-env.d.ts`
- `apps/web/proxy.ts`

Recommendation: do not include these in a Martingale capital-model checkpoint unless they are explicitly needed for the fixed Martingale UI.

## Category D: Docs, Scripts, And Temporary Monitoring Artifacts

Docs and scripts that may be useful:

- `docs/superpowers/plans/2026-06-25-martingale-margin-capital-parity-fix-for-glm.md`
- Other `docs/superpowers/plans/*.md`
- Other `docs/superpowers/reports/*.md`
- `scripts/cross_task_recombine.py`
- `scripts/download_funding.py`
- `scripts/download_klines.py`
- `scripts/monitor_martingale_backtests.sh`
- `scripts/optimize_martingale_lp_portfolios.py`

Temporary monitor files should not be committed:

- `.monitor_all.txt`
- `.monitor_candidates.txt`
- `.monitor_detail.txt`
- `.monitor_logs_w2.txt`
- `.monitor_logs_w2b.txt`
- `.monitor_logs_w3.txt`
- `.monitor_logs_w3b.txt`
- `.monitor_task_status.txt`
- `.monitor_worker_cpu.txt`
- `.monitor_worker_health.txt`

## Current Backtest State

Three parity tasks exist:

- `lp-martingale-conservative-20260625-parity`
- `lp-martingale-balanced-20260625-parity`
- `lp-martingale-aggressive-20260625-parity`

Observed DB state:

```text
conservative: running, progress_pct=35, stage=search_symbol
balanced: queued
aggressive: queued
```

Observed worker state:

- Container sees 30 CPUs via `nproc`.
- `BACKTEST_WORKER_MAX_THREADS=12`.
- `docker stats` showed `grid-binance-backtest-worker-1` using about `211%` CPU, roughly two cores.
- Worker logs show only:
  - one earlier failure due to missing `start_ms/end_ms`,
  - then kline load completed for conservative,
  - no recent detailed screening progress.

Concern:

- These tasks use `search_mode=funding_repriced_lp_recombine`, which appears to be a recombine/reprice path rather than a guaranteed from-scratch search of all candidates.
- The user's requirement is to rerun corrected backtests under the fixed capital model. The recombine path may be useful as a fast verification, but should not be treated as the final authoritative search unless GLM proves it recomputes every candidate metric from raw 1m bars under the new model.

## Current Smoke Test State

Smoke portfolio found:

```text
portfolio_id=mp_live_smoke_50_v2_20260624
status=stopped
live_executor_started=true
live_executor_state=started
```

Smoke executor strategies:

```text
smoke-sol-50-v2-long-20260624: Running
smoke-sol-50-v2-short-20260624: Running
```

Local DB order state:

```text
smoke-sol-50-v2-long-20260624: 1 Working order
smoke-sol-50-v2-short-20260624: 1 Working order
```

No DB runtime positions were found for those strategies.

Recent strategy events repeatedly show:

```text
order notional 6.802 is below minimum 20 for SOLUSDT
```

Interpretation:

- The 50U smoke test is not clean.
- It appears to reuse a prior stopped portfolio with stale executor state and stale Working orders.
- The attempted SOL smoke order notional is below Binance min notional, so it is not a valid live smoke test for order placement.
- Do not simply start `trading-engine`; first reset or recreate a clean smoke portfolio with orders above Binance min notional and margin within 50U.

## Recommended Checkpoint Strategy

Before GLM continues:

1. Create a non-destructive patch archive or commit plan.
2. Split work conceptually into:
   - Martingale capital model core fix.
   - Martingale/live parity support changes.
   - UI/billing unrelated work.
   - docs/scripts.
3. Do not commit temp monitor files.
4. Do not commit web/billing files into the Martingale core fix unless reviewed.

Suggested first checkpoint commit, after tests are green:

```text
fix: align martingale backtest and live margin capital model
```

Include only Category A plus directly required Category B files.

Suggested second checkpoint commit, if needed:

```text
fix: harden martingale live preflight and exchange accounting
```

Suggested separate UI commit:

```text
feat: update web strategy and account management screens
```

Only if the user wants those web changes kept.
