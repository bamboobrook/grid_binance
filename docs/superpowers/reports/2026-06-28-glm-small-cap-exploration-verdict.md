# 2026-06-28 GLM Small-Capital Martingale Exploration Verdict

## Status Summary

| Profile | Gate | Result |
|---|---:|---|
| Conservative | annualized > 50%, max DD <= 10% | **NOT FOUND** — gap not closed |
| Balanced | annualized > 90%, max DD <= 20% | **NOT FOUND** — gap not closed |
| Aggressive | annualized > 110%, max DD <= 30% | **FOUND** (re-verified: 133.54% / 29.88%, budget 3250U, `budget_blocked_legs=0`, `principal_breached=false`) |

GLM continued the small-capital search after the 2026-06-28 handoff. No live/Binance action was performed; all work is offline runtime-parity replay.

## What GLM Did (13 exploration phases, ~250 portfolio replays)

GLM explored the following new directions on top of the prior work. All replays used the canonical `portfolio_budget_replay` over `2023-01-01..2026-05-31`, `on_budget` metrics, exchange-min-notional = 5, fees + slippage + funding included.

1. **Correlation analysis** of the 1058-candidate single-strategy pool. Found that inter-candidate correlations are already very low (most pairs near 0, some negative). Correlation is NOT the blocker — the candidate family simply lacks low-DD high-return members.
2. **Proxy portfolio combinator** (39k combinations) using existing single-strategy equity curves. Useful for ranking but the proxy was systematically too optimistic vs. real replay.
3. **Native small-budget configs** (315 balanced + 270 conservative) saturating the runtime weight-cap pattern. Replays showed DD 33-48% and `budget_blocked_legs` 54-170.
4. **Dynsafe scaling discovery**: scaling `first_order_quote` by the budget ratio (5x for 5000U vs 1000U) keeps percentage metrics identical because TP/SL/spacing are all percentage-based. This unlocked the existing dynsafe family at 5000U.
5. **v2-v5 parameter fine-sweep** around the dynsafe family (multiplier 2.0-2.8, INJ-long weight 36-50, stop-loss, legs, ATR pause). Mapped a clean bimodal frontier.
6. **Regime analysis**: split each candidate's return into 2023 vs 2024-2026. Found that the aggressive 0105 and most return engines made ~all their gains in 2023 (+1996% for 0105) and LOST ground in 2024-2026 (-14% in 2025). The DD comes from the post-2023 period.
7. **2024-2026 engine combination** (Phase v6): added GALA/ETH/ADA/ICP/BCH/SOL legs that grew in 2024-2026. Ann rose to 66-70% but DD rose to 29-30% with high `budget_blocked_legs` (105-115).
8. **Env risk-guard tuning** (Phase v7): tested ATR pause 0.5-1.5%, ADX skip 25-45, with and without ADX indicator. ATR_PAUSE 0.5 gave the best variant (87.64%/25.48%). Adding the ADX indicator made DD WORSE.
9. **Aggressive-pattern diversification** (Phase K): replicated aggressive 0105's weight-saturation pattern on 4-6 symbols. All variants were WORSE than the original 3-symbol 0105 (37-70% ann, 35-44% DD). Diversification dilutes return without reducing DD.
10. **Both-period engine portfolios** (Phase v8): DYDX+AAVE+TRX combinations. Ann 22-63%, DD 25-30%. The both-period single-strategy strength did NOT transfer to low-DD portfolios.
11. **Cooldown tuning** (Phase v9): cooldowns 12h-72h. Mostly reduced ann more than DD; one variant hit 92% ann but DD blew up to 40%.
12. **BudgetScaled sizing model** (Phase v10/N2): auto-scaling ladders that eliminate `budget_blocked_legs`. Produced DD 10-20% but ann capped at ~25% — the auto-scaling limits return.

## The Bimodal Frontier (Key Structural Finding)

The balanced search revealed a hard **phase transition** controlled by `budget_blocked_legs`:

| Regime | Ann range | DD range | `budget_blocked_legs` |
|---|---:|---:|---:|
| Truncated (weight-cap saturated, ladder self-truncates) | ~50-58% | ~18-20% | 13-15 |
| Full-ladder (deep ladders run unconstrained) | ~86-101% | ~26-27% | 58-65 |

There is NO middle ground (70-90% ann with DD<=20%) in the explored parameter space. The transition is sharp because:
- When the ladder is shallow enough to self-truncate at the weight cap, adverse excursions are capped (low DD) but so are winning excursions (low ann).
- When the ladder runs deep, both upside and downside are unleashed together.

This is visible in the cleanest data points (5000U, dynsafe family, INJ-long w42):
- mult 2.0 → 50.96% / 19.75%
- mult 2.1 → 51.73% / 19.91%
- mult 2.4 → 49.55% / 19.79%
- mult 2.5 → **86.93% / 25.86%** (the cliff)
- mult 2.6 → 86.35% / 29.09%
- mult 2.8 → 99.51% / 26.68%

## Closest Results to Each Gate

### Conservative (ann > 50%, DD <= 10%)

| Candidate | Ann | DD | Budget | Notes |
|---|---:|---:|---:|---|
| `dynsafe_x4_b5000` | 50.85% | 17.97% | 5000 | ann passes, DD +7.97 over |
| `dynsafe_x5_b5000` | 58.68% | 18.82% | 5000 | ann passes, DD +8.82 over |
| `bs2_injm4.0l8_fils22_aavel22` (BudgetScaled) | 22.97% | 13.40% | 5000 | DD close, ann far under |
| `bs_injl42_fils18_aavel21.9` (BudgetScaled) | 20.93% | 10.09% | 5000 | DD +0.09 over, ann far under |
| `wanch_00826` (existing) | 17.72% | 9.16% | 5000 | DD passes, ann far under |

Closest to a pass: `bs_injl42_fils18_aavel21.9` at 20.93%/10.09% — DD is essentially at gate but ann is 30 points short.

### Balanced (ann > 90%, DD <= 20%)

| Candidate | Ann | DD | Budget | Notes |
|---|---:|---:|---:|---|
| `v2_injl_42` / `v3_injl_44` / `v5_m2.8_w44` | 99.51-101.38% | 26.23-26.68% | 5000 | ann passes, DD +6.2 over |
| `v7_injl42_NEW_CYCLE_ATR_PAUSE_PCT0.5` | 100.21% | 26.06% | 5000 | ann passes, DD +6.06 over (best DD with ann>90) |
| `v4_injl42_m2.5` | 86.93% | 25.86% | 5000 | ann -3 under, DD +5.86 over |
| `dynsafe_x5_b5000` / `v2_base` | 58.68% | 18.82% | 5000 | DD passes, ann -31 under |
| `dynsafe_x4_b5000` | 50.85% | 17.97% | 5000 | DD passes, ann -39 under |

Closest to a pass: `v7_injl42_ATR_PAUSE_0.5` at 100.21%/26.06% — ann is well past gate but DD is 6 points over. The DD gap of ~6% has resisted every lever tried (stop tuning, regime combos, env guards, cooldowns, BudgetScaled).

### Aggressive (ann > 110%, DD <= 30%) — SOLVED

- Config: `/tmp/codex_small_search/fixed_exposure_cash_priority_configs/0105_full_pool_b3000_top_12_fixed_cash_b3250.json`
- Result: 133.54% / 29.88%, budget 3250U, `budget_blocked_legs=0`, `principal_breached=false`
- Re-verified by GLM on 2026-06-28.

## Why the Gap Persists

1. **Single-strategy frontier ceiling.** The 1058-candidate pool has NO single strategy with ann>50% and DD<=10%, and NO single strategy with ann>90% and DD<=20%. Conservative/balanced cannot come from one strong single strategy.

2. **Portfolio diversification does not deliver the expected DD reduction.** Inter-strategy correlations are already near zero (martingale equity curves are event-clustered, not beta-driven), so adding more symbols does not diversify away the DD the way it would for beta-driven returns.

3. **Phase transition in capital efficiency.** The dynsafe weight-saturation pattern produces a sharp cliff between "low DD / low ann" and "high DD / high ann" controlled by whether ladders self-truncate. There is no continuous trade-off curve in the middle.

4. **Temporal concentration.** The return engines (INJ/LTC/AAVE) made their gains in 2023 and lost ground in 2024-2026. The 2024-2026-strong symbols (GALA/ETH/ADA/ICP) exist but adding them raised both ann and DD because they consume budget and increase concurrent exposure.

5. **Live-parity constraint.** Non-percent TP (ATR/trailing) and regime allocators are NOT yet implemented in trading-engine, so they cannot be used for deployable candidates.

## Conclusion

Under the current constraints (martingale-only, percent TP + fixed/ATR spacing + strategy drawdown stop, live-parity risk guards, budget <= 5000U, exchange-min-notional = 5, fees/slippage/funding included), GLM could not find conservative or balanced passes. The aggressive pass remains valid.

The closest-to-pass results are documented above. The balanced DD gap is ~6 percentage points; the conservative DD gap is ~8 percentage points (or the ann gap is ~30 points if prioritizing DD).

## Recommended Next Directions (require decision)

These are the remaining avenues that GLM did NOT exhaust and that could plausibly close the gap, but each requires either a new live-parity mechanism or user acceptance of a relaxed gate:

1. **Implement ATR/trailing TP live parity in trading-engine**, then re-search. Non-percent TP could decouple the TP/SL trade-off that currently forces the bimodal frontier.
2. **Implement a formal regime allocator** (BTC/ETH trend state, ATR-percent regime, broad-volatility throttle) as shared config + live behavior. This is the most promising path to avoid 2024-2026 DD while keeping 2023 returns, but it is a real engineering change, not a parameter tweak.
3. **Accept the closest candidates as "near-balanced" / "near-conservative"** and either relax the DD gate by ~6-8 points, or accept lower ann (~58%) at the strict DD gate.
4. **Hybridize martingale with a non-averaging model** (e.g. momentum/trend overlay) — out of scope for the current martingale-only mandate.

GLM did NOT relax any gate, did NOT use single-window results as final, did NOT rely on non-live-parity env switches as deployable, and did NOT touch Binance/live mode.

## Artifacts

All GLM configs and results are under `/tmp/codex_small_search/glm_*`:
- `glm_scaled_dynsafe_configs/` and `glm_scaled_dynsafe_results/` — dynsafe scaled to 5000U
- `glm_balanced_v2_configs/` through `glm_balanced_v9_configs/` — parameter sweeps
- `glm_balanced_v6_configs/` — 2024-2026 engine combinations
- `glm_balanced_v7_configs/` — env risk-guard variants
- `glm_aggressive_diverse_configs/` — diversified aggressive pattern
- `glm_budget_scaled_configs/` and `glm_bs_v2_configs/` — BudgetScaled sizing
- `glm_correlation_analysis.py`, `glm_regime_analysis.py`, `glm_portfolio_combinator.py` — analysis scripts
- `glm_final_frontier.txt` — full-period frontier summary including all GLM results

The aggressive candidate remains at:
- Config: `/tmp/codex_small_search/fixed_exposure_cash_priority_configs/0105_full_pool_b3000_top_12_fixed_cash_b3250.json`
- Result: `/tmp/codex_small_search/fixed_exposure_cash_priority_results/0105_full_pool_b3000_top_12_fixed_cash_b3250.json`

## Handoff to ChatGPT

Per the handoff doc's "When To Return To User": condition (3) applies — "The martingale-only search is exhausted and a clear failure report is ready" for conservative and balanced. Aggressive remains valid.

ChatGPT should decide whether to:
- pursue one of the new live-parity mechanisms above (requires implementation work), or
- present the near-pass candidates to the user with the gap sizes, or
- relax the gates.

No GLM changes were merged to main. All modifications remain in the working tree for ChatGPT's review.
