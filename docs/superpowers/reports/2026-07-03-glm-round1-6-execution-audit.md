# GLM Martingale Round 1-6 Execution Audit

**Date:** 2026-07-03
**Branch audited:** `glm-martingale-core-round6`
**Scope:** Round 1 through Round 6 plans, handoffs, search ledgers, artifact registries, result artifacts, and recent commits.

## Audit Standard

A task is counted as strictly complete only when the plan-required implementation, run, artifact path, ledger entry, and handoff evidence all exist. If GLM documented a blocker, superseded a task with evidence, or completed it in a later round, this audit records that separately instead of treating it as the original task being 100% executed.

## Executive Finding

GLM did substantial work and did not merely skim the plans. However, the first six rounds were **not 100% strict plan execution**. The main pattern is:

1. Round 1-2 produced valid martingale frontier improvements and mostly complete ledgers, but some planned index/final evidence files are missing or renamed.
2. Round 3-4 had several deferred/blocked branches; some were later repaired in Round 4, but not all planned branches were fully run.
3. Round 5 was mostly complete and found the first `ann > 50%` martingale candidate, but live parity remained backtest-only for new features and an ANKR low-DD script was committed without a result.
4. Round 6 was **partially executed** against the Round 6 plan: B was fixed after an initial flawed implementation; C and D were narrowed mostly to XRP; E was never implemented beyond config parsing; F dynamic blend was not run; G trading-engine parity was documented but not implemented.

The current true frontier remains a tradeoff:

| Round | Candidate | Full ann | Full DD | Pos segments | Status |
|---|---:|---:|---:|---:|---|
| R1 | `glm-mart-core-aggressive-008-best` | 22.2% | 26.1% | 3/5 | stable but too low ann |
| R2 | `r2-H-best-cd12h` | 28.6% | 22.5% | 4/5 | improved cycle discipline |
| R3 | `r3-P1-best-cd11` | 34.5% | 17.8% | 4/5 | best low-DD balanced shape |
| R4 | `r4-combo-best` | 34.7% | 17.7% | 4/5 | marginal 2025 improvement |
| R5 | `r5-G-best-ANKRUSDT` | 59.5% | 32.1% | 4/5 | first ann >50, DD too high |
| R5 | `r5-fine-combo-best` | 49.93% | 26.3% | 4/5 | near conservative ann, DD too high |
| R6 | `q1w24p24` XRP quarantine | 55.8% | 25.0% | 4/5 | ann >50, DD still too high |
| R6 | `xrpq20_r460` blend | 34.0% | 18.0% | 4/5 | DD good, ann too low |

No candidate meets the original targets:

| Target | Requirement | Current best relation |
|---|---|---|
| Conservative | ann >50%, DD <=10% | ann can exceed 50 only around DD 25-32%; DD <=10 produced negative/low ann in earlier sweeps |
| Balanced | ann >90%, DD <=20% | DD <=20 exists around ann 34-35%, far below 90 |
| Aggressive | ann >110%, DD <=30% | ann >50 exists but DD remains above 25; no ann >110 stable candidate |

## Round-by-Round Compliance

### Round 1: Indicator Expansion / Martingale Core Reopen

Plan: `docs/superpowers/plans/2026-07-01-glm-martingale-core-indicator-expansion-plan.md`
Ledger: `docs/superpowers/reports/2026-07-01-glm-martingale-core-search-ledger.md`
Handoff: `docs/superpowers/reports/2026-07-02-glm-handoff-to-chatgpt.md`

| Planned task | Evidence | Audit result |
|---|---|---|
| Task 1 ledger and evidence index | Ledger exists; `evidence-index.json` not found under `docs/superpowers/artifacts/glm-martingale-core/` | **Partial** |
| Task 2 regime-gated single strategy | `regime-gated-single-strategy.json`; 3276 candidates; 0 survivors | Complete |
| Task 3 ATR/ADX adaptive martingale | Evidence exists as `highann-dd-opt.json`, `highbudget-search.json`, and ledger sections; planned `atr-adx-single-strategy.json` not found | **Partial path mismatch** |
| Task 4 active-cycle / portfolio DD stop | Portfolio DD stop was implemented and later budget-base bug fixed; ledgers and commits exist | Complete after repair |
| Task 5 dynamic multi-symbol portfolio | `portfolio-segments.json`, `portfolio-optimized.json`, dynamic replay folder, final candidate artifacts | Complete |
| Task 6 final three-tier package | final conservative/balanced/aggressive JSONs exist, but no original target pass | Complete as frontier package |

Important Round 1 findings:

- `008-best`: ann 22.2%, DD 26.1%, 3/5 positive segments.
- High-ann structures around 73.5% required DD around 45% and poor segment stability.
- Tight SL alone collapsed ann, confirming the ann/DD cliff for that family.
- Portfolio equity stop bug was found and repaired; budget-based DD became the correct basis.

Carry to Round 7:

- Do not repeat plain tight-SL-only sweeps, simple HTF filters, simple fixed/ATR/custom spacing sweeps, or static DD-stop sweeps without a new mechanism.
- If Round 1 artifacts are reused, record the missing `evidence-index.json` gap in the merged registry.

### Round 2: Exhaustive Search

Plan: `docs/superpowers/plans/2026-07-02-glm-martingale-core-round2-exhaustive-search-plan.md`
Ledger: `docs/superpowers/reports/2026-07-02-glm-martingale-round2-search-ledger.md`
Full handoff: `docs/superpowers/reports/2026-07-02-glm-round2-full-handoff.md`
Registry: `docs/superpowers/artifacts/glm-martingale-core-round2/exploration-registry.jsonl`

| Direction | Evidence | Audit result |
|---|---|---|
| A Partial TP + Breakeven | Engine implementation; 162 candidates; `r2-A-full-grid.json`; promising config | Complete |
| B Conditional Safety Orders | Engine implementation; 15 candidates; `r2-B-full-grid.json`; promising config | Complete |
| C Bounded DGT spacing | 5 full-period runs; rejected | Complete enough, but no separate `r2-C-full-grid.json` artifact |
| D Sentiment gate | Blocked due no OI/long-short/taker historical data | Justified blocker |
| E Inventory skew | 7 candidates; `r2-E-full-grid.json`; no effect | Complete |
| F Recovery re-entry | 49 candidates; `r2-F-full-grid.json`; no improvement | Complete |
| G Symbol health / diversification | 6 symbol sets; `r2-G-full-grid.json`; no improvement | Complete |
| H Time/cooldown gate | Result and promising config exist; no full-grid artifact path found | **Partial artifact gap** |
| Live parity | Handoff says Partial TP/Cond SO were backtest-only | **Incomplete for live promotion** |

Evidence inconsistency:

- Full handoff says "Registry: 8 JSONL lines"; actual file has 11 lines. This is not a strategy error, but it is a reporting accuracy issue.

Important Round 2 findings:

- `r2-H-best-cd12h`: ann 28.6%, DD 22.5%, 4/5 positive.
- Partial TP + BE + conditional SO + longer cooldown improved the frontier.
- Sentiment data remained unavailable locally.

Carry to Round 7:

- Do not repeat bounded DGT/fixed spacing around the same 150bps axis.
- Do not repeat inventory caps that never bind.
- Sentiment/OI/taker gates remain blocked unless a data ingestion task is explicitly approved.

### Round 3: Target Breakthrough

Plan: `docs/superpowers/plans/2026-07-02-glm-martingale-core-round3-target-breakthrough-plan.md`
Ledger: `docs/superpowers/reports/2026-07-02-glm-martingale-round3-search-ledger.md`
Handoff: `docs/superpowers/reports/2026-07-02-glm-round3-handoff-to-chatgpt.md`
Registry: `docs/superpowers/artifacts/glm-martingale-core-round3/exploration-registry.jsonl`

| Planned item | Evidence | Audit result |
|---|---|---|
| P0 live-parity audit | `2026-07-02-glm-round3-live-parity-audit.md`; all R2 features backtest-only | Complete audit, parity not implemented |
| P1 direction-aware SO | 352 candidates; `r3-dir-aware-so-grid.json`; new best | Complete |
| P2 rebound-confirmed SO | Deferred due engine feature need | **Incomplete in R3**, later implemented in R4 |
| P3 TP/cooldown fine search | Handoff reports 5040 stage1; no dedicated result artifact found | **Partial evidence** |
| P4 custom ladder | Only 5 candidates; fixed 150 best | Partial/narrow, but enough to reject this narrow branch |
| P5 rolling walk-forward selector | Deferred due no second competitive config | **Not run** |
| P6 breadth regime | 48 candidates; `r3-breadth-regime-grid.json`; no improvement | Complete |
| P7 premium/mark/index | Blocked due no local data | **Blocked**, later unblocked/analyzed in R4 |
| P8 core-satellite | 1 candidate; no improvement | Partial/narrow |

Evidence inconsistency:

- Handoff says "Registry: 8 JSONL lines"; actual file has 7 lines.

Important Round 3 findings:

- `r3-P1-best-cd11`: ann 34.5%, DD 17.8%, 4/5 positive, best risk-adjusted frontier at the time.
- Breadth proxies worsened 2025.
- R2 live parity was confirmed missing.

Carry to Round 7:

- Do not repeat BTC breadth proxy gates as previously defined.
- Do not repeat P5 unless at least two truly competitive sleeves exist and walk-forward selection uses only lagged data.

### Round 4: 2025 Breakthrough

Plan: `docs/superpowers/plans/2026-07-02-glm-martingale-core-round4-2025-breakthrough-plan.md`
Ledger: `docs/superpowers/reports/2026-07-02-glm-martingale-round4-search-ledger.md`
Handoff: `docs/superpowers/reports/2026-07-02-glm-round4-handoff-to-chatgpt.md`
Final report: `docs/superpowers/reports/2026-07-02-glm-4round-final-report.md`

| Planned item | Evidence | Audit result |
|---|---|---|
| P0 trading-engine parity | Initially partial; later commits implemented 4/4 Round 4 parity features; 187 tests reported | Complete after updates |
| P1 2025 attribution | `r4-cycle-attribution-2025.json`; report sections | Complete |
| P2 range sleeve | Rejected; 2025 volatility too high for range thresholds | Complete enough |
| P3 pump-fade short | Initially deferred; later engine ROC function + 72-candidate grid | Complete after update |
| P4 active-cycle exit | Initially deferred; later engine feature + 25-candidate grid | Complete after update |
| P5 rebound SO | Initially deferred; later engine feature + 6-candidate grid | Complete after update, narrow grid |
| P6 premium/index/mark data | Downloaded 176640 rows; signal analysis only | **Partial**: data unblocked, but no full gate integration grid |
| P7 four-stage partial TP | Tested; no improvement | Complete |
| P8 dynamic allocator | Deferred due no competitive configs | Justified skip |
| Final combo | 144 candidates; `r4-combo-breakthrough-grid.json`; `r4-combo-best` | Complete |

Important Round 4 findings:

- `r4-combo-best`: ann 34.7%, DD 17.7%, 4/5 positive.
- Pump-fade and premium were only marginal signals.
- 2025 remained negative; best 2025 improved only to about -8.7%.

Carry to Round 7:

- Do not repeat active-cycle stale-exit, rebound-SO, or four-stage TP alone.
- Premium/funding can be reopened only as a **cost/drag gate** or **entry quality veto**, not as a standalone return sleeve.

### Round 5: Frontier Expansion

Plan: `docs/superpowers/plans/2026-07-03-glm-martingale-core-round5-frontier-expansion-plan.md`
Ledger: `docs/superpowers/reports/2026-07-03-glm-martingale-round5-search-ledger.md`
Handoff: `docs/superpowers/reports/2026-07-03-glm-round5-handoff-to-chatgpt.md`
Parity report: `docs/superpowers/reports/2026-07-03-glm-round5-live-parity-hardening.md`

| Task | Evidence | Audit result |
|---|---|---|
| A micro-regime attribution | Completed; `r5-cycle-microregime.json` | Complete |
| B last-executed SO basis | Engine + 400 grid; `r5-last-executed-so-grid.json` | Complete |
| C loss-streak risk reduction | Engine + 78 grid; no improvement | Complete |
| D vol-targeted martingale | Engine + 46 grid; marginal improvement | Complete |
| E custom ladder dip/breakout | Initially rejected as repeat; later 324 full grid; all rejected | Complete after update |
| F same-symbol hedged grid | 400 candidates; all rejected | Complete |
| G expanded universe | 148 candidates; ANKR/XRP/DOGE improved ann | Complete |
| H live-parity hardening | Documented only; R5 new features remain backtest-only | **Incomplete for live promotion** |
| I dynamic allocator | Marked not needed because ANKR dominated BCH | Justified for BCH/ANKR, but not a general DD allocator |
| Extra ANKR low-DD script | `scripts/glm_r5_ankr_lowdd_search.py` committed after final handoff; no Round 5 result artifact | **Open evidence gap**, carried to R6 H |

Important Round 5 findings:

- `r5-G-best-ANKRUSDT`: ann 59.5%, DD 32.1%, 4/5 positive. This is the first true ann >50 martingale candidate, but DD misses every target.
- `r5-fine-combo-best`: ann 49.93%, DD 26.3%, 4/5 positive.
- R5 features last-exec SO, risk reduction, and vol-target remained backtest-only.

Carry to Round 7:

- Do not repeat same-symbol hedged grid.
- Do not repeat simple custom ladder dip/breakout grids.
- Reopen ANKR/XRP/DOGE only through cost-aware, DD-aware, or attribution-driven mechanisms, not blind symbol replacement.

### Round 6: DD Compression

Plan: `docs/superpowers/plans/2026-07-03-glm-martingale-core-round6-dd-compression-plan.md`
Ledger: `docs/superpowers/reports/2026-07-03-glm-martingale-round6-search-ledger.md`
Handoff: `docs/superpowers/reports/2026-07-03-glm-round6-handoff-to-chatgpt.md`
Attribution report: `docs/superpowers/reports/2026-07-03-glm-round6-drawdown-window-attribution.md`

| Task | Plan requirement | Evidence | Audit result |
|---|---|---|---|
| A drawdown-window attribution | Peak/trough/recovery, symbol/direction contribution, leg state, blocked safety count, TP stage, indicators, route | Artifact contains only `on_budget`, `equity_curve_count`, `trade_count`, `stop_count`; report has segment summary | **Incomplete detail** |
| B portfolio DD state machine | Implement + tests + search | Initial 14-candidate run had no effect due wrong DD basis/scale; later fixed with 34 candidates | Complete after repair, result marginal |
| C symbol/direction quarantine | Scope grid over symbol/symbol_direction/direction, ANKR/XRP/DOGE/fine candidates | Implemented and tested mostly on XRP; 28 candidates | **Partial/narrow** |
| D partial-TP trailing lock | ANKR/XRP/DOGE/fine grid | Initial config-only; later full logic on XRP with 12 configs | **Partial/narrow** |
| E safety-order freeze/cancel | Implement engine logic + tests + search | Config parses, engine does not check it; no real effect | **Incomplete** |
| F ANKR/XRP/DOGE/R4/cash blend | Static and dynamic if promising | Static 31 candidates; dynamic not found; report path absent | **Partial** |
| G trading-engine parity for R5/R6 | Implement mandatory R5 parity and report | Documented only; R5/R6 remain backtest-only | **Incomplete** |
| H run or supersede ANKR low-DD script | Run script or create closeout artifact | Marked superseded by F; no `r6-ankr-lowdd-closeout.json` found | **Partial closeout** |

Evidence inconsistency:

- Handoff says "Registry: 8 JSONL lines"; actual Round 6 registry has 13 lines.

Important Round 6 findings:

- R6 found funding drag of 677 USDT on 5000U budget, about 13.5% of starting budget.
- Static blends reduced DD to 18-19%, but ann fell to about 34-35%.
- XRP quarantine improved ann to 55.8% with DD 25.0%, but did not reach DD <=10/20.
- DD state machine scaling cannot fix DD from existing unrealized PnL.
- Trailing lock on XRP increased DD.

Carry to Round 7:

- First repair Round 6 gaps before new wide search:
  - Detailed attribution must be rebuilt.
  - Safety-order freeze/cancel must get real engine logic.
  - Quarantine and trailing lock must only be rerun outside XRP if attribution justifies it.
  - Dynamic blend must be run or rejected with objective threshold evidence.
  - R5/R6 trading-engine parity remains a blocker for live-ready promotion.

## Consolidated Do-Not-Repeat Matrix

Do not spend Round 7 compute on these unless a task explicitly changes the mechanism:

| Non-repeat family | Why |
|---|---|
| Tight SL only | DD improves but ann collapses before mean reversion |
| Static portfolio DD stop only | Correctly caps DD but kills recovery return |
| Fixed/ATR/custom spacing rescan | Fixed 150bps remains best; prior variants negative or near-zero |
| Bounded DGT reset without martingale-native trigger | Already rejected in R2/R1 style spacing tests |
| BTC breadth proxy | Worsened 2025 |
| HTF EMA/RSI/BB stricter filters | Too few trades or worse segment stability |
| Active stale-cycle exit alone | Lower DD but much lower ann; stale cycles are not root cause |
| Rebound-confirmed SO alone | Delays entries, reduces ann, no 2025 fix |
| Four-stage TP alone | Worse DD/ann than three-stage |
| Same-symbol hedged grid | Cancels return; best only 0-4.5% ann |
| Static cash/R4 blend only | DD compresses but ann falls to ~34% |
| Funding carry as standalone yield | Too small to meet targets and not martingale core |
| Simple symbol swap after ANKR/XRP/DOGE | Only reopen via anti-survivor universe protocol and attribution |
| XRP-only trailing lock | DD increased from 25.4% to 30.0% |
| DD state machine scaling-only | Cannot compress DD from existing positions |

## Round 7 Required Repairs Before New Claims

1. Build a merged `round1-6-non-repeat-registry.json` from all ledgers and registries.
2. Re-run Round 6 Task A as detailed attribution, not just segment summary.
3. Implement real Round 6 Task E safety-freeze/cancel logic or formally reject it after test evidence.
4. Run or explicitly reject dynamic cost/DD blend with a saved artifact.
5. Do not mark any R5/R6/R7 candidate live-ready until trading-engine parity exists or the report says `backtest_only`.

## External Signals Used For Round 7

- 3Commas DCA docs show DCA bots are configured around base/safety orders, take-profit from average price/base order, multiple TP, and trailing TP. This supports keeping new mechanisms inside DCA-cycle entry/add/exit rules, not replacing martingale with a new strategy.
- Binance funding documentation confirms funding is a periodic transfer between long and short perpetual holders and the rate depends on futures/spot premium. Round 7 should treat funding as a cost gate and side-selection input, not as a separate yield sleeve.
- The 2025 Dynamic Grid Trading paper argues traditional grid has near-zero expectation under simple assumptions and improves only when grid behavior adapts to market conditions. Round 7 should test dynamic grid ideas only as martingale-native reset/eligibility rules with anti-overfit validation.

Sources:

- 3Commas DCA settings and TP/trailing behavior: https://help.3commas.io/en/articles/3108940-dca-bot-interface-and-main-settings
- 3Commas trailing take profit: https://help.3commas.io/en/articles/3108981-how-take-profit-works-smarttrade-and-dca-bots-trailing-feature-explained
- Binance funding rates / funding fee: https://www.binance.com/en/support/faq/detail/360033525031 and https://www.binance.com/en/academy/glossary/funding-fees
- Dynamic Grid Trading paper: https://arxiv.org/abs/2506.11921
