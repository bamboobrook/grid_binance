# 2026-06-27 Small-Capital Martingale Search Verdict

## Verdict

No <=5000U martingale portfolio was found that passes the required runtime-parity gates:

| profile | required gate | best verified result found | verdict |
|---|---:|---:|---|
| Conservative | ann > 50%, DD <= 10% | no verified DD<=10 portfolio above 50%; direction-mode validation had no DD<=10 survivor | FAIL |
| Balanced | ann > 90%, DD <= 20% | representative replay best under DD<=20 was 16.96% / 14.36%; LP frontier replay reached 71.94% / 41.50% | FAIL |
| Aggressive | ann > 110%, DD <= 30% | closest near-miss: 111.90% / 32.44%; 117.23% / 34.03% | FAIL |

Do not launch live trading from these results. The failure is not a budget-cap bug: the closest replays had `budget_blocked_legs = 0`, so the strategies were not failing because the runtime cap clipped their ladders. The blocker is the risk/return shape of the candidate family under full-cycle real-principal replay.

## What Was Tested

All validation used the corrected margin-principal model and `portfolio_budget_replay` runtime-parity path:

- principal denominator = margin capital, not leveraged notional
- `max_global_budget_quote` injected as margin principal
- `portfolio_weight_pct` converted into per-strategy margin caps
- fees, slippage, funding and mark-to-market equity included
- full period: `2023-01-01` to `2026-05-31`, 1m data
- exchange minimum notional = 5 USDT

### Existing Candidate Combination Search

I exported low-cap and full-period candidate curves from `backtest_candidate_summaries`, then ran offline exact-scaled portfolio optimization.

Optimistic curve recombination looked better than live-equivalent replay, so every promising result was converted into an executable portfolio config and replayed.

Representative runtime-parity replay results:

| candidate | budget | ann | DD | blocked legs | pass |
|---|---:|---:|---:|---:|---|
| `budget_5000_lowest_dd_over_ann50` | 5000 | 16.96% | 14.36% | 0 | no |
| `budget_3000_best_under_dd20` | 3000 | 76.07% | 43.80% | 0 | no |
| `budget_5000_best_under_dd20` | 5000 | 71.94% | 41.50% | 0 | no |
| `budget_1000_lowest_dd_over_ann110` | 1000 | 82.09% | 34.23% | 0 | no |
| prior low-cap near best | 1000 | 104.79% | 31.48% | 0 | no |

Important finding: normalized LP/equity-curve recombination can show low drawdown, but real joint replay with simultaneous strategies has much higher drawdown. Final selection must be based on replay, not LP-only curves.

### Direction-Mode Search

I extended `apps/backtest-engine/src/bin/search_small_capital_martingale.rs` to support:

- `--direction-modes long_only,short_only,long_and_short`
- optional `--entry-filters none,trend,trend_rsi` (compiled; only lightly probed)

Short-window 2023H1 results produced many attractive `long_only` / `short_only` candidates, but full-cycle replay showed severe overfit. The 36 best short-window candidates all failed full-cycle replay.

Direction-mode full-cycle frontier:

| constraint | best full-cycle result |
|---|---:|
| DD <= 10% | none |
| DD <= 20% | none |
| DD <= 30% | 31.13% / 28.64% (BTC long-only) |
| ann > 50% | none |
| ann > 90% | none |
| ann > 110% | none |

### Aggressive Near-Miss Weight Search

The closest candidate family was a 3-member 1000U portfolio based on XRP / INJ / DOGE. I ran 30 runtime-parity replays around nearby weights and cap buffers.

Best near misses:

| candidate | ann | DD | blocked legs | max capital used | trades | verdict |
|---|---:|---:|---:|---:|---:|---|
| `xrp_low_u985c1008` | 111.90% | 32.44% | 0 | 927.34 | 6602 | fails DD |
| `inj_up3_u985c1008` | 117.23% | 34.03% | 0 | 886.81 | 6982 | fails DD |
| `inj_heavy_u985c1008` | 118.96% | 39.93% | 0 | 884.29 | 6402 | fails DD |
| `base_u985c1008` | 104.79% | 31.48% | 0 | 888.52 | 7796 | fails ann and DD |

This confirms the trade-off: increasing INJ/high-return exposure can push annualized return above 110%, but drawdown rises above 30%; lowering drawdown pulls annualized return below 110%.

## Current Code/Artifacts

Uncommitted files on the WSL repo:

- `apps/backtest-engine/src/bin/search_small_capital_martingale.rs`
  - exploratory/offline search tool, now supports direction modes and entry filter enum
  - not used by live trading
- `scripts/validate_small_candidates.py`
  - validation helper from earlier work
- `docs/superpowers/plans/2026-06-27-glm-step5-nogo-verdict-and-next-plan.md`

Temporary result files are under `/tmp/codex_small_search/`, especially:

- `/tmp/codex_small_search/final_summary.json`
- `/tmp/codex_small_search/direction_full_validation.json`
- `/tmp/codex_small_search/near_miss_results/*.json`
- `/tmp/codex_small_search/full_pool_replay_results_buffered/*.json`
- `/tmp/codex_small_search/replay_results_buffered/*.json`

No live services were started and no Binance orders were touched.

## Interpretation

The user's intuition was right that a valid scalable strategy should preserve percentages when capital changes. The current failures are not from using notional as principal anymore; that bug was already corrected. The issue now is that:

1. Old high-capital LP portfolios cannot be scaled to <=5000U without changing execution geometry.
2. Exact-scaled low-capital candidates still have martingale drawdowns above the required caps.
3. Short-window high-return candidates are regime-overfit and fail full-cycle replay.
4. Normalized curve recombination understates drawdown versus real simultaneous replay.

At this point, the evidence does not support claiming that the requested targets are achievable with the current martingale family under <=5000U capital.

## Recommendation

Do not display any new final portfolio in `flyingkid` and do not proceed to smoke/live trading.

If the user insists on continuing the same strategy family, the next work should be a larger, replay-first research job, not live preparation:

- parallel candidate generation over multiple windows
- out-of-sample gating before full-cycle replay
- final selection only from runtime-parity replay
- accept that the likely reachable aggressive frontier is around ann 100-120% with DD 32-40%, or ann 60-80% with DD around 20-30%

If DD <=10/20/30 is hard, switch away from martingale averaging-down to a different strategy family.
