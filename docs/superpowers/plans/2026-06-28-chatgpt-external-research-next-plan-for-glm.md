# 2026-06-28 ChatGPT 给 GLM 的下一轮马丁小资金组合探索计划

> 目标：在 GLM 已提交成果基础上继续探索，找到预算不超过 5000U、能真实实盘复现的马丁组合。
> 重要安全边界：本计划只允许回测、代码修复、离线验证和只读交易所检查；不得启动 Binance 实盘、不得烟测下单、不得改账户仓位或挂单。

## 0. 当前基线必须先锁住

当前已推送到远端仓库：

- 分支：`main`
- 提交：`cd5b9d6 martingale: add small-capital parity research tools`
- 工作区在提交后已确认干净。

已保留的最接近成功成果：

- 代码能力：
  - `apps/backtest-engine/src/martingale/indicator_runtime.rs`
    - 支持跨币种指标表达式，例如 `BTCUSDT.close > BTCUSDT.ema(30)`。
    - 支持 `atr_percent(period)`。
  - `apps/backtest-engine/src/martingale/rules.rs`
    - ATR spacing 预热期回退到 `min_step_bps`。
  - `apps/backtest-engine/src/bin/search_small_capital_martingale.rs`
    - 小资金原生候选搜索工具。
  - `apps/backtest-engine/src/bin/portfolio_budget_replay.rs`
    - 预算本金重放工具，按保证金本金计算年化和回撤。
- 候选配置与结果：
  - `docs/superpowers/artifacts/glm-conservative-candidate/best_conservative_core_sat_b5000.json`
  - `docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_btc_shortdown_b5000.json`
  - `docs/superpowers/artifacts/glm-aggressive-candidate/best_aggressive_fixed_cash_b3250_config.json`
  - `docs/superpowers/artifacts/glm-aggressive-candidate/best_aggressive_fixed_cash_b3250_result.json`
- 候选池：
  - `docs/superpowers/artifacts/glm-small-cap-pools/glm_robust_pool.json`
  - `docs/superpowers/artifacts/glm-small-cap-pools/glm_diversified_pool.json`

已知最好结果：

| 模式 | 目标 | 当前最好 | 结论 |
|---|---:|---:|---|
| 保守 | 年化 >50%，DD <=10% | 32.94% / 10.72% | 未达标，DD 接近但收益不足 |
| 平衡 | 年化 >90%，DD <=20% | 99.73% / 23.77% | 收益达标，DD 超标，且严重依赖 H1-2023 |
| 激进 | 年化 >110%，DD <=30% | 133.54% / 29.88%，预算 3250U | 表面达标，但必须检查过拟合、预算鲁棒性和 live parity |

提交前已跑过：

```bash
PATH=$HOME/.cargo/bin:$PATH cargo test -p backtest-engine --quiet
PATH=$HOME/.cargo/bin:$PATH cargo test -p trading-engine --quiet
PATH=$HOME/.cargo/bin:$PATH cargo build -p backtest-engine --bin portfolio_budget_replay --bin search_small_capital_martingale --release --quiet
```

## 1. 外部资料给出的方向

本轮不是继续盲扫参数。外部资料和当前失败形态共同指向：传统马丁/网格的瓶颈来自趋势破位、高波动窗口、资金地板和过拟合，而不是某一个 multiplier 或 TP 能单独解决。

参考资料：

- Moreira & Muir, Volatility-Managed Portfolios：高波动时降低风险暴露可以提高风险调整后表现。参考：<https://www.nber.org/papers/w22208>
- Chen/Chen/Jang, Dynamic Grid Trading Strategy：传统 grid 在简单假设下接近零期望，动态重置 grid 才可能改善收益与风险。参考：<https://arxiv.org/abs/2506.11921>
- Bailey & López de Prado, Deflated Sharpe Ratio：大量参数试验会导致选择偏差，必须记录 trial 并校正过拟合。参考：<https://papers.ssrn.com/sol3/papers.cfm?abstract_id=2460551>
- Bailey/Borwein/López de Prado/Zhu, Probability of Backtest Overfitting：投资回测中普通 hold-out 不可靠，应使用 CSCV/PBO 评估选择过程。参考：<https://papers.ssrn.com/sol3/papers.cfm?abstract_id=2326253>
- Sullivan/Timmermann/White, Data-Snooping and Technical Trading Rules：技术规则搜索必须量化 data-snooping bias。参考：<https://ideas.repec.org/a/bla/jfinan/v54y1999i5p1647-1691.html>
- Binance USD-M Futures 文档：
  - 普通下单：<https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/New-Order>
  - Algo/条件单 TP/SL/Trailing：<https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/New-Algo-Order>
  - exchange filters：<https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Exchange-Information>
  - positionRisk：<https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/Position-Information-V3>
  - account V3：<https://developers.binance.com/docs/derivatives/usds-margined-futures/account/rest-api/Account-Information-V3>
  - income history：<https://developers.binance.com/docs/derivatives/usds-margined-futures/account/rest-api/Get-Income-History>
  - cancel all open orders：<https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/Cancel-All-Open-Orders>

这些资料落到本项目，就是四个必须探索的机制：

1. **波动率管理**：高波动时缩小首单、减少新周期、拉宽间距或暂停，而不是靠固定几何阶梯硬扛。
2. **动态 grid / 动态重置**：当价格中心、趋势或波动状态改变时，允许暂停、重置或结束周期，避免旧 grid 在趋势市里无限恶化。
3. **预算自适应主动币种上限**：候选池可以很大，但同一时刻只能交易有限 K 个币种；K 必须随预算、minNotional、波动率和相关性动态决定。
4. **反过拟合验证**：所有结果必须过 full-period gate、分段 gate、预算重放 gate、trial registry/PBO 或近似 PBO 检查；不能只看 2023H1。

## 2. 先补齐 live parity，避免继续搜索出“回测幻觉”

下一轮任何新机制，只有在回测和实盘都能同路径表达时，才允许作为最终候选。当前代码里还有几个研究开关不能直接作为最终候选依据：

- `MARTINGALE_BT_NEW_CYCLE_DD_PAUSE_PCT`
- `MARTINGALE_BT_NEW_CYCLE_ATR_PAUSE_PCT`
- `MARTINGALE_BT_SAFETY_SKIP_ADX`
- `MARTINGALE_BT_PORTFOLIO_EQUITY_STOP_PCT`
- `MARTINGALE_BT_PORTFOLIO_STOP_COOLDOWN_HOURS`
- `MARTINGALE_BT_MAX_PORTFOLIO_ACTIVE_CYCLES`

这些必须二选一：

- 要么迁移为 `shared_domain::martingale` 的正式结构化配置，并贯通 api-server publish、backtest-engine、trading-engine。
- 要么从最终搜索空间剔除，只保留为研究诊断工具。

### P0.1 结构化配置改造

建议新增或扩展风险限制字段，命名以现有 `MartingaleRiskLimits` 风格为准：

- `new_cycle_drawdown_pause_pct`
- `new_cycle_atr_pause_pct`
- `safety_skip_adx_threshold`
- `portfolio_equity_stop_pct`
- `portfolio_stop_cooldown_seconds`
- `max_portfolio_active_cycles`
- `max_active_trading_symbols`

验收：

```bash
PATH=$HOME/.cargo/bin:$PATH cargo test -p backtest-engine martingale::kline_engine --quiet
PATH=$HOME/.cargo/bin:$PATH cargo test -p trading-engine martingale --quiet
PATH=$HOME/.cargo/bin:$PATH cargo test -p api-server martingale_publish --quiet
```

### P0.2 跨币种指标 live 数据依赖

当前跨币种表达式已经能算，但实盘必须保证被引用币种的 1m kline 进入同一个 `IndicatorRuntimeContext`。不要用“极小权重 observer 策略”作为最终方案，因为它会污染预算、组合展示、策略计数和风险解释。

实现要求：

- 增加 `indicator_runtime::extract_symbol_dependencies(config)` 或等价函数。
- 解析所有 `IndicatorExpression`、Indicator Stop、ATR TP/SL 可能引用的 `SYMBOL.indicator(...)`。
- backtest 加载交易币种 + 依赖币种的 bars；依赖币种不参与交易、不计入资金、不计入策略数量。
- trading-engine 启动时订阅交易币种 + 依赖币种的 kline；依赖币种只推入指标上下文。
- api-server 展示 `market_data_dependencies`，让用户知道为什么组合需要 BTC/ETH 等观察币种。

验收测试必须覆盖：

- 一个 SOL 策略用 `BTCUSDT.close > BTCUSDT.ema(50)`，回测只交易 SOL，但 bars 加载 BTC+SOL。
- 实盘 runtime 只为 SOL 下单，但 BTC kline 会更新表达式。
- 依赖币种没有权重、没有预算、没有订单、没有手续费统计。

### P0.3 TP/SL/Trailing/ATR 模型实盘一致性

回测引擎里已经有 Percent、Amount、ATR、Trailing、Mixed TP，以及 PriceRange/ATR/Indicator/StrategyDrawdown 等 Stop。最终搜索只能使用实盘能复现的模型。

建议先做一个支持矩阵：

| 模型 | 回测支持 | trading-engine 支持 | Binance 条件单支持 | 最终搜索允许 |
|---|---|---|---|---|
| Percent TP | 是 | 必须确认 | TAKE_PROFIT_MARKET 或内部 close | 是 |
| ATR TP | 是 | 必须补齐或禁用 | 需要内部重算触发价/重挂单 | 待定 |
| Trailing TP | 是 | 必须补齐或禁用 | TRAILING_STOP_MARKET | 待定 |
| Indicator Stop | 是 | 必须补齐或禁用 | 内部触发后 MARKET reduceOnly | 待定 |
| Portfolio Equity Stop | env 研究 | 必须结构化 | 内部 flatten | 待定 |

验收：

- 每个允许模型必须有 backtest 单测 + trading-engine 单测。
- 价格、数量、tick/step rounding、positionSide、reduceOnly、closePosition、workingType 规则必须和 Binance 文档一致。
- 不支持的模型不能进入最终搜索空间。

### P0.4 统计一致性

最终组合要能展示实盘和回测一致的统计：

- 成交记录：order id、client order id、symbol、side、positionSide、qty、avgPrice、status。
- 手续费：来自 fill 或 income history，不能估算覆盖真实成交。
- 资金费率：回测使用 funding_rates；实盘使用 income history 的 funding fee。
- 持仓：`positionRisk` + user data stream `ACCOUNT_UPDATE` 共同校验。
- 开放订单：启动前必须读 open orders，恢复时做策略归属判断，不能重复开单。

## 3. 下一轮搜索主线

### 主线 A：预算自适应主动币种组合

用户提出“根据本金动态调整组合中币种数量”是正确方向，但不能只是硬性 `max_active_cycles`。GLM 已发现硬性上限会压收益。需要做“候选池大、实际交易 K 小、K 随预算变化”的资本分配器。

设计：

- 候选池：使用 `glm_robust_pool.json` + `glm_diversified_pool.json`，再补充全池搜索中的高流动性币种。
- 每个候选策略计算：
  - `min_first_order_notional`
  - `first_leg_margin`
  - `planned_margin`
  - `realized_vol` / `atr_percent`
  - 分段收益和分段 DD
  - 与 BTC/ETH/核心策略的相关性
- 每个预算计算 K：
  - 先满足 Binance minNotional 后的最小可交易首单。
  - 再满足每个成员至少有 `min_strategy_margin_buffer`。
  - 预算越小，K 越小；预算大才允许多币种。
- 组合时只允许同时启用 Top K 个 active symbols，排序可以使用：
  - 低波动优先
  - 近期状态匹配优先
  - 分段稳健收益优先
  - 与当前持仓相关性低优先

必须输出：

- `min_executable_principal_quote`
- `recommended_budget_range`
- `active_symbol_count_by_budget`
- 每个预算的 `budget_blocked_legs` 和 `max_capital_used_quote`

建议预算网格：

```text
500, 750, 1000, 1500, 2000, 3000, 4000, 5000
```

最终可接受标准：

- 每个模式的最终组合必须在 `<=5000U` 的某个预算通过 gate。
- 同一个组合要给出可运行预算区间；低于区间时必须显示“不建议运行/不可执行”，而不是硬跑。
- 对 1000U 这类小预算，如果确实达不到目标，需要如实给出原因：minNotional、K 太小、收益/回撤前沿不可突破，不能用失真计算强行达标。

### 主线 B：波动率管理首单与间距

在 `MartingaleSizingModel` 或新配置中加入可部署的 volatility scaling：

- `first_order_quote = base_margin_budget * target_vol_pct / realized_vol_pct * leverage`
- 设置上下限：
  - `min_notional_quote >= exchange_min_notional`
  - `max_first_order_quote`
  - `max_strategy_margin_quote`
- 间距使用 ATR：
  - 低波动：较密 step
  - 高波动：较宽 step 或暂停新周期
- 不允许用未来数据，只能用当前 bar 以前的滚动数据。

搜索维度：

- `target_symbol_atr_pct`: 0.6, 0.8, 1.0, 1.2, 1.5
- `btc_atr_pause_pct`: 1.5, 2.0, 2.5, 3.0
- `symbol_atr_pause_pct`: 2.0, 3.0, 4.0, 5.0
- `atr_spacing_multiplier`: 0.6, 0.8, 1.0, 1.2
- `min_step_bps`: 80, 120, 180, 250

验收重点：

- 同一配置在 1000/2000/3000/5000 预算下，指标不能剧烈跳变。
- 如果因为 minNotional 导致小预算无法等比例缩放，必须在 `minimum_capital` 中体现。

### 主线 C：动态 grid reset / cycle reset

外部 DGT 资料说明传统 grid 如果不重置，容易在趋势改变后失真。马丁策略也同理：旧周期可能在错误 regime 中越补越深。

新增可部署机制：

- `new_cycle_only_when_regime_ok`：只控制新周期。
- `cycle_reset_when_regime_breaks`：已有周期遇到 regime break 时平仓/停止补仓/转入只减仓。
- `grid_center_reset`：当价格偏离 EMA/VWAP/rolling median 超过 ATR 倍数时，不继续按旧中心补仓。
- `cooldown_after_reset`：reset 后冷却，避免震荡中反复止损。

先从最保守版本开始：

- 只禁止新增安全单，不强制平仓。
- 第二版再测试 portfolio-level stop/flatten。

候选表达式：

- 多头新周期：`BTCUSDT.close > BTCUSDT.ema(50)` 且 `BTCUSDT.atr_percent(14) < X`
- 空头对冲：`BTCUSDT.close < BTCUSDT.ema(30)` 才开
- 高波动暂停：`BTCUSDT.bb_bandwidth(20,2) < Y` 或 `BTCUSDT.atr_percent(14) < X`
- 单币种过热禁止：多头 `rsi(14) < 65`，空头 `rsi(14) > 35`

注意：

- reset/stop 如果只是回测 env 开关，不能作为最终结果。
- reset 会产生真实亏损，必须纳入费用、滑点、资金费率。

### 主线 D：稳健候选池重构，而不是继续依赖 2023H1

GLM 已发现平衡候选几乎全部收益来自 H1-2023，后续分段亏损。这类组合不允许最终入选。

下一轮使用稳健池：

- 从 `glm_robust_pool.json` 读取所有分段正收益候选。
- 对 NEAR/GALA/ADA/COMP/ALGO 等重新做组合参数，而不是直接搬单币 native params。
- 每个候选都必须输出分段：
  - H1-2023
  - H2-2023
  - 2024
  - 2025
  - 2026-01-01..2026-05-31

分段硬约束建议：

- 保守：
  - 任一年度段不能本金击穿。
  - 任一年度段 DD <= 12%。
  - 至少 4/5 个分段 total_return >= 0。
- 平衡：
  - 任一年度段不能本金击穿。
  - 任一年度段 DD <= 24%。
  - 至少 3/5 个分段 total_return >= 0，且 2024-2026 合计不能亏损。
- 激进：
  - 任一年度段不能本金击穿。
  - 任一年度段 DD <= 36%。
  - 2024-2026 合计不能大幅亏损；如果亏损，必须说明它只是高风险不可实盘候选。

## 4. 具体执行步骤

### Step 1：复现基线

```bash
cd /home/bumblebee/Project/grid_binance
git status --short
git rev-parse --short HEAD

PATH=$HOME/.cargo/bin:$PATH cargo test -p backtest-engine --quiet
PATH=$HOME/.cargo/bin:$PATH cargo test -p trading-engine --quiet
PATH=$HOME/.cargo/bin:$PATH cargo build -p backtest-engine --bin portfolio_budget_replay --bin search_small_capital_martingale --release --quiet
```

用 `portfolio_budget_replay` 对 3 个候选做预算重放：

```bash
for profile in conservative balanced aggressive; do
  case "$profile" in
    conservative) cfg=docs/superpowers/artifacts/glm-conservative-candidate/best_conservative_core_sat_b5000.json ;;
    balanced) cfg=docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_btc_shortdown_b5000.json ;;
    aggressive) cfg=docs/superpowers/artifacts/glm-aggressive-candidate/best_aggressive_fixed_cash_b3250_config.json ;;
  esac
  for budget in 1000 2000 3000 4000 5000; do
    target=docs/superpowers/reports/replay_${profile}_${budget}.json
    ./target/release/portfolio_budget_replay \
      --config "$cfg" \
      --budget "$budget" \
      --profile "$profile" \
      --start-ms 1672531200000 \
      --end-ms 1780271999999 \
      --market-data data/market_data_full.db \
      --funding-data data/funding_rates.db \
      --exchange-min-notional 5 \
      --equity-curve-points 600 > "$target"
  done
done
```

输出一个基线表：

- full-period ann/DD
- max_capital_used
- min_equity
- principal_breached
- budget_blocked_legs
- minimum_capital
- 分段表现

### Step 2：新增分段/预算验证工具

现有 `portfolio_budget_replay` 一次只跑一个区间。建议新增脚本或扩展参数：

```bash
scripts/validate_martingale_portfolio_robustness.py \
  --config <config> \
  --profile conservative|balanced|aggressive \
  --budgets 500,750,1000,1500,2000,3000,4000,5000 \
  --segments h1_2023,h2_2023,2024,2025,2026_ytd \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --out docs/superpowers/reports/<candidate>_robustness.json
```

该工具必须返回：

- `full_period_gate`
- `budget_matrix`
- `segment_matrix`
- `min_executable_principal_quote`
- `recommended_budget_range`
- `overfit_flags`
- `live_parity_flags`

### Step 3：把 env 研究开关转配置，或者禁用

先实现 P0.1/P0.2，否则后续搜索结果不允许进入最终候选。

完成后 rerun：

```bash
PATH=$HOME/.cargo/bin:$PATH cargo test -p backtest-engine --quiet
PATH=$HOME/.cargo/bin:$PATH cargo test -p trading-engine --quiet
PATH=$HOME/.cargo/bin:$PATH cargo test -p api-server --quiet
```

如果 `api-server` 全量测试太慢，至少跑 martingale publish 相关测试。

### Step 4：扩展搜索器

在 `search_small_capital_martingale.rs` 或新 bin 中增加：

- `--budgets`
- `--candidate-pool docs/superpowers/artifacts/glm-small-cap-pools/glm_robust_pool.json`
- `--max-active-symbols-by-budget 500:1,1000:2,2000:3,3000:4,5000:6`
- `--allocator vol_target|robust_score|correlation_penalty`
- `--regime-filters btc_trend,btc_atr,symbol_atr,bb_width`
- `--dynamic-reset none|pause_safety|pause_cycle|flatten_on_break`
- `--trial-log docs/superpowers/reports/trial_registry_<run>.jsonl`

搜索顺序：

1. 单策略候选：先按每个币种、方向、预算找稳定参数。
2. 小组合 beam：从 2-3 个币种开始，逐步加到预算允许的 K。
3. 三个 profile 分别优化，不要用一个激进组合硬降权当保守组合。
4. 每 5000 个 trial 输出一次 frontier，避免超时损失。

### Step 5：验收 gate

最终候选必须同时满足：

- 保守：full-period 年化 >50%，DD <=10%。
- 平衡：full-period 年化 >90%，DD <=20%。
- 激进：full-period 年化 >110%，DD <=30%。
- `principal_breached=false`
- `max_capital_used_quote <= budget_quote`
- `min_executable_principal_quote <= 5000`
- `runtime_weight_caps_applied=true`
- 如果使用跨币种指标，`market_data_dependencies` 明确列出并通过 live 订阅测试。
- 如果使用 TP/SL/Trailing/ATR/Indicator stop，必须在 trading-engine 中有对应实现和测试。
- 2024-2026 不能作为整体持续亏损来源；否则判定为 2023H1 过拟合。
- 所有 trial 进入 `trial_registry`，最终报告给出 trial 数量和选择偏差风险。

### Step 6：结果展示前的最终复验

找到候选后，不要直接写入 flyingkid。先生成以下文件：

```text
docs/superpowers/reports/2026-06-28-final-small-cap-martingale-candidates.md
docs/superpowers/reports/2026-06-28-final-small-cap-martingale-candidates.json
docs/superpowers/reports/2026-06-28-final-small-cap-martingale-validation-matrix.json
```

报告必须包含：

- 三个最终组合配置路径。
- 每个组合的预算区间和推荐预算。
- 各预算 ann/DD 表。
- 各时间段 ann/DD/return 表。
- active symbol count by budget。
- max_capital_used、budget_blocked_legs、fees、slippage、funding。
- 是否使用研究开关；如果使用，必须已结构化进配置。
- live parity 支持矩阵。

只有报告通过后，才执行：

- 将最终 3 个组合展示到 `flyingkid` 回测账户。
- 归档其他组合，只保留最终 3 个组合。

## 5. 后续实盘验证计划只写文档，不启动

回测达标并展示后，再写实盘验证计划。注意这一步仍然不能直接启动实盘。

实盘计划至少包括：

1. Dry-run 启动：
   - 不下单，只跑行情、指标、信号、预算、TP/SL 价格、rounding。
   - 对比同一时间窗口回测 replay。
2. 50U 小资金测试：
   - 只在用户再次明确确认后执行。
   - 覆盖首单、安全单、TP、SL、资金费率、手续费、恢复已有仓位/挂单。
3. 1000U 正式启动：
   - 必须在启动前再次向用户确认。
   - 启动前检查 open orders、positions、account mode、isolated/leverage、multi-assets off。
   - 如果已有仓位/挂单，必须判断是否属于策略；不允许重复开单。
4. 异常处理：
   - 任意指标缺失、订阅断开、订单状态不一致、费用/资金费率无法同步，立即 pause new cycles。
   - 如需 cancel/flatten，必须先给出账户现状和影响，取得用户确认。

## 6. 如果仍然找不到

如果完成上述搜索仍无法达标，必须输出失败证明，而不是继续无限搜索：

- 搜索空间定义。
- trial 总数。
- 每个 profile 的 Pareto frontier。
- 不同预算下的最佳 ann/DD。
- 为什么 <=5000U 不能同时满足收益和回撤：
  - minNotional 地板
  - active symbol count 不足
  - 高收益候选集中在 2023H1
  - DD 来自不可过滤的趋势窗口
  - budget clipping 导致收益/DD 前沿断层
- 哪些新机制可能需要产品层确认：
  - 接受更大预算
  - 接受更高 DD
  - 接受更低年化
  - 接受动态平仓/重置带来的真实止损

## 7. 推荐的下一步优先级

1. 先做 P0.1/P0.2：把研究开关和跨币种依赖变成正式可部署能力。
2. 做 `validate_martingale_portfolio_robustness.py`，统一预算/分段/过拟合验证。
3. 用 robust/diversified pool 运行预算自适应 active-symbol 搜索。
4. 加入 volatility-managed sizing 和 ATR spacing。
5. 最后才测试 dynamic reset/portfolio equity stop，因为它改变交易语义最多，必须最谨慎。

核心判断：下一轮的突破点不应是“再多扫 multiplier”，而是“在小资金下只交易少数最合适币种，并让首单/间距/新周期资格随波动率和市场 regime 动态变化”。所有机制都必须先进入结构化配置和实盘 runtime，再允许成为最终组合。
