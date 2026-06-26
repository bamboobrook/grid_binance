# Martingale Margin Capital Parity Fix Plan For GLM

Date: 2026-06-25
Owner account: flyingkid2022@outlook.com
Remote repo: `/home/bumblebee/Project/grid_binance`
Portfolio involved in the failed live run: `mp_funding_conservative_20260623`

## Critical Safety Rule

Do not restart formal live trading until this plan is completed and verified.

Allowed before final user confirmation:

- Code inspection and fixes.
- Backtests and simulations.
- Read-only Binance probes.
- 50 USDT smoke trading only after code fixes pass tests.

Not allowed before final user confirmation:

- Restarting the formal conservative portfolio with 1000 USDT or 2000 USDT.
- Creating live executor strategies for the formal portfolio.
- Letting `trading-engine` submit formal orders.

Current known safe baseline from the previous cleanup:

- `trading-engine` should remain stopped.
- Binance USDT-M should have `openOrderCount=0`.
- Binance USDT-M should have `nonzeroPositionCount=0`.
- Multi-Assets mode should be off.

Reconfirm these before doing any live-related operation.

## Problem Statement

The Martingale backtest and live system currently mix these concepts:

- Order notional: the leveraged position size sent to Binance.
- Margin capital: real account capital reserved/used by the strategy.
- Strategy/portfolio principal: denominator for return, annualized return, drawdown, and budget checks.

The user identified a specific example:

```text
Symbol: BTCUSDT
First order input: 10 USDT
Leverage: 2x
Order multiplier: 2x
Max legs: 4
Leveraged order notionals: 10, 20, 40, 80
Correct principal/margin capital: 10/2 + 20/2 + 40/2 + 80/2 = 75 USDT
Wrong principal if using leveraged notional sum: 150 USDT
```

This means all old Martingale search results, annualized return rankings, drawdown numbers, and live capital limits are suspect. Treat previous Martingale portfolio winners as stale until rerun under the fixed capital model.

## Confirmed Code Hotspots

Backtest:

- `apps/backtest-engine/src/martingale/rules.rs`
  - `compute_leg_notionals` currently returns values that are later treated as margin in some places and notional in others.
- `apps/backtest-engine/src/martingale/kline_engine.rs`
  - `run_kline_screening_with_funding` initializes `budget_quote` via `portfolio_budget_quote`.
  - `StrategyRuntime::new` builds `margins` from `compute_leg_notionals`, then builds `notionals` by multiplying by leverage.
  - `total_return_pct`, `annualized_return_pct`, `max_drawdown_pct`, `planned_margin_quote`, `max_capital_used_quote`, and `equity_curve` all depend on this interpretation.
- `apps/backtest-engine/src/martingale/metrics.rs`
  - `planned_margin_quote`, `notional_quote`, and related tests encode the current assumptions.
- `apps/backtest-engine/src/portfolio_search.rs`
  - `combine_equity_curves` scales member equity curves from each candidate initial equity; if candidate initial equity uses the wrong capital denominator, portfolio return and drawdown are also wrong.
- `apps/backtest-worker/src/main.rs`
  - Candidate summaries and portfolio summaries persist the metrics and should expose the corrected fields.

Live:

- `apps/trading-engine/src/martingale_runtime.rs`
  - `place_leg` uses `margin_quote = leg_notional(...)`, then `notional_quote = margin_quote * leverage`.
  - `order_margin_quote` divides stored notional by leverage.
  - This must align exactly with the fixed backtest model.
- `apps/trading-engine/src/main.rs`
  - `strategy_planned_budget_quote` and `strategy_planned_budget_from_sizing` compute planned budget from sizing without a clearly enforced margin/notional semantic.
  - `martingale_runtime_config_from_portfolio` uses this for `portfolio_budget_quote`.
  - `reconcile_running_martingale_portfolios` and `reconcile_martingale_executor_strategies` need portfolio-wide budget reservation, not isolated per-strategy checks.
- `apps/api-server/src/services/martingale_publish_service.rs`
  - `confirm_start_portfolio` currently accepts `max_global_budget_quote` and sets portfolio status to `running` after readiness checks.
  - It must add a hard capital preflight before status changes.
  - `live_portfolio_config_snapshot` must not duplicate weights when a candidate expands into long and short internal strategies.

## Required Decision: Canonical Field Semantics

Before changing code, choose and document one canonical semantic for `MartingaleSizingModel`:

Preferred fix for this incident:

- Treat `first_order_quote`, `max_budget_quote`, and `custom_sequence.notionals` as leveraged order notional for futures.
- Convert each futures leg to required margin with `margin_quote = notional_quote / leverage`.
- For spot, `margin_quote = notional_quote`.
- Return, annualized return, drawdown, capital usage, portfolio weights, and live budgets use margin capital.
- Fees, slippage, funding, PnL, quantity, TP/SL, and exchange orders use notional.

If GLM chooses the alternative semantic, where `first_order_quote` means margin capital, then the user example above will not match. Do not choose that alternative unless the user explicitly changes the requirement.

Add a code comment and user-facing docs to remove the ambiguity.

## Implementation Tasks

### 1. Freeze And Mark Stale

1. Confirm `trading-engine` is stopped:

```bash
cd /home/bumblebee/Project/grid_binance
docker compose --env-file .env -f deploy/docker/docker-compose.yml ps trading-engine api-server
```

2. Read-only probe Binance USDT-M:

```bash
cd /home/bumblebee/Project/grid_binance
node /tmp/binance_usdm_readonly_probe_all.js
```

3. Do not start the formal portfolio. If needed, set its DB status back to `pending_confirmation` or `paused` only after backing up the DB rows.

### 2. Add A Shared Capital Model

Create a shared helper in backtest-engine, or a shared crate if cleaner:

```text
leg_notional_quote(strategy, leg_index)
leg_margin_quote(strategy, leg_index)
planned_notional_quote(strategy)
planned_margin_quote(strategy)
order_quantity(notional_quote, price)
```

Rules:

- Futures notional sequence = `first_order_quote * multiplier^leg`.
- Futures margin = `notional / leverage`.
- Spot margin = notional.
- Fees/slippage = notional-based.
- Funding = notional-based.
- PnL = notional exposure and price movement.
- Capital budgets and returns = margin-based.

Hard test:

```text
first_order_quote=10
multiplier=2
max_legs=4
leverage=2
planned_notional_quote=150
planned_margin_quote=75
```

### 3. Fix Backtest Metrics

In `apps/backtest-engine/src/martingale/kline_engine.rs`:

- Build `notionals` directly from sizing.
- Build `margins` from `notional / leverage`.
- `portfolio_budget_quote` must use planned margin, not planned notional.
- `capital_required` must be `margin + entry fees + slippage`.
- `capital_used_quote` and `max_capital_used_quote` must track margin plus costs.
- `equity_quote = initial_margin_capital + realized_pnl + unrealized_pnl`.
- `total_return_pct = net_pnl / initial_margin_capital * 100`.
- `annualized_return_pct` must use the same initial margin capital.
- `max_drawdown_pct` must be based on the corrected equity curve.
- `planned_margin_quote` must equal planned margin, not planned notional.
- Add `planned_notional_quote` if the API/UI needs to display both.

In `apps/backtest-engine/src/martingale/metrics.rs`:

- Rename helpers or add new helpers so names cannot lie.
- Keep backward-compatible serde fields only if needed, but document their meaning.
- Update tests that currently assume old capital semantics.

In `apps/backtest-engine/src/portfolio_search.rs`:

- Ensure each `EvaluatedCandidate.planned_margin_quote` is corrected.
- Ensure `combine_equity_curves` scales from corrected candidate initial margin equity.
- Ensure portfolio drawdown and annualized return are recomputed from corrected combined equity.
- Ensure member allocation uses margin capital weights, not notional exposure weights.

### 4. Fix Search And Candidate Persistence

In `apps/backtest-worker/src/main.rs`:

- Update output summaries so candidate and portfolio result JSON includes:
  - `planned_margin_quote`
  - `planned_notional_quote`
  - `max_capital_used_quote`
  - `total_fee_quote`
  - `total_slippage_quote`
  - `total_funding_quote`
  - corrected `total_return_pct`
  - corrected `annualized_return_pct`
  - corrected `max_drawdown_pct`
- Mark old results stale or rerun tasks so UI does not mix old and new capital models.

### 5. Fix Live Runtime

In `apps/trading-engine/src/martingale_runtime.rs`:

- Ensure generated order quantity uses notional / price.
- Ensure budget checks use margin.
- Store enough metadata to recover margin from each live order without guessing.
- Do not rely on `notional / leverage` if leverage can change after order creation; persist planned leverage or margin on the runtime order.
- Budget enforcement must include:
  - active filled positions,
  - live open orders,
  - DB runtime orders already submitted,
  - newly generated Working orders.

In `apps/trading-engine/src/main.rs`:

- Replace `strategy_planned_budget_quote` with margin-aware planned capital.
- Add portfolio-level reservation across all executor strategies.
- `reconcile_martingale_executor_strategies` must not generate safety orders independently if the whole portfolio would exceed `max_global_budget_quote`.
- If a safety leg is blocked by budget, do not persist it as a submit-ready `Working` order.

### 6. Fix Publish And Start Preflight

In `apps/api-server/src/services/martingale_publish_service.rs`:

- Fix `live_portfolio_config_snapshot` so item weight is divided across expanded internal strategies when one candidate contains both long and short legs.
- Add `confirm_start_portfolio` hard preflight:
  - parse portfolio config,
  - apply corrected margin model,
  - simulate initial live orders across the whole portfolio,
  - read Binance USDT-M available USDT balance,
  - read Binance USDT-M open orders and positions,
  - compute projected margin reservation,
  - reject if projected capital exceeds `max_global_budget_quote`,
  - reject if projected capital exceeds available balance after a safety buffer,
  - reject if existing live orders/positions conflict.
- Persist preflight details into `risk_summary.live_start_preflight`.
- Only set status to `running` if this preflight passes.

Suggested safety buffer:

```text
required_margin_with_buffer = projected_margin * 1.05 + projected_fees_slippage
```

### 7. Preserve Normal Grid Trading

Do not break ordinary grid execution:

- Keep normal grid order generation untouched except shared order-sync improvements.
- Run existing normal grid tests.
- Add one regression test proving ordinary grid still starts, submits orders, records fills, closes, and computes stats.

### 8. Tests Required Before Any Smoke Trade

Minimum test list:

```bash
cd /home/bumblebee/Project/grid_binance
. "$HOME/.cargo/env"
cargo test -p backtest-engine martingale
cargo test -p backtest-engine portfolio
cargo test -p trading-engine martingale
cargo test -p api-server martingale
cargo test -p trading-engine order_sync
```

Add explicit tests:

- `futures_planned_margin_uses_notional_divided_by_leverage`
  - Input: 10, multiplier 2, 4 legs, 2x leverage.
  - Expected planned notional 150, planned margin 75.
- `annualized_return_uses_margin_principal`
  - PnL divided by 75 in the example, not 150.
- `drawdown_uses_margin_equity_curve`
  - Drawdown denominator is corrected equity peak.
- `portfolio_combines_corrected_margin_equity`
  - Multi-member portfolio initial equity equals total allocated margin capital.
- `publish_long_short_weight_does_not_double_capital`
  - One item with 100% weight and two internal strategies must not become 200% capital.
- `confirm_start_rejects_when_projected_margin_exceeds_budget`
  - API start fails before status becomes `running`.
- `budget_blocked_safety_leg_not_persisted_as_working_order`
  - No unplaceable `Working` order remains after budget block.
- `ordinary_grid_order_sync_regression`
  - Existing normal grid flow still works.

### 9. Rebuild And Rerun Backtests

After tests pass:

1. Rebuild affected services:

```bash
cd /home/bumblebee/Project/grid_binance
docker build -f deploy/docker/rust-service.Dockerfile --build-arg APP_NAME=backtest-worker -t grid-binance-backtest-worker:latest .
docker build -f deploy/docker/rust-service.Dockerfile --build-arg APP_NAME=api-server -t grid-binance-api-server:latest .
docker build -f deploy/docker/rust-service.Dockerfile --build-arg APP_NAME=trading-engine -t grid-binance-trading-engine:latest .
```

2. Restart API and backtest worker only. Keep `trading-engine` stopped until 50 USDT smoke is approved:

```bash
docker compose --env-file .env -f deploy/docker/docker-compose.yml up -d api-server backtest-worker
```

3. Rerun all three Martingale searches from scratch under the corrected capital model:

- Conservative: must search again for annualized return > 50% under corrected margin principal and drawdown requirement.
- Balanced: must beat the previous best under corrected margin principal.
- Aggressive: must beat the previous best under corrected margin principal.

4. Old results must not be displayed as valid winners unless they are recomputed.

### 10. 50 USDT Smoke Test Before Formal Live

Only after code fixes, tests, rebuild, and corrected backtests:

1. Create a dedicated 50 USDT smoke portfolio or clone of the conservative portfolio with `max_global_budget_quote=50`.
2. Run exchange preconfigure.
3. Run `confirm-start` with 50 USDT only.
4. Start `trading-engine`.
5. Verify:
   - initial order quantity matches corrected notional model,
   - margin reservation matches corrected margin model,
   - TP order or local TP logic matches backtest,
   - SL logic matches backtest,
   - safety order generation respects global budget,
   - no duplicate orders after restart,
   - trade records record real commission,
   - funding records are captured,
   - positions are synced by symbol and position side,
   - normal grid strategies still reconcile.
6. Stop the smoke test and flatten/cancel everything.
7. Only then ask the user for explicit approval before starting the formal 1000 USDT conservative portfolio.

## Acceptance Criteria

The work is complete only when all are true:

- The 10/2x/4-leg example returns planned margin 75 USDT and planned notional 150 USDT.
- Backtest annualized return and drawdown use corrected margin capital.
- Portfolio search uses corrected member equity curves and corrected margin weights.
- Live start preflight rejects any configuration that would exceed budget or available USDT.
- Live runtime cannot submit orders beyond the corrected global budget.
- Budget-blocked safety orders are not persisted as live-submittable `Working` orders.
- Existing positions and open orders are checked before any restart.
- Normal grid trading tests still pass.
- Three portfolio searches have been rerun from scratch.
- A 50 USDT smoke test passes before any formal live deployment.
- Formal 1000 USDT launch waits for explicit final user confirmation.

## Notes For GLM

Do not treat this as a UI-only or ranking-only bug. This is a core capital model bug that affects:

- result correctness,
- portfolio selection,
- live budget limits,
- order sizing,
- risk controls,
- and user trust.

Do not reuse the previous winning combinations until they are recomputed under the corrected margin principal model.
