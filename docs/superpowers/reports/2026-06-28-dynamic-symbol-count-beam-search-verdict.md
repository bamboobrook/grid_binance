# 2026-06-28 Dynamic Symbol Count Beam Search Verdict

## Question

Can we make the small-principal martingale portfolios work by dynamically reducing the number of symbols when principal is below 5000U? For example, use only about 5 symbols below 5000U.

## Short Verdict

Dynamic symbol count is necessary for executability, but it is not sufficient to meet the current conservative and balanced gates with the existing candidate pool.

It helps because small principal cannot be split across too many symbols without hitting exchange minimum order constraints and first-order scaling floors. However, the current failure is mostly a risk-return frontier problem, not just a symbol-count problem.

## Gates

| Profile | Required Gate |
|---|---:|
| Conservative | annualized > 50%, DD <= 10% |
| Balanced | annualized > 90%, DD <= 20% |
| Aggressive | annualized > 110%, DD <= 30% |

## Search Performed

Artifacts:

- `work/small_cap_search/optimize_low_cap_frontier_beam.py`
- `work/small_cap_search/frontier_beam_smoke.json`
- `work/small_cap_search/frontier_beam_smoke.md`
- `work/small_cap_search/frontier_beam_5000_max5.json`
- `work/small_cap_search/frontier_beam_5000_max5.md`
- `work/small_cap_search/frontier_beam_search.json`
- `work/small_cap_search/frontier_beam_search.md`

Search setup:

- Input: `work/small_cap_search/full_period_candidates.csv.gz`
- Candidate pool after filters: 503 candidates from the full-period pool
- Budgets tested: 1000U, 3000U, 5000U
- Dynamic member limits:
  - 1000U: up to 4 members
  - 3000U: up to 6 members
  - 5000U: up to 8 members
- Per-symbol caps tested: 20%, 25%, 30%, 35%, 45%
- A separate 5000U max-5-members search was also run to directly test the user's suggested 5-symbol direction.

## Result Summary

### Best Results By Budget

| Budget | Profile | Best annualized under DD gate | Lowest DD over return target |
|---:|---|---:|---:|
| 1000U | Conservative | 16.91% / 8.46% DD | 62.19% / 28.63% DD |
| 1000U | Balanced | 66.99% / 19.99% DD | 90.27% / 26.54% DD |
| 1000U | Aggressive | 122.52% / 29.98% DD | 110.83% / 28.25% DD |
| 3000U | Conservative | 17.66% / 8.30% DD | 65.99% / 23.85% DD |
| 3000U | Balanced | 65.10% / 19.98% DD | 95.03% / 27.25% DD |
| 3000U | Aggressive | 127.94% / 29.95% DD | 112.98% / 27.08% DD |
| 5000U | Conservative | 23.60% / 9.21% DD | 54.89% / 23.13% DD |
| 5000U | Balanced | 67.81% / 18.99% DD | 90.83% / 26.01% DD |
| 5000U | Aggressive | 128.03% / 29.97% DD | 111.22% / 27.74% DD |

### Direct 5000U Max-5 Test

When forcing 5000U to at most 5 members:

- Conservative best under 10% DD: 19.27% / 9.99% DD
- Balanced best under 20% DD: 66.60% / 19.98% DD
- Aggressive passes: 124.18% / 29.89% DD

This means reducing to 5 members improves executability but does not solve conservative/balanced.

## Interpretation

The user's intuition is directionally correct:

1. Smaller principal should use fewer symbols or fewer strategy members.
2. Otherwise the account budget is split too thinly and some legs become non-tradable or unrealistic.
3. The backtest/live system should eventually encode a minimum principal and dynamic member-count rule.

But the current search shows the main blocker is not just too many symbols. With the existing candidate curves:

- Conservative can keep DD <= 10%, but return collapses to about 17-24%.
- Conservative can exceed 50% annualized, but DD rises to about 23%+.
- Balanced can keep DD <= 20%, but return is only about 65-68%.
- Balanced can exceed 90% annualized, but DD rises to about 26%+.
- Aggressive is feasible under several small-principal configurations.

Therefore, current conservative and balanced failures are caused by the available martingale candidate frontier. They require either better single-strategy candidates, more regime-aware entries/exits, or a different portfolio allocator that avoids correlated drawdowns before they occur.

## Next Search Direction

Do not spend more time only tuning member count on this candidate pool.

Recommended next steps:

1. Keep dynamic member count as a hard executability rule:
   - <=1000U: max 3-4 members
   - <=3000U: max 4-6 members
   - <=5000U: max 5-8 members depending on first-order floors
2. Generate a new candidate pool aimed specifically at low DD:
   - lower martingale depth
   - smaller multipliers
   - stronger entry filters
   - regime filters that avoid high-correlation adverse periods
   - live-supported exit types unless trading-engine parity is implemented first
3. Only after conservative and balanced have theoretical candidates near the gate should we spend runtime replay resources.
4. Any final candidate still must pass `portfolio_budget_replay` with:
   - gate passed
   - `budget_blocked_legs = 0`
   - max capital used <= principal budget
   - no principal breach
   - live-supported indicator and order semantics

## Current Status

No final three-portfolio set exists yet.

Valid current small-principal profile:

- Aggressive only.

Missing:

- Conservative >50% annualized and DD <=10%
- Balanced >90% annualized and DD <=20%

No live or Binance action was performed.
