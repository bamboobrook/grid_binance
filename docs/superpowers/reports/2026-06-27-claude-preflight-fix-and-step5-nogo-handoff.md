# Handoff to GLM/ChatGPT: Preflight Fix Done + Step 5 NO-GO (DD target was an artifact)

Date: 2026-06-27
From: Claude Code (executor of GLM's `2026-06-26-glm-margin-budget-preflight-fix-and-live-runbook.md` plan, Steps 1-5)
To: GLM / ChatGPT (for review + next-step plan)
Repo: `/home/bumblebee/Project/grid_binance`, branch `main`, HEAD `a4a4232` (engineering fixes) + uncommitted tool/report (committed in the handoff commit)
Owner: `flyingkid2022@outlook.com`

> **Headline:** Your plan's Bug A (confirm-start uncapped full-series preflight) and Bug B (runtime strategy-cap margin/notional unit mismatch) are **fixed, tested, and reviewed clean** (Tasks 2-4 below). Step 5 (the mandatory 1000U budget-aware replay) was run and the gate **FAILED on drawdown** — but the failure exposes that **the "conservative DD≤10%" target was never achievable on real capital; it was an artifact of measuring DD against 144,436 USDT of uncapped planned margin.** The conservative LP portfolio's inherent peak-to-trough drawdown is ~45-59% on any real budget. A decision is needed before any 1000U launch.

---

## 1. What was executed from your plan

| Plan step | Status | Result |
|---|---|---|
| Step 1 Freeze safe state | ✅ Done | trading-engine OFF; `BINANCE_LIVE_MODE=0` both envs; no Working orders; 4 portfolios `pending_confirmation`. **Finding:** stale abandoned SOL smoke (`mp_live_smoke_50_v2_20260624`) leaves 1 `strategy_runtime_positions` row (0.3 SOL long @66.48, updated 0625) + 3 `martingale_live_strategy_instances` status=`running`; its `strategy_orders`=0 rows. Binance-side truth not yet verified (needs the live probe; deferred to pre-smoke). Unrelated to margin-v2. |
| Step 2 Shared budget-capped projection | ✅ Done (Task 2, `e9bf35f`) | New helpers in `capital.rs`. |
| Step 3 Fix confirm-start preflight | ✅ Done (Task 3, `592ae17`) | Gates on budget-capped margin; full-series now diagnostic. |
| Step 4 Fix runtime strategy-cap unit bug | ✅ Done (Task 4, `a4a4232`) | Floors at first-leg MARGIN not NOTIONAL. |
| Step 5 1000U budget-aware replay | ✅ Done — **NO-GO** | ann 117.95% / **DD 46.90%** → fails DD≤10%. See §3. |
| Steps 6-12 (smoke, 1000U preflight/launch) | ⛔ Blocked | Pending the §5 decision. Step 5 NO-GO means do NOT launch 1000U per your plan. |

No live orders were ever placed. `trading-engine` was never started. `BINANCE_LIVE_MODE=0`.

---

## 2. Engineering fixes (Tasks 2-4) — reviewed clean

All three TDD-implemented via subagent + independent review; SPEC ✅ / Quality Approved each.

### Task 2 — `e9bf35f` — shared budget-capped capital projection (`apps/backtest-engine/src/martingale/capital.rs`)
Added pure helpers consumed by Steps 3 & 4:
- `first_leg_margin_quote(sizing, market, leverage, min_notional) -> Result<f64,String>`
- `project_strategy_capital(strategy_id, sizing, market, leverage, strategy_margin_cap, available_global_margin, min_notional) -> Result<StrategyCapitalProjection,String>` — walks legs, accepts a leg iff notional ≥ min AND cumulative margin ≤ strategy cap AND ≤ remaining global pool; **contiguous accepted-prefix** (a rejected leg breaks the chain — matches runtime `next_leg_index` semantics and backtest `new_legs_blocked`).
- `project_portfolio_capital(strategies, weights, global_margin_cap, min_notional, entry_fee_bps, fee_buffer_pct) -> Result<PortfolioCapitalProjection,String>` — applies weights (`weight_factor = portfolio_weight_pct/100`), per-strategy cap floored at `max(global×weight, first_leg_margin)`, shared global pool consumed in order.
- Structs: `LegCapitalProjection`, `StrategyCapitalProjection`, `PortfolioCapitalProjection` (full-series + budget-capped margin/notional, first-leg margin/notional, `all_strategies_can_start`, fee, required-with-buffer).
- 12 capital tests (7 new) green. Minor review findings recorded (unused `_exchange_min_notional` param kept for signature fidelity; `"prior leg not accepted"` skip_reason added).

### Task 3 — `592ae17` — confirm-start preflight (`apps/api-server/src/services/martingale_publish_service.rs`)
- Replaced `portfolio_projected_capital` (full uncapped series) with `project_portfolio_capital`. The gate (`preflight_rejection_reason`, pure) now rejects ONLY on: global cap ≤ 0; `!all_strategies_can_start` (a strategy first leg can't fit the global margin pool); available USDT < required-with-buffer. **Full-series margin is recorded as a diagnostic, no longer a gate.**
- New pure helper `extract_portfolio_weight_factors(config_value)` reads `portfolio_weight_pct` per strategy from the same raw JSON the runtime reads.
- `risk_summary.live_start_preflight` enriched with `capital_model:"margin_budget_cap"`, `full_series_projected_margin/notional_quote`, `budget_capped_projected_margin/notional_quote`, `first_leg_margin/notional_quote`, `projected_fee_quote`, `required_with_buffer_quote`, `available_usdt`, `all_strategies_can_start`, `per_strategy[]` with `skipped_legs[]`.
- Old `confirm_start_rejects_when_projected_margin_exceeds_budget` test replaced by 5 new tests (4 pure + 1 integration). `cargo test -p api-server --lib confirm_start` → 6 passed.

### Task 4 — `a4a4232` — runtime strategy-cap unit fix (`apps/trading-engine/src/martingale_budget.rs` + `main.rs`)
- `cap_strategy_budget` was flooring the per-strategy MARGIN cap at `first_order_quote` (NOTIONAL) — up to leverage× too large for futures. Now floors at first-leg **MARGIN** (`first_order_quote / leverage`; spot = `first_order_quote`).
- Relocated `cap_strategy_budget` + `apply_global_budget_allocations` + `cap_optional_budget_limit` into lib module `martingale_budget.rs` for unit testability (main.rs imports them). Global-cap enforcement in `martingale_runtime.rs::enforce_budget_for_next_leg` untouched (already margin-correct).
- 5 new `martingale_budget` tests; martingale + order_sync regression suites green; `cargo build -p trading-engine` OK. One stale inline assertion corrected (it had encoded the old buggy notional-floor value 11 → correct margin-floor value 5; the load-bearing "leg 0 still places at notional 11" assertion is intact).

### Hard example from your Step 3 now holds
`confirm_start` with `first_order_quote=250, leverage=5, budget=50`: first-leg margin = 50; passes the gate (notional 250 is diagnostic only). At budget=10 it rejects (first-leg margin 50 > 10 global pool).

### Pre-existing test failures (NOT caused by Tasks 2-4; identical at `e9bf35f`; defer to your Step 7)
`apps/api-server/tests/martingale_backtest_flow.rs`: `confirm_start_rechecks_conflicts_after_paused_portfolio`, `conflicting_pending_portfolios_allow_only_one_confirm`, `publish_rejects_same_symbol_leverage_conflict` — 14 pass / 3 fail at both `e9bf35f` and `a4a4232`. Cause: confirm-start/publish contract drift (the `exchange_preconfigure` readiness gate + futures `max_global_budget_quote` requirement were added after these tests were written; they don't seed preconfigure / pass a budget). They are test-maintenance fixes, not behavior changes.

---

## 3. Step 5 result — the reason for this handoff (NO-GO on DD)

Full report: `docs/superpowers/reports/2026-06-26-step5-1000u-budget-replay-result.md`.
Tool: new binary `apps/backtest-engine/src/bin/portfolio_budget_replay.rs` (joint kline sim + on-budget rebase; reusable for any budget/portfolio).

**Gate (your Step 5): annualized > 50% AND max DD ≤ 10% under 1000U → FAIL.**

| Budget | Ann (on budget) | Max DD (on budget) | Total return | Max capital used | Blocked legs | Gate |
|---|---|---|---|---|---|---|
| **1000U** | **117.95%** ✅ | **46.90%** ❌ | 1332% | 999.8U | 141 | **FAIL** |
| 2000U | 125.48% | 45.15% | 1508% | 1999.8U | 166 | FAIL |
| 5000U | 115.50% | 50.11% | 1278% | 5000.0U | 82 | FAIL |
| 10000U | 59.02% | 58.92% | 388% | 9999.1U | 30 | FAIL |

(1000U: trade_count=66262, stop_count=32007, total_fee=1579U, total_funding=-1375U, total_slippage=702U. Range 2023-01-01→2026-05-31, 1m, 14.4M bars, mark-to-market equity.)

### Why the gate fails — and why the displayed "DD≤10%" was an artifact
The backtest engine's stock return/DD denominator is the **uncapped planned margin** = Σ all leg margins across the 16 strategies = **144,436 USDT**. On that base the joint sim's own DD is **2.55%** (lower than the optimizer's reported 10%). The SAME ~3,866 USDT peak-to-trough drawdown (mark-to-market, incl. unrealized — kline_engine.rs:428) is 2.55% of 144,436U but **46.9% of real 1000U**. The displayed conservative DD≤10% measured drawdown against 144,436U of capital that a 1000U live run never deploys.

**DD is the strategy's inherent profile, not a function of budget.** Max DD stays ~45-59% from 1000U to 10000U. Martingale averaging-down inherently draws down ~half its deployed capital; scaling the budget scales PnL proportionally, so DD% is ~scale-invariant. At 10000U ann collapses to 59% while DD stays ~59%.

This corroborates the prior project finding `martingale-conservative-bottleneck` ("ann>50% & DD≤10% 根本矛盾; seed 521 实证 0/64 候选达标, portfolio_count=0"). The two independent analyses agree: **ann>50% ∧ DD≤10% is essentially infeasible for these martingale strategies on real capital.**

### Methodology notes for your review
- The replay uses the JOINT portfolio sim (`run_kline_screening_with_funding`) — all 16 strategies against the same 1m bars, shared global margin pool, `max_global_budget_quote` enforced mid-sim via `budget_rejection_reason`. This is the faithful live model. The optimizer's per-candidate-curve combination (source of the "10%") ignores shared-capital / correlated drawdowns and is therefore optimistic.
- The binary rebases cumulative PnL onto the budget principal because the sim's stock metrics use the 144,436U denominator (meaningless for a capped run). The rebase is consistent with the sim's own DD (identical drawdown quote ~3,866U; only the percentage base differs) — verified, not a bug.
- Equity is mark-to-market (tests `total_return_includes_final_unrealized_loss`, `multi_symbol_equity_keeps_other_symbol_unrealized_pnl`), so 46.9% is not understated.
- One nuance worth your judgment: the 46.9% DD is peak-to-trough on `1000 + cum_pnl`. At the worst point the account had grown to ~8,243U then dropped ~3,866U to ~4,378U — i.e. it gave back profit, never breaching the 1,000U principal at that instant. (Whether cum_pnl ever went below 0 / breached principal earlier is not yet extracted; the standard max-drawdown metric is 46.9% regardless.)

---

## 4. Current safe state
- `BINANCE_LIVE_MODE=0` (`.env` + `.worktrees/full-v1/.env`); `api-server` running on it; `trading-engine` NOT running.
- Binance (flyingkid): `/exchange/binance/account` → healthy, hedge on, withdrawals disabled. Open orders / positions not yet re-probed live (preflight needs LIVE_MODE=1; deferred). DB shows no margin-v2 (LTC/BTC/…) runtime positions; the only `strategy_runtime_positions` row is the stale 0.3 SOL from the abandoned 0624/0625 SOL smoke (unrelated).
- DB: 4 portfolios `pending_confirmation` (conservative/balanced/aggressive + `mp_smoke_50u_ltc_btc_20260626`).
- Engineering commits `e9bf35f..a4a4232` on `main`. Docker images NOT yet rebuilt from these (your Step 8) — current running images still predate the fixes.

---

## 5. Open decision for GLM (the next-step plan needs one of these)

Step 5 NO-GO blocks the 1000U launch per your own plan. The user asks you to review and plan the next step. Options:
1. **Re-optimize for real-capital DD≤10%** (smaller sizes / tighter risk), using the joint-sim/budget-aware metric as the selection criterion. Prior evidence (0/64 on seed 521; 10000U→59% ann here) suggests this likely yields low ann or no candidates. The new `portfolio_budget_replay` binary can drive this selection.
2. **Accept the strategy's true profile** (ann ~118%, DD ~47%), relabel high-risk, launch 1000U with eyes open (the user would have to explicitly accept ~47% peak DD).
3. **Abort the 1000U launch**, keep the engineering fixes (Tasks 2-4 are valid bug-fixes regardless), optionally run a 50U parity-validation smoke (validates the parity fixes live on a small strategy subset) without committing to the 1000U strategy.
4. **Investigate further** — e.g. extract the principal-breach / underwater metric, test other date ranges, or reconcile the joint-sim 47% vs optimizer 10% methodology gap in detail — before deciding.

My (Claude's) assessment: the engineering fixes are correct and should be kept/deployed regardless. The DD finding is decisive — "conservative DD≤10%" is not achievable for this strategy family on real capital (two independent analyses agree). The realistic choice is between (2) accept-and-relabel or (3) abort-1000U-but-keep-fixes; (1) is likely futile. But this is your call.

---

## 6. Key references for planning
- Your plan: `docs/superpowers/plans/2026-06-26-glm-margin-budget-preflight-fix-and-live-runbook.md`
- Step 5 full report: `docs/superpowers/reports/2026-06-26-step5-1000u-budget-replay-result.md`
- Prior handoff (parity fix + original preflight blocker): `docs/superpowers/reports/2026-06-26-claude-parity-fix-and-preflight-blocker-handoff.md`
- Replay tool: `apps/backtest-engine/src/bin/portfolio_budget_replay.rs` — `--config <portfolio_config json> --budget <dec> --start-ms <i64> --end-ms <i64> --market-data data/market_data_full.db --funding-data data/funding_rates.db`. Prints on-budget ann/DD + sim stock metrics + max_capital_used + budget-blocked legs + `gate_pass`.
- Step 2 helper: `apps/backtest-engine/src/martingale/capital.rs::project_portfolio_capital`.
- Step 3 preflight: `apps/api-server/src/services/martingale_publish_service.rs::confirm_start_portfolio` + `preflight_rejection_reason` + `extract_portfolio_weight_factors`.
- Step 4 runtime cap: `apps/trading-engine/src/martingale_budget.rs::cap_strategy_budget` + `apply_global_budget_allocations`.
- Backtest budget enforcement (already correct, what Step 5 relied on): `apps/backtest-engine/src/martingale/kline_engine.rs::budget_rejection_reason` (765-828), enforced mid-sim; equity/drawdown at kline_engine.rs:428-511.
