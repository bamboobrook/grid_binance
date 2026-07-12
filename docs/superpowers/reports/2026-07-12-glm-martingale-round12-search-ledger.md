# GLM Martingale Round 12 Search Ledger


## Canonical Carry-In

- Source: `docs/superpowers/artifacts/glm-martingale-core-round11/r1-r11-corrected-status.json`
- R9 `64.4196/18.2111` is curve-reuse diagnostic, NOT event-level candidate.
- After ANKR funding correction: R9 curve diagnostic = `62.7845/18.3840`, R7 event-level = `62.3718/28.6873`.
- R7 at 2000U = `5.4232/45.5823` — cannot use 5000U single point for small capital.
- P1 only has helper/static-start gate; started executor bypasses allocator.
- P3 has no native minigrid engine/live semantics.
- P4 only negates partial-TP approximation.
- P5's 8100 labels: 7996 are fixed TP; real ATR spacing untested.
- P6 historical ADX/DD-scale dimensions inert; script fixed but not rerun.
- P7 no new sleeves; weight field is not actual fractional weights.
- Target hits through Round 11: none (event-level).
- Do not repeat: curve slicing as promotion, LP recombination as promotion, strict all-or-nothing SO blocking, trailing_lock approximation, partial-TP minigrid approximation, static-gate-only live_ready claims.

## r12-P1-frozen-data-001 (Task P1: Frozen Data + Funding Gate)
- Script: scripts/glm_r12_build_frozen_data_manifest.py
- Created data/funding_rates_round12.db with ANKRUSDT (3997 rows) + LTCUSDT (3741 rows) funding补齐 from Binance official /fapi/v1/fundingRate
- Manifest: docs/superpowers/artifacts/glm-martingale-core-round12/run-manifests/r12-data-manifest.json
- **Funding gate PASS: all 15 candidate symbols have funding coverage**
- Development window: 2023-01-01 to 2026-05-31 23:59:59.999
- Untouched holdout: 2026-06-01 to 2026-07-10 23:59:59.999 (LOCKED, not opened until P9)
- 4 new tests (r12_funding_manifest.rs), all PASS:
  - funding_manifest_detects_gap_duplicate_and_text_mark_price
  - canonical_range_hash_changes_when_in_range_row_changes
  - market_manifest_is_stable_when_rows_after_end_ms_are_appended
  - replay_rejects_traded_futures_symbol_with_missing_funding
- Manifest includes per-symbol: market row count/min/max/duplicates/canonical_sha256, funding row count/min/max/canonical_sha256, engine binary hash

## r12-P2-promotion-validator-001 (Task P2: Unified Promotion Validator + OOS Harness)
- Script: scripts/glm_r12_validate_candidate.py
- Validation schema: docs/superpowers/artifacts/glm-martingale-core-round12/r12-validation-schema.json
- Gates implemented:
  - Event-level shared-budget replay (portfolio_budget_replay)
  - 5 cold-start segments (independent engine starts)
  - 4 anchored walk-forward folds
  - Budget ladder (1000, 2000, 3000, 4000, 4999)
  - Cost stress (base, fee x1.5, slippage x2, combined)
- Auto-derivation: fully_live_ready and target_hit derived from gate results, NOT manually set
- Validator ready for use by P3-P7 candidates

## r12-P3-event-level-r9-benchmark-001 (Task P3: Event-Level R9 Benchmark)
- Script: scripts/glm_r12_r9_event_level_benchmark.py
- Merged all 5 R9 sleeves into one 36-strategy portfolio with shared 4999U budget
- 8 symbols: AAVEUSDT, ANKRUSDT, BCHUSDT, BNBUSDT, DOTUSDT, SOLUSDT, TRXUSDT, XRPUSDT
- Funding: data/funding_rates_round12.db (Round12 frozen with ANKR/LTC补齐)
- **CRITICAL RESULT: event-level ann=-7.5% / DD=56.0% — FAMILY BENCHMARK FAILURE**
- R9 curve diagnostic (corrected): ann=62.7845 / DD=18.3840
- Event-level delta: **-70.3pp ann, +37.6pp DD** — curve diagnostic grossly overstated performance
- Root cause: 36 strategies sharing 4999U → severe budget contention. Max capital used only 1016U of 4999U (20% utilization). 57408 trades, 32 budget-blocked legs. Most strategies cannot fill due to insufficient budget.
- Budget ladder confirms: only 4000U+ avoids total disaster; 1000U gives ann=-37.8%/DD=85.5%
- Positive segments: 4/5 (2025 is -26.1%) — but full-period is negative due to budget contention
- **Decision per plan P3.3: STOP expanding R9 selector parameters.** The curve-reuse diagnostic (62.78/18.38) is NOT reproducible at event level. The R9 allocator family is closed as a target candidate.
- Non-repeat key: r12-event-level-r9-36-strategy-budget-contention-failure
- Non-repeat scope: any combined multi-sleeve portfolio with >20 strategies sharing <5000U budget will suffer the same contention. Future allocator designs MUST use proper sleeve budget allocation, not naive strategy merge.

## r12-P3-r4-event-level-baseline-001 (R4-combo Event-Level Baseline)
- R4-combo (6 strategies, 6 symbols) event-level with Round12 frozen funding
- Full (4999U): **ann=34.73% / DD=17.69% / 4/5 positive segments** (2025 -8.7%)
- Budget ladder: 1000U=-15.7%, 2000U=-7.1%, 3000U=49.5%, 4000U=40.7%, 4999U=34.7%
- **Critical: 1000U and 2000U are NEGATIVE** — small capital is a genuine challenge
- This is the true event-level baseline to beat. All future candidates must be validated at event level.
- R9 curve diagnostic (62.78/18.38) was NOT reproducible; R4-combo event-level (34.73/17.69) IS the real frontier.

## r12-P5-so-v2-binding-probe-001 (Task P5: SO V2 Binding Probes)
- Binding probe 1: ADX skip threshold 35 vs 70 → **SAME trades (4758), SAME ann (34.73%), SAME DD (17.69%) → DOES NOT BIND**
- Binding probe 2: DD scale 0.5 vs 1.0 → **SAME trades (4758), SAME ann (34.73%), SAME DD (17.69%) → DOES NOT BIND**
- **Decision per plan P5.1: STOP — do NOT run the 4608 SO v2 search.**
- Root cause: R4-combo uses partial TP (not percent TP), so safety_skip_adx_threshold and drawdown_state_rules may not affect partial-TP cycles. The engine's safety order path for partial TP doesn't consult these fields in the same way as percent TP.
- Non-repeat key: r12-so-v2-adx-ddscale-inert-on-r4-combo-partial-tp
- Non-repeat scope: any search on R4-combo varying safety_skip_adx_threshold or drawdown_state_rules will produce identical results. These fields only bind on percent-TP strategies.

## r12-P6-atr-cycle-depth-001 (Task P6: ATR Spacing + Cycle-Depth TP)
- Binding probes: ATR spacing binds (13750 trades) but ann=-4.74% (worse than R4-combo 34.73%)
- Fixed-percent step binds: 150bps=34.73%, 300bps=-3.21%
- Search: 4 step × 4 tp0 × 3 tp2 × 3 tp4 × 4 age = 576 configs
- Run: 576 configs × 6 replays, 2998s
- **RESULT: 576/576 evaluated, 0 target hits.**
- Best: s150_t0_300_t2_600_t4_500: ann 26.0% / DD 28.6% / 2/5 pos (worse than R4-combo baseline)
- **Conclusion: cycle-depth TP variants do NOT improve on R4-combo's original partial TP ladder.**
- ATR spacing confirmed inferior to fixed-percent for this strategy family.
- Non-repeat key: r12-cycle-depth-tp-no-target

## r12-P4-native-minigrid-binding-probe-001 (Task P4: Native DCA Minigrid Binding Probe)
- Probe 1: R4-combo without dca_minigrid vs with dca_minigrid(3/50/1of4/35/2)
  - No minigrid: ann=34.73% dd=17.69% trades=4758
  - With minigrid: ann=34.73% dd=17.69% trades=4758 → **IDENTICAL → DOES NOT BIND**
- Probe 2: dca_minigrid(5/30/1of6/20/3) → ann=34.73% dd=17.69% trades=4758 → **IDENTICAL**
- **Root cause: kline_engine.rs does NOT read or process the dca_minigrid config field.** The config struct exists (R11 P3) but the engine integration was deferred. The field is silently ignored.
- **Decision per plan P4: STOP — native minigrid engine integration is required for any binding.** Config struct + validation + price math are complete (R11 P3: 5 tests pass), but kline_engine.rs needs ~200 lines of minigrid logic insertion after safety order fills.
- research_only: true (config struct ready, engine integration not done)
- Non-repeat key: r12-native-minigrid-config-field-inert-in-kline-engine
- Non-repeat scope: any search using dca_minigrid config field on current engine will produce identical results to base. Engine integration required.

## r12-P7-lp-rebuild-001 (Task P7: LP Portfolio Event-Level Rebuild)
- Source: docs/superpowers/reports/2026-06-26-margin-v2-lp-portfolios.md (conservative 8-symbol members)
- Symbols: LTCUSDT, DYDXUSDT, INJUSDT, FILUSDT, ICPUSDT, XRPUSDT, UNIUSDT, BTCUSDT
- Used R4-combo's martingale parameters (proven event-level) on LP symbols
- Budget: 4999U shared (original LP used 18k-144k planned margins)
- **RESULT: ann=30.44% / DD=41.31% / 4/5 positive segments**
  - Worse than R4-combo baseline (34.73%/17.69%) in both ann and DD
  - DD 41.31% far exceeds balanced gate (20%)
  - 2026_ytd segment: -55.0% (severe recent underperformance)
- Budget ladder: 1000U=-17.8%, 2000U=-4.0%, 3000U=36.8%, 4000U=21.0%, 4999U=30.4%
- **Conclusion: LP portfolio designed for high-capital (18k-144k) does NOT work at 4999U shared budget.** The DD inflation from 10% (high-cap) to 41.31% (4999U) confirms the capital sensitivity. LP portfolios are diagnostic-only at this budget.
- Non-repeat key: r12-lp-portfolio-4999u-dd-inflation
- Non-repeat scope: any LP-derived portfolio scaled to <5000U shared budget will suffer DD inflation. Original LP metrics require high planned margins.

## r12-P8-batch-benchmark-001 (Task P8: Batch Acceleration Benchmark)
- Baseline: one-process-per-config, avg 21.7s/config, 166 configs/hour single, 692 configs/hour with 28 workers (15% parallel efficiency)
- Bottleneck: 117GB market_data_full.db re-read by each subprocess
- Batch plan: preload + Rayon (not implemented; current throughput adequate for grids <=1000)
- GPU: not recommended (complex event branches + Decimal logic)

## r12-P9-holdout-001 (Task P9: Untouched Holdout Validation)
- Holdout window: 2026-06-01 to 2026-07-10 (39 days)
- Candidate: R4-combo (best event-level)
- **RESULT: 4999U holdout return = -11.0% (NEGATIVE)**
- Budget ladder: all budgets negative in holdout
- DD 12.9% (within balanced tier but return negative)
- **Holdout gate FAILED: return < 0.** R4-combo is losing money in the most recent 39-day period.
- This is a significant OOS finding: the R4-combo strategy may be decaying.

## r12-P10-production-parity-001 (Task P10: Production DB/Executor Parity)
- R11 P1 already wired allocator to main.rs with 7 production tests (all pass)
- R10 P1 has 3 integration tests (all pass)
- R11 P3 has 5 native minigrid config tests (all pass)
- R12 P1 has 4 funding manifest tests (all pass)
- Full suites: backtest-engine 211 pass, trading-engine 205 pass, 0 failures
- Native minigrid live parity: NOT applicable (config field inert, engine integration pending)
