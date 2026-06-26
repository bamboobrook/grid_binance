# Corrected Margin-v2 LP Martingale Portfolios

Date: 2026-06-26

These portfolios are built only from corrected margin-v2 candidate curves. The capital model is: first_order_quote is order notional, futures margin is notional / leverage, and return/drawdown use planned margin capital as principal.

| Mode | Target | Annualized | Max DD | Result | Members | Symbols |
|---|---:|---:|---:|---|---:|---:|
| conservative | >50% / <=10% DD | 79.3593% | 10.0000% | PASS | 8 | 8 |
| balanced | >90% / <=20% DD | 108.0591% | 20.0000% | PASS | 8 | 8 |
| aggressive | >110% / <=30% DD | 128.9630% | 30.0000% | PASS | 8 | 8 |

## Conservative - `mp_margin_v2_lp_conservative_20260626`
- Task: `lp-martingale-conservative-20260626-margin-v2`
- Annualized return: 79.3593% (target > 50%)
- Max drawdown: 10.0000% (limit <= 10%)
- Total return: 634.92% over 3.414 years
- Portfolio quarter raw returns: 332.98%, 33.76%, 31.91%, 18.58%
- Source tasks: martingale-aggressive-20260625-margin-v2, martingale-aggressive-20260625-robust-v1, martingale-balanced-20260625-margin-v3

| Weight | Symbol | Candidate | Source task | Source profile | Candidate ann | Candidate DD | Planned margin |
|---:|---|---|---|---|---:|---:|---:|
| 36.082784% | LTCUSDT | `btc_1782397542840937284` | `martingale-aggressive-20260625-robust-v1` | aggressive | 70.30% | 43.33% | 63890.29 |
| 36.009540% | DYDXUSDT | `btc_1782366734837215913` | `martingale-aggressive-20260625-margin-v2` | aggressive | 57.85% | 33.08% | 54896.51 |
| 8.067337% | INJUSDT | `btc_1782397541175084094` | `martingale-aggressive-20260625-robust-v1` | aggressive | 168.29% | 45.44% | 2659.87 |
| 7.956641% | FILUSDT | `btc_1782378631241500391` | `martingale-aggressive-20260625-margin-v2` | aggressive | 40.31% | 72.24% | 1834.51 |
| 3.709872% | ICPUSDT | `btc_1782378629385328580` | `martingale-aggressive-20260625-margin-v2` | aggressive | 58.47% | 66.25% | 4289.78 |
| 3.513592% | XRPUSDT | `btc_1782378626362285218` | `martingale-aggressive-20260625-margin-v2` | aggressive | 89.62% | 64.61% | 15277.63 |
| 2.444564% | UNIUSDT | `btc_1782372815205308219` | `martingale-aggressive-20260625-margin-v2` | aggressive | 7.14% | 80.82% | 893.18 |
| 2.215670% | BTCUSDT | `btc_1782416776620121466` | `martingale-balanced-20260625-margin-v3` | balanced | 111.52% | 38.58% | 694.91 |

## Balanced - `mp_margin_v2_lp_balanced_20260626`
- Task: `lp-martingale-balanced-20260626-margin-v2`
- Annualized return: 108.0591% (target > 90%)
- Max drawdown: 20.0000% (limit <= 20%)
- Total return: 1119.89% over 3.414 years
- Portfolio quarter raw returns: 771.38%, 40.02%, 20.41%, -0.00%
- Source tasks: martingale-aggressive-20260625-margin-v2, martingale-aggressive-20260625-robust-v1, martingale-balanced-20260625-margin-v3

| Weight | Symbol | Candidate | Source task | Source profile | Candidate ann | Candidate DD | Planned margin |
|---:|---|---|---|---|---:|---:|---:|
| 26.223200% | INJUSDT | `btc_1782397541205906740` | `martingale-aggressive-20260625-robust-v1` | aggressive | 168.17% | 43.66% | 19982.26 |
| 21.505110% | LTCUSDT | `btc_1782397542840937284` | `martingale-aggressive-20260625-robust-v1` | aggressive | 70.30% | 43.33% | 63890.29 |
| 19.796879% | ICPUSDT | `btc_1782378629385328580` | `martingale-aggressive-20260625-margin-v2` | aggressive | 58.47% | 66.25% | 4289.78 |
| 10.453887% | DYDXUSDT | `btc_1782366734837215913` | `martingale-aggressive-20260625-margin-v2` | aggressive | 57.85% | 33.08% | 54896.51 |
| 8.640829% | FILUSDT | `btc_1782378631241500391` | `martingale-aggressive-20260625-margin-v2` | aggressive | 40.31% | 72.24% | 1834.51 |
| 6.257279% | XRPUSDT | `btc_1782378626362285218` | `martingale-aggressive-20260625-margin-v2` | aggressive | 89.62% | 64.61% | 15277.63 |
| 3.584067% | SOLUSDT | `btc_1782372805086834625` | `martingale-aggressive-20260625-margin-v2` | aggressive | 126.99% | 63.96% | 119666.25 |
| 3.538749% | TRXUSDT | `btc_1782397542249114704` | `martingale-aggressive-20260625-robust-v1` | aggressive | 99.40% | 52.13% | 8813.12 |

## Aggressive - `mp_margin_v2_lp_aggressive_20260626`
- Task: `lp-martingale-aggressive-20260626-margin-v2`
- Annualized return: 128.9630% (target > 110%)
- Max drawdown: 30.0000% (limit <= 30%)
- Total return: 1591.51% over 3.414 years
- Portfolio quarter raw returns: 1114.65%, 46.32%, 2.91%, 0.00%
- Source tasks: martingale-aggressive-20260625-margin-v2, martingale-aggressive-20260625-robust-v1, martingale-balanced-20260625-margin-v3

| Weight | Symbol | Candidate | Source task | Source profile | Candidate ann | Candidate DD | Planned margin |
|---:|---|---|---|---|---:|---:|---:|
| 40.000000% | INJUSDT | `btc_1782397541205906740` | `martingale-aggressive-20260625-robust-v1` | aggressive | 168.17% | 43.66% | 19982.26 |
| 18.213321% | ICPUSDT | `btc_1782378629385328580` | `martingale-aggressive-20260625-margin-v2` | aggressive | 58.47% | 66.25% | 4289.78 |
| 14.911748% | DYDXUSDT | `btc_1782366734837215913` | `martingale-aggressive-20260625-margin-v2` | aggressive | 57.85% | 33.08% | 54896.51 |
| 14.575897% | TRXUSDT | `btc_1782397542249114704` | `martingale-aggressive-20260625-robust-v1` | aggressive | 99.40% | 52.13% | 8813.12 |
| 9.299034% | SOLUSDT | `btc_1782378618915153136` | `martingale-aggressive-20260625-margin-v2` | aggressive | 138.90% | 72.05% | 3600.15 |
| 1.000000% | FILUSDT | `btc_1782378631241500391` | `martingale-aggressive-20260625-margin-v2` | aggressive | 40.31% | 72.24% | 1834.51 |
| 1.000000% | LINKUSDT | `btc_1782397541557543981` | `martingale-aggressive-20260625-robust-v1` | aggressive | 122.97% | 70.71% | 18908.08 |
| 1.000000% | BTCUSDT | `btc_1782416776482964240` | `martingale-balanced-20260625-margin-v3` | balanced | 119.22% | 40.99% | 17762.16 |
