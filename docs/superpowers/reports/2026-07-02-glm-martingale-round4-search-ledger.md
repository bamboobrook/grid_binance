# GLM Martingale Round 4 2025 Breakthrough Search Ledger

Branch: `glm-martingale-core-round4`
Plan: `docs/superpowers/plans/2026-07-02-glm-martingale-core-round4-2025-breakthrough-plan.md`
Round 3 best: `r3-P1-best-cd11` ann 34.5%, DD 17.8%, 4/5 pos, agg +29.2%, h1c 43.7%.
2025 blocker: -10.1%. Order: P1(attribution) → P2(range sleeve) → P3(pump-fade) → P4(active exit) → P5(rebound SO) → P6(premium data) → P7(4-stage TP) → P8(dynamic allocator).


## r4-P1-gate-relax-001 (Long gate relaxation tests)
- Tested 7 relaxed long gates (ce50, ce100, ce200, ce50+rsi, bb_dip, rsi30).
- **RESULT: all worse or equal.** ce200: ann 32.8%/DD 18.7% but 2025 -15.5% (worse), 2026 -17.3% (negative). Relaxing long gate lets in 2025/2026 bear chop where martingale loses even when direction is up (BNB DD -41%, BCH DD -30% in 2025).
- **Non-repeat key: long gate relaxation cannot fix 2025 — the 2025 bull is too choppy for martingale longs.**

## r4-P2-range-sleeve-001 (Range-Regime Martingale Sleeve)
- Tested BB-lower + ADX<18-26 range sleeve in 2025: ~0% (21-225 trades, barely fires). 2025 vol too high for ADX range condition. Rejected.

## r4-P3-pump-fade-001 (Pump-Fade Short — DEFERRED)
- Requires 24h-ROC or wick ratio (not in expression language). Deferred.

## r4-P4-active-exit-001 (Active-Cycle Exit — DEFERRED)
- Requires new engine features (max_cycle_age, no_progress_exit). Deferred.

## r4-P5-rebound-so-001 (Rebound-Confirmed SO — DEFERRED)
- Requires new engine features (rebound_bps, wick_ratio). Deferred.

## r4-P6-premium-data-001 (Premium Data — BLOCKED)
- No local premium/index/mark data. Same as R3 P7.

## r4-P7-four-stage-tp-001 (Four-Stage TP — no improvement)
- 4-stage TP doesn't beat 3-stage. Best 4-stage: ann34.0/DD31.2 (worse DD). Baseline 3-stage: ann34.0/DD18.2.

## r4-P8-dynamic-allocator-001 (Dynamic Config Allocator — DEFERRED)
- No competitive boost configs from P2/P3/P7. Nothing to allocate.

## r4-P4-active-exit-full-001 (P4: Active-Cycle Exit — FULL engine implementation + grid)
- Implemented max_cycle_age_hours + no_progress_exit_hours + no_progress_mfe_bps in backtest engine. 208 tests pass.
- Grid: 5 max_age × 5 no_progress = 25 candidates × 6 replays.
- **RESULT: no improvement.** Best = no-exit baseline (ann34.0/DD18.2/4pos). All active exits reduce ann. Stale cycles are NOT the problem.

## r4-P5-rebound-so-full-001 (P5: Rebound-Confirmed SO — FULL engine + grid)
- Implemented `safety_order_rebound_bps` + local extreme tracking. 208 tests pass.
- Grid: 6 rebound_bps values (None, 20, 40, 60, 100, 150) × 6 replays.
- **RESULT: no improvement.** Best=baseline(ann34.0/DD18.2/4pos). rb20 drops to 2/5 pos. Rebound confirmation delays safety execution, reducing ann without improving 2025.

## r4-P3-pump-fade-short-full-001 (P3: Pump-Fade Short — FULL engine + grid)
- Implemented `roc()` expression function in indicator_runtime. 208 tests pass.
- Grid: 3 roc_periods × 4 roc_thresh × 3 rsi × 2 martingale params = 72 candidates × 6 replays.
- **RESULT: marginal improvement.** BEST: rp720rt18rsi65m1.8tp500 = ann 34.7%, DD 17.7%, 4/5 pos, 2025 -9.96% (slight improvement from -10.16%). 33 frontier_improvements. Pump-fade roc(720)>18 marginally improves 2025 but doesn't flip it positive.
- The ROC function works correctly and is now available for future pump-fade strategies.

## r4-P6-premium-data-full-001 (P6: Premium Data Gate — UNBLOCKED + analyzed)
- Downloaded `data/premium_index.db`: 176,640 rows, 6 symbols (BNB/TRX/BCH/AAVE/SOL/DOT), 2023-2026, 1h interval.
- Signal test: BTC premium > 0.0003 → next 24h avg -0.76% (weak mean-reversion). Occurs 35.7% of 2025 hours.
- Crash coins (AAVE/SOL/DOT) have negative premium in 2025 (-0.0004), meaning shorts collect funding — but still lose on choppy price action.
- **Decision: weak signal.** Premium gate could marginally help but 0.76% per signal is too small to flip 2025 -10%.
- Data is now available for future premium-gate integration if a stronger signal is found.
