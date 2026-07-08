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
