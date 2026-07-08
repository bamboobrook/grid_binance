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
