# GLM Martingale Core Round 15：双向完整袖套、首达风险阶梯与相关簇库存计划（执行版 v2）

执行者：GLM

版本：`v2 / 2026-07-15`。本文件是 Round 15 唯一执行计划；不得另写简化版覆盖本计划，也不得
把未通过机器 gate 的 phase 用自然语言标记为完成。

上游唯一权威输入：

- `docs/superpowers/reports/2026-07-14-glm-round14-execution-audit-and-fix.md`
- `docs/superpowers/artifacts/glm-martingale-core-round14/r1-r14-corrected-status.json`
- `docs/superpowers/artifacts/glm-martingale-core-round14/r14-final-validation.json`
- `docs/superpowers/artifacts/glm-martingale-core-round14/r14-independent-recheck.json`

旧 Round 14 final handoff、P2-P5/P7/P8 搜索 JSON 和“2030 configs / 3488 replays”不得作为
晋级证据。它们受 BatchReplay 混入 spot/高周期 bar、XS 时钟/预热/方向池错误、惰性 active
count 和错误预算 cap 影响。P6 仅保留“ICP/TRX 两币、full-window、精确参数”的 scoped
negative evidence，不能外推到 5+ 币组合。

## 1. 唯一目标与禁止项

Martingale/DCA cycle 必须是唯一收益来源。外部指标只允许决定：

1. 哪个 symbol/direction 可以开启新的 Martingale cycle；
2. first order、safety order、spacing、TP、cooldown 和库存预留如何变化；
3. 已有 Martingale cycle 是否暂停 SO、reduce-only 或按真实聚合持仓退出；
4. 多个 Martingale sleeves 如何共享同一个 `<5000U` 账户。

禁止独立趋势仓、breakout 仓、buy-and-hold、funding carry、pair-neutral、stat-arb、shadow
PnL、预计算 curve PnL 或外部 benchmark 收益进入 equity。命中任一项，整组实验作废。

| 档位 | 年化收益 | 最大回撤 | 正收益 cold-start segments |
|---|---:|---:|---:|
| 保守 | >=50% | <=10% | >=4/5 |
| 平衡 | >=90% | <=20% | >=4/5 |
| 激进 | >=110% | <=30% | >=3/5 |

共同硬门：

- launch budget 必须是 `1000/2000/3000/4000/4999U` 中一个，且严格 `<5000U`；
- 五档预算均需可执行、无 principal breach，并报告 min equity、拒单和最大占资；
- 单一 event-level shared account，至少 5 个真实成交 symbols、至少 3 个独立 base assets；
- configured capital、realized gross profit、realized positive net PnL 的单币占比均 `<=35%`；
- fee、slippage、funding、minNotional、tickSize、stepSize、leverage、liquidation buffer 生效；
- nested WFO、邻域、LOSO/LOCO、成本/延迟压力和 production trace 全通过；
- 30 天未来 paper 前只能叫 backtest candidate，不能叫 production-ready。

### 1.1 唯一机器状态与执行顺序

GLM 启动时必须创建：

```text
docs/superpowers/artifacts/glm-martingale-core-round15/round15-execution-state.json
docs/superpowers/artifacts/glm-martingale-core-round15/exploration-registry.jsonl
scripts/glm_r15_validate_execution_state.py
```

`round15-execution-state.json` 只能由 validator 根据 artifacts、registry 和测试结果生成，至少含：

```json
{
  "plan_sha256": "...",
  "phase": "P0_CANONICAL_PARITY",
  "status": "blocked",
  "required_gates": [],
  "passed_gates": [],
  "blocked_reasons": [],
  "hashes": {"engine": "...", "data": "...", "manifest": "...", "config": "..."},
  "counts": {"unique_configs": 0, "binary_replays": 0, "cache_hits": 0, "duplicates": 0, "timeouts": 0},
  "artifacts": [],
  "validated_at": "..."
}
```

固定 phase 顺序，禁止并行跨 phase 或跳过：

| 顺序 | phase | 唯一完成条件 |
|---:|---|---|
| 0 | `P0_CANONICAL_PARITY` | canonical 数据、CLI/Batch parity、旧结果反例全部通过 |
| 1 | `P1_EVENT_PRODUCTION_WIRING` | selector/ladder/scheduler 接入 event 与 production state |
| 2 | `P2_BINDING_PROBES` | 每个开放参数改变 config hash，且至少一个指定 event hash 改变 |
| 3 | `P3_BASELINE_UNIVERSE` | 固定 baseline、T1/T2/T3 universe 和数据资格 |
| 4 | `P4_TRAIN_SCREEN` | 仅 train 的分层筛选完成，配额和重复率合规 |
| 5 | `P5_NESTED_WFO` | 4 folds 按 train 选参、冻结后各读 validation 一次 |
| 6 | `P6_ROBUSTNESS` | budget、集中度、平台、LOSO/LOCO、成本/延迟全部完成 |
| 7 | `P7_PRODUCTION_PARITY` | finalist backtest/live/restart/order trace 一致 |
| 8 | `P8_FUTURE_LOCK` | 冻结提交后至多读取一次；无新数据则记 `not_available`，不得伪造验证 |
| 9 | `P9_FINAL_HANDOFF` | validator 汇总所有命中、失败和 exact non-repeat scope |

validator 规则：缺字段、hash 不一致、前序非 `complete`、存在 `running`、gate 证据缺失或统计
无法从明细重算时，当前 phase 必须为 `blocked`。GLM 只能补证据后重新运行 validator，禁止手改
`status=complete`。每阶段都先落盘 `running`，validator 通过后单独 commit 并 push；远端提交可见后
才能进入下一阶段。`required_gates` 在 bootstrap commit 冻结；后续删除或降级 gate 必须 fail-closed。
计划 SHA256 也必须冻结；GLM 无权自行修改本计划，确需变更时先停止并交回 ChatGPT/用户审批。

## 2. Round 15 启动前必须闭合的修复

### P0.1 分支和输入冻结

从 Round 14 审计修复提交创建分支，不得从 GLM 原 final commit 直接继续：

```bash
git fetch origin
git checkout glm-martingale-core-round14
git pull --ff-only
git checkout -b glm-martingale-core-round15
```

启动 manifest 必须记录：

```text
git commit + dirty source hash
market/funding/premium DB full SHA256
futures_usdt_perp/1m canonical row hashes
portfolio_budget_replay SHA256
r14_htf_search 或其替代 batch runner SHA256
development/holdout/future-lock windows
resolved config SHA256
raw command、exit code、wall time、peak RSS
```

当前审计数据基线：

```text
market DB:  c404e4c80de4ac2a633e740f15f577239f539a29fc1c974dc6cfc05b1c29f790
funding DB: 4d77dbdeddc42f8bb800e4b784bc6eb3e77d5213212be4e8bd3226274a4e1114
premium DB: 78bd01280bbb349383e090b9576cfa543865a5680997329c33f2e05c089a2627
```

若文件或 canonical row hash 改变，先重建 manifest 和基线，禁止静默复用指标。

### P0.2 Batch/CLI 权威一致性

扩搜索前必须新增并通过：

```text
batch_preload_uses_only_futures_usdt_perp_1m
batch_rejects_spot_and_higher_timeframe_duplicates
batch_cli_20_historical_configs_event_hash_parity
batch_cli_trade_pnl_dd_funding_parity_1e_9
budget_ladder_reapplies_weight_caps_per_budget
effective_config_hash_changes_when_bound_parameter_changes
identical_effective_config_is_cache_hit_not_new_trial
old_icp_trx_xs_exact_config_fails_closed
old_3000u_50_70_result_does_not_reproduce
old_4999u_35_07_dd13_56_result_does_not_reproduce
```

每个 parity case 比较 events、trades、realized PnL、equity、DD、funding 和 rejection reasons，
不能只比较两个都调用 BatchReplay 的入口。任一不一致，P2-P5 每 family 最多运行 4 个
binding probes，不得启动 Sobol/grid。

旧结果反例必须使用原 artifact 中的精确 resolved config，不得手工近似：

```text
source A: r14-p4-dual-state-ablation-final.json / p4_baseline_xs_rev
source B: r14-p5-inventory-final.json / p5_pen1.0_floor0.25

exact XS config expected:
  fail closed: min_active_symbols=3 exceeds traded universe=2

canonical diagnostic after removing invalid XS, same P4/P5 parameters:
  A 3000U: ann 21.3373%, DD 36.3246%, positive 4/5
  A 4999U: ann 36.3790%, DD 28.6797%, positive 4/5
  B 3000U: ann 18.6454%, DD 38.0651%, positive 5/5
  B 4999U: ann 32.6729%, DD 29.4675%, positive 5/5
```

在本计划冻结的 engine/data hash 下，诊断值允许 `0.02pp` 数值误差；超差即阻塞并追查数据、
预算 cap 或 event 语义。两组结果只作 regression，不得进入 promotion pool。

### P0.3 Round 14 新机制闭合

1. XS 在 completed portfolio timestamp 上只 rebalance 一次；完整 lookback+skip 后才打分；
2. long/short 分别按真实方向 sleeve universe 排名；`active_count >= direction_count` fail-closed；
3. `min_active_symbols` 是全期真实成交门，不能用配置声明代替；
4. HTF 参数必须进入 shared config/effective config/hash，禁止始终使用 default；
5. P4 SO scale 与 spacing multiplier 都必须改变真实 order/trigger trace；
6. P5 必须接入真实 `InventoryScheduler` state，不得只用 `capital_used/budget` 公式冒充；
7. inventory-only 模式也必须更新 completed HTF/downside state；
8. 所有 runner 的 budget ladder 每档重新 resolve weight/cap。

### P0.4 搜索前的 event/production 基础接线

P2/P3 新机制必须先经过启动中的 executor、test DB writer/read、reconcile 和 restart，证明状态被
真实读取并影响 order trace。此阶段只允许最小 synthetic/binding trace，不做收益搜索。任一机制
仅存在于 helper、JSON 或离线 runner 中，`P1_EVENT_PRODUCTION_WIRING` 必须保持 `blocked`。

## 3. 外部研究只转化为可证伪机制

| 来源 | Round 15 转化 | 禁止外推 |
|---|---|---|
| Leung/Li, DOI `10.1142/S021902491550020X` | 用带成本、止损的均值回归边界估计 cycle admission、spacing、deadline | 不把论文策略收益加入账户 |
| Kitapbayev/Leung, DOI `10.1142/S0219024918500048` | cycle age/deadline 进入 SO 暂停和真实 reduce-only 状态 | 不按事后最优退出时间调参 |
| Busseti/Ryu/Boyd, DOI `10.3905/joi.2016.25.3.118` | risk-constrained fraction 只作为 FO/SO 和 safety reserve 上限 | 不使用 Kelly 收益预测或加杠杆绕过 budget |
| Cherny/Obloj, DOI `10.1007/S00780-013-0209-4` | peak/DD state 对新 cycle 风险预算施加单调约束 | 不声称理论 drawdown 保证适用于本回测 |
| Giudici et al., DOI `10.3389/FRAI.2020.00022` | 仅用 train 内 lagged correlation network 冻结 clusters | 不按全样本收益选择币种/cluster |
| Leung/Li, DOI `10.1007/S10690-016-9215-9` | long/short 方向都用相同的 Martingale cycle contract | 不增加独立期货方向仓 |
| Bailey et al., DOI `10.21314/JCF.2016.322` | 保存所有 trials，计算真实 CSCV/PBO | 不用简化“top-5 proxy”冒充 PBO |
| Bailey/Lopez de Prado, DOI `10.3905/JPM.2014.40.5.094` | 用完整 return series、偏度、峰度和 trial count 算 DSR | 不用 ann 均值/标准差 proxy 冒充 DSR |

文献只定义机制和检验，不保证达到收益目标。

## 4. P1：方向完整的双向 Martingale sleeve universe

Round 14 的 R4/R7 每方向只有 3 个 symbols，active=3/5 实际无筛选或筛到不存在的方向
sleeve。Round 15 必须先构造方向完整 universe：

- 在任何收益搜索前冻结 symbols，并先验证数据完整、期货流动性、minNotional 和上市时长；
- 每个 symbol 同时有 long 与 short Martingale sleeve，但同一 symbol 同时最多一个 live cycle；
- direction flip 只影响下一 cycle，不能反转、复制或强平旧 cycle；
- selector active count：每方向 `2/3/4/5`，必须 `< direction universe size`；
- 全期至少 5 symbols 真正成交，不是依赖 symbol 或配置中出现 5 个名字；
- 每个 cluster 至少保留 2 个候选，避免 selector 退化为单币轮动。

独立 family，禁止看结果后混 horizon：

```text
D1 completed-HTF direction:
  timeframe = 1h / 4h
  score = 6/12/24-bar return sign + EMA slope
  action = allow only matching-direction NEW Martingale cycle

D2 completed mean-reversion admission:
  displacement = log(price / EMA) / lagged realized vol
  windows = 24h / 72h / 168h
  entry boundary = 0.5 / 1.0 / 1.5 sigma
  action = open opposite-direction Martingale cycle only after boundary close

D3 variance-ratio state:
  q = 4 / 8 / 16
  action = choose D1 or D2 admission contract; never place an independent order
```

先做 16 个 binding probes/family。每个变动参数必须改变 effective config hash，并在至少一个
synthetic trace 中改变 `cycle_admitted` 事件；否则记 `parameter_inert`，停止该 family。

### P1.1 三条预声明主线

Round 15 只允许下列三条 track。不得看到 validation 后新增币种、改名复制 track 或混用最优参数：

```text
T1 R7 frontier rescue（仅诊断，不可晋级）
  baseline = corrected R7 exact config
  只开放 admission、existing-cycle reserve、cluster cap
  冻结 R7 FO/SO/multiplier/legs/spacing/TP/cooldown
  目标 = 诊断能否保留约 62% ann，同时降低 DD 和集中度
  限制 = R7 universe 含 survivor-picked ANKR，因此结果永远不得进入 promotion pool

T2 primary promotion universe（主要晋级路线）
  symbols = BTC ETH BNB SOL XRP DOGE ADA TRX LINK LTC BCH DOT
  每币同时建立 long/short Martingale sleeve
  universe、方向合同、fold 和预算选择规则在搜索前冻结
  任一币数据不合格则 T2 blocked；不得按收益替换币种

T3 small-cap shallow ladder（小资金可执行路线）
  universe-8  = BTC ETH BNB SOL XRP DOGE ADA TRX
  universe-12 = 与 T2 相同
  first_order = 10 / 15 / 25U
  multiplier  = 1.3 / 1.6 / 2.0
  max_legs    = 4 / 6 / 8
  spacing_bps = 80 / 120 / 180 / 250
  tp_bps      = 80 / 120 / 180 / 250
  只用 seeded Sobol；禁止完整笛卡尔积
```

T2 是资源优先级最高的路线；T1 最多占 Round 15 binary replay 总量的 `10%`，T3 最多 `35%`，
其余资源给 T2。T1 即使命中收益/DD，也只能证明机制，不算目标命中。

## 5. P2：首达概率/半衰期驱动的 Martingale ladder

目的：仍以亏损加仓和 cycle TP 获利，但不在回归概率低、首达时间过长时机械加码。

所有估计只用 decision 前 completed bars：

```text
state inputs:
  realized_vol_24h/72h
  downside_semivariance_72h
  displacement_z
  OU/AR(1) half_life bucket (clamped; unstable/unit-root => UNKNOWN)
  cycle_age and current depth

outputs:
  next_spacing_mult
  next_so_scale
  cycle_deadline
  required_safety_reserve
```

三组独立机制：

```text
H1 volatility-normalized spacing:
  spacing = max(exchange_floor, k * lagged_vol)
  k = 0.50 / 0.75 / 1.00 / 1.25

H2 half-life/deadline ladder:
  half_life buckets = <=12h / 12-48h / 48-168h / UNKNOWN
  deadline = 2 / 3 / 5 estimated half-lives, capped at 24/72/168h
  after deadline = freeze SO or 10/20/30% true aggregate reduce-only

H3 risk-constrained reserve:
  risk fraction = 0.20 / 0.35 / 0.50 of remaining free capital
  reserve must cover all existing cycles' next 1/2 safety legs + liquidation buffer
  new FO allowed only from residual free capital
```

必须满足单调性：vol/downside risk/age/depth 增加时，风险上限不得上升。SO size 仍可随深度
Martingale 增长，但总量受 reserve cap 限制；若 cap 不足则拒绝新 cycle/下一 SO，不能缩小
账面成本后仍按原 notional 计收益。

测试至少包括：

```text
lagged_state_cannot_use_current_bar
unit_root_or_unstable_half_life_fails_to_unknown
adverse_state_widens_real_next_trigger
deadline_reduce_uses_aggregate_average_entry
reserve_includes_existing_cycles_before_new_fo
reserve_cap_binds_margin_notional_fee_and_event
restart_restores_half_life_deadline_and_reserve
```

## 6. P3：相关簇与库存 admission scheduler

只在 train 窗口用 lagged 1h returns 建 network/cluster；每个 WFO fold 独立冻结，validation
不得重聚类。比较三个预先声明 family：

```text
C0 no cluster scheduler (control)
C1 absolute-correlation threshold: 0.55 / 0.70 / 0.85
C2 minimum-spanning-tree communities: target clusters 3 / 4 / 5
```

Scheduler 必须接入 event engine 和 production state：

- symbol inventory cap `20/25/30%` budget；
- cluster live margin cap `30/35/40%` budget；
- cluster downside contribution cap `35/45%`；
- 同一 completed minute 每 cluster 最多 admission `1/2` 个新 cycles；
- existing cycles 的 safety reserve 优先于任何新 FO；
- 排名 tie 按 frozen symbol id，不能受 strategy iteration order 影响；
- inactive sleeve 继续管理已有 cycle；
- 每个 symbol 的 configured、gross profit、positive net PnL 集中度均输出。

若 scheduler helper 与 event trace/DB state 不一致，整个 P3 未执行。

## 7. P4：有限、分层搜索协议

禁止再次先跑数千个配置再检查绑定，也禁止 `512/fold` 无门槛扩搜。严格执行以下 successive
gates；某 gate 没有 survivor 就终止该 family：

| gate | 输入上限 | 执行内容 | 输出上限 |
|---|---:|---|---:|
| G0 binding | `4-16/family` | synthetic trace；每个开放参数必须改变 effective config 和指定 event hash | 通过/停止 |
| G1 cheap stress | Round 15 全局 `128` seeded Sobol | 仅 fold-train；4 个固定时间块 × `1000/3000/4999U`；真实 binary replay | 全局 `24` Pareto configs |
| G2 full development | 全局 `24` | 完整 train、5 cold-start train segments、账户约束和集中度 | 每 fold `8` |
| G3 nested validation | 每 fold `8` | train 内选择并冻结；validation 每 config 只读一次 | 全局 `3` finalists |
| G4 robustness | `3` total | 五预算、参数平台、LOSO/LOCO、成本/延迟、production parity | 命中或零命中 |

G1 的 4 个时间块由每个 fold 的 train 窗口按时间等分后固定生成，不得按行情标签或结果挑选。
同一 `effective config + engine + data + window + budget` 只运行一次；重复 effective config 只计
cache hit，不占输入/输出配额。128 个 Sobol 名额固定为 `T1=12 / T2=72 / T3-8=22 /
T3-12=22`；四条独立 sampler 的 scramble seed 依次为 `20260715/20260716/20260717/20260718`，
并记录 sampler 实现与版本 hash。不能因中间收益改配额。任一 family 的 duplicate rate `>25%`
时停止并修复 sampler。

G1 只决定淘汰，不能被称为有效绩效；G2/G3 不得用 G1 的短窗口年化冒充 full-window 年化。
没有 family 达到 G2 train median ann `>=30%`、worst DD `<=35%`、至少 3/4 train blocks 正收益，
不得进入 G3。任一档目标候选出现后停止同 family 扩搜，直接转 G4，禁止继续寻找更漂亮数字。

优化器只能看：

```text
train ann, train DD, train Calmar,
worst train subperiod, concentration penalty,
principal breach, trade count and capital utilization
```

不得使用 opened holdout、future lock、paper 或 validation winner 反向修改范围。

## 8. P5：防过拟合与晋级合同

Anchored nested WFO：

```text
F1 train H1-2023             purge 7d -> validate H2-2023
F2 train 2023                purge 7d -> validate 2024
F3 train 2023-2024           purge 7d -> validate 2025
F4 train 2023-2025           purge 7d -> validate 2026-01-01..2026-05-31
```

每 fold 从头估计 selector state、half-life 和 clusters。必须保存所有 train trials 和所有被选
validation traces，计算真实 CSCV/PBO、DSR、selection frequency 和 rank degradation。

launch budget 是 train-only hyperparameter，而不是看完结果后的展示选项：

- 每 fold 只能用 train 从 `1000/2000/3000/4000/4999U` 中选择 launch budget；
- 写入 resolved config/hash 后才能读取该 fold validation；
- validation 前不得知道其他预算的 validation 指标；
- 4 folds 中至少 3 folds 选择相同或相邻预算，否则候选判为 budget-unstable；
- 禁止看 validation 后改 launch budget、权重 cap 或以另一预算重新命名候选。

候选还必须通过：

- 参数平台：预先生成 12 个最近邻 effective configs；至少 `8/12` 的 ann 保留中心点 `>=80%`，
  且 DD 不高于中心点 `+3pp`，否则是孤立最优点并判 overfit；
- 最近邻由连续参数 `±10%/±20%` 和离散参数相邻档构成，必须改变 event hash；
- LOSO：每次删除 1 symbol；LOCO：每次删除 1 cluster；
- 成本：fee/slippage `1.0x/1.5x/2.0x`；funding adverse stress；
- 延迟：entry/SO/TP decision 延迟 `1/2` completed bars；
- same-bar 保守成交顺序；
- budget `1000/2000/3000/4000/4999U` 全阶梯；
- min equity、principal breach、liquidation buffer；
- 三种集中度均 `<=35%`；
- 每档所需 positive segments。

已打开的 2026-06-01..2026-07-10 holdout 只作诊断，不能称 untouched OOS。若数据库具备
`2026-07-11` 之后新数据，参数冻结并提交 hash 后才允许一次读取；否则等待未来数据。任何读取
都要登记，禁止反复调参。

## 9. P6：Production parity

只有先通过 P5 的 candidate 才做 production promotion，但基础接线测试必须先完成：

```text
started_executor_applies_selector_htf_ladder_scheduler
db_writer_persists_completed_boundary_state
reconcile_does_not_duplicate_cycle_or_so
restart_restores_selector_half_life_deadline_cluster_reserve
backtest_live_order_trace_matches
inactive_cycle_continues_so_tp_sl
same_timestamp_strategy_order_invariant
exchange_rounding_min_notional_parity
```

禁止恒真断言、两个相同 helper state 互比或只证明 JSON 可序列化。测试必须经过 production
executor、真实 test DB writer/read、reconcile、restart 和 order submission trace。

## 10. 结果落盘、提交和去重合同

每次尝试开始前追加 `running`，结束后追加终态：

```text
experiment_id, family, hypothesis, parent_config_hash,
resolved_config_hash, effective_config_json/hash,
engine/data/manifest/code hashes,
train/validation window, budget, seed, optimizer,
raw command, exit code, actual binary replays, cache hits,
wall time, peak RSS, full/segment/budget metrics,
three concentration metrics, rejection reasons,
event/metric/artifact hashes, exact_non_repeat_scope, status
```

合法状态：

```text
running / complete / rejected / parameter_inert / invalid_engine /
invalid_data / timeout / skipped_duplicate
```

失败也必须保存。下一轮只跳过相同 `effective_config + engine + canonical data + window`；引擎
语义已变化的旧无效实验不能被错误登记为“机制穷尽”。

每个 phase 的 Git 纪律：

1. 开始前写入 registry/state 的 `running` 记录；
2. 运行结束保存 raw artifacts、validator 输出和终态，不得只保存汇总；
3. 一个 phase 一个独立 commit，并 push 到 `origin/glm-martingale-core-round15`；
4. commit log 必须同时包含 `问题描述`、`复现路径`、`修复思路`；
5. `git status --short` 非空、push 失败或远端 commit 不一致时，不得进入下一 phase；
6. 禁止写“用户接受”“结构硬下限”“全部穷尽”或“production-ready”，除非对应机器 gate 支持。

## 11. Round 15 明确不重复范围

| key | 不重复范围 | 仍允许探索 |
|---|---|---|
| `r14-audit-r4-baseline` | 修正 engine/data 下精确 R4 full/segments/budgets | 新双向 universe 和 scheduler |
| `r14-audit-r7-baseline` | 修正 engine/data 下精确 R7 full/segments/budgets | 非 ANKR survivor-picked universe |
| `r14-audit-r4-xs-rev72-a2` | 修正 clock/warmup/direction pool 后的 6 币 REV72 active2 | 方向完整 8/12 币 families |
| `r14-p6-two-symbol-minigrid-depth` | ICP/TRX 两币 exact 544+800 full-window configs | 5+ 币、shared inventory、完整验证 |
| `r14-invalid-batch-searches` | 不可作为成功/失败证据，仅记录 invalid hashes | 修正引擎后按本计划有限重跑 |

## 12. 停止规则与交接

立即停止 family：

- 参数未绑定或有效配置重复率 `>25%`；
- binding probe 无 event trace 差异；
- 两个 WFO validation folds 年化为负；
- worst validation DD `>45%` 或任一 principal breach；
- 实际成交 symbols `<5` 或 concentration `>50%`；
- 非 Martingale PnL 混入；
- CLI/Batch/production trace 不一致。

最终 handoff 必须明确：

1. 每档命中列表或空数组；
2. launch budget 与五档预算完整结果；
3. 5 个 cold starts、4 个 nested WFO folds、future-lock 状态；
4. 三种 concentration、LOSO/LOCO、成本/延迟压力；
5. actual unique configs、actual binary replays、cache hits、invalid/timeouts；
6. engine/data/config/event hashes；
7. `backtest_candidate` 与 `production_ready` 分开；
8. 所有失败的 exact non-repeat scope。

handoff 的计数必须由 registry 明细重算并与 `round15-execution-state.json` 完全一致；声明的
`unique configs`、`binary replays`、`duplicates`、`timeouts` 任一无法重建，P9 保持 blocked。

收益目标是筛选门，不是承诺。没有一个候选同时通过全部门，就必须输出零命中，不能降低目标、
接受两币结果或用更小本金导致的高 DD 年化冒充成功。
