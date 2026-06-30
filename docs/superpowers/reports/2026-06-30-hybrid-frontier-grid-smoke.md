# 2026-06-30 Hybrid Frontier Grid Smoke

本轮使用已合并的 `scripts/hybrid_martingale_frontier_probe.py` 做离线 research-only 小网格：

- martingale sleeve: `docs/superpowers/reports/replay_{profile}_4000.json`
- trend sleeve: `BTCUSDT,ETHUSDT,BNBUSDT` 日线 EMA20/50 long/flat
- funding sleeve: `BTCUSDT,ETHUSDT` short-perp funding
- budget: `5000`
- martingale allocation: `500/1000/1500/2000`
- trend allocation: 每 symbol `500`
- funding allocation: 每 symbol `250`

结果均为 `live_parity_status=research_only`，未触碰 live、Binance、flyingkid 或真实资金。

| Profile | Best row | Full ann | Full DD | Capital | Segment gate | 主要失败原因 |
|---|---|---:|---:|---:|---|---|
| Conservative | `hybrid_conservative_500.json` | 14.40% | 22.41% | 3751.23 | FAIL | 年化远低于 50%、DD 高于 10%、`budget_blocked=75`、仅 3/5 正段 |
| Balanced | `hybrid_balanced_500.json` | 14.07% | 22.17% | 4177.61 | PASS | 年化远低于 90%、DD 高于 20%、`budget_blocked=39` |
| Aggressive | `hybrid_aggressive_2000.json` | 32.16% | 20.95% | 4735.22 | FAIL | 年化远低于 110%、2024-2026 合计 -3.03% |

补充：

- aggressive `m=1000` 的 segment gate 可过，full ann 31.20%、DD 20.64%、capital 4735.22，但仍远低于 110%。
- balanced 的固定混合组合改善了分段，但没有足够收益源。
- conservative 仍被 segment DD、positive segment count、budget blocked 一起否决。

结论：当前 **固定三趋势币 + 两 funding 币 + 单一 EMA20/50 + replay_4000** 的混合组合族没有达标候选。下一步若继续追原目标，应扩大 Phase 1 搜索维度：

1. 趋势规则：EMA 20/50、50/200、momentum 20/60、Donchian/high-low breakout。
2. 趋势币池：funding DB 覆盖的 30 个 futures symbols，而不是只 BTC/ETH/BNB。
3. 组合权重：在 `<5000U` 下做 profile-specific allocation grid，并直接优化 C/B/A gates。
4. 输出 Pareto frontier：分别记录 DD 合格时最高年化、年化合格时最低 DD、segment-pass 子集。

