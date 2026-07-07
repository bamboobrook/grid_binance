# GLM Martingale Round 9 Search Ledger


## r9-P0-round8-correction-001 (Task P0: Round 8 Evidence Repair Registry)
- Read audit: docs/superpowers/reports/2026-07-07-glm-round8-execution-audit-and-recheck.md
- Created: docs/superpowers/artifacts/glm-martingale-core-round9/r9-round8-correction.json
- Corrected status:
  - best_research_allocator_leaky: ann 62.0% / DD 18.2% (live_ready=false, timing leak)
  - best_research_allocator_no_current_interval_leak: ann 60.4% / DD 18.2% (live_ready=false)
  - best_live_ready: R4-combo ann 34.7% / DD 17.7% (only fully live-ready sleeve)
  - valid_backtest_sleeve_ankr_q: ann 63.5% / DD 28.2%
- 7 invalid claims documented with corrections.
- All three targets (conservative/balanced/aggressive) NOT MET through Round 8.
- Cannot claim architecture impossibility until Round 9 P1-P5 complete.

## r9-P1-allocator-repair-001 (Task P1: Repair Allocator Semantics + Segment Validation)
- Semantic validation script: scripts/glm_r9_validate_allocator_semantics.py
  - PASS case_no_current_interval_leak (forward ret within 0.5pp of leaky; no future-info edge)
  - PASS case_weight_params_bind (capped B-forced ret 16.1 vs uncapped 1427.0 — params bind)
  - PASS case_segment_metrics_present (all 5 segments have full ann/dd/ret)
- Repaired allocator: scripts/glm_r8_regime_rescue_allocator.py
  - Timing leak FIXED: rebalance at ts only affects intervals AFTER ts
  - max_high_ann_weight / min_low_dd_weight now bind (exclude/force sleeve eligibility)
  - traded_symbols pulled from underlying sleeve configs (8 real exchange symbols, not sleeve labels)
  - TRUE per-segment allocator replay (slices each sleeve curve to segment window, re-runs allocator)
  - portfolio_candidate flag, symbol_count, target_profile_hit all computed
- Grid: 3 lookbacks × 3 rebalances × 4 scores × 3 max_hi × 3 min_lo × 3 cash_dd × 3 hyst = 2916 non-duplicate rows × 6 allocator replays (full + 5 segments), 305s
- **RESULT: BEST at DD<=20 (balanced DD gate): ann 64.48% / DD 18.21% / 5/5 positive segments**
  - Better than R8 leaky 62.0%/18.2% AND R8 corrected 60.4%/18.2%
  - 5/5 positive segments (R8 claimed 4/5) — 2025 segment is +4.76% positive!
  - 8 traded symbols, portfolio_candidate=true, live_ready=false (P2 will fix)
- Distinct metric tuples: 48 / 2916 rows (params bind meaningfully)
- Top by ann (ignoring DD gate): ann 64.5% / DD 21.4% / 5/5 pos — exceeds balanced DD by 1.4pp
- Target hits: 0 (balanced needs ann>=90, aggressive needs ann>=110 — both far off)
- Saved: r9-allocator-repair-grid.json
