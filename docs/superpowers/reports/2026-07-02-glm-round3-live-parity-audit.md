# GLM Round 3 P0: Live-Parity Audit (2026-07-02)

## Audit method
Code search (`rg`) for each Round 2 feature across `crates/shared-domain`, `apps/backtest-engine`, `apps/trading-engine`.

## Result

| Feature | shared config | backtest behavior | trading-engine behavior | status | blocker |
|---|---|---|---|---|---|
| Partial TP stages (`Partial` variant) | YES (`martingale.rs:103`) | YES (`kline_engine.rs` TakeProfit block + signal) | **NO** | backtest-only | `apps/trading-engine/src/take_profit.rs` + `martingale_runtime.rs` lack partial-close + stage tracking |
| Breakeven after partial TP | YES (`breakeven_after_stage`) | YES (`breakeven_stop_active` in `StrategyRuntime`, `triggered_stop` migration) | **NO** | backtest-only | same: trading-engine has no `breakeven_stop_active` field or logic |
| Conditional safety orders (`safety_order_condition`) | YES (`martingale.rs:208`) | YES (`kline_engine.rs` safety-order block) | **NO** | backtest-only | `apps/trading-engine/src/martingale_runtime.rs` safety-order path doesn't evaluate the condition |
| Equity-reclaim re-entry (`reentry_equity_reclaim_fraction`) | YES (`martingale.rs:215`) | YES (`kline_engine.rs` cooldown + reclaim logic) | **NO** | backtest-only | `apps/trading-engine/src/main.rs` portfolio-stop cooldown is calendar-only |

## Conclusion
ALL four Round 2 features are **backtest-only**. The trading-engine has none of them. The shared-domain config fields exist (so configs parse), but the live execution path ignores them.

## Implication for Round 3
- Round 3 search results using these features are valid as RESEARCH evidence but CANNOT be promoted to live deployment until the trading-engine implements parity.
- P0.2 (patch trading-engine) is required before any candidate using Partial TP / Conditional SO / equity-reclaim can be considered live-ready.
- The portfolio equity stop (Round 1, `portfolio_equity_stop_pct` + cooldown) IS live-parity (implemented in Round 1 in both engines).

## Recommended P0.2 scope (deferred — large change)
Implementing all 4 features in trading-engine is a substantial change (partial-close order sizing, stage persistence across reconcile ticks, BE stop migration, SO condition evaluation, reclaim re-entry). This should be done after a candidate proves the return target in backtest, to avoid wasted engineering on features that don't help.
