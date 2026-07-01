# 2026-07-01 Final Martingale Verdict and External Check

> Objective checked: `<5000U`, multi-symbol, anti-overfit, balanced H1-2023/H2-2023/2024/2025/2026_ytd performance, conservative `ann >50% / DD<=10%`, balanced `ann >90% / DD<=20%`, aggressive `ann >110% / DD<=30%`, and eventually live-reproducible.
>
> Safety: this report is offline/research-only. It does not touch Binance, flyingkid, live mode, or real funds.

## Verdict

No qualifying martingale/grid portfolio has been found.

Current evidence is strong enough to reject the available martingale/grid paths in this repo under the original gates. It is not a mathematical proof that no strategy in the universe can ever satisfy the gates; it is a current-state engineering verdict: the searched pure martingale, regime-filtered martingale, hybrid martingale+trend/funding, DGT reset variants, pair-neutral spread grids, and risk-controlled pair-neutral grids all fail at least one mandatory gate.

The recurring failure pattern is stable:

- High annualized return comes with too much drawdown, too much capital, or 2023H1 dependence.
- Low drawdown and balanced segments produce annualized returns far below 50/90/110%.
- Full-period aggressive candidates can look acceptable, but segment robustness rejects them.
- `research_only` DGT can create very high offline returns, but only by accepting capital/DD/segment failures.
- Pair-neutral spread grids reduce beta and improve segment balance, but the best risk-controlled frontier still cannot combine the required annualized return and DD limits.

## Requirement Matrix

| Requirement | Evidence status | Current finding |
|---|---|---|
| `<5000U` small capital | Tested by replay budget gates, DGT max-input gates, trend/funding allocation, pair-neutral allocation | Some candidates stay below budget; high-return candidates often need far more capital or accept DD above the gate. |
| Multi-symbol | Enforced in DGT, trend sleeves, pair-neutral grid pairs, and saved replay rows where available | Multi-symbol alone does not rescue return/DD/segment gates. |
| Anti-overfit / segment balance | H1-2023, H2-2023, 2024, 2025, 2026_ytd checks | Still a rejection gate for many high-return candidates. Pair-neutral grids can reach `5/5` positive segments, but then miss return/DD targets. |
| Conservative `>50% / DD<=10%` | Pure/replay, hybrid, DGT, pair-neutral, pair-neutral risk control, pair-neutral portfolio | No pass. Under segment/capital filters, the best ann within DD<=10% is `33.95%`; reaching ann >50% requires about `17.74%` DD. |
| Balanced `>90% / DD<=20%` | Pure/replay, hybrid, DGT, pair-neutral, pair-neutral risk control, pair-neutral portfolio | No pass. Under segment/capital filters, DD<=20% tops out at `54.41%` ann; reaching ann >90% requires about `40.70%` DD. |
| Aggressive `>110% / DD<=30%` | Pure/replay, hybrid, DGT, pair-neutral, pair-neutral risk control, pair-neutral portfolio | No pass. Under segment/capital filters, DD<=30% tops out at `54.41%` ann; reaching ann >110% requires about `40.70%` DD. |
| Live reproducibility | Existing martingale replay has live-parity gates; DGT/pair-neutral probes are explicitly `research_only` | No final candidate reaches the stage where live promotion is justified. |

## Internal Evidence

### GLM Phase A

GLM's handoff (`docs/superpowers/plans/2026-06-30-glm-phaseA-handoff-for-chatgpt.md` in `worktree-p4-cycle-exit`) reports about 1500 candidates and 590 segment validations using `portfolio_budget_replay` over `market_data_full.db`.

Key findings:

- Large-cap regime MR: conservative best `1.5% ann / 9.0% DD`, balanced best `4.2% ann / 11.4% DD`.
- Broad alt pool + wide SL + portfolio stop: conservative `3.5% / 5.8%`, balanced `9.3% / 13.7%`, aggressive `14.5% / 22.7%`.
- Per-coin regime allocator: aggressive reached `21.2% ann / 41.7% DD`, with only `3/5` positive segments.
- Cross-experiment mining found only 2 configs with `positive_segments >= 4` and positive 2024-2026, both near `0.8% ann`.
- `2024 >= 0` and `2025 >= 0` simultaneously: `0/590`.

This supports the structural diagnosis: the parameter sets that work in 2024 and 2025 are different and often conflict.

### ChatGPT Phase A Audit

`docs/superpowers/reports/2026-06-30-chatgpt-phaseA-audit-and-external-check.md` reran representative candidates and scanned prior pools.

Representative failures:

- Conservative 5000U: `33.56% ann / 10.67% DD / blocked 75`; segment gate fails, only `2/5` positive segments.
- Balanced 5000U: `99.42% ann / 24.27% DD / blocked 68`; segment gate fails, only `2/5` positive segments, 2024-2026 `-29.4%`.
- Aggressive 4000U: full-period gate true at `120.59% ann / 28.97% DD / blocked 0`, but segment gate fails; H2-2023 DD `48.2%`, 2025 DD `51.3%`, 2026_ytd DD `39.3%`, only `1/5` positive segment, 2024-2026 `-62.8%`.

Eight aggressive full-pass-like rows were rerun through segment robustness. All failed.

### Hybrid Martingale + Trend/Funding

`docs/superpowers/reports/2026-06-30-hybrid-frontier-wide-search.md` and `docs/superpowers/reports/2026-06-30-hybrid-frontier-trend-rules-search.md` expanded research-only trend and funding sleeves.

Best trend-rule search results:

- Conservative: `13.54% ann / 6.32% DD / cap 1343.89`, `0` passes.
- Balanced: `25.31% ann / 33.07% DD / cap 1144.32`, `0` passes.
- Aggressive: `39.37% ann / 36.78% DD / cap 1233.73`, `0` passes.

Funding helps as a low-correlation sleeve but is too small to bridge a 50/90/110% annualized-return target.

### DGT Dynamic Grid Probe

`docs/superpowers/reports/2026-07-01-dgt-dynamic-grid-probe.md` tested dynamic grid reset mechanics with full 1m accounting and daily first/high/low/close equity compression before union/carry-forward portfolio combination.

Search results:

- Smoke: `1080` rows, `0` offline passes.
- Group size 2: `5000` rows, `0` offline passes.
- Group size 3: `5000` rows, `0` offline passes.

Best DGT examples:

- Group size 2 best annualized overall: `SOLUSDT,XRPUSDT`, `698.09% ann / 69.12% DD / 79693.65U max_input`; fails DD, budget, and segment balance.
- Group size 2 best annualized under 5000U: `BTCUSDT,BNBUSDT`, `238.17% ann / 48.76% DD / 4549.56U max_input`; fails DD for every profile.
- Group size 3 best annualized under 5000U: `BTCUSDT,SOLUSDT,XRPUSDT`, `198.12% ann / 63.24% DD / 4868.18U max_input`; fails DD and segment balance.
- Lowest-DD under-budget group size 3: `13.05% ann / 0.00% DD / 150U max_input`; fails annualized-return and segment gates.

DGT therefore confirms the same frontier: return can be manufactured, but not with the required DD, budget, and robustness constraints.

### Pair-Neutral Grid And Risk Control

`docs/superpowers/reports/2026-07-01-pair-neutral-grid-probe.md` tested pair-neutral spread grids over 3024 research-only rows. It found `0` passes. The strongest pair-neutral row was `BNBUSDT,SOLUSDT` with `54.41% ann / 23.60% DD / 1000U cap / 5/5 positive segments`, which is useful evidence but still fails conservative DD, balanced return/DD, and aggressive return gates.

`docs/superpowers/reports/2026-07-01-pair-neutral-risk-control-probe.md` added DD stop/cooldown variants over 27216 rows. It also found `0` passes. The best risk-controlled balanced-style frontier was `BNBUSDT,SOLUSDT + dd10_cd60`, with `54.41% ann / 17.74% DD / 1000U cap / 5/5 positive segments`, still below the balanced `>90%` target. The conservative version still requires more than 10% DD to exceed 50% ann.

`docs/superpowers/reports/2026-07-01-pair-neutral-portfolio-probe.md` tested strict non-overlapping multi-pair portfolios over 93 research-only rows. It also found `0` passes. Diversification lowered drawdown: the nearest conservative-style row used `BNBUSDT,SOLUSDT`, `BTCUSDT,LINKUSDT`, `ETHUSDT,XRPUSDT`, and `ADAUSDT,DOGEUSDT` with `33.95% ann / 7.62% DD / 3000U cap / 5/5 positive segments`, but the return still missed the conservative `>50%` target. The highest-return multi-pair row reached `42.66% ann / 20.27% DD`, still below every final return target.

Portfolio-level leverage does not fix that gap under an optimistic linear check: scaling the `33.95% / 7.62%` row to `50%` ann implies about `11.23%` DD, already above the conservative gate; scaling toward balanced/aggressive either exceeds DD or pushes capital above `5000U`.

### Target Gap And Completion Audits

`docs/superpowers/reports/2026-07-01-martingale-target-gap-audit.md` normalized 64508 saved research rows across trend, trend risk control, pair-neutral, pair-neutral risk control, pair-neutral portfolio, funding, and saved result leak artifacts. It reports `0` final target passes.

The current frontier under capital and segment filters is:

- Conservative: DD<=10% tops out at `33.95%` ann; ann >50% requires about `17.74%` DD.
- Balanced: DD<=20% tops out at `54.41%` ann; ann >90% requires about `40.70%` DD.
- Aggressive: DD<=30% tops out at `54.41%` ann; ann >110% requires about `40.70%` DD.

`docs/superpowers/reports/2026-07-01-martingale-frontier-evidence-audit.md` indexes 12 reports and 130036 rows/symbol-level evidence with `0` machine-reported final/pass rows. `docs/superpowers/reports/2026-07-01-martingale-goal-completion-audit.md` marks the original objective as incomplete because all C/B/A final gates, external-claim proof, and live-ready proof fail.

## External Check

External references do not provide a missing free lunch; they reinforce the implementation constraints and overfit risk discipline.

- Binance USD-M `exchangeInfo` is the required source for per-symbol filters such as quantity, price tick, step size, and notional constraints. A final live candidate cannot rely only on a single local `5U` minimum-notional approximation.
- Binance USD-M order APIs and Algo Order APIs matter for TP/SL/stop behavior. `POST /fapi/v1/algoOrder` is the documented path for USD-M Futures TP/SL and trailing stop conditional orders, so live parity must prove whether exits use current algo-order endpoints or engine-side reduce-only exits.
- Binance's derivatives change log says USD-M Futures conditional orders migrated to Algo Service effective 2025-12-09 for `STOP_MARKET`, `TAKE_PROFIT_MARKET`, `STOP`, `TAKE_PROFIT`, and `TRAILING_STOP_MARKET`. Old condition-order assumptions are not enough for a live-ready design.
- Deflated Sharpe Ratio and Probability of Backtest Overfitting literature both match the observed failure mode: many trials plus full-period winner selection tends to select 2023H1-like overfit candidates.
- Dynamic-grid literature, including the recent DGT direction, treats static/traditional grid payoff as weak or near-zero before market adaptation. The local DGT probe tested one adaptive reset family and still failed the combined gates.
- A 2026-07-01 web refresh checked current Pionex, Phemex, OKX, 3Commas, Bitsgap, and Neutralis/DGT-style public material. It found no externally published martingale/grid/DGT portfolio with trade-level evidence for `<5000U`, multi-symbol exposure, the required 2023-2026 segments, DD gates, and current Binance-live reproducibility.

Sources checked:

- Binance USD-M Futures exchange information: https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Exchange-Information
- Binance USD-M Futures new order / algo order documentation: https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api
- Binance derivatives change log: https://developers.binance.com/docs/derivatives/change-log
- Deflated Sharpe Ratio: https://papers.ssrn.com/sol3/papers.cfm?abstract_id=2460551
- Probability of Backtest Overfitting: https://papers.ssrn.com/sol3/papers.cfm?abstract_id=2326253
- Dynamic Grid Trading Strategy: https://arxiv.org/abs/2506.11921
- External martingale/grid claim matrix: docs/superpowers/reports/2026-07-01-external-martingale-grid-claim-gate-matrix.md

## What This Does And Does Not Prove

Proves for current repo evidence:

- No discovered pure martingale/grid/live-parity candidate satisfies all original C/B/A gates.
- No discovered hybrid martingale+trend/funding research candidate satisfies the gates.
- No discovered DGT research candidate satisfies the gates.
- No discovered pair-neutral or risk-controlled pair-neutral grid candidate satisfies the gates.
- The dominant blocker is structural, not a single missed parameter: return/DD/budget/segment requirements conflict.

Does not prove:

- All possible non-martingale strategies are impossible.
- A future live-ready trend-following or stat-arb engine could not satisfy some target.
- External venues, options, basis trades, or non-Binance instruments cannot produce different frontiers.

## Remaining Plausible Paths

Only paths that still make engineering sense:

1. **Change the strategy class**: build a live-parity trend/breakout or stat-arb sleeve first as a standalone edge, then combine with low-risk MR. This is no longer "martingale strategy finds the target"; it is a broader portfolio strategy.
2. **Funding/basis as auxiliary yield**: use it for smoothing or small return contribution, not as the main source for 50/90/110%.
3. **Relax the target frontier**: if the mandate remains martingale/grid/live-parity, a realistic target is closer to `10-20% ann` depending on DD, not 50/90/110%.
4. **Freeze the martingale/grid search space**: the current indexed evidence is broad enough to stop repeating martingale/grid variants unless a genuinely new mechanism or relaxed target is introduced.

## Operational Safety

- No live/Binance/flyingkid/real-funds action was taken.
- DGT and pair-neutral probes remain `research_only`.
- No candidate should be promoted to live. Promotion requires a separate design, live-parity implementation, and explicit user approval.

## Final Decision

Do not launch a martingale/grid portfolio under the original gates.

The honest answer to the original request is: no qualifying martingale strategy combination has been found. The best next work is either to change the strategy class or to lower the targets to the empirically observed frontier.
