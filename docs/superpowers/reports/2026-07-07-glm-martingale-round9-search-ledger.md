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

## r9-P2-live-allocator-parity-001 (Task P2: Implement Live-Reproducible Portfolio Allocator)
- New module: apps/backtest-engine/src/martingale/allocator_replay.rs (pure Rust port of repaired Python allocator)
- Public API: AllocatorConfig, AllocatorScoreFunction (4 variants), AllocatorState, RollingMetrics, AllocatorMetrics
- Pure function: run_allocator_replay(curves, cfg, budget) -> Option<AllocatorMetrics>
- Live runtime: AllocatorState with new(), may_open_new_cycle_for(), rebalance()
- 3 invariants proven:
  - Forward-only timing (no current-interval leak)
  - Weight params bind (max_high_ann_weight excludes, min_low_dd_weight forces)
  - No forced cycle close on sleeve switch (only new-cycle gating)
- Tests added (all PASS):
  - backtest-engine: allocator_switch_uses_completed_interval_only, allocator_respects_max_high_ann_and_min_low_dd_weights, live_runtime_persists_allocator_active_sleeve_until_next_rebalance
  - trading-engine: r9_live_runtime_persists_allocator_active_sleeve_until_next_rebalance, r9_allocator_state_default_persists_initial_sleeve
- Full test suites PASS:
  - backtest-engine: 214 tests (188 lib + 3 probe + 23 splits), 0 failures
  - trading-engine: 195 tests across all binaries (incl. 2 new R9), 0 failures
- Gap: trading-engine main.rs per-tick dispatch wiring is not yet connected to AllocatorState. The module is complete and parity-proven; live event-loop integration is the remaining follow-up.

## r9-P3-multi-symbol-sleeve-library-001 (Task P3: Expand Multi-Symbol Sleeve Library)
- Symbol universe: top 50 by quote volume with full 2023-2026 coverage (206 qualified)
- Correlation ranking: 43 candidates ranked by max abs correlation to existing anchors (BNB/TRX/AAVE/SOL/DOT/BCH/ANKR/XRP)
- Lowest-correlation candidates: PAXGUSDT (0.13), LPTUSDT (0.48), 1000LUNCUSDT (0.49), ZECUSDT (0.49), CFXUSDT (0.50)
- Portfolio grid: 7 long/short cells × 3 max_sym_pct × 2 corr × 4 anchors × 3 first_order × 3 cooldown = 1512 specs
- Each portfolio uses R4-combo's validated martingale parameters (mult 2.8 long / 1.8 short, partial TP ladder, ATR/ADX indicators, BTC trend gates) but varies the symbol set
- Run: 1512 specs × 6 replays (full + 5 segments), 2261s
- **RESULT: 520 evaluated (992 timed out on n8/n10 due to 600s per-portfolio limit), 0 promoted**
- **KEY FINDING: 0/520 portfolios achieved 4/5 positive segments.** Best was 2/5 (n6_L5S1_ANKR-q: ann 35.0% / DD 31.9% / 2/5 pos).
- Top by ann: n6_L3S3_ANKR-q fo30 cd86400: ann 47.3% / DD 36.1% / 1/5 pos
- Best at DD<=20: n6_L4S2_R4 fo30 cd21600: ann 13.5% / DD 18.7% / 2/5 pos
- Distinct symbols touched across all evaluated: 13 (BNBUSDT, TRXUSDT, BCHUSDT, AAVEUSDT, SOLUSDT, DOTUSDT, ANKRUSDT, XRPUSDT, PAXGUSDT, LPTUSDT, 1000LUNCUSDT, ZECUSDT, CFXUSDT)
- **Conclusion: R4-combo's 4/5 positive segments is NOT robust to symbol substitution.** None of 520 random multi-symbol portfolios using the same martingale parameters reproduced 4/5. The R4-combo result appears to benefit from specific symbol-period fit, not a generalizable martingale + multi-symbol property. This is a critical anti-overfitting finding.
- Saved: r9-multi-symbol-sleeve-library.json
- Non-repeat key: r9-multi-symbol-random-substitution-no-4of5-reproduction
