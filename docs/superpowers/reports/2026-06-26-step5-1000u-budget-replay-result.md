# Step 5 Result: Conservative 1000U Budget-Aware Replay — NO-GO (DD)

Date: 2026-06-26
Plan: `docs/superpowers/plans/2026-06-26-glm-margin-budget-preflight-fix-and-live-runbook.md` (Step 5)
Portfolio: `mp_margin_v2_lp_conservative_20260626` (16 strategies, 8 symbols, long_and_short)
Tool: new binary `apps/backtest-engine/src/bin/portfolio_budget_replay.rs` (joint kline sim + on-budget rebase)
Data: `data/market_data_full.db` (107G), `data/funding_rates.db`; range 2023-01-01 → 2026-05-31 (1247 days, 1m); 14,365,440 bars.

## Gate (plan Step 5): annualized > 50% AND max DD <= 10% under 1000U → **FAIL**

| Budget | Annualized (on budget) | Max DD (on budget) | Total return | Max capital used | Budget-blocked legs | Gate |
|---|---|---|---|---|---|---|
| **1000U** | **117.95%** ✅ | **46.90%** ❌ | 1332% | 999.8U | 141 | **FAIL** |
| 2000U | 125.48% | 45.15% | 1508% | 1999.8U | 166 | FAIL |
| 5000U | 115.50% | 50.11% | 1278% | 5000.0U | 82 | FAIL |
| 10000U | 59.02% | 58.92% | 388% | 9999.1U | 30 | FAIL |

Other (1000U): trade_count=66262, stop_count=32007, total_fee=1579U, total_funding=-1375U, total_slippage=702U.

## Why the gate fails — and why raising the budget does not help

**The displayed "conservative DD≤10%" was an artifact of the denominator.** The backtest engine's stock return/DD denominator is the **uncapped planned margin capital** = Σ all leg margins across the 16 strategies = **144,436 USDT** (a number that has no relation to a 1000U live budget). On that base the joint sim's own DD is **2.55%** (even lower than the optimizer's reported 10%). The SAME drawdown (~3,866 USDT, peak-to-trough, mark-to-market) is:
- 2.55% of the 144,436U planned-margin base (the displayed number), but
- **46.9% of real 1000U capital** (the number that actually matters live).

**DD is the strategy's inherent risk profile, not a function of budget.** The sensitivity table shows max DD stays ~45–59% across 1000U→10000U. Martingale averaging-down has an inherent peak-to-trough drawdown of roughly half its deployed capital; scaling the budget scales the position PnL proportionally, so the DD percentage is approximately scale-invariant. At 10000U the annualized collapses to 59% (capital deployed but returns don't scale up) while DD stays ~59%.

**Equity model is mark-to-market** (kline_engine.rs:428 includes `unrealized_pnl`; tests `total_return_includes_final_unrealized_loss` + `multi_symbol_equity_keeps_other_symbol_unrealized_pnl` confirm). So 46.9% is the true peak-to-trough including open-position drawdown — not understated.

## Methodology

- Ran the JOINT portfolio simulation (`run_kline_screening_with_funding`) — all 16 strategies against the same 1m bars simultaneously, sharing one global margin pool, with `max_global_budget_quote` enforced mid-sim via `budget_rejection_reason` (blocks legs that would exceed the cap). This is the faithful model of live behavior (the optimizer's per-candidate-curve combination ignores shared-capital / correlated drawdowns and is the source of the optimistic "10%").
- The sim's stock `annualized_return_pct` / `max_drawdown_pct` use the 144,436U planned-margin denominator (meaningless for a capped run), so the binary rebases cumulative PnL onto the budget principal: `equity(t) = budget + cum_pnl(t)`; annualized = `((1+total_return)^(365/days)-1)*100`; max DD = peak-to-trough on the rebased equity. Sim's internal DD (2.55%) and the rebased DD (46.9%) share the identical drawdown quote (~3,866U), confirming the rebase is consistent, not a bug.
- 141 budget-blocked legs at 1000U (deep averaging-down legs the cap correctly prevents); max_capital_used 999.8U (cap binds).

## Conclusion / decision needed

The conservative LP portfolio **cannot meet DD≤10% on real capital at any budget** — its inherent peak-to-trough drawdown is ~45–59%. The "DD≤10%" label came from measuring DD against 144,436U of uncapped planned margin, not against live capital. This corroborates the prior finding `martingale-conservative-bottleneck` ("ann>50% & DD<=10% 根本矛盾; seed 521 实证 0/64 候选达标").

Per plan Step 5: **do NOT start the formal 1000U live run.** Options for the user:
1. Re-optimize strategy parameters (smaller sizes / tighter risk) for real-capital DD≤10% — but prior evidence (0/64 candidates) suggests ann>50% ∧ DD≤10% may be infeasible; ann would drop sharply (see 10000U → 59% ann).
2. Accept the strategy's true profile (ann ~118%, DD ~47%) — relabel as high-risk, launch 1000U with eyes open.
3. Abort the 1000U launch; keep the engineering fixes (Tasks 2–4 are valid regardless); optionally do a 50U parity-validation smoke only.
4. Investigate further before deciding (methodology, other date ranges, etc.).

The engineering work (Tasks 2–4: budget-capped preflight, runtime margin-cap unit fix) is correct and valuable independent of this finding — it fixes real bugs and makes the preflight honest.
