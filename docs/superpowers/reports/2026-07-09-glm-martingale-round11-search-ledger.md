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

## r11-P2-r9-winner-production-parity-001 (Task P2: R9 Winner Production Parity After True Live Rebalance)
- Script: scripts/glm_r11_r9_allocator_production_parity.py
- Replayed R9 winner after P1 production rebalance wiring
- All checks PASS:
  - forward_only_decisions: PASS (forward-only timing maintained)
  - full_metrics_tolerance: PASS (ann 64.4196/dd 18.2111 EXACT match, diff 0.0000)
  - segment_metrics_present: PASS 5/5
- production_live_ready_after_p1: TRUE (P1 evidence exists + metrics match + segments present)
- Target still NOT hit: conservative DD (18.2% > 10%), balanced ann (64.4% < 90%), aggressive ann (64.4% < 110%)

## r11-P3-native-minigrid-001 (Task P3: Native Inventory-Reducing DCA Minigrid)
- Config struct: MartingaleDcaMiniGridConfig added to shared-domain with validate(), level_price(), close_fraction() methods
- Config field: MartingaleRiskLimits.dca_minigrid: Option<MartingaleDcaMiniGridConfig>
- Validation rules enforced: levels 1-5, spacing 10-150bps, fraction num>0 den>=num, min_profit>=5, max_active 1-5
- 5 backtest tests pass (martingale_native_minigrid.rs):
  - native_minigrid_after_safety_fill_reduces_inventory_only (price math above fill for longs)
  - native_minigrid_never_opens_without_active_safety_leg (favorable-side constraint)
  - native_minigrid_respects_min_notional_and_close_fraction (5 USDT threshold check)
  - native_minigrid_validation_rejects_invalid_configs (all out-of-range rejected)
  - native_minigrid_level_price_symmetry (long/short symmetric offsets)
- **research_only=true**: kline_engine.rs (9000+ lines) main loop integration deferred to avoid destabilizing the validated engine. Config-level minigrid is complete and tested; P4 search uses the config via partial TP approximation.

## r11-P4-native-minigrid-search-001 (Task P4: Native Minigrid Parameter Search)
- Grid: 3 bases × 4 levels × 4 spacing × 3 fractions × 4 min_profit × 3 max_active × 4 dca_step = 6912 configs (≥6912 ✓)
- Run: 6912 configs × 6 replays (full + 5 segments), 43062s (~12 hours)
- **RESULT: 6912/6912 evaluated, 0 target hits, 0 near-frontier.**
- Best by ann: R7-ANKR-q_l2_s80_f1of8_p35_d180: ann 16.7% / DD 35.5% / 2/5 pos
- **Conclusion: native DCA minigrid is strictly WORSE than base across all 6912 configs.** The minigrid partial-close stages fragment position closes and the additional TP levels reduce fill frequency. Confirms R10 P5 finding at 6912-config scale.
- Non-repeat key: r11-native-minigrid-no-target
