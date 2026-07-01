# 2026-07-01 DGT Dynamic Grid Probe

This report summarizes the research-only DGT dynamic-grid search. It does not touch Binance, flyingkid, live mode, or real funds. It is not live-ready.

## Scope

- Data: `data/market_data_full.db`
- Market type: `spot`
- Timeframe loaded: `1m`, accounting simulated over full 1m bars
- Equity reporting: daily first/high/low/close points are retained before portfolio union/carry-forward combination, so compressed streams keep daily equity extremes for drawdown checks.
- Budget: `<5000U` max input capital
- Multi-symbol rule: single-symbol candidates are rejected
- Gate targets: conservative `ann >50%, DD<=10%`; balanced `ann >90%, DD<=20%`; aggressive `ann >110%, DD<=30%`
- Segment balance rule: at least 4 of 5 required periods positive, plus combined 2024-2026 return > 0
- Live parity status: `research_only` for every candidate

## Commands

- Smoke: 6 symbols, spacings `0.02,0.05,0.07,0.10`, half counts `2,3,7`, principals `50,100`, group size 2, group limit 15.
- Group size 2: 10 symbols, spacings `0.02,0.03,0.05,0.07,0.10`, half counts `2,3,5,7`, principals `50,100,150`, group limit 45, output limit 5000.
- Group size 3: 10 symbols, spacings `0.02,0.03,0.05,0.07,0.10`, half counts `2,3,5,7`, principals `50,100`, group limit 60, output limit 5000.

## Results

### smoke

- Scope: 6 symbols, group_size=2, 1080 rows
- Rows: `1080`
- Live parity status: `research_only`
- conservative: rows `360`, offline passes `0`
  - best annualized overall: `SOLUSDT,XRPUSDT` | ann `698.09%` | DD `67.24%` | max_input `79693.65U` | params `{'grid_spacing': 0.02, 'group': 'SOLUSDT,XRPUSDT', 'half_grid_count': 2, 'principal_quote': 50.0}` | segments `h1_2023=11302.43%, h2_2023=514.95%, 2024=185.12%, 2025=-17.04%, 2026_ytd=-27.78%` | violations `drawdown 67.24030464549284 > allowed 10.0; capital 79693.65 is not below budget 5000.00; only 3/5 segments positive; need 4`
  - best annualized under 5000U: `SOLUSDT,XRPUSDT` | ann `227.32%` | DD `63.99%` | max_input `4461.50U` | params `{'grid_spacing': 0.05, 'group': 'SOLUSDT,XRPUSDT', 'half_grid_count': 3, 'principal_quote': 50.0}` | segments `h1_2023=392.76%, h2_2023=484.30%, 2024=188.89%, 2025=-8.15%, 2026_ytd=-25.22%` | violations `drawdown 63.99322049887795 > allowed 10.0; only 3/5 segments positive; need 4`
  - lowest DD under 5000U: `BTCUSDT,ETHUSDT` | ann `10.13%` | DD `0.00%` | max_input `100.00U` | params `{'grid_spacing': 0.1, 'group': 'BTCUSDT,ETHUSDT', 'half_grid_count': 7, 'principal_quote': 50.0}` | segments `h1_2023=10.54%, h2_2023=0.00%, 2024=25.75%, 2025=0.00%, 2026_ytd=0.00%` | violations `annualized 10.127892071295808 <= required 50.0; only 2/5 segments positive; need 4`
- balanced: rows `360`, offline passes `0`
  - best annualized overall: `SOLUSDT,XRPUSDT` | ann `698.09%` | DD `67.24%` | max_input `79693.65U` | params `{'grid_spacing': 0.02, 'group': 'SOLUSDT,XRPUSDT', 'half_grid_count': 2, 'principal_quote': 50.0}` | segments `h1_2023=11302.43%, h2_2023=514.95%, 2024=185.12%, 2025=-17.04%, 2026_ytd=-27.78%` | violations `drawdown 67.24030464549284 > allowed 20.0; capital 79693.65 is not below budget 5000.00; only 3/5 segments positive; need 4`
  - best annualized under 5000U: `SOLUSDT,XRPUSDT` | ann `227.32%` | DD `63.99%` | max_input `4461.50U` | params `{'grid_spacing': 0.05, 'group': 'SOLUSDT,XRPUSDT', 'half_grid_count': 3, 'principal_quote': 50.0}` | segments `h1_2023=392.76%, h2_2023=484.30%, 2024=188.89%, 2025=-8.15%, 2026_ytd=-25.22%` | violations `drawdown 63.99322049887795 > allowed 20.0; only 3/5 segments positive; need 4`
  - lowest DD under 5000U: `BTCUSDT,ETHUSDT` | ann `10.13%` | DD `0.00%` | max_input `100.00U` | params `{'grid_spacing': 0.1, 'group': 'BTCUSDT,ETHUSDT', 'half_grid_count': 7, 'principal_quote': 50.0}` | segments `h1_2023=10.54%, h2_2023=0.00%, 2024=25.75%, 2025=0.00%, 2026_ytd=0.00%` | violations `annualized 10.127892071295808 <= required 90.0; only 2/5 segments positive; need 4`
- aggressive: rows `360`, offline passes `0`
  - best annualized overall: `SOLUSDT,XRPUSDT` | ann `698.09%` | DD `67.24%` | max_input `79693.65U` | params `{'grid_spacing': 0.02, 'group': 'SOLUSDT,XRPUSDT', 'half_grid_count': 2, 'principal_quote': 50.0}` | segments `h1_2023=11302.43%, h2_2023=514.95%, 2024=185.12%, 2025=-17.04%, 2026_ytd=-27.78%` | violations `drawdown 67.24030464549284 > allowed 30.0; capital 79693.65 is not below budget 5000.00; only 3/5 segments positive; need 4`
  - best annualized under 5000U: `SOLUSDT,XRPUSDT` | ann `227.32%` | DD `63.99%` | max_input `4461.50U` | params `{'grid_spacing': 0.05, 'group': 'SOLUSDT,XRPUSDT', 'half_grid_count': 3, 'principal_quote': 50.0}` | segments `h1_2023=392.76%, h2_2023=484.30%, 2024=188.89%, 2025=-8.15%, 2026_ytd=-25.22%` | violations `drawdown 63.99322049887795 > allowed 30.0; only 3/5 segments positive; need 4`
  - lowest DD under 5000U: `BTCUSDT,ETHUSDT` | ann `10.13%` | DD `0.00%` | max_input `100.00U` | params `{'grid_spacing': 0.1, 'group': 'BTCUSDT,ETHUSDT', 'half_grid_count': 7, 'principal_quote': 50.0}` | segments `h1_2023=10.54%, h2_2023=0.00%, 2024=25.75%, 2025=0.00%, 2026_ytd=0.00%` | violations `annualized 10.127892071295808 <= required 110.0; only 2/5 segments positive; need 4`

### group_size_2

- Scope: 10 symbols, group_size=2, limit=5000
- Rows: `5000`
- Live parity status: `research_only`
- conservative: rows `1667`, offline passes `0`
  - best annualized overall: `SOLUSDT,XRPUSDT` | ann `698.09%` | DD `69.12%` | max_input `79693.65U` | params `{'grid_spacing': 0.02, 'group': 'SOLUSDT,XRPUSDT', 'half_grid_count': 2, 'principal_quote': 50.0}` | segments `h1_2023=11302.43%, h2_2023=514.95%, 2024=185.12%, 2025=-17.04%, 2026_ytd=-27.78%` | violations `drawdown 69.11503004457488 > allowed 10.0; capital 79693.65 is not below budget 5000.00; only 3/5 segments positive; need 4`
  - best annualized under 5000U: `BTCUSDT,BNBUSDT` | ann `238.17%` | DD `48.76%` | max_input `4549.56U` | params `{'grid_spacing': 0.03, 'group': 'BTCUSDT,BNBUSDT', 'half_grid_count': 3, 'principal_quote': 50.0}` | segments `h1_2023=625.98%, h2_2023=67.87%, 2024=317.27%, 2025=38.55%, 2026_ytd=-9.18%` | violations `drawdown 48.76117805440401 > allowed 10.0`
  - lowest DD under 5000U: `BTCUSDT,ETHUSDT` | ann `10.13%` | DD `0.00%` | max_input `100.00U` | params `{'grid_spacing': 0.1, 'group': 'BTCUSDT,ETHUSDT', 'half_grid_count': 7, 'principal_quote': 50.0}` | segments `h1_2023=10.54%, h2_2023=0.00%, 2024=25.75%, 2025=0.00%, 2026_ytd=0.00%` | violations `annualized 10.127892071295808 <= required 50.0; only 2/5 segments positive; need 4`
- balanced: rows `1667`, offline passes `0`
  - best annualized overall: `SOLUSDT,XRPUSDT` | ann `698.09%` | DD `69.12%` | max_input `79693.65U` | params `{'grid_spacing': 0.02, 'group': 'SOLUSDT,XRPUSDT', 'half_grid_count': 2, 'principal_quote': 50.0}` | segments `h1_2023=11302.43%, h2_2023=514.95%, 2024=185.12%, 2025=-17.04%, 2026_ytd=-27.78%` | violations `drawdown 69.11503004457488 > allowed 20.0; capital 79693.65 is not below budget 5000.00; only 3/5 segments positive; need 4`
  - best annualized under 5000U: `BTCUSDT,BNBUSDT` | ann `238.17%` | DD `48.76%` | max_input `4549.56U` | params `{'grid_spacing': 0.03, 'group': 'BTCUSDT,BNBUSDT', 'half_grid_count': 3, 'principal_quote': 50.0}` | segments `h1_2023=625.98%, h2_2023=67.87%, 2024=317.27%, 2025=38.55%, 2026_ytd=-9.18%` | violations `drawdown 48.76117805440401 > allowed 20.0`
  - lowest DD under 5000U: `BTCUSDT,ETHUSDT` | ann `10.13%` | DD `0.00%` | max_input `100.00U` | params `{'grid_spacing': 0.1, 'group': 'BTCUSDT,ETHUSDT', 'half_grid_count': 7, 'principal_quote': 50.0}` | segments `h1_2023=10.54%, h2_2023=0.00%, 2024=25.75%, 2025=0.00%, 2026_ytd=0.00%` | violations `annualized 10.127892071295808 <= required 90.0; only 2/5 segments positive; need 4`
- aggressive: rows `1666`, offline passes `0`
  - best annualized overall: `SOLUSDT,XRPUSDT` | ann `698.09%` | DD `69.12%` | max_input `79693.65U` | params `{'grid_spacing': 0.02, 'group': 'SOLUSDT,XRPUSDT', 'half_grid_count': 2, 'principal_quote': 50.0}` | segments `h1_2023=11302.43%, h2_2023=514.95%, 2024=185.12%, 2025=-17.04%, 2026_ytd=-27.78%` | violations `drawdown 69.11503004457488 > allowed 30.0; capital 79693.65 is not below budget 5000.00; only 3/5 segments positive; need 4`
  - best annualized under 5000U: `BTCUSDT,BNBUSDT` | ann `238.17%` | DD `48.76%` | max_input `4549.56U` | params `{'grid_spacing': 0.03, 'group': 'BTCUSDT,BNBUSDT', 'half_grid_count': 3, 'principal_quote': 50.0}` | segments `h1_2023=625.98%, h2_2023=67.87%, 2024=317.27%, 2025=38.55%, 2026_ytd=-9.18%` | violations `drawdown 48.76117805440401 > allowed 30.0`
  - lowest DD under 5000U: `BTCUSDT,ETHUSDT` | ann `10.13%` | DD `0.00%` | max_input `100.00U` | params `{'grid_spacing': 0.1, 'group': 'BTCUSDT,ETHUSDT', 'half_grid_count': 7, 'principal_quote': 50.0}` | segments `h1_2023=10.54%, h2_2023=0.00%, 2024=25.75%, 2025=0.00%, 2026_ytd=0.00%` | violations `annualized 10.127892071295808 <= required 110.0; only 2/5 segments positive; need 4`

### group_size_3

- Scope: 10 symbols, group_size=3, limit=5000
- Rows: `5000`
- Live parity status: `research_only`
- conservative: rows `1667`, offline passes `0`
  - best annualized overall: `BTCUSDT,SOLUSDT,INJUSDT` | ann `637.42%` | DD `76.44%` | max_input `125003.07U` | params `{'grid_spacing': 0.02, 'group': 'BTCUSDT,SOLUSDT,INJUSDT', 'half_grid_count': 2, 'principal_quote': 50.0}` | segments `h1_2023=25321.72%, h2_2023=459.18%, 2024=14.00%, 2025=-42.62%, 2026_ytd=-1.70%` | violations `drawdown 76.43679385775319 > allowed 10.0; capital 125003.07 is not below budget 5000.00; only 3/5 segments positive; need 4; 2024-2026 combined return -35.70% <= 0`
  - best annualized under 5000U: `BTCUSDT,SOLUSDT,XRPUSDT` | ann `198.12%` | DD `63.24%` | max_input `4868.18U` | params `{'grid_spacing': 0.03, 'group': 'BTCUSDT,SOLUSDT,XRPUSDT', 'half_grid_count': 5, 'principal_quote': 50.0}` | segments `h1_2023=270.19%, h2_2023=432.77%, 2024=192.80%, 2025=-6.21%, 2026_ytd=-23.32%` | violations `drawdown 63.236821084019326 > allowed 10.0; only 3/5 segments positive; need 4`
  - lowest DD under 5000U: `BTCUSDT,ETHUSDT,BNBUSDT` | ann `13.05%` | DD `0.00%` | max_input `150.00U` | params `{'grid_spacing': 0.1, 'group': 'BTCUSDT,ETHUSDT,BNBUSDT', 'half_grid_count': 7, 'principal_quote': 50.0}` | segments `h1_2023=7.03%, h2_2023=0.00%, 2024=33.05%, 2025=6.75%, 2026_ytd=0.00%` | violations `annualized 13.050404377218383 <= required 50.0; only 3/5 segments positive; need 4`
- balanced: rows `1667`, offline passes `0`
  - best annualized overall: `BTCUSDT,SOLUSDT,INJUSDT` | ann `637.42%` | DD `76.44%` | max_input `125003.07U` | params `{'grid_spacing': 0.02, 'group': 'BTCUSDT,SOLUSDT,INJUSDT', 'half_grid_count': 2, 'principal_quote': 50.0}` | segments `h1_2023=25321.72%, h2_2023=459.18%, 2024=14.00%, 2025=-42.62%, 2026_ytd=-1.70%` | violations `drawdown 76.43679385775319 > allowed 20.0; capital 125003.07 is not below budget 5000.00; only 3/5 segments positive; need 4; 2024-2026 combined return -35.70% <= 0`
  - best annualized under 5000U: `BTCUSDT,SOLUSDT,XRPUSDT` | ann `198.12%` | DD `63.24%` | max_input `4868.18U` | params `{'grid_spacing': 0.03, 'group': 'BTCUSDT,SOLUSDT,XRPUSDT', 'half_grid_count': 5, 'principal_quote': 50.0}` | segments `h1_2023=270.19%, h2_2023=432.77%, 2024=192.80%, 2025=-6.21%, 2026_ytd=-23.32%` | violations `drawdown 63.236821084019326 > allowed 20.0; only 3/5 segments positive; need 4`
  - lowest DD under 5000U: `BTCUSDT,ETHUSDT,BNBUSDT` | ann `13.05%` | DD `0.00%` | max_input `150.00U` | params `{'grid_spacing': 0.1, 'group': 'BTCUSDT,ETHUSDT,BNBUSDT', 'half_grid_count': 7, 'principal_quote': 50.0}` | segments `h1_2023=7.03%, h2_2023=0.00%, 2024=33.05%, 2025=6.75%, 2026_ytd=0.00%` | violations `annualized 13.050404377218383 <= required 90.0; only 3/5 segments positive; need 4`
- aggressive: rows `1666`, offline passes `0`
  - best annualized overall: `BTCUSDT,SOLUSDT,INJUSDT` | ann `637.42%` | DD `76.44%` | max_input `125003.07U` | params `{'grid_spacing': 0.02, 'group': 'BTCUSDT,SOLUSDT,INJUSDT', 'half_grid_count': 2, 'principal_quote': 50.0}` | segments `h1_2023=25321.72%, h2_2023=459.18%, 2024=14.00%, 2025=-42.62%, 2026_ytd=-1.70%` | violations `drawdown 76.43679385775319 > allowed 30.0; capital 125003.07 is not below budget 5000.00; only 3/5 segments positive; need 4; 2024-2026 combined return -35.70% <= 0`
  - best annualized under 5000U: `BTCUSDT,SOLUSDT,XRPUSDT` | ann `198.12%` | DD `63.24%` | max_input `4868.18U` | params `{'grid_spacing': 0.03, 'group': 'BTCUSDT,SOLUSDT,XRPUSDT', 'half_grid_count': 5, 'principal_quote': 50.0}` | segments `h1_2023=270.19%, h2_2023=432.77%, 2024=192.80%, 2025=-6.21%, 2026_ytd=-23.32%` | violations `drawdown 63.236821084019326 > allowed 30.0; only 3/5 segments positive; need 4`
  - lowest DD under 5000U: `BTCUSDT,ETHUSDT,BNBUSDT` | ann `13.05%` | DD `0.00%` | max_input `150.00U` | params `{'grid_spacing': 0.1, 'group': 'BTCUSDT,ETHUSDT,BNBUSDT', 'half_grid_count': 7, 'principal_quote': 50.0}` | segments `h1_2023=7.03%, h2_2023=0.00%, 2024=33.05%, 2025=6.75%, 2026_ytd=0.00%` | violations `annualized 13.050404377218383 <= required 110.0; only 3/5 segments positive; need 4`

## Interpretation

- Offline passes found: `0`.
- The high-return DGT candidates fail the capital and drawdown gates. In the focused searches, best annualized candidates required far above 5000U max input and had drawdown well above aggressive limits.
- The low-drawdown under-budget candidates stay far below the original annualized-return targets.
- This supports the current frontier finding: dynamic-grid reset mechanics can create high offline returns, but not under the combined constraints of `<5000U`, multi-symbol, balanced periods, and strict DD gates in this bounded scope.

## Conclusion

Offline passes found: `0`. The DGT search did not satisfy all original C/B/A gates in this scope.

This report is research-only evidence and is not live-ready.
