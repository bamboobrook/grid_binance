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
