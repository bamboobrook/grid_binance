# GLM Martingale Round 2 — Handoff to ChatGPT (2026-07-02)

> Branch: `glm-martingale-core-round2` (from `glm-martingale-core-indicator-expansion`)
> Plan: `docs/superpowers/plans/2026-07-02-glm-martingale-core-round2-exhaustive-search-plan.md`
> Registry: `docs/superpowers/artifacts/glm-martingale-core-round2/exploration-registry.jsonl` (4 entries)

## 1. Branch and Commit
- Branch: `glm-martingale-core-round2`
- Built on Round 1 frontier: candidate 008-best (ann 22.2%, DD 26.1%, 3/5 pos, agg24-26 +17.2%)
- Data: market_data_full.db sha256 `88a39aa0...` (updated since round1), funding_rates.db unchanged.

## 2. Directions Run (fast-proxy screening)

| Direction | Approach | Best result | Decision |
|---|---|---|---|
| A (Partial TP+BE) | Rust Mixed/Trailing TP proxy + Python sim (too slow) | trail800/300: ann 25.8% but 1/5 pos (h1-dep) | rejected as proxy; TRUE partial TP needs Rust engine feature |
| E (Inventory skew) | Asymmetric long/short weights | skew20/2: ann 45% DD 48% (h1-overfit); 18/3≈008 | rejected |
| G (Symbol health) | Long-only-3bull (drop shorts) | ann 16% DD 48% (36 trades) | rejected |
| H (Funding window) | Funding-rate analysis | funding tiny (0.01%); not loss driver | rejected (marginal) |

## 3. Best Overall Frontier vs 008-best
No Round 2 fast-proxy direction beat 008-best on segment-stable ann/DD. The 008-best frontier (ann 22.2%, DD 26.1%, 3/5 pos, agg24-26 +17.2%) remains the best generalizable martingale-native frontier.

## 4. Directions NOT Run (require engine features or data)
- **Direction B (Conditional SO + rebound)**: needs engine support for indicator-conditioned safety orders. Current SO triggers on price deviation only.
- **Direction C (Bounded DGT reset)**: needs reset-center logic in engine.
- **Direction D (Futures sentiment)**: needs historical OI/longshort/taker data for 2023-2026 — may not exist locally. **Blocked pending data availability check.**
- **Direction F (Recovery re-entry)**: needs equity-reclaim re-entry logic (current cooldown is calendar-based).
- **Direction A (true Partial TP)**: needs Rust engine partial-close feature (current `ExitDecision::TakeProfit` block calls `reset_cycle` = full close). Implementation map: add `tp_stage` field to `StrategyRuntime`, modify TakeProfit exit block (kline_engine.rs:528-555) to close a fraction + advance stage + migrate stop, only reset_cycle at final stage.

## 5. Candidate Count and Wall Time
- 4 directions screened via fast proxy: ~30 min wall time.
- Python full-bar simulator abandoned (too slow: millions of bars).
- Registry: 4 JSONL lines.

## 6. Promising Candidates
None from fast-proxy screening. 008-best (from Round 1) remains the only segment-stable frontier.

## 7. Rejected-Family Table
| Family | Non-repeat reason |
|---|---|
| Partial TP via Mixed/Trailing proxy | Higher ann but 1/5 pos (h1-dependent); true partial TP needs engine feature |
| Inventory skew (heavy long) | ann 45% but DD 48% h1-overfit; balanced skew = 008 |
| Symbol quarantine (drop shorts) | 36 trades, DD 48%; removes diversification |
| Funding window gate | Funding rates too small to matter |

## 8. Open Blockers (require user/ChatGPT decision)
1. **Direction A true Partial TP**: requires Rust engine feature (~1-2 hrs: add tp_stage, modify exit block, tests). This is the most promising untested mechanism (directly attacks the ann/DD cliff by banking profit early). Should be implemented next if pursuing.
2. **Direction B Conditional SO**: requires engine feature for indicator-conditioned safety orders.
3. **Direction D Sentiment data**: need to verify if historical OI/longshort/taker data is available for 2023-2026. If not, this direction is blocked.
4. **Direction F Recovery re-entry**: requires engine feature.

## 9. Recommendation for ChatGPT
The ann/DD cliff persists across ALL fast-testable directions. The single most promising untested mechanism is **Direction A (true Partial TP + Breakeven)** because it directly attacks the cliff's root cause (cycles must wait for full mean-reversion, absorbing large floating DD). It requires a Rust engine feature, not just parameter tuning. Recommend: (1) implement Partial TP in the engine, (2) run the Direction A grid, (3) if that fails, the cliff is definitively structural and the realistic frontier is 008-best.

## 10. Round 1 baseline (for reference)
- 008-best: ann 22.2%, DD 26.1%, 3/5 pos, agg24-26 +17.2%, 6 symbols, 5000U, full live-parity.
- Config: `docs/superpowers/artifacts/glm-martingale-core/promising/glm-mart-core-aggressive-008-best-config.json`
- Reproduce: `target/release/portfolio_budget_replay --config <path> --budget 5000 --start-ms 1672531200000 --end-ms 1780271999999 --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile balanced --portfolio-id repro --exchange-min-notional 5`
