# 马丁回测结果完整性与真实组合回测修复设计

**状态:** 待用户确认后交给 Claude 执行  
**日期:** 2026-05-18  
**目标:** 修复当前马丁回测在多空、年化、图表、交易明细、候选数量、组合回测、杠杆本金计算上的偏差，使结果可用于后续组合筛选与实盘发布评估。

## 1. 背景与问题

用户执行一轮马丁自动回测后发现 6 个关键问题：

1. 选择 `long+short` 后，结果只有 `long`，没有同时回测多头与空头腿。
2. 结果没有年化收益率，无法横向比较不同时间窗口或组合。
3. 没有收益/回撤图表，也没有交易明细，无法核对策略执行过程。
4. 组合无法查看详细信息；且当前“组合”实际只是从单策略结果中挑最优，不是真正的资金组合。
5. 回测结果太少，后续无法从充足候选中进行组合优化。
6. 杠杆没有正确参与回测；收益和回撤必须按杠杆前实际本金/计划保证金计算。

补充定义：

- “组合”不是选择单个最优策略。
- “组合”必须是将总资金按照一定比例分配给多个策略，基于各策略真实资金曲线合成整个组合资金曲线，并计算组合收益、组合最大回撤、组合年化、组合收益/回撤比、组合交易明细和组合配置。

## 2. 目标体验

用户输入：

- 交易对列表。
- 方向：`long`、`short`、`long_short`。
- 风险档位/最大回撤限制。

系统输出：

- 每个交易对尽量多的合格候选，前端至少展示单币种 Top 10，并在详情中能看到同币种更多候选数量统计。
- `long_short` 必须产生包含 long leg 与 short leg 的候选；两边可使用不同参数、不同权重。
- 每个候选展示：总收益、年化收益、最大回撤、收益/回撤比、Score、杠杆、计划保证金、手续费/滑点、交易数、方向腿组成。
- 每个候选可查看资金曲线、回撤曲线、交易明细。
- 组合 Top 3 必须是真实组合回测结果：由多个候选按资金权重组合而成，不得退化为单策略 Top 3。
- 组合详情展示：成员策略、资金权重、每个成员的收益/回撤/杠杆/方向、组合资金曲线、组合回撤曲线、组合交易明细摘要。

## 3. 明确行为要求

### 3.1 long_short 必须双腿回测

当请求方向为 `long_short`：

- 候选配置必须包含至少一条 long 策略腿和一条 short 策略腿。
- long 与 short 不得共享同一个方向字段后只跑 long。
- long 与 short 可以使用不同：
  - `spacing_bps`
  - `take_profit_bps`
  - `tail_stop_bps`
  - `max_legs`
  - `order_multiplier`
  - `leverage`
  - `weight_pct`
- 单候选结果的 `direction` 或 `direction_mode` 应明确为 `long_short`。
- 候选详情必须展示 long leg 与 short leg 的参数。

验收标准：

- 对 BTCUSDT + ETHUSDT 选择 `long_short` 执行任务后，返回的合格候选中至少存在 `direction_mode=long_short`，且其配置中同时包含 `Long` 与 `Short` 策略。
- 前端结果不能只显示 `long`。

### 3.2 年化收益率

每个单策略候选与组合候选必须计算并持久化：

- `annualized_return_pct`
- `return_drawdown_ratio`
- `backtest_days`

计算规则：

```text
ending_equity = initial_planned_margin + net_pnl
period_return = ending_equity / initial_planned_margin - 1
annualized_return = (1 + period_return)^(365 / backtest_days) - 1
annualized_return_pct = annualized_return * 100
```

边界：

- `backtest_days <= 0` 时年化为 `null` 并记录 `annualized_unavailable`。
- `ending_equity <= 0` 时年化为 `-100%` 或按现有数值模型安全截断，不能 NaN/Inf。
- 前端显示 `—` 代替 null。

### 3.3 图表与交易明细

每个候选 artifact 必须包含：

- `equity_curve`: 时间戳 + 资金值。
- `drawdown_curve`: 时间戳 + 当前回撤百分比。
- `trades`: 交易明细数组。

交易明细字段最低要求：

```json
{
  "timestamp_ms": 1672531200000,
  "symbol": "BTCUSDT",
  "direction": "long",
  "event_type": "open_leg|close_cycle|stop_loss|liquidation_guard|fee",
  "leg_index": 1,
  "price": 16500.0,
  "margin_quote": 10.0,
  "notional_quote": 30.0,
  "leverage": 3,
  "fee_quote": 0.012,
  "slippage_quote": 0.005,
  "realized_pnl_quote": 0.8,
  "equity_after_quote": 150.8
}
```

前端要求：

- 候选详情区展示资金曲线和回撤曲线。
- 鼠标悬停可看到日期、资金、收益百分比、回撤百分比。
- 交易明细支持展开查看，默认展示最近或关键 100 条，并显示总交易数。
- 如果 artifact 缺少曲线/交易，必须显示“数据缺失原因”，不得静默空白。

### 3.4 真实组合回测

组合 Top 3 生成方式必须重构：

- 输入是单策略候选池，不是直接取单策略 Top 3。
- 系统从候选池中搜索多个成员策略组成组合。
- 每个组合包含 2 到 N 个成员，N 可配置，默认最多 8 个。
- 每个成员有资金权重 `allocation_pct`，总和必须等于 100%。
- 组合收益曲线由成员候选资金曲线按权重合成：

```text
portfolio_equity[t] = sum(member_normalized_equity[t] * allocation_pct / 100)
```

其中每条成员曲线必须先归一化到该成员分配本金：

```text
member_normalized_equity[t] = allocation_capital * candidate_equity[t] / candidate_initial_planned_margin
```

- 组合最大回撤从合成后的 `portfolio_equity` 计算。
- 组合年化收益从合成后的起止资金计算。
- 组合交易明细为成员交易明细按时间合并，并附加 `member_candidate_id` 与 `allocation_pct`。
- 组合不得只包含 1 个成员，除非候选池不足且返回 `portfolio_degraded_single_member=true`，此时不得进入正式 Top 3，只能作为诊断。

组合排序：

- 必须满足最大回撤限制。
- 必须正收益。
- 优先收益/回撤比高。
- 曲线稳定性、候选分散度、币种分散度作为加分项。
- 高相关同向候选不能全部堆满，应有集中度惩罚。

组合详情前端必须展示：

- 组合总收益、年化收益、最大回撤、收益/回撤比、Score。
- 成员列表：交易对、方向、权重、杠杆、收益、回撤、Score、参数摘要。
- 组合资金曲线、组合回撤曲线。
- 组合交易明细摘要。
- “查看详情”入口可打开完整详情。

### 3.5 候选数量

为组合留出足够素材：

- 每个交易对内部候选池应尽量保留所有满足最大回撤限制、正收益、数据质量合格的候选。
- 前端主表展示 Top 10，但 API/artifact 中应包含更多 `eligible_candidates` 统计。
- Worker 持久化候选时，默认每个交易对至少尝试保留 20 个合格候选给组合搜索；若不足，必须在任务 summary 中说明原因：
  - 数据不足。
  - 无正收益。
  - 超过最大回撤。
  - 交易数不足。
  - 强平/尾部风险失败。
- 在最大回撤限制下，应尽量多找符合要求结果，而不是只返回极少数。

### 3.6 杠杆与本金计算

逐仓马丁回测必须按以下规则：

- 用户/搜索参数中的首单金额视为保证金 `margin_quote`。
- `notional_quote = margin_quote * leverage`。
- 价格变化产生的 PnL 按名义仓位计算。
- 手续费按名义成交额计算。
- 滑点按名义成交额或成交价格影响计算。
- 收益率、回撤、资金曲线按杠杆前实际计划本金/计划保证金计算。
- 多层计划本金：

```text
planned_margin = first_margin + first_margin*multiplier + ... + first_margin*multiplier^(max_legs-1)
```

示例：

- 首层保证金 10U。
- 倍投 2 倍。
- 最大 4 层。
- 计划保证金 = `10 + 20 + 40 + 80 = 150U`。
- 2 倍杠杆时第一层名义仓位 = 20U。
- 第一层价格上涨 1% 的毛 PnL = 0.2U。
- 候选收益率贡献按 `0.2 / 150`，不是 `0.2 / 10`。

验收标准：

- 测试必须覆盖上述例子。
- 前端候选详情必须展示杠杆、计划保证金、名义仓位说明。

## 4. API/Artifact 合约

单候选 summary 至少包含：

```json
{
  "candidate_id": "...",
  "symbol": "BTCUSDT",
  "direction_mode": "long_short",
  "total_return_pct": 52.3,
  "annualized_return_pct": 18.7,
  "max_drawdown_pct": 14.2,
  "return_drawdown_ratio": 3.68,
  "score": 86.4,
  "trade_count": 1830,
  "planned_margin_quote": 150.0,
  "max_leverage_used": 5,
  "total_fee_quote": 42.1,
  "total_slippage_quote": 8.4,
  "legs": [
    {"direction":"long","weight_pct":60,"leverage":4,"spacing_bps":120,"max_legs":5,"take_profit_bps":80},
    {"direction":"short","weight_pct":40,"leverage":3,"spacing_bps":180,"max_legs":4,"take_profit_bps":100}
  ],
  "equity_curve": [],
  "drawdown_curve": [],
  "trades_preview": [],
  "artifact_path": "..."
}
```

组合 summary 至少包含：

```json
{
  "portfolio_id": "...",
  "portfolio_rank": 1,
  "member_count": 5,
  "total_return_pct": 60.0,
  "annualized_return_pct": 21.0,
  "max_drawdown_pct": 16.5,
  "return_drawdown_ratio": 3.64,
  "score": 88.0,
  "members": [
    {"candidate_id":"...","symbol":"BTCUSDT","direction_mode":"long","allocation_pct":25.0,"leverage":4,"total_return_pct":55.0,"max_drawdown_pct":14.0},
    {"candidate_id":"...","symbol":"ETHUSDT","direction_mode":"short","allocation_pct":15.0,"leverage":3,"total_return_pct":28.0,"max_drawdown_pct":11.0}
  ],
  "equity_curve": [],
  "drawdown_curve": [],
  "trades_preview": [],
  "artifact_path": "..."
}
```

## 5. 非目标

- 本轮不要求接入真实交易下单。
- 本轮不要求新做实盘动态调仓。
- 本轮不要求追求一定盈利承诺；目标是修复回测真实性、完整性和组合定义。

## 6. 验收清单

- [ ] `long_short` 任务可产生包含 long + short 双腿的候选。
- [ ] 单候选与组合均有年化收益率。
- [ ] 单候选与组合均有资金曲线、回撤曲线、交易明细。
- [ ] 组合 Top 3 是多成员资金权重组合，不是单策略挑选。
- [ ] 每个交易对尽量保留足够合格候选，结果不足时有原因统计。
- [ ] 杠杆按名义仓位计算 PnL，按计划保证金计算收益率/回撤。
- [ ] 前端能查看候选详情与组合详情。
- [ ] 测试覆盖核心合约，Worker/API/Web 构建通过。
