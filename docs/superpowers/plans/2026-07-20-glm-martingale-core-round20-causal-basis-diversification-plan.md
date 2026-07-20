# GLM Martingale Core Round 20：Causal 分散修复、Spot-Perp Basis 与 Soft-SEL 计划

制定日期：2026-07-20。

前置唯一权威：

- `docs/superpowers/artifacts/glm-martingale-core-round19/round19-corrected-authority.json`
- `docs/superpowers/reports/2026-07-20-chatgpt-round19-execution-audit-and-correction.md`
- `docs/superpowers/artifacts/glm-martingale-core-round19/audit/round19-corrected-failure-ledger.jsonl`

本文件是 Round 20 唯一执行任务书。原 Round 19 handoff 已 superseded，不得把其 phase complete、G1/G2
survivor 或 `VALID_SEARCH_NO_FRONTIER_PROGRESS` 继承为有效状态。

## 0. 本轮目的和不可妥协原则

Round 20 不是继续盲调普通 Martingale spacing/multiplier。它必须先修复 Round 19 的执行有效性，再围绕
两个有证据的新缺口搜索：

1. `M1R_F3_028/044` 暴露的 event-level symbol/group contribution concentration；
2. 从未原生执行过的多币 spot-perpetual basis loss-after-add Martingale。

同时必须补齐 Round 19 未执行的 P1/K1/V1，并按原机器定义修复 M2F。原则：

1. **收益始终来自 Martingale cycle**：FO 后 aggregate 净亏损且 residual/basis 继续 adverse，才允许
   multiplier `>1` 的真实 SO。指标、模型、funding、basis、break state 只能决定方向、FO/SO/TP/abort，
   不能产生独立 sleeve PnL。
2. **先有效后搜索**：P0-P3 任一 hard gate 未过，禁止启动 G1。缺实现时整轮状态为
   `BLOCKED_ENGINE_DATA_OR_EXECUTION`，不得再写 `blocked_implementation_scope` 后继续宣告完整。
3. **严格 causal**：每个 fit 必须在对应 replay 第一决策时间之前结束。禁止 outer-train-end fit 回放早期
   inner block；禁止读取 future/validation 后回 train 调参。
4. **小资金和实盘先行**：所有搜索从一开始就使用 exchange filters、maintenance/liquidation、shared cash、
   partial fill/reject/legging 和全成本；不得 finalist 后补。
5. **每次探索可追溯**：只允许一个中央 append-only registry。启动前 `running`，结束后 terminal；没有
   command/hash/trace 的回放视为从未执行。
6. **不承诺收益**：公开案例只能形成假设。没有一次性 OOS、robustness 和 production parity 前，禁止
   使用“已找到”“可实盘”或“目标命中”。

## 1. 不变目标和共同 hard gates

### 1.1 三档目标

| 档位 | portfolio ann | portfolio max equity DD | cold starts |
|---|---:|---:|---:|
| 保守 | >=50% | <=10% | >=4/5 正 |
| 平衡 | >=90% | <=20% | >=4/5 正 |
| 激进 | >=110% | <=30% | >=3/5 正 |

必须另报 `5/5` 全正。balance DD 不能替代 floating equity DD；同时输出：

```text
max_equity_dd / max_balance_dd / DDR = max_equity_dd / max_balance_dd
```

任何档位只要 equity DD 超限即失败，不得使用较小的 balance DD 宣传。

### 1.2 小资金、多币和集中度

- principal 必须 `<5000U`；搜索预算 `750/1000/1500/2000/3000/4000/4999U`；robustness 加 `500U`；
- 报 exact minimum executable principal，包含 spot cash、perp margin、fees、maintenance 和 next-SO reserve；
- 实际成交 base assets `>=5`，不是配置 symbols 数；
- max symbol abs net PnL share `<=50%`；
- max group abs net PnL share `<=50%`；
- 另报更严格 `<=35%` 是否通过，不强行用亏损组稀释 concentration；
- 每个评估窗口必须有真实 SO；最终另报 SO cycle 占比和 SO-cycle PnL attribution；
- max family gross `<=40%`、max group gross `<=25%`（event-level combination 时）。

### 1.3 成本、生存和实盘语义

每条 replay 从 G0 起必须包含：

```text
spot/perp entry + SO + reduce + close fee
market/limit slippage and queue policy
funding at exact settlement boundary
spot cash occupation and optional borrow interest
perp initial/maintenance margin tier and liquidation
PRICE_FILTER / LOT_SIZE / MARKET_LOT_SIZE / MIN_NOTIONAL or NOTIONAL rounding
next SO + close fee + maintenance reserve
partial fill / reject / one-leg delay / hedge-or-flatten legging loss
terminal forced close in equity and balance curves
```

任一 stale leg、principal breach、unmodeled liquidation、NaN、负 quantity、未来 bar 或 filter bypass 立即
淘汰。

## 2. P0：Round 19 authority、历史去重和中央状态机

### 2.1 启动条件

先验证：

```text
Round19 corrected_machine_state = materially_incomplete_invalid_results
Round19 target_hit = false
Round19 production_ready_candidates = 0
Round19 strict_valid_search_rows = 0
```

创建：

```text
docs/superpowers/artifacts/glm-martingale-core-round20/
  round20-execution-state.json
  exploration-registry.jsonl
  failure-ledger.jsonl
  historical-fingerprint-index.json
  p0/..p12/
```

### 2.2 中央 registry 合同

所有 runner 只能调用一个 launcher。禁止 R6/R7 自建简化 registry。每次 experiment 至少两行：

```text
running:
  experiment_id / parent_id / family / fold / budget / seed
  fingerprint_sha256 / raw_command / pid / started_at / git_commit
  engine/data/funding/filter/maintenance/fit/weights/effective_config/cost hashes

terminal:
  experiment_id / fingerprint / exit_code / wall_s / rss_mb / terminal_status
  full metrics / segment metrics / exact rejection reason
  event/trade/order/equity/funding/rejection stream hashes
```

状态文件只能由 validator 从 registry、raw artifact 和 Git commit 重算。至少新增 negative tests：

1. gate stub 写 complete、registry 为 0 行 => fail；
2. local registry 有行、central 为 0 => fail；
3. terminal 缺 running/command/exit/hash => fail；
4. G2 config ID 不在 committed G1 survivors => fail；
5. `all_bound=false` 但另有 conditional flag => fail；
6. 五 family 少一个实现 => round blocked；
7. fit end 晚于 replay first decision => invalid_data_leakage；
8. original handoff 与 corrected authority 冲突 => corrected authority 胜出。

### 2.3 历史 exact 去重

递归扫描 Round 1-19 的 registry、checkpoint、configs、corrected authority、plans 和 reports。canonical
fingerprint 至少包含：

```text
engine + data + funding + exchange-filter snapshot + maintenance tiers
+ cycle topology + trigger/fit contract + universe/weights + scheduler
+ effective config + cost + window + budget + fold + seed
```

Round 19 无效 M2F/P1/K1/V1 可作为 `repair_or_never_executed`；普通 soft multiplier、DD scaling、HTF gate、
standalone funding sleeve、DGT/breakout/trend、旧 XS selector 和原样 M1R config 禁止重复。

## 3. P1：冻结数据、filters 和 future lock

### 3.1 数据合同

权威查询必须显式限定：

```sql
market_type='spot' AND timeframe='1m'
market_type='futures_usdt_perp' AND timeframe='1m'
```

本地已知可用输入：

- `data/market_data_full.db`：BTC/ETH/BNB/SOL/XRP/DOGE 等 spot + perp 1m，自 2023-01-01 起；
- `data/funding_rates_round12.db`：32 symbols，2023-01-01..2026-07-10；
- `data/premium_index.db`：6 symbols，2023-01-01..2026-05-31。

必须重新输出 schema、row count、distinct symbol、min/max time、missing minute、duplicate、OHLC、funding gap
和 source provenance。spot/perp 同 timestamp 缺一腿时 basis UNKNOWN，不能 forward-fill 下单。

### 3.2 Exchange 规则快照

冻结当前 Binance 官方 `exchangeInfo`、maintenance brackets 和费率假设，记录 response hash 与获取时间：

- spot filters：`https://developers.binance.com/docs/binance-spot-api-docs/filters`
- USD-M exchange info：`https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Exchange-Information`
- USD-M common definitions：`https://developers.binance.com/docs/derivatives/usds-margined-futures/common-definition`

历史规则不可得时用当前规则与更保守 stress 两套，不得假设零门槛。reverse basis 若缺历史 borrow availability/
interest，则该方向 `blocked_missing_borrow_data`，不能当免费 short spot。

### 3.3 Future lock

`2026-07-11+` 继续封存。只有 provisional finalist、selection 已 commit/push 且数据满 30 天后，才能一次性
future check。当前日期 2026-07-20，不满足 30 天，禁止读取该区间做选择或诊断。

## 4. P2：统一 production-conservative Martingale 引擎

任何 family 搜索前至少通过以下真实 unit/integration tests：

1. residual/basis 正负生成相反的 long/short pair/basket；
2. beta sign 和 constrained weights 进入 rounded quantity；
3. FO/SO 使用 fill quantity 计算 aggregate PnL；
4. SO 必须同时满足净亏损、adverse movement、reserve 和 liquidation gate；
5. TP 扣预计 close/legging cost 后为正；
6. FO/SO/TP/abort/end-close 全收费并进入 equity/balance DD；
7. funding 使用持仓方向、当前 mark quantity 和 settlement timestamp；
8. spot cash 与 futures margin 分账后汇总 shared equity；
9. reserve 包含 next SO、close fee 和 maintenance buffer；
10. PRICE_FILTER、LOT_SIZE、MARKET_LOT_SIZE、MIN_NOTIONAL/NOTIONAL 生效；
11. maintenance tier 切换与 conservative liquidation 生效；
12. 25/50/75% partial fill 后按 policy hedge/flatten，损失入账；
13. 后腿 reject/timeout 触发 deterministic legging policy；
14. stale spot/perp/factor/任一 basket leg 不能下单；
15. same timestamp N-leg order 和 fill ordering 确定性，且 64 个并发 replay hash 一致；
16. permanent reject 进入 freeze，temporary reject 使用 state-hash cooldown；
17. 同 `(group,depth,reason,state_hash)` 不逐分钟重复；
18. actual symbol/group/family concentration 从成交和 funding 重算；
19. no-SO window 由 runner 真正标记 `not_martingale_no_so`；
20. terminal close 后 group 可开新 cycle；
21. cycle-open symbols/directions/weights/fit hash 冻结，active cycle 不换腿；
22. kill/restart 后 cycle、fills、reserve 和 client order id 幂等恢复；
23. backtest adapter 与 fake exchange 的 order/equity/rejection suffix hash 一致；
24. wrong-market/timeframe rows 注入时 loader fail；
25. train-end fit 注入早期 inner replay 时 validator fail。

不得用“ideal atomic”作为 candidate path。它只能作为 upper-bound control，必须与 conservative path 分栏。

## 5. P3：Causal nested fit 和 inner replay

沿用 outer folds，禁止事后改日期：

```text
F1 train 2023-01-01..2023-06-30, purge 7d, validation 2023-07-08..2023-12-31
F2 train 2023-01-01..2023-12-31, purge 7d, validation 2024-01-08..2024-12-31
F3 train 2023-01-01..2024-12-31, purge 7d, validation 2025-01-08..2025-12-31
F4 train 2023-01-01..2025-12-31, purge 7d, validation 2026-01-08..2026-05-31
```

每个 outer train 内生成 causal inner walk-forward：

```text
fit only data <= inner_fit_end
purge >= max(signal lookback, settlement latency, 1 completed decision bar)
replay starts strictly after fit_end + purge
roll/expand fit before each later block
stitch only inner-OOS equity; never replay an earlier block with a later fit
```

小于 180 天的单 block 不输出/不排序 annualized return，只看 raw return、DD、PF、cost、SO 和 survival。
只有 stitched causal inner-OOS 有效天数 `>=180` 才报告 ann。F1 数据不足时作为 sign/stability veto，不用短窗
ann 夸大排名。

每条结果记录 `fit_start/end/replay_start/end/purge/fit_hash`。必须注入一次 train-end fit 反例证明 fail-close。

## 6. P4：六个执行 family

### 6.1 C1：M1R event-balanced concentration repair（主攻一）

只把 `M1R_F3_028/044` 当 changed-engine control，不得原样重跑进入排名。新机制：

```text
3-6 train-stable disjoint residual groups
train-frozen equal-risk group budget
deficit-round-robin/activity quota for FO admission
shared next-SO reserve before new FO
max group gross 20/25%
max concurrent groups 2/3/4
active cycle keeps group/beta/direction/ladder frozen
```

调度只能使用当时已完成数据和 train-frozen spread vol、cycle duration、cost、historical activation rate；禁止
按未来或 validation PnL 给赢家加权。目标是提高独立 group 的成交机会，不能靠添加亏损单稀释 PnL share。

开放维度仅限 scheduler：`group_cap=20/25%`、`max_live=2/3/4`、`reserve_depth=next/all`、
`quota_window=7/30d`。M1R ladder 仅用 Round 19 两个邻域中心和明确相邻值；exact unchanged vector 只跑一次
control，标记 `control_not_new_search`。

### 6.2 B1：多币 spot-perp basis aggregate Martingale（主攻二）

这是对旧 standalone funding probe 的新机制，不是 funding sleeve：

```text
basis_t = log(perp_completed_close_or_mark_t / spot_completed_close_t)

FO:
  synchronized fresh spot+perp
  AND train-frozen basis/funding state valid
  AND expected convergence after all costs > 0
  -> positive basis: long spot + short perp
  -> negative basis: short spot + long perp only with frozen borrow data

SO:
  aggregate spot+perp cycle net PnL after fee/funding/borrow < 0
  AND basis moves adversely from last paired fill
  AND reserve/liquidation/legging gates pass
  -> both legs add paired notional, next layer > previous layer

TP/ABORT:
  basis converges and aggregate net after close > floor
  OR deadline/break/liquidation-buffer breach -> paired conservative close
```

要求至少 5 个 base assets 在一个 shared-cash portfolio 内真实成交。funding 必须属于对应 cycle PnL；禁止
把 buy-and-hold spot、裸 perp 或独立 carry 曲线加入组合。

开放参数：

```text
fit lookback        30/60/120/240d
entry basis z       1.0/1.5/2.0/2.5
SO adverse step z   0.25/0.50/0.75
layer schedule      1.0,1.25,1.55,1.90 OR 1.0,1.35,1.80,2.40
max paired layers   3/4
deadline            1/3/7/14d
funding gate        cost-only / favorable-veto
```

不得用 premium_index 缺失币种的零值；可从同 timestamp spot/perp 推导 basis，premium 仅作有数据币种的
交叉校验。

### 6.3 M2R：修正的动态 factor-residual basket

Round 19 M2F 是 invalid repair input。必须实现：

```text
cycle open current residual ranks -> choose long most-negative / short most-positive
train-frozen constrained solve:
  sum(abs(w))=1
  abs(sum(w*beta))<=0.10
  max abs(w)<=0.25
  long gross>=0.35 and short gross>=0.35
  actual legs>=6
actual basket residual = sum(w_i * residual_i) plus explicit BTC hedge if used
```

SO adverse signal、aggregate PnL 和实际持仓必须使用同一 signed-weight residual。BTC/PC1 factor 与所有腿
必须在同 timestamp fresh；PC1 必须真正实现 train-only loadings，不能返回 `None`。四个 fold beta exposure
unit test 全过后才允许 G0。

### 6.4 P1：Partial-cointegration Martingale（Round 19 强制补做）

实现 `residual = random_walk + mean_reverting` state-space，只允许 MR component 产生 signal。必须通过：

- fixed-vs-partial likelihood；
- identifiability；
- block-bootstrap parameter stability；
- RW variance share 和 MR half-life gate；
- t 完成更新、t+1 最早下单。

开放维度沿用 Round 19：lookback `30/60/120/240d`、RW share max `0.10/0.25/0.40`、half-life max
`12/36/72/168h`、entry `1/1.5/2/2.5`、SO step `0.3/0.5/0.75`、layer schedule `1.2/1.35/1.5`
与 `3/4/5` 层。任何不收敛为 UNKNOWN，block FO/freeze SO。

### 6.5 K1：spurious-control causal Kalman Martingale（Round 19 强制补做）

process/observation noise 只在 train blocked likelihood 选择。先过四个 adversarial controls：

```text
independent random walks: >=95% block
permuted symbols: no candidate
fixed cointegration: beta drift not exaggerated
synthetic time-varying beta: path error better than static control
```

active cycle 冻结 open beta/weights；innovation CUSUM/break 只 block FO、freeze SO 或 aggregate reduce，不
产生独立收益。

### 6.6 V1：Sparse VECM aggregate Martingale（Round 19 强制补做）

固定 8/12 symbol universe，train-only Johansen rank + adaptive-Lasso sparse VECM，penalty 只按 blocked-CV
forecast/residual stability选择，不看 strategy return。要求 5-8 非零 weights、max abs weight 25%、双边 gross
各 35%、abs BTC beta 0.10、稳定 eigenvalue/alpha/half-life。portfolio error-correction residual 驱动一个
N-leg aggregate Martingale cycle。

## 7. P5：Soft Martingale + Strategy-Embedded Labeling enhancement

来源 `10.3390/a19060442`。论文的 442.6% 年化来自单 EUR/USD、1:500、79.97% equity DD，不是收益
先验。本轮只测试两个此前未执行的 exact mechanism：

1. 十层次线性相对 lot schedule：`linspace(1,5,10)`，并测试预冻结 prefix `4/6/10` 层；
2. Strategy-Embedded Label：标签由完整 forward Martingale path simulation 产生，不是固定 horizon 涨跌。

只允许 C1/B1/P1 中通过 causal G1 的 parent 进入，最多 2 个 parent。模型限制为 regularized logistic/GAM
一类可解释低容量模型；features 只用 completed residual/basis、vol、volume、funding、cycle reserve、break
state。模型选择必须 nested、embargo、permuted-label control、calibration 和 ablation；不得用 outer validation
收益调模型。

SEL 只能 veto/side-select Martingale FO/SO。若 feature/model 无 order delta、permuted label 仍有收益、邻域不稳
或 SO 消失，立即 `invalid_mechanism_or_overfit`。不再搜索普通 `multiplier 1.15-1.60`，避免与前轮重复。

## 8. P6：G0 activation 和配额冻结

每 family 每个开放参数至少：

```text
8 synthetic traces
4 real causal short windows: bull / bear / range / jump/basis shock
low/high config hash delta
state delta
order or rejection delta
```

任一参数 inert、5+ assets 无法成交、无真实 SO、factor/basis stale、fit 不 causal 或成本字段为空，立即停止
family。`all_bound=false` 不能被其他字段覆盖。

G0 全过后冻结 unique config 清单：

| family | configs/fold | seed | 备注 |
|---|---:|---:|---|
| C1 | 64 | 20261001 | scheduler repair；原样 control 不计配额 |
| B1 | 96 | 20261002 | 新 spot-perp paired cycle |
| M2R | 64 | 20261003 | invalid M2F repair |
| P1 | 64 | 20261004 | Round 19 never executed |
| K1 | 48 | 20261005 | 先过 adversarial controls |
| V1 | 64 | 20261006 | 先过 sparse fit stability |
| Soft-SEL | <=32/parent | 20261007 | 仅 parent gate 后生成 |

使用 deterministic Sobol/LHS，先写 `config-list-sha256` 并 commit。运行中禁止因为看到结果而扩维。

## 9. P7：G1 causal inner-OOS 全量搜索

每个 config/fold 在 `1000/4999U` 上执行全部 causal inner blocks，禁止快筛近似。immediate rejection：

```text
breach/liquidation/filter bypass/stale leg
equity DD >45%
stitched inner-OOS negative or >=2 negative blocks
任一 block actual assets <5
任一 block symbol/group concentration >50%
任一 block no real SO
unique legging/rejection failure >10%
cost / gross profit >50%
fit/break/adversarial state invalid
DDR >3 unless max equity DD <=5%
```

每 family/fold 最多 12 个 config 进入 G2。排名只用 causal inner-OOS raw/stitched performance、worst DD、
positive blocks、cost、concentration、SO attribution 和 fit stability；小于 180 天 block 不用 ann。

G1 survivor artifact 必须 commit/push。G2 只能读取该 artifact 的 exact config ID，禁止从 checkpoint 重新
宽松选 topN。

## 10. P8：G2 budget、plateau 和 concentration recovery

G1 survivor 在同 outer train 的 causal inner-OOS 上运行：

```text
budgets = 750/1000/1500/2000/3000/4000/4999U
center + 12 local neighbors
one-symbol/group-left-out train diagnostics
fee/slippage 1.0x/1.5x
one-leg delay 1 bar control
```

G2 strict train gate：

```text
stitched causal ann >=30%（仅有效天数>=180时）
否则 stitched raw return >0 且不得用 ann 排名
worst equity DD <=35%
>=4/5 positive diagnostics where five blocks exist
all windows survival/filter/causality pass
actual assets >=5
symbol/group concentration <=50%
real SO in every scored window
neighbor 8/12 retains >=80% center return and DD <= center+3pp
budget is an executable plateau, not one isolated principal
```

C1 的专门推进门：`M1R_F3_028/044` changed-engine control 的 concentration pass 从 `4/5`/`3/5` 改善到
`5/5`，且 causal stitched performance 仍为正。该门只叫 mechanism progress，不是目标命中。

## 11. P9：selection freeze 与一次性 outer validation

每 family/fold 最多 3 个 config+budget。先生成并 commit/push：

```text
selected-configs.json
fit/symbol/weights/scheduler/config/budget hashes
engine/data/filter/maintenance/cost/validator hashes
all train metrics and reject ledger
```

validator 必须证明 validation 首次读取时间晚于 selection commit。之后每个 selected row 只启动一次。
validation 立即淘汰条件与 G2 相同，并额外要求：

- 不得有两 outer validation folds ann/return 为负；
- 权重/预算跨 fold 不得跳到孤立极值；
- 收益不能主要来自一个 symbol/group/family；
- target profile 的 DD 必须使用 equity DD。

validation 失败后禁止调参重读。若发现 engine bug，所有受影响行标 `invalid_engine_bug`，修测试、改变
engine hash，并从该 family G1 全量重跑。

## 12. P10：event-level 多 family 组合

单一 C1/B1/M2R/V1 本身已须多币。只有至少两个独立 parent family 通过 outer validation，才允许最多 32
个组合：

```text
one shared cash/equity/margin/liquidation ledger
train-frozen equal-risk or inverse-downside-vol family weights
max family gross 40%
max group gross 25%
reserve active cycles and all declared next SO before new FO
no curve sum / curve rotation / post-validation optimizer
```

所有 PnL 仍来自真实 Martingale cycle。组合必须重新 event-level replay，并在 validation 前单独 commit。
无法满足一次性 validation 时不做组合，不得拼接已读 validation 曲线。

## 13. P11：finalist robustness 和 production parity

每档最多 2 个 finalist，执行：

1. budgets `500/750/1000/1500/2000/3000/4000/4999U`；
2. 五 cold starts、full window、LOSO/LOGO/LORO；
3. 12 参数邻域、fit lookback/rank/beta/weight drift；
4. fee/slippage `1.0x/1.5x/2.0x`、adverse funding/borrow；
5. partial fill `25/50/75%`、one-leg delay `1/2/3 bars`、one-leg reject；
6. maintenance tier/filter snapshot stress、liquidation gap；
7. cancel/replace、duplicate/out-of-order exchange events；
8. DSR、CSCV/PBO、selection frequency、rank degradation，trial count 合并 Round 1-20；
9. exact minimum executable principal 和每个 blocked leg 原因；
10. equity DD、balance DD 和 DDR。

production parity 必须启动真实 service entry + fake exchange：load frozen artifact/SQLite、N-leg rounded intents、
partial fills/rejects、持久化、kill/restart、exchange reconcile、next SO/TP/reduce。backtest conservative adapter 与
service 的 order/rejection/equity suffix hash 必须 exact。

GPU 只可用于确定性的 fit/SEL 矩阵计算，并须与 CPU tolerance test 一致；canonical event replay 和最终指标
必须由 release CPU engine 重放。允许多进程、read-only data cache 和 checkpoint，不允许用 GPU/快筛近似
替代 full event replay。

## 14. 防重复表

| 已探索/无效方向 | Round 20 处理 |
|---|---|
| 普通 multiplier/spacing/legs sweep | 禁止重复；Soft-SEL 只跑论文 exact schedule + path label |
| standalone funding carry sleeve | 禁止；B1 必须 paired spot-perp loss-after-add cycle |
| 普通 DD scaling | 不作为收益 family，只作 reserve/size hard gate |
| HTF/ADX/EMA/VR 普通 gate | 禁止重复，除非作为 frozen SEL feature 且 ablation 有增量 |
| DGT/breakout/trend 独立 PnL | 禁止，所有 PnL 必须来自 Martingale cycle |
| Round 19 原样 M1R_F3_028/044 | 只作一次 changed-engine control，不进新搜索排名 |
| Round 19 M2F | 旧结果 invalid；仅 M2R 修复后新 fingerprint 可跑 |
| P1/K1/V1 | Round 19 0 replay，必须首次实现，不算重复 |
| curve-level allocator/rotation | 禁止；只允许 event-level shared cash |

每个新失败追加 exact fingerprint、first failed gate、metrics 和 never-repeat reason。禁止只写自然语言总结。

## 15. 外部来源如何转成可证伪任务

| 来源 | 采用机制 | 禁止外推 |
|---|---|---|
| `10.3390/a19060442` | exact soft schedule、SEL、equity/balance DDR | 不复制 442.6%，不使用 1:500 |
| `10.3390/risks11050093` | multiple-pair causal diversification | 不按 validation PnL 动态配权 |
| `10.1080/14697688.2017.1370122` | partial cointegration MR component | RW component 不发单 |
| `10.1080/07474938.2020.1861776` | Kalman spurious controls | 未过 random-walk null 不搜索 |
| `10.1111/anzs.12304` | sparse VECM penalty/stability | 不按策略收益选 rank/penalty |
| `10.1111/mafi.70018` | spot-perp/funding anchoring | funding 不做独立 carry sleeve |
| `10.1007/s10100-021-00763-4` | bounded-risk pair abort | abort 不产生独立收益 |
| `10.1016/j.ejor.2022.07.037` | liquidation/leverage stress | 不假设自动强平无损 |

## 16. 进展与最终状态

除三档目标外，本轮只允许以下推进声明：

| ID | 条件 |
|---|---|
| P-A | C1 causal concentration `5/5` pass，stitched positive，DD<=20% |
| P-B | 任一 causal multi-asset family train ann>=40%、DD<=20%、>=4/5 |
| P-C | 一次性 outer validation ann>=35%、DD<=30%、5/5、5+ assets |
| P-D | 保守/平衡/激进任一档完整命中 |

实现代码、跑很多次、短窗高 ann、修 bug 都不是 frontier progress。

最终状态只能是：

```text
TARGET_HIT_PROVISIONAL_FUTURE_LOCK
FRONTIER_PROGRESS_ONLY
VALID_SEARCH_NO_FRONTIER_PROGRESS
BLOCKED_ENGINE_DATA_OR_EXECUTION
MATERIALLY_INCOMPLETE_INVALID_RESULTS
```

只有 P0-P12 全部按合同执行、所有 mandatory family 都有 terminal evidence，才允许
`VALID_SEARCH_NO_FRONTIER_PROGRESS`。任一 family 因实现 scope 被跳过，整轮必须 `BLOCKED_ENGINE_DATA_OR_EXECUTION`。

## 17. Handoff 必填内容

最终 handoff 必须由 validator 生成：

1. P0-P12 machine state 和 first blocked gate；
2. Round 1-19 corrected authority 继承表；
3. family planned/running/terminal/duplicate/timeout/invalid counts；
4. causal fit/replay timestamps、selection commit 和 validation read-once 证据；
5. 三档目标、P-A/P-B/P-C/P-D 和 5/5 表；
6. top 10：assets、groups、每腿 weight/direction/leverage、budget、ann、equity/balance DD、DDR；
7. exact minimum principal 和 reserve/liquidation buffer；
8. FO/SO/TP/abort/liquidation/partial-fill/legging/rejection counts 与全部成本；
9. symbol/group/family concentration 和 SO PnL attribution；
10. backtest_candidate 与 production_ready 分栏；
11. raw command、五类 hashes、order stream 和 registry 路径；
12. 全部失败 exact fingerprint + never-repeat reason；
13. `2026-07-11+` future lock 状态。

没有 candidate 也要输出 top diagnostic，但必须写 first failed gate、selection leakage 状态和
`not_candidate`。

## 18. Git 和执行纪律

1. 从包含本计划的远端 commit 创建 `glm-martingale-core-round20`；
2. P0/P1/P2/P3/G0/G1/G2/selection/validation/robustness/handoff 分 phase commit/push；
3. 每个 commit body 必须含 `问题描述`、`复现路径`、`修复思路`；
4. selection 必须 commit/push 后才能读 validation；
5. registry 每 experiment 实时 append，checkpoint 至少每 30 分钟；
6. 不提交 DB、target、cache 或大 stdout，只提交 manifest、config、registry、summary、必要 raw evidence；
7. 不覆盖历史 artifact；所有修正使用 additive authority；
8. handoff 前 `git status --short` 必须为空，且当前 branch 已 push。
