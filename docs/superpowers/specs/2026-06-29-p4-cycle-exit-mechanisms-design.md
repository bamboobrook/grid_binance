# 2026-06-29 P4 设计:cycle 级退出机制(max_cycle_age_hours + regime_break_stop)

> 承接 ChatGPT 计划 `docs/superpowers/plans/2026-06-28-chatgpt-verdict-after-glm-optimization.md` 第 5 节 P4 + 行动清单第 4 条。P2 报告 `docs/superpowers/reports/2026-06-29-p2-2025-segment-search-findings.md` 已证明:2025 short 收益源存在但跨不过 2023 H1 转折期(ema 滞后 → short 被套),需 cycle 级退出机制才能突破。

## 1. 目标与非目标

**目标**:实现两个 **live-parity**(backtest + trading-engine 双端一致、可实盘复现)的 cycle 级退出机制,打破"2025 short 收益 vs 全周期存活"矛盾,使 short/long_and_short 候选能在 2023 牛市转折期及时止损、在 2025 熊市保留收益。

**非目标(YAGNI)**:
- 不扩展 indicator expression 语言支持 AND/OR(路径 1,改动面大、live parity 复杂)。
- 不改 `exit_rules.rs::evaluate_exit_priority` 的优先级结构(避免连锁)。
- 不实现 ChatGPT 提到的其他 research-only 机制(`portfolio_equity_stop_pct`/`portfolio_stop_cooldown`/`max_portfolio_active_cycles` 的 live 落地不在本 spec 范围)。
- max_cycle_age 放 **strategy 级 risk_limits**(每策略各自 cycle age),不做 portfolio 级。

## 2. 机制 A:`max_cycle_age_hours`(超时强平)

### 2.1 配置 schema
`crates/shared-domain/src/martingale.rs` 的 `MartingaleRiskLimits`(`:146` 附近)新增字段:
```rust
#[serde(default)]
pub max_cycle_age_hours: Option<f64>,
```
仿 `new_cycle_drawdown_pause_pct` 模式(已 parity-structured)。放 strategy 级(`MartingaleStrategyConfig.risk_limits`)。

### 2.2 语义
cycle 从首 leg(leg_index=0)成交时刻开始计时;若 `now_ms - cycle_start_ms >= max_cycle_age_hours * 3_600_000` 且 cycle 仍 active(未 TP),则 market 平掉整个 cycle(reduceOnly)。`None` = 不启用。

### 2.3 backtest 实现(`apps/backtest-engine/src/martingale/`)
- **数据**:`StrategyRuntime`(`kline_engine.rs:753-765`)加 `cycle_start_ms: Option<i64>`。
- **赋值**:entry 块 `add_leg(state, 0, bar.open, ...)`(`kline_engine.rs:317-324`)后设 `state.cycle_start_ms = Some(bar.open_time_ms)`。
- **清空**:`reset_cycle()`(`kline_engine.rs:818-826`)加 `cycle_start_ms = None`。
- **检查点**:`triggered_stop()`(`kline_engine.rs:1352`)末尾加 age 分支:若 `risk_limits.max_cycle_age_hours` 为 `Some(h)` 且 `bar.open_time_ms - cycle_start_ms >= h*3.6e6`,置 `strategy_stop = true`(price 用 `bar.close`)。
- **退出通道**:复用 `ExitDecision::StrategyStop`(`kline_engine.rs:480-512`),仅 event_type 用新字符串 `"cycle_age_stop"`(便于回测统计区分,不改退出语义)。

### 2.4 trading-engine 实现(`apps/trading-engine/src/`)
- **内存态**:`CycleState`(`martingale_runtime.rs:122-127`)加 `start_ms: Option<i64>`;`start_cycle`(`martingale_runtime.rs:312-338`)从 `context.now_ms`(`:34`)赋值。
- **持久化起点(关键)**:runtime 每次 reconcile 从 DB 重建,内存 `start_ms` 会丢。必须从持久层推导:leg-0 fill 的 `created_at`(`main.rs:841` 已有 `event.created_at.timestamp_millis()`)。在 `reconcile_martingale_executor_strategies`(`main.rs:665-785`)重建 cycle 时,取该 cycle 最早 leg-0 fill 的 created_at 作 `start_ms`。
- **检查点**:`martingale_exit_signal()`(`main.rs:1881-1931`)加 age 分支:若 `now_ms - start_ms >= h*3.6e6` → 调 `request_martingale_close`(`main.rs:1972-2025`),event_type `"cycle_age_stop"`。

## 3. 机制 B:`regime_break_stop`(regime 反转 + 浮亏 → 平仓)

### 3.1 配置 schema
`crates/shared-domain/src/martingale.rs` 的 `MartingaleStopLossModel`(`:99-106`)新增枚举变体:
```rust
RegimeBreakStop {
    ema_period: u32,         // EMA 周期, 如 50/100
    drawdown_pct_bps: u32,   // 触发浮亏阈值, bps (500=5%, 2000=20%); 触发条件 drawdown_pct >= bps/100
},
```
serde `rename_all = "snake_case"` → `"regime_break_stop": { "ema_period": 50, "drawdown_pct_bps": 1000 }`。

### 3.2 语义
- **long cycle**(持仓中):若本币种 `close < ema(ema_period)` **且** `drawdown_pct >= drawdown_pct_bps/100` → market 平整个 cycle。
- **short cycle**(持仓中):若 `close > ema(ema_period)` **且** 浮亏达阈值 → 平整个 cycle。
- drawdown_pct 复用现有定义:`(-net_pnl).max(0)/invested*100`(`kline_engine.rs:1369-1384` 同 StrategyDrawdownPct)。
- 两个条件必须**同时**满足(AND),这是区别于"单条件 Indicator SL"的关键(P2 探针证明单条件 `close>ema` 在牛市频繁误触发)。

### 3.3 backtest 实现
`triggered_stop()`(`kline_engine.rs:1352`)加 match arm `RegimeBreakStop { ema_period, drawdown_pct_bps }`:
1. 取 `ema = indicator_context.latest_ema(symbol, *ema_period)`(复用 EMA cache;数据不足返回 None → 不触发)。
2. 算 `drawdown_pct`(复用 `strategy_net_pnl` `:1628` / `capital_used_quote` `:807-809`,同 StrategyDrawdownPct 逻辑)。
3. long 方向:`close < ema && drawdown_pct >= *drawdown_pct_bps as f64/100.0` → `strategy_stop=true`;short 反向。
4. price 用 `bar.close`。

### 3.4 trading-engine 实现
`martingale_exit_signal()`(`main.rs:1881-1931`)加 `RegimeBreakStop` 分支:
1. 签名需把 `&IndicatorRuntimeContext`(或其快照)传入 —— 当前签名只有 price/position。`apply_martingale_market_ticks`(`main.rs:1805-1870` `:1835` 调用点)已有 `persisted_ctx`(`main.rs:588,592` 持久化的 indicator context)可用,传入即可。
2. EMA 用**最近完成的 1m bar** 的值(parity:backtest 也是 bar 级 EMA;tick 级用最新完成 1m,不超前)。
3. drawdown 复用 `martingale_strategy_drawdown_pct()`(`martingale_exit.rs:17-55`)。
4. 触发 → `request_martingale_close`,event_type `"regime_break_stop"`。

## 4. 退出优先级
- 两个机制都复用 `ExitDecision::StrategyStop` 通道(`kline_engine.rs:480-512`),与现有 `StrategyDrawdownPct` **同优先级**(GlobalStop > SymbolStop > **StrategyStop** > TakeProfit)。
- 触发任一 strategy-level stop(StrategyDrawdownPct / CycleAge / RegimeBreak)即平整个 cycle。它们之间是 OR 关系(最先满足的触发),无需新优先级层。

## 5. live parity 三处对齐
| 端 | 文件 | 改动 |
|---|---|---|
| backtest | `kline_engine.rs` | `triggered_stop` 加 age + RegimeBreakStop 分支;`StrategyRuntime.cycle_start_ms`;entry/reset_cycle 维护 |
| trading-engine | `martingale_exit.rs` + `main.rs` + `martingale_runtime.rs` | `martingale_exit_signal` 加 age + RegimeBreakStop 分支(传 indicator_ctx);`CycleState.start_ms` + reconcile 从 leg-0 fill 推导 |
| parity gate | `budget_replay.rs:533 live_parity_check` | SL 白名单 match 加 `RegimeBreakStop { .. }` arm(放行);**同时把 `live_parity_check` 接入 search/publish 流程**(P7,当前无 rust 调用者) |

注:`max_cycle_age_hours` 是 risk_limits 字段,`live_parity_check` 不查 risk_limits(`:537-557` 只查 TP/SL),故 gate 免改 —— 但 trading-engine 必须实现,否则 backtest illusion。

## 6. 测试策略(TDD,先写失败测试再实现)
**backtest 单元**(`apps/backtest-engine/src/martingale/kline_engine.rs` 测试块):
- `max_cycle_age`:构造 cycle,推进 bars 超过 N 小时 → 断言平仓 + event `cycle_age_stop`;未超时不断言。
- `regime_break_long`:close 跌破 ema 且浮亏 → 平仓;close>ema 或不亏 → 不触发。
- `regime_break_short`:close 涨破 ema 且浮亏 → 平仓。
- EMA 数据不足(warmup 期)→ 不触发(不平仓)。

**trading-engine 单元**(`apps/trading-engine/tests/martingale_runtime.rs`):
- age 从 leg-0 fill `created_at` 正确推导(reconcile 后 start_ms 一致)。
- regime_break 用 indicator_ctx 的 EMA 触发平仓。

**parity 集成**:
- 同一 config(含 RegimeBreakStop + max_cycle_age)+ 同一段 1m 历史,backtest 与 trading-engine replay 结果(total_return / DD / 平仓事件数)在容差内一致。
- `live_parity_check`:RegimeBreakStop 放行;Trailing/Indicator/其他 TP/SL 仍 reject。

## 7. 实现顺序(worktree 隔离,TDD 每步)
1. **config schema**:`martingale.rs` 加 `RegimeBreakStop` 变体 + `max_cycle_age_hours` 字段(+ serde 测试:JSON 往返)。
2. **backtest**:先写 §6 backtest 测试(失败)→ 实现 `triggered_stop` 两分支 + `cycle_start_ms` 生命周期 → 测试通过。
3. **trading-engine**:先写 §6 trading-engine 测试(失败)→ 实现 `martingale_exit_signal` 两分支 + indicator_ctx 传入 + reconcile start_ms 推导 → 测试通过。
4. **parity gate**:`live_parity_check` 加 RegimeBreakStop arm + 接入 search/publish(P7)+ 测试。
5. **重跑搜索 + 验证**(见 §8)。

## 8. 搜索重跑与验收
**搜索参数(扩展 search_small_capital_martingale)**:
- `regime_break`:ema_period ∈ {50, 100},drawdown_pct_bps ∈ {500, 1000, 1500, 2000}(5%-20%)。
- `max_cycle_age_hours` ∈ {24, 48, 72, 120, 168}(None 作 baseline)。
- **优先作用于 short 候选**(BCH/DOT/APT/ETC/NEAR/COMP)+ long_and_short —— P2 证明这些在 2023 H1 被套;long_only trend 已健康(可不加 regime_break)。

**验收(对齐 ChatGPT 计划第 6 节)**:重搜后对每个候选给 full + segment(H1-2023/H2-2023/2024/2025/2026)return/DD、budget matrix(1000-5000)、live parity(RegimeBreakStop + max_cycle_age 双端一致)、overfit flags(H1 贡献、2024-2026 合计)。**突破标志**:某 short/long_and_short 候选加 regime_break + age 后,2023 H1 段不再击穿、full ann 向 balanced 90/DD 20 靠近,且 segment gate 通过。

## 9. 风险与回退
- **实盘 cycle 起点持久化**是最大风险(runtime 重建语义)。若 reconcile 从 leg-0 fill 推导 start_ms 在边界情况(部分成交、手动干预)不可靠,降级方案:max_cycle_age 仅用于 backtest 搜索筛选,live 不启用(但 regime_break 不受影响,它不需 start_ms)。
- regime_break 的 EMA warmup 期(backtest 数据不足)默认不触发,避免误平。
- 若重搜后仍无达标组合,按 ChatGPT 第 7 节输出失败证明(Pareto 前沿 + 是否改善)。
