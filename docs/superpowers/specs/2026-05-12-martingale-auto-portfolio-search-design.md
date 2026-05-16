# 马丁自动组合搜索与权重编排优化设计

**状态:** 已确认，待实施  
**日期:** 2026-05-12  
**目标分支:** `feature/full-v1`  
**背景:** 用户确认采用“每币种 Top 5 + 组合编排”的方案。

## 1. 目标

当前马丁回测向导仍要求用户手动填写统一的加仓间距、首单金额、倍率、最大层数、止盈、杠杆等参数。用户真实目标是：只选择币种、市场、方向和风险承受程度，系统自动为每个币种寻找适合自身波动特征的马丁参数，再由用户从每币种 Top 5 参数组中挑选多个策略实例，设置资金权重合计 100%，并可微调推荐杠杆后发布到实盘组合。

本优化必须让回测从“手动填参数”升级为“风险画像驱动自动搜索 + 人工组合编排”。

## 2. 参数语义修正

### 2.1 移动回撤不是止损

UI 中原有 `trailingPct` 容易被理解为止损。其真实语义应为“移动止盈回撤”：

- 先达到整体止盈阈值，例如 `takeProfitPct = 1%` 后才激活。
- 激活后，从激活后的最大有利价格或最大浮盈回撤 `trailingPct` 才止盈离场。
- 因此 `trailingPct = 0.4%` 不应在价格下跌 0.4% 时提前止损，也不应阻止 1% 间距的补仓。
- UI 文案必须改为“移动止盈回撤”，并明确“不是止损”。

### 2.2 真正止损独立配置

止损归入风险规则：

- 组合最大回撤。
- 单策略最大回撤。
- ATR 止损。
- 价格区间止损。
- 止损次数熔断。

## 3. 自动时间范围

默认自动回测时间范围：

- 起点固定为 `2023-01-01`。
- 终点为当前日期所在月份的上个月月底。
- 例如当前日期为 `2026-05-12` 时，终点为 `2026-04-30`。

自动切分策略：

- train: 起点到总跨度约 70%。
- validate: 后续约 15%。
- test: 最后约 15%。
- stress windows: 自动附带 `flash_crash`、`trend_up`、`trend_down`、`high_volatility` 标签，供结果展示与后续评分扩展使用。

UI 应显示自动计算出的 train / validate / test 区间；高级模式允许手动覆盖，但默认不要求用户输入日期。

## 4. 自动参数搜索

### 4.1 用户只需配置

向导默认模式下用户只需配置：

- symbol 白名单或黑名单。
- market: spot 或 USDT-M futures。
- direction: long only、short only、long + short。
- risk profile: conservative、balanced、aggressive。
- 每币种输出数量，默认 Top 5。

### 4.2 每币种独立参数空间

系统必须按每个币种独立生成搜索空间，而不是所有币种统一间距。第一版先在前端和 worker contract 中显式表达 per-symbol intent，后端搜索输出按 symbol 分组 Top 5。

风险画像对应默认参数空间：

- conservative:
  - spacing_bps: `[120, 160, 220, 300]`
  - first_order_quote: `[8, 10, 15]`
  - order_multiplier: `[1.25, 1.4, 1.6]`
  - take_profit_bps: `[60, 80, 100]`
  - max_legs: `[3, 4, 5]`
  - leverage: futures `[1, 2]`
- balanced:
  - spacing_bps: `[80, 120, 160, 220]`
  - first_order_quote: `[10, 15, 25]`
  - order_multiplier: `[1.4, 1.6, 2.0]`
  - take_profit_bps: `[80, 100, 130]`
  - max_legs: `[4, 5, 6]`
  - leverage: futures `[2, 3, 4]`
- aggressive:
  - spacing_bps: `[50, 80, 120, 160]`
  - first_order_quote: `[10, 20, 35]`
  - order_multiplier: `[1.6, 2.0, 2.4]`
  - take_profit_bps: `[100, 130, 180]`
  - max_legs: `[5, 6, 8]`
  - leverage: futures `[3, 5, 8]`

后续可以用 ATR/波动率动态放大或缩小这些空间；本轮必须先落地可解释、可测试的风险画像空间。

## 5. 每币种 Top 5 结果

Worker 保存候选时必须支持按 symbol 分组：

- 每个 symbol 至多保留 Top 5 候选。
- 候选 summary 必须包含：
  - `symbol`
  - `recommended_weight_pct`
  - `recommended_leverage`
  - `risk_profile`
  - `parameter_rank_for_symbol`
  - `portfolio_group_key`
  - `spacing_bps`
  - `first_order_quote`
  - `order_multiplier`
  - `max_legs`
  - `take_profit_bps`

排序仍使用生存优先评分：先过滤不可生存候选，再按 rank score 排序。

## 6. 组合编排 UI

结果页增加“组合篮子”概念：

- 从每币种 Top 5 结果里勾选一个或多个参数组。
- 每个选中实例显示 symbol、方向、参数摘要、推荐杠杆、推荐权重。
- 用户可以修改每个实例权重和杠杆。
- 权重合计必须显示；合计为 100% 时为绿色，否则为黄色提示。
- 第一版可以先作为前端编排视图与发布前说明，不必须把篮子作为新 API 提交；候选发布仍走单候选 publish-intent。后续再扩展为多候选组合发布 API。

## 7. 验证要求

必须新增或更新测试证明：

- 自动时间范围以当前日期计算到上个月月底。
- `trailingPct` 文案为移动止盈回撤，不再误称止损。
- 向导 payload 包含 `per_symbol_top_n: 5` 与 `risk_profile`。
- Worker 保存候选时按 symbol 分组，且每个 symbol 最多 5 个。
- 候选 summary 包含推荐权重、推荐杠杆、symbol 内排名。
- 前端 build 通过。

## 8. 非目标

- 本轮不承诺 GPU 加速。
- 本轮不实现完全无人值守自动实盘发布。
- 本轮不强制改变现有单候选 publish-intent API。
- 本轮不把风险画像做成机器学习模型。
