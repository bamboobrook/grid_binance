# GLM Plan: Margin Budget Preflight Fix And Live Runbook

Date: 2026-06-26
Remote repo: `/home/bumblebee/Project/grid_binance`
Input handoff: `docs/superpowers/reports/2026-06-26-claude-parity-fix-and-preflight-blocker-handoff.md`

## Executive Decision

The user's definition is correct and must be preserved:

- `50U smoke` means **50 USDT margin principal cap**, not 50 USDT notional cap.
- `1000U formal live` means **1000 USDT margin principal cap**, not 1000 USDT notional cap.
- With 5x leverage, a 50 USDT margin cap can support up to 250 USDT notional exposure if the configured strategy and budget gates allow it.
- Backtest/live return, drawdown, and budget must use margin principal. Orders, PnL, TP/SL, fees, and funding use notional.

The current blocker is not that the user misunderstood leverage. The blocker is that `confirm_start_portfolio` uses an uncapped full-geometric-series projection that does not match runtime budget enforcement.

However, there is a second critical fact: the final LP portfolios were not proven under a 1000 USDT global margin cap. The conservative LP members' weighted full-series planned margin is about 43,915 USDT. Before any formal launch, GLM must prove that the same strategy logic still satisfies the target under the actual 1000U margin cap, or rerun the optimization with that budget cap.

## Current Facts

From Claude's handoff:

- Parity fixes were committed and pushed to `main`, HEAD `a022963` plus handoff commit `6e3da7c`.
- No live orders were placed.
- `trading-engine` was never started.
- `BINANCE_LIVE_MODE=0` was restored.
- Smoke `confirm-start` failed with:

```text
projected margin 64585.1964 exceeds max_global_budget_quote 50.0000
```

Code facts:

- `apps/api-server/src/services/martingale_publish_service.rs::portfolio_projected_capital` sums full `planned_margin_quote(...)` for every strategy.
- `planned_margin_quote` calls `compute_leg_notionals(..., f64::MAX, ...)`, so `Multiplier` sizing expands all legs without considering the live budget cap.
- Runtime budget enforcement uses margin for the next leg:
  - `apps/trading-engine/src/martingale_runtime.rs::enforce_budget_for_next_leg`
  - `next_margin = next_notional / leverage`
  - global cap compares `global_margin_exposure() + next_margin` to `portfolio_budget_quote`.
- Backtest budget rejection uses active margin capital plus next margin/cost in `apps/backtest-engine/src/martingale/kline_engine.rs::budget_rejection_reason`.

Therefore, the preflight currently checks "can every possible leg of every strategy fill at full geometric size?" while runtime checks "can the next actual leg fit inside the margin budget?" These are not the same system.

## Critical Bug To Fix

### Bug A: confirm-start preflight projects uncapped full-series margin

This is the smoke blocker.

Wrong behavior:

- A 50U smoke is rejected because full theoretical planned margin is 64,585U.
- A 1000U formal start would also be rejected because full theoretical planned margin is much higher than 1000U.

Correct behavior:

- `max_global_budget_quote` is a margin principal cap.
- Preflight should verify that the configured portfolio can start and remain bounded by that margin cap using the same next-leg budget semantics as runtime/backtest.
- Preflight may report the full theoretical margin as a risk diagnostic, but must not reject solely because full theoretical margin exceeds the user-supplied margin cap.

### Bug B: strategy-level budget cap mixes margin cap with first-leg notional

`apps/trading-engine/src/main.rs::cap_strategy_budget` currently does:

```rust
effective_budget_cap = max(strategy_budget_cap, first_order_quote)
```

But `strategy_budget_cap` is margin principal, while `first_order_quote` is notional. This is a unit mismatch. It can make per-strategy caps too loose, especially with leverage.

Correct behavior:

- Compare a margin cap against first-leg margin, not first-leg notional.
- For futures, `first_leg_margin = first_order_quote / leverage`.
- For spot, `first_leg_margin = first_order_quote`.
- Use `max(strategy_budget_cap, first_leg_margin)` only if the design intentionally allows the first leg even when the LP weight allocation is smaller than first-leg margin.
- Still enforce the portfolio global margin cap across all strategies.

## Critical Verification Gap Before 1000U

The final visible conservative portfolio has:

- Annualized: 79.3593%
- Max DD: 10.0000%
- Portfolio ID: `mp_margin_v2_lp_conservative_20260626`

But its source snapshots have no 1000U budget cap. A read-only calculation showed weighted full-series planned margin around:

```text
43915.0909 USDT
```

This does not mean 1000U cannot run. It means the final 1000U live behavior will stop adding legs once active margin reaches 1000U, whereas the displayed LP backtest was produced from source curves without that exact 1000U cap. GLM must therefore run a budget-aware replay/backtest before formal live:

- Global margin cap: 1000 USDT.
- Same LP weights.
- Same strategy parameters.
- Same entry/exit/TP/SL/funding/fee rules.
- Same 1m market data.
- Same runtime next-leg budget-blocking semantics.

If the budget-aware replay no longer meets the user's targets, do not start formal live. Re-optimize under a 1000U cap.

## Required Fix Strategy

Do not "fix" this by converting everything to `BudgetScaled` at 50U/1000U.

Why not:

- `BudgetScaled` scales every leg proportionally.
- For these high-multiplier strategies, scaling the full ladder down to 50U can shrink the first order below Binance min notional.
- That changes the backtested mechanics and can create invalid live orders.

Preferred fix:

1. Keep `Multiplier` sizing semantics: `first_order_quote` is order notional.
2. Treat `max_global_budget_quote` as margin cap.
3. During preflight, simulate or project budget-capped execution using the same next-leg margin checks as runtime/backtest.
4. Reject only when:
   - no valid first order can be placed within budget,
   - min-notional constraints fail,
   - required margin + entry fee buffer exceeds available USDT,
   - budget-aware projected state violates the cap,
   - config/live state conflicts exist.

## Step 1: Freeze Safe State

Before coding:

```bash
cd /home/bumblebee/Project/grid_binance
docker compose -p grid-binance --env-file .worktrees/full-v1/.env -f deploy/docker/docker-compose.yml stop trading-engine || true
grep -n '^BINANCE_LIVE_MODE=' .env .worktrees/full-v1/.env
```

Both env files must be:

```text
BINANCE_LIVE_MODE=0
```

Probe Binance read-only through the existing app/API path and prove:

- zero open orders,
- zero non-zero positions,
- hedge mode enabled,
- multi-assets disabled.

Do not start live trading while implementing this plan.

## Step 2: Add A Shared Budget-Capped Projection

Add a shared helper, preferably in `apps/backtest-engine/src/martingale/capital.rs` or a new nearby module, that can compute:

- full theoretical planned margin,
- full theoretical planned notional,
- budget-capped feasible margin projection,
- budget-capped feasible notional projection,
- first-leg margin,
- first-leg notional,
- per-strategy allocation,
- rejected/skipped legs with reasons.

The helper must use the same units everywhere:

- notional for order size,
- margin for budget,
- fee on notional,
- funding on notional/position.

For a portfolio:

1. Read `max_global_budget_quote` as margin cap.
2. Apply LP `portfolio_weight_pct` exactly once.
3. Split long/short internal strategy weights exactly as regenerated by Claude. Do not double them.
4. Compute each strategy's margin allocation:

```text
strategy_margin_cap = global_margin_cap * strategy_weight_pct / 100
```

5. For each strategy, iterate leg notionals in order.
6. Convert each leg notional to leg margin:

```text
leg_margin = leg_notional / leverage
```

7. Include entry fee/slippage buffer in preflight available-balance check, but do not confuse fees with margin principal.
8. Stop adding a strategy's projected legs when the next leg would exceed strategy or global cap.
9. Reject if the first leg itself cannot fit inside both the strategy cap and the global cap, unless the design explicitly allows borrowing unused cap from other strategies. If borrowing is allowed, document it and match runtime.
10. Reject if any actual order notional is below Binance min notional.

Record both:

- `full_series_projected_margin_quote` for risk visibility,
- `budget_capped_projected_margin_quote` for the start gate.

The start gate should compare `budget_capped_projected_margin_quote` and available balance against the user budget, not compare the uncapped full-series number against the budget.

## Step 3: Fix API confirm-start

Modify `apps/api-server/src/services/martingale_publish_service.rs`.

Required behavior:

- `confirm_start_portfolio(... max_global_budget_quote=50)` treats `50` as margin principal cap.
- `confirm_start_portfolio(... max_global_budget_quote=1000)` treats `1000` as margin principal cap.
- `risk_summary.live_start_preflight` must include:
  - `capital_model: margin_budget_cap`
  - `max_global_budget_quote`
  - `full_series_projected_margin_quote`
  - `full_series_projected_notional_quote`
  - `budget_capped_projected_margin_quote`
  - `budget_capped_projected_notional_quote`
  - `first_leg_margin_quote`
  - `first_leg_notional_quote`
  - `projected_fee_quote`
  - `required_with_buffer_quote`
  - `available_usdt`
  - per-strategy projections and skip reasons.

Do not remove the full-series diagnostic. It is useful, but it is not the budget gate.

Update or replace the old test:

```rust
confirm_start_rejects_when_projected_margin_exceeds_budget
```

It currently encodes the wrong behavior for multiplier strategies. New tests should be:

- `confirm_start_accepts_multiplier_when_budget_capped_projection_fits_margin_cap`
- `confirm_start_rejects_when_first_leg_margin_exceeds_budget`
- `confirm_start_rejects_when_available_usdt_below_margin_plus_fee_buffer`
- `confirm_start_records_full_series_and_budget_capped_projection`
- `confirm_start_does_not_treat_leveraged_notional_as_budget`

Hard example:

- first_order_quote=250, leverage=5, max_legs large.
- Budget 50 should allow first-leg margin 50, even though first-leg notional is 250 and full series is much larger.
- If next leg would exceed the 50U cap, it is budget-blocked later, not a confirm-start failure.

## Step 4: Fix Runtime Strategy Budget Cap Unit Mismatch

Modify `apps/trading-engine/src/main.rs`.

Current issue:

```rust
cap_strategy_budget(strategy, budget_cap)
first_leg_budget_quote(strategy) returns first_order_quote
```

`first_order_quote` is notional, but `budget_cap` is margin.

Fix:

- Rename helper to `first_leg_margin_quote`.
- For futures: `first_order_quote / leverage`.
- For spot: `first_order_quote`.
- Use that to determine the minimum per-strategy margin cap.

Add tests:

- With first_order_quote=250, leverage=5, weight-derived strategy cap=10, effective cap should be 50, not 250.
- Global cap still prevents aggregate active margin from exceeding 50/1000.
- Long/short strategy weights still sum to 100, not 200.

## Step 5: Add Budget-Aware Backtest/Replay For Final LP Portfolios

This is mandatory before formal 1000U.

Use the final conservative portfolio:

```text
mp_margin_v2_lp_conservative_20260626
```

Create a replay/backtest path that applies:

```text
max_global_budget_quote = 1000
```

It must use the same budget-blocking semantics as runtime:

- first_order_quote remains notional,
- active margin exposure is sum of open leg margins,
- next leg is skipped/blocked when adding it would exceed global/strategy/symbol/direction cap,
- closing a cycle releases active margin,
- fees/funding/PnL remain notional-based,
- TP/SL behavior matches the parity-fixed runtime.

Output required:

- annualized return,
- max drawdown,
- total return,
- equity curve,
- drawdown curve,
- number of budget-blocked legs,
- symbols/strategies most often budget-blocked,
- max active margin used,
- max notional exposure,
- min order notional observed,
- all min-notional violations if any.

Acceptance:

- If conservative under 1000U still has annualized > 50% and DD <= 10%, continue.
- If it fails, do not start formal live. Re-optimize under the 1000U cap or report the shortfall to the user.

Repeat the same replay for a 50U smoke portfolio if the smoke is intended to validate more than one or two source members.

## Step 6: 50U Smoke Design After Fix

50U smoke is a margin cap smoke. It does not mean 50U notional.

The smoke should be small but valid:

- Use `max_global_budget_quote=50`.
- Use a small subset of strategies if the full 8-member conservative portfolio cannot place valid min-notional first orders under 50U.
- Prefer 1-2 symbols and at most 2-4 internal strategies.
- Ensure each first order notional is above Binance min notional.
- Ensure first-leg margin plus fee buffer fits within 50U.
- Ensure at least one scenario validates safety-leg budget blocking.

Do not use `max_legs=1` as the only smoke if the goal is to validate martingale safety legs. It can be an initial "leg-0 smoke", but there must also be a controlled safety-leg test using a budget that allows leg 0 and at least one next-leg decision.

Smoke phases:

1. **Dry preflight smoke**: API confirm-start passes with 50U; no trading-engine.
2. **Leg-0 live smoke**: start engine, place exactly expected initial order(s), validate TP/SL/order sync, then cleanup.
3. **Restart reconciliation smoke**: restart engine with known open order/position and prove no duplicate order.
4. **Budget-block safety-leg smoke**: prove next leg is either placed if it fits or blocked if it exceeds margin cap; no stale DB `Working` order if blocked.
5. **Cleanup**: cancel orders, flatten positions, set `BINANCE_LIVE_MODE=0`, prove zero orders/positions.

## Step 7: Tests To Run

Run and save logs:

```bash
cd /home/bumblebee/Project/grid_binance
. "$HOME/.cargo/env"
cargo test -p backtest-engine martingale -- --nocapture
cargo test -p backtest-engine portfolio -- --nocapture
cargo test -p backtest-worker -- --nocapture
cargo test -p trading-engine martingale -- --nocapture
cargo test -p trading-engine order_sync -- --nocapture
cargo test -p api-server martingale -- --nocapture
cargo test -p api-server confirm_start -- --nocapture
```

If the last package filter is invalid, run the specific api-server martingale service tests by name.

Also add regression tests proving:

- 50U with leverage 5 permits 250U first-order notional when first-leg margin is 50U.
- 50U does not permit first-leg margin > 50U.
- full-series margin greater than budget is recorded but does not reject by itself.
- budget-capped projection and runtime next-leg budget checks agree.
- 1000U conservative dry preflight uses margin cap, not notional cap.

## Step 8: Build And Deploy

Only after tests pass:

```bash
docker compose -p grid-binance --env-file .worktrees/full-v1/.env -f deploy/docker/docker-compose.yml build api-server trading-engine
docker compose -p grid-binance --env-file .worktrees/full-v1/.env -f deploy/docker/docker-compose.yml up -d --no-deps api-server
```

Do not start `trading-engine` yet.

Verify:

- API image contains the new preflight fields.
- Trading-engine image contains the first-leg margin cap fix.
- `BINANCE_LIVE_MODE=0`.

## Step 9: Execute 50U Smoke

Before live smoke:

1. Probe Binance:
   - zero open orders,
   - zero non-zero positions,
   - hedge mode enabled,
   - multi-assets disabled.
2. Verify smoke DB state:
   - portfolio status `pending_confirmation`,
   - no stale executor strategies,
   - no stale `Working` orders,
   - no runtime positions.
3. Set `.worktrees/full-v1/.env` `BINANCE_LIVE_MODE=1`.
4. Restart only api-server if required for env pickup.
5. Run exchange preconfigure.
6. Run confirm-start with `max_global_budget_quote=50`.
7. Inspect `risk_summary.live_start_preflight` and verify:
   - `budget_capped_projected_margin_quote <= 50`,
   - first order notional above min notional,
   - full-series projected margin may be >50 but is diagnostic only.

Then start `trading-engine` for the smoke only.

Monitor:

- expected order count,
- order notional,
- margin quote,
- position side,
- TP/SL state,
- DB order status,
- Binance order status,
- fees,
- funding sync path,
- duplicate order prevention.

Cleanup:

- stop `trading-engine`,
- cancel all open orders,
- flatten all positions,
- set `BINANCE_LIVE_MODE=0`,
- restart api-server if needed,
- prove zero open orders and zero positions.

Hard stop and cleanup immediately if:

- any duplicate order,
- any min-notional rejection,
- any `-2019 Margin is insufficient`,
- any stale `Working` order created for a blocked leg,
- any TP/SL mismatch,
- any fee/funding/statistics mismatch,
- any open order/position remains after cleanup.

## Step 10: 1000U Conservative Dry Preflight

Only after the budget-aware 1000U replay passes and 50U smoke passes.

Use:

```text
mp_margin_v2_lp_conservative_20260626
max_global_budget_quote=1000
```

Run exchange preconfigure and confirm-start preflight.

Do not start `trading-engine`.

Save:

- full-series projected margin,
- budget-capped projected margin,
- budget-capped projected notional,
- required with buffer,
- available USDT,
- per-strategy margin caps,
- first-leg margin/notional per strategy,
- expected initial order list,
- expected min-notional status.

If confirm-start passes but budget-aware replay failed, do not start. The preflight is only a gate, not a substitute for replay.

## Step 11: User Confirmation Before 1000U Start

After all evidence exists, stop and ask the user:

```text
The 1000U conservative margin-cap live run is ready to start.

Verified:
- budget-aware 1000U replay/backtest passes target
- live/backtest parity tests pass
- confirm-start preflight uses margin cap, not notional cap
- 50U smoke passed and cleanup confirmed
- 1000U dry preflight passed
- Binance is clean before start

Please confirm whether to start the formal 1000U live run.
```

Do not infer this from any previous approval.

## Step 12: Formal 1000U Launch Procedure

Only after explicit user confirmation:

1. Confirm `BINANCE_LIVE_MODE=0` before final prechecks.
2. Probe Binance clean.
3. Set `BINANCE_LIVE_MODE=1`.
4. Start `trading-engine`.
5. Watch first reconciliation cycle.
6. Confirm expected initial orders only.
7. Confirm no duplicate orders.
8. Confirm active margin <= 1000U.
9. Confirm DB orders match Binance orders.
10. Confirm TP/SL state exists for each active cycle.
11. Save launch report.

First 30 minutes monitoring:

- active margin used,
- notional exposure by symbol/direction,
- open orders,
- positions,
- duplicate client order IDs,
- rejected orders,
- TP/SL generation,
- budget-blocked legs,
- fees and funding sync,
- portfolio drawdown guard state,
- API/trading-engine logs.

First 24 hours monitoring:

- every 15 minutes for first 2 hours,
- hourly after that,
- immediate alert on any hard-stop condition.

## Emergency Procedure

If anything goes wrong:

1. Stop `trading-engine`.
2. Snapshot DB order/position/runtime state.
3. Read Binance open orders and positions.
4. Cancel open orders.
5. Flatten positions with reduce-only market orders if needed.
6. Set `BINANCE_LIVE_MODE=0`.
7. Restart api-server on live mode off.
8. Verify zero open orders and zero non-zero positions.
9. Mark affected portfolio/strategies `paused` or `stopped`.
10. Write incident report before any restart.

Never "just restart" after an order mismatch. Reconcile first.

## Required Deliverables From GLM

Create or update:

```text
docs/superpowers/reports/2026-06-26-margin-budget-preflight-fix-report.md
docs/superpowers/reports/2026-06-26-50u-smoke-report.md
docs/superpowers/reports/2026-06-26-1000u-dry-preflight-report.md
```

The reports must include:

- code commits,
- tests run and pass/fail,
- exact preflight JSON,
- budget-aware replay result,
- Binance before/after probes,
- smoke order IDs and DB order IDs,
- cleanup proof,
- 1000U dry preflight proof,
- exact user confirmation text before formal start.

Do not proceed to 1000U formal start until these reports exist and the user confirms.
