# 2026-06-28 GLM Exhausted Search Handoff to ChatGPT

## TL;DR

GLM continued the small-capital martingale search across 13 exploration phases (~250 portfolio replays). Result:

- **Aggressive**: still valid (133.54% / 29.88%, re-verified).
- **Balanced**: NOT FOUND. Closest = 100.21% / 26.06% (ann passes, DD +6 over).
- **Conservative**: NOT FOUND. Closest = 50.85% / 17.97% (ann passes, DD +8 over) OR 20.93% / 10.09% (DD ~at gate, ann -30 under).

Full report: `docs/superpowers/reports/2026-06-28-glm-small-cap-exploration-verdict.md`

## Key New Findings (not in prior handoffs)

1. **Dynsafe scales with budget.** Scaling `first_order_quote` by budget ratio (5x for 5000U) preserves percentage metrics exactly. This means the dynsafe family's 1000U results (e.g. 58.68%/18.82%) are directly reusable at 5000U.

2. **Bimodal frontier with a phase transition.** The balanced search hits a sharp cliff between "low DD / low ann" and "high DD / high ann" controlled by whether the martingale ladder self-truncates at the runtime weight cap. NO middle ground (70-90% ann at DD<=20%) exists in the explored space. See the multiplier sweep: mult 2.4→49.55%/19.79%, mult 2.5→86.93%/25.86%.

3. **DD is temporal.** The return engines (INJ/LTC/AAVE) made ~all gains in 2023 and lost ground in 2024-2026. Aggressive 0105: +1996% in 2023, -14% in 2025. The DD comes from continued trading in the post-2023 choppy/bearish period.

4. **Diversification does not help.** Inter-strategy correlations are already near zero (martingale equity curves are event-clustered). Adding 4-6 symbols to the aggressive pattern made results WORSE (37-70% ann, 35-44% DD) than the concentrated 3-symbol 0105.

5. **2024-2026-strong symbols exist** (GALA +117%, ETH +90%, ADA +61%, ICP +52%, BCH +54%, SOL +47% in 2024-2026) but adding them raised both ann and DD due to concurrent budget consumption.

6. **BudgetScaled sizing** (auto-scaling ladders, eliminates `budget_blocked_legs`) gives DD 10-20% but ann caps at ~25%.

7. **Env guards are marginal.** ATR_PAUSE 0.5 helped slightly (87.64/25.48). ADX indicator made DD worse. Cooldowns hurt ann more than DD.

## Closest-to-Pass Candidates (all verified full-period runtime-parity)

### Balanced — closest with ann>90
- Config: `/tmp/codex_small_search/glm_balanced_v3_configs/v3_injl_44.json` (env: `MARTINGALE_BT_NEW_CYCLE_ATR_PAUSE_PCT=0.5`)
- Result: **100.21% / 26.06%**, budget 5000U, `budget_blocked_legs=65`, `principal_breached=false`
- Symbols: AAVEUSDT, FILUSDT, INJUSDT
- Without env tweak: 99.51% / 26.68% (same config, default ATR pause)
- DD gap to gate: **+6.06**

### Balanced — closest with DD<=20
- Config: `/tmp/codex_small_search/glm_scaled_dynsafe_configs/dynsafe_b1000_scaled_x5_b5000.json`
- Result: **58.68% / 18.82%**, budget 5000U, `budget_blocked_legs=0`, `principal_breached=false`
- Ann gap to gate: **-31.32**

### Conservative — closest with ann>50
- Config: `/tmp/codex_small_search/glm_scaled_dynsafe_configs/dynsafe_b1000_scaled_x4_b5000.json` (4x foq scale)
- Result: **50.85% / 17.97%**, budget 5000U, `budget_blocked_legs=0`
- DD gap to gate: **+7.97**

### Conservative — closest with DD<=10
- Config: `/tmp/codex_small_search/glm_bs_v2_configs/bs2_injm4.0l8_fils22_aavel22.json` (BudgetScaled)
- Result: **22.97% / 13.40%**, budget 5000U, `budget_blocked_legs=0`
- Or: `glm_budget_scaled_configs/bs_injl42_fils18_aavel21.9.json` → **20.93% / 10.09%** (DD +0.09 over gate, ann -29 under)

## What GLM Did NOT Do

- Did NOT touch Binance or live mode.
- Did NOT run smoke trades.
- Did NOT relax any gate.
- Did NOT treat single-window or theoretical results as final.
- Did NOT rely on non-live-parity env switches as deployable (the ATR_PAUSE 0.5 tweak IS a default-value change, so it would need to ship to live to be deployable, but the underlying mechanic is live-parity).
- Did NOT merge anything to main. All changes are in the working tree for review.

## Paths That Remain Untried (require decision/implementation)

1. **ATR/trailing TP live parity** — implement non-percent TP in trading-engine's `martingale_exit_signal`, then re-search. Could decouple the TP/SL trade-off causing the bimodal frontier.
2. **Formal regime allocator** — BTC/ETH trend state or ATR-percent regime that disables new martingale cycles pre-emptively (not after DD). Most promising for the 2024-2026 DD problem but requires shared-config + live implementation.
3. **Relax DD gate by 6-8 points** — would let `v3_injl_44 + ATR_PAUSE 0.5` (100.21%/26.06%) pass balanced with a 26% DD limit instead of 20%.
4. **Hybrid martingale + trend overlay** — out of current scope.

## Recommended Decision for ChatGPT

The martingale-only parameter space under live-parity constraints is effectively exhausted for conservative and balanced. The decision points are:

(a) Implement a new live-parity mechanism (ATR/trailing TP or regime allocator) — engineering work, then re-search.
(b) Present the near-pass candidates to the user with the documented gaps and ask whether to relax gates.
(c) Accept aggressive-only as the deployable martingale portfolio.

GLM's recommendation: (a) the regime allocator is the most likely to close the balanced gap, because the DD is demonstrably concentrated in 2024-2026 and a pre-emptive regime filter is the only lever not yet tried that targets the actual DD source.
