# 2026-06-30 Hybrid Frontier Trend Rules Search

本轮在 Phase 1 `research_only` 探针上扩展趋势 sleeve 规则：

- `ema20_50_lf`
- `mom20_lf`
- `mom20_ls`
- `mom60_ls`
- `donchian20_lf`
- `donchian20_ls`

分别对 conservative/balanced/aggressive 各跑 1200 行 bounded wide search。使用主仓离线数据：

- market: `/home/bumblebee/Project/grid_binance/data/market_data_full.db`
- funding: `/home/bumblebee/Project/grid_binance/data/funding_rates.db`

未触碰 live/Binance/flyingkid/真实资金。本报告不是 live-ready 证据。

## conservative

- rows: 1200
- passes: 0
- best_ann_seg_cap: `13.54% ann / 6.32% DD / cap 1343.89`
- best_dd_seg_cap: `13.54% ann / 6.32% DD / cap 1343.89`
- best rule: `none`，即 trend allocation 为 0

| Rank | Ann | DD | Cap | Replay | m/t/f | Rule | Trend | Funding | Violations |
|---:|---:|---:|---:|---|---|---|---|---|---|
| 1 | 13.54% | 6.32% | 1343.89 | `replay_conservative_1000.json` | 500/0/500 | `none` | `` | `DYDXUSDT,AAVEUSDT` | annualized <= 50.0; budget blocked events 228 > 0 |

## balanced

- rows: 1200
- passes: 0
- best_ann_seg_cap: `25.31% ann / 33.07% DD / cap 1144.32`
- best_dd_seg_cap: `13.16% ann / 6.56% DD / cap 1394.32`
- best annualized rule: `donchian20_lf`

| Rank | Ann | DD | Cap | Replay | m/t/f | Rule | Trend | Funding | Violations |
|---:|---:|---:|---:|---|---|---|---|---|---|
| 1 | 25.31% | 33.07% | 1144.32 | `replay_balanced_1000.json` | 500/250/0 | `donchian20_lf` | `BTCUSDT,SOLUSDT,XRPUSDT` | `` | annualized <= 90.0; DD > 20.0; budget blocked events 145 > 0 |
| 2 | 24.83% | 33.01% | 1144.32 | `replay_balanced_1000.json` | 500/250/0 | `donchian20_lf` | `BTCUSDT,SOLUSDT,LINKUSDT` | `` | annualized <= 90.0; DD > 20.0; budget blocked events 145 > 0 |
| 3 | 23.94% | 30.35% | 1144.32 | `replay_balanced_1000.json` | 500/250/0 | `donchian20_lf` | `BTCUSDT,ETHUSDT,SOLUSDT` | `` | annualized <= 90.0; DD > 20.0; budget blocked events 145 > 0 |
| 4 | 22.78% | 32.77% | 1144.32 | `replay_balanced_1000.json` | 500/250/0 | `donchian20_lf` | `ETHUSDT,SOLUSDT,XRPUSDT` | `` | annualized <= 90.0; DD > 20.0; budget blocked events 145 > 0 |
| 5 | 22.48% | 23.44% | 1144.32 | `replay_balanced_1000.json` | 500/250/0 | `mom60_ls` | `ETHUSDT,BNBUSDT,LINKUSDT` | `` | annualized <= 90.0; DD > 20.0; budget blocked events 145 > 0 |

## aggressive

- rows: 1200
- passes: 0
- best_ann_seg_cap: `39.37% ann / 36.78% DD / cap 1233.73`
- best_dd_seg_cap: `27.66% ann / 11.71% DD / cap 1483.73`
- best annualized rule: `mom20_lf`

| Rank | Ann | DD | Cap | Replay | m/t/f | Rule | Trend | Funding | Violations |
|---:|---:|---:|---:|---|---|---|---|---|---|
| 1 | 39.37% | 36.78% | 1233.73 | `replay_aggressive_1000.json` | 500/250/0 | `mom20_lf` | `BTCUSDT,SOLUSDT,DOGEUSDT` | `` | annualized <= 110.0; DD > 30.0; budget blocked events 137 > 0 |
| 2 | 37.30% | 39.18% | 1233.73 | `replay_aggressive_1000.json` | 500/250/0 | `mom20_lf` | `SOLUSDT,DOGEUSDT,XRPUSDT` | `` | annualized <= 110.0; DD > 30.0; budget blocked events 137 > 0 |
| 3 | 37.21% | 31.74% | 1233.73 | `replay_aggressive_1000.json` | 500/250/0 | `donchian20_lf` | `BTCUSDT,SOLUSDT,DOGEUSDT` | `` | annualized <= 110.0; DD > 30.0; budget blocked events 137 > 0 |
| 4 | 37.00% | 35.45% | 1233.73 | `replay_aggressive_1000.json` | 500/250/0 | `mom20_lf` | `BNBUSDT,SOLUSDT,DOGEUSDT` | `` | annualized <= 110.0; DD > 30.0; budget blocked events 137 > 0 |
| 5 | 36.81% | 39.88% | 1233.73 | `replay_aggressive_1000.json` | 500/250/0 | `mom20_lf` | `SOLUSDT,DOGEUSDT,ADAUSDT` | `` | annualized <= 110.0; DD > 30.0; budget blocked events 137 > 0 |

## 结论

- 三档各 1200 行增强趋势规则搜索仍然 0 pass。
- conservative 的最优仍来自无趋势 sleeve，趋势规则未改善低风险组合。
- balanced 最佳年化从上一轮约 `19.54%` 提升到 `25.31%`，但 DD 升到 `33.07%`，同时远低于 `90%` 年化门槛。
- aggressive 最佳年化从上一轮约 `29.13%` 提升到 `39.37%`，但 DD 为 `36.78%`，仍低于 `110%` 年化且超过 `30%` DD 门槛。
- 所有 best rows 仍带 martingale replay 的 `budget blocked events`，因此即使收益/回撤接近，也不能视为可实盘候选。

继续追原目标时，下一步不应只调趋势规则；主要瓶颈仍是 martingale sleeve 的 budget blocked 与收益/回撤比不足。This is Phase 1 research-only evidence and is not live-ready.
