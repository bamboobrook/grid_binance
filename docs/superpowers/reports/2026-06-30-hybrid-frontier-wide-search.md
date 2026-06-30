# 2026-06-30 Hybrid Frontier Wide Search

本轮分别对 conservative/balanced/aggressive 各跑 1200 行 bounded wide search。全部为 `research_only`，未触碰 live/Binance/flyingkid/真实资金。

## conservative

- rows: 1200
- passes: 0
- best_ann_seg_cap: `{'ann': 13.535705941256616, 'c2426': 6.524257198475203, 'cap': 1343.8943031420026, 'dd': 6.315745869302603, 'f_alloc': 500.0, 'full_pass': False, 'funding_symbols': 'DYDXUSDT,AAVEUSDT', 'm_alloc': 500.0, 'pass': False, 'pos': 4, 'profile': 'conservative', 'replay': 'replay_conservative_1000.json', 'seg_pass': True, 't_alloc': 0.0, 'trend_symbols': '', 'violations': ['annualized 13.535705941256616 <= required 50.0', 'budget blocked events 228 > 0']}`
- best_dd_seg_cap: `{'ann': 13.535705941256616, 'c2426': 6.524257198475203, 'cap': 1343.8943031420026, 'dd': 6.315745869302603, 'f_alloc': 500.0, 'full_pass': False, 'funding_symbols': 'DYDXUSDT,AAVEUSDT', 'm_alloc': 500.0, 'pass': False, 'pos': 4, 'profile': 'conservative', 'replay': 'replay_conservative_1000.json', 'seg_pass': True, 't_alloc': 0.0, 'trend_symbols': '', 'violations': ['annualized 13.535705941256616 <= required 50.0', 'budget blocked events 228 > 0']}`

| Rank | Ann | DD | Cap | Replay | m/t/f | Trend | Funding | Violations |
|---:|---:|---:|---:|---|---|---|---|---|
| 1 | 13.54% | 6.32% | 1343.89 | `replay_conservative_1000.json` | 500.0/0.0/500.0 | `` | `DYDXUSDT,AAVEUSDT` | annualized 13.535705941256616 <= required 50.0; budget blocked events 228 > 0 |

## balanced

- rows: 1200
- passes: 0
- best_ann_seg_cap: `{'ann': 19.53511094747311, 'c2426': 46.25523628718709, 'cap': 1144.3206246169343, 'dd': 25.23051774888257, 'f_alloc': 0.0, 'full_pass': False, 'funding_symbols': '', 'm_alloc': 500.0, 'pass': False, 'pos': 3, 'profile': 'balanced', 'replay': 'replay_balanced_1000.json', 'seg_pass': True, 't_alloc': 250.0, 'trend_symbols': 'BTCUSDT,BNBUSDT,DOGEUSDT', 'violations': ['annualized 19.53511094747311 <= required 90.0', 'drawdown 25.23051774888257 > allowed 20.0', 'budget blocked events 145 > 0']}`
- best_dd_seg_cap: `{'ann': 13.163326571170654, 'c2426': 2.7629192914246703, 'cap': 1394.3206246169343, 'dd': 6.563731211706273, 'f_alloc': 500.0, 'full_pass': False, 'funding_symbols': 'ETHUSDT,DYDXUSDT', 'm_alloc': 500.0, 'pass': False, 'pos': 3, 'profile': 'balanced', 'replay': 'replay_balanced_1000.json', 'seg_pass': True, 't_alloc': 0.0, 'trend_symbols': '', 'violations': ['annualized 13.163326571170654 <= required 90.0', 'budget blocked events 145 > 0']}`

| Rank | Ann | DD | Cap | Replay | m/t/f | Trend | Funding | Violations |
|---:|---:|---:|---:|---|---|---|---|---|
| 1 | 19.54% | 25.23% | 1144.32 | `replay_balanced_1000.json` | 500.0/250.0/0.0 | `BTCUSDT,BNBUSDT,DOGEUSDT` | `` | annualized 19.53511094747311 <= required 90.0; drawdown 25.23051774888257 > allowed 20.0; budget blocked events 145 > 0 |
| 2 | 18.96% | 25.13% | 1144.32 | `replay_balanced_1000.json` | 500.0/250.0/0.0 | `BTCUSDT,ETHUSDT,BNBUSDT` | `` | annualized 18.955542767673816 <= required 90.0; drawdown 25.1255299410552 > allowed 20.0; budget blocked events 145 > 0 |
| 3 | 18.27% | 23.43% | 1344.32 | `replay_balanced_1000.json` | 500.0/250.0/100.0 | `BTCUSDT,BNBUSDT,DOGEUSDT` | `DYDXUSDT,AAVEUSDT` | annualized 18.2745107021808 <= required 90.0; drawdown 23.425240323151392 > allowed 20.0; budget blocked events 145 > 0 |
| 4 | 18.27% | 23.44% | 1344.32 | `replay_balanced_1000.json` | 500.0/250.0/100.0 | `BTCUSDT,BNBUSDT,DOGEUSDT` | `ETHUSDT,DYDXUSDT` | annualized 18.271218014858736 <= required 90.0; drawdown 23.435847243990835 > allowed 20.0; budget blocked events 145 > 0 |
| 5 | 18.26% | 23.44% | 1344.32 | `replay_balanced_1000.json` | 500.0/250.0/100.0 | `BTCUSDT,BNBUSDT,DOGEUSDT` | `BTCUSDT,DYDXUSDT` | annualized 18.261783553208378 <= required 90.0; drawdown 23.437729428237066 > allowed 20.0; budget blocked events 145 > 0 |

## aggressive

- rows: 1200
- passes: 0
- best_ann_seg_cap: `{'ann': 29.131296823095454, 'c2426': 1.3735856722323136, 'cap': 1233.727289315676, 'dd': 34.89986406450731, 'f_alloc': 0.0, 'full_pass': False, 'funding_symbols': '', 'm_alloc': 500.0, 'pass': False, 'pos': 3, 'profile': 'aggressive', 'replay': 'replay_aggressive_1000.json', 'seg_pass': True, 't_alloc': 250.0, 'trend_symbols': 'BTCUSDT,BNBUSDT,INJUSDT', 'violations': ['annualized 29.131296823095454 <= required 110.0', 'drawdown 34.89986406450731 > allowed 30.0', 'budget blocked events 137 > 0']}`
- best_dd_seg_cap: `{'ann': 27.664930629918526, 'c2426': 2.362672761968554, 'cap': 1483.727289315676, 'dd': 11.709050196186812, 'f_alloc': 500.0, 'full_pass': False, 'funding_symbols': 'ETHUSDT,DYDXUSDT', 'm_alloc': 500.0, 'pass': False, 'pos': 4, 'profile': 'aggressive', 'replay': 'replay_aggressive_1000.json', 'seg_pass': True, 't_alloc': 0.0, 'trend_symbols': '', 'violations': ['annualized 27.664930629918526 <= required 110.0', 'budget blocked events 137 > 0']}`

| Rank | Ann | DD | Cap | Replay | m/t/f | Trend | Funding | Violations |
|---:|---:|---:|---:|---|---|---|---|---|
| 1 | 29.13% | 34.90% | 1233.73 | `replay_aggressive_1000.json` | 500.0/250.0/0.0 | `BTCUSDT,BNBUSDT,INJUSDT` | `` | annualized 29.131296823095454 <= required 110.0; drawdown 34.89986406450731 > allowed 30.0; budget blocked events 137 > 0 |
| 2 | 28.58% | 34.17% | 1233.73 | `replay_aggressive_1000.json` | 500.0/250.0/0.0 | `BNBUSDT,INJUSDT,DOGEUSDT` | `` | annualized 28.58037455749418 <= required 110.0; drawdown 34.17299981349817 > allowed 30.0; budget blocked events 137 > 0 |
| 3 | 28.18% | 39.75% | 1233.73 | `replay_aggressive_1000.json` | 500.0/250.0/0.0 | `ETHUSDT,BNBUSDT,INJUSDT` | `` | annualized 28.180981147155283 <= required 110.0; drawdown 39.74821219952298 > allowed 30.0; budget blocked events 137 > 0 |
| 4 | 27.69% | 11.82% | 1483.73 | `replay_aggressive_1000.json` | 500.0/0.0/500.0 | `` | `DYDXUSDT,AAVEUSDT` | annualized 27.687630048081747 <= required 110.0; budget blocked events 137 > 0 |
| 5 | 27.66% | 11.71% | 1483.73 | `replay_aggressive_1000.json` | 500.0/0.0/500.0 | `` | `ETHUSDT,DYDXUSDT` | annualized 27.664930629918526 <= required 110.0; budget blocked events 137 > 0 |

## 结论

- 三档各 1200 行 bounded wide search 均 0 pass。
- conservative 可找到 segment-pass 且 DD<10 的低风险组合，但年化约 13.5%，远低于 50%，且仍有 budget blocked。
- balanced/aggressive 在当前搜索顺序和参数内没有 segment-pass + cap<5000 的有效前沿。
- 下一步要继续追原目标，需要扩展趋势规则本身（momentum/Donchian/long-short）和修正 martingale sleeve 的 budget blocked，而不是只调当前 EMA20/50 long-flat 权重。

This is Phase 1 research-only evidence and is not live-ready.
