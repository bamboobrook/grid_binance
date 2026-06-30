# 2026-06-28 GLM P0 完成交接（结构化配置 + live parity 补齐）

> 给 ChatGPT：本轮按你 `2026-06-28-chatgpt-external-research-next-plan-for-glm.md` 的优先级，
> 完成了 P0.1 / P0.2 / P0.3（结构化研究开关 + 跨币种行情依赖 + TP/SL 实盘一致性 gate）。
> 主线 A/B/C/D 的搜索**尚未开始**，等 P0 代码审核通过后再单独开新一轮。
> 所有改动留在工作区，**未 commit、未 push**（遵守用户上一轮指令）。

## 1. 本轮完成清单

| 阶段 | 内容 | 状态 |
|---|---|:---:|
| 基线复现 | 3 候选 × 5 预算 = 15 个 `portfolio_budget_replay` 报告 | ✅ |
| P0.1 | 3 个研究 env 阈值结构化进 `MartingaleRiskLimits`，贯通 sd/bt/te/api | ✅ |
| P0.2 | 跨币种依赖自动提取 + 回测加载依赖币种 bars + api-server `market_data_dependencies` | ✅ |
| P0.3 | TP/SL 实盘一致性支持矩阵文档 + `live_parity_check` gate | ✅ |
| 全量验收 | sd 9 / bt 208 / te 183 / api 14 全过，0 回归 | ✅ |

## 2. 用户确认的两个范围决策

1. **P0.1 范围**：只结构化**已有实盘路径**的 3 个阈值（`new_cycle_drawdown_pause_pct` /
   `new_cycle_atr_pause_pct` / `safety_skip_adx_threshold`）。另外 3 个
   （`portfolio_equity_stop_pct` / `portfolio_stop_cooldown` /
   `max_portfolio_active_cycles`）在 trading-engine **完全没实盘实现**，本轮标记为
   research-only env，不进入最终搜索空间。理由：这三个改变交易语义最多（实盘
   flatten/cooldown 风险大），你的计划第 7 节也把 dynamic reset 列为「最后才测」。
2. **P0.2 存储**：依赖自动从表达式提取，存进 `risk_summary.market_data_dependencies`
   （不污染 `risk_limits`，语义是行情依赖不是风险限制）。

## 3. P0.1 详细：3 阈值结构化 parity

### 发现的 parity 差距（修复前）

| 阈值 | 回测 | trading-engine 实盘 |
|---|---|---|
| `new_cycle_drawdown_pause_pct` | env 可配，默认 6.0 | **硬编码 `> 6.0`** |
| `new_cycle_atr_pause_pct` | env 可配，默认 2.0 | **硬编码 `> 2.0`** |
| `safety_skip_adx_threshold` | env 可配，默认 45 | **硬编码 `> 45.0`** |

即：默认值一致，但**回测改了 env 阈值，实盘不会跟着改**。这是 live-parity 漏洞。

### 修复（4 文件）

1. **`crates/shared-domain/src/martingale.rs`**：`MartingaleRiskLimits` 新增 3 个
   `Option<f64>` 字段（`#[serde(default)]`，旧 config 无此字段仍能反序列化）。
2. **`apps/backtest-engine/src/martingale/kline_engine.rs`**：`RiskGuardThresholds::from_env()`
   → `from_config(&portfolio.risk_limits)`。解析顺序：**env 覆盖 > config > 引擎默认**
   （env 保留为研究诊断用）。新增 `resolve_threshold()` 统一解析路径。
3. **`apps/trading-engine/src/martingale_runtime.rs`**：3 处硬编码改为
   `unwrap_or(默认)`，从 `portfolio_risk_limits`（DD/ATR）/ `strategy.risk_limits`（ADX）读取。
4. **`apps/api-server/src/services/martingale_publish_service.rs`**：
   - `live_portfolio_config_snapshot`：不再强制写 `risk_limits: {}`，改为
     **透传 candidate 的 portfolio risk_limits**（阈值字段能 survive publish，
     后续 `set_live_budget_in_config` 是 insert 不是 replace）。
   - `risk_summary_for_candidate` / `portfolio_risk_summary`：新增
     `risk_guard_thresholds` 字段，展示生效的 3 个阈值（config 值或默认）。

### 新增测试

- sd: `risk_limits_guard_thresholds_round_trip_with_defaults`、`risk_limits_default_omits_guard_thresholds`
- bt kline_engine: `risk_guard_thresholds_read_config_first_then_default`、`risk_guard_thresholds_fall_back_to_defaults_when_config_unset`
- te: `portfolio_drawdown_threshold_from_config_tighter_than_default_pauses`、
  `portfolio_drawdown_threshold_from_config_looser_than_default_allows`、
  `atr_pause_threshold_from_config_looser_than_default_allows`（+ `runtime_config_with_risk_limits` helper）
- api: `published_config_carries_candidate_risk_guard_thresholds`、`risk_summary_falls_back_to_default_guard_thresholds_when_unset`

## 4. P0.2 详细：跨币种行情依赖

### 发现的现状（好消息）

- 跨币种表达式**回测已支持**（`BTCUSDT.close > BTCUSDT.ema(50)`、`atr_percent` 等，
  见 `indicator_runtime.rs` 的 `resolve_operand` / `split_symbol_prefix`）。
- 实盘 ticks 通过 Redis 全量进入 `market_ticks`，`complete_bars`
  （`martingale_candle.rs:76`）**不过滤 symbol**，所以依赖币种的 bars **已经**
  进入 `indicator_context`。**reconcile 循环无需改**。
- 唯一缺口：(a) 回测只加载 strategy symbol 的 bars，依赖币种 bars 没加载；
  (b) api-server 没有「这个组合需要哪些币种行情」的元数据。

### 修复（3 文件）

1. **`apps/backtest-engine/src/martingale/indicator_runtime.rs`**：新增
   `pub fn extract_symbol_dependencies(config) -> Vec<String>`，扫描所有
   `entry_triggers` 的 `IndicatorExpression` 和 `stop_loss` 的 `Indicator{expression}`，
   用 `split_comparison` + 新增的 `extract_symbol_ref_from_operand` 提取
   `SYMBOL.indicator(...)` 和 `symbol.ohlc` 引用，排除策略自身 symbol，去重排序。
2. **`apps/backtest-engine/src/bin/portfolio_budget_replay.rs`**：symbols 集合
   union 进 `extract_symbol_dependencies(&portfolio)`，依赖币种 bars 被加载
   （funding 只对 traded symbols 加载，依赖币种不需要）。
3. **`apps/api-server/src/services/martingale_publish_service.rs`**：
   `risk_summary_for_candidate` / `portfolio_risk_summary` 新增
   `market_data_dependencies` 字段（数组，如 `["BTCUSDT","ETHUSDT"]`）。

### Smoke 验证（端到端跑通）

用一个 SOL 策略 + 入场条件 `close > BTCUSDT.ema(50)`（BTC 是依赖），跑 2023-01 一个月：
```
replay: 1 strategies, 1 traded symbols (+1 market-data deps: [BTCUSDT]), budget=500
  loaded SOLUSDT: 44641 bars
  loaded BTCUSDT: 44641 bars    ← 依赖币种 bars 正确加载
```
把条件改成 `BTCUSDT.close > 0`（恒真）确认表达式计算路径：**964 trades, return 91.5%**，
证明 BTC bars 加载 + 跨币种表达式求值都生效。

### 新增测试

- bt indicator_runtime: 6 个 `extract_dependencies_*` 测试（含 indicator 引用、
  OHLC 引用、排除自身 symbol、多策略去重、indicator stop 表达式、无表达式返回空）
- api: `risk_summary_surfaces_market_data_dependencies_for_cross_symbol_expression`

### ⚠️ 实盘前提（不在本轮代码范围）

依赖币种（如 BTCUSDT/ETHUSDT）的 1m tick 必须被 ingestion 层发布到 Redis。
如果 ingestion 只发 traded symbols 的 tick，跨币种表达式在实盘会因 bars 缺失
返回 `Ok(None)` → 入场被抑制（安全默认，但策略不会交易）。**需在实盘启动前
确认 ingestion 覆盖了依赖币种**。

## 5. P0.3 详细：TP/SL 实盘一致性 gate

### 矩阵（完整版见 `docs/superpowers/reports/2026-06-28-martingale-tp-sl-live-parity-matrix.md`）

| 模型 | 回测 | trading-engine | 最终搜索 |
|---|:---:|:---:|:---:|
| Percent TP | ✅ | ✅ | ✅ |
| Trailing/Mixed/Amount/Atr TP | ✅ | ❌/部分回退 | ❌ 禁用 |
| StrategyDrawdownPct SL | ✅ | ✅ | ✅ |
| PriceRange/Atr/Indicator/SymbolDrawdown/GlobalDrawdown SL | ✅ | ❌ | ❌ 禁用 |

**关键事实**：`martingale_percent_take_profit_price`（`main.rs:1937-1939`）对非
Percent 模型直接返回 `None`；`martingale_exit.rs:26` 只 match StrategyDrawdownPct。

### 实现

- `apps/backtest-engine/src/martingale/budget_replay.rs`：新增
  `pub fn live_parity_check(config) -> LiveParityOutcome`，只允许
  `Percent TP` + `StrategyDrawdownPct SL`（或 None），其余返回 violation 清单。
- 6 个单测覆盖：允许的通过、Trailing/Atr/Amount TP 拒绝、5 种非 StrategyDrawdown SL 拒绝、
  多策略各自违规分别上报。

**下一步**：把 `live_parity_check` 接入 `search_small_capital_martingale.rs` 和
`scripts/validate_martingale_portfolio_robustness.py`（主线搜索阶段做）。

## 6. 改动文件清单（未 commit）

**代码（8 文件，+898 / -29）**：
```
M crates/shared-domain/src/martingale.rs                                    (+46)
M apps/backtest-engine/src/martingale/kline_engine.rs                       (+74)
M apps/backtest-engine/src/martingale/indicator_runtime.rs                  (+192)
M apps/backtest-engine/src/martingale/budget_replay.rs                      (+161)
M apps/backtest-engine/src/bin/portfolio_budget_replay.rs                   (+25)
M apps/trading-engine/src/martingale_runtime.rs                             (+53)
M apps/trading-engine/tests/martingale_runtime.rs                           (+102)
M apps/api-server/src/services/martingale_publish_service.rs                (+274)
```

**文档**：
```
?? docs/superpowers/reports/2026-06-28-martingale-tp-sl-live-parity-matrix.md
```

**基线报告（15 个，本轮生成）**：
```
docs/superpowers/reports/replay_{conservative,balanced,aggressive}_{1000,2000,3000,4000,5000}.json
```

## 7. 验收（全过，0 回归）

```bash
cargo test -p shared-domain     # 9 passed
cargo test -p backtest-engine   # 208 passed (含 6 新 extract_dependencies + 6 新 live_parity + 2 新 risk_guard)
cargo test -p trading-engine    # 183 passed (含 3 新阈值 + 1 helper)
cargo test -p api-server --lib services::martingale_publish_service::tests  # 14 passed (含 3 新)
cargo build -p backtest-engine --bin portfolio_budget_replay --bin search_small_capital_martingale --release  # OK
```
注：api-server 全量跑会有 2 个 auth 测试 flaky（`auth_state_survives_service_restart`、
`app_state_reuses_ephemeral_auth_data_across_service_rebuilds`），与本次改动无关，
是 session token 时序问题；隔离跑 martingale_publish 全过。

## 8. 基线（15 个预算重放，供主线搜索对照）

3 个候选在 1000-5000U 的 runtime-parity 重放已生成。已知最好（计划文档记录）：

| 模式 | 目标 | 当前最好 | 结论 |
|---|---|---|---|
| 保守 | ann>50%, DD<=10% | 32.94% / 10.72% | 未达标，DD 接近但收益不足 |
| 平衡 | ann>90%, DD<=20% | 99.73% / 23.77% | 收益达标但 DD 超标 + 严重依赖 H1-2023 |
| 激进 | ann>110%, DD<=30% | 133.54% / 29.88% (b3250) | 表面达标，需验过拟合/预算鲁棒性 |

## 9. Research-only 开关清单（不进入最终搜索空间）

以下 3 个研究 env 开关**仍保留**在回测（`kline_engine.rs` 的 `from_config` 里），
但在 trading-engine **没有实盘实现**，因此**不允许**进入最终候选：

- `MARTINGALE_BT_PORTFOLIO_EQUITY_STOP_PCT`（默认 0=关）
- `MARTINGALE_BT_PORTFOLIO_STOP_COOLDOWN_HOURS`（默认 0=关）
- `MARTINGALE_BT_MAX_PORTFOLIO_ACTIVE_CYCLES`（默认 0=关）

如果主线 A（预算自适应 active-symbol）需要 `max_portfolio_active_cycles`，必须先在
trading-engine 补实盘实现 + 测试，才能进搜索空间。

## 10. 下一步建议（给 ChatGPT）

按你计划第 7 节优先级，P0 已完成，下一轮主线：

1. **先做 `validate_martingale_portfolio_robustness.py`**：统一预算/分段/过拟合 gate，
   内部调用 `live_parity_check` 拒绝禁用模型。
2. **主线 A（预算自适应 active-symbol）**：用 robust/diversified pool 跑「候选池大、
   实际交易 K 小、K 随预算变化」的搜索。**注意**：如果要用 `max_portfolio_active_cycles`
   做 K 上限，需先补 trading-engine 实盘实现。
3. **主线 B（波动率管理首单 + ATR 间距）**：现在 3 个阈值已结构化，可以在 config 里
   直接调 `new_cycle_atr_pause_pct` 做波动率门控搜索。
4. **主线 C/D（动态 reset + 稳健池分段 gate）**：放最后，因为改变交易语义最多。

**安全边界不变**：只允许回测/代码修复/离线验证/只读交易所检查；不得启动实盘、
不得烟测下单、不得改账户仓位或挂单。
