# Round 4 P0 Live-Parity Implementation Update (2026-07-02)

## Status after P0 implementation

| Feature | shared config | backtest | trading-engine | status | notes |
|---|---|---|---|---|---|
| Conditional SO (`safety_order_condition`) | YES | YES | **YES** (P0.3 done) | **live-parity** | expression evaluated before safety leg in `mark_leg_filled_with_context`; 2 new tests pass |
| Partial TP (first-stage TP price) | YES | YES (full multi-stage) | **YES** (P0.1 first-stage approximation) | **partial-parity** | trading-engine uses first-stage bps as TP trigger; full multi-stage partial-close requires persistent stage tracking across reconcile ticks (future work) |
| Breakeven stop | YES | YES | **approximate** (via StrategyDrawdownPct fallback) | **partial-parity** | BE stop migration not yet in trading-engine; falls back to drawdown-pct SL |
| Equity-reclaim re-entry | YES | YES | NO (calendar cooldown only) | backtest-only | calendar cooldown works; reclaim fraction not evaluated |

## Tests
- trading-engine: 187 tests pass (was 185, +2 Conditional SO tests)
- backtest-engine: 208 tests pass (unchanged)

## What this means for deployment
- Candidates using Conditional SO (like r3-P1-best-cd11) are now closer to live-parity.
- The Partial TP first-stage approximation means live will trigger TP at the first-stage bps (conservative — banks profit early). The backtest's multi-stage behavior (partial close + stage advance + BE) is a refinement that requires more engine work for exact parity.
- For initial deployment, the first-stage approximation is SAFE (it exits earlier than the full multi-stage, so it won't hold positions longer than backtest).
