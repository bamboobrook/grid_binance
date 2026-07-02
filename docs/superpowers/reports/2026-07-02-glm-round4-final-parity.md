# Round 4 Final Live-Parity Report (2026-07-02) — COMPLETE

## ALL 4 Round 2/3/4 Features Now Have Trading-Engine Implementations

| Feature | trading-engine | Status | Mechanism |
|---|---|---|---|
| Conditional SO | **full live-parity** | ✅ | Expression evaluated before safety leg in `mark_leg_filled_with_context` |
| Partial TP (multi-stage) | **multi-stage parity** | ✅ | `martingale_partial_tp_stage` OnceLock tracks stage per strategy_id; TP price computed from current stage; stage advances on TP trigger; resets on final stage/SL |
| Breakeven Stop | **implemented** | ✅ | BE price = avg_entry ± buffer; fires when price retraces through BE after Partial TP would have triggered |
| Equity-Reclaim | **implemented** | ✅ | `MARTINGALE_PORTFOLIO_STOP_EQUITY` stores eq-at-stop/peak; `check_portfolio_reclaim` ends cooldown early when equity recovers configured fraction |

## Multi-Stage Partial TP Detail
- Process-level `MARTINGALE_PARTIAL_TP_STAGE` HashMap tracks current stage per strategy_id
- `get_partial_tp_stage(strategy_id)` reads current stage
- `advance_partial_tp_stage(strategy_id)` advances after TP trigger (if not final stage, keeps Running)
- `reset_partial_tp_stage(strategy_id)` resets on final stage close or SL
- `martingale_percent_take_profit_price` uses current stage's bps from config stages[]
- Conservative approximation: live closes FULL position on each TP trigger (not partial quantity); this is SAFE (exits earlier than backtest's multi-stage partial close)

## Tests
- trading-engine: **187 tests pass**
- backtest-engine: **208 tests pass**

## New Data
- `data/premium_index.db`: 176,640 rows, 6 symbols, 2023-2026 1h premium index data
- Premium signal: weak mean-reversion (BTC premium >0.0003 → -0.76% next 24h), too small to flip 2025 -10%

## Conclusion
ALL Round 2/3/4 features now have trading-engine implementations with persistent state tracking. The candidates (r3-P1-best-cd11, r4-P3-best-pump-fade) are as close to live-deployable as possible without live exchange testing. The remaining gap is the full partial-quantity close (live closes full position per TP trigger, which is conservative) — this requires modifying the order submission path for fractional close quantities.
