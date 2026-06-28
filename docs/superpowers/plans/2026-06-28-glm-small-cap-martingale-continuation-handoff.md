# 2026-06-28 GLM Small-Capital Martingale Continuation Handoff

## Purpose

This document hands off the current small-capital martingale research state to GLM.

User's hard objective remains:

| Profile | Required full-period result |
|---|---:|
| Conservative | annualized return > 50%, max drawdown <= 10% |
| Balanced | annualized return > 90%, max drawdown <= 20% |
| Aggressive | annualized return > 110%, max drawdown <= 30% |

Preferred deployable principal is <= 5000 USDT. "Principal" means margin capital, not leveraged notional. For example, with 5x leverage, 50U principal can support about 250U notional exposure only if the ladder margin sum stays within 50U.

Do not perform any Binance, live trading, smoke trading, flyingkid display, or real-account action while continuing this exploration. The current task is offline research only.

## Non-Negotiable Acceptance Standard

A final portfolio is valid only if it passes full-period runtime-parity replay:

- Binary: `target/release/portfolio_budget_replay`
- Period: `2023-01-01` through `2026-05-31`
- Data: 1m market data plus funding rates
- Metrics: use `on_budget` metrics, rebased to margin principal
- `budget_blocked_legs = 0`
- `principal_breached = false`
- `max_capital_used_quote <= budget_quote`
- Exchange minimum notional is respected, currently 5 USDT
- Fees, slippage, and funding are included
- Backtest mechanics must exist in live/runtime before any launch candidate is treated as deployable

Do not accept any theoretical curve-combination result, single-window result, or search summary as final proof.

## Current Work Results

### Margin/principal model

The corrected model is:

- `first_order_quote` is order notional.
- Per-leg margin is `leg_notional / leverage`.
- Planned margin is the sum of every ladder leg margin.
- Portfolio budget is margin principal.
- Annualized return and max drawdown must be calculated on the margin principal budget, not on leveraged notional.

Do not revert this model.

### Current validated status

| Profile | Current status |
|---|---|
| Conservative | Not found |
| Balanced | Not found |
| Aggressive | Found, but only aggressive |

Known aggressive runtime-parity candidate:

- Config: `/tmp/codex_small_search/fixed_exposure_cash_priority_configs/0105_full_pool_b3000_top_12_fixed_cash_b3250.json`
- Result: `/tmp/codex_small_search/fixed_exposure_cash_priority_results/0105_full_pool_b3000_top_12_fixed_cash_b3250.json`
- Budget: 3250U
- Symbols: `AAVEUSDT, INJUSDT, LINKUSDT`
- Annualized/DD: about `133.54% / 29.88%`
- `budget_blocked_legs = 0`

Do not display or launch this alone. The user asked for the final three portfolios together.

### Dynamic symbol/member count verdict

The user's idea is directionally correct:

- Small principal cannot support too many symbols or members.
- Reducing member count is necessary for executability.
- Recommended budget rule for future searches:
  - `<= 1000U`: 1-3 members
  - `1000U-3000U`: 2-4 members
  - `3000U-5000U`: 3-5 members

But current evidence shows this is not sufficient by itself.

Important observed full-period frontiers:

- Conservative under DD <= 10%: best seen roughly `17%-24%` annualized, still far below `>50%`.
- Conservative over annualized > 50%: drawdown rises to roughly `18%-25%+`.
- Balanced under DD <= 20%: best seen roughly `58%-68%` annualized, still below `>90%`.
- Balanced over annualized > 90%: drawdown rises to roughly `26%-28%+`.
- Aggressive can pass around `110%-133%` annualized with DD below or near `30%`.

Therefore dynamic member count should remain a hard search constraint, but it is not the final solution.

### Full-period candidate pool facts

From `work/small_cap_search/full_period_candidates.csv.gz`:

- Candidate count inspected: 1058
- Symbols: 30
- Single-strategy pass count:
  - Conservative: 0
  - Balanced: 0
  - Aggressive: 0

Single-strategy frontier:

- DD <= 10%: best annualized about `9.39%`
- DD <= 20%: best annualized about `33.39%`
- DD <= 30%: best annualized about `47.31%`
- Annualized > 50%: lowest DD about `31.23%`
- Annualized > 90%: lowest DD about `36.73%`
- Annualized > 110%: lowest DD about `38.27%`

This means conservative and balanced cannot be solved by picking one existing strong strategy. They require either new better candidates or a genuinely better portfolio/regime allocator.

## Paths Already Tried

Do not repeat these blindly.

### 1. Dynamic member count and safe budget replay

Artifacts:

- `docs/superpowers/reports/2026-06-28-dynamic-symbol-count-beam-search-verdict.md`
- `docs/superpowers/reports/2026-06-28-small-cap-dynamic-symbol-search-verdict.md`
- `/tmp/codex_small_search/dynamic5_safe_replay_results/summary_frontier.json`
- `/tmp/codex_small_search/dynamic_member_results/summary_frontier.json`

Result:

- Solved some executability/budget-blocking issues.
- Did not find conservative or balanced.
- Aggressive only.

### 2. Hidden guard threshold probes

Artifacts:

- `/tmp/codex_small_search/guard_probe_v2/conservative_guard.json`
- `/tmp/codex_small_search/guard_probe_v2/balanced_guard.json`
- `/tmp/codex_small_search/guard_probe_v2/anchor_guard.json`
- Summary helper: `/tmp/codex_small_search/summarize_guard_probe.py`

Results:

- `conservative_guard`: 800 rows, 0 passes. Best conservative-under-DD result was only about `11.58% / 7.89%`.
- `balanced_guard`: 1000 rows, 0 passes. Best balanced-under-DD result was only about `25.73% / 17.73%`.
- `anchor_guard`: 1000 rows, 0 passes. Higher-return rows had very high drawdown.

These guard knobs are research-only unless implemented as formal shared config and live runtime behavior.

Backtest-only env switches currently include:

- `MARTINGALE_BT_NEW_CYCLE_DD_PAUSE_PCT`
- `MARTINGALE_BT_NEW_CYCLE_ATR_PAUSE_PCT`
- `MARTINGALE_BT_SAFETY_SKIP_ADX`
- `MARTINGALE_BT_PORTFOLIO_EQUITY_STOP_PCT`
- `MARTINGALE_BT_PORTFOLIO_STOP_COOLDOWN_HOURS`
- `MARTINGALE_BT_MAX_PORTFOLIO_ACTIVE_CYCLES`

Do not use results depending on these as deployable without live parity.

### 3. Portfolio equity stop / cooldown

Result:

- Can reduce drawdown.
- Destroys return.
- Did not meet any target.

### 4. TP / SL / spacing tuning with live-supported mechanics

Result:

- Conservative stayed around `59% / 22%+` in the relevant high-return branch.
- Balanced stayed around `91% / 27%+`.
- Lower drawdown variants lost too much return.

### 5. Max active portfolio cycles

Result:

- Tight concurrency limits reduce return too much.
- Loose limits leave drawdown too high.
- No pass.

### 6. Window-filter / robust-parameter search

Artifacts:

- `docs/superpowers/reports/2026-06-28-small-cap-window-filter-search-progress.md`
- `/tmp/codex_small_search/window_filter_summary.json`
- `/tmp/codex_small_search/window_dynamic_portfolio_results_probe/summary_live.json`
- `/tmp/codex_small_search/window_anchor_portfolio_results_*`

Important caveat:

- Single-window 2023H1 candidates look extremely strong.
- Full-period replay invalidated representative examples.
- Do not accept any single-window candidate as final.

Some window-derived dynamic/anchor portfolio replays were run. They did not produce conservative, balanced, or aggressive passes in those branches.

## Interpretation

The current blocker is not only capital size.

If min-notional constraints and budget caps are respected, changing 1000U to 5000U mostly scales the position. If percentage metrics change drastically, it usually means one of these changed:

- which candidates are executable,
- first-order scaling floor,
- budget cap truncation,
- member count,
- per-strategy weight cap,
- or exchange minimum notional behavior.

The strongest current finding is that the existing martingale candidate family has a persistent frontier:

- Low DD implies low return.
- High return implies DD too high for conservative/balanced.
- Aggressive is feasible.

## Recommended Next Exploration

### Phase A: Baseline sanity before new runs

Run:

```bash
cd /home/bumblebee/Project/grid_binance
git status --short
cargo build -p backtest-engine --bin portfolio_budget_replay --bin search_small_capital_martingale --release --quiet
python3 /tmp/codex_small_search/summarize_guard_probe.py | sed -n '1,260p'
```

Confirm no live service work is being done.

### Phase B: Continue dynamic member count, but only with new candidate sources

Keep the budget-member rule:

- `<= 1000U`: 1-3 members
- `1000U-3000U`: 2-4 members
- `3000U-5000U`: 3-5 members

Do not spend more time recombining the same 1058 full-period candidate pool unless there is a new selection criterion. It has already shown the conservative/balanced frontier gap.

New candidate generation should focus on finding lower-correlation, lower-drawdown candidates:

- Symbol-specific direction modes, not generic long+short.
- Entry filters that are live-supported: `ema`, `rsi`, `bollinger`, `adx`, `atr`.
- Regime filters based on BTC/ETH market state.
- Volatility filters that prevent new cycles in explosive regimes.
- Softer filters that preserve trade count; strict filters have often killed return.
- Separate long-only and short-only pools before recombination.

Every promising row must be converted to full-period `portfolio_budget_replay`.

### Phase C: Try formal regime allocator only if live parity is planned

The most plausible remaining martingale-only path is a portfolio-level allocator that reduces exposure before correlated adverse periods, not after drawdown already happens.

Possible research ideas:

- BTC/ETH trend regime disables counter-trend martingale starts.
- ATR-percent or Bollinger-bandwidth regime disables new cycles during expansion.
- Portfolio-level "new cycle throttle" based on broad market volatility.
- Per-symbol cooldown after stop or after max adverse excursion.
- Capital allocator that leaves cash idle when all candidate regimes are poor.

But any such mechanism must become explicit shared configuration, and trading-engine must implement the same behavior before it can be a final deployable candidate.

If GLM uses an env-only backtest switch, the result must be labeled research-only.

### Phase D: Validation protocol for every promising candidate

For each candidate, record:

- Config path
- Result path
- Budget
- Member count
- Symbols
- Annualized return
- Max drawdown
- Total return
- Max capital used
- `budget_blocked_legs`
- `principal_breached`
- Trade count
- Stop count
- Total fees
- Slippage
- Funding
- Whether every mechanic is live-supported

Replay command template:

```bash
cd /home/bumblebee/Project/grid_binance
target/release/portfolio_budget_replay \
  --config /path/to/config.json \
  --budget 5000 \
  --start-ms 1672531200000 \
  --end-ms 1780271999999 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --profile conservative \
  --portfolio-id candidate_id \
  --exchange-min-notional 5
```

Parallel helper if using a manifest:

```bash
python3 /tmp/codex_small_search/run_parallel_replays.py \
  --repo /home/bumblebee/Project/grid_binance \
  --config-dir /tmp/codex_small_search/<configs> \
  --result-dir /tmp/codex_small_search/<results> \
  --max-parallel 12 \
  --profile conservative
```

Then summarize with the appropriate summary helper or create a JSON frontier summary.

### Phase E: If still no conservative/balanced

If the next search still cannot find conservative and balanced under <=5000U, GLM should explicitly report that the current martingale family cannot satisfy the requested gates under the current capital and risk limits.

The report should include:

- Best conservative under DD <= 10%
- Lowest DD conservative candidate over 50% annualized
- Best balanced under DD <= 20%
- Lowest DD balanced candidate over 90% annualized
- Whether each failed due to return, drawdown, budget truncation, or live-parity gap

Do not quietly relax gates or move to live testing.

## What Not To Do

- Do not touch Binance or live mode.
- Do not run 50U smoke trades.
- Do not launch or prepare 1000U live trading.
- Do not display partial results in flyingkid.
- Do not treat theoretical curve search as final.
- Do not use single-window 2023H1 results as final.
- Do not rely on backtest-only env guards as deployable.
- Do not change order semantics in backtest without listing the required live changes.
- Do not classify a candidate as valid if `budget_blocked_legs > 0`.

## When To Return To User

Return to the user only when one of these is true:

1. All three profiles pass full-period runtime-parity replay under the acceptance standard.
2. A new live-parity mechanism is required, and GLM needs permission to implement it.
3. The martingale-only search is exhausted and a clear failure report is ready.

If all three profiles pass, then the next separate phase is:

1. Display only the final three portfolios in the `flyingkid` account and archive old candidates.
2. Write a live validation and launch runbook.
3. Run offline/live-parity checks.
4. Ask the user for confirmation before any 1000U real-money start.
