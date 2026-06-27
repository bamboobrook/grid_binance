# Runtime-Parity 1000U Replay Results — All Three LP Portfolios FAIL (Case B)

Date: 2026-06-27
Plan: `docs/superpowers/plans/2026-06-27-glm-step5-nogo-verdict-and-next-plan.md` (Steps 2-4 + 4A)
Executor: Claude Code (GLM plan, RT1-RT3)
Repo: `/home/bumblebee/Project/grid_binance`, branch `main`
Tool: `apps/backtest-engine/src/bin/portfolio_budget_replay.rs` (fixed in RT2: now applies the runtime's
per-strategy `portfolio_weight_pct` → `max_strategy_budget_quote` caps via the shared
`capital::apply_portfolio_weight_margin_caps` — RT1 — before running the joint sim)
Data: `data/market_data_full.db`, `data/funding_rates.db`; range 2023-01-01 → 2026-05-31 (1247 days, 1m, 14,365,440 bars); budget = 1000U margin principal.

## Headline

Under the **runtime-parity** budget model (the one the live trading-engine actually uses — global cap
1000U **plus** per-strategy caps derived from `portfolio_weight_pct`), **all three LP portfolios FAIL
their gates.** Conservative and balanced **blow the account up** (principal breached: on-budget equity
goes negative; fees + slippage alone exceed the 1000U budget). Aggressive survives (does not breach
principal, +223% total return) but misses its targets by a wide margin (ann 41% vs >110%; DD 52% vs ≤30%).

This is **Case B** of the plan's decision tree: do NOT launch 1000U; the existing LP result is invalid
for real-capital launch; re-optimize/search under the runtime-parity gate with a ≤5000U small-capital-native
candidate pool.

The prior "Step 5 46.90% DD" number is **superseded** — it was not runtime-parity (it set the global cap
but did not apply the per-strategy weight caps). The number below is the valid one.

## Gate results (1000U, runtime-parity)

| Portfolio | Gate | Ann (on budget) | Max DD (on budget) | Total return | Min equity | Principal breached | Verdict |
|---|---|---|---|---|---|---|---|
| conservative (ann>50, DD≤10) | **FAIL** | n/a (loss) | **334.51%** | **-347.30%** | **-2473.50U** | **YES** | account liquidated |
| balanced (ann>90, DD≤20) | **FAIL** | n/a (loss) | **142.91%** | **-212.33%** | **-1129.24U** | **YES** | account liquidated |
| aggressive (ann>110, DD≤30) | **FAIL** | 41.06% | **52.33%** | +223.90% | 978.03U | no | misses both targets |

(`runtime_weight_caps_applied = true` for all three. `annualized_return_pct` is null for conservative/
balanced because total return is below -100%, i.e. the principal is more than wiped out.)

### Additional gate checks

| Check | conservative | balanced | aggressive |
|---|---|---|---|
| `max_capital_used_quote ≤ 1000 + ε` | 419.55U ✅ | 464.83U ✅ | 439.53U ✅ |
| no min-notional order failure | (sim uses configured first orders; no runtime min-notional gate in sim) | same | same |
| no negative-equity point (`principal_breached`) | **breached** ❌ | **breached** ❌ | ok ✅ |
| fees + slippage + funding included, MTM | yes | yes | yes |
| `runtime_weight_caps_applied` | true ✅ | true ✅ | true ✅ |
| `max_global_budget_quote = 1000` in replay config | yes | yes | yes |

### Cost / churn breakdown

| Portfolio | trades | stops | fee (U) | slippage (U) | funding (U) | fee+slip vs 1000U budget |
|---|---|---|---|---|---|---|
| conservative | 133,820 | 66,056 | 2824.8 | 1255.5 | -31.7 | **4080U = 4.08× budget** |
| balanced | 128,541 | 63,503 | 2885.4 | 1282.4 | -119.7 | **4168U = 4.17× budget** |
| aggressive | 104,075 | 51,360 | 1854.9 | 824.4 | -123.9 | 2679U = 2.68× budget |

### Budget-rejection breakdown (all 1000U)

| Portfolio | global | strategy | symbol | direction | total |
|---|---|---|---|---|---|
| conservative | 0 | **210** | 0 | 0 | 210 |
| balanced | 0 | **194** | 0 | 0 | 194 |
| aggressive | 0 | **166** | 0 | 0 | 166 |

Every blocked leg is blocked by the **per-strategy** cap (the runtime weight cap), not the global cap.
This is the direct evidence that the weight caps are binding and truncating each strategy's averaging-down
ladder.

## Minimum-capital diagnostic (Step 4A)

| Portfolio | natural unscaled planned margin | LP-weighted planned margin | min exact-scaled executable principal (≥5U first orders) | bottleneck | scale-to-1000 min first order |
|---|---|---|---|---|---|
| conservative | 144,437U | 21,958U | **53,686U** | XRPUSDT/staged-19925-Long (foq 80) | 1.49U (< 5U) |
| balanced | 288,650U | 15,641U | **1,029,696U** | SOLUSDT/staged-13640-Long (foq 30) | 0.029U (< 5U) |
| aggressive | 130,087U | 9,482U | **209,834U** | LINKUSDT/staged-14623-Long (foq 90) | 0.43U (< 5U) |

None of the three can be exact-proportionally cloned to 1000U (every `min_exact_scaled_executable_principal`
is 54k–1,030k U, far above 1000U; the scaled smallest first order would be 0.03–1.49U, below Binance's ~5U
minimum notional). So at 1000U the only available model is **cap-truncated** (`scale_model_used_for_gate =
"cap_truncated"`): keep the original (large) first orders and let the global+per-strategy caps block the
deep averaging-down legs.

## Root cause — why cap-truncation at 1000U destroys these portfolios

The LP portfolios were designed and selected on a **full-ladder, ~144,436U planned-margin** basis (the LP
optimizer recombines normalized candidate equity curves that assume every leg of the martingale ladder can
be placed). At 1000U the runtime's per-strategy weight caps (`max_strategy_budget_quote = 1000 × weight`,
e.g. LTC 180U, BTC 11U) **truncate each ladder to 1–5 accepted legs** (see `accepted_static_legs` in the
per-strategy diagnostics; 210/194/166 deep legs blocked).

A martingale strategy with its averaging-down ladder truncated cannot recover adverse entries by averaging
down — instead it **stops out and re-enters repeatedly**, realizing every loss. The result is enormous
churn (133k / 128k / 104k trades) whose **fees + slippage alone (4080U / 4168U / 2679U) exceed the entire
1000U budget** for conservative and balanced. That friction, plus the realized stop losses, drives the
on-budget equity negative — the account is liquidated. This is exactly the cap-truncation pathology the
plan's Addendum 1 predicted ("fixed first orders plus 1000U cap truncates martingale legs and changes the
original strategy").

Aggressive avoids liquidation only because its particular candidate set churns less (104k trades) and
happened to net positive after costs — but it still earns just 41% annualized (vs >110% target) with 52%
drawdown (vs ≤30% target).

**Key distinction from the displayed LP numbers:** the LP "conservative DD≤10% / ann>50%" measured
drawdown against 144,436U of uncapped planned margin and assumed full ladders. Neither assumption holds at
1000U live: the denominator is 1000U and the ladders are truncated. The displayed LP profile is not
achievable on 1000U real capital with these candidates.

## Methodology (why this number is valid)

- The replay injects `max_global_budget_quote = 1000` AND applies
  `capital::apply_portfolio_weight_margin_caps(portfolio, raw_config)` — the **same function the live
  trading-engine calls** (RT1 unified them; verified byte-for-byte: weight = `portfolio_weight_pct/100`,
  cap = `global × weight` floored at first-leg MARGIN, `min(existing, effective)`). So the replay's budget
  model is structurally identical to live. `runtime_weight_caps_applied = true` in every output.
- The joint sim (`run_kline_screening_with_funding`) runs all 16 strategies on the same 1m bars, sharing
  one global margin pool, with the per-strategy caps enforced mid-sim via `budget_rejection_reason`
  (kline_engine.rs:765-828; reads `strategy.risk_limits.max_strategy_budget_quote` at :815-817).
- The sim's stock metrics use the 144,436U uncapped-planned-margin denominator (meaningless for a capped
  run), so the tool rebases cumulative PnL onto the 1000U principal: `equity_on_budget(t) = 1000 +
  cum_pnl(t)`; annualized with 1000 as principal; max DD peak-to-trough; `principal_breached` if equity
  ever ≤ 0. Equity is mark-to-market (includes unrealized PnL), so the breach is not understated.

## Verdict: Case B

All three existing LP portfolios fail the 1000U runtime-parity gate; conservative and balanced liquidate
the account. Per the plan's decision tree (Case B):

1. **Do NOT launch 1000U live.** (Already not launched; `BINANCE_LIVE_MODE=0`, trading-engine off.)
2. The LP result is marked **invalid for real-capital launch** — its risk/return profile depends on
   full-ladder capital that 1000U cannot provide.
3. The existing candidates cannot be exact-scaled to 1000U (min executable principal 54k–1,030k U) and
   their cap-truncated form blows up. **A new ≤5000U small-capital-native search is required** (plan
   Addendum 2): generate candidates whose full ladders fit the budget (Mode A: exact full-ladder) or whose
   cap-truncated replay itself passes (Mode B), with first orders chosen above Binance min notional, fewer/
   shallower legs, and the runtime-parity replay as the selection gate. Sweep budgets 500/1000/1500/2000/
   3000/5000.
4. Do not proceed to 50U smoke or 1000U launch until a ≤5000U candidate passes the runtime-parity replay
   AND the user confirms.

## Engineering state

- RT1 (`039a9bf..6c848bf`): canonical `apply_portfolio_weight_margin_caps` in `capital.rs`; trading-engine
  rewired to it (one source of truth). Reviewed SPEC ✅ / Approved; parity byte-for-byte.
- RT2 (`6c848bf..705b1f9`): replay tool fixed (applies weight caps, parameterized gate, diagnostics,
  minimum-capital view, principal-breach check). Reviewed SPEC ✅ / Approved; parity chain confirmed.
- Both TDD, tests green. Docker images NOT yet rebuilt (no live action pending).
- Known follow-up (non-blocking): `api-server`'s `extract_portfolio_weight_factors` still diverges from the
  canonical (f64 return, silent-omit ≤0) — a latent preflight-parity gap to clean up before any live
  confirm-start.

## Appendix: capital frontier (existing LP portfolios scaled to 1000/2000/3000/5000U)

To bound whether MORE capital rescues the existing (well-optimized) candidates, the same three portfolios
were replayed at 2000/3000/5000U (same runtime-parity model). Result: **none passes at any budget up to
5000U.** Drawdown is the persistent blocker; fees+slippage are ~3800-4800U regardless of budget (trade
churn is roughly constant, so friction is a fixed structural drag).

| portfolio | budget | ann (target) | max DD (target) | total ret | principal breached | fee+slip | gate |
|---|---|---|---|---|---|---|---|
| conservative | 1000 | n/a (>50) | 334.5% (≤10) | -347.3% | YES | 4080 | FAIL |
| conservative | 2000 | n/a | 100.5% | -100.5% | YES | 3829 | FAIL |
| conservative | 3000 | -42.9% | 89.3% | -85.2% | no | 4223 | FAIL |
| conservative | 5000 | 4.3% | **51.3%** | +15.4% | no | 4295 | FAIL |
| balanced | 1000 | n/a (>90) | 142.9% (≤20) | -212.3% | YES | 4168 | FAIL |
| balanced | 2000 | 25.1% | 58.9% | +115.1% | no | 3795 | FAIL |
| balanced | 3000 | 38.9% | 46.4% | +207.5% | no | 4615 | FAIL |
| balanced | 5000 | 22.4% | **45.1%** | +99.3% | no | 4821 | FAIL |
| aggressive | 1000 | 41.1% (>110) | 52.3% (≤30) | +223.9% | no | 2679 | FAIL |
| aggressive | 2000 | 58.8% | 42.9% | +385.6% | no | 3830 | FAIL |
| aggressive | 3000 | 48.5% | 39.1% | +286.1% | no | 3898 | FAIL |
| aggressive | 5000 | **66.4%** | **36.7%** | +469.3% | no | 4423 | FAIL |

Reading:
- **Drawdown is inherent, not budget-dependent.** Even at 5000U (where caps truncate fewer legs), peak-to-
  trough DD stays 36-51% — far above the 10/20/30% targets. This is the martingale averaging-down profile;
  scaling capital does not remove it.
- **Fees+slippage are a fixed ~4000U drag** (trade count ~100-130k regardless of budget). At low budgets
  this alone exceeds the principal (the 1000U blow-ups).
- **Aggressive is the closest** to viable: at 5000U ann 66% / DD 37%. If the aggressive target were relaxed
  to roughly `ann > 60% & DD ≤ 40%`, it would pass at 5000U. Conservative is paradoxically the *worst*
  (its high-weight LTC/DYDX candidates churn hardest when truncated).
- This is a third independent line of evidence (after the seed-521 candidate search and the original
  backtest-robustness work) that `ann > 50/90/110% ∧ DD ≤ 10/20/30%` is not achievable for these martingale
  strategies on real capital up to 5000U. The blocker is fundamental drawdown, not capital size.

Caveat: this frontier is on the EXISTING high-capital candidates run cap-truncated. A purpose-built
≤5000U small-capital-NATIVE search (full ladders that fit the budget, first orders ≥ min notional) would
behave differently (full ladders recover instead of truncating, cutting churn) and is what the plan's Case
B prescribes. But the inherent-DD floor (~37% even at 5000U with minimal truncation) indicates the DD
targets — especially conservative ≤10% — are very likely out of reach for martingale regardless of design.
