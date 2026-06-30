# 2026-06-28 ChatGPT 对 GLM 小资金优化结果的复核与下一步计划

> 结论先行：按 GLM 当前已回测出的候选和报告，**现有结果里不能直接组合出同时满足“小资金 <5000U、多币种、抗过拟合、各周期均衡、保守 >50/10、平衡 >90/20、激进 >110/30、且实盘可复现”的三档组合**。
> 当前最有价值的成果是 P0 live-parity 补齐和 `best_balanced_l5_robust_b5000.json` 这个抗过拟合基线，但它还不是达标组合。

## 1. 我复核了什么

读取并核对：

- `docs/superpowers/reports/2026-06-28-small-cap-optimization-progress.md`
- `docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_floor1500_b5000.json`
- `docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_floor1500_legs5_b5000.json`
- `docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_l5_robust_b5000.json`
- `docs/superpowers/artifacts/glm-small-cap-pools/glm_robust_pool.json`
- `docs/superpowers/artifacts/glm-small-cap-pools/glm_diversified_pool.json`
- `docs/superpowers/artifacts/glm-p0-search/screen/*.json`
- `docs/superpowers/reports/replay_{conservative,balanced,aggressive}_{1000,2000,3000,4000,5000}.json`
- `docs/superpowers/plans/2026-06-28-glm-p0-structured-config-handoff.md`
- `docs/superpowers/reports/2026-06-28-martingale-tp-sl-live-parity-matrix.md`

只做只读回测/验证，没有触碰 Binance、实盘、挂单或仓位。

## 2. 三条新平衡候选的复验结果

使用 `scripts/validate_martingale_portfolio_robustness.py` 对三条新候选在 5000U 下做 full-period + 五段验证。

### 2.1 `best_balanced_floor1500_b5000.json`

- Full-period：**99.43% / 24.24%**，收益达标，DD 超平衡门槛 4.24 点。
- Segment gate：失败。
- 主要问题：
  - H1-2023 DD 24.24% > 24% segment gate。
  - 2025 DD 30.71% > 24%。
  - 2026 DD 29.81% > 24%。
  - 2024-2026 合计 return 为 **-4.6%**。
- 判断：这是“收益足够但回撤和抗过拟合不足”的候选，不能作为最终平衡组合。

### 2.2 `best_balanced_floor1500_legs5_b5000.json`

- Full-period：**68.94% / 28.16%**，收益和 DD 都不达平衡门槛。
- Segment gate：失败 1 项，H1-2023 DD 27.09% > 24%。
- 2024：+6.53%，DD 9.75%。
- 2025：-5.14%，DD 19.28%。
- 2026：+9.87%，DD 14.42%。
- 判断：这是很有价值的“抗过拟合方向验证”，但牺牲牛市收益后 full-period 年化只有约 69%，不能作为平衡达标组合。

### 2.3 `best_balanced_l5_robust_b5000.json`

- Full-period：**70.48% / 26.40%**，收益和 DD 都不达平衡门槛。
- Segment gate：**通过**。
- 2024：+5.29%，DD 10.69%。
- 2025：-4.34%，DD 19.04%。
- 2026：+10.04%，DD 15.47%。
- Live parity：通过当前 gate（Percent TP + StrategyDrawdownPct SL）。
- 判断：这是目前最健康的基线，说明“robust 卫星 + 限 legs + tighter SL”方向对过拟合有效；但距离平衡目标还差约 **+20 年化** 和 **-6.4 DD**。

## 3. 现有结果是否能通过组合达标

我的判断：**不能，至少现有已验证结果里没有足够证据支持可以组合达标。**

原因：

1. 三条新候选基本位于同一前沿：
   - `floor1500`：年化够，但 DD 和分段风险过高。
   - `legs5/l5_robust`：分段健康，但年化不够，full DD 仍偏高。
   - 简单混合这两类候选，收益会被拉低；而 DD 不会线性下降到 20 以下，因为核心 FIL/AAVE/INJ 暴露和 2025 风险窗口高度重叠。

2. 现有 screen 候选没有漏掉的达标点：
   - `glm-p0-search/screen/*.json` 中没有 DD<=20 的平衡候选。
   - ann>90 的最低 DD 约 29.83%。
   - ann>110 的 screen 候选不存在。

3. 早前 dynamic-symbol/frontier 报告已经给出类似前沿：
   - 保守 DD<=10 时最高年化大约 17-24%，远低于 50%。
   - 平衡 DD<=20 时最高年化大约 65-68%，远低于 90%。
   - 年化超过 90 时 DD 约 26% 以上。

4. 激进旧候选只是 full-period 表面达标，不抗过拟合：
   - H1-2023 极高收益。
   - H2-2023/2024/2025/2026 多段亏损。
   - 2025 segment DD 约 59%，不应视为可实盘候选。

5. 预算不是线性缩放问题：
   - 已有 `replay_*.json` 显示同一组合在 1000/2000/3000/5000 的 ann/DD 会跳变。
   - 原因是 Binance minNotional、per-strategy cap、budget-blocked legs、首单地板共同作用。
   - 因此不能假设“5000U 好，1000U 只是缩放版”。

## 4. 当前最重要的事实

GLM 这轮已经把根因定位得更清楚了：

- 旧候选不是一般过拟合，而是**收益集中在 H1-2023**。
- 2025 山寨币熊市是关键破坏段。
- BTC 下跌过滤器对“山寨独立熊市”无效，因为 2025 BTC 本身只小跌，而 INJ/AAVE/GALA/NEAR 等山寨大跌。
- 只暂停新周期不能解决已有深套仓位。
- tighter long SL 和 max_legs 限制可以显著改善 2025，但会压低 full-period 年化。

所以，下一步不能继续只扫 `multiplier / TP / SL floor / max_legs`。这些已经把当前前沿扫得比较清楚了。

## 5. GLM 下一步要怎么推进

### P1：把 `l5_robust` 定为抗过拟合基线

后续所有新机制都应和它对比：

- 基线配置：`docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_l5_robust_b5000.json`
- 基线指标：70.48% / 26.40%，segment gate 通过。

任何新候选至少要满足：

- 2024-2026 合计不能亏。
- 2025 DD 不得重新扩大到 24% 以上。
- full-period ann 必须向 90% 靠近。
- full-period DD 必须向 20% 靠近。

如果新候选只是把 H1-2023 收益拉高、2025 风险打回原形，直接淘汰。

### P2：先找“2025 正收益/低亏损”的可实盘单策略

当前组合无法达标的核心不是权重，而是缺少 2025 熊市收益源。必须先建一个 2025-focused 候选池。

搜索目标：

- 时间段：`2025-01-01..2025-12-31`
- 模型限制：只允许 Percent TP + StrategyDrawdownPct SL。
- 方向：
  - long_only
  - short_only
  - long_and_short
- 过滤器：
  - per-symbol downtrend short：`close < ema(30/50/100)`
  - per-symbol bear long 禁入：long only when `close > ema(50/100)` 或 `rsi`/`bb` mean-reversion 触发
  - 不要只用 BTC filter，要加本币种 filter。
- 标的：
  - 从 `glm_robust_pool.json` 中优先扫 BTC/TRX/XRP/BCH/ETC/LTC/HBAR/DOT 等 2025 不崩或 segment 表现较好的币种。
  - 从 `glm_diversified_pool.json` 中扫 TRX/GALA/ADA/NEAR/UNI/APT/COMP/ICP，但不要小权重挂卫星，要单独找到可赚钱参数。

单策略进入组合池的最低要求：

- 2025 total_return >= 0，或亏损不超过 -2% 且 DD <= 12%。
- 2024、2026 不能明显负收益。
- full-period 不依赖 H1-2023 单段贡献超过 70%。
- max_capital_used <= 该策略预算 cap。
- principal_breached=false。

如果找不到任何 2025 正收益单策略，要直接报告“现有 martingale-only + live-parity 模型缺少熊市收益源”，不要继续组合。

### P3：做 segment-first 组合，而不是 full-period-first 组合

当前失败来自 full-period-first 选中了 H1-2023 票据。下一轮组合搜索应改为：

1. 先按 segment 选单策略：
   - H1-2023 engine
   - 2024 engine
   - 2025 bear engine
   - 2026 engine
2. 再组合。
3. 最后看 full-period。

组合目标函数建议：

```text
score =
  full_ann
  - 4.0 * full_dd
  + 0.5 * min(segment_returns)
  - 2.0 * max(0, abs(negative_2025_return))
  - 3.0 * max(0, segment_dd_2025 - target_segment_dd)
  - h1_concentration_penalty
```

必须输出：

- H1-2023/H2-2023/2024/2025/2026 的 total_return、DD、trade_count、max_capital_used。
- 2024-2026 combined return。
- H1-2023 contribution ratio。

### P4：给已有仓位风险加“可实盘”的退出机制

目前已证实：只暂停新周期不够，因为已有深套仓位继续扩大 DD。要打破前沿，需要处理已有 cycle。

建议优先实现两个 live-parity 机制，再搜索：

1. `max_cycle_age_hours`
   - cycle 超过 N 小时仍未 TP，则按 StrategyDrawdownPct 或 market reduceOnly 平仓。
   - 回测和 trading-engine 都要实现。
   - 先扫 24h、48h、72h、120h、168h。

2. `regime_break_stop`
   - long cycle 持仓中，如果本币种 `close < ema(50/100)` 且浮亏超过 X，则停止补仓或平仓。
   - short cycle 反向。
   - 必须结构化配置 + backtest + trading-engine 测试。

注意：

- 这两个机制改变交易语义，必须先补 live parity，再进入搜索。
- 不要使用当前仍 research-only 的 `portfolio_equity_stop_pct`、`portfolio_stop_cooldown`、`max_portfolio_active_cycles` 作为最终结果，除非先补 trading-engine 实现。

### P5：重新设计主动币种数量，不要只用小权重卫星

`l5_robust` 中 robust 卫星权重是 8% 一组，但它们更多是在增加交易次数和轻微改善分段，没有成为真正收益源。下一轮应做预算自适应 K：

- 1000U：1-2 个 active symbols。
- 2000U：2-3 个 active symbols。
- 3000U：3-4 个 active symbols。
- 5000U：4-6 个 active symbols。

但 K 不能用没有实盘实现的 `max_portfolio_active_cycles` 强行控制。可以先用组合配置静态限制：

- 针对每个预算生成不同 portfolio config。
- 每个 config 只包含预算允许的 K 个策略/币种。
- 每个 config 单独回放和验证。

### P6：三档目标分别搜索，不能共享同一核心

保守、平衡、激进现在不能再用同一 FIL/AAVE/INJ 核心简单调权。

建议：

- 保守：
  - 从低 DD 单策略开始，目标先设 `DD<=10`，看年化最高能到多少。
  - 不允许 H1-2023 集中。
  - 如果仍低于 35%，说明保守目标在当前纯马丁模型下需要新交易语义或不可达。
- 平衡：
  - 从 `l5_robust` 出发，目标是增加 2025 正收益 engine，同时降低 full DD 到 20 以下。
  - 重点不是再提高 INJ/AAVE/FIL 权重。
- 激进：
  - 旧 aggressive 必须废除或降级为“表面 full-period 候选”。
  - 新 aggressive 要求 full >110/DD<=30，同时 segment gate 通过。
  - 允许更高 H1 收益，但 2024-2026 合计不能负，任一 segment DD 不得 >36。

### P7：验证工具必须修正一个小问题

`scripts/validate_martingale_portfolio_robustness.py` 里 `evaluate_gate()` 有一段：

```python
and (metrics["max_capital_used"] or 0) <= 0  # <= budget checked separately
```

这个函数目前没有被最终 full_gate 使用，但逻辑本身是错的，容易误导后续维护。应改成传入 budget 或删除该函数，避免以后被误用。

另外，验证工具要直接调用 Rust 的 `live_parity_check` 结果，避免 Python 版和 Rust 版规则漂移。

## 6. 下一轮验收标准

任何候选都必须给出以下 JSON/MD：

- full-period metrics：
  - annualized_return_pct
  - max_drawdown_pct
  - total_return_pct
  - max_capital_used_quote
  - budget_blocked_legs
  - principal_breached
- segment metrics：
  - H1-2023
  - H2-2023
  - 2024
  - 2025
  - 2026_ytd
- budget matrix：
  - 1000
  - 2000
  - 3000
  - 4000
  - 5000
- live parity：
  - Percent TP only
  - StrategyDrawdownPct SL only
  - market_data_dependencies 已列出
  - no research-only env mechanisms
- overfit flags：
  - H1 concentration ratio
  - 2024-2026 combined return
  - worst segment DD

## 7. 如果下一轮仍然找不到

如果完成 P2-P6 后仍然找不到三档达标组合，应输出失败证明，而不是继续无边界试参：

- 搜索空间。
- trial 数。
- 每档 Pareto frontier：
  - DD<=10 最高 ann。
  - DD<=20 最高 ann。
  - DD<=30 最高 ann。
  - ann>50 最低 DD。
  - ann>90 最低 DD。
  - ann>110 最低 DD。
- 是否存在 2025 正收益 live-parity 单策略。
- 是否在加入 `max_cycle_age_hours` / `regime_break_stop` 后前沿改善。

若仍无解，需要明确告诉用户：在 `<=5000U + martingale-only + live parity + 抗过拟合` 条件下，当前收益/回撤目标可能不可同时满足，必须放宽目标或引入非马丁策略族。

## 8. 当前给 GLM 的明确行动清单

1. 保留 `best_balanced_l5_robust_b5000.json` 作为抗过拟合基线。
2. 新建 2025-focused 单策略搜索，不要再 full-period-first。
3. 优先寻找 2025 正收益/低亏损的 short 或 long-short 策略。
4. 若找不到 2025 收益源，先实现 `max_cycle_age_hours` 和 `regime_break_stop` 的 live parity，再搜索。
5. 三档组合分开优化，不再共用 FIL/AAVE/INJ 核心。
6. 不使用任何没有 trading-engine 实现的 TP/SL/portfolio stop/research-only env 机制。
7. 每个候选必须通过 `validate_martingale_portfolio_robustness.py` 的 full + segment + budget + live parity 验证。

一句话：**当前已有结果已经证明“调参和简单组合”不足以达标；下一轮突破必须来自 2025 熊市收益源，或者来自可实盘复现的 cycle 级退出/减风险机制。**
