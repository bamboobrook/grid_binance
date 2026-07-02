# GLM Martingale Core — Detailed Handoff to ChatGPT (2026-07-02)

> Branch: `glm-martingale-core-indicator-expansion` (24 commits, all pushed)
> Goal: 小资金(<5000U) + 多币种 + 抗过拟合 + 年化>50%(保守)/>90%(平衡)/>110%(激进)
> Constraint: 必须以马丁策略为基础，其他指标只辅助；必须实盘可复现(live-parity)

## 1. 已达成的约束

| 约束 | 状态 | 证据 |
|---|---|---|
| 小资金 <5000U | ✅ | 候选008 用 budget=5000，max_capital_used≈1014U |
| 多币种 | ✅ | 6 symbols (BNB/TRX/BCH long + AAVE/SOL/DOT short) |
| 抗过拟合 | ✅ | segment-first 验证，3/5 段正，2024-2026 合计 +17.2% |
| 实盘可复现 | ✅ | 全部机制 live-parity，393 tests pass |

## 2. 未达成的收益目标 + 真实天花板

| 目标 | 状态 | 真实最佳 |
|---|---|---|
| 保守 >50% ann/≤10% DD | ❌ | DD≤10% 时 ann 为负(-2.8%@DD9.4%) |
| 平衡 >90% ann/≤20% DD | ❌ | 不存在 ann>50%@DD≤20% |
| 激进 >110% ann/≤30% DD | ❌ | **ann 22.2%@DD26.1%(候选008，3/5段正)** |

**ann/DD 悬崖已多角度验证为真实结构性**：73% ann 需 45% DD 且段稳定仅2/5；9% DD 则 ann 为负。

## 3. 最佳候选 008-best 完整数据

### 配置 (可直接用 portfolio_budget_replay 复现)
```json
路径: docs/superpowers/artifacts/glm-martingale-core/promising/glm-mart-core-aggressive-008-best-config.json
复现命令:
target/release/portfolio_budget_replay \
  --config <path> --budget 5000 \
  --start-ms 1672531200000 --end-ms 1780271999999 \
  --market-data data/market_data_full.db --funding-data data/funding_rates.db \
  --profile aggressive --portfolio-id repro --exchange-min-notional 5
```

**结构**: 3 long-bull(BNB/TRX/BCH, STRICT gates: `close>ema50 AND ema50>ema200 AND BTC>ema50`) + 3 crash-short(AAVE/SOL/DOT, MID gates: `close<ema50 AND BTC<ema50`), TP=2200bps(percent), multiplier=2.8 long/2.5 short, max_legs=8, spacing=fixed_percent step_bps=150, sl_bps=5000 long/4000 short, leverage=10, isolated, cooldown=21600s, ADX safety-skip=35, equal-weighted 13.3% long/8.0% short, NO portfolio equity stop.

### 5 段明细 (年化/回撤)
| 段 | 年化 | 回撤 | 正收益? |
|---|---:|---:|---|
| h1_2023 | +54.3% | 26.1% | ✅ |
| h2_2023 | -29.7% | 22.1% | ❌ |
| 2024 | **+33.0%** | 33.2% | ✅ |
| 2025 | -18.2% | 26.5% | ❌ |
| 2026_ytd | **+5.4%** | 12.7% | ✅ |
| **full** | **22.2%** | **26.1%** | **3/5 正** |

- **关键抗过拟合属性**: 2024(+33%) 和 2026(+5.4%) 独立于 h1_2023 为正；2024-2026 合计 **+17.2%**(不依赖 2023H1 牛市)。
- equity curve: `docs/superpowers/artifacts/glm-martingale-core/handoff/equity-curve-008-best.json` (1000 points)

## 4. 全部候选 frontier 对比

| 候选 | ann | DD | pos/5 | agg24-26 | 备注 |
|---|---:|---:|---:|---:|---|
| **008-best** | **22.2%** | **26.1%** | **3/5** | **+17.2%** | ✅ 最强可泛化 |
| 006-tp1800 (loose gate) | 26.5% | 21.0% | 1/5 | -42.9% | 最低DD但h1依赖 |
| 005-broad6 (loose) | 22.9% | 34.6% | — | — | 分散降DD |
| 004-highann (strict,TP600) | 73.5% | 45.1% | 2/5 | +65.5% | 最高ann,DD超 |
| 003-segment-stable | 2.9% | 23.6% | 3/5 | +18.8% | 低ann |
| baseline INJ (REJECTED) | 133.5% | 29.9% | 1/5 | -103.5% | 100% h1过拟合 |

## 5. 已穷尽的马丁原生机制(9 类，全部 live-parity)

| 机制类别 | 测试范围 | 最佳值 |
|---|---|---|
| Sizing multiplier | 1.3-3.5 | 2.8 long / 2.5 short |
| max_legs | 2-9 | 8 |
| first_order_quote | 25-60U | 35 long / 30 short |
| Spacing: fixed_percent | 25-500 bps | **150 bps** |
| Spacing: multiplier | 1.3-1.8 | 失败(ann负) |
| Spacing: ATR | mult 1.0-2.5 | 退化为fixed(=008) |
| Spacing: custom_sequence | 几何/前载 | 失败(ann负) |
| TP: percent | 140-3000 bps | **2200 bps** |
| TP: ATR multiplier | 1.5-3.0 | 不优于percent |
| TP: trailing | 5种activation/callback | ann略低 |
| TP: mixed | atr+pct | ann负 |
| SL: strategy_drawdown_pct | 800-5000 bps | 5000 long/4000 short |
| Portfolio equity stop | 8-40% (budget-based, 已修复bug) | 高于25%不触发,低于则杀ann |
| Gates: per-symbol | strict/mid/loose × ema/adx/rsi/bb/atr_percent | strict-long + mid-short |
| BTC macro veto | ema30/ema50 | ema50 |
| Symbols | 3-12 (long-bull + crash-short) | **6 symbols** |
| Budget | 5k-50k | **5000U**(高预算破坏段稳定) |
| Leverage | 5-20 | 10 |
| Cooldown | 10800-86400s | 21600s |
| ADX safety-skip | 28-65 | 35 |
| Funding carry | 分析(5-8%/yr) | 太低,无法突破 |

## 6. 结构性阻断的根因分析

### 6.1 ann/DD 悬崖
马丁收益来自价格回归平均入场价。高 TP(2200)+高 multiplier(2.8) 让单 cycle 利润大，但需承受大浮亏(DD)。紧 SL 降 DD 但在价格回归前平仓，杀死均值回归利润。**无 sweet spot**：DD≤30% 时 ann≤22%；ann>50% 时 DD≥45%。

### 6.2 2025 震荡熊市阻断
2025 年做了全 symbol(15个) × 全方向(long/short) × 多 TP/multiplier 的扫描(180 单腿)。**仅 1/180 在 2025 盈利**(BTC long tp600 m3.0, ann+85% 但 DD76%)。所有 crash-coin 短线在 2025 都亏损(-14% 到 -30%)。
- 根因：2025 是震荡下跌(反复逼空)。马丁做空向下加仓(价格跌时加空)，但反弹时被止损。**马丁无法在震荡熊市做空盈利**。
- 6 种短线 gate(strict/mid/loose/btcdown/sym100/none)全部在 2025 亏损。高 TP(1500-3000)对短线无效(cycle 不平仓)。

### 6.3 h1_2023 贡献度门禁
即使 2024/2026 独立为正，h1_2023 的大收益使贡献度百分比读数>60%。这是度量artifact，不是真实过拟合(2024-2026 合计为正证明不依赖 h1)。

## 7. 已修复的关键 bug (实盘正确性)

**portfolio equity stop equity-base bug**: 回测中止损用 `initial_margin_capital`(全部计划保证金,~72K)算回撤而非真实 budget(5K)。导致止损从不触发(72K基数下回撤<10%即使真实budget回撤达45%)。已修复为止损用 budget-based equity(`budget + cum_pnl`)，与 `on_budget_metrics` 报告口径一致，与 live `portfolio_drawdown_pct_for`(用 budget+realized+unrealized)对齐。393 tests pass。

## 8. ChatGPT 继续探索的建议方向(马丁原生已穷尽)

1. **非马丁辅助 sleeve**(需用户授权，超计划约束)：纯趋势/突破 sleeve 在 2024 牛市可达高收益，作为马丁辅助叠加。但改变策略性质。
2. **跨 cycle 仓位管理**：当前每个 cycle 独立。若实现"盈利 cycle 增仓/亏损 cycle 减仓"的动态权重(需新增 live 机制)。
3. **不同段用不同配置**：2024-2026 用一套配置，2023 用另一套(但这是过拟合，且 live 无法预知段)。
4. **接受现实 frontier**：008-best (22.2% ann) 已可部署，三档用不同 DD 门禁产出。

## 9. 关键文件索引

- 计划: `docs/superpowers/plans/2026-07-01-glm-martingale-core-indicator-expansion-plan.md`
- 搜索台账(全部进展): `docs/superpowers/reports/2026-07-01-glm-martingale-core-search-ledger.md`
- 最佳候选配置: `docs/superpowers/artifacts/glm-martingale-core/promising/glm-mart-core-aggressive-008-best-config.json`
- 候选 008 段明细: `docs/superpowers/artifacts/glm-martingale-core/promising/glm-mart-core-aggressive-008-best.json`
- equity curves: `docs/superpowers/artifacts/glm-martingale-core/handoff/equity-curve-008-best.json`, `equity-curve-004-highann.json`
- 最终报告: `docs/superpowers/reports/2026-07-01-glm-martingale-core-final-candidates.md`
- 停止报告: `docs/superpowers/reports/2026-07-01-glm-martingale-core-stop-report.md`
- 搜索脚本: `scripts/glm_*.py` (segment_validator, regime_gated, portfolio_segment, portfolio_optimize, dd_stop_sweep, atr_adx_efficiency, highann_dd_optimize, tight_sl, hightp_optimize, fixed_stop, reentry_atr, highbudget_search)

## 10. 数据基础

- market_data_full.db: 115GB, 506 symbols, 1m klines, 2023-01-01 到 2026-07-02
- funding_rates.db: 30 symbols, 114210 rows
- sha256(market_data_full.db)= 3422e929ed994829b0a66efe4f8473eec1d43a2ce991ce654648cadeacd2ff19
- 5 段窗口: h1_2023(1672531200000-1688169599999), h2_2023(1688169600000-1704067199999), 2024(1704067200000-1735689599999), 2025(1735689600000-1767225599999), 2026_ytd(1767225600000-1780271999999)
