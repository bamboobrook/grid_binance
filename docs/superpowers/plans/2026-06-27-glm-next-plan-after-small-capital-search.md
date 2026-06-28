# 2026-06-27 GLM Next Plan After Small-Capital Search

## Current Decision

Do not launch live trading.

The latest search did not find conservative, balanced, or aggressive portfolios that satisfy:

- Conservative: annualized > 50%, max DD <= 10%
- Balanced: annualized > 90%, max DD <= 20%
- Aggressive: annualized > 110%, max DD <= 30%
- margin principal <= 5000U
- runtime-parity replay, full period, fees/slippage/funding included

The closest aggressive result was `111.90% / 32.44%`, which still fails the DD gate.

## What GLM Should Do First

Read:

- `docs/superpowers/reports/2026-06-27-small-capital-martingale-search-verdict.md`
- `/tmp/codex_small_search/final_summary.json`
- `/tmp/codex_small_search/near_miss_results/*.json`
- `/tmp/codex_small_search/direction_full_validation.json`

Confirm no leftover offline processes:

```bash
ps -eo pid,pcpu,etime,args | grep -E '[s]earch_small_capital_martingale|[p]ortfolio_budget_replay'
```

No live actions are authorized.

## If Continuing Search

Do not use LP-only normalized curve recombination as final proof. It may be used only as a proposal generator.

Every candidate must pass:

```bash
target/release/portfolio_budget_replay \
  --config <candidate.json> \
  --budget <principal> \
  --start-ms 1672531200000 \
  --end-ms 1780271999999 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --profile conservative|balanced|aggressive \
  --portfolio-id <id> \
  --exchange-min-notional 5
```

Acceptance requires:

- `gate.passed = true`
- `budget_blocked_legs = 0` or explicitly accepted as a cap-truncated strategy
- `on_budget.principal_breached = false`
- `on_max_capital_used.max_capital_used_quote <= budget`
- no live-unsupported indicator, TP, SL, or sizing model

## Required Search Architecture

The previous bottleneck was single-core full-cycle brute force. Use a two-stage and parallel approach:

1. Run short-window probes in parallel by symbol and parameter family.
2. Require candidates to pass at least two separated windows, not only 2023H1.
3. Full-cycle replay only the candidates that survive window robustness.
4. Store each full-cycle replay result JSON.
5. Build the final report from replay outputs, not from screening metrics.

Suggested windows:

- 2023H1: `1672531200000..1688169600000`
- 2024H1: `1704067200000..1719792000000`
- 2025H1: `1735689600000..1751328000000`
- full: `1672531200000..1780271999999`

## Search Space Worth Trying

The current exploratory tool was extended:

```bash
target/release/search_small_capital_martingale \
  --direction-modes long_only,short_only,long_and_short \
  --entry-filters none,trend,trend_rsi
```

But do not run massive full-cycle grids directly. Indicator expressions can be expensive. Use short-window prefiltering first.

Potential filters supported in both backtest and live runtime:

- `close > ema(200)` for long trend
- `close < ema(200)` for short trend
- `rsi(14) < 65` for long overheat guard
- `rsi(14) > 35` for short overheat guard
- `adx(14) > N`

Because `indicator_expression` supports only one comparison per trigger, use multiple trigger entries to express AND.

## What Not To Do

- Do not start 50U smoke or 1000U live until a full-cycle runtime-parity candidate passes.
- Do not present short-window H1 candidates as final. The best H1 candidates failed full-cycle badly.
- Do not present LP recombination curves as live-executable proof.
- Do not force the existing high-capital LP portfolios into <=5000U by cap truncation.

## Reporting Requirements

If no target passes after the next bounded search, write an infeasibility report with:

- best annualized return under DD <=10/20/30
- lowest DD above ann >50/90/110
- best result per budget bucket: 500, 1000, 2000, 3000, 5000
- distinction between:
  - failed because exact-scaled runtime replay risk/return is bad
  - failed because min-notional makes scaling impossible
  - failed because cap truncation changes the strategy
  - failed because short-window overfit did not survive full period

## Live/Smoke Plan

Only if a candidate passes all gates:

1. Write the final executable `portfolio_config`.
2. Run `portfolio_budget_replay` one more time from the saved config.
3. Add a preflight record showing margin principal, min notional, weight caps, fees/slippage/funding, and live-supported indicators.
4. Ask the user for explicit confirmation before any smoke.
5. Start with 50U smoke only if the same strategy geometry is executable at 50U; otherwise state that 50U smoke is not representative.
6. Ask the user again before any 1000U formal live run.
