# GLM Round 8 Execution Audit And Recheck

**Date:** 2026-07-07
**Branch audited:** `glm-martingale-core-round8`
**Handoff audited:** `docs/superpowers/reports/2026-07-07-glm-round8-handoff-to-chatgpt.md`

## Executive Verdict

Round 8 produced a valuable research near-frontier, but the handoff overstates execution completeness and validity. The best Round 8 result is not a deployable target hit.

Corrected status:

| Candidate | Status | Ann | DD | Target result |
|---|---|---:|---:|---|
| R8 P2 allocator, GLM script as written | research-only, Python | 62.0% | 18.2% | misses all original tiers |
| R8 P2 allocator, no-current-interval-leak replay | research-only, corrected timing | 60.4% | 18.2% | misses all original tiers |
| R4-combo | only fully live-ready sleeve | 34.7% | 17.7% | misses all original tiers |
| R7 ANKR-q | valid backtest sleeve | 63.5% | 28.2% | misses all original tiers |

Original targets remain unmet:

| Tier | Requirement | Corrected Round 8 verdict |
|---|---|---|
| Conservative | ann >=50%, DD <=10%, <5000U, multi-symbol, live reproducible | fail: best ann>=50 research DD is 18.2%; live-ready ann is 34.7% |
| Balanced | ann >=90%, DD <=20%, <5000U, multi-symbol, live reproducible | fail: DD research near-frontier is <=20, but ann is only about 60-62% |
| Aggressive | ann >=110%, DD <=30%, <5000U, multi-symbol, live reproducible | fail: ann is far below 110% |

Round 8 should be treated as a near-frontier discovery round, not a completion round.

## Material Corrections To GLM Handoff

| Handoff claim | Audit correction |
|---|---|
| Branch is `glm-martingale-core-indicator-expansion` | Current audited branch is `glm-martingale-core-round8`. |
| "All 6 tasks P0-P6 complete" | Not strict. P0 and P1 are absent from the registry; P2 lacks true 5-segment allocator validation; P3/P4 ran narrower grids than the Round 8 plan requested. |
| "Total explorations registered: 7" | `exploration-registry.jsonl` has 5 lines: P2-P6 only. |
| "P1 live-parity semantic tests complete" | Tests exist and pass, but most new tests are shallow config/smoke assertions, not behavioral parity. They do not prove trading-engine reproduces R5-R8 portfolio behavior. |
| "P2 5184 configs all full 5-segment validated" | False. P2 script builds four full-period equity curves once, then computes allocator metrics from those curves. It does not compute true allocator segment metrics per candidate. |
| "Allocator has zero free parameters fitted on full period" | Overstated. 5184 parameter combinations were ranked on full-period metrics. This is still parameter search and needs walk-forward or leave-one-segment-out validation. |
| "R8 P2 ann 62.0/DD18.2" | Reproducible only with the current Python timing. A no-current-interval-leak replay gives ann 60.4/DD18.2. |
| "Traded symbols recorded for P2" | P2 grid stores sleeve names in `traded_symbols`, not actual traded symbols. |
| "Pareto ceiling proven" | Not proven. P2 needs repair, live parity is missing, and broad independent sleeve expansion has not been completed. |

## Round 8 Task Compliance

| Task | Plan requirement | Evidence | Audit result |
|---|---|---|---|
| P0 starting frontier | Create corrected frontier with traded-symbol metadata | `r8-starting-frontier.json` exists; registry has no P0 line | Partial |
| P1 live parity and semantic tests | Behavioral tests for funding gate, taper, last-exec SO, vol target, quarantine | Tests exist and pass, but several assert config presence only | Partial |
| P2 regime-rescue allocator | 864+ candidates with full + five segment metrics, no leakage, portfolio gates | 5184 full-period allocator rows; no true segment metrics; timing leak; unused parameters | Partial research result |
| P3 DCA dynamic grid reset | Minimum 432 candidates and preserve 5+ traded symbols | 148 candidates | Partial/narrow, no target hit |
| P4 cost-cover rescue exit | Minimum 324 candidates | 96 candidates | Partial/narrow, no target hit |
| P5 small-capital packaging | Package target or near-target configs with capital and symbol metadata | 4 sleeves packaged; segment metrics exist; some required top-level fields are null | Partial |
| P6 final validation | Exact branch, counts, live-ready status, target answer | Handoff exists but has branch/count/completeness errors | Partial |

## Independent Recheck Evidence

Artifact counts:

```bash
jq '.total_configs, (.results | length)' docs/superpowers/artifacts/glm-martingale-core-round8/r8-regime-rescue-grid.json
jq '.results | length' docs/superpowers/artifacts/glm-martingale-core-round8/r8-dca-dynamic-grid-reset.json
jq '.results | length' docs/superpowers/artifacts/glm-martingale-core-round8/r8-cost-cover-rescue-exit.json
jq '.packages | length' docs/superpowers/artifacts/glm-martingale-core-round8/r8-small-capital-packages.json
```

Observed: P2 5184, P3 148, P4 96, P5 4.

Target-hit check over P2 full-period metrics:

```bash
jq '[.results[] | select(.full_metrics.ann >= 50 and .full_metrics.dd <= 10)] | length' docs/superpowers/artifacts/glm-martingale-core-round8/r8-regime-rescue-grid.json
jq '[.results[] | select(.full_metrics.ann >= 90 and .full_metrics.dd <= 20)] | length' docs/superpowers/artifacts/glm-martingale-core-round8/r8-regime-rescue-grid.json
jq '[.results[] | select(.full_metrics.ann >= 110 and .full_metrics.dd <= 30)] | length' docs/superpowers/artifacts/glm-martingale-core-round8/r8-regime-rescue-grid.json
```

Observed: 0, 0, 0. No original target hit even before live-parity restrictions.

Corrected allocator timing replay:

```text
curve_points {'ANKR-q': 5000, 'QB': 5000, 'R4': 5000, 'fine': 5000}
leaky_replay_like_glm (62.0, 18.2, 420.3)
no_current_interval_leak (60.4, 18.2, 402.2)
```

Interpretation: P2 remains interesting, but the reported 62.0% uses a timing convention that applies the rebalance decision to the interval that helped choose it. The corrected version still has good DD but lower ann.

Engineering verification:

```bash
cargo test -p backtest-engine
```

Result: passed with 185 library tests, 3 `market_data_probe` tests, and 23 `search_scoring_time_splits` tests. Existing warnings remain.

```bash
cargo test -p trading-engine
```

Sandbox run failed only because `execution_effects_emit_telegram_logs_when_bound_and_configured` could not bind a local test server. The same command was rerun outside the sandbox and passed all trading-engine tests, including the four Round 8 live-parity smoke tests.

## P2 Script Defects To Repair

File: `scripts/glm_r8_regime_rescue_allocator.py`

Defects:

1. `max_high_ann_weight` and `min_low_dd_weight` are part of the 5184-grid label but are not used by `run_allocator`.
2. `traded_symbols` is set to `list(CANDS.keys())`, which records sleeve names (`ANKR-q`, `QB`, `R4`, `fine`) instead of actual exchange symbols.
3. Candidate rows do not include true `segment_metrics`, `positive_segments`, `symbol_count`, `portfolio_candidate`, or `max_symbol_budget_pct`.
4. The rebalance decision is computed at timestamp `ts` and then the step from `prev_ts` to `ts` is applied to the newly selected sleeve. This leaks the current interval into the decision.
5. The allocator is Python-only and not implemented in `trading-engine`.

## Validated Carry-Forward

Safe to carry forward:

- P2 corrected timing still gives a strong research near-frontier around ann 60.4% / DD 18.2%.
- P5 sleeve package metrics for individual sleeves are useful and mostly reproducible.
- R4-combo remains the only fully live-ready sleeve.
- No original target is met through Round 8.

Not safe to carry forward as final:

- Any claim that Round 8 fully completed P0-P6.
- Any claim that P2 is live-ready.
- Any claim that P2 passed true five-segment validation.
- Any claim that 5K/current architecture impossibility has been proven.

## Next Action

Round 9 must first repair P2 evidence and live parity, then expand independent multi-symbol sleeve discovery. Budget expansion above 5000U may be logged as diagnostic only, but it cannot count toward the user's target.
