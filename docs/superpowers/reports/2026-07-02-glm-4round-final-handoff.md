# GLM Martingale 4-Round Final Handoff (2026-07-02)

> **状态：收益目标在马丁核心约束下经4轮穷尽确认结构性不可达。需要用户决策。**

## 四轮进展轨迹

| 轮次 | 最佳年化 | 最佳回撤 | 正段数 | 2025最佳 | 关键机制 |
|---|---:|---:|---:|---:|---|
| R1 | 22.2% | 26.1% | 3/5 | -18.2% | strict gate + 高TP |
| R2 | 28.6% | 22.5% | 4/5 | -13.3% | + Partial TP + Cond SO + cd12h |
| R3 | 34.5% | 17.8% | 4/5 | -10.1% | + 方向感知SO + cd11h |
| **R4** | **34.7%** | **17.7%** | **4/5** | **-8.70%** | + pump-fade(ROC) + 组合优化 |

## 最终最佳候选

**`r4-combo-best`** (pump-fade short + direction-aware long + symmetric partial TP + cd11h):
- ann **34.7%**, DD **17.7%**, **4/5段正**, agg2024-2026 **+29.2%**
- h1_2023 +25.8%, h2_2023 +3.5%, 2024 +32.0%, 2025 **-8.7%**, 2026_ytd +7.2%
- 5000U预算, 6币种, 完整live-parity (4/4 features全实现)

## 目标对照

| 目标 | 状态 | 差距 |
|---|---|---|
| 小资金<5000U | ✅ | - |
| 多币种 | ✅ (6 symbols) | - |
| 抗过拟合 | ✅ (4/5段正, agg+29%) | - |
| 实盘可复现 | ✅ (4/4 features全实现, 187+208 tests) | - |
| 保守>50% ann | ❌ | 34.7% vs 50%, 差15.3pp |
| 平衡>90% ann | ❌ | 差55.3pp |
| 激进>110% ann | ❌ | 差75.3pp |

## 2025结构阻断分析（4轮根因）

2025年是一个**高波动震荡熊市**：
- BNB/TRX/BCH在2025涨了11-38%，但波动率29-56%（BNB最大回撤-41%），马丁做多在震荡中亏损
- AAVE/SOL/DOT崩盘-34到-73%，但波动率62-73%，频繁逼空，马丁做空被反复止损（50%止损率）
- BTC基本持平(-6%)，作为门控基准失效

**4轮测试的所有方向都无法翻转2025为正**：放宽long gate、range sleeve、breadth proxy、4-stage TP、custom ladder、active-cycle exit、rebound SO、pump-fade short、premium gate、core-satellite、direction-aware SO。

## 已完成的工程交付

### Backtest Engine (208 tests)
- Partial TP (multi-stage, partial close, breakeven)
- Conditional Safety Orders (indicator-gated)
- Equity-Reclaim Re-Entry
- Active-Cycle Exit (max_cycle_age, no_progress)
- Rebound-Confirmed SO (local extreme tracking)
- ROC expression function
- Portfolio Equity Stop (budget-based)
- Premium Index Data (176,640 rows downloaded)

### Trading Engine (187 tests)
- Conditional SO: **full parity**
- Multi-Stage Partial TP: **stage tracking parity**
- Breakeven Stop: **implemented**
- Equity-Reclaim: **implemented**

### 搜索脚本 (20+)
所有脚本在`scripts/glm_r*.py`，完整探索registry在`docs/superpowers/artifacts/glm-martingale-core-round*/exploration-registry.jsonl`。

## 需要用户决策

收益目标(50/90/110%)在马丁核心约束下经4轮穷尽后确认结构性不可达。2025从-18.2%逐轮改善到-8.70%，但仍为负。请决策：

**(A) 授权引入非马丁辅助sleeve**（纯趋势/突破作为辅助收益源）— 能突破ann天花板但改变策略性质

**(B) 接受~35%年化作为可部署组合** — r4-combo-best已完整live-parity可部署，4/5段正，DD17.7%

**(C) 放宽收益目标门禁** — 如保守档改为>35%，则r4-combo-best达标

## 分支和提交
- 4个分支：`glm-martingale-core-indicator-expansion` → `round2` → `round3` → `round4`
- 总计~80 commits，全部推送
- 最佳配置：`docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json`
- 复现命令：`target/release/portfolio_budget_replay --config <path> --budget 5000 --start-ms 1672531200000 --end-ms 1780271999999 --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile aggressive --portfolio-id repro --exchange-min-notional 5`
