# GLM Martingale Round 11 Search Ledger


## Canonical Carry-In

- Source: `docs/superpowers/artifacts/glm-martingale-core-round10/r1-r10-corrected-status.json`
- Best research: R9/R10 allocator ann 64.4196% / DD 18.2111% / 5/5, module-ready plus static live gate, not fully live-ready.
- Best fully live-ready: R4-combo ann 34.7233% / DD 17.6843% / 4/5.
- Target hits through Round10: none.
- Do not repeat: Round8 leaky allocator timing, Round9 random symbol substitution, reserve-only DD compression, Round10 strict condition-gated SO, Round10 trailing_lock approximation, Round10 partial-TP minigrid approximation, static-gate-only live_ready claims.

## r11-P1-live-allocator-production-wiring-001 (Task P1: Finish True Production Live Allocator)
- R10 gap CLOSED: production rolling metrics + dynamic rebalance + state persistence now wired.
- New module: apps/trading-engine/src/martingale_allocator_live.rs
  - parse_allocator_config, parse_strategy_to_sleeve_id
  - allocator_state_from_json, state_to_json
  - parse_observations, completed_allocator_metrics (uses obs <= rebalance_boundary only)
  - read_allocator_state_with_priority (risk_summary > config > fallback)
- main.rs changes:
  - runtime_with_allocator_state rewritten: uses priority reader, computes completed metrics, calls state.rebalance when now_ms >= next_rebalance_ms
  - last_allocator_snapshot tracked across strategy loop
  - risk_summary[allocator_state] persisted via update_martingale_portfolio_risk_summary after cycle loop
- 7 new production tests (martingale_allocator_live_production.rs), all PASS:
  - r11_allocator_rebalances_when_next_rebalance_due_in_main_reconcile
  - r11_allocator_persists_state_and_blocks_new_cycles_after_switch
  - r11_allocator_uses_completed_observations_only
  - r11_allocator_reads_risk_summary_state_before_config_fallback
  - r11_parse_observations_filters_invalid_entries
  - r11_parse_allocator_config_round_trips
  - r11_parse_strategy_to_sleeve_id_handles_objects
- Full suites green: backtest-engine 211 pass, trading-engine 205 pass (incl. 7 new R11).
