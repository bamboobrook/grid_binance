# GLM Martingale Core Round 14：真实 HTF、横截面选择与库存感知组合计划

执行者：GLM

上游权威状态：

- `docs/superpowers/artifacts/glm-martingale-core-round13/r1-r13-corrected-status.json`
- `docs/superpowers/artifacts/glm-martingale-core-round13/r13-final-validation.json`
- `docs/superpowers/reports/2026-07-13-glm-round13-execution-audit-and-fix.md`

原 Round 13 handoff、ledger 和搜索产物必须先应用上述审计修正，不能直接作为晋级依据。

## 1. 唯一目标

Round 14 继续以 Martingale/DCA/grid cycle 为唯一交易和收益核心。趋势、动量、反转、
波动率、相关性和 allocator 只能决定：

1. 哪些 Martingale symbol/direction 可开新 cycle；
2. first order、safety order、spacing、TP 和资金预留如何缩放；
3. 哪些已有 Martingale inventory 应 reduce-only、暂停加仓或退出；
4. 多个 Martingale sleeves 如何共享 `<5000U` 真实账户。

禁止把独立趋势仓、breakout 仓、buy-and-hold、funding carry、pair neutral、stat-arb、
curve rotation 或 shadow PnL 计入收益。任一非 Martingale 收益进入 equity，整组实验作废。

| 档位 | 年化收益 | 最大回撤 | 正收益 cold-start segments |
|---|---:|---:|---:|
| 保守 | >=50% | <=10% | >=4/5 |
| 平衡 | >=90% | <=20% | >=4/5 |
| 激进 | >=110% | <=30% | >=3/5 |

共同硬门：

- 明确一个 launch budget：`1000/2000/3000/4000/4999U`，严格 `<5000U`；
- 五档预算都必须能执行、无 principal breach，并报告最小权益、拒单和最大占资；
- event-level shared account，至少 5 个 traded symbols、至少 3 个独立 base assets；
- 单 symbol configured capital、realized gross profit、realized net positive PnL 均 `<=35%`；
- fee、slippage、funding、minNotional、tickSize、stepSize、leverage 和 liquidation buffer 生效；
- nested WFO、邻域、LOSO/LOCO、成本与延迟压力、锁定 OOS、生产 trace 全部通过；
- 没有 30 天未来 paper 证据只能叫 backtest candidate，不能叫 production-ready。

## 2. Round 13 强制纠偏

以下是 Round 14 的启动 gate，不是可选建议：

1. P2 旧“HTF”只是当前 1m 上的 BTC EMA expression，没有 1h/4h completed-bar state；
   只能拒绝该精确表达式，不能拒绝 HTF trend-directed Martingale。
2. P3 allocator/scheduler 是孤立 helper，没有接入 event replay、started executor 或 DB；
   旧参数不 bind 不能证明动态 allocator 失败。
3. P4 旧 minigrid 从安全腿直接删除 quantity，并按该腿价格虚构 PnL；真实交易所按方向持仓
   均价结算。旧 512-config 数值全部失效，必须在 corrected engine 上重跑。
4. P5 旧 depth TP 忽略自己的 reduce pct，partial TP 又重复扣 entry costs；旧 432-config
   数值全部失效，不能进入 non-repeat。
5. P6 的 28 份配置来自 `glm_robust_pool.json`，不是原 LP members；ICP/TRX 还只抽取
   long leg。它们可作为 robust-pool extracts，但不得再叫“原 LP 恢复”。
6. P9 同名测试没有调用 production executor/DB。没有 started path、writer、restart、
   reconcile 和 order trace，就必须 `production_ready=false`。
7. Round 13 holdout 已经打开；任何后续调参都不能再称该窗口 untouched。
8. 所有受 partial fee、vol-target capital accounting、minNotional 修复影响的旧数值均为 stale。

## 3. 外部研究转化的可证伪假设

外部资料只定义机制和检验，不证明本项目有收益：

| 研究 | 可测试转化 | 禁止外推 |
|---|---|---|
| Moskowitz/Ooi/Pedersen, *Time Series Momentum*, DOI `10.1016/j.jfineco.2011.11.003` | completed 1h/4h 方向状态只控制 Martingale 新 cycle 和 SO | 不开独立趋势仓 |
| Drogen/Hoffstein/Otte, *Cross-sectional Momentum in Cryptocurrency Markets*, DOI `10.2139/ssrn.4322637` | 用 lagged rank 选择可开 Martingale 的 symbols | 不把论文组合收益搬入回测 |
| Dobrynskaya, *Cryptocurrency Momentum and Reversal*, DOI `10.3905/jai.2023.1.189` | momentum 与短周期 reversal 分成两个独立 family | 不事后混合最优 horizon |
| Moreira/Muir, *Volatility-Managed Portfolios*, DOI `10.1111/jofi.12513` | 高实现波动时缩小 FO/SO 和组合风险 | 不假设缩波动必然增收益 |
| Qiao/Yan/Deng, *Downside Volatility-Managed Portfolios*, DOI `10.3905/jpm.2020.1.162` | downside semivariance 单独驱动风险缩放 | 不用全样本调目标波动率 |
| Lo/MacKinlay variance-ratio test, DOI `10.1093/rfs/1.1.41` | 区分 trend-like 与 mean-reverting state | VR 只能分类，不能单独下单 |
| Chu/Zhang/Chan, adaptive crypto markets, DOI `10.1016/j.irfa.2019.05.008` | 参数必须按 walk-forward 重新选择，允许状态失效 | 不宣称固定 regime 永久有效 |
| Stoikov/Saglam, inventory risk, DOI `10.1007/s11147-009-9036-3` | 对已有 inventory、方向暴露和 safety reserve 加惩罚 | 期权做市结论不能直接当币圈收益 |
| Bailey et al., PBO, DOI `10.21314/jcf.2016.322` | 记录全部 trials，估计 selection overfit | PBO 不替代 OOS 收益门 |
| Bailey/Lopez de Prado, Deflated Sharpe, DOI `10.3905/jpm.2014.40.5.094` | 修正多重试验和非正态收益 | DSR 不替代 DD/资金门 |

## 4. P0：冻结输入与追加式台账

### P0.1 分支与输入

从包含 Round 13 审计修复的远端提交创建：

```bash
git fetch origin
git checkout glm-martingale-core-round13
git pull --ff-only
git checkout -b glm-martingale-core-round14
```

启动时记录并验证：

```text
market DB SHA256
funding DB SHA256
manifest SHA256
release replay binary SHA256
git commit + dirty source hash
31-symbol development/holdout gates
```

若任一 hash 与上游 corrected status 不同，先重建 manifest 并复算 baseline；不得静默沿用数值。

### P0.2 Registry 合同

新目录：

```text
docs/superpowers/artifacts/glm-martingale-core-round14/
  exploration-registry.jsonl
  checkpoints/
  run-manifests/
  rejected/
  promising/
  traces/
```

每次尝试先写 `running`，结束追加终态。每条至少包含：

```text
experiment_id, family, hypothesis, parent_config_hash,
resolved_config_hash, engine_hash, data_hash, code_commit,
train_window, validation_window, budget, seed, optimizer,
actual_binary_replays, cache_hits, wall_seconds, peak_rss,
full metrics, segment metrics, concentration, reject_reasons,
artifact hashes, exact non_repeat_scope, status
```

JSONL 任一行非法、trial 只有赢家、缺 raw command/hash，当前 phase 直接失败。

## 5. P1：先闭合引擎与批量回放可信度

### P1.1 成交和资本语义

在搜索前用 synthetic event tests 锁定：

1. reduce-only 按 symbol/direction 聚合持仓均价结算，不能指定某个 safety leg 盈利；
2. partial/minigrid/depth reduce 同比缩减 quantity、margin、entry fee 和 entry slippage；
3. 被预算拒绝的 order 不计 fee/slippage，不改变 equity；
4. vol-target 后的实际 notional 同时用于 budget、fee、capital 和 event；
5. minNotional/tickSize/stepSize 在 entry、SO、partial、close 四条路径一致；
6. same-bar 同时触发 SO/TP/SL 时采用保守、确定的顺序；
7. funding 与 forced final close 不重复记账；
8. gross/net PnL contribution 可逐 symbol 对账到总账户。

### P1.2 BatchReplay 完成标准

补齐 Round 13 未完成项：

- CLI raw JSON resolver 与 typed config 使用同一 weight/cap/filter 逻辑；
- bars/funding 一次加载，immutable `Arc` 共享，runtime state 完全独立；
- effective-config + engine + data + window hash cache；
- checkpoint/resume，重复运行顺序和结果不变；
- 20 个历史 configs 对 CLI subprocess：events、trades、PnL、DD、funding，容差 `1e-9`；
- 100 configs benchmark，吞吐至少 subprocess `5x`；未达到必须附 profiler；
- 坏 SQLite 行、缺 dependency symbol、缺 funding 一律 fail-closed。

GPU 仍不直接跑分支密集的 event engine。只有 profiler 证明无分支 indicator matrix 占
总时间 `>60%`，才可用 5090 批量算 EMA/ADX/ATR/VR 特征；最终 event trace 必须由 CPU
权威引擎复核。

P1 未通过，只允许每 family 运行最多 16 个 binding probes。

## 6. P2：真正 completed-HTF Time-Series Regime

### P2.1 状态实现

从 1m 数据确定性聚合 1h/4h。时间 `t` 的决策只能读取结束时间 `<=t` 的完整 HTF bar。
每个 traded symbol 独立计算，BTC 只可作为背景状态：

```text
TREND_LONG
TREND_SHORT
MEAN_REVERTING
NEUTRAL
EXTREME_DOWNSIDE_VOL
```

输入分层，不一次混成黑箱：

- Family A：EMA 50/200 position + 6/12/24-bar slope；
- Family B：1h/4h time-series return sign + 20/40-bar ADX；
- Family C：variance ratio `q=4/8/16` 区分 trend/mean reversion；
- Family D：downside semivariance percentile `60/75/90` 只做风险状态。

状态只能执行：允许/禁止新 long/short Martingale cycle、FO scale、SO scale、下一腿 spacing
和 cooldown。状态变化不得复制、强平或 orphan 已有 cycle。

### P2.2 必需测试

```text
htf_uses_only_completed_boundary
incomplete_htf_bar_cannot_change_state
per_symbol_state_not_replaced_by_btc_state
long_state_blocks_only_new_short_cycle
existing_cycle_remains_managed_after_state_flip
restart_restores_htf_state
backtest_live_htf_trace_matches
extreme_downside_vol_scales_real_order_and_budget
```

### P2.3 搜索协议

Bases：修复后 R4、R7、robust-pool extracts；每个 base 分开记录。

1. 每 family 16 个极值 binding probes；
2. 每 family 256 Sobol train-only screen；
3. 只有 validation median ann `>=30%`、worst DD `<=35%`、至少 3 folds 正收益，才扩至
   每 fold最多 1024 constrained trials；
4. 每 fold最多选 20 个 Pareto configs，validation 只跑一次；
5. 旧 `BTCUSDT.close > EMA` 当前周期表达式 hash 命中时 `skipped_duplicate`。

## 7. P3：横截面 Momentum/Reversal Martingale Selector

这是未探索主线，不允许再用预计算 sleeve curves。

### P3.1 双状态 event 模型

- Shadow：所有 sleeves 只维护 lagged score 和虚拟观察，不进入 live equity；
- Live：被选 sleeve 只能从切换后开新 Martingale base cycle；
- inactive 已有 cycle 继续管理 SO/TP/SL，直到自然关闭；
- rebalance 不能复制 shadow position、历史 PnL 或未成交 safety legs；
- 全部 sleeves 共用同一真实 budget、margin 和 exchange filters。

### P3.2 两个独立 family

先分别检验，禁止看到结果后临时混合 horizon：

```text
XS-MOM:
  lookback = 7/14/28/56d
  skip_recent = 0/1/3d
  rebalance = 1/3/7d
  active ranks = top 3/5/7 long, bottom 3/5 short

XS-REVERSAL:
  lookback = 4h/12h/1d/3d
  rebalance = 4h/12h/1d
  active ranks = 3/5
  confirmation = variance-ratio mean-reverting state only
```

Rank 在 completed boundary 计算；universe 在 train 前冻结，禁止按全样本盈利选币。新 cycle
单 symbol cap `20/25/30%`，cluster cap `35/45%`，至少 5 symbols 实际成交。

搜索顺序：32 binding probes -> 384 train-only screen/family -> 达到 `ann>=35%, DD<=30%,
3/5 positive` 才进入每 fold 1024 trials。否则只关闭精确 family/horizon 范围。

## 8. P4：Mean-Reversion/Trend 双状态 Martingale Ladder

目的不是预测收益，而是避免逆强趋势无限加码：

```text
MEAN_REVERTING:
  normal DCA spacing and multiplier
TREND_ALIGNED:
  normal/new cycle; optional wider TP
TREND_ADVERSE:
  SO scale 0.25/0.5/0.75; spacing x1.25/1.5/2.0
EXTREME:
  freeze next SO 1/2/4 HTF bars; never orphan existing inventory
```

状态来源只使用 P2 已通过的 EMA/ADX/VR 组件。每一 safety trigger 在上一 completed HTF
boundary 冻结，不能随当前 1m bar 漂移。

Ablation：state gate、SO scale、spacing、gate+SO、SO+spacing、全部组合。每项先 128 configs；
只有单组件相对父 config 在至少 3 个 validation folds 改善 Calmar 且收益不下降超过 10%，
才允许组合。禁止直接跑笛卡尔积。

## 9. P5：Inventory-Aware Ladder 与 Downside-Vol Risk Budget

### P5.1 真实库存状态

每个决策边界计算：

- 当前方向 notional/margin、聚合均价、未实现 PnL；
- 剩余原 ladder safety reserve；
- symbol/cluster/global gross exposure；
- 过去 7/30/60d downside semivariance；
- 距 liquidation、symbol cap、portfolio DD gate 的 buffer。

调度器先为已有 cycle 预留下一腿，再决定新 cycle。score 只能排序 Martingale cycle，不能
生成独立订单。

### P5.2 可测试参数

```text
max live cycles: 2/3/4/5
reserve next legs: 1/2/all
symbol cap: 20/25/30/35%
cluster cap: 35/45/55%
downside-vol target percentile: 40/60/80
risk scale floor: 0.25/0.5/0.75
inventory penalty: 0/0.25/0.5/1.0
```

先做 256 mechanism screen。继续条件：相对父 config，validation worst DD 至少下降 15%，
median ann 保留至少 85%，且 1000/2000U blocked-leg 和 breach 不恶化。

## 10. P6：修正后的 Minigrid 与 Depth TP 重跑

旧 Round 13 结果不是负证据，必须使用 corrected engine 新建 experiment IDs。

### P6.1 Minigrid

先区分两个 family：

- `aggregate_profit_reduce`：以整个 symbol/direction 聚合均价计算，扣成本后仍盈利才减仓；
- `bounded_risk_reduce`：允许小额已知亏损换取 margin 释放，但每 cycle loss budget 明确封顶。

不能声称“卖出某个 safety leg”。支持 1/2/3 active bands 时，每个 band 的总 reduce cap 不得
超过其 safety quantity，但交易所 realized PnL 仍按聚合均价。partial fill/restart 必须幂等。

32 binding probes 后最多 512 Sobol configs/family。只有 `ann>=35%, DD<=25%, 3/5 positive`
才进入 nested WFO。

### P6.2 Safety-Depth TP

`filled_safety_leg_count` 必须独立于当前剩余 legs；每次新 safety fill 才进入新 depth。
reduce pct、entry cost、margin release、breakeven 和后续 TP 必须可逐 event 对账。

先复跑旧 top/bottom/center 32 个配置验证修复影响，再跑最多 768 train-only trials。
禁止把旧 432-config artifact 合并计数。

## 11. P7：受控组合搜索

只有 P2-P6 中通过各自 continue gate 的组件可组合。组合次序固定：

1. HTF regime + inventory budget；
2. XS selector + inventory budget；
3. 双状态 ladder + corrected depth TP；
4. selector + 最多一个 exit/reduce mechanism；
5. 最后才允许三组件组合。

每次只新增一个机制并保留 parent ablation。每个组合最多 512 train trials；若新增机制在 4 个
validation folds 中少于 3 个改善，立即登记失败，不继续叠加。

三档 profile 必须是三套独立 resolved configs，不得用同一曲线乘杠杆或线性缩放伪造。

## 12. P8：抗过拟合与严格晋级

### P8.1 Anchored nested WFO

```text
F1 train H1-2023       purge 7d -> validate H2-2023
F2 train 2023          purge 7d -> validate 2024
F3 train 2023-2024     purge 7d -> validate 2025
F4 train 2023-2025     purge 7d -> validate 2026-YTD
```

每 fold 在 train 内重新搜索、选参；validation cold-start 且只读一次。固定参数分段重放不叫
WFO。记录 selection frequency、rank degradation、PBO proxy、DSR、总试验数和所有失败。

### P8.2 Robustness

- 连续参数 `+/-5%, +/-10%`，离散参数相邻档；>=60% 邻域保持 tier DD 且 ann>=中心70%；
- leave-one-symbol-out、leave-one-cluster-out、leave-one-side-out；
- universe split：train symbols 与 symbol-holdout 分开，禁止全样本挑币；
- gross profit、net positive PnL、DD、capital、funding 分 symbol/cluster 归因；
- 任一 concentration `>35%` 直接拒绝，不允许靠增加亏损币稀释权重。

### P8.3 压力测试

```text
fee x1.5
slippage x2
fee x1.5 + slippage x2
funding adverse +2bps/event
entry/SO delay 1 and 3 bars
partial fill 50%
tick/step/minNotional boundary
one symbol data delay
restart during open cycle
```

stress 必须改变对应 event hash。要求无 breach、ann>0、DD<=tier+5pp。

## 13. P9：锁定 OOS 与生产复现

`2026-06-01..2026-07-10` 已被多轮打开，只能作 contaminated diagnostic。新候选在读取任何
后续数据前提交 config/engine/data hashes。预留 `2026-07-14` 之后至少 30 个完整自然日作为
未来 paper 窗口；在当前日期无法完成该门，必须诚实标记 pending。

每个 finalist 的生产测试必须调用真实入口：

```text
started_executor_applies_selector_and_htf_gate
inactive_shadow_never_changes_live_equity
existing_inactive_cycle_remains_managed
production_writer_persists_observations
restart_restores_htf_selector_inventory_state
db_reconcile_matches_backtest_switch_trace
reduce_only_order_matches_average_entry_accounting
exchange_filters_match_replay
partial_fill_is_idempotent
budget_rejection_matches_replay
```

必须附 DB rows、order intents、fills、state snapshots 和 backtest trace diff。测试名相同但只调用
helper，按未执行处理。

## 14. 停止、晋级与禁止重复

立即停止当前 config/family并记录，如果：

- resolved hash 重复或参数不改变 event trace；
- 使用 full period 排名选参或 validation 反向调参；
- 依赖 symbol/funding 不在 manifest；
- shadow/curve PnL 混入 live equity；
- 非 Martingale order 产生收益；
- symbol/cluster concentration 超限；
- 任一 WFO、邻域、LOSO、stress 或 production gate 失败。

可直接跳过的精确范围：

- Round 12 静态 36-strategy simultaneous merge；
- Round 12 普通 partial-stage + max-age 576 grid；
- Round 10 strict all-or-nothing conditional SO；
- Round 11 fixed-TP dominated ranges；
- Round 13 当前周期 BTC EMA expression 伪 HTF；
- Round 13 R4 上不接主循环的 scheduler 参数 screen；
- 同 engine/data/window 下 hash 完全相同的 corrected baseline。

不得跳过：真实 completed-HTF、event-level XS selector、production allocator、聚合均价
minigrid、修正 depth TP、inventory/downside-vol scheduler。旧结果在错误语义上运行，重跑不是
重复探索。

## 15. 交付与提交

必须生成：

```text
docs/superpowers/artifacts/glm-martingale-core-round14/exploration-registry.jsonl
docs/superpowers/artifacts/glm-martingale-core-round14/r14-final-validation.json
docs/superpowers/artifacts/glm-martingale-core-round14/r1-r14-corrected-status.json
docs/superpowers/artifacts/glm-martingale-core-round14/run-manifests/r14-data-manifest.json
docs/superpowers/reports/2026-07-XX-glm-martingale-round14-search-ledger.md
docs/superpowers/reports/2026-07-XX-glm-round14-handoff-to-chatgpt.md
```

每 50 trials checkpoint，每个 phase 独立 commit + push。最终逐档回答 target、launch budget、
五档预算、symbols、concentration、WFO、neighbor、LOSO/LOCO、stress、OOS、production parity、
replay/config/cache 数量。没有命中只写精确失败范围，禁止写“马丁全部可能性已穷尽”。

提交日志必须包含“问题描述”“复现路径”或“修复思路”，例如：

```bash
git commit -m "feat: 修复思路 Round14接入completed-HTF马丁状态"
git push -u origin glm-martingale-core-round14
```
