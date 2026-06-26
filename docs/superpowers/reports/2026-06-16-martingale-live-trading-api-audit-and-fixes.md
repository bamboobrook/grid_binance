# 2026-06-16 马丁实盘交易 API 审核与修复交接

## 背景

本次检查的是 GLM 已优化过的马丁实盘交易部分，重点覆盖：

- 实盘下单接口
- 实盘启动和账户设置校验
- 实盘交易成交回填
- 实盘统计、钱包余额和收益统计
- 后续 GLM 继续加入 ATR/ADX、止盈止损参数时需要遵守的接口边界

对照来源只使用 Binance 官方文档：

- USD-M 普通下单：<https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api>
- USD-M 条件单/止盈止损/追踪止损：<https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/New-Algo-Order>
- USD-M symbol 配置：<https://developers.binance.com/docs/derivatives/usds-margined-futures/account/rest-api/Symbol-Config>
- USD-M 账户 V3：<https://developers.binance.com/docs/derivatives/usds-margined-futures/account/rest-api/Account-Information-V3>
- USD-M 余额 V3：<https://developers.binance.com/docs/derivatives/usds-margined-futures/account/rest-api/Futures-Account-Balance-V3>
- USD-M 成交列表：<https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/Account-Trade-List>
- USD-M income：<https://developers.binance.com/docs/derivatives/usds-margined-futures/account/rest-api/Get-Income-History>

## 已修复内容

1. 修复 USD-M 钱包余额 V3 解析错误。

   Binance 官方 `GET /fapi/v3/balance` 返回顶层数组，原实现按 `{ "assets": [...] }` 对象解析，会导致实盘统计中的钱包余额、可用余额同步失败。已改为直接解析 `Vec<AccountV3BalancePayload>`，并更新回归测试。

2. 修复 USD-M symbol 配置读回接口。

   Binance 官方 `GET /fapi/v1/symbolConfig` 返回数组，原实现按单个对象解析；更重要的是实盘启动读回还在用 `/fapi/v2/positionRisk` 当作配置来源。官方变更说明中配置字段应从 `symbolConfig/accountConfig` 获取，仓位接口不应作为 margin/leverage 配置源。已将 `read_usdm_symbol_settings()` 改为委托 `read_usdm_symbol_config()`，并按目标 symbol 精确匹配数组项。

3. 修复 USD-M 成交方向解析。

   Binance `GET /fapi/v1/userTrades` 响应里有显式 `side` 和 `realizedPnl`。原实现主要按现货 `isBuyer` 推导方向；USD-M 响应缺少 `isBuyer` 时会默认成 `SELL`，导致 BUY 成交统计和事件回填方向错误。已改为优先使用官方 `side` 字段，缺失时才回退 `isBuyer`。

4. 新增 USD-M 条件单接口能力。

   Binance 已将 USD-M 止盈、止损、追踪止损类条件单迁移到 `POST /fapi/v1/algoOrder`，普通 `/fapi/v1/order` 不应承载这些条件单类型。已新增：

   - `BinanceAlgoOrderRequest`
   - `BinanceAlgoOrderResponse`
   - `BinanceClient::place_usdm_algo_order()`

   当前实现支持官方条件单核心参数：`algoType=CONDITIONAL`、`type`、`triggerPrice`、`positionSide`、`closePosition`、`workingType`、`priceProtect`、`activatePrice`、`callbackRate`、`clientAlgoId` 等。回归测试覆盖了 TAKE_PROFIT_MARKET 场景，并确认 Hedge Mode 下没有误传 `reduceOnly`。

5. 修复马丁单策略实盘同步时自动改账户设置的问题。

   原 `sync_live_orders()` 中马丁策略会在每轮同步里直接调用：

   - `set_usdm_position_mode(true)`
   - `set_usdm_margin_type(...)`
   - `set_usdm_leverage(...)`

   这在已有持仓或挂单时会被 Binance 拒绝，也可能影响账户内其他策略。已删除同步循环中的自动修改行为，改为仅读取当前 Binance 设置并通过 `sync_martingale_production_start()` 的 futures preflight 校验。账户设置修改仍应由专门的预配置流程完成。

## 当前接口状态

- 普通限价/市价下单：仍走 `POST /fapi/v1/order`。
- Hedge Mode 下单：已有 `positionSide`，且不发送 `reduceOnly`。
- 账户设置修改：已有 `POST /fapi/v1/positionSide/dual`、`POST /fapi/v1/marginType`、`POST /fapi/v1/leverage`，并保留 idempotent 成功处理。
- 账户设置读回：改为 `GET /fapi/v1/symbolConfig`。
- 持仓统计：使用 `GET /fapi/v3/account`。
- 钱包余额统计：使用 `GET /fapi/v3/balance`。
- 成交/手续费：使用 `GET /fapi/v1/userTrades` 的真实 `commission`、`commissionAsset`、`side`、`realizedPnl`。
- funding/commission/realized income：使用 `GET /fapi/v1/income`。
- 条件止盈/止损/追踪止损：已有客户端底座 `POST /fapi/v1/algoOrder`，但尚未接入策略运行逻辑。

## GLM 后续必须注意

1. ATR/ADX 不是阻止项，实盘必须实现同等计算。

   当前代码已有 `MartingaleRuntime::warmup_indicators_from_bars()`、`evaluate_entry_triggers()`、`latest_atr_for_strategy()`，并且 Binance 客户端已有 `fetch_klines()`。后续 GLM 需要把实盘启动和运行中的 K 线 warmup 接进去：回测用了哪些指标，实盘就要从 Binance K 线拉足 warmup 数据，并按同一套 `IndicatorRuntimeContext` 计算。

2. TP/SL 参数不能走普通 `/fapi/v1/order`。

   如果后续加入 `STOP_MARKET`、`TAKE_PROFIT_MARKET`、`STOP`、`TAKE_PROFIT`、`TRAILING_STOP_MARKET`，必须使用本次新增的 `place_usdm_algo_order()`。普通入场/补仓限价单继续走 `place_order()`。

3. 实盘同步循环不要再自动修改账户级或 symbol 级设置。

   修改 Hedge Mode、margin type、leverage 应保留在预配置流程，且预配置流程已有挂单/持仓阻断。实盘下单循环只能读回并校验，不能每轮主动 set。

4. 后续若实现条件单同步，还需要继续补齐：

   - `GET /fapi/v1/openAlgoOrders`
   - `GET /fapi/v1/algoOrder`
   - `DELETE /fapi/v1/algoOrder`
   - `ALGO_UPDATE` user stream 解析

   目前只是把新建条件单的 REST 客户端底座补上，策略状态机尚未消费条件单回报。

## 已执行验证

- `cargo test -p shared-binance --lib -- --nocapture --test-threads=1`
  - 35 项通过。
- `cargo test -p trading-engine --test order_sync -- --nocapture --test-threads=1`
  - 15 项通过。
- `cargo test -p trading-engine --test martingale_runtime -- --nocapture --test-threads=1`
  - 14 项通过。
- `cargo test -p trading-engine --bin trading-engine martingale -- --nocapture --test-threads=1`
  - 3 项通过。

## 验证限制

`cargo test -p trading-engine --test trade_sync -- --nocapture --test-threads=1` 在当前 sandbox 内有 2 项 Telegram HTTP mock 测试因为本地端口绑定权限失败，不是本次代码逻辑失败；其余 10 项通过。需要在允许本地 bind 的环境下重跑该测试文件。

## 本次改动文件

- `crates/shared-binance/src/client.rs`
- `apps/trading-engine/src/main.rs`
- `docs/superpowers/reports/2026-06-16-martingale-live-trading-api-audit-and-fixes.md`

