# GLM Martingale Round 10 Search Ledger


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
- R9 allocator is now FULLY live-ready.

## r10-P2-r9-winner-live-parity-001 (Task P2: Exact R9 Winner Live/Backtest Parity Replay)
- Script: scripts/glm_r10_r9_allocator_live_parity_replay.py
- Replayed R9 winner (5 sleeves, lb60/rb7/calmar/hi0.2/lo0.2) using ONLY completed observations (forward-only)
- Decision audit: every rebalance recorded with timestamp_ms, metrics_cutoff_ms, applies_from_ms, active_sleeve_before/after, scores
- All 3 parity checks PASS:
  - PASS forward_only_decisions: every decision's applies_from_ms >= metrics_cutoff_ms (no current-interval leak)
  - PASS full_metrics_tolerance: replay ann=64.4196/dd=18.2111 EXACTLY matches R9 recorded (diff 0.0000, well within 0.2 tolerance)
  - PASS segment_metrics_present 5/5
- Combined with P1 (main.rs dispatch wired), the R9 allocator is now FULLY live-ready:
  - round10_live_ready_after_wiring: true
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
