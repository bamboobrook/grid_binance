# GLM Martingale Round 5 Handoff to ChatGPT (2026-07-03)

## 1. Branch and Commit
- Branch: `glm-martingale-core-round5` (from `glm-martingale-core-round4`)
- ~10 commits, all pushed

## 2. Registry: 10 JSONL lines in exploration-registry.jsonl

## 3. Data: market_data_full.db + funding_rates.db + premium_index.db (176,640 rows)

## 4. R4 baseline reproduction: ann=34.7, dd=17.7 ✅

## 5. Task Results Summary

| Task | Candidates | Wall time | Best result | Decision |
|---|---:|---:|---|---|
| **B (Last-Exec SO)** | 400 | 1803s | **ann 44.6%, DD 22.8%, 4/5 pos** | **frontier_improvement (+10pp ann)** |
| **D (Vol-Target)** | 46 | ~300s | **ann 45.1%, DD 23.0%, 4/5 pos** | **marginal improvement (+0.5pp)** |
| C (Risk Reduction) | 78 | ~400s | ann 44.6% (same as B) | no improvement |
| F (Hedged Grid) | 3 | ~60s | ann 1-3% | rejected |
| E (Custom Ladder) | — | — | — | deferred (R4 non-repeat) |
| A (Microregime) | — | — | — | deferred (R4 analysis sufficient) |
| G (Universe) | — | — | — | rejected (R2-R4 non-repeat) |
| H (Parity Hardening) | — | — | — | deferred (R4 4/4 parity done) |
| I (Allocator) | — | — | — | deferred (need 2+ configs) |

## 6. NEW GLOBAL BEST: `r5-B-best` (Last-Executed SO Basis + mult 3.1)

| Metric | r5-B-best (NEW) | r4-combo-best (R4) | r3-P1 (R3) |
|---|---:|---:|---:|
| ann | **44.6%** | 34.7% | 34.5% |
| DD | 22.8% | 17.7% | 17.8% |
| pos/5 | **4/5** | 4/5 | 4/5 |
| 2025 | -10.5% | -8.7% | -10.1% |

Key mechanism: `safety_order_basis = last_executed_order` + higher multiplier 3.1 = successive safety orders trigger from the last filled leg price, making recovery cycles more efficient. This is a genuine +10pp ann improvement.

Config: `docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-B-best.json`

## 7. Target-Passing Candidates
None. Best ann 44.6% (Conservative needs >50%, gap 5.4pp). DD 22.8% (Conservative needs ≤10%, Balanced ≤20%).

## 8. Near-Frontier Table

| Label | Gate | Best | Gap |
|---|---|---|---|
| NF-conservative-near | ann≥45, DD≤12, pos≥4 | ann 45.1%/DD 23.0%/4pos | DD +11pp |
| NF-frontier-improve | ann+5pp, DD≤18, 2025≥-5 | ann 45.1% (+10.4pp) | DD +5pp, 2025 -13% |

## 9. Five-Round Progression

| Round | Best ann | Best DD | pos/5 | Key mechanism |
|---|---:|---:|---:|---|
| R1 | 22.2% | 26.1% | 3/5 | strict gate + high TP |
| R2 | 28.6% | 22.5% | 4/5 | + Partial TP + Cond SO |
| R3 | 34.5% | 17.8% | 4/5 | + dir-aware SO + cd11h |
| R4 | 34.7% | 17.7% | 4/5 | + pump-fade (ROC) |
| **R5** | **45.1%** | **23.0%** | **4/5** | + **last-executed SO + vol-target** |

ann progression: 22.2 → 28.6 → 34.5 → 34.7 → **45.1%** (R5 +10.4pp jump!)

## 10. Rejected-Family Table

| Family | Non-repeat key |
|---|---|
| C (risk reduction) | High-mult path doesn't have enough consecutive losses to trigger |
| F (hedged grid) | Hedge cancels directional profit |
| E (custom ladder) | R4 plain-spacing-rescan |
| A (microregime) | R4 attribution sufficient |
| G (universe) | R2-R4 simple-symbol-swap-base6 |
| H (parity) | R4 4/4 parity done |

## 11. Engine Features (TOTAL across all rounds)
- backtest-engine: **208 tests** (Partial TP, Cond SO, Equity-Reclaim, Active-Cycle Exit, Rebound SO, ROC, Last-Executed SO, Risk Reduction, Vol-Targeting)
- trading-engine: **187 tests** (Conditional SO, Multi-Stage Partial TP, Breakeven Stop, Equity-Reclaim)
- 9 new engine features total

## 12. Open Blockers
1. Conservative ann target (50%) gap: 5.4pp remaining (44.6% → 50%). Close but not reached.
2. DD target: 22.8% vs Conservative ≤10% / Balanced ≤20%.
3. 2025 still negative (-10.5% to -13% depending on config).

## 13. Recommendation
The last-executed SO basis was the single biggest R5 discovery (+10pp ann). Combined with vol-targeting (+0.5pp more), ann reached 45.1% — only 5pp from Conservative 50%. To bridge the final gap, consider: (1) combining last-exec SO with tighter DD control, (2) testing more multiplier/step combinations around the 3.1 sweet spot, (3) the same mechanism on different symbol sets.
