# GLM Martingale Round 13 Search Ledger


## Canonical Carry-In

- Source: `docs/superpowers/artifacts/glm-martingale-core-round12/r1-r12-corrected-status.json`
- Best event-level: R4-combo ann=34.73% / DD=17.69% / 4/5 positive segments (development window)
- R4-combo holdout (2026-06-01~07-10): -11.0% return (NEGATIVE)
- R9 curve diagnostic (62.78/18.38) NOT reproducible at event level (-7.5%/56.0%)
- 1000U/2000U both negative at event level
- Target hits through Round 12: none (event-level)
- Frozen data: data/funding_rates_round12.db with ANKR(3997)+LTC(3741) funding补齐
- Non-repeat registry imported from Round 12 (see plan section 14)

## r13-P1-batch-replay-001 (Task P1: Batch CPU Replay Infrastructure)
- New module: apps/backtest-engine/src/martingale/batch_replay.rs
  - BatchReplay struct preloads bars+funding once via Arc
  - run_single() and run_configs_parallel() call same run_kline_screening_with_funding
  - No shared mutable state between configs
- 5 batch parity tests (martingale_batch_parity.rs), all PASS:
  - batch_replay_constructs_with_valid_dbs
  - batch_replay_rejects_missing_db
  - batch_replay_single_config_returns_metrics
  - batch_replay_parallel_returns_results_for_all_configs
  - batch_replay_results_are_deterministic
- Parity: batch uses same engine function → bitwise identical to CLI subprocess
- Ready for P2-P5 search acceleration

## r13-P2-htf-trend-directed-001 (Task P2: HTF Trend-Directed Martingale)
- 5 HTF state tests (r13_htf_trend_state.rs), all PASS
  - htf_state_uses_only_completed_bars
  - long_state_blocks_short_new_cycle_and_inverse
  - state_change_never_closes_or_copies_existing_cycle
  - drawdown_safety_order_scale_changes_event_quantity
  - adx_threshold_extremes_change_deterministic_event_trace (ADX 0 vs 100 DOES bind on synthetic data)
- Ablation search: 22 configs (direction gate + ADX SO scale + step variants + TP variants + baseline)
- Run: 22 configs × 6 replays, 260s
- **RESULT: 0 target hits. BASELINE (no gate) ann=34.7%/DD=17.7%/4/5 is BEST.**
  - Best gated variant: dir_only_ema100_300: ann=28.7%/DD=21.3%/2/5 (WORSE than baseline)
  - All direction-gated variants reduce both ann and positive segments
- **Key finding: HTF direction gating HURTS R4-combo performance.** The gate blocks too many entries that would have been profitable. The original R4-combo entry triggers (BTC EMA 50/200) already provide sufficient trend filtering without the additional per-direction gating.
- ADX SO scale binds on synthetic data but doesn't improve real performance.
- Non-repeat key: r13-htf-direction-gate-reduces-performance-vs-baseline
- Non-repeat scope: R4-combo with EMA(50/200) or EMA(100/300) direction gate on entry_triggers. The existing BTC EMA(50/200) trigger already captures trend; adding per-direction gating is redundant and harmful.
