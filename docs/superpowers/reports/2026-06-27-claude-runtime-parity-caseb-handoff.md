# Handoff to GLM/ChatGPT: Runtime-Parity Replay Fixed → All 3 LP Portfolios FAIL at 1000U AND 1000-5000U (Case B)

Date: 2026-06-27
From: Claude Code (executor of GLM's `2026-06-27-glm-step5-nogo-verdict-and-next-plan.md`, RT1-RT4)
To: GLM / ChatGPT (for review + re-spec of targets/approach)
Repo: `/home/bumblebee/Project/grid_binance`, branch `main`, HEAD `2beba00`
Owner: `flyingkid2022@outlook.com`

> **Headline:** You were right — my prior `46.90% DD` was **not runtime-parity** (it set the global cap but
> not the per-strategy `portfolio_weight_pct` caps the runtime applies). I fixed it exactly as your plan
> prescribed (RT1 unified the applier into `capital.rs`, shared by trading-engine AND the replay; RT2 fixed
> the replay tool). Rerunning under the **correct** runtime-parity model: **all three LP portfolios FAIL at
> 1000U** — conservative and balanced **liquidate the account** (principal breached; fees+slippage alone
> exceed the budget). A capital-frontier sweep then showed they **FAIL at every budget from 1000U to
> 5000U**: drawdown is inherent (~37-51% even at 5000U with minimal ladder truncation) and fees+slippage
> are a fixed ~4000U drag. This is a **third independent line of evidence** (after seed-521 0/64 and the
> robustness/WFE work) that `ann>50/90/110% ∧ DD≤10/20/30%` is not achievable for these martingale
> strategies on real capital. The user reviewed this and chose to **hand it back to you to re-spec**
> (targets / strategy family / whether a bounded search is worth running) rather than commit to a long
> search that the evidence suggests is likely futile.

---

## 1. What was executed from your plan

| Plan item | Status | Result |
|---|---|---|
| Step 1 Keep system safe | ✅ | trading-engine stopped; `BINANCE_LIVE_MODE=0` both envs; no live orders ever. |
| Step 2 Move budget allocation into shared code | ✅ RT1 (`6c848bf`) | Canonical `apply_portfolio_weight_margin_caps` in `capital.rs`; trading-engine rewired to it. |
| Step 3 Fix `portfolio_budget_replay` | ✅ RT2 (`705b1f9`) | Applies weight caps, parameterized gate, diagnostics, min-capital view, principal-breach check. |
| Step 4 Rerun all 3 @1000U | ✅ RT3 (`d742311`) | **All 3 FAIL.** Conservative/balanced liquidate. See §3. |
| Step 4A Minimum-capital diagnostic | ✅ (in RT2) | None can exact-scale to 1000U (min executable principal 54k/1030k/210kU). |
| Decision tree | → **Case B** | Existing LP invalid for real-capital launch. User chose hand-back for re-spec. |
| Case B search / smoke / 1000U launch | ⛔ Not started | Pending your re-spec. |

---

## 2. Engineering fixes (RT1/RT2) — valid regardless of the strategy verdict

### RT1 — `6c848bf` — one source of truth for the budget-cap applier
- New canonical fns in `apps/backtest-engine/src/martingale/capital.rs`: `apply_portfolio_weight_margin_caps
  (config, raw_json) -> Result<(),String>` and `extract_portfolio_weight_factors(raw_json) -> Result<HashMap,
  String>`, plus `first_leg_margin_for_strategy` (distinct name; no clash with the prior low-level
  `first_leg_margin_quote`). Caps in MARGIN units (futures `first_order_quote/leverage`, spot
  `first_order_quote`); `cap = global × weight` floored at first-leg margin; `min(existing, effective)`.
- `apps/trading-engine/src/martingale_budget.rs` + `main.rs::apply_portfolio_weight_scaling` rewired to call
  it. **The live runtime and the replay now call the identical function** — parity is structural.
- Reviewed SPEC ✅ / Approved; parity verified byte-for-byte (weight = `portfolio_weight_pct/100`; missing
  weight → equal-cap fallback; present ≤0 → Err; global ≤0 → no-op). All suites green.
- Non-blocking follow-up flagged: `api-server::extract_portfolio_weight_factors` still diverges (f64 return,
  silent-omit ≤0) — a latent preflight-parity gap; off the replay critical path, clean up before any live
  confirm-start.

### RT2 — `705b1f9` — replay tool is runtime-parity
- `apps/backtest-engine/src/bin/portfolio_budget_replay.rs` now: injects `max_global_budget_quote`, then
  calls `apply_portfolio_weight_margin_caps` (with the strategies-rooted raw JSON) BEFORE the sim. New
  testable lib module `apps/backtest-engine/src/martingale/budget_replay.rs`. Emits `runtime_weight_caps_
  applied`, per-strategy caps, rejection breakdown (global/strategy/symbol/direction), the on-budget rebase
  (`equity_on_budget = budget + cum_pnl`; ann on budget; peak-to-trough DD; `principal_breached` if ≤0),
  and the minimum-capital feasibility view. Gate parameterized by profile (conservative 50/10, balanced
  90/20, aggressive 110/30). Hardcoded portfolio_id removed.
- Reviewed SPEC ✅ / Approved; parity chain confirmed; 15 new tests.

---

## 3. RT3 result — all 3 FAIL at 1000U (the valid, runtime-parity number)

Full report + per-strategy diagnostics: `docs/superpowers/reports/2026-06-27-runtime-parity-1000u-replay-results.md`.

| Portfolio | Gate | Ann (on 1000U) | Max DD | Total return | Min equity | Principal breached | fee+slip |
|---|---|---|---|---|---|---|---|
| conservative (ann>50, DD≤10) | **FAIL** | n/a (loss) | **334.5%** | **-347.3%** | **-2474U** | **YES** | 4080U |
| balanced (ann>90, DD≤20) | **FAIL** | n/a (loss) | **142.9%** | **-212.3%** | **-1129U** | **YES** | 4168U |
| aggressive (ann>110, DD≤30) | **FAIL** | 41.1% | 52.3% | +223.9% | 978U | no | 2679U |

`runtime_weight_caps_applied = true` for all. All 210/194/166 budget-rejections are the **strategy** cap
(the per-strategy weight caps) — direct evidence the caps truncate each ladder to 1-5 accepted legs.

**Root cause of the blow-ups:** the LP portfolios were selected on a full-ladder, ~144,436U planned-margin
basis. At 1000U the per-strategy caps (`1000×weight`: LTC 180U, BTC 11U, …) truncate each averaging-down
ladder. A truncated martingale cannot recover by averaging down → it stops out and re-enters repeatedly →
enormous churn (128k-134k trades) whose **fees+slippage alone (4080/4168U) exceed the 1000U budget** →
on-budget equity goes negative → account liquidated. This is exactly the cap-truncation pathology your
Addendum 1 predicted.

Minimum-capital (Step 4A): none can exact-proportionally clone to 1000U (min executable principal
conservative 53,686U / balanced 1,029,696U / aggressive 209,834U; scaled smallest first order 0.03-1.49U,
below Binance ~5U min). So 1000U forces the cap-truncated model, which blows up.

---

## 4. Capital frontier — all 3 FAIL at every budget 1000U-5000U

To bound whether more capital rescues the existing candidates, the same three were replayed at
2000/3000/5000U (same runtime-parity model):

| Portfolio | 1000U | 2000U | 3000U | 5000U | target |
|---|---|---|---|---|---|
| conservative ann | n/a | n/a | -42.9% | 4.3% | >50% |
| conservative DD | 334.5% | 100.5% | 89.3% | **51.3%** | ≤10% |
| balanced ann | n/a | 25.1% | 38.9% | 22.4% | >90% |
| balanced DD | 142.9% | 58.9% | 46.4% | **45.1%** | ≤20% |
| aggressive ann | 41.1% | 58.8% | 48.5% | **66.4%** | >110% |
| aggressive DD | 52.3% | 42.9% | 39.1% | **36.7%** | ≤30% |

(All FAIL at all four budgets. Full table incl. total return / principal_breached / fee+slip in the report appendix.)

**Reading:**
- **Drawdown is inherent, not budget-dependent.** Even at 5000U (caps truncate few legs), peak-to-trough DD
  stays 36-51% — far above the 10/20/30% targets. Scaling capital does not remove martingale's averaging-down
  drawdown.
- **Fees+slippage ≈ 3800-4800U regardless of budget** (trade churn ~100-134k is roughly constant) — a fixed
  structural drag that alone exceeds low-budget principals.
- **Aggressive is the closest to viable**: at 5000U, ann 66.4% / DD 36.7%. If its target were relaxed to
  roughly `ann > 60% & DD ≤ 40%`, it passes at 5000U. Conservative is paradoxically the *worst* (its
  high-weight LTC/DYDX candidates churn hardest when truncated).

Caveat: this frontier is on the EXISTING high-capital candidates run cap-truncated. A purpose-built
≤5000U small-capital-NATIVE search (full ladders fitting the budget) would behave differently (full ladders
recover, cutting churn). BUT the inherent-DD floor (~37% even at 5000U with minimal truncation) indicates
the DD targets — especially conservative ≤10% — are very likely out of reach for martingale regardless of
design.

---

## 5. Synthesis — three independent evidence lines agree

| Evidence line | Verdict |
|---|---|
| seed-521 candidate search (prior work, `martingale-conservative-bottleneck`) | 0/64 candidates meet conservative ann>50 ∧ DD≤10; portfolio_count=0 |
| Backtest robustness / walk-forward (prior work) | ann>100% ⇒ regime-overfit (WFE<0); robust ann ceiling ~85-96% |
| **Runtime-parity capital frontier (this work)** | **All 3 LP portfolios FAIL at every budget 1000-5000U; DD inherent ~37-51%; fees ~4000U fixed** |

All three say the same thing: the stated targets are not achievable for these martingale strategies on real
(retail) capital. The blocker is fundamental drawdown, not capital size, not a code bug (the code is now
correct and runtime-parity).

---

## 6. Current safe state

- `BINANCE_LIVE_MODE=0` (both `.env` and `.worktrees/full-v1/.env`); `api-server` running on it;
  `trading-engine` stopped. No live orders ever placed. No margin-v2 runtime positions.
- Engineering commits `6c848bf` (RT1), `705b1f9` (RT2) on `main`; reports at `d742311`, `2beba00`; all
  pushed. Docker images NOT yet rebuilt from RT1/RT2 (no live action pending).
- DB: 4 portfolios still `pending_confirmation` (conservative/balanced/aggressive + the LTC/BTC smoke),
  `max_global_budget_quote = NULL` on the three LP ones.

---

## 7. Decision fork — the user asks you to re-spec

The user reviewed the above and chose to hand it back to you rather than commit to a long search. The
realistic options:

1. **Relax the targets to martingale's real profile and proceed.** Aggressive @5000U is the closest
   (ann ~66%, DD ~37%). A target like `ann > 60% & DD ≤ 40%` is achievable now at 5000U; the user would
   accept ~40% peak DD. This unblocks a real (high-risk) launch with eyes open.
2. **Authorize a bounded ≤5000U small-capital-NATIVE search** (your Case B), focused on the aggressive
   profile (the only near-viable one), full-ladder Mode A, first orders ≥ min notional. Bounded scope,
   hours of compute; may improve on 66%/37% but the DD floor suggests conservative ≤10% stays out of reach.
3. **Switch strategy family.** If DD≤10-30% is a hard requirement, martingale averaging-down is the wrong
   tool — a non-averaging or tightly-stopped family is needed. This is a larger re-architecture.
4. **Declare the stated targets infeasible; stop margin-v2 live.** Three independent evidence lines suffice.
   Keep the engineering fixes (they are correct bug-fixes); write the infeasibility report; archive.

My (Claude's) assessment: the evidence is now overwhelming that the *stated* targets are infeasible for
martingale. The pragmatic path is (1) accept aggressive's real profile at 5000U, or (3) change strategy
family if the DD caps are truly hard. A long search (2) for conservative/balanced DD targets is likely to
only re-confirm infeasibility. But this is your call.

---

## 8. Key references for re-spec

- Your plan: `docs/superpowers/plans/2026-06-27-glm-step5-nogo-verdict-and-next-plan.md`
- This result + frontier appendix: `docs/superpowers/reports/2026-06-27-runtime-parity-1000u-replay-results.md`
- Prior handoff (preflight fix + original Step 5): `docs/superpowers/reports/2026-06-27-claude-preflight-fix-and-step5-nogo-handoff.md`
- Replay tool (now runtime-parity): `apps/backtest-engine/src/bin/portfolio_budget_replay.rs` + lib
  `apps/backtest-engine/src/martingale/budget_replay.rs`. Args: `--config <json> --budget <dec> --start-ms
  <i64> --end-ms <i64> --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile
  conservative|balanced|aggressive --portfolio-id <id> [--exchange-min-notional 5.0]`. Reusable for any
  budget/portfolio; prints on-budget ann/DD + principal_breached + per-strategy caps + rejection breakdown
  + minimum-capital view + `gate.passed`.
- Shared applier (one source of truth): `apps/backtest-engine/src/martingale/capital.rs::apply_portfolio_weight_margin_caps`.
- Backtest budget enforcement: `apps/backtest-engine/src/martingale/kline_engine.rs::budget_rejection_reason`
  (765-828); equity/drawdown mark-to-market at :428-511.
