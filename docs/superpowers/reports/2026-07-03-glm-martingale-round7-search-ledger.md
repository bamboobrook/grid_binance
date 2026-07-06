# GLM Martingale Round 7 Cost-Aware Gap Repair Search Ledger


## r7-A-merge-registry-001 (Task A: Non-repeat registry merge)
- 29 do_not_repeat + 6 partial_or_missing + 5 promising entries.

## r7-B-dd-cost-attribution-001 (Task B: Detailed DD and Cost Attribution)
- 6 candidates fully attributed. Key: funding drag 8-17% of budget. Routes → C/F/G.

## r7-C-cost-gate-001 (Task C: Funding/fee cost gate — FULL 176-candidate grid)
- Config fields added (max_expected_funding_cost_bps, funding_side_bias_mode, taper_safety_after_leg/scale). 208 tests pass.
- Grid: 176 candidates (4 bases × 5 funding thresholds × 2 modes × 4 taper configs) × 6 replays, 971s.
- **RESULT: config-only, no effect.** All candidates identical to baselines. Engine doesn't implement funding cost lookup or taper scaling logic.
- Non-repeat key: cost-gate-config-only-needs-engine-logic

## r7-G-dynamic-blend-001 (Task G: Dynamic cost/DD blend — 324 allocator configs)
- Built equity curves for 6 candidates, ran lagged allocator with 324 parameter combinations.
- **RESULT: ann 53.9%, DD 24.3%** with 30d lookback / 30d rebalance / ann_minus_dd score.
  - DD 24.3% still above balanced 20% gate.
  - ann 53.9% exceeds conservative 50% gate.
  - This is a Python-only equity curve blend (no engine changes).
  - The allocator picks the best 30d performer among all 6 candidates using only prior data.
  - All score functions converge to the same allocation (the best 30d performer is consistent).

## r7-D-turnover-quality (DEFERRED — depends on Task C engine logic)
## r7-E-quarantine (DEFERRED — R6 quarantine already complete)
## r7-F-safety-freeze-taper (DEFERRED — config-only, engine logic needed)
## r7-H-parity (R5/R6/R7 features backtest-only, 4/4 R4 parity in trading-engine)
## r7-I-final-validation (No target pass. See handoff.)

## r7-CF-engine-logic-001 (Task C+F FIXED: Engine logic implemented + full 180-candidate grid)
- Implemented funding cost gate (lazy per-symbol funding lookup) + safety taper (scale margin/notional by taper_scale after leg threshold). 208 tests pass.
- Grid: 180 candidates (4 bases × 5 funding thresholds × 2 modes × 5 taper configs) × 6 replays, 1074s.
- **RESULT: taper fires, funding gate marginal.**
  - ANKR_tp2ts0.5: ann 27.4%, DD 24.2% (DD 32.1→24.2, -7.9pp improvement!)
  - ANKR_tp3ts0.5: ann 27.6%, DD 30.7%
  - Funding gate (ft2-20): ann changes by ±0.2pp (funding rates ~1bps/event, rarely exceeds thresholds)
  - R4 candidates: unchanged (already low funding drag, taper legs not reached often)
- **No config achieves ann>50 AND DD≤20 simultaneously.**
