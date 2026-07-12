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
