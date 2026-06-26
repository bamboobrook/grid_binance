# GLM Plan: Margin-v2 Live Validation And 1000U Launch

Date: 2026-06-26
Remote repo: `/home/bumblebee/Project/grid_binance`
Final backtest handoff: `docs/superpowers/reports/2026-06-26-margin-v2-final-backtest-handoff.md`
Final result report: `docs/superpowers/reports/2026-06-26-margin-v2-lp-portfolios.md`

## Non-Negotiable Gates

Do not start 1000U formal live trading until the user explicitly confirms again.

Do not start any live smoke or formal live executor until the live/backtest parity checks in this plan pass.

Do not use stale smoke state. Start from a clean smoke portfolio or fully reset old smoke portfolios, executor strategies, local working orders, runtime positions, events, and snapshots.

Do not change the final backtest logic in a way that live cannot reproduce. If you change any parameter, indicator, entry rule, order sizing rule, TP/SL rule, funding rule, fee rule, or portfolio allocation rule, update the live module and rerun the relevant backtests before live validation.

## Final Backtest Inputs

These are the only final portfolios to use:

| Mode | Portfolio ID | Source task | Annualized | Max DD |
|---|---|---|---:|---:|
| Conservative | `mp_margin_v2_lp_conservative_20260626` | `lp-martingale-conservative-20260626-margin-v2` | 79.3593% | 10.0000% |
| Balanced | `mp_margin_v2_lp_balanced_20260626` | `lp-martingale-balanced-20260626-margin-v2` | 108.0591% | 20.0000% |
| Aggressive | `mp_margin_v2_lp_aggressive_20260626` | `lp-martingale-aggressive-20260626-margin-v2` | 128.9630% | 30.0000% |

The formal 1000U target is the conservative portfolio unless the user explicitly chooses otherwise.

## Step 1: Freeze And Verify Current Runtime State

Expected safe state before any work:

- `trading-engine` is not running.
- `BINANCE_LIVE_MODE=0`.
- Binance USDT-M has zero open orders.
- Binance USDT-M has zero non-zero positions.
- Multi-Assets Mode is disabled.
- Hedge Mode is enabled.
- The only visible flyingkid backtest tasks are the three final `lp-martingale-*-20260626-margin-v2` tasks.
- The only visible flyingkid martingale portfolios are the three final `mp_margin_v2_lp_*_20260626` portfolios.

Save a report section with command output proving the state.

## Step 2: Live/Backtest Parity Audit

Audit each source strategy in the conservative portfolio first.

For every LP member and every internal long/short strategy:

- Confirm `first_order_quote` is treated as order notional.
- Confirm planned margin is `sum(notional_leg / leverage)`.
- Confirm live budget checks use margin, not notional.
- Confirm order quantity is `notional / price`.
- Confirm `portfolio_weight_pct` is applied exactly once.
- Confirm long/short candidate weight splitting does not double capital.
- Confirm ATR spacing uses the same ATR period and kline interval as backtest.
- Confirm `indicator_expression` uses the same indicator runtime as backtest.
- Confirm cooldown logic is identical.
- Confirm `take_profit.percent.bps` places/maintains the expected TP order.
- Confirm `stop_loss.strategy_drawdown_pct.pct_bps` is enforced the same way as the backtest.
- Confirm safety-leg generation uses the same multiplier and max legs from the final parameter snapshot.
- Confirm fees use the same configured fee model or live fills record actual commission accurately.
- Confirm funding fee sync is recorded and not double-counted.
- Confirm realized PnL, unrealized PnL, margin, fees, funding, position side, and order status are recorded accurately.
- Confirm ordinary grid strategies still pass existing live/order sync tests.

Known anomaly from GLM smoke to specifically fix or prove fixed:

- A safety leg was generated despite portfolio config `max_legs=1`, because the executor used a stale strategy instance snapshot for leg count. Before any smoke, prove there is exactly one authoritative config snapshot and live runtime uses it consistently for sizing, max legs, triggers, TP, and SL.

## Step 3: Unit And Integration Tests

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
```

Add or confirm tests for:

- LP portfolio member weights are not double-applied.
- LP portfolios with many member strategies preflight using weighted margin correctly.
- `strategy_planned_budget_quote` uses planned margin under portfolio weights.
- Source parameter snapshot and portfolio config cannot disagree on `max_legs`.
- TP/SL orders are generated exactly once and survive restart reconciliation.
- Restart reconciliation never duplicates an existing open order or position.
- Fees and funding sync update live statistics without double-counting.
- Ordinary grid order sync remains green.

## Step 4: Build And Deploy Only After Tests Pass

Rebuild affected services only after tests are green:

```bash
docker compose -p grid-binance --env-file .worktrees/full-v1/.env -f deploy/docker/docker-compose.yml build api-server trading-engine
docker compose -p grid-binance --env-file .worktrees/full-v1/.env -f deploy/docker/docker-compose.yml up -d --no-deps api-server
```

Do not start `trading-engine` yet.

Verify the running binaries include the margin-v2 fields and any new parity fixes.

## Step 5: Clean 50U Smoke Design

The smoke must validate all functions required by the final conservative portfolio without risking the 1000U allocation.

Use a fresh smoke portfolio or fully reset old smoke rows.

Smoke sizing rules:

- Total budget: 50 USDT.
- Initial order notional must be above Binance min notional, preferably >= 25 USDT.
- Planned margin plus fee/slippage buffer must be <= 50 USDT.
- If testing both long and short, calculate total margin for both sides.
- Avoid source configs that create first orders below Binance min notional after weight scaling.

Smoke must cover:

- Exchange preconfigure.
- Confirm-start preflight.
- First order placement.
- TP order placement or TP logic validation.
- Stop-loss path, either by controlled simulation/test mode or carefully bounded live validation.
- Safety leg generation and budget blocking.
- Restart reconciliation with existing order/position.
- Trade fill sync.
- Fee sync.
- Funding sync path.
- Position flatten and cleanup.

## Step 6: 50U Smoke Execution

Before start:

- Probe Binance and prove zero open orders and zero non-zero positions.
- Prove smoke DB state has no stale `Working` orders and no runtime positions.
- Set `BINANCE_LIVE_MODE=1` only for the smoke window.

During smoke:

- Start `trading-engine`.
- Watch logs and DB rows in real time.
- Confirm no duplicate order generation.
- Confirm actual Binance order notional equals configured notional after quantity rounding.
- Confirm DB order status matches Binance.
- Confirm portfolio and strategy statuses are consistent.

After smoke:

- Stop `trading-engine`.
- Cancel all open orders.
- Flatten all positions.
- Restore `BINANCE_LIVE_MODE=0`.
- Probe Binance and prove zero open orders and zero non-zero positions.
- Save the smoke report.

Hard stop:

- Any duplicate order.
- Any stale order resurrected after restart.
- Any min-notional rejection.
- Any `-2019 Margin is insufficient`.
- Any TP/SL mismatch.
- Any fee/funding/statistics mismatch.
- Any open order or position remains after cleanup.

## Step 7: 1000U Conservative Dry Preflight

Only after 50U smoke passes.

Use:

`mp_margin_v2_lp_conservative_20260626`

Run exchange preconfigure and confirm-start preflight with:

`max_global_budget_quote=1000`

Do not start `trading-engine`.

Save:

- Projected margin.
- Projected notional.
- Required with buffer.
- Available USDT.
- Per-member budget allocation.
- Per-symbol initial order notional after weighting.
- Any member that would fall below Binance min notional.
- Exact executor strategy instances that would be created.

If any member falls below min notional after weighting, do not start. Fix budget scaling or portfolio member handling, rerun preflight, and revalidate in 50U smoke.

## Step 8: User Confirmation Required

After Steps 1-7 pass, stop and ask the user:

```text
The corrected conservative portfolio `mp_margin_v2_lp_conservative_20260626` is ready for formal 1000U live start.

Verified:
- backtest target passed: 79.3593% annualized, 10.0000% max DD
- live/backtest parity tests passed
- 50U smoke passed and cleanup confirmed
- 1000U preflight passed
- Binance is clean before start

Please confirm whether to start the formal 1000U live run.
```

Do not infer confirmation from earlier messages. The confirmation must happen after all evidence is available.

## Step 9: Formal 1000U Start After Confirmation

Only after explicit confirmation:

1. Confirm Binance clean one final time.
2. Set `BINANCE_LIVE_MODE=1`.
3. Start `trading-engine`.
4. Monitor first cycle until all expected initial orders are either placed, filled, or intentionally skipped.
5. Confirm no duplicate orders.
6. Confirm DB/live state matches Binance.
7. Save a launch report.

If anything deviates, stop engine, cancel open orders, flatten positions if needed, restore `BINANCE_LIVE_MODE=0`, and report the issue.

## Required GLM Deliverables

Create:

`docs/superpowers/reports/2026-06-26-margin-v2-live-validation-report.md`

It must include:

- tests run and logs,
- code changes made,
- 50U smoke portfolio ID,
- Binance before/after probes,
- order IDs and DB order IDs,
- fee/funding/statistics evidence,
- 1000U dry preflight numbers,
- final user confirmation text before formal start,
- launch evidence if the user confirms.

Do not mark the task complete until the launch report exists and matches the actual runtime state.
