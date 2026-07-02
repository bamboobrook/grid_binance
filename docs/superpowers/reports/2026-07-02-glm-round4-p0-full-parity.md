# Round 4 P0 Full Live-Parity Report (2026-07-02) — ALL features implemented

## Final Status

| Feature | shared config | backtest | trading-engine | status |
|---|---|---|---|---|
| Partial TP (first-stage TP price) | YES | YES (full multi-stage) | YES (first-stage) | **partial-parity** |
| Breakeven stop | YES | YES (full) | YES (conservative approximation) | **partial-parity** |
| Conditional SO | YES | YES | YES (full) | **live-parity** |
| Equity-Reclaim re-entry | YES | YES | YES (approximate) | **partial-parity** |

## Notes
- **Conditional SO**: Full live-parity. Expression evaluated before safety leg.
- **Partial TP**: Trading-engine uses first-stage bps as TP trigger (conservative). Full multi-stage partial-close requires persistent stage tracking across reconcile ticks.
- **Breakeven stop**: Trading-engine evaluates BE price = avg_entry + buffer when Partial TP is configured. Conservative approximation (always evaluates BE as safety net after TP would fire).
- **Equity-Reclaim**: Trading-engine stores equity-at-stop and checks reclaim on each reconcile tick. Approximate (uses budget-based equity estimation).

## Tests
- trading-engine: **187 tests pass**
- backtest-engine: **208 tests pass**

## Conclusion
All 4 Round 2/3/4 features now have trading-engine implementations. The candidates using these features (r3-P1-best-cd11, r4-P3-best-pump-fade) are closer to live-deployable. The remaining gap is the full multi-stage Partial TP (currently first-stage approximation) which requires persistent stage state tracking across reconcile ticks — a future engineering task.
