# Martingale 实盘 ATR/ADX 指标 Parity 补全计划（备用，待搜索达标后启动）

> **状态**：备用计划。前置条件：conservative ATR 搜索（重建 backtest-worker 后）验证达标。若 conservative 用 ATR 仍无法突破 DD 瓶颈，本计划暂缓。
> **来源**：2026-06-13 parity 验证（general-purpose agent）+ Plan agent 设计 + Phase 3 代码验证。
> **估算**：~6.5 天（v1，不含 Drawdown 类 SL）。
> **关联**：交接文档 `docs/superpowers/reports/2026-06-11-martingale-next-optimization-handoff.md`；plan `docs/superpowers/plans/2026-06-11-martingale-next-optimization-deepseek-plan.md` Task2。

## 一、问题（Parity 验证结论）

回测侧 ATR/ADX 已正确实现（`backtest-engine`：`kline_engine.rs` 用共享 `IndicatorRuntimeContext` 计算 ATR/ADX，per-cycle 快照冻结）。但**实盘侧 `trading-engine` 未接线**——DeepSeek 加了 4 个 indicator 方法但全是死代码，主循环从未调用：

| Gap | 文件:行 | 描述 | 严重度 |
|-----|---------|------|--------|
| 1 | `martingale_runtime.rs:171` `warmup_indicators_from_bars` | 定义但零调用，context 永远空 | 致命 |
| 2 | `martingale_runtime.rs:177` `evaluate_entry_triggers` | 定义但零调用，ADX/IndicatorExpression 入场过滤永不执行 | 致命 |
| 3 | `martingale_runtime.rs:208` `has_indicator_warmup_for` | 定义但零调用，无 warmup gate | 致命 |
| 4 | `main.rs:765-828` `martingale_runtime_config_from_strategy` | 单策略路径硬编码 FixedPercent/Percent{100}/stop_loss:None/indicators:[]/entry_triggers:[] | 致命 |
| 5 | `martingale_runtime.rs:236-262` `start_cycle` | 不调 evaluate_entry_triggers，入场过滤被绕过 | 致命 |
| 6 | `martingale_runtime.rs` 整体 | runtime 无 per-strategy TP/SL 评估（ATR TP/SL/Indicator SL 无执行路径） | 致命 |
| 7 | `main.rs` 全文 | 无 completed-candle kline fetch/cache | 致命 |

**后果**：搜出的 ATR 策略上线后，`latest_atr` 恒 None（ATR spacing 退化），ADX/IndicatorExpression 入场不执行（任何条件都入场），ATR TP/SL 不评估（只靠 portfolio 总 PnL bps 退出）。

**编译**：`cargo check --workspace --lib` 通过（能编译 ≠ 逻辑正确）。

## 二、范围（v1）

✅ **包含**：warmup + completed-candle 流、evaluate_entry + warmup gate、持续 ATR/Indicator/Percent/Trailing TP/SL 评估、指标快照审计、parity 测试
❌ **不含（另开 task）**：Drawdown 类 SL（StrategyDrawdownPct/SymbolDrawdown/GlobalDrawdown，需 portfolio 聚合 pnl，与现有 `overall_*_bps` 退出重叠，易双重触发）
ℹ️ **单策略路径降级**：`StrategyRevision`（`shared-domain/src/strategy.rs:150-166`）无完整 martingale 配置字段（只有 grid_spacing_bps 等），单策略路径无法拿 ATR 配置。**ATR 闭环只在 portfolio 路径启用**（`martingale_runtime_config_from_portfolio` main.rs:831 已真实反序列化 `MartingalePortfolioConfig`）。

## 三、关键架构决策（已验证）

### 1. TP/SL 复用 = 方案 A（零 backtest 重构）
`apps/backtest-engine/src/martingale/exit_rules.rs` 纯函数全 `pub`：
- `take_profit_price(average_entry, direction, model, latest_atr)` :32（支持 Percent/Amount/Atr/Trailing/Mixed）
- `weighted_average_entry(legs)` :17
- `evaluate_exit_priority(global, symbol, strategy, take_profit) -> ExitDecision` :97
- `ExitDecision` enum :8

trading-engine 已依赖 backtest-engine 且已 import `IndicatorRuntimeContext`/`latest_atr_for_strategy`/`KlineBar`（`martingale_runtime.rs:1-5`）。**直接 import 纯函数，新增编排层**，不动 backtest-engine 内部。

### 2. 进程级 indicator feed 持久化（最大风险点）
`reconcile_running_martingale_portfolios`（main.rs:252）是 **one-shot 启动**（`live_executor_started` 后跳过 :257-264），且 runtime 每 strategy 重建（:328）→ `indicator_context` 每 tick 丢失。必须在 main.rs 进程级持久化（仿 `LIVE_TICK_QUEUE` main.rs:47 的 OnceLock 模式）：

```rust
struct IndicatorFeedState {
    context: IndicatorRuntimeContext,
    last_pushed_open_time_ms: HashMap<String, i64>,  // per symbol 去重
    warmed_up: HashMap<String, bool>,
}
static INDICATOR_FEEDS: OnceLock<Mutex<HashMap<String /*portfolio_id*/, IndicatorFeedState>>> = OnceLock::new();
```

### 3. 持续 TP/SL 评估需新增路径
当前 one-shot 启动后只靠 `overall_*_bps` PnL 退出（main.rs:1061-1104, :1527-1538）。新增 `evaluate_running_portfolios_exits`：每 tick 对已 started portfolio 用 completed-candle 评估 ATR TP/SL，触发则平仓。与 one-shot 启动分开。

### 4. warmup 数据来源
`BinanceClient::fetch_klines(market, symbol, interval, start_ms, end_ms, limit) -> Vec<KlineRecord{time:String, open, high, low, close, volume}>`（`crates/shared-binance/src/client.rs:1595`，USD-M 用 market="usdm"）。`KlineRecord.time` 是 epoch ms 字符串（client.rs:1667），转 `KlineBar.open_time_ms` 直接 `time.parse::<i64>()`。

### 5. warmup 需求
ATR(period=N) 需 N 根（`indicator_runtime.rs:74` warmup_ranges.len()==period）；ADX 需 N+1 根（`indicators.rs`）。warmup_bars = `max(strategy 各 indicator period) + 2`（余量）。

## 四、修改/新增点

### `apps/trading-engine/src/martingale_runtime.rs`
- 新增 `evaluate_strategy_exit(&self, strategy_id, bar) -> Result<ExitDecision>`：从 `self.orders` 过滤该 strategy Filled 订单 → `MartingaleLegState`（backtest state.rs:34，已 pub）→ `weighted_average_entry` → 按 stop_loss/take_profit model 分支调 exit_rules 纯函数（ATR via `indicator_latest_atr`:201）
- 新增 `set_indicator_context(&mut self, ctx)`：回灌持久化状态（否则 evaluate 全基于空 context）
- 抽 `pub fn atr_stop_price(average_entry, direction, atr, multiplier) -> f64`（kline_engine.rs:1095-1098 公式，4 行）放 exit_rules.rs 共享
- 完善 `evaluate_entry_triggers`（:177）补 PriceRange/TimeWindow/Cooldown/Capacity（对齐 kline_engine.rs:1151-1186）

### `apps/trading-engine/src/main.rs`
- `INDICATOR_FEEDS: OnceLock<Mutex<HashMap<PortfolioId, IndicatorFeedState>>>`（进程级持久化）
- **warmup**：reconcile 内首次 fetch 历史 1m（warmup_bars = max_indicator_period + 2），转 KlineBar，`warmup_indicators_from_bars`
- **completed-candle 增量**：`last_completed_open = (now_ms/60000-1)*60000`，>last_pushed 则 fetch 单根/补缺（按 open_time 升序 push，不断 true_range 链），过滤未收盘根（`open_time_ms + 60000 <= now_ms`）
- **entry + warmup gate**：start_cycle 前（:331）查持久化 warmed_up + `evaluate_entry_triggers`，未满足则 blocked（cycle_results 记录）
- **持续评估路径** `evaluate_running_portfolios_exits`：已 started portfolio 每 tick 评估 exit，触发则平仓
- **snapshot 审计**：`cycle_results` JSON 加 `indicator_snapshot`（candle_open_time_ms/atr_value/adx_value/entry_expression+result/exit_tp_price/exit_sl_price/exit_decision），落库 portfolio.risk_summary

### `apps/backtest-engine/src/martingale/exit_rules.rs`
- 加 `pub fn atr_stop_price(average_entry, direction, atr, multiplier) -> f64`（共享 SL 价格公式，供两侧复用）

### 新增 `apps/trading-engine/tests/indicator_parity.rs`
合成 K 线序列，回测 `kline_engine` vs 实盘 `MartingaleRuntime` 逐根对比：ATR 值、entry bool、TP price、SL price、ExitDecision 相等。覆盖矩阵：ATR TP long/short、ATR SL、Indicator SL、warmup 边界（period-1/period/period+1）、incremental(push_bar) vs batch(ensure_atr_cached) ATR 一致性。

## 五、Parity 保证
- 纯公式 + 指标单点定义（exit_rules + IndicatorRuntimeContext 两侧 import 同一实例代码）
- parity 测试进 CI（backtest 改公式会同步失败）
- 实盘 snapshot 可事后用同一 K 线喂回测 engine 对账

## 六、风险
1. **runtime 每次重建** → INDICATOR_FEEDS 必须真持久化（日志验证 `bars_by_symbol.len()` 跨 tick 单调增长）
2. **incremental ATR vs batch ATR 数值漂移** → parity 测试必须覆盖（两条路径：push_bar incremental vs ensure_atr_cached batch）
3. **completed-candle 时延**（5s tick + Binance klines lag）→ fetch 返回空跳过本轮，下轮补；漏根补齐按 open_time 升序，否则 true_range 断链
4. **Drawdown 类 SL 不在 v1**（文档明确，另开 task）

## 七、验证
```bash
cargo check --workspace --lib
cargo test -p trading-engine --test indicator_parity -- --nocapture
cargo test -p trading-engine --test martingale_runtime -- --nocapture
cargo test -p trading-engine --test order_sync -- --nocapture
cargo test -p backtest-engine martingale::kline_engine -- --nocapture
```
+ 实盘 dry-run：合成 K 线序列验证 TP/SL 触发 + warmup 行为。

## 八、工作量
| 块 | 估算 |
|----|------|
| evaluate_strategy_exit 编排 + legs 重构 + atr_stop_price 抽取 | 1.5 天 |
| 持久化 feed + warmup + completed-candle（含缺根补齐/去重） | 2 天 |
| entry gate + 状态回灌 + 补齐 entry trigger 变体 | 1 天 |
| snapshot 审计 + 落库 | 0.5 天 |
| parity 测试（合成数据 + 双侧跑 + 断言矩阵） | 1.5 天 |
| **合计** | **~6.5 天** |
