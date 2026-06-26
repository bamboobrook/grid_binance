# Martingale LP Portfolio Results (2026-06-23)

All three portfolios are recombinations of existing full-window candidate equity curves. Drawdown is computed from the blended equity curve, not from weighted single-candidate drawdowns.

## Conservative - `mp_lp_conservative_20260623`
- Status: `pending_confirmation`
- Annualized return: 67.7248%
- Max drawdown: 9.8000% (limit 10.00%)
- Total return: 484.52% over 3.414 years
- Direction: `long_short`; source task: `lp-martingale-conservative-20260623`

| Weight | Symbol | Candidate | Source task | Candidate ann | Candidate DD |
|---:|---|---|---|---:|---:|
| 31.060075% | BCHUSDT | `btc_1781608171281158306` | `fk-18-conservative-seed521-lshort-20260615` | 42.98% | 48.64% |
| 26.429787% | DYDXUSDT | `btc_1782139340783464258` | `fk-18-conservative-seed521-b2v2fix-20260622` | 58.74% | 65.87% |
| 21.261852% | UNIUSDT | `btc_1781903777124863557` | `fk-18-conservative-seed521-tailstop-20260619` | 66.25% | 86.62% |
| 9.932054% | DOTUSDT | `btc_1781903777523944238` | `fk-18-conservative-seed521-tailstop-20260619` | 12.67% | 79.13% |
| 5.878983% | INJUSDT | `btc_1781817710694720297` | `fk-18-conservative-seed521-lshort30-20260618` | 164.60% | 42.00% |
| 2.384168% | ETCUSDT | `btc_1781903776996789298` | `fk-18-conservative-seed521-tailstop-20260619` | 74.57% | 70.78% |
| 1.777081% | LINKUSDT | `btc_1781608170892170289` | `fk-18-conservative-seed521-lshort-20260615` | 88.43% | 68.48% |
| 1.276000% | BTCUSDT | `btc_1781608170361879885` | `fk-18-conservative-seed521-lshort-20260615` | 125.60% | 40.80% |

## Balanced - `mp_lp_balanced_20260623`
- Status: `pending_confirmation`
- Annualized return: 104.8547%
- Max drawdown: 18.3312% (limit 20.00%)
- Total return: 988.61% over 3.329 years
- Direction: `long_short`; source task: `lp-martingale-balanced-20260623`
- Baseline ann: 65.52% -> exceeded: `True`

| Weight | Symbol | Candidate | Source task | Candidate ann | Candidate DD |
|---:|---|---|---|---:|---:|
| 40.000000% | ZECUSDT | `btc_1780327744508342319` | `fk-18-bal-v2-seed53-20260601` | 105.51% | 56.52% |
| 40.000000% | AAVEUSDT | `btc_1780133375781642441` | `fk-18-balanced-wide-seed127-20260530` | 110.05% | 68.94% |
| 15.000000% | ETHUSDT | `btc_1780853146914419440` | `fk-18-balanced-atradx-seed307-20260605` | 95.00% | 70.00% |
| 1.000000% | XRPUSDT | `btc_1780327744544706417` | `fk-18-bal-v2-seed53-20260601` | 81.06% | 46.45% |
| 1.000000% | BTCUSDT | `btc_1780853146975727433` | `fk-18-balanced-atradx-seed307-20260605` | 87.24% | 82.87% |
| 1.000000% | SOLUSDT | `btc_1780049465976499346` | `glm-7-balanced-seed29-20260529` | 70.03% | 50.09% |
| 1.000000% | DOGEUSDT | `btc_1780049481679382037` | `glm-7-balanced-seed17-20260529` | 87.28% | 87.49% |
| 1.000000% | BNBUSDT | `btc_1780853147143967782` | `fk-18-balanced-atradx-seed307-20260605` | 67.93% | 45.90% |

## Aggressive - `mp_lp_aggressive_20260623`
- Status: `pending_confirmation`
- Annualized return: 118.2931%
- Max drawdown: 29.5000% (limit 30.00%)
- Total return: 1245.06% over 3.329 years
- Direction: `long`; source task: `lp-martingale-aggressive-20260623`
- Baseline ann: 77.00% -> exceeded: `True`

| Weight | Symbol | Candidate | Source task | Candidate ann | Candidate DD |
|---:|---|---|---|---:|---:|
| 40.000000% | DOGEUSDT | `btc_1780346475878615937` | `fk-18-agg-v2-seed173-20260601` | 131.26% | 71.18% |
| 27.817028% | XRPUSDT | `btc_1780884807261588490` | `fk-18-aggressive-atradx-seed307-20260608` | 103.68% | 85.81% |
| 25.561692% | AAVEUSDT | `btc_1780346475907446344` | `fk-18-agg-v2-seed173-20260601` | 118.66% | 69.15% |
| 2.621280% | INJUSDT | `btc_1780346476371358710` | `fk-18-agg-v2-seed173-20260601` | 66.45% | 63.96% |
| 1.000000% | BCHUSDT | `btc_1780884807474774118` | `fk-18-aggressive-atradx-seed307-20260608` | 95.67% | 72.79% |
| 1.000000% | BNBUSDT | `btc_1780884809164019747` | `fk-18-aggressive-atradx-seed307-20260608` | 78.15% | 58.94% |
| 1.000000% | ETHUSDT | `btc_1780377104292057864` | `fk-18-agg-v2-seed173-20260601` | 98.43% | 65.66% |
| 1.000000% | BTCUSDT | `btc_1780884808168951568` | `fk-18-aggressive-atradx-seed307-20260608` | 83.75% | 67.56% |
