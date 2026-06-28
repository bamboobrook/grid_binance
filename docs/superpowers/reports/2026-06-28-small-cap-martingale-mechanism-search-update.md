# 2026-06-28 Small-Capital Martingale Mechanism Search Update

## Current Verdict

I continued the small-capital martingale search after the dynamic-symbol-count round.

Current validated status:

| Profile | Gate | Current Status |
|---|---:|---|
| Conservative | annualized > 50%, DD <= 10% | Not found |
| Balanced | annualized > 90%, DD <= 20% | Not found |
| Aggressive | annualized > 110%, DD <= 30% | Found previously: 133.54% / 29.88%, 3250U |

No live/Binance action was performed.

## Important Baseline Facts

Full-period candidate pool:

- source: `work/small_cap_search/full_period_candidates.csv.gz`
- candidates inspected: 1058
- symbols: 30

Single-strategy gates in that full candidate pool:

| Gate | Count |
|---|---:|
| ann > 50% and DD <= 10% | 0 |
| ann > 90% and DD <= 20% | 0 |
| ann > 110% and DD <= 30% | 0 |

Best single-strategy low-DD frontier:

- DD <= 10%: best annualized only 9.39%
- ann > 50%: lowest DD 31.23%
- ann > 90%: lowest DD 36.73%
- ann > 110%: lowest DD 38.27%

This means conservative and balanced cannot come from one strong single strategy. They require portfolio diversification to do nearly all the risk reduction.

## Paths Tested After Dynamic Symbol Count

### 1. Hidden Risk Guard Thresholds

Tested representative near-frontier portfolios with variants of:

- new-cycle drawdown pause
- new-cycle ATR pause
- ADX safety-leg skip
- ADX skip disabled

Result:

- conservative best remained around 59.67% / 24.63%
- balanced best remained around 91.47% / 27.45% or 98.20% / 27.93%
- aggressive stayed valid only near default or mild variants
- all important runs had `budget_blocked_legs = 0`

Conclusion: hidden guard thresholds are not the blocker and not the solution.

### 2. Portfolio Equity Stop + Cooldown

Implemented offline research switches:

- `MARTINGALE_BT_PORTFOLIO_EQUITY_STOP_PCT`
- `MARTINGALE_BT_PORTFOLIO_STOP_COOLDOWN_HOURS`

The implementation closes all active cycles using real close price, exit fee, and slippage. It is not a report-only fake stop.

Best representative results:

| Profile | Best Relevant Variant | Ann | DD |
|---|---|---:|---:|
| Conservative | stop20/cooldown72h | 9.89% | 11.24% |
| Balanced | fixed stop20/cooldown72h | 21.35% | 15.85% |
| Aggressive | stop20/cooldown72h | 30.33% | 19.62% |

Result: portfolio stop can reduce drawdown, but it destroys return. It does not meet any target.

### 3. Live-Supported Exit Parameter Tuning

Tested only parameters the current live path understands:

- fixed-percent take profit bps
- strategy drawdown stop bps
- fixed-percent spacing bps

Representative results:

| Profile | Closest Result | Ann | DD |
|---|---|---:|---:|
| Conservative | `sp150_tp075_sl075` | 59.00% | 22.68% |
| Conservative | baseline | 59.67% | 24.63% |
| Balanced | `sl050` | 91.48% | 27.43% |
| Balanced | baseline | 91.47% | 27.45% |

Low-DD variants existed, but return collapsed:

- conservative lower-DD examples were still above 13% DD and had negative annualized return.
- balanced DD under 20% had annualized return around -1% to 8%.

Conclusion: percent TP / strategy SL / spacing does not break the risk-return frontier.

### 4. Portfolio Max Active Cycle Limit

Implemented offline research switch:

- `MARTINGALE_BT_MAX_PORTFOLIO_ACTIVE_CYCLES`

This limits simultaneous active martingale cycles across the whole portfolio.

Representative results:

| Profile | Best Relevant Variant | Ann | DD |
|---|---|---:|---:|
| Conservative | max_active3_dd4 | 58.82% | 23.92% |
| Balanced | max_active4 | 89.07% | 27.98% |
| Aggressive | max_active4 | 132.07% | 30.25% |

Result: tight concurrency limits kill return; loose limits keep drawdown too high. No pass.

### 5. Model Family Check

The candidate pool includes:

- 699 percent TP + fixed spacing candidates
- 279 percent TP + ATR spacing candidates
- 80 ATR TP + fixed spacing candidates

Per-model single-strategy bests still do not reach gates:

- ATR TP + fixed spacing: `ann > 50` lowest DD was 48.67%
- percent TP + ATR spacing: `ann > 50` lowest DD was 32.57%
- percent TP + fixed spacing: `ann > 50` lowest DD was 31.23%

Non-percent TP remains a live parity concern: backtest supports ATR/trailing/mixed TP, but the main trading-engine exit detector currently only triggers percent TP in `martingale_exit_signal`. If future results rely on non-percent TP, live parity must be implemented before any launch.

## Current Best Valid Candidate

Only aggressive remains valid:

- config: `/tmp/codex_small_search/fixed_exposure_cash_priority_configs/0105_full_pool_b3000_top_12_fixed_cash_b3250.json`
- result: `/tmp/codex_small_search/fixed_exposure_cash_priority_results/0105_full_pool_b3000_top_12_fixed_cash_b3250.json`
- budget: 3250U
- annualized/DD: 133.54% / 29.88%
- symbols: AAVEUSDT, INJUSDT, LINKUSDT
- `budget_blocked_legs = 0`

## Interpretation

The current martingale family is showing a persistent frontier:

- DD <= 10% implies annualized return in single digits or low teens.
- annualized > 50% implies DD about 22%+ even after tuning.
- annualized > 90% implies DD about 27%+.
- annualized > 110% can fit DD <= 30% only in the aggressive fixed-exposure family.

This is not caused by budget truncation. Recent representative runs all had `budget_blocked_legs = 0`.

## Next Recommended Direction

If the targets are hard and capital must remain <=5000U, continuing to tune the same martingale mechanics is unlikely to find conservative/balanced.

The next meaningful options are:

1. Switch conservative/balanced to a different strategy family, or hybridize martingale with a non-averaging entry/exit model.
2. Add a new portfolio-level regime allocator that disables high-risk martingale exposure before adverse regimes, not after drawdown has already appeared.
3. If non-percent TP/ATR/trailing/mixed exits become the next research direction, first fix live parity in `trading-engine` so those exits are actually executable.
4. Keep aggressive candidate as the only current valid martingale portfolio, but do not display or launch it until the user accepts that conservative/balanced are not yet found.

## Artifacts

Recent result locations:

- `/tmp/codex_small_search/portfolio_stop_replays/summary.json`
- `/tmp/codex_small_search/exit_param_replays_cb/results/`
- `/tmp/codex_small_search/exit_param_replays_balanced/summary.json`
- `/tmp/codex_small_search/max_active_replays/summary.json`

Research-only local/remote switches added during this phase:

- `MARTINGALE_BT_NEW_CYCLE_DD_PAUSE_PCT`
- `MARTINGALE_BT_NEW_CYCLE_ATR_PAUSE_PCT`
- `MARTINGALE_BT_SAFETY_SKIP_ADX`
- `MARTINGALE_BT_PORTFOLIO_EQUITY_STOP_PCT`
- `MARTINGALE_BT_PORTFOLIO_STOP_COOLDOWN_HOURS`
- `MARTINGALE_BT_MAX_PORTFOLIO_ACTIVE_CYCLES`

These switches are research-only. They are not yet formal shared config and are not live parity deliverables.
