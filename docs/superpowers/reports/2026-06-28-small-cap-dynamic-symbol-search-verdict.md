# 2026-06-28 Small-Capital Dynamic-Symbol Martingale Search Verdict

## Executive Summary

User hypothesis tested: for <=5000U principal, dynamically reduce portfolio symbol count, e.g. <=5000U use <=5 symbols, so each symbol has enough margin and the runtime can execute without truncating martingale ladders.

Verdict:

- The hypothesis is valid for executability: reducing symbols and adding cash buffer can make `budget_blocked_legs = 0`.
- It is not sufficient to satisfy all target gates under the current martingale family.
- Runtime-parity replay confirms an aggressive portfolio is available under <=5000U.
- Conservative and balanced targets are still not found under <=5000U after the new search.

Required gates:

| Profile | Required | Best valid result found in this round | Status |
|---|---:|---:|---|
| Conservative | ann > 50%, DD <= 10% | 8.65% / 7.05% under DD<=10; 58.68% / 18.82% if ann>50 | FAIL |
| Balanced | ann > 90%, DD <= 20% | 98.20% / 27.93% if ann>90; 58.68% / 18.82% under DD<=20 | FAIL |
| Aggressive | ann > 110%, DD <= 30% | 133.54% / 29.88% (`AAVEUSDT,INJUSDT,LINKUSDT`, budget 3250U) | PASS |

No live/Binance action was taken.

## What Was Run

### 1. Dynamic symbol-count curve search

Local scripts/results:

- `work/small_cap_search/optimize_low_cap_portfolios_dynbudget.py`
- `work/small_cap_search/dynbudget_combined_replay_search.json`
- `work/small_cap_search/dynbudget_combined_replay_search.md`
- `work/small_cap_search/dynbudget_safe_replay_search.json`
- `work/small_cap_search/dynbudget_safe_replay_search.md`

Budget rules tested:

- 1000U: max 3 symbols
- 3000U: max 4 symbols
- 5000U: max 5 symbols

Candidate pool:

- input: full-period candidates from `/tmp/codex_small_search/full_period_candidates.csv.gz`
- candidate count used by expanded search: 503
- full period: 2023-01-01 to 2026-05-31, 1m
- exchange minimum notional: 5 USDT

Curve-combination search can look optimistic. Therefore all useful candidates were converted to replay configs and validated by `portfolio_budget_replay`.

### 2. Runtime-parity replay of safe dynamic-symbol configs

Remote configs/results:

- configs: `/tmp/codex_small_search/dynbudget_safe_replay_configs/`
- results: `/tmp/codex_small_search/dynbudget_safe_replay_results/`
- summary: `/tmp/codex_small_search/dynbudget_safe_replay_results/summary_frontier.json`

162 runtime-parity replays were run. All were configured with safer utilization variants to avoid cap truncation. Result:

- conservative passes: 0
- balanced passes: 0
- aggressive passes: 0 in this specific safe-dynamic branch
- all important frontier rows had `budget_blocked_legs = 0`

Best runtime-parity frontiers from this branch:

| Frontier | Budget | Ann | DD | Symbols | Blocked |
|---|---:|---:|---:|---|---:|
| best_under_dd10 | 1000 | 8.65% | 7.05% | BTC,DOGE,EGLD | 0 |
| best_under_dd20 | 1000 | 58.68% | 18.82% | AAVE,FIL,INJ | 0 |
| best_under_dd30 | 5000 | 105.48% | 29.18% | INJ,LINK,TRX,XRP | 0 |
| lowest_dd_over_ann90 | 5000 | 98.20% | 27.93% | INJ,LINK,TRX,XRP | 0 |
| lowest_dd_over_ann110 | 1000 | 112.59% | 33.72% | FIL,INJ,LINK | 0 |

Interpretation: dynamic symbol count and safe margin allocation solve budget truncation, but the risk/return frontier still misses balanced and aggressive gates in this branch.

### 3. Reconfirmed existing fixed-exposure aggressive pass

Remote results:

- configs: `/tmp/codex_small_search/fixed_exposure_cash_priority_configs/`
- results: `/tmp/codex_small_search/fixed_exposure_cash_priority_results/`
- summary: `/tmp/codex_small_search/fixed_exposure_cash_priority_results/summary_frontier.json`

Valid aggressive pass examples:

| Name | Budget | Ann | DD | Symbols | Blocked |
|---|---:|---:|---:|---|---:|
| `0105_full_pool_b3000_top_12_fixed_cash_b3250` | 3250 | 133.54% | 29.88% | AAVE,INJ,LINK | 0 |
| `0105_full_pool_b3000_top_12_fixed_cash_b3500` | 3500 | 128.82% | 29.57% | AAVE,INJ,LINK | 0 |
| `0105_full_pool_b3000_top_12_fixed_cash_b4000` | 4000 | 120.59% | 28.97% | AAVE,INJ,LINK | 0 |
| `0105_full_pool_b3000_top_12_fixed_cash_b4500` | 4500 | 113.63% | 28.39% | AAVE,INJ,LINK | 0 |
| `0105_full_pool_b3000_top_12_fixed_cash_b4750` | 4750 | 110.53% | 28.11% | AAVE,INJ,LINK | 0 |

This is the best currently validated <=5000U aggressive family.

Closest balanced-like fixed-exposure frontier:

| Name | Budget | Ann | DD | Symbols | Blocked |
|---|---:|---:|---:|---|---:|
| `0087_full_pool_b1500_top_15_fixed_cash_b3250` | 3250 | 91.47% | 27.45% | AAVE,INJ,LINK | 0 |
| `0100_full_pool_b1500_top_21_fixed_cash_b3250` | 3250 | 91.08% | 27.58% | BTC,INJ,LINK | 0 |
| `0105_full_pool_b3000_top_12_fixed_cash_b5000` | 5000 | 107.65% | 27.83% | AAVE,INJ,LINK | 0 |

These fail balanced because drawdown remains far above 20%.

Closest conservative-like fixed-exposure frontier:

| Name | Budget | Ann | DD | Symbols | Blocked |
|---|---:|---:|---:|---|---:|
| `0056_full_pool_b1500_top_10_fixed_cash_b5000` | 5000 | 59.67% | 24.63% | INJ,XRP | 0 |

This fails conservative because drawdown remains far above 10%.

## Important Finding About Dynamic Symbol Count

Dynamic symbol count helps with small-capital executability:

- fewer symbols means each strategy receives enough first-order and ladder margin;
- safe utilization avoids `budget_blocked_legs`;
- this directly addresses the user's concern that <=5000U cannot support too many simultaneous symbols.

But it does not change martingale's core drawdown frontier:

- When DD <=10%, annualized return stayed around 6-9% in runtime replay.
- When annualized return exceeded 50%, DD was about 18-25% or higher.
- When annualized return exceeded 90%, DD was about 27-38% or higher.
- When annualized return exceeded 110%, DD was about 28-44% depending on family; only the fixed-exposure aggressive family is inside DD<=30.

So the blocker is not primarily symbol count anymore. The blocker is the current martingale strategy family's risk/return shape.

## Invalid/Discarded Attempt

I briefly generated `/tmp/codex_small_search/fixed_exposure_dense_budget_configs` by directly modifying global budget on existing fixed-exposure configs. This was stopped because it caused `budget_blocked_legs > 0` on lower budget points. Those partial results must not be used as final evidence.

The related stray replay processes were killed and verified stopped.

## Current Best Valid Portfolio Candidates

Only aggressive is currently valid:

1. Aggressive candidate A:
   - config: `/tmp/codex_small_search/fixed_exposure_cash_priority_configs/0105_full_pool_b3000_top_12_fixed_cash_b3250.json`
   - result: `/tmp/codex_small_search/fixed_exposure_cash_priority_results/0105_full_pool_b3000_top_12_fixed_cash_b3250.json`
   - budget: 3250U
   - ann/DD: 133.54% / 29.88%
   - symbols: AAVEUSDT, INJUSDT, LINKUSDT
   - blocked: 0

2. Aggressive lower-return lower-DD variant:
   - config: `/tmp/codex_small_search/fixed_exposure_cash_priority_configs/0105_full_pool_b3000_top_12_fixed_cash_b4750.json`
   - result: `/tmp/codex_small_search/fixed_exposure_cash_priority_results/0105_full_pool_b3000_top_12_fixed_cash_b4750.json`
   - budget: 4750U
   - ann/DD: 110.53% / 28.11%
   - symbols: AAVEUSDT, INJUSDT, LINKUSDT
   - blocked: 0

No conservative or balanced final candidate should be displayed or prepared for live yet.

## Next Recommended Direction

If the original gates are hard and capital must stay <=5000U, continuing to only adjust symbol count is unlikely to find conservative/balanced. The next research direction must change strategy mechanics, for example:

1. Add explicit portfolio-level drawdown throttle / cooldown as a configurable strategy parameter, not a hidden hard-coded guard.
2. Add regime filters that reduce stop churn without killing trade frequency. The earlier hard RSI/BB filters were too restrictive, so use softer trend/volatility filters or adaptive cooldown instead.
3. Explore non-martingale or hybrid strategy family for conservative/balanced. Martingale averaging-down naturally has high peak-to-trough drawdown.
4. Keep the validation rule strict: final acceptance only from `portfolio_budget_replay` with `budget_blocked_legs = 0`, `principal_breached = false`, and max capital used <= budget.

Do not proceed to flyingkid display or live validation until all three final portfolios exist. At present, only aggressive exists.
