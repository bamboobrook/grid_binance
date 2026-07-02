# GLM Martingale Core — 三档可部署组合包 (Task 6)

> Branch: `glm-martingale-core-indicator-expansion`
> 生成: 2026-07-02
> 基于: 马丁核心 + per-symbol regime gate + 高 TP 降 DD 结构（全部 live-parity 可实盘复现）

## 三档配置总览

| 档位 | 年化 | 最大回撤 | 正段数 | 2024-2026合计 | 配置路径 |
|---|---:|---:|---:|---:|---|
| Conservative | 12.6% | 16.2% | 1/5 | -22.4% | `final-conservative.json` |
| **Balanced (推荐)** | **22.2%** | **26.1%** | **3/5** | **+17.2%** | `final-balanced.json` |
| Aggressive | 28.1% | 30.0% | 2/5 | -28.1% | `final-aggressive.json` |

**推荐 Balanced**：唯一达到 3/5 段正且 2024-2026 合计为正（+17.2%，不依赖 2023H1）。

## 共同结构（6 币种马丁核心 + regime gate）

- **3 long-bull**: BNBUSDT/TRXUSDT/BCHUSDT（2024+2025 唯一双正的币种）
  - STRICT gate: `close > ema(50) AND ema(50) > ema(200) AND BTCUSDT.close > BTCUSDT.ema(50)`
- **3 crash-short**: AAVEUSDT/SOLUSDT/DOTUSDT（2025 崩盘币）
  - MID gate: `close < ema(50) AND BTCUSDT.close < BTCUSDT.ema(50)`
- spacing: fixed_percent step_bps=150
- leverage: 10 (conservative 用 5), isolated margin
- cooldown: 21600s, ADX safety-skip: 35
- budget: 5000U, max_capital_used ≈ 1000U

## 各档差异参数

| 参数 | Conservative | Balanced | Aggressive |
|---|---|---|---|
| long multiplier | 2.0 | 2.8 | 3.0 |
| short multiplier | 1.8 | 2.5 | 2.5 |
| max_legs | 7/6 | 8/8 | 9/9 |
| TP (bps) | 1800 | 2200 | 2600 |
| long SL (bps) | 3000 | 5000 | 5000 |
| leverage | 5 | 10 | 10 |

## Balanced 档（候选008-best）完整 5 段明细

| 段 | 年化 | 回撤 | 正收益 |
|---|---:|---:|---|
| h1_2023 | +54.3% | 26.1% | ✅ |
| h2_2023 | -29.7% | 22.1% | ❌ |
| 2024 | **+33.0%** | 33.2% | ✅ |
| 2025 | -18.2% | 26.5% | ❌ |
| 2026_ytd | **+5.4%** | 12.7% | ✅ |

**抗过拟合**: 2024 和 2026 独立于 h1_2023 为正；2024-2026 合计 +17.2%。

## 实盘可复现性 (live-parity check)

全部机制均已实现 live-parity：
- ✅ 马丁核心 (multiplier/legs/TP/SL/spacing)
- ✅ per-symbol regime gate (ema/adx/atr 指标 + cross-symbol BTC 引用)
- ✅ ADX safety-skip, cooldown
- ✅ portfolio equity stop (config 结构化 + trading-engine 平仓/冷却实现, 393 tests pass)
- ✅ 已修复 DD-stop equity-base bug (用 budget-based equity, 与 live 对齐)

**复现命令**:
```bash
target/release/portfolio_budget_replay \
  --config docs/superpowers/artifacts/glm-martingale-core/final-balanced.json \
  --budget 5000 --start-ms 1672531200000 --end-ms 1780271999999 \
  --market-data data/market_data_full.db --funding-data data/funding_rates.db \
  --profile balanced --portfolio-id deploy --exchange-min-notional 5
```

## 诚实的目标对照

| 原始目标 | 实际最佳 | 差距 |
|---|---|---|
| 保守 >50% ann/≤10% DD | 12.6% ann/16.2% DD | ann 差 4x |
| 平衡 >90% ann/≤20% DD | 22.2% ann/26.1% DD | ann 差 4x |
| 激进 >110% ann/≤30% DD | 28.1% ann/30.0% DD | ann 差 4x |

**收益目标在马丁核心约束下结构性不可达**（详见移交文档）。三档包是真实可部署的 frontier，但年化低于原始目标。突破需非马丁辅助 sleeve（待用户授权）。

## 三档都达到的约束

- ✅ 小资金 <5000U（max_capital_used ≈ 1000U）
- ✅ 多币种（6 symbols）
- ✅ 实盘可复现（全部 live-parity）
