# 马丁组合实盘交易所自动预配置设计

日期：2026-05-25
范围：马丁组合发布到实盘前，自动设置 Binance USDT-M Futures 的双向持仓、逐仓/全仓与杠杆，并在启动前做风险确认和一致性校验。

## 背景

当前马丁组合已经可以从回测结果发布为实盘组合，并将每个策略成员的权重、方向、逐仓模式、杠杆等参数写入组合配置。但启动实盘前，交易所侧的状态仍依赖用户手动设置：

- Long+Short 组合要求 Binance Futures 开启 Hedge Mode。
- 每个交易对需要匹配策略要求的 `margin_mode`，当前主要为 `isolated`。
- 每个交易对需要匹配策略成员的 `leverage`。

如果交易所状态不一致，当前运行时会 preflight 拒绝启动；用户体验是“发布了组合但启动不了”，且容易误以为系统已经配置好交易所。

## 目标

新增“预配置交易所”能力：用户在实盘组合页面点击按钮后，系统自动调用 Binance Futures API，把交易所状态调整到组合要求，再做二次读取校验。只有校验成功的组合，才允许继续人工确认启动。

成功标准：

1. 对 Long+Short 组合，可自动开启 Hedge Mode，并展示这是账户级变更。
2. 对组合中的每个 USDT-M Futures symbol，可自动设置 margin type 与 leverage。
3. 自动配置完成后，系统读取交易所状态并确认与组合配置一致。
4. 配置过程必须有明确风险确认，不允许静默自动执行。
5. 配置失败时，不能启动实盘；页面展示失败 symbol、目标值、错误信息、下一步建议。
6. 每个策略成员的杠杆继续从回测组合成员传递到发布组合，并同步到交易所设置与运行时。

## 非目标

本轮不做以下内容：

- 不自动下实盘订单；预配置只负责交易所账户/symbol 设置。
- 不修改现货策略逻辑；本功能只针对 Binance USDT-M Futures。
- 不绕过 Binance 限制强制修改：若 symbol 有持仓或挂单导致 margin type 修改失败，系统只提示用户处理。
- 不做跨交易所抽象；接口先落在现有 Binance client。
- 不自动调高到超过回测值的杠杆；交易所杠杆必须等于组合成员配置。

## 关键风险约束

### 账户级 Hedge Mode

Binance 的 position mode 是账户级 Futures 设置，不是单个 symbol 设置。开启 Hedge Mode 会影响该账户下 USDT-M Futures 的持仓模式。因此：

- 前端必须展示“这是账户级设置，会影响该账户所有 USDT-M Futures 交易”。
- 后端必须要求明确字段，例如 `confirm_account_level_hedge_mode_change=true`。
- 若组合不是 Long+Short，但账户当前是 Hedge Mode，不应自动关闭；避免影响其他策略。

### Margin Type 修改限制

Binance 修改 margin type 可能因已有持仓/挂单失败。系统处理：

- 先查 open orders / positions，能识别风险时提前阻断并提示。
- 调用失败时记录 Binance 错误码与消息。
- 不在本轮自动撤单或平仓。

### 杠杆一致性

同一 symbol 的 long/short 在 Binance 上共享杠杆与 margin type。系统处理：

- 发布服务已要求同 symbol 的 margin/leverage 不能冲突。
- 预配置阶段再次按 symbol 聚合目标配置。
- 如果同 symbol 目标值冲突，拒绝预配置。

## 用户流程

1. 用户从回测结果发布组合到实盘组合，组合状态为 `pending_confirmation`。
2. 用户进入 `马丁组合详情页`。
3. 页面显示“交易所预配置”卡片：
   - 目标 Hedge Mode：需要/不需要。
   - 每个 symbol 的目标 margin type、leverage。
   - 当前交易所状态：未检查/一致/不一致/无法读取。
4. 用户点击“检查交易所配置”。
   - 系统只读取，不修改。
   - 展示差异清单。
5. 若存在差异，用户勾选风险确认后点击“自动预配置交易所”。
   - 系统按顺序执行：Hedge Mode → margin type → leverage → 二次读取校验。
6. 校验全部一致后，页面显示“交易所配置已就绪”。
7. 用户再点击“确认启动实盘组合”。
   - 启动前仍执行现有 runtime preflight。
   - 若预配置后交易所状态被人工改动，启动仍会被拒绝。

## 后端设计

### Binance client 新增能力

在 `crates/shared-binance/src/client.rs` 增加 USDT-M Futures signed POST 接口：

- `set_usdm_position_mode(dual_side_position: bool)`
  - Endpoint: `POST /fapi/v1/positionSide/dual`
  - 参数：`dualSidePosition=true|false`
- `set_usdm_margin_type(symbol, margin_type)`
  - Endpoint: `POST /fapi/v1/marginType`
  - 参数：`symbol=BTCUSDT`, `marginType=ISOLATED|CROSSED`
  - 对 Binance 返回“无需修改/已经是目标模式”的错误码应视为 idempotent success。
- `set_usdm_leverage(symbol, leverage)`
  - Endpoint: `POST /fapi/v1/leverage`
  - 参数：`symbol`, `leverage`
- 读取当前状态：
  - 继续使用 `GET /fapi/v1/positionSide/dual` 读取 Hedge Mode。
  - 新增读取 symbol 当前 leverage/margin type 的方法，优先使用 `/fapi/v2/positionRisk` 或账户持仓信息中可稳定解析的字段。

### API server 新增服务

在实盘组合服务中新增“交易所预配置”动作：

- `GET /martingale-portfolios/{id}/exchange-preflight`
  - 读取组合目标配置与交易所当前状态。
  - 返回每个 symbol 的 `target/current/status/message`。
- `POST /martingale-portfolios/{id}/exchange-preconfigure`
  - 请求体必须包含风险确认字段：
    - `confirm_account_level_hedge_mode_change`
    - `confirm_no_auto_orders`
    - `confirm_symbol_margin_leverage_change`
  - 只允许组合 owner 操作。
  - 只允许 `pending_confirmation` 或 `paused` 的组合执行。
  - 执行后写入审计事件/组合 risk_summary。

### 执行顺序

1. 解析组合 `portfolio_config.strategies`。
2. 过滤 `market=usd_m_futures` 的策略。
3. 聚合 symbol 目标：`symbol -> {margin_mode, leverage}`。
4. 若 Long+Short，确保 Hedge Mode 目标为 true。
5. 调用 Binance：
   - 若需要 Hedge Mode 且当前不是，则 set position mode true。
   - 对每个 symbol 设置 margin type。
   - 对每个 symbol 设置 leverage。
6. 二次读取当前状态。
7. 返回配置结果，并保存到 `risk_summary.exchange_preconfigure`。

## 前端设计

在马丁实盘组合详情页新增“交易所预配置”区块：

- 显示组合成员：symbol、方向、权重、目标杠杆、目标 margin type。
- 显示账户级提示：Long+Short 需要 Hedge Mode，属于账户级设置。
- 按钮：
  - “检查交易所配置”：只读。
  - “自动预配置交易所”：带三项确认 checkbox。
  - “确认启动”：只有检查或预配置成功后更明显，但后端仍最终校验。
- 对错误进行人类可读展示：
  - “已有持仓或挂单，Binance 不允许修改逐仓模式，请先处理该 symbol。”
  - “API Key 没有 Futures 交易权限。”
  - “Hedge Mode 与当前账户状态不一致。”

## 数据与审计

不新增独立表，先复用：

- `martingale_portfolios.risk_summary.exchange_preconfigure`
- `martingale_live_snapshots` 可展示最新状态
- 现有事件/通知机制若可用，记录操作摘要

建议保存字段：

```json
{
  "exchange_preconfigure": {
    "status": "succeeded|failed|partial|checked",
    "checked_at": "ISO-8601",
    "configured_at": "ISO-8601",
    "hedge_mode": { "target": true, "current": true, "changed": false },
    "symbols": [
      {
        "symbol": "BTCUSDT",
        "target_margin_mode": "isolated",
        "current_margin_mode": "isolated",
        "target_leverage": 6,
        "current_leverage": 6,
        "status": "ok",
        "message": ""
      }
    ]
  }
}
```

## 测试要求

### Rust 单元测试

- Binance client signed request 测试：
  - position mode POST path 和参数正确。
  - margin type POST path 和参数正确。
  - leverage POST path 和参数正确。
  - “already target margin type” 类型错误可幂等处理。
- API service 测试：
  - 缺少风险确认字段时拒绝预配置。
  - Long+Short 组合会要求 Hedge Mode 确认。
  - 同 symbol 杠杆冲突拒绝。
  - 配置成功后写入 `risk_summary.exchange_preconfigure`。

### 前端测试/构建

- 组合详情页可展示目标杠杆和 margin mode。
- 未勾选确认时按钮不可提交或后端拒绝。
- `npm run build` 通过。

### 集成验证

- 使用 mock/fake Binance client 验证调用顺序：Hedge Mode → margin type → leverage → readback。
- 实盘环境不使用真实 API 做自动化测试；真实 API 调用必须由用户在 UI 点击触发。

## 上线与回滚

上线步骤：

1. 增加后端 client/service/API 测试。
2. 增加前端 UI。
3. 部署 api-server/web/trading-engine 如需要。
4. 使用只读检查接口验证现有组合目标与交易所状态展示。
5. 不自动对任何组合执行预配置；必须用户点击。

回滚：

- 回滚 API/UI 即可；已写入的 `risk_summary.exchange_preconfigure` 是附加 JSON，不影响现有组合运行。
- 若用户已经改了 Binance 账户设置，系统回滚不会自动恢复，需用户在交易所或后续工具中手动调整。

## 自检

- 无占位符/TBD。
- 已明确 API 边界、用户确认、失败处理和测试要求。
- 已区分账户级 Hedge Mode 与 symbol 级 margin/leverage。
- 未承诺自动下单，仍保留人工确认启动。

## 页面信息架构优化补充

### 问题

当前回测页和马丁实盘组合页承载了大量信息：任务列表、创建向导、候选 Top10、组合 Top3、图表、交易明细、组合沙盒、发布篮子、实盘组合状态等。多个区域纵向堆叠且同时展开，导致：

- 用户难以判断当前主线动作：是创建回测、看结果、调组合，还是发布实盘。
- 图表区域和表格区域相互挤压，资金曲线/回撤曲线观察不清楚。
- 组合沙盒、发布篮子、候选列表同时出现，容易误操作。
- 实盘组合详情页即将新增“交易所预配置”，如果继续堆叠会更拥挤。

### 回测页目标布局

回测页改成“三段式工作台”：

1. 顶部：任务创建与任务状态
   - 左侧为“创建回测任务”折叠面板。
   - 右侧为“任务列表/进度”，固定展示当前选中任务、状态、进度、删除/刷新。
   - 默认只展开创建向导的核心字段：币种、方向、风险档位、开始按钮；高级参数折叠。

2. 中部：结果探索
   - 使用 tabs 或 segmented control：
     - “单币 Top10”
     - “组合 Top3”
     - “图表与明细”
   - 选中单币候选或组合后，图表区域占据整行宽度，避免被挤在窄列。
   - 组合 Top3 卡片只展示核心指标：年化、回撤、成员数、最大单币权重、主要杠杆；成员明细点击展开。

3. 底部：组合沙盒与发布
   - 沙盒独立成一个可折叠区域，默认只在用户点击“编辑组合/加入沙盒”后展开。
   - 发布篮子与沙盒合并视觉语义：沙盒重算满意后点击“用作发布篮子”，发布区展示最终权重/杠杆/风险确认。
   - 发布按钮固定在沙盒区域底部，不与候选表格混在一起。

### 回测页交互细节

- 当前选中对象有明确 breadcrumb：`任务 → 单币候选/组合 → 图表`。
- 图表区宽度使用整行，资金曲线、回撤曲线上下排列或左右两列仅在超宽屏使用。
- 交易明细默认折叠，只展示最近/关键 100 条，支持展开。
- 候选表格列减少默认展示：排名、symbol、方向、杠杆、年化、回撤、交易数、操作；参数详情放到展开行。
- 组合成员必须显示：symbol、方向、权重、杠杆、年化、回撤。

### 马丁实盘组合列表目标布局

实盘组合列表改成“监控卡片 + 操作入口”：

- 每个组合卡片顶部展示：名称、状态、市场、方向、成员数。
- 指标区展示：总权重、最大杠杆、最大单币权重、最近 runtime 状态。
- 风险 chip 独立一行展示：Hedge Mode、逐仓、API 权限、交易所预配置状态。
- 主要按钮只保留“查看详情”；启动/暂停/停止不在列表页直接做，避免误触。

### 马丁实盘组合详情目标布局

详情页改成四个清晰区块：

1. 概览
   - 状态、组合 ID、来源任务、市场/方向、成员数、最大杠杆。
   - 风险摘要与当前 runtime snapshot。

2. 交易所预配置
   - 新增本轮功能的核心卡片。
   - 表格展示每个 symbol 的目标/当前 margin mode 与 leverage。
   - 按钮顺序：检查交易所配置 → 勾选风险确认 → 自动预配置交易所。

3. 策略成员
   - 每个成员展示 symbol、方向、权重、杠杆、参数摘要、当前状态。
   - 成员级暂停/恢复/停止放在成员卡片内。

4. 启动与运行控制
   - 组合级确认启动、暂停、停止集中到单独危险操作区。
   - 启动按钮旁展示阻断原因：未预配置、Hedge Mode 不一致、杠杆不一致、API 权限不足等。

### 响应式布局

- 桌面端：使用 `xl:grid-cols-[360px_minmax(0,1fr)]` 的任务/内容双栏，但图表和沙盒可跨全宽。
- 平板端：任务列表置顶，内容纵向。
- 手机端：所有大表格改为卡片列表，隐藏次要参数，只保留展开详情。

### 本轮验收标准补充

1. 回测页初始视图不再同时铺开所有大组件；用户能清楚看到“创建/任务/结果/沙盒发布”的层级。
2. 组合图表区域可用整行宽度观察，资金曲线和回撤曲线不再挤在小块区域。
3. 组合沙盒显示每个成员的杠杆，并且默认不抢占主结果区。
4. 实盘组合详情页新增交易所预配置区块后，启动控制仍然清晰，不与成员列表混杂。
5. 不改变现有回测算法和发布数据结构，只调整信息架构、展示层和新增预配置交互。
