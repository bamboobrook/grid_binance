# GLM Round 7 Execution Audit And Recheck

**Date:** 2026-07-07
**Branch audited:** `glm-martingale-core-round7`
**Scope:** Round 1-7 execution completeness, Round 7 artifacts, replay validity, target status, and next-round blockers.

## Executive Verdict

Round 7 was not 100% complete as described by its final handoff, because the handoff was committed before later repair commits and was not updated. The branch itself contains more work than the handoff says: Task C/F backtest engine logic was implemented and searched, and Task D/E turnover-quarantine was later searched.

The best valid Round 7 frontier is:

| Candidate | Full ann | Full DD | Total return | Positive segments | Max capital used | Principal breached |
|---|---:|---:|---:|---:|---:|---|
| `ANKR_q1w24p24` | 63.5105% | 28.1972% | 436.4939% | 4/5 | 1729.28U | false |

It is a valid backtest frontier improvement over `R5-ANKR` because annualized return increased and full-period DD decreased. It still does not meet the requested targets and cannot be called live-ready because R5-R7 feature parity in `trading-engine` is still incomplete.

## Target Status

| Target | Requirement | Round 7 status |
|---|---|---|
| Conservative | ann >=50%, DD <=10%, small capital, reproducible | **Fail**. Best ann candidates have DD above 24%; no Round 7 candidate reaches DD <=10%. |
| Balanced | ann >=90%, DD <=20%, small capital, reproducible | **Fail**. No Round 7 candidate reaches ann >=90 with DD <=20. |
| Aggressive | ann >=110%, DD <=30%, small capital, reproducible | **Fail**. `ANKR_q1w24p24` DD is under 30 but ann is 63.51, far below 110. |

`ANKR_q1w24p24` is small-budget runnable in the backtest engine under the 5000U budget cap, but the replay uses cap-truncated static legs and records 24 budget-blocked legs. A live promotion must prove the trading engine rejects or truncates legs in the same way.

## Round 7 Handoff Corrections

| Handoff claim | Corrected finding |
|---|---|
| "Registry: 10 JSONL lines" | Actual `exploration-registry.jsonl` has 11 lines. |
| Task C cost gate deferred/config-only | Initial run was config-only, but commit `6329f9c` implemented backtest engine funding gate and safety taper, then ran 180 candidates. |
| Task D/E deferred | Later commit `6fd9179` ran 60 turnover/quarantine candidates and found `ANKR_q1w24p24`. |
| R7 final frontier remains 59.5% ann / 18.0% DD | Outdated. R7 best new single backtest candidate is `ANKR_q1w24p24` at 63.51% ann / 28.20% DD. The 18.0% DD figure is the earlier R6 low-DD blend, not a same-candidate R7 target pass. |
| Dynamic blend is the R7 frontier improvement | It is a research-only Python equity-curve blend at 53.93% ann / 24.31% DD. It is not live parity and not the best R7 ann/DD candidate after D/E. |
| "Engine logic for R5/R6/R7 features not fully implemented" | More precise: backtest logic exists for Round 7 cost gate/taper; trading-engine parity remains missing for R5-R7 promoted features. |

## Round 7 Task Compliance

| Task | Plan requirement | Audit result |
|---|---|---|
| A non-repeat registry | Merge R1-R6 failed/partial/promising branches | Complete. 29 `do_not_repeat`, 6 partial/missing, 5 promising entries. |
| B DD/cost attribution | Attribute six anchors and route next searches | Mostly complete. Report and JSON exist. Fixed a malformed Markdown segment-DD table header during this audit. |
| C funding/fee cost gate | Implement and search cost-aware cycle gate | Complete after repair for backtest engine. Funding gate was marginal; no target hit. Missing dedicated semantic tests. |
| D turnover quality | Search turnover/fee-aware martingale variants | Partial but useful. Later 60-candidate D/E run found `ANKR_q1w24p24`; not as broad as a full attribution-turnover family. |
| E quarantine | Search symbol/direction quarantine improvements | Partial but useful. Same D/E run found the best R7 candidate. |
| F safety freeze/taper | Implement and search safety-order taper/freeze | Complete after repair for backtest engine. Taper reduced DD but killed ann; no target hit. Missing dedicated semantic tests. |
| G dynamic blend | Lagged allocator across martingale sleeves | Complete as research-only. Not live parity and not a deployable strategy by itself. |
| H live parity | Bring R5/R6/R7 behavior into trading engine | Incomplete. Documented only; must be Round 8 P0/P1 before promotion. |
| I final validation | Validate original targets and package handoff | Conclusion "no target pass" is correct, but handoff metrics are outdated and must be superseded by this audit. |

## Recheck Evidence

Commands and observations run during this audit:

```bash
git status --short --branch
```

Result: branch `glm-martingale-core-round7` was clean and aligned with `origin/glm-martingale-core-round7` before edits.

```bash
wc -l docs/superpowers/artifacts/glm-martingale-core-round7/exploration-registry.jsonl \
  docs/superpowers/reports/2026-07-03-glm-martingale-round7-search-ledger.md \
  docs/superpowers/reports/2026-07-03-glm-round7-handoff-to-chatgpt.md
```

Result: registry 11 lines, ledger 49 lines, handoff 71 lines.

```bash
jq '.results | length' docs/superpowers/artifacts/glm-martingale-core-round7/r7-cost-aware-gate-grid.json
jq '.results | length' docs/superpowers/artifacts/glm-martingale-core-round7/r7-cost-gate-taper-engine.json
jq '.results | length' docs/superpowers/artifacts/glm-martingale-core-round7/r7-dynamic-cost-dd-blend.json
jq '.results | length' docs/superpowers/artifacts/glm-martingale-core-round7/r7-turnover-quarantine.json
```

Result: 176, 180, 324, and 60 candidates respectively.

```bash
target/release/portfolio_budget_replay \
  --config docs/superpowers/artifacts/glm-martingale-core-round7/promising/r7-ANKR-q1w24p24.json \
  --budget 5000 \
  --start-ms 1672531200000 \
  --end-ms 1780271999999 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --profile aggressive \
  --portfolio-id r7-ankr-q-full-recheck-20260707 \
  --exchange-min-notional 5
```

Replay result matched the artifact:

- Annualized return: 63.51046213512199%
- Max drawdown: 28.197233976815543%
- Total return: 436.4938993890554%
- Max capital used: 1729.2803179982502U
- Principal breached: false
- Funding: 852.600038866412U
- Fees: 319.509819817492U
- Slippage: 142.00436436332964U
- Trades: 377
- Stops: 47

Segment metrics from the audited result artifact:

| Segment | Ann | DD | Return | Breached |
|---|---:|---:|---:|---|
| h1_2023 | 431.8297% | 28.1972% | 129.0352% | false |
| h2_2023 | 10.2158% | 25.1083% | 5.0257% | false |
| 2024 | 62.3259% | 33.6494% | 62.5414% | false |
| 2025 | -17.7670% | 33.2917% | -17.7670% | false |
| 2026_ytd | 40.8566% | 20.2510% | 15.2255% | false |

This is 4/5 positive segments, but segment DD control is weak because 2024 and 2025 both exceed 33% DD.

Engineering verification after documentation edits:

```bash
cargo test -p backtest-engine
```

Result: passed. The run reported 182 library tests, 3 `market_data_probe` tests, and 23 `search_scoring_time_splits` tests passing. Existing Round 7 warnings remain in `apps/backtest-engine/src/martingale/kline_engine.rs`.

```bash
cargo test -p trading-engine
```

Result: sandbox run failed only because `execution_effects_emit_telegram_logs_when_bound_and_configured` could not bind a local test server (`Operation not permitted`). The same command was rerun outside the sandbox for that reason and passed all trading-engine unit and integration tests.

## Round 1-6 Carryover

The earlier audit `docs/superpowers/reports/2026-07-03-glm-round1-6-execution-audit.md` remains valid: Rounds 1-6 were substantial but not strict 100% plan execution. The largest unresolved carryovers are still:

- R5/R6 live parity for promoted backtest features.
- R6 safety-order freeze engine gap.
- R6 dynamic blend not fully implemented as a reproducible engine feature.
- Some earlier artifact path mismatches and registry count mismatches.

Round 7 repaired some evidence gaps but did not close live parity or original targets.

## Validity Verdict

The seven-round result set is usable for continued research, but no result meets the user's required production target. The next round must not chase wider blind sweeps first. It must:

1. Make R5-R7 feature behavior reproducible in live runtime or mark candidates as research-only.
2. Attack the 2025 negative segment and 2024/2025 segment-DD weakness with martingale-native controls.
3. Keep every exploration in the registry with a non-repeat key.
4. Validate small-budget execution under 5000U with explicit cap/truncation/live-order parity.
