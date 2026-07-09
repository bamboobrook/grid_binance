# GLM Martingale Round 10 Search Ledger

> **Correction notice (2026-07-09):** This ledger is superseded for live-readiness status by `docs/superpowers/reports/2026-07-09-glm-round10-execution-audit-and-fix.md` and `docs/superpowers/artifacts/glm-martingale-core-round10/r1-r10-corrected-status.json`. Corrected facts: R10 P1 wired a static inactive-sleeve new-cycle gate only; it did not wire production rolling allocator metrics, dynamic rebalance, or allocator_state persistence. R9 allocator remains research/module-ready plus partial static live gate, not fully live-ready.


## Canonical Carry-In

- Source: `docs/superpowers/artifacts/glm-martingale-core-round9/r1-r9-corrected-status.json`
- Best research: R9 allocator ann 64.4196% / DD 18.2111% / 5/5, module-ready but not fully live-ready.
- Best fully live-ready: R4-combo ann 34.7233% / DD 17.6843% / 4/5.
- Target hits through Round 9: none.
- Do not repeat: Round8 leaky allocator timing, R9 random symbol substitution, reserve-only DD compression, stale handoff registry counts, module-only live_ready claims.

## r10-P1-live-allocator-mainloop-wiring-001 (Task P1: Wire Allocator Into Trading-Engine Main Loop)
- R9 gap CLOSED: trading-engine main.rs per-tick dispatch is now wired to the allocator.
- Implementation:
  - MartingaleRuntime: added allocator_state + strategy_to_sleeve_id fields; added set_allocator_state_for_test, allocator_allows_new_cycle, rebalance_allocator methods.
  - main.rs reconcile_running_martingale_portfolios: added runtime_with_allocator_state() helper that parses optional allocator_config + strategy_to_sleeve_id + allocator_state from portfolio JSON; the per-strategy loop now consults allocator_allows_new_cycle before start_cycle_with_futures_preflight.
  - Gate records 'martingale_allocator_blocked_new_cycle' event and skips the new cycle for inactive sleeves.
  - Existing cycles are NEVER force-closed (verified by r10_allocator_switch_does_not_cancel_existing_cycle).
- 3 new integration tests (apps/trading-engine/tests/martingale_allocator_live_integration.rs), all PASS:
  - r10_allocator_blocks_new_cycles_for_inactive_sleeves_in_main_reconcile
  - r10_allocator_switch_does_not_cancel_existing_cycle
  - r10_allocator_rebalance_uses_completed_equity_only_in_main_loop
- Full suites green: backtest-engine 211 pass, trading-engine 198 pass (incl. 3 new R10 + 2 carried R9 tests).
- Corrected 2026-07-09: this is only a partial close. R10 wired static inactive-sleeve new-cycle gating when `allocator_state` is already present. It did not wire production rolling metrics, dynamic rebalance, or `allocator_state` persistence, so the R9 allocator is NOT fully live-ready.

## r10-P2-r9-winner-live-parity-001 (Task P2: Exact R9 Winner Live/Backtest Parity Replay)
- Script: scripts/glm_r10_r9_allocator_live_parity_replay.py
- Replayed R9 winner (5 sleeves, lb60/rb7/calmar/hi0.2/lo0.2) using ONLY completed observations (forward-only)
- Decision audit: every rebalance recorded with timestamp_ms, metrics_cutoff_ms, applies_from_ms, active_sleeve_before/after, scores
- All 3 parity checks PASS:
  - PASS forward_only_decisions: every decision's applies_from_ms >= metrics_cutoff_ms (no current-interval leak)
  - PASS full_metrics_tolerance: replay ann=64.4196/dd=18.2111 EXACTLY matches R9 recorded (diff 0.0000, well within 0.2 tolerance)
  - PASS segment_metrics_present 5/5
- Combined with corrected P1, the R9 allocator is replay/module-valid plus static-gate-ready, but NOT fully live-ready:
  - round10_static_gate_ready_after_wiring: true
  - round10_live_ready_after_wiring: false
  - remaining live gaps: production rolling metrics, dynamic rebalance, allocator_state persistence
  - annualized_return_pct: 64.4196
  - max_drawdown_pct: 18.2111
  - target_hit: false (still misses balanced ann>=90 and conservative DD<=10)

## r10-P3-condition-triggered-so-ladder-001 (Task P3: Condition-Triggered Safety Order Ladders)
- Grid: 2 bases × 2 calc_modes × 4 min_dev × 3 step_scale × 3 vol_scale × 3 max_legs × 5 conditions × 2 batch = 2880 configs (≥2500 ✓)
- Mechanism: indicator conditions gate whether a safety order is ALLOWED after min deviation. Uses safety_order_basis, safety_order_condition, Multiplier spacing, sizing multiplier scaling.
- Run: 2880 configs × 6 replays (full + 5 segments), 10146s
- **RESULT: 0 target hits, 0 near-frontier.** Condition-triggered SO ladders do NOT improve the ann/DD frontier.
- positive_segments distribution: 186 at 0/5, 1220 at 1/5, 1006 at 2/5, 370 at 3/5, 92 at 4/5, 6 at 5/5
- Best by ann: R4-combo_from_md250_ss1.35_vs1.2_ml6_atr_c: ann 41.4% / DD 28.2% / 2/5 pos
- Best at DD<=20: R7-ANKR-q_from_md80_ss1.15_vs1.2_ml4_atr_c: ann 20.9% / DD 19.4% / 4/5 pos (well below R4-combo ann 34.7%)
- Best low-DD: R7-ANKR-q_from_md80_ss1.05_ml5_rsi_r: ann 18.5% / DD 13.1% / 4/5 pos
- **Conclusion: indicator-gated safety orders REDUCE performance.** The conditions block too many SO fills, preventing cycles from averaging down. The unrestricted SO path (R4-combo default) is strictly better.
- Non-repeat key: r10-condition-triggered-safety-orders-no-target

## r10-P4-multi-tp-trailing-runner-001 (Task P4: Multi-TP With Final Trailing Runner)
- Grid: 2 bases × 3 TP ladders × 4 trail_act × 4 trail_dev × 2 be_after × 3 be_buf × 4 time_limit = 2304 configs (≥1200 ✓)
- Mechanism: partial TP ladder with final stage trailing runner via trailing_lock_* fields (R6 Task D)
- Run: 2304 configs × 6 replays (full + 5 segments), 11285s
- **RESULT: 0 target hits. NO candidate reached DD<=20.** positive_segments: 768 at 0/5, 1536 at 1/5.
- Best by ann: R7-ANKR-q_25_25_25_ta80_td25_be1_bb30: ann 15.7% / DD 44.8% / 1/5 pos
- **Conclusion: multi-TP + trailing runner is strictly WORSE than base.** The 4-stage TP ladders fragment position closes, and trailing lock at final stage causes the position to hold too long into reversals. The R4-combo 3-stage partial TP (without trailing) is strictly better.
- Non-repeat key: r10-multi-tp-trailing-runner-no-target

## r10-P5-dca-minigrid-hybrid-001 (Task P5: DCA Minigrid Hybrid Sleeves)
- Grid: 2 bases × 4 dca_step × 3 dca_scale × 3 dca_vol × 3 mg_levels × 2 mg_spacing × 2 mg_pt × 3 max_active = 2592 configs (≥1500 ✓)
- Mechanism: DCA spacing via Multiplier mode, minigrid-like profit-taking via additional partial TP stages, max_active_cycles for concurrency. All martingale-native (no new trade type).
- Run: 2592 configs × 6 replays (full + 5 segments), 14321s
- **RESULT: 0 target hits.** positive_segments: 1581 at 0/5, 678 at 1/5, 291 at 2/5, 42 at 3/5. NONE reached 4/5.
- Best by ann: R7-ANKR-q_ds250_dsc1.1_dv1.0_ml2_ms70_mp55: ann 10.5% / DD 19.5% / 1/5 pos
- **Corrected conclusion: partial-TP minigrid approximation is strictly WORSE than base.** The engine did not have a native minigrid executor; this script approximated minigrids via additional partial TP stages and `max_active_cycles`. Native inventory-reducing DCA minigrid remains untested and must not be counted as exhausted.
- Non-repeat key: r10-dca-minigrid-partial-tp-approx-no-target

## r10-P6-regime-defensive-allocator-v2-001 (Task P6: Regime-Defensive Allocator V2)
- Grid: 4 lookback × 4 rebalance × 4 score × 4 cash_trigger × 3 defensive_floor × 3 hyst = 2304 configs (≥2000 ✓)
- Lagged features only (forward-only): rolling returns, DD, funding drag approximations. Forbidden: current_interval_return, future labels, full-period rank.
- Run: 2304 configs × 6 allocator replays (curve-reuse, fast), 593s
- **RESULT: 0 target hits. LOSO not triggered (no candidates passed targets).**
- Best: lb60_rb7_calmar_like_none_df0.0: ann 64.4% / DD 18.2% / 5/5 pos (same as R9 winner — no improvement)
- 0 configs reached ann>=90 (balanced gate) or DD<=10 (conservative gate)
- **Corrected conclusion: defensive allocator V2 approximation did not improve the R9 frontier.** Cash/funding/chop features were partly mapped to DD-style approximations, so exact funding-drag, chop-state, and cash-defense implementations remain eligible only if they stay inside martingale capital gating and use forward-only inputs.
- Non-repeat key: r10-regime-defensive-allocator-v2-approx-no-target
