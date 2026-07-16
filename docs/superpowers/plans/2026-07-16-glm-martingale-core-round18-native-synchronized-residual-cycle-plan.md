# GLM Martingale Core Round 18：原生同步残差 Cycle、跳跃首达与牛熊状态合同计划

执行者：GLM。审计/计划制定：ChatGPT。日期：2026-07-16。

权威输入：

- `docs/superpowers/reports/2026-07-16-chatgpt-round17-execution-audit.md`
- `docs/superpowers/artifacts/glm-martingale-core-round16/r1-r16-corrected-status.json`
- `docs/superpowers/artifacts/glm-martingale-core-round17/b2/g1-checkpoint.json`，只作历史去重输入
- 当前分支上的 Round 17 代码，只作待修基线，不信任其 complete gate

## 0. 执行原则

Round 18 不承诺必然找到目标组合。任何模型都不能诚实保证下一轮命中 50/90/110% 年化和
10/20/30% DD。本轮目标是把命中概率和信息增益集中到一个前 17 轮没有原生执行过的结构，并用硬门
禁止 GLM 再以 config 数量、call site 或短窗口年化代替进展。

唯一主线是：

```text
5+ 币原生同步 residual Martingale cycle
  -> cycle 净亏损且 residual 继续不利时同步 SO
  -> 所有腿共同 TP/reduce/abort
  -> train-only residual/regime/jump/inventory 控制
  -> event-level Binance 成交、保证金、费用、funding 和 live parity
```

若原生同步 residual cycle 的机制门失败，必须修复后继续，禁止退回 Round 17 基础 ladder Sobol。

## 1. 不可变目标

| 档位 | 年化收益 | 最大回撤 | 5 个 cold starts | advertised minimum |
|---|---:|---:|---:|---:|
| 保守 | >=50% | <=10% | >=4/5 正 | exact principal <5000U |
| 平衡 | >=90% | <=20% | >=4/5 正 | exact principal <5000U |
| 激进 | >=110% | <=30% | >=3/5 正 | exact principal <5000U |

共同硬门：

1. 任一 full/cold/WFO/robustness replay 无本金跌破、无未建模 liquidation；
2. 全期实际成交 `>=5` symbols，配置里写 5 个名字不算；
3. 任一 symbol 的 configured weight、gross profit、net PnL、peak margin、DD contribution 均不得
   `>50%`；`<=35%` 另列为优质候选，不得把 35% 偷换成用户硬门；
4. 至少 3 个 residual groups 或一个实际成交 6+ symbols 的 basket；
5. fee、maker/taker、slippage、funding、minQty、stepSize、minNotional、percent-price、maintenance
   margin、liquidation 和 partial fill 全部进入 event engine；
6. backtest/live 共用同一个 effective config、completed-bar 边界、cycle state 和 order/rejection hash；
7. exact minimum executable principal 必须从预算梯子实测，不能用 4999U 结果宣传 1000U；
8. 所有收益必须来自 Martingale cycle 的 FO/SO/TP/reduce，禁止独立 trend、carry、hedge、market
   making 或 stat-arb PnL；
9. 2026-07-11 之后的数据继续锁定，至少连续 30 天后才允许一次性 future check；此前候选最多是
   `provisional_backtest_candidate`。

## 2. “仍然是 Martingale”的机器定义

每个同步 group/basket 必须满足：

```text
cycle_open:
  residual 在 completed boundary 穿越冻结 entry threshold
  -> 同 timestamp 产生全部腿 FO intent

safety_order:
  aggregate_cycle_net_pnl_after_cost < 0
  AND residual 从上次 fill 后继续向不利方向移动 >= frozen SO step
  -> 全部存活腿按 frozen hedge ratio 同步增加 notional
  -> group aggregate notional 严格增加，受 hard reserve/ruin cap 限制

take_profit:
  aggregate_cycle_net_pnl_after_fee_funding_slippage > frozen TP floor
  AND residual 进入 frozen exit region
  -> 全部腿共同 reduce/close

abort:
  cointegration break / jump ruin / deadline / exchange rejection
  -> 只允许真实 aggregate reduce/close，损失进入 equity
```

以下任一情况使 config `not_martingale`：

- 没有发生亏损后加仓，收益只来自一次性 pair/stat-arb entry；
- 一条 hedge 腿不属于 cycle，或其 PnL 被单独加入；
- residual 信号直接生成 cycle 外的趋势/反转订单；
- SO 在 cycle 净盈利时加仓；
- 用 shadow/预计算 curve 代替真实同步 order；
- 一腿成交失败后仍把另一腿按理想对冲记账；
- 通过调高 leverage 或虚构 margin 绕开 `<5000U`。

## 3. Round 17 前置修复门 R0

R0 完成前禁止收益搜索。

### 3.1 修 validator，不再修文字

新增 fail-close tests，必须先让当前代码失败：

```text
r17_family_config_hash_only_is_not_bound
r17_family_two_traces_cannot_satisfy_eight_to_sixteen
r17_identical_ablation_event_hash_blocks_b2
r17_noop_vol_cap_is_rejected
r17_noop_cluster_scheduler_is_rejected
r17_hazard_same_open_and_now_never_counts_as_active
r17_production_zero_ohlc_bar_is_rejected
r17_checkpoint_without_command_and_hash_is_not_actual_registry_replay
```

validator 必须从 raw evidence 重算，禁止读取 `passed=true/all_bound=true/used_real_service=true` 作为
事实。`bound` 的最低定义：

```text
resolved hash changes
AND effective hash changes
AND state/admission/reserve/deadline hash changes
AND at least one real order or rejection hash changes
AND restart suffix exact
```

### 3.2 修共享 event/production 事实合同

Round 18 可以复用 R17 类型，但必须修正：

1. production router 接收每个 strategy symbol 的真实 completed Kline，不得硬编码 BTC 或 zero OHLC；
2. router 的 breadth/persistence/dwell/range displacement/SHOCK 参数必须真实进入状态变化；
3. cycle 保存真实 `opened_at_ms` 和 `last_fill_at_ms`，deadline 用时间差，不得用 legs count；
4. half-life 必须由该 fold train 或 completed pre-decision bars 估计，unstable/unit-root 进入 UNKNOWN；
5. vol cap 必须改变真实 FO/SO allowed quote 或 rejection；
6. cluster/inventory scheduler 必须读取真实 open cycle、next SO reserve 和 margin；
7. funding window 必须按 symbol/time 排序，current/incomplete observation 不可见；
8. backtest action `reduce_20pct` 必须产生真实 aggregate reduce order 和 realized loss/profit；
9. production state 必须写 SQLite，restart + exchange reconcile 后不重复 FO/SO、不丢 TP；
10. 未修的 R17 mechanism 必须 fail-closed disabled，禁止带入 Round 18 config。

只运行用于证明修复的 synthetic + 两个 adversarial real windows，不重跑 R17 192 exact configs。

### 3.3 修正 Round 17 authority

GLM 必须在旧 handoff 顶部写 `SUPERSEDED_BY_CHATGPT_AUDIT`，Round 17 machine state 改成
`materially_incomplete_invalid_mechanism_search`。原始 artifact 只追加修正引用，不覆盖历史行。

## 4. 全历史去重与 append-only ledger

### 4.1 先生成 canonical fingerprint index

扫描 Round 1-17 所有 registry、checkpoint、config 和 corrected authority，生成 append-only index。
每次 launch 前必须计算：

```json
{
  "engine_sha256": "...",
  "market_data_sha256": "...",
  "funding_data_sha256": "...",
  "cycle_topology": "independent_symbol|synchronized_pair|synchronized_basket",
  "martingale_trigger_contract_sha256": "...",
  "fit_contract_sha256": "...",
  "universe_and_group_sha256": "...",
  "resolved_config_sha256": "...",
  "effective_config_sha256": "...",
  "window": "...",
  "budget": 0,
  "fold": "...",
  "seed": 0,
  "cost_model_sha256": "..."
}
```

同 fingerprint 必须 `skipped_duplicate`，不得启动 binary。只改 label、JSON 字段顺序、未触发参数或
runner 名称都算重复。

### 4.2 每次探索必须记录

registry 每个 experiment 至少有一条 immutable `running` 和一条 terminal：

```text
hypothesis_id / parent_id / family / novelty_proof
source_to_mechanism / exact difference from R1-R17
full resolved/effective config + hashes
engine/data/plan/validator hashes
command/exit/wall/RSS/window/budget/fold/seed
FO/SO/TP/reduce/rejection counts
actual symbols/groups and all concentration metrics
fee/slippage/funding/liquidation/partial-fill metrics
ann/DD/min equity/breach/cold starts
five trace hashes + cycle-group hash
status + exact rejection/failure reason
```

terminal 状态只允许：

```text
complete / rejected_gate / invalid_mechanism / invalid_data /
timeout / skipped_duplicate / blocked_predecessor
```

禁止覆盖、删除或把失败行改名后重跑。handoff 必须输出 `new_failures_to_never_repeat`。

## 5. 与历史方向的严格差异

| Round 18 family | 与旧方向的订单级差异 | 若差异不成立 |
|---|---|---|
| M1 synchronized pair cycle | 旧 probe 只按 daily close 合成 0.5A-0.5B equity；M1 是 1m event、同步 FO/SO/TP、真实 margin/funding/legging | `duplicate_pair_probe`，停止 |
| M2 synchronized factor-residual basket | 旧 XS 是独立 sleeve 排名；M2 是一个 6+ 腿 aggregate Martingale cycle，统一亏损/SO/TP | `duplicate_xs_reversal`，停止 |
| M3 dynamic inventory reservation | Round 2 是静态 weight/cap；M3 每个 completed event 按 signed residual inventory 单调偏移下一 FO/SO threshold | `duplicate_static_inventory_cap`，停止 |
| M4 jump first-passage SO gate | 旧 H2 是固定 half-life deadline 且 R17 inert；M4 用 train-only jump/first-passage lower bound 决定继续 SO | `duplicate_fixed_deadline`，停止 |
| M5 frozen regime ladder contract | 旧 router 只 allow/block；M5 在 cycle open 冻结完整 ladder envelope，active cycle 不翻向 | `duplicate_direction_gate`，停止 |

禁止重跑审计报告第 7 节列出的 exact families。

## 6. 外部研究的允许转化

| 来源 | Round 18 只允许的转化 |
|---|---|
| `10.1080/14697680701381228` | signed inventory 只偏移 residual FO/SO reservation threshold，不建 market-making PnL |
| `10.1007/s11579-012-0087-0` | 剩余 reserve/horizon 对下一 SO 风险单调收缩 |
| `10.1080/14697688.2017.1403035` | train-only regime-switching residual state，validation 冻结 |
| `10.1080/14697688.2017.1374549` | 成本下有限 horizon 和部分调整，只控制同步 cycle |
| `10.1080/14697688.2022.2064760` | delayed residual memory，不能全样本拟合 |
| `10.1080/14697688.2020.1736613` | jump-aware first-passage threshold，只 gate SO/TP/abort |
| `10.1109/ACCESS.2020.3024619` | intraday crypto residual 候选；必须重点压测成本和参数敏感性 |
| `10.1137/16M1100861` | DD/vol 上升时 FO/SO/reserve cap 单调下降 |
| `10.1214/21-AAP1684` | regime-dependent 但预冻结的 ladder contract |
| `10.1214/23-AOS2276` + arXiv `2208.02814` | drift-aware risk calibration diagnostic，不作收益模型 |

任何论文、vendor 或 SSRN claim 都不能导入其收益数字、最优参数或事后 symbol。

## 7. M1：原生同步 Pair Martingale Cycle

### 7.1 数据和 residual fit

推广 universe 固定为：

```text
BTC ETH BNB SOL XRP DOGE ADA TRX LINK LTC BCH DOT
```

每个 WFO fold 只在 train：

1. 从 66 个 pair 中按数据完整、成交活跃、funding 完整过滤；
2. 用 train subblocks 估计 `log(A)-beta*log(B)-mu`；
3. 记录 ADF/KPSS、beta drift、half-life、jump frequency 和 residual autocorrelation；
4. 只按 stationarity/stability/liquidity 排序，不按 strategy return 排序；
5. 用确定性 maximum-weight matching 选 3/4 个不重叠 pairs，得到 6/8 symbols；
6. validation 中 beta、mu、groups、threshold fit 全部冻结。

若任一 fold 找不到至少 3 个稳定不重叠 pairs，该 fold 为 `blocked_no_stable_groups`，不得用收益挑币
补齐。

### 7.2 同步订单合同

- residual 在上一 completed 1m/5m boundary 计算，current bar 不可见；
- FO 两腿按 dollar-beta neutral quote 同 timestamp 排队；
- SO 只有在 aggregate cycle net PnL<0 且 residual 再不利时触发；
- 每层 SO 两腿同步增加，hedge ratio 在 cycle open 冻结；
- 任一腿 minNotional/lot/margin/rejection 不通过，则整层不成交并记录 `group_atomic_reject`；
- live 非原子成交必须记录 legging interval；未成交腿按 adverse fill 或整组 abort，禁止理想补齐；
- TP 按 aggregate PnL after all costs + residual exit 双门；
- 一个 pair 同时最多一个 active group cycle；
- 同 symbol 不得同时属于两个 active pair；
- 3+ pair 共享真实 portfolio reserve，existing SO 优先于新 FO。

### 7.3 M1 搜索参数

```text
bar boundary        = 1m / 5m
fit lookback        = 30 / 60 / 120 train days
entry z             = 1.0 / 1.5 / 2.0 / 2.5
SO residual step z  = 0.25 / 0.40 / 0.60 / 0.80
group FO quote      = 20 / 30 / 45 / 60U
multiplier          = 1.20 / 1.35 / 1.50 / 1.70
max legs            = 3 / 4 / 5
exit z              = 0.10 / 0.25 / 0.50
TP net bps floor    = 20 / 40 / 70 / 100
leverage            = 2 / 3 / 5
group gross cap     = 12 / 18 / 25% budget
```

`multiplier>1` 且至少允许一次 SO。若全期没有真实 SO，该 config 不算 Martingale 候选。

## 8. M2：市场因子残差 Basket Martingale Cycle

M2 不是旧 XS reversal。一个 basket 就是一个 aggregate cycle：

1. universe 固定为 6/8/12 symbols，不按全样本收益删币；
2. train-only 用 BTC factor 或 PC1，二者作为两个预声明 family，不在 validation 选择；
3. 对 factor residual 最负组做 long、最正组做 short，basket ex-ante beta 绝对值 `<=0.10`；
4. FO、SO、TP、abort 对 6+ 腿同步执行；
5. SO 必须满足 basket aggregate net loss + basket residual adverse；
6. 不允许单腿独立 TP，不允许 shadow PnL，不允许按当期赢家替换 active leg；
7. active basket 的 symbols/weights/hedge ratio 直到 cycle close 都冻结。

参数范围：

```text
factor              = BTC / train-PC1
basket symbols      = 6 / 8 / 12
residual lookback   = 24h / 72h / 168h
entry dispersion z  = 1.0 / 1.5 / 2.0
SO basket step z    = 0.30 / 0.50 / 0.75
per-leg FO quote    = 5 / 8 / 12U
multiplier          = 1.20 / 1.35 / 1.50
max legs            = 3 / 4
TP net bps floor    = 20 / 40 / 70
leverage            = 2 / 3
```

若实际订单少于 5 symbols，直接 `rejected_gate`。

## 9. M3-M5：只在父机制有效后逐项增强

禁止一开始把三个增强器笛卡尔积混在一起。

### 9.1 M3 dynamic inventory reservation

每个 completed boundary 计算：

```text
signed beta inventory
group gross/net exposure
unrealized loss
next 1/2 SO reserve + fees + liquidation buffer
remaining portfolio DD budget
```

只允许：

```text
inventory same-sign risk rises
  -> new FO entry threshold farther
  -> next SO residual step no smaller
  -> allowed SO quote no larger
```

`reservation_skew_k=0.0/0.25/0.50/1.00`。`k=0` 是 parent control。必须至少改变一个真实
FO/SO rejection/order hash，且变化不是因为静态 cap 命中。

### 9.2 M4 jump/first-passage SO gate

只用 fold train 和 decision 前 completed residual：

- 估计 jump threshold、first passage 到 TP/deadline/ruin 的 block empirical distribution；
- 按 regime/depth/age/jump bucket 计算 block-bootstrap 或 weighted conformal risk diagnostic；
- 输出 `P(TP before ruin/deadline)` lower bound；
- lower bound 低于 `0.50/0.60/0.70` 时只 freeze/reduce 下一 SO；
- 不允许该 probability 直接确定收益、方向或 leverage；
- bucket sample `<100 cycles` 时 UNKNOWN，默认不继续 SO。

这不是 R17 固定 deadline。必须证明同一个 cycle age 下，不同 jump/first-passage state 产生不同真实
SO order/rejection。

### 9.3 M5 牛/熊/震荡预冻结 ladder

只对新 cycle 选择 envelope：

```text
RANGE / fast residual MR:
  multiplier 1.35-1.70, legs 4-5, normal SO step

BULL or BEAR / slow residual MR:
  factor-neutral residual cycle only
  multiplier 1.20-1.50, legs 3-4, SO step x1.25-1.75

SHOCK / cointegration break / UNKNOWN:
  no new cycle; active cycle freeze SO or aggregate reduce
```

cycle open 后 envelope hash 冻结。regime flip 不反转、复制、换腿或重写历史 threshold。只有 SHOCK
可通过预声明 risk action freeze/reduce。

### 9.4 单项继续门

每个增强器相对同一 parent，在 train-only 4 blocks 中必须：

```text
至少 3/4 blocks Calmar 改善
median ann 保留 >=90%
worst DD 至少下降 10%，或 5/5 stability 明确改善
trade count 保留 >=60%
实际 symbols >=5
```

不通过就记录 exact failure，禁止组合。只有两个单项都通过，才允许最多 32 个组合 config。

## 10. 数据、fold 与防过拟合合同

固定 anchored folds：

```text
F1 train 2023-01-01..2023-06-30, purge 7d, validate 2023-07-08..2023-12-31
F2 train 2023-01-01..2023-12-31, purge 7d, validate 2024-01-08..2024-12-31
F3 train 2023-01-01..2024-12-31, purge 7d, validate 2025-01-08..2025-12-31
F4 train 2023-01-01..2025-12-31, purge 7d, validate 2026-01-08..2026-05-31
```

五个 cold starts 仍是 H1-2023、H2-2023、2024、2025、2026-YTD，但不得用于参数选择。每 fold：

1. pair/basket fit、group、beta、PC1、state boundary、jump threshold 仅用 train；
2. purge 后先 commit selected config/group/budget hash，再读取 validation 一次；
3. validation 失败后不得回到 train 改参数再读同一 validation；
4. 所有 16+1 轮 trial count 合并计算 DSR、CSCV/PBO、selection frequency、rank degradation；
5. report 必须明确：历史开发区已被多轮观察，真正 unseen 仍需 2026-07-11+ future lock。

## 11. 分层搜索配额

### G0：binding + adversarial activation

每 family `8-16` synthetic traces + 至少 4 个真实短窗口 replay。真实窗口必须覆盖：

```text
bull trend / bear trend / range / jump-shock
```

每个开放参数必须产生预期 state + order/rejection delta。任何 inert parameter 立即停止该 family。

### G1：nested train-only successive halving

冻结 192 unique configs：

```text
M1 synchronized pair = 96, seed 20260731
M2 residual basket    = 64, seed 20260801
M3-M5 enhancement     = 32, seed 20260802，仅父机制继续门通过后生成
```

若 enhancement 未解锁，其配额不转给 M1/M2，记录 `blocked_parent_gate`。

每 fold 内：

1. 192 configs × 3 个预声明 train stress blocks × `1000/4999U`；
2. 只按 train median return、worst DD、positive blocks、cost ratio、concentration、group stability
   选最多 32；
3. 淘汰任何 breach、DD>45%、实际 symbols<5、单 symbol>50%、两 block 负收益、atomic reject>10%；
4. 短窗口年化只作淘汰，禁止进入候选表。

### G2：full train + budget select

每 fold 最多 32 configs：

```text
full train + 5 train-only cold subblocks
budgets = 1000 / 2000 / 3000 / 4000 / 4999U
```

train strict gate：

```text
median ann >=30%
worst subblock DD <=35%
>=4/5 train subblocks positive
no breach
actual symbols >=5
concentration <=50%
```

每 fold 最多 8 个 config+budget 进入 validation。

### G3：anchored validation

selected hash commit 后，每个 validation 只运行一次。立即淘汰：

- 任意 principal breach 或 DD>45%；
- 两个 validation folds ann<0；
- 实际 symbols<5 或 concentration>50%；
- pair/basket beta drift 超阈值或 group atomic failure>10%；
- 3/4 folds budget 选择不相同或不相邻；
- 结果主要来自单一 group/symbol。

最多 4 finalists。

### G4：一次预声明 train-only 扩展

仅当 G3 至少有一个 candidate 命中第 12 节任一“推进门”但未命中最终档位，才允许：

- 围绕其 train-selected plateau 生成最多 128 个 trust-region configs；
- 参数每次只移动一个相邻离散档；
- 仍在每 fold train 内选择，validation 不能再次读取；
- 不允许增加新 symbol、换 group 或事后新增指标。

没有推进门候选时禁止盲目扩到数万 config，直接完整报告 M1-M5 的有效失败边界。

## 12. Round 18 必须报告的推进门

最终 50/90/110% 目标不变。除最终档位外，至少以下一项可称为“前沿推进”：

| 进展 ID | 要求 |
|---|---|
| P-A | `ann>62.51%`、`DD<=28.62%`、`>=4/5`、5+ symbols、event-level，严格支配现有 R7 前沿 |
| P-B | `ann>=40%`、`DD<=20%`、`>=4/5`、5+ symbols、event-level，改善低 DD 前沿 |
| P-C | `ann>=35%`、`DD<=30%`、`5/5`、5+ symbols、event-level，首次得到有效多币 5/5 |
| P-D | 任一保守/平衡/激进最终档位完整命中 |

若四项均未命中，handoff 必须写 `frontier_progress=false`，不能使用“breakthrough/接近目标/全部完成”。
有效地排除一个真正新 family 是研究进展，但不是收益前沿推进，两者必须分栏。

## 13. Finalist robustness

每个 finalist 必须完成：

1. budgets `500/750/1000/1500/2000/3000/4000/4999U` full + 5 cold starts；
2. 报 exact minimum executable principal 和 blocked/rejected leg 原因；
3. 12 个参数邻域，至少 8/12 ann 保留中心 `>=80%`，DD 不高于中心 `+3pp`；
4. leave-one-pair/group-out、LOSO、leave-one-regime-out；
5. beta/PC1 drift、cointegration break、jump-cluster 和 market decoupling stress；
6. fee/slippage `1.0x/1.5x/2.0x`、adverse funding；
7. 一腿 delay `1/2/3 bars`、partial fill `25/50/75%`、一腿 reject、cancel/replace；
8. same-bar conservative ordering、entry/SO/TP delay；
9. maintenance margin/liquidation、minNotional/rounding、funding settlement boundary；
10. 真实 started service + fake exchange + SQLite restart/reconcile；
11. backtest/live event、cycle-group、order、rejection、equity suffix hash exact；
12. DSR/PBO、selection frequency、rank degradation 和 trial count 使用 raw return series 重算。

任一 robustness breach 使 candidate 淘汰，不能降档宣传。

## 14. Production parity

必须从真实 service 入口：

```text
completed WS/catchup bars for every symbol
 -> train-frozen residual state
 -> synchronized cycle decision
 -> portfolio reserve and inventory reservation
 -> N real order intents with deterministic tie-break
 -> exchange adapter partial fills/rejections
 -> SQLite cycle/group/leg state
 -> restart + exchange reconcile
 -> next SO/TP/reduce intent
```

持久化：fit hash、group id、symbols、beta/weights、entry residual、last-fill residual、cycle opened/last
fill timestamp、depth、aggregate entry/notional/PnL、next SO、TP、deadline、regime envelope、inventory
skew、partial fills、client order ids 和 reserve。

Binance 多腿订单不是原子的。production-ready 要求 legging failure policy 已被 stress，不得把测试中的
“整组原子成交”直接当实盘事实。

## 15. 阶段状态机

```text
R0 audit/validator/shared-control repair
R1 historical fingerprint + append-only registry
R2 native synchronized cycle domain + state persistence
R3 backtest event wiring + production service parity skeleton
R4 data/fold/group-fit freeze
R5 G0 binding/adversarial activation
R6 G1 nested train screen
R7 G2 full train + budget select
R8 G3 validation + optional G4
R9 finalist robustness
R10 production parity + future lock
R11 machine handoff + failure ledger
```

predecessor 未通过，后续写 `blocked_predecessor`。0 survivor 时 robustness/parity 写
`not_applicable_zero_survivors`，不能写 complete。每 phase 由 validator 重算后独立 commit/push。

## 16. GLM 交接要求

最终 handoff 必须包含：

```text
1. corrected Round 17 status
2. R0-R11 machine state and blocked reasons
3. unique configs / actual binary launches / cache hits / duplicates / timeouts
4. M1-M5 每个 hypothesis 的 novelty proof 和 activation delta
5. 所有失败 exact fingerprint 与 never-repeat 原因
6. 每 fold train selection hash 和一次性 validation 结果
7. 三档目标命中表
8. P-A/P-B/P-C/P-D 前沿推进表
9. top 10：symbols/groups/weights/leverage/direction/min principal/ann/DD/5 cold starts
10. backtest_candidate 与 production_ready 分开
11. raw series、trades、orders、rejections、cycle groups 和 five hashes 路径
12. 2026-07-11+ future lock 状态
```

任何结果表不得遗漏币种、组、每腿权重、每腿 leverage、多空、实际成交数和 exact minimum principal。

## 17. Git 和工作区纪律

1. 从包含本计划的远端 commit 新建 Round 18 branch；
2. 不覆盖当前 `b3/g2-g3.json` 的既有未提交修改；
3. 生成文件按 phase 分类到 `docs/superpowers/artifacts/glm-martingale-core-round18/`；
4. 禁止把大日志、临时 DB、target、cache 提交；
5. 每 phase commit 后 push，commit body 必须包含：

```text
问题描述：...
复现路径：...
修复思路：...
```

6. handoff 前 `git status --short` 只能有明确列入报告的外部/用户改动；
7. 禁止 reset、checkout 或覆盖非本轮改动。

## 18. 停止条件

以下任一情况停止相应 family，不停止其他已解锁 family：

- 不是真实亏损后加仓 Martingale；
- 与历史 fingerprint 重复；
- 参数只改 config hash，不改 state/order/rejection；
- 找不到 3 个 train-stable disjoint pairs 或 6+ symbol basket；
- 多腿 partial fill/legging 无法保守建模；
- 小资金 minNotional/margin 使实际 symbols<5；
- train strict gate 0 survivor；
- 两个 validation folds 为负或任何 principal breach；
- 收益来自单一 symbol/group 超 50%；
- production restart/reconcile 不能幂等。

Round 18 结束时只能输出以下之一：

```text
TARGET_HIT_PROVISIONAL_FUTURE_LOCK
FRONTIER_PROGRESS_ONLY
VALID_NOVEL_FAMILIES_EXHAUSTED_NO_FRONTIER_PROGRESS
BLOCKED_ENGINE_OR_DATA
```

禁止在没有 P-D 时输出“已实现目标”，也禁止在没有 P-A/P-B/P-C 时输出“取得收益前沿突破”。
