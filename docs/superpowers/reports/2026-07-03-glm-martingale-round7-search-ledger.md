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
