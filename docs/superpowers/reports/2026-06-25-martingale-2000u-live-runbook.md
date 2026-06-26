# FlyingKid 保守马丁 2000U 实盘启动 Runbook

> 日期：2026-06-25（Asia/Shanghai）
> 账户：`flyingkid2022@outlook.com`
> 组合：`mp_funding_conservative_20260623`
> 状态：启动前准备完成，等待用户最终确认启动 2000U 正式保守组合。

## 1. 当前结论

截至最后一次只读观察：

- Binance USDT-M open orders：`0`
- Binance USDT-M non-zero positions：`0`
- Hedge Mode：`true`
- Multi-Assets Mode：`false`
- API 服务：`grid-binance-api-server-1` healthy
- Trading engine：未运行
- 正式组合状态：`pending_confirmation`
- 旧 live snapshot：`martingale_live_portfolios=0`，`martingale_live_strategy_instances=0`
- 正式策略实例：`strategies.source_template_id='mp_funding_conservative_20260623'` 计数为 `0`
- 预配置状态：`exchange_preconfigure.status='ready'`

注意：`exchange_preconfigure` 有 10 分钟 TTL。若用户确认时 TTL 已过，必须先刷新 preconfigure，再执行正式启动。

## 2. 已完成的实盘风险修复

### 2.1 Binance REST 请求重试

文件：`crates/shared-binance/src/client.rs`

问题：`/fapi/v1/time` 等公共请求和签名请求偶发 `error sending request` 时会直接失败，导致 exchange-preflight / exchange-preconfigure 不稳定。

修复：

- 默认 HTTP timeout 从 5s 提升到 10s。
- `public_get`、`api_key_request`、`signed_request` 统一走 `send_with_retries`。
- 复用已有 `is_retryable_error` / `retry_delay` / `MAX_RETRIES`，覆盖 timeout、网络错误、429、5xx、Binance 临时错误码。

验证：

- `cargo test -p shared-binance -p trading-engine` 通过。
- API 镜像已重建并重启。

### 2.2 Multi-Assets Mode / 逐仓杠杆预配置

文件：`apps/api-server/src/services/martingale_exchange_preconfigure_service.rs`

问题：正式组合使用 isolated margin，Binance USDT-M Multi-Assets Mode 必须关闭，否则逐仓设置不兼容。

修复：

- preconfigure 请求增加 `confirm_account_level_multi_assets_mode_change`。
- 目标交易所设置增加 `requires_single_asset_mode`。
- readback response 增加 `multi_assets_mode` 字段。
- 若组合需要 isolated margin，preconfigure 会确认并设置 Multi-Assets Mode 为 `false`。
- 同 symbol 多策略 leverage 取最大值，而不是视为冲突。

验证：

- 正式组合 preconfigure readback 为 ready：
  - Hedge Mode target/current：`true/true`
  - Multi-Assets target/current：`false/false`
  - 目标 symbol 均为 isolated 且 leverage 匹配
  - open_order_count：`0`
  - nonzero_position_count：`0`

### 2.3 马丁组合预算不再压缩首单

文件：`apps/trading-engine/src/main.rs`

问题：旧逻辑会在组合权重预算小于完整马丁计划资金时，把整条 sizing 序列等比缩小。正式组合中 14 个策略里有 11 个在旧逻辑下首单会低于 Binance 最小名义额或被 stepSize 压成 0。

修复：

- `portfolio_weight_pct` 不再改写 `first_order_quote` / multiplier / max_legs。
- 权重预算只作为 `max_strategy_budget_quote` 风控上限。
- 若权重预算小于首单保证金，上限至少提升到首单保证金，确保首单可以按回测参数执行。
- 后续补仓若会超过预算，由 `enforce_budget_for_next_leg` 阻止继续开下一腿。

验证：

- 新增测试 `martingale_weight_cap_keeps_first_order_when_cap_is_below_first_order`。
- `cargo test -p trading-engine` 全部通过，包含普通网格、马丁 runtime、order sync、trade sync、统计测试。
- 交易引擎镜像 `grid-binance-trading-engine` 已重建。

## 3. 50U 烟测结果

烟测组合：`mp_live_smoke_50_v2_20260624`

结果：

- 状态：`stopped`
- `risk_summary.live_executor_state='smoke_passed_stopped'`
- `risk_summary.live_smoke_result.status='passed'`
- 最终 Binance：`openOrderCount=0`，`nonzeroPositionCount=0`

覆盖项：

- exchange preconfigure
- 首单下单并满足最小名义额
- 成交同步
- TP market close
- 手续费 / realized PnL 记录
- stop 后取消工作订单
- 最终账户恢复空仓空挂单

## 4. 启动前必须检查

启动前逐项确认：

1. Binance open orders = `0`
2. Binance non-zero positions = `0`
3. Hedge Mode = `true`
4. Multi-Assets Mode = `false`
5. `martingale_portfolios.status='pending_confirmation'`
6. `risk_summary.exchange_preconfigure.status='ready'`
7. `exchange_preconfigure.checked_at` 距 now 不超过 600 秒
8. `risk_summary` 中没有旧 `live_start`
9. `martingale_live_portfolios` 对该 portfolio 计数为 `0`
10. `martingale_live_strategy_instances` 对该 portfolio 计数为 `0`
11. `strategies.source_template_id='mp_funding_conservative_20260623'` 计数为 `0`
12. Trading engine 容器未运行

只读观察命令：

```bash
cd /home/bumblebee/Project/grid_binance
node /tmp/conservative_2000_observe.js
```

若 preconfigure 过期，刷新命令：

```bash
API_BASE_URL=http://172.18.0.3:8080 node /tmp/conservative_2000_api_prepare.js
```

若 SSH wrapper 超时但数据库已经写入新的 `checked_at`，先检查并终止悬挂的 Node 客户端进程。不要重复发 preconfigure。

## 5. 正式启动步骤

只有用户明确回复以下句子后才能继续：

```text
确认启动 2000U 正式保守组合
```

确认后执行：

1. 再运行一次 `node /tmp/conservative_2000_observe.js`。
2. 若 `exchange_preconfigure.age_secs > 600`，先刷新 preconfigure。
3. 执行 confirm-start：

```bash
cd /home/bumblebee/Project/grid_binance
API_BASE_URL=http://172.18.0.3:8080 node /tmp/conservative_2000_confirm_start.js
```

4. 启动 trading-engine：

```bash
cd /home/bumblebee/Project/grid_binance
docker compose --env-file .env -f deploy/docker/docker-compose.yml up -d trading-engine
```

5. 立即观察：

```bash
cd /home/bumblebee/Project/grid_binance
node /tmp/conservative_2000_observe.js
```

## 6. 启动后观察重点

必须持续观察：

- portfolio 是否从 `pending_confirmation` 转为 `running`
- `risk_summary.live_start.executor_state` 是否为 `pending_pickup`
- trading-engine 是否 pickup 并写入 `live_executor_state='started'`
- `strategies` 是否生成 14 个正式子策略
- 订单是否只来自当前 portfolio / strategy，不混入旧快照
- Binance open orders 与 DB `strategy_orders` 是否匹配
- 是否出现 orphan orders
- 是否出现重复开仓
- 任何已成交订单是否写入：
  - `strategy_fills`
  - `exchange_account_trade_history`
  - `strategy_profit_snapshots`
- 手续费、realized PnL、unrealized PnL、funding 字段是否有更新
- TP/SL market close 行为是否和回测配置一致

观察命令：

```bash
node /tmp/conservative_2000_observe.js
docker logs --tail 200 grid-binance-trading-engine-1
docker exec grid-binance-postgres-1 psql -U postgres -d grid_binance -qAt -c "select status,count(*) from strategies where source_template_id='mp_funding_conservative_20260623' group by status order by status;"
```

## 7. 异常处理

如果发现错误挂单、未知挂单或与策略不符的仓位：

1. 先停止 trading-engine：

```bash
docker compose --env-file .env -f deploy/docker/docker-compose.yml stop trading-engine
```

2. 只读确认 Binance open orders / positions。
3. 判断订单 clientOrderId 是否属于当前 `mg-*` 策略。
4. 若确认为错误挂单或错误仓位，按用户授权取消挂单并清仓。
5. 清理本次运行数据前先备份相关 DB 行。
6. 重新回到启动前检查流程。

## 8. 待补充

正式 2000U 启动后补充：

- confirm-start 返回结果
- trading-engine 启动时间
- 生成策略数
- 初始订单明细
- Binance open orders / positions 截图或 JSON
- 前 15/30/60 分钟运行观察
- 成交、手续费、资金费率、PnL 统计验证
- 最终用户侧操作步骤总结
