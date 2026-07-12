# GLM Martingale Round 11 Search Ledger

> 2026-07-12 独立审计已校正本台账。原始“完全 production live-ready”“原生
> minigrid 失败”“全部非 R4 架构失败”和 `159192` 次真实回测均为过度表述。
> 机器可读权威结果见
> `docs/superpowers/artifacts/glm-martingale-core-round11/r11-final-validation.json`。

## Canonical Carry-In

- Round1-10 权威状态：`docs/superpowers/artifacts/glm-martingale-core-round10/r1-r10-corrected-status.json`。
- Round10 前最佳曲线研究值：R9 allocator `64.4196% / 18.2111% / 5/5`，当时已明确不是 fully live-ready。
- Round10 前最佳已接受 live-ready sleeve：R4-combo `34.7233% / 17.6843% / 4/5`。
- Round1-10 无任何三档目标命中。

## r11-P1-live-allocator-production-wiring-001

**校正状态：PARTIAL。**

已完成：

- `martingale_allocator_live.rs` 的 config/state/observation 解析与滚动指标 helper；
- `risk_summary > config > fallback` 状态优先级；
- 初次创建 executor 前的 active sleeve 新 cycle gate；
- allocator state JSON 序列化与初始路径持久化；
- 7 个 helper/direct-runtime 测试。

未完成：

- `live_executor_started=true` 后，`main.rs` 直接调用
  `reconcile_martingale_executor_strategies(...)` 并 `continue`，动态 rebalance 被绕过；
- started-executor 路径不安装 allocator state，也不 gate 新 cycle；
- 仓库只有 `allocator_observations` reader，没有生产 writer；
- inactive sleeves 没有可用于决策的 shadow equity/position 状态；
- 7 个测试均未调用 DB-backed `reconcile_running_martingale_portfolios`。

结论：仅 helper + static-start gate 可用，`fully_live_ready=false`。

## r11-P2-r9-winner-production-parity-001

**校正状态：RESEARCH CURVE REPLAY ONLY。**

原脚本只调用 Python `run_allocator_repaired`：

- 未启动 `trading-engine`；
- 未执行 DB reconcile/state persistence/executor；
- `forward_only_decisions_pass` 曾直接硬编码 `True`；
- `production_live_ready_after_p1` 曾只检查 P1 JSON 文件是否存在；
- 未输出 rebalance decision trace；
- 未保持切换时的共享资金与 open-cycle 状态。

历史数据库下复算仍精确得到：

```text
ann=64.4196, DD=18.2111, total_return=446.7542, curve-slice pos=5/5
```

但 `funding_rates.db` 完全没有 ANKRUSDT。使用 Binance 官方接口补齐 3997 个
ANKRUSDT funding 点到临时数据库后：

```text
ann=62.7845, DD=18.3840, total_return=428.3994, curve-slice pos=5/5
```

该结果只能保留为曲线分配诊断，不可推广为 event-level 或 production parity。

## r11-P3-native-minigrid-001

**校正状态：CONFIG/HELPERS ONLY。**

实际完成 `MartingaleDcaMiniGridConfig`、validation、level price 和 close fraction
数学 helper。5 个测试没有执行 `kline_engine` bar loop，没有断言真实
`dca_minigrid_take_profit` event，也没有验证 live reduce-only order。

结论：

```text
native_minigrid_backtest_ready=false
native_minigrid_live_ready=false
research_only=true
```

## r11-P4-native-minigrid-search-001

**有效失败范围：partial-TP minigrid approximation。**

- 6912 labels，41472 次 `portfolio_budget_replay`，0 skipped，0 target。
- 最佳：`16.6529% / 35.5198% / 2/5`。
- 分段正收益计数：`0:4341, 1:1941, 2:444, 3:162, 4:24, 5:0`。
- 脚本通过额外 partial-TP stages 近似 minigrid，`dca_minigrid` config 本身未被 engine 执行。
- 缺少计划要求的 realized PnL contribution 和真实 live readiness gate；因 0 target，
  不影响“本近似无命中”的结论。

非重复键：`r11-partial-tp-minigrid-approx-6912-no-target`。

原生 minigrid 仍未测试，禁止使用旧键 `r11-native-minigrid-no-target`。

## r11-P5-non-r4-architecture-001

**有效失败范围：fixed-TP dominated narrow grid。**

- 8100 labels，48600 次 `portfolio_budget_replay`，0 skipped，0 target。
- 最佳：`21.9292% / 23.1575% / 2/5`。
- family 数：fixed_tp `2304`、fixed_tp_v3 `5692`、low_mult `36`、
  vol_ladder `4`、asym `64`。
- 分段正收益计数：`0:4217, 1:2707, 2:982, 3:167, 4:27, 5:0`。
- 99% labels 为 fixed TP；所谓 vol_ladder 只改 `new_cycle_atr_pause_pct`，
  并未使用 `MartingaleSpacingModel::Atr`。

非重复键：`r11-fixed-tp-dominated-7996-no-target`。

禁止将本结果写成“所有非 R4 架构失败”；真实 ATR spacing、cycle-depth TP、
timeframe ensemble 等仍未测试。

## r11-P6-conditional-so-v2-001

**校正状态：PARTIAL，存在 inert dimensions。**

- 4608 labels，27648 次 `portfolio_budget_replay`，0 skipped，0 target。
- 只有 217 组不同 full metrics、260 组不同 segment metrics。
- 最佳历史值：`63.5105% / 28.1972% / 4/5`，等于缺失 ANKR funding 的 R7 base。
- 正确分段计数：`0:816, 1:1072, 2:1472, 3:880, 4:368, 5:0`。

根因：脚本把 `safety_skip_adx_threshold` 和 `drawdown_state_rules` 写到
strategy risk limits；engine 从 portfolio risk limits 读取这两个字段，因此历史
ADX/DD-scale 维度无效。rebound、basis、taper、step、FOQ 维度仍有效。

审计后已修正脚本字段层级，但旧 artifact 没有重跑；Round12 必须以新
`semantics_version=2` 重新执行。

非重复键：`r11-so-v2-effective-controls-historical-grid-no-target`。

## r11-P7-heterogeneous-allocator-001

**校正状态：LEGACY SAME-SLEEVE CURVE GRID。**

- 仍是 R9 的 ANKR-q/QB/R4/fine/p4-ANKR-q-rp10-fos05 五个 sleeves；
- 未加入 P4/P5/P6 新架构候选；
- 6912 labels 仅对应 768 组有效非 weight 组合；
- `max_high_ann_weight` 的所有正值等价，`min_low_dd_weight` 的所有正值也只是
  eligibility switch，不是 fractional weight；
- 仅 72 组不同 full metrics、316 组不同 full+segments；
- 只构建 5 条底层权益曲线，其余 41472 次是内存曲线计算，不是
  `portfolio_budget_replay` 进程；
- 0 target；最佳仍为历史 `64.4196% / 18.2111% / 5/5`。

更根本的语义问题：inactive sleeve 在预计算曲线中持续运行并积累持仓；切换后
直接取得其下一段 PnL。生产 gate 只允许 active sleeve 开新 cycle，无法继承这些
未实盘建立的持仓。

非重复键：`r11-r9-same-sleeve-curve-allocator-grid-no-target`。

## Data Audit

Round11 manifest 与当前文件：

```text
market_data_full.db manifest cc5b88c1... current be2514c8... (数据库后续追加)
funding_rates.db     356e270d... unchanged
premium_index.db     78bd0128... unchanged
```

R9 的 8 个交易币在目标区间均有恰好 `1,795,680` 条 futures 1m bar，min/max/sum
与连续分钟区间一致；历史 R9 结果也能精确复现。因此 market DB 整库 hash 变化
未破坏本次目标区间。但没有 immutable range snapshot，未来仍不能只依赖整库 hash。

资金费覆盖中，AAVE/BCH/BNB/DOT/SOL/TRX/XRP 各有 3741 个目标区间点，
ANKR 为 0；这已导致前沿收益修正。

## Corrected Replay Accounting

```text
P4 binary replays: 41472
P5 binary replays: 48600
P6 binary replays: 27648
P2/P7 underlying curve builds: about 10
estimated portfolio_budget_replay processes: 117730
historical claim: 159192
```

历史差额主要是把 P7 的 41472 次内存 curve calculations 当成真实组合回测。

## Corrected Target Verdict

| Tier | Gate | Round11 corrected result |
|---|---|---|
| Conservative | ann >=50%, DD <=10%, `<5000U`, multi-symbol, OOS, live | FAIL |
| Balanced | ann >=90%, DD <=20%, `<5000U`, multi-symbol, OOS, live | FAIL |
| Aggressive | ann >=110%, DD <=30%, `<5000U`, multi-symbol, OOS, live | FAIL |

补齐资金费后的最佳 event-level R7 backtest 为 `62.3718% / 28.6873% / 4/5`，
仍非 fully live-ready；R4-combo 仍是此前已接受的 live-ready sleeve，但只有
`34.7233% / 17.6843% / 4/5`。

小资金补充复算：同一 R7 config 在 `4999U` 为 `62.3795% / 28.6894%`，在
`2000U` 仅为 `5.4232% / 45.5823%`。以后不得再用 5000U 单点或
`max_capital_used` 代替预算阶梯验证。
