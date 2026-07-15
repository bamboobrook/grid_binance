# GLM Martingale Core Round 17：共享 Router、Hazard、Crowding 与 Cluster 实盘闭环计划

执行者：GLM。审计/计划制定：ChatGPT。日期：2026-07-15。

权威输入：

- `docs/superpowers/artifacts/glm-martingale-core-round16/r1-r16-corrected-status.json`
- `docs/superpowers/artifacts/glm-martingale-core-round16/round16-independent-recheck.json`
- `docs/superpowers/reports/2026-07-15-chatgpt-round16-execution-audit-and-fix.md`

## 0. 不可变目标

唯一收益引擎是 Martingale：FO 开 cycle，价格不利后 SO 加仓，TP/reduce-only 退出。趋势、breadth、
funding、volatility、change point、half-life、cluster 只能控制 Martingale admission、SO、deadline 和
资金上限，禁止产生独立趋势仓、carry 仓、对冲仓或网格外 PnL。

| 档位 | 年化 | 最大回撤 | cold-start 正收益 | 预算 |
|---|---:|---:|---:|---:|
| 保守 | >=50% | <=10% | >=4/5 | advertised minimum <5000U |
| 平衡 | >=90% | <=20% | >=4/5 | advertised minimum <5000U |
| 激进 | >=110% | <=30% | >=3/5 | advertised minimum <5000U |

共同硬门：无本金跌破；实际成交 >=5 symbols；任一 symbol 正收益/保证金/风险贡献不得 >50%；真实
fee、slippage、funding、minNotional、rounding、liquidation；backtest/live 同配置、同 completed-bar
边界、同 event/order/rejection hash。

“小资金可运行”必须写明 exact minimum executable principal。不能用 4999U 结果宣传 1000U；低于
advertised minimum 的 stress 可以失败，但候选必须至少在一个 `<5000U` 冻结预算通过全部目标门。

## 1. Round 16 禁止重复范围

禁止再次运行：

1. `configs/g1` 的 128 个 exact 默认 HTF config × 原四窗口 × 三预算；
2. `configs/g2` 的 8 个 exact config × full/五 segments @4999U；
3. Round 12 ATR spacing、已失败的 native minigrid/depth-TP exact family；
4. Round 13/14 已审计的 curve allocator、旧 XS reversal、固定默认 HTF exact family；
5. Round 15 T3-8 long-only partial-TP exact family；
6. 任何只改 JSON label、helper 或 test，未改变真实 order/rejection trace 的参数。

允许重测的唯一条件：Round 17 shared mechanism 已进入 effective config、真实 backtest event loop 和
started production service，且 engine/config hash 与上述 exact family 不同。

## 2. 外部研究转化边界

以下资料只定义机制假设，不证明 crypto 或本策略收益：

| 来源 | Round 17 只允许的转化 |
|---|---|
| Moskowitz/Ooi/Pedersen, `10.1016/j.jfineco.2011.11.003` | completed multi-horizon trend 只作 long/short cycle direction gate |
| Moreira/Muir, `10.1111/jofi.12513` | realized vol 上升时 FO/SO/risk cap 单调下降，不放大预测杠杆 |
| Barroso/Santa-Clara, `10.1016/j.jfineco.2014.11.010` | momentum crash/高波动时暂停新 cycle，不复制论文收益 |
| Adams/MacKay, arXiv `0710.3742` | change-point 思想转为可解释 CUSUM/SHOCK cooldown；不在 validation 拟合复杂 BOCPD |
| Fan et al., SSRN `4361410` | funding 仅识别拥挤并 veto 不利方向，不建立 carry trade |
| Bailey/Lopez de Prado, `10.3905/jpm.2014.40.5.094` | DSR/selection bias 诊断；不得把 in-sample Sharpe 当目标命中 |
| Busseti/Ryu/Boyd, `10.3905/JOI.2016.25.3.118` | risk-constrained FO/SO/reserve 上限，不用 Kelly 放大收益 |

DOI 元数据已于 2026-07-15 通过 Crossref 核对。若 funding、premium 或 completed breadth 数据在
manifest 中不完整，相关 family 必须 `blocked_missing_data`，不能用近似 future 数据替代。

## 3. 执行顺序与状态机

```text
A0 bootstrap/audit freeze
A1 shared config + one event state machine
A2 real backtest wiring
A3 real production service wiring + persistence/reconcile
A4 10-family binding
B0 data/split/search contract freeze
B1 mechanism ablation
B2 G1 train-only staged search
B3 G2 anchored nested WFO
B4 finalist robustness
B5 production parity
B6 future lock
B7 machine handoff
```

任一 predecessor 未通过，后续 phase 写 `blocked`，不得写 complete。0 survivor 时 G3/B4/B5 写
`not_applicable_zero_survivors`，不得写 passed。每 phase 独立 commit/push；commit body必须包含：

```text
问题描述：...
复现路径：...
修复思路：...
```

## 4. A0：先修 authority，不得直接回测

1. 从本计划 commit 创建 Round 17 分支，不从 GLM Round 16 原 handoff 继续。
2. 冻结 plan、validator、release binary、market/funding/premium DB SHA256 和 git commit。
3. validator 先加载 `r1-r16-corrected-status.json`，拒绝 Round 16 的 superseded handoff。
4. registry 每次 launch 前写 running，结束后追加 terminal；原始行只追加，不覆盖。
5. terminal 必须含 experiment/family/track、完整 resolved/effective config JSON+SHA256、engine/data/
   plan/validator hash、window/budget/seed/fold、完整 command、exit code、wall/RSS、ann/DD/min equity/
   breach、symbols、三种 concentration、fee/funding/rejection 和五类 trace hash。
6. duplicate key 在启动前 cache-hit；若实际启动，必须同时计 actual replay 和 duplicate launch。

A0 validator 必须有负测试：缺 1 replay、错 1 family、伪 `used_real_service=true`、exit code 0、G2
缺 window、篡改 plan hash、source 反例失败、DB/CLI 缺失、实际 config 少于声明数量，均按预期
fail-close；任何 test 禁止用静默 `return` 表示跳过。

## 5. A1：只保留一套共享控制合同

复用现有 `MartingaleRiskLimits`、`HtfRegimeComputer`、inventory/capital scheduler 与 persistence
设施；禁止再建一套只供 tests/search 的平行 router。shared config 至少表达并参与 hash：

```text
router:
  trend_horizon_4h       = 12 / 24 / 48
  breadth_threshold      = 0.60 / 0.70
  enter_persistence      = 2 / 3 / 6 completed 4h bars
  exit_persistence       = 1 / 2 / 3 completed 4h bars
  minimum_dwell_hours    = 12 / 24 / 48
  range_displacement_z   = 0.75 / 1.25 / 1.75
  shock_downside_q       = train 90 / 95 percentile
  cusum_sigma            = 4 / 6
  shock_cooldown_hours   = 12 / 24 / 48
funding_crowding:
  completed_window       = 30 / 90 observations
  adverse_z_veto         = 2 / 3
hazard:
  half_life_window_h     = 72 / 168 / 336
  deadline_half_lives    = 2 / 3 / 5
  deadline_cap_h         = 24 / 72 / 168
  after_deadline         = freeze_so / reduce_20pct
  reserve_next_legs      = 1 / 2
cluster:
  mode                   = none / abs_corr / mst
  abs_corr_threshold     = 0.65 / 0.80
  mst_target_clusters    = 3 / 4 / 5
  symbol_margin_cap_pct  = 20 / 25 / 30
  cluster_margin_cap_pct = 30 / 35 / 40
vol_cap:
  completed_window_h     = 24 / 72
  risk_fraction          = 0.20 / 0.35 / 0.50
```

单调性硬约束：vol、downside、cycle age、depth、funding crowding 任一上升时，允许的 admission/SO
risk cap 不得上升。short ladder 必须 `mult<=long`、`legs<=long`、`spacing>=long`、
`deadline<=long`、aggregate notional `<=40% budget`。

## 6. A2：真实 backtest event wiring

在 canonical `BatchReplay/portfolio_budget_replay` 的同一 event loop 中：

1. 只在 completed 1m/1h/4h boundary 更新状态；current bar 不可见；
2. market breadth 使用冻结 T2 universe 中有完整数据的 symbols，缺失成员和分母写 trace；
3. BULL 只允许新 long，BEAR 只允许浅 short，RANGE 仅在 displacement 达阈值时允许反向 cycle，
   SHOCK/UNKNOWN 全部阻止新 cycle；
4. funding z 只 veto 拥挤方向；不得加方向收益；
5. change point 只进入 SHOCK/cooldown；
6. 已有 cycle 不因 regime flip 被反转/复制/平移；TP 继续，SO 受 deadline/reserve 控制；
7. 同 symbol 同时最多一个 live direction；
8. scheduler 先为已有 cycle 的 next SO+fees+liquidation buffer 留 reserve，再处理新 FO；
9. 同 timestamp 按冻结 symbol id tie-break；策略配置顺序变化不得改变 trace；
10. 每次状态、admission、SO freeze/reduce、reserve、cluster rejection 都进入 event/rejection hash。

输出 train-only first-passage diagnostic：按 state/displacement/depth/half-life bucket 统计 deadline 前
TP 到达率及 Wilson 95% lower bound。该统计只能 gate/freeze，不得直接记收益。

## 7. A3：真实 production service wiring

必须从真实 started executor/main service 入口测试，不能直接调用 runtime helper 代替：

```text
completed websocket/catchup bar
 -> shared router/hazard/scheduler state
 -> new-cycle admission or rejection
 -> real order submission adapter
 -> SQLite event/state writer
 -> restart load
 -> exchange reconcile
 -> next event/order
```

落盘并恢复：completed boundaries、router state/persistence/dwell、active direction、cycle id/age、平均
成本、next SO、half-life/deadline/freeze、funding window、CUSUM、cluster assignment/exposure/reserve、
last admission 和 tie-break。restart/reconcile 不得重复 FO/SO，不能丢失已有 TP。

源码静态门必须在 `apps/trading-engine/src` 找到这些方法从 main/executor 的调用点；测试文件、方法定义
和 evidence JSON 布尔值不算调用点。

## 8. A4：10-family 参数绑定硬门

必须分别测试：

```text
D1 trend + breadth
D2 RANGE displacement
D3 SHOCK + CUSUM cooldown
F1 funding crowding veto
H1 half-life estimation
H2 deadline action
H3 reserve/risk fraction
C1 abs-correlation cluster
C2 MST cluster
V1 volatility monotone cap
```

每 family 8-16 synthetic traces，并至少 2 次真实短窗口 replay。每个开放参数必须同时改变：

```text
resolved config hash
effective config hash
state/admission/deadline/reserve hash
至少一个真实 order 或 rejection trace
restart 后相同 trace suffix
```

参数 inert、只变 label、只改 JSON、只改 helper、restart 不一致时立即停止该 family。A4 未全部通过，
禁止进入 B1。

## 9. B0：冻结数据、universe 和 anti-overfit 合同

推广 universe：

```text
T2:   BTC ETH BNB SOL XRP DOGE ADA TRX LINK LTC BCH DOT
T3-8: BTC ETH BNB SOL XRP DOGE ADA TRX
T3-12:same as T2
T1 diagnostic: 只做旧回归，永不晋级
```

固定 anchored folds：

```text
F1 train H1-2023       purge 7d -> validate H2-2023
F2 train 2023          purge 7d -> validate 2024
F3 train 2023-2024     purge 7d -> validate 2025
F4 train 2023-2025     purge 7d -> validate 2026-01-01..2026-05-31
```

`2026-06-01..2026-07-10` 只作已打开诊断，不参与选择。`2026-07-11+` 在连续 30 天前锁定，最早
2026-08-10 后才能一次性读取。每 fold 的 breadth threshold、funding z 基线、shock percentile、
half-life bucket 和 clusters 只能用该 fold train 拟合，validation 冻结。

## 10. B1：先做机制 ablation

使用同一低风险 ladder center 和 T3-8/T2，各机制只开/关一次：

```text
baseline default HTF（仅回归引用，不重复计搜索）
+ breadth
+ funding veto
+ CUSUM shock
+ deadline
+ reserve/cluster
all combined
```

只看 train blocks。每项必须报告 admission 数、SO 数、cycle duration、fee/funding、DD、被拒原因和
event delta。机制若不改变订单、使所有窗口交易数接近零，或只靠少于 5 symbols，立即淘汰。

## 11. B2：G1 staged search

生成 192 个 unique scrambled Sobol effective configs，固定 seed：

```text
T2=96 seed 20260721
T3-8=64 seed 20260722
T3-12=32 seed 20260723
```

基础 ladder 只用保守范围，避免重复 Round 16 广调：long FO `10/15/20`、mult `1.25/1.40/1.55`、
legs `4/5`、spacing `120/180/250bps`、TP `120/180/250bps`、leverage `3/5`；short 按 A1 硬约束。
主要 Sobol 维度必须是 A4 已绑定的 router/hazard/funding/cluster 参数。

四个预声明 train-only 30 天窗口 × `1000/3000/4999U`，理论 `2304` unique binary replays，必须
精确配额，不能容忍 `required-50`。G1 只淘汰：任何 breach、DD>45%、实际 symbols<5、集中度>50%、
两窗口负收益或 duplicate rate>25% 立即停止 exact family。禁止发布短窗口年化为候选收益。

Pareto 只能按 train median return、worst DD、positive-window count、concentration 和 turnover 选择，
最多 24 configs；不能取单一窗口最高 ann。

## 12. B3：G2 anchored nested WFO

每 fold：

1. 最多 24 configs × `1000/3000/4999U` 只运行 train；
2. train 内重新估计状态边界/half-life/clusters，选择一个 config+advertised minimum budget；
3. 选择和 hash commit 后，最多 8 configs 各读取 validation 一次；
4. train strict gate：median ann>=30%、worst block DD<=35%、>=3/4 positive、无 breach；
5. 两个 validation folds ann<0、任一 DD>45%、breach、symbols<5 或 concentration>50% 立即停止 family；
6. 至少 3/4 folds 选择相同或相邻 budget，否则 `budget_unstable`；
7. 最多 3 finalists。

保存完整 return series，计算真实 CSCV/PBO、DSR、selection frequency、rank degradation；禁止用 top-5
mean 或自报 `strict_checked=true` 代替 raw recompute。

## 13. B4：finalist robustness

每 finalist 必须完成：

- `1000/2000/3000/4000/4999U` full + 5 cold starts，并写 exact minimum executable principal；
- 12 个参数邻域，至少 8/12 ann 保留中心 >=80%，DD<=center+3pp；
- LOSO 每次删 1 symbol、LOCO 每次删 1 frozen cluster；
- fee/slippage `1.0x/1.5x/2.0x`、adverse funding；
- entry/SO/TP delay 1/2 completed bars、same-bar conservative order；
- 每个 cold boundary start/restart parity；
- min equity、liquidation distance、blocked legs、rejections、三种 concentration。

只有一个 `<5000U` 冻结预算同时达到对应档位全部目标，才能列入该档命中。研究前沿不能叫候选。

## 14. B5-B7：production、future 与交接

B5 对每 finalist 启动真实 service，比较 backtest/live event、order、rejection、funding、equity hash；覆盖
断线补 bar、restart、reconcile、duplicate websocket event、same timestamp reorder、minNotional/rounding。

B6 按 B0 future lock；未到日期写 `waiting_future_data`。B7 handoff 必须由 validator 从 raw registry
重算：unique/actual/duplicate/timeout、每 phase 精确状态、三档命中、每 finalist 全门、所有失败 exact
scope、`research_frontier/backtest_candidate/production_ready` 三类分开。

最终结论允许是零命中，但必须写成“这些 exact family 已失败”；禁止宣称穷尽 Martingale 全部可能。

## 15. 运行效率

当前 canonical event engine 是分支密集、状态依赖的 CPU replay。Round 17 禁止为使用 RTX 5090 先重写
GPU engine；那会扩大 parity 风险。优先：一次预载市场数据、只读共享 bars、进程级并行、按 phase
checkpoint、cache exact key、先 binding/ablation 后长窗口。只有 CPU profiler 证明指标矩阵占主要耗时，
且 GPU/CPU trace 逐事件一致，才可另开 GPU 优化任务；它不能阻塞本轮目标搜索。
