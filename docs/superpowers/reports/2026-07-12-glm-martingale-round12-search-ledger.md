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
