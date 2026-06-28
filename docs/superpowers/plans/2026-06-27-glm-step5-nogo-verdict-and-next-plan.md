# 2026-06-27 GLM Execution Plan: Step 5 NO-GO Verification, Runtime-Parity Replay, and Next Actions

## Purpose

This document reviews Claude's latest handoff:

- `docs/superpowers/reports/2026-06-27-claude-preflight-fix-and-step5-nogo-handoff.md`
- `docs/superpowers/reports/2026-06-26-step5-1000u-budget-replay-result.md`

The user asked whether the latest Step 5 result proves the original backtest was invalid, or whether Claude's validation was wrong. This plan gives the verdict and the exact next work GLM must execute.

Do not start live trading from the current state. Do not use the 46.90% DD result as the final launch decision until the replay tool is fixed to match the live runtime budget behavior.

## Short Verdict

There are two separate truths:

1. Claude is right that the displayed LP conservative result is not sufficient proof for a 1000U live account.
   - The LP result was built from normalized persisted candidate equity curves.
   - It was not a joint 1000U margin-cap live replay.
   - The final database portfolios currently have `max_global_budget_quote = NULL`, so they are not yet explicitly configured as 1000U principal portfolios.

2. Claude's Step 5 `46.90% DD` replay is not runtime-parity yet.
   - `portfolio_budget_replay.rs` injects only `portfolio.risk_limits.max_global_budget_quote = 1000`.
   - It does not apply the same per-strategy `portfolio_weight_pct` -> `max_strategy_budget_quote` allocation that the trading runtime applies before execution.
   - Therefore it is a useful warning, but it is not a final valid no-go measurement.

The next step is not live trading and not immediate abandonment. The next step is to fix the replay/backtest validation so it exactly matches live runtime budget semantics, then rerun all three target portfolios under real capital:

- Conservative: annualized > 50%, max DD <= 10%, budget = 1000U margin principal.
- Balanced: annualized > 90%, max DD <= 20%, budget = 1000U margin principal.
- Aggressive: annualized > 110%, max DD <= 30%, budget = 1000U margin principal.

If the fixed runtime-parity replay fails, re-optimize/search under the same budget-aware gate. If no portfolio can meet the targets, report infeasibility with evidence instead of launching.

## 2026-06-27 Addendum: The Core Cause Is Not Just "Budget Too Small"

The user's latest question is correct: if the portfolio were truly scaled linearly, changing principal from 100000U to 1000U should mostly shrink position size, not radically change return/DD percentages.

The current system is not doing that. There are three different capital models being mixed:

1. **LP display model**
   - Combines normalized candidate equity curves by weight.
   - It is a mathematically valid curve recombination.
   - It is not, by itself, an executable 1000U live configuration.

2. **Generated live config without scaling**
   - Copies each selected candidate's original strategy sizing.
   - Keeps `sizing.multiplier.first_order_quote` unchanged.
   - Writes `portfolio_weight_pct`, but does not scale first orders.
   - Without an explicit `max_global_budget_quote`, runtime falls back to the unweighted full planned margin.

3. **1000U cap model**
   - Keeps original first-order notionals.
   - Adds a 1000U margin cap.
   - Deep martingale legs get blocked by budget.
   - This is not proportional scaling; it is strategy truncation.

So the main reason the 1000U replay diverges is: **1000U is currently being used as a hard margin cap against unscaled source strategies, not as a clean scale factor that produces a smaller but geometrically identical portfolio.**

There is a second, unavoidable market constraint: exact linear scaling down to 1000U would make many first orders smaller than Binance minimum notional, so the original LP strategy family has a large minimum executable principal.

## Minimum Capital Diagnostic

Read-only static calculations from the current DB portfolios:

```text
Portfolio                                weighted planned margin   min exact-scaled principal for >=5U first orders
mp_margin_v2_lp_conservative_20260626     43,915.09 USDT             108,703.78 USDT
mp_margin_v2_lp_balanced_20260626         31,283.00 USDT             556,473.12 USDT
mp_margin_v2_lp_aggressive_20260626       18,964.66 USDT             945,404.20 USDT
```

For the conservative portfolio, exact scaling to 1000U gives these smallest first-order notionals by candidate:

```text
XRPUSDT   0.045997U
ICPUSDT   0.086482U
LTCUSDT   0.169428U
DYDXUSDT  0.262381U
UNIUSDT   0.547385U
INJUSDT   0.606597U
BTCUSDT   1.275369U
FILUSDT   1.301162U
```

These are below a realistic Binance Futures minimum notional such as 5U. Therefore a 1000U exact proportional clone of the displayed LP portfolio is not executable. To make every first order at least 5U while preserving the same full-ladder proportions, the conservative portfolio needs approximately **108,704U** principal. Balanced and aggressive are even higher because they include very low-weight candidates with huge full ladders.

This explains the user's intuition:

- The intuition is right in a frictionless scaling model.
- The current implementation is not using that model.
- At 1000U, exact scaling violates exchange minimum order size.
- The alternative, fixed first orders plus 1000U cap, changes martingale geometry and invalidates the original curve quality.

GLM must therefore add a **minimum executable principal** calculation to every martingale portfolio result.

Required fields:

```text
min_executable_principal_quote
min_executable_principal_reason
scaled_first_order_min_notional_quote
scale_model = exact_full_ladder | cap_truncated | hybrid
```

Final portfolios must not be labeled as suitable for 1000U live unless:

1. the exact-scaled model is executable under Binance min notional, or
2. the cap-truncated/hybrid model itself is replayed and passes the target gates.

The current LP portfolios satisfy neither condition yet.

## 2026-06-27 Addendum 2: Do Not Stop At Minimum Capital; Run Small-Capital Native Search

The minimum-capital diagnostic above explains why the current LP portfolios cannot be linearly scaled to 1000U. It must not be used as the final answer.

The user cannot reasonably run a 100k+ USDT martingale account. GLM must therefore run a new **small-capital-native** search instead of trying to preserve the existing high-capital LP portfolio exactly.

New research question:

```text
Within margin principal <= 5000U, can we find executable martingale portfolios that meet:

conservative: annualized > 50%,  max DD <= 10%
balanced:     annualized > 90%,  max DD <= 20%
aggressive:   annualized > 110%, max DD <= 30%
```

This search must treat 5000U as a hard upper bound, not as a display label. It should evaluate budgets such as:

```text
500U, 1000U, 1500U, 2000U, 3000U, 5000U
```

If a target cannot be reached at 1000U but can be reached at 3000U or 5000U, report that clearly. If no budget <=5000U reaches a target, report the best frontier instead of forcing a false pass.

### Why this can differ from the current LP result

The current LP result uses high-capital candidates and then assigns small weights. That creates "dust" allocations when scaled down. A small-capital-native strategy must be built differently:

- fewer symbols if 8 symbols forces too many tiny allocations,
- larger minimum member weights,
- first orders chosen directly above Binance min notional,
- fewer or shallower martingale legs,
- multipliers constrained so full ladder or accepted ladder fits the budget,
- no candidate with `min_exact_scaled_executable_principal_quote > budget` may be accepted under exact scaling,
- cap-truncated strategies are allowed only if the cap-truncated replay itself passes.

### Required small-capital search modes

GLM must run at least these two modes.

#### Mode A: Exact Full-Ladder Small-Capital Search

For every generated strategy:

```text
first_order_quote >= Binance min notional for symbol
planned_margin_quote <= allocated_strategy_margin_cap
allocated_strategy_margin_cap = portfolio_budget * member_weight
```

For long/short internal strategies, split the member weight explicitly and make sure both internal first orders remain executable.

This mode preserves martingale geometry because the full ladder fits inside the budget. It is the cleanest model. If it can hit the targets, prefer it.

#### Mode B: Budget-Truncated Small-Capital Search

For every generated strategy:

```text
first_order_quote >= Binance min notional for symbol
next-leg budget rejection uses the same runtime margin cap logic
the backtest/replay result is measured directly on budget principal
```

This mode intentionally allows the budget cap to block deeper martingale legs. It is a different strategy family from the original LP. It may be acceptable only if the replayed cap-truncated strategy itself meets the mode target.

### Required parameter ranges for small-capital search

GLM should not reuse only the old high-capital candidate pool. Generate a new pool with low-capital-friendly parameters:

```text
symbols: liquid futures symbols only, with reliable 1m data and funding
portfolio budgets: 500, 1000, 1500, 2000, 3000, 5000
member count: 2..8, but do not force 8 if it creates dust allocations
minimum member weight: 5% or 10% in low-budget modes
first_order_quote: Binance min notional .. 100U, budget-aware
leverage: 2..10, but record liquidation/risk implications
max_legs: 2..8 for <=1000U, 2..10 for <=5000U
multiplier: 1.1..2.4, with budget feasibility pruning
spacing: existing fixed/ATR variants, but only if live runtime supports them
take_profit / stop_loss: only models that live runtime supports exactly
entry triggers: only indicators supported identically by live runtime
fees/slippage/funding: always included
```

Prune early:

```text
if first_order_quote < min_notional: reject
if exact mode and full planned margin > allocated cap: reject
if mode B and first leg cannot fit cap + fee: reject
if any indicator/TP/SL is not live-compatible: reject
```

### Required reporting for <=5000U search

Create:

```text
docs/superpowers/reports/2026-06-27-small-capital-martingale-search-results.md
```

Report one table per budget:

```text
budget | mode | risk profile | best ann | best DD | pass/fail | symbols | strategies | max capital used | fees | funding | blocked legs | min executable principal
```

Report final candidates, if found:

```text
conservative <=5000U candidate
balanced <=5000U candidate
aggressive <=5000U candidate
```

If multiple budgets pass, prefer the smallest principal that passes each profile. If no budget passes, provide the efficient frontier:

```text
for DD<=10: best annualized
for DD<=20: best annualized
for DD<=30: best annualized
for ann>50: lowest DD
for ann>90: lowest DD
for ann>110: lowest DD
```

### Execution priority for GLM

1. Fix runtime-parity replay and minimum-capital diagnostics.
2. Run quick feasibility sweeps for budgets 500/1000/2000/5000 using small-capital-native generation.
3. If any budget looks promising, expand the search around that parameter region.
4. Only after passing candidates exist, update flyingkid display.
5. Do not proceed to 50U smoke or live launch until a <=5000U candidate passes replay and the user confirms.

## Evidence From Inspection

### A. Margin-v2 capital model itself is correct

The current backtest engine uses:

- `first_order_quote` as leveraged order notional.
- futures margin as `notional / leverage`.
- PnL, fees, funding, TP/SL and quantity from notional.
- capital usage and budget checks from margin.

Evidence:

- `apps/backtest-engine/src/martingale/kline_engine.rs:617-634`
  - `leg_notional_series(...)` builds order notional.
  - `margins = notional / leverage`.
- `apps/backtest-engine/src/martingale/kline_engine.rs:765-828`
  - `budget_rejection_reason` compares active capital plus next capital against global/symbol/direction/strategy margin caps.

So the old "notional as principal" bug is not the immediate issue here.

### B. The displayed LP DD is not a 1000U live-cap DD

The LP optimizer combines normalized candidate curves:

- `scripts/optimize_margin_v2_lp_portfolios.py:360-382`
  - `matrix = np.column_stack([row["equity"] ...])`
  - `equity = matrix @ weights`
  - `drawdowns = (peaks - equity) / peaks`

This is a recombination of already-persisted candidate curves. It is not a joint simulation with one shared 1000U live margin pool.

When saving the live portfolio, the script copies the source strategy configs and writes `portfolio_weight_pct`:

- `scripts/optimize_margin_v2_lp_portfolios.py:474-490`

It does not scale `sizing.multiplier.first_order_quote` down to a 1000U portfolio. The weight is metadata used by runtime budget allocation; it is not order-size scaling.

### C. Current final LP portfolios do not have 1000U set in DB

Read-only DB query showed:

```text
mp_margin_v2_lp_aggressive_20260626   max_global_budget_quote = NULL
mp_margin_v2_lp_balanced_20260626     max_global_budget_quote = NULL
mp_margin_v2_lp_conservative_20260626 max_global_budget_quote = NULL
```

Therefore, before any formal 1000U launch, GLM must explicitly set `portfolio_config.risk_limits.max_global_budget_quote = "1000"` on the final selected portfolio and rerun preflight.

### D. Static capital diagnostics for conservative current config

For `mp_margin_v2_lp_conservative_20260626`:

```text
strategy_count                                      16
full_planned_margin_unweighted                      144,436.6663 USDT
first_leg_margin_unweighted_total                   99.1667 USDT
runtime-style 1000U weighted-cap projected margin    627.1571 USDT
remaining under static projection                    372.8429 USDT
```

Interpretation:

- The 144,436U number explains why an uncapped/full-ladder denominator can make drawdown look small.
- The first cycle is small enough to start, but deeper legs are constrained by caps.
- The runtime-style weighted caps materially change behavior. A replay that ignores those caps is not equivalent to live.

### E. Claude's Step 5 replay misses the live weight-cap step

`apps/backtest-engine/src/bin/portfolio_budget_replay.rs:89-121`:

```rust
portfolio.risk_limits.max_global_budget_quote = Some(args.budget);
let result = run_kline_screening_with_funding(portfolio, &bars, &funding)?;
```

It sets the global cap but does not apply per-strategy caps from `portfolio_weight_pct`.

Live runtime does apply them:

- `apps/trading-engine/src/main.rs:1495-1506`
  - load `portfolio_config`
  - `apply_portfolio_weight_scaling(&mut config, &config_value)?`
- `apps/trading-engine/src/main.rs:1529-1534`
  - extracts weights and calls `apply_global_budget_allocations`
- `apps/trading-engine/src/martingale_budget.rs`
  - converts global budget times weight into per-strategy margin caps, floored at first-leg margin.

Therefore Claude's `46.90% DD` may be directionally useful, but it is not a final runtime-equivalent validation.

## Required Fix: Runtime-Parity Replay

### Step 1. Keep the system safe

Before coding or replaying:

```bash
cd /home/bumblebee/Project/grid_binance
docker compose -p grid-binance --env-file .worktrees/full-v1/.env -f deploy/docker/docker-compose.yml stop trading-engine || true
grep -n '^BINANCE_LIVE_MODE=' .env .worktrees/full-v1/.env
```

Both env files must show:

```text
BINANCE_LIVE_MODE=0
```

Do not place or cancel live orders in this phase.

### Step 2. Move budget allocation into shared code

The same logic must be used by:

- API confirm-start preflight.
- Trading runtime config loading.
- Backtest/replay validation.

Implement a shared helper, preferably in `apps/backtest-engine/src/martingale/capital.rs` or another shared module already usable by all three crates:

```text
apply_portfolio_weight_margin_caps(config, raw_portfolio_config_json)
```

Required behavior:

- Read per-strategy `portfolio_weight_pct` from the raw portfolio config JSON.
- If `config.risk_limits.max_global_budget_quote` is positive, set each strategy's `risk_limits.max_strategy_budget_quote`.
- Per-strategy cap = `global_margin_cap * weight_factor`, floored at first-leg margin.
- First-leg margin = `first_order_quote / leverage` for futures, `first_order_quote` for spot.
- Do not use notional as the cap floor.
- Preserve existing narrower strategy caps by taking `min(existing_cap, computed_cap)` unless the existing cap is empty/null.

Then replace duplicate logic in:

- `apps/trading-engine/src/martingale_budget.rs`
- `apps/trading-engine/src/main.rs`
- `apps/api-server/src/services/martingale_publish_service.rs` if needed
- `apps/backtest-engine/src/bin/portfolio_budget_replay.rs`

The goal is one source of truth, not three similar implementations.

### Step 3. Fix `portfolio_budget_replay`

Update `apps/backtest-engine/src/bin/portfolio_budget_replay.rs`:

1. Parse and keep the raw `portfolio_config` JSON.
2. Inject `max_global_budget_quote = budget`.
3. Apply the same `portfolio_weight_pct` per-strategy cap logic used by live runtime.
4. Print diagnostics:
   - `runtime_weight_caps_applied: true`
   - `global_margin_cap_quote`
   - `first_leg_margin_total_quote`
   - `full_series_margin_quote`
   - `budget_capped_projected_margin_quote`
   - `max_capital_used_quote`
   - per-strategy: strategy id, symbol, direction, weight, first-leg margin, effective cap, accepted static legs.
5. Break down budget rejections:
   - global budget exceeded
   - strategy budget exceeded
   - symbol budget exceeded
   - direction budget exceeded
6. Keep the real-principal metric:
   - `equity_on_budget(t) = budget + (sim_equity(t) - sim_initial_equity)`
   - annualized return uses `budget` as principal.
   - max drawdown uses peak-to-trough on `equity_on_budget`.
   - if `equity_on_budget <= 0` at any point, fail the gate.

Add a regression test proving replay/runtime cap parity:

```text
global cap 1000, strategy weights 18.041392% / 18.041392% / ...
=> replay config has the same per-strategy caps as trading runtime.
```

Also add the hard example:

```text
first_order_quote=250, leverage=5, global cap=50, weight=100%
=> first-leg margin=50, cap passes; notional=250 is diagnostic only.
```

### Step 4. Rerun fixed runtime-parity replay for all three final portfolios

Use the same date range and data:

```text
start_ms = 1672531200000   # 2023-01-01
end_ms   = 1780271999999   # 2026-05-31
interval = 1m
market data = data/market_data_full.db
funding data = data/funding_rates.db
budget = 1000U margin principal
```

Run all three:

```text
mp_margin_v2_lp_conservative_20260626
mp_margin_v2_lp_balanced_20260626
mp_margin_v2_lp_aggressive_20260626
```

Acceptance gates:

```text
conservative: annualized_return_pct > 50  and max_drawdown_pct <= 10
balanced:     annualized_return_pct > 90  and max_drawdown_pct <= 20
aggressive:   annualized_return_pct > 110 and max_drawdown_pct <= 30
```

Additional gates:

- `max_capital_used_quote <= 1000.00 + epsilon`
- no min-notional order failure
- no live/runtime unsupported indicator or TP/SL model
- no negative account-equity point under `budget + cum_pnl`
- result must include fees, slippage, funding and mark-to-market unrealized PnL
- replay config must include `max_global_budget_quote = 1000`
- replay must show `runtime_weight_caps_applied = true`

Write the report to:

```text
docs/superpowers/reports/2026-06-27-runtime-parity-1000u-replay-results.md
```

### Step 4A. Add a minimum-capital report before replay gates

Before accepting or rejecting any portfolio, compute three capital views:

1. **Natural unscaled planned margin**
   - Sum all selected live strategies' full-ladder margins with original `first_order_quote`.
   - This is the capital required if copied candidate ladders remain unscaled.

2. **LP-weighted planned margin**
   - Sum over candidate members: `candidate_full_margin * lp_weight`.
   - This is the nominal principal implied by normalized LP recombination.
   - It is a useful diagnostic, not sufficient live proof.

3. **Minimum exact-scaled executable principal**
   - For each member/internal strategy, compute the principal required so the scaled first order stays above exchange minimum notional.
   - Formula:

```text
required_principal >= exchange_min_notional * candidate_full_margin / (candidate_weight * first_order_quote)
```

   - The portfolio minimum is the max of all member/internal-strategy requirements.
   - Use real Binance filters per symbol if available. If offline, use a conservative default and label it.

Add these fields to reports and persisted summaries:

```text
natural_unscaled_planned_margin_quote
lp_weighted_planned_margin_quote
min_exact_scaled_executable_principal_quote
min_exact_scaled_bottleneck_symbol
min_exact_scaled_bottleneck_strategy_id
min_exact_scaled_bottleneck_first_order_quote
scale_to_1000_min_first_order_quote
scale_model_used_for_gate
```

If `min_exact_scaled_executable_principal_quote > 1000`, an exact proportional clone is not executable for a 1000U live account. GLM must then either:

- search a new 1000U-native portfolio with first orders large enough to trade, or
- explicitly validate a cap-truncated/hybrid model and accept that it is a different strategy from the displayed LP curve.

## Decision Tree After Fixed Replay

### Case A: All three existing LP portfolios pass

Then:

1. Update DB portfolio configs to set:
   - final selected portfolio only: `max_global_budget_quote = "1000"`
   - status remains `pending_confirmation` until the user confirms.
2. Update the flyingkid display:
   - show only the three final passed portfolios.
   - archive all older martingale portfolio/backtest display tasks.
3. Rebuild and restart only non-trading services needed for display/API.
4. Do not start formal live.
5. Prepare 50U smoke with the same fixed replay/runtime logic.

### Case B: One or more existing LP portfolios fail

Then:

1. Do not launch 1000U live.
2. Mark the failing LP result as invalid for real-capital launch in the report.
3. Re-optimize/search under the fixed runtime-parity gate.
4. Prioritize the new `<=5000U` small-capital-native search before declaring the target infeasible.

The search pipeline may still use LP recombination as a proposal generator, but no result may be persisted as final unless the fixed replay passes the mode gate.

Required search output:

- final candidate portfolio id
- annualized return
- max drawdown
- max capital used
- fees
- funding
- slippage
- blocked legs
- symbols
- strategy count
- proof that replay used runtime weight caps
- minimum executable principal
- scale model used for selection
- min-notional bottleneck if rejected

If no combination meets the thresholds, report why:

- best conservative found under DD<=10
- best balanced found under DD<=20
- best aggressive found under DD<=30
- best result by budget bucket: 500U, 1000U, 1500U, 2000U, 3000U, 5000U
- efficient frontier table
- whether the failure is caused by martingale inherent drawdown, budget caps, fees/funding, min-notional, or insufficient candidate pool

The report must distinguish:

- fails because risk/return is bad under a valid 1000U executable model,
- fails because exact scaling below 1000U violates exchange minimum order size,
- fails because fixed first-order plus 1000U cap truncates martingale legs and changes the original strategy.

### Case C: Fixed replay exposes a code mismatch

If replay and runtime still disagree:

1. Stop.
2. Add parity tests before more search.
3. Fix code until these match:
   - backtest leg notional/margin
   - runtime order quantity
   - runtime strategy cap
   - runtime global cap
   - TP/SL behavior
   - funding/fee accounting
   - mark-to-market equity

No live smoke until parity is proven.

## Re-optimization Requirements If Needed

If the fixed replay fails, update `scripts/optimize_margin_v2_lp_portfolios.py` or add a new script:

```text
scripts/optimize_margin_v2_runtime_parity_portfolios.py
scripts/search_small_capital_martingale_portfolios.py
```

Minimum requirements:

1. Candidate proposals can come from existing corrected margin-v2 candidates, but GLM must also generate new low-capital-native candidates.
2. Every proposed portfolio must be converted into executable `portfolio_config`.
3. `max_global_budget_quote` must be injected before validation for each searched budget.
4. Runtime weight caps must be applied before replay.
5. The final selection must be based on replay metrics, not only normalized LP metrics.
6. Persist only portfolios that pass the mode gate.
7. If parameters/order style change in search, immediately port the same behavior to live runtime and add parity tests.
8. Search must be 1000U-native, not only high-capital LP recombination:
   - exact-scaling portfolios must have `min_exact_scaled_executable_principal_quote <= 1000`,
   - cap-truncated/hybrid portfolios must pass the gate directly under that exact executable model,
   - no normalized LP-only result may be displayed as final unless executable 1000U replay also passes.
9. Candidate generation must include a low-capital/min-notional-aware branch:
   - fewer symbols if 8-symbol diversification forces dust-sized weights,
   - larger minimum per-symbol/member weight,
   - first-order sizes chosen so scaled orders stay above Binance min notional,
   - leg count/multiplier adjusted for 1000U capital,
   - explicit reject reasons when minimum executable principal exceeds 1000U.
10. Repeat the same native search for budgets up to 5000U. The user needs to know whether the targets are possible under any capital below 5000U, not only under 1000U.

For CPU utilization:

- Parallelize by portfolio proposal batches.
- Avoid one huge serial LP loop followed by one serial replay.
- Run independent replay jobs per candidate/proposal where possible.
- Log progress every few minutes: candidate count, current best ann/DD, rejected reason counts.

## 50U Smoke After Backtest Passes

Only after the fixed 1000U replay passes for the final conservative portfolio:

1. Create a dedicated 50U smoke portfolio, not the production portfolio.
2. Set `max_global_budget_quote = "50"`.
3. Use a tiny subset that still exercises:
   - long and short directions
   - first leg
   - at least one safety leg if possible within min-notional
   - TP order
   - SL/guard path in dry-run or controlled mode
   - fee/funding/stat persistence
4. Before enabling live mode:
   - Binance open orders = 0
   - Binance non-zero positions = 0
   - DB runtime positions/orders reconciled with Binance
   - stale SOL smoke state either reconciled or cleared only after Binance truth is verified
5. Run smoke with very small live exposure only after user confirmation.
6. Immediately flatten and stop after the smoke acceptance checklist.

Smoke pass criteria:

- no duplicate opening order
- no order outside budget
- quantities match `notional / price`
- margin accounting = `notional / leverage`
- TP/SL order placement matches backtest model
- fills are recorded exactly once
- fees and funding are persisted
- runtime positions match Binance positions
- open orders after stop are either expected reduce-only exits or zero

## Formal 1000U Live Launch Gate

Only after:

- fixed 1000U runtime-parity replay passes
- 50U smoke passes
- Binance account is clean or intentionally reconciled
- current code/images contain the fixes
- user explicitly confirms launch

Then GLM may prepare the formal conservative 1000U launch.

Before starting:

1. Confirm with the user again.
2. Set final conservative portfolio `max_global_budget_quote = "1000"`.
3. Confirm isolated margin and leverage for every symbol.
4. Confirm Multi-Assets mode off and Hedge mode on.
5. Confirm no conflicting positions/orders.
6. Start trading-engine only after final confirmation.

Monitoring for first 24 hours:

- open orders and positions every 1-5 minutes for first hour
- budget usage and per-strategy caps
- duplicate order detector
- TP/SL existence for each active cycle
- funding fee events
- realized/unrealized PnL
- DB/Binance reconciliation
- error logs from trading-engine and api-server

Emergency stop:

1. Stop trading-engine.
2. Cancel all non-reduce-only unexpected open orders.
3. Flatten only after confirming with the user unless an already-approved emergency rule applies.
4. Record Binance order ids, fills, fees, funding, DB order ids, and positions.
5. Do not restart until root cause is written down and fixed.

## Final Instruction To GLM

Do not continue from Claude's Step 5 directly into live smoke or 1000U live.

First fix the replay/runtime parity gap. The current 46.90% DD result is a warning, not a final answer. The original 10% LP display is also not a final answer for 1000U live. The only acceptable next result is a fixed runtime-parity replay report for all three modes under 1000U margin principal, followed by either:

- passed portfolios shown in flyingkid and prepared for smoke, or
- a new budget-aware search / infeasibility report.
