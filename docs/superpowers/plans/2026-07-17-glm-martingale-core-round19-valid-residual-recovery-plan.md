# GLM Martingale Core Round 19：有效残差恢复、Partial Cointegration 与稀疏 VECM 组合计划

制定日期：2026-07-17。

前置权威审计：

- `docs/superpowers/reports/2026-07-17-chatgpt-round18-execution-audit-and-fix.md`
- `docs/superpowers/artifacts/glm-martingale-core-round18/r18-corrected-authority.json`

本文件是 Round 19 唯一执行任务书。GLM 不得用 Round 18 原 handoff、checkpoint 或 gate stub 替代本
计划的 validator 结论。

## 0. 核心原则

1. **始终是 Martingale**：每个收益订单必须属于一个真实亏损后加仓、`multiplier>1`、至少发生一次
   SO 的 aggregate cycle。Residual、Kalman、VECM、break detector 只判断多空、FO/SO/TP/abort，不能
   自己产生独立 PnL。
2. **先有效，再搜索**：R0-R5 任一 fail-close 未过，相应 family 禁止进入 G1。
3. **逐 fold nested**：每个 fold 在自己的 train 内 fit、筛选、选预算、commit hash，之后 validation
   只读一次。禁止 F2 选参后套用所有 validation。
4. **所有失败落账**：每次启动前写 `running`，结束写 terminal；失败、超时、重复、无 SO、无效机制都
   必须记录 exact fingerprint 和 reason。
5. **不把旧无效结果当重复**：Round 18 M1 必须以 `repair_replay` 完整重跑；旧 engine hash 的 exact
   config 只作为对照，不计作新探索。其他已经有效执行过的 R1-R17 exact family 禁止重复。
6. **不承诺收益**：本轮要提高命中概率并形成可信边界，禁止在证据前写“下一轮一定命中”。

## 1. 不可变目标与 hard gates

### 1.1 三档目标

| 档位 | portfolio ann | portfolio max DD | cold starts |
|---|---:|---:|---:|
| 保守 | >=50% | <=10% | >=4/5 正 |
| 平衡 | >=90% | <=20% | >=4/5 正 |
| 激进 | >=110% | <=30% | >=3/5 正 |

同时单独报告是否 `5/5` 全正，不得用平均收益掩盖负区间。

### 1.2 小资金与多币种

- principal 必须 `<5000U`；主搜索预算 `1000/2000/3000/4000/4999U`；robustness 追加
  `500/750/1500U`；
- 报 exact minimum executable principal，不得只写“低于 5000”；
- 实际成交 symbols `>=5`，不是配置中列出 `>=5`；
- max symbol abs net PnL share `<=50%`；
- max group abs net PnL share `<=50%`；
- 任一行 `groups_with_so=0` 只能记录 `not_martingale_no_so`，不得进入收益排名。

### 1.3 成本与生存

所有回放必须包含：

```text
entry fee + entry slippage
close/reduce fee + close slippage
funding at exact settlement boundary
mark-to-market gross/margin
maintenance margin + liquidation
tickSize/stepSize/minQty/minNotional rounding
partial fill / reject / legging loss policy
terminal forced close in equity/DD
```

任一 principal breach、unmodeled liquidation、NaN、负数量、未来数据或 stale-leg order 立即淘汰。

### 1.4 防过拟合

- 选择只用 train；validation 失败不得回 train 调参后重读；
- 合并 Round 1-19 全 trial 数计算 DSR、CSCV/PBO、selection frequency、rank degradation；
- 所有 indicator/state update 只读当前 decision 前 completed bars；
- 2026-07-11 之后数据继续 future lock，满 30 天且有 provisional finalist 后才允许一次性读取；
- 年化少于 180 天的窗口只作淘汰诊断，不进入 top candidate 表。

## 2. R0：修正 Round 18 authority 和中央状态机

先验证以下状态存在且内容一致：

```text
Round18 corrected_machine_state = materially_incomplete_invalid_results
Round18 target_hit = false
Round18 production_ready_candidates = 0
```

创建：

```text
docs/superpowers/artifacts/glm-martingale-core-round19/
  round19-execution-state.json
  exploration-registry.jsonl
  historical-fingerprint-index.json
  failure-ledger.jsonl
  r0/..r12/
```

`round19-execution-state.json` 只能由 validator 从原始 registry/artifacts 重算，禁止 runner 自报 complete。
每 phase 必须有 `pending/running/complete/blocked/invalid`，且 predecessor 未过时后续为
`blocked_predecessor`。

## 3. R1：扫描 Round 1-18 的 canonical 去重索引

必须递归扫描，而不是只扫最近一轮：

```text
docs/superpowers/artifacts/glm-martingale-core-round*/**/*.json
docs/superpowers/artifacts/glm-martingale-core-round*/**/*.jsonl
docs/superpowers/reports/*round*.md
docs/superpowers/plans/*round*.md
```

索引至少记录：

```text
source_round / source_path / source_row
authority_status / retained_scope
engine_sha256 / data_sha256 / funding_sha256
cycle_topology / trigger_contract_sha256 / fit_contract_sha256
universe_group_weights_sha256
resolved_config_sha256 / effective_config_sha256 / cost_model_sha256
window / budget / fold / seed
event/trade/equity/funding/rejection sha256
terminal_status / failure_reason
```

canonical fingerprint：

```text
sha256(engine + data + funding + topology + trigger + fit + universe/weights
       + resolved/effective config + cost + window + budget + fold + seed)
```

同 fingerprint 已有可信 terminal 时写 `skipped_duplicate`，不启动进程。Round 18 旧 M1 因 engine/fit
contract 改变可作为 `repair_replay`，但必须在 registry 标明旧 fingerprint 和新 fingerprint 的映射。

### 3.1 append-only 行合同

每个 experiment 至少两行：

```text
running: experiment_id, fingerprint, raw_command, pid, started_at, all hashes
terminal: exit_code, wall_s, rss_mb, status, exact metrics, all trace hashes, reason
```

禁止覆盖、truncate 或重写历史 JSONL。进程崩溃后由 recovery validator 把 orphan running 行补成
`interrupted`，不能静默删除。

## 4. R2：回放引擎 fail-close 修复门

在任何搜索前，新增独立 negative/positive tests。至少覆盖：

```text
1. positive/negative residual 生成相反 pair 方向
2. beta sign/absolute beta 生成正确方向和 quote notional
3. static [1,1] M1 fit 不能影响运行时方向
4. unequal FO/SO 使用 quantity-weighted PnL
5. FO/SO/TP/abort/end-close 都收 fee/slippage
6. terminal close 进入 equity/DD
7. funding 使用 cycle direction + current mark notional
8. group cap 比较 gross notional，不是 margin
9. portfolio reserve 包含 next SO + fees + maintenance buffer
10. stale leg/current incomplete bar 不能发同步订单
11. same timestamp N-leg 顺序稳定
12. minQty/stepSize/tickSize/minNotional 四类 filter 生效
13. maintenance margin breach 触发 conservative liquidation
14. partial fill/reject 后按 policy hedge/flatten，损失入账
15. deadline close 后 group 可开始后续新 cycle
16. actual symbol/group concentration 从成交重算
17. no-SO 行被标记 not_martingale_no_so
18. permanent minNotional/gross-cap reject 不得每分钟无限重试
19. central registry 缺 running/command/hash 时 validator fail
20. 静态 gate stub 与 raw registry 不一致时 validator fail
```

### 4.1 rejection cooldown

拒单必须分类：

```text
permanent_config: minNotional impossible / group cap impossible -> cycle/family freeze
temporary_margin: wait until portfolio state changes or cooldown expires
market_data: wait until fresh common boundary
exchange_transient: exponential backoff with max attempts
```

同一个 `(group, depth, reason, state_hash)` 禁止每分钟重复 event。G1 gate 使用 unique rejection attempts，
不使用重复 bar 数。

### 4.2 conservative multi-leg execution

研究 base 可以同时计算 ideal atomic control，但 candidate 必须使用 conservative path：

1. 订单按 deterministic symbol order 发出；
2. 每腿应用 exchange rounding；
3. 前腿已成交、后腿 reject 时，立即按预声明 policy 补 hedge 或 flatten；
4. flatten 收两侧 fee/slippage；
5. partial fill 数量进入真实 beta/gross/margin；
6. 未恢复到 beta exposure limit 前禁止 SO/TP 新指令。

## 5. R3：统一 Martingale cycle 机器定义

所有 family 共用一个 cycle ledger：

```text
FO:
  completed common boundary
  AND family residual state valid
  AND entry threshold crossed
  -> N-leg real first orders

SO:
  aggregate cycle net PnL after all cost < 0
  AND family residual moved adversely from last fill
  AND reserve/liquidation/break gate allows
  -> every required leg adds real notional, multiplier > 1

TP:
  aggregate net PnL - estimated close cost > TP floor
  AND residual enters exit region
  -> aggregate close/reduce

ABORT:
  structural break / deadline / legging failure / liquidation buffer breach
  -> aggregate conservative close/reduce; loss enters equity
```

Active cycle 必须冻结：group、symbols、weights/beta、direction、ladder、fit/state hash。状态更新不能替换
active leg、复制仓位、反转方向或重写历史 SO threshold。

## 6. R4：五个独立 family

### 6.1 M1R：修正后的 static cointegration pair control

这是 Round 18 M1 的 mandatory repair，不算新 family：

- train-only OLS/log-price residual；
- pair selection 只看 liquidity、stationarity、beta/half-life stability，不看策略收益；
- 每 fold 至少 3 个 disjoint pairs，组合实际 symbols `>=5`；
- `q_dep=-sign(z)`，`q_factor=sign(z)*beta`；
- quote weights `|beta|:1`；
- active cycle beta 固定；
- Round 18 96 exact parameter vectors 全量重跑，验证修正前后 delta。

若某 fold 无 3 个 stable disjoint pairs，写 `blocked_no_stable_groups`，不得用其他 fold 收益补齐。

### 6.2 M2F：同步 factor-residual basket

M2F 是独立主 family，禁止因 M1R 失败而 block：

```text
factor = BTC / train-PC1
universe = fixed 6 / 8 / 12 symbols
fit = residual_i = log(P_i) - beta_i * factor - mu_i
entry = cross-sectional residual dispersion + aggregate basket residual
legs = most-negative residual long + most-positive residual short + optional BTC hedge
```

权重通过 train-frozen constrained solve 获得：

```text
sum(abs(weight)) = 1
abs(sum(weight * beta)) <= 0.10
max abs symbol weight <= 0.25
long gross and short gross each >= 0.35
actual traded symbols >= 6
```

PC1 loadings只用 train。Cycle open 时 residual rank、symbols、signs、weights 冻结；SO/TP/abort 对同一
basket aggregate 执行。禁止按当期赢家替换 active leg。

### 6.3 P1：Partial-cointegration Martingale

来源：`10.1080/14697688.2017.1370122`。对 pair residual 建模：

```text
residual_t = random_walk_t + mean_reverting_t
```

只允许 mean-reverting component 产生 z signal。开放参数：

```text
fit lookback       = 30d / 60d / 120d / 240d
RW variance share  = max 0.10 / 0.25 / 0.40
MR half-life       = max 12h / 36h / 72h / 168h
entry z            = 1.0 / 1.5 / 2.0 / 2.5
SO step z          = 0.30 / 0.50 / 0.75
multiplier         = 1.20 / 1.35 / 1.50
max legs           = 3 / 4 / 5
```

train 必须通过 fixed-vs-partial likelihood diagnostic、参数可识别性和 block-bootstrap stability。RW share
超阈值、MR coefficient 非收敛、样本不足时 UNKNOWN，block FO/freeze SO。在线 filter 只用 completed t
更新，订单最早在 t+1；hyperparameter 由 train 冻结。

### 6.4 K1：spurious-control Kalman time-varying beta

来源：`10.1080/07474938.2020.1861776`。该论文明确说明普通 time-varying state-space 容易把 integrated
error 转移到 state，故 K1 只有通过以下对抗门才可运行：

```text
independent random-walk null -> >=95% block
fixed cointegration control  -> beta drift 不得被夸大
time-varying synthetic truth -> beta path error 低于 static control
permuted-symbol control       -> no candidate
```

运行合同：

- process/observation noise 只在 train blocked likelihood 中选择；
- validation 中每个 completed bar 做一次 deterministic predict/update，t+1 才下单；
- beta 每 bar/每天 change cap 预冻结；
- active cycle 使用 open 时 beta/weights，不随 filter 改仓；
- innovation CUSUM 或 Gregory-Hansen style break 超阈值时 block FO、freeze SO 或 aggregate reduce；
- break detector 只控风险，不产生新收益。

### 6.5 V1：5+ 币 sparse VECM aggregate Martingale

来源：`10.1080/14697688.2010.481634`、`10.1111/anzs.12304`、
`10.1016/j.automatica.2019.108651`。

train-only pipeline：

```text
fixed 8/12-symbol universe
-> Johansen rank / sparse VECM
-> blocked-CV 选择 adaptive-Lasso penalty（不读 strategy return）
-> 5-8 个非零 weights
-> error-correction alpha/half-life/stability gate
-> market/beta-neutral normalize
```

权重约束：

```text
actual symbols >=5
max abs weight <=25%
gross long >=35%, gross short >=35%
abs BTC beta <=0.10
same-sign alpha or unstable eigenvalue -> reject fit
```

portfolio residual z 决定 aggregate cycle 方向。FO/SO/TP/abort 全部为 N-leg Martingale 订单，禁止把
VECM forecast 单独做成 stat-arb sleeve。

## 7. R5：G0 binding 和 adversarial activation

每 family 每个开放参数必须有：

```text
8-16 synthetic traces
>=4 real short-window replays: bull / bear / range / jump-shock
low-vs-high effective config delta
state delta
至少一个 order/rejection delta
```

M1R 必须证明 positive/negative residual 都出现 long/short pair；M2F/V1 必须证明 5+ 实际腿；P1/K1
必须证明 RW/break/spurious state 会 block/freeze，而不是只改 hash。

参数 inert、无真实 SO、无法激活 5+ legs 或依赖 validation fit 时立即写 `invalid_mechanism`，禁止搜索。

## 8. 数据与 anchored folds

沿用固定 folds，不事后改日期：

```text
F1 train 2023-01-01..2023-06-30, purge 7d, validate 2023-07-08..2023-12-31
F2 train 2023-01-01..2023-12-31, purge 7d, validate 2024-01-08..2024-12-31
F3 train 2023-01-01..2024-12-31, purge 7d, validate 2025-01-08..2025-12-31
F4 train 2023-01-01..2025-12-31, purge 7d, validate 2026-01-08..2026-05-31
```

每 fold 独立生成：

```text
fit artifact + fit code/data hash
G1 train blocks
G2 full train/subblocks/budget selection
selection commit sha256
one-shot validation terminal rows
```

禁止调用 `load_f2_fits()` 服务所有 fold。Validator 必须注入“错 fold fit”反例并 fail。

五个 cold starts 保留：H1-2023、H2-2023、2024、2025、2026-YTD。它们只在 finalist robustness 使用，
不得参与 G1/G2 参数选择。

## 9. R6：G1 nested train-only successive halving

冻结 unique config 配额：

```text
M1R repair control = 96, seed 20260731（Round 18 exact vectors，新 engine hash）
M2F new family     = 96, seed 20260901
P1 new family      = 96, seed 20260902
K1 new family      = 64, seed 20260903
V1 new family      = 96, seed 20260904
```

每个 family、每个 fold：

```text
configs × 3 predeclared train stress blocks × budgets 1000/4999U
```

先执行 deterministic Sobol/LHS 清单并冻结 `config-list-sha256`，禁止运行中增加“看起来更好”的参数。

G1 immediate rejection：

```text
breach or liquidation
DD >45%
two train blocks ann <0
actual symbols <5
max symbol/group concentration >50%
unique atomic/legging failure >10%
no real SO
cost / gross profit >50%
fit/break state invalid
```

每 family 每 fold最多选 16 个 distinct configs。排名只用 train median return、worst DD、positive blocks、
cost ratio、concentration、fit stability；短窗口 ann 只作淘汰。

## 10. R7：G2 full train、五 subblocks 与预算选择

每个 G1 survivor 在同一 fold train 内运行：

```text
full train
5 non-overlapping or explicitly anchored train-only subblocks
budgets = 1000/2000/3000/4000/4999U
```

禁止 Round 18 那种高度重叠 subblock 未披露。每个 subblock 路径、交集和有效天数写入 artifact。

G2 strict gate：

```text
median subblock ann >=30%
worst subblock DD <=35%
>=4/5 positive
full-train no breach/liquidation
actual symbols >=5
max symbol/group concentration <=50%
at least one real SO in >=4/5 subblocks
unique atomic/legging failure <=10%
```

每 family 每 fold最多 4 个 config+budget。预算选择只用该 fold train，并记录是否处于参数 plateau。参数
邻域不稳时不进 validation。

## 11. R8：selection freeze，先 commit 再读 validation

每个 fold 生成：

```text
selected-configs.json
fit/group/weights hash
effective config + budget hash
engine/data/cost/validator hash
raw train score and rejection ledger
```

必须 `git commit` 并 push 后，才允许 G3 读取该 fold validation。Commit body 包含：

```text
问题描述：...
复现路径：...
修复思路：...
```

Validator 检查 validation 文件 mtime/registry started_at 晚于 selection commit；否则标记
`validation_read_before_freeze` 并整 fold 作废。

## 12. R9：一次性 anchored validation

每个 selected `(family, config, budget, fold)` 只启动一次。立即淘汰：

```text
breach / liquidation / DD>45%
actual symbols<5
symbol/group concentration>50%
no real SO
atomic/legging failure>10%
fit state invalid or break action bypassed
two validation folds ann<0
budget selection across folds不相同或不相邻
收益主要来自一个 group/symbol
```

Validation 失败后禁止改参数、fit、group、权重或预算并再次读取该 fold。需要修 bug 时：

1. 标记所有受影响行 `invalid_engine_bug`；
2. 修复并新增 fail-close test；
3. engine hash 变化；
4. 从该 family 的 G1 全部重跑，不能只重跑 validation。

## 13. R9.5：允许的组合方式

本轮目标是多币种组合，但禁止回测曲线事后加权。只有至少两个独立 parent family 通过 G3 后，才允许
最多 32 个 event-level shared-cash 组合：

```text
all PnL comes from Martingale cycles
one shared equity/margin/liquidation ledger
train-only equal-risk or inverse-vol fixed group weights
max family gross 40%
max group gross 25%
no validation-return optimizer
existing active cycle cannot be switched/hidden
```

组合配置在各 fold train 内选择并重新 commit；不得复用已经读过的 validation。若无法满足一次性 validation，
本轮不做组合，保留独立 family 结果。

## 14. 收益前沿推进门

最终目标不变。另设进展门只用于判断是否有可信推进：

| ID | 要求 |
|---|---|
| P-A | ann >62.51%、DD <=28.62%、>=4/5、5+ symbols、真实 event-level Martingale |
| P-B | ann >=40%、DD <=20%、>=4/5、5+ symbols |
| P-C | ann >=35%、DD <=30%、5/5、5+ symbols |
| P-D | 保守/平衡/激进任一最终档位完整命中 |

只有 P-A/P-B/P-C/P-D 才能写 `frontier_progress=true`。修 bug、跑很多 configs、实现新 family 都是工程/
研究进展，不是收益前沿推进。

## 15. R10 finalist robustness

每个最多 4 个 finalist 必须完成：

1. budgets `500/750/1000/1500/2000/3000/4000/4999U`；
2. 五 cold starts 和 full window；
3. 12 个参数邻域，至少 8/12 ann 保留中心 `>=80%` 且 DD 不高于 `+3pp`；
4. leave-one-symbol/group-out、leave-one-regime-out；
5. fit window、beta/weight drift、RW share、VECM rank、break/jump stress；
6. fee/slippage `1.0x/1.5x/2.0x`、adverse funding；
7. one-leg delay `1/2/3 bars`；partial fill `25/50/75%`；one-leg reject；
8. cancel/replace、out-of-order fill、duplicate exchange event；
9. same-bar conservative ordering、entry/SO/TP delay；
10. maintenance tier/liquidation、exchange filter snapshot变化；
11. DSR/PBO、selection frequency、rank degradation，trial count 合并 R1-R19；
12. exact minimum executable principal 和每个 blocked leg 原因。

任何 robustness breach 淘汰，禁止降档宣传。

## 16. R11 production parity

只有 robustness survivor 执行。必须启动真实 service 入口和 fake exchange：

```text
completed WS/catchup bars for every symbol
-> load frozen fit/state from versioned artifact/SQLite
-> family residual update
-> sync_cycle_decide
-> N-leg rounded order intents
-> exchange partial fills/rejections
-> SQLite group/cycle/leg/order persistence
-> process kill/restart
-> exchange position/open-order reconcile
-> next SO/TP/reduce intent
```

必须持久化：fit hash、family、group、symbols、weights/beta、directions、entry/last residual、depth、每腿 fill
quantity/price、aggregate PnL、fees/funding、reserve、next SO、TP、deadline/break state、client order ids。

通过条件：backtest conservative adapter 与 fake exchange 的 order/rejection/equity suffix hash exact；restart
前后不重复 FO/SO、不丢 TP、不改变 active weights。

只定义 helper、只 push bar/read z、只在内存 HashMap 存 state，一律 fail。

## 17. R12 handoff 和 top 表

最终 handoff 必须从 validator 输出，包含：

```text
1. R0-R12 machine state + blocked reason
2. R1-R18 inherited authority + Round19 corrected authority
3. 每 family planned/started/terminal/duplicate/timeout/invalid counts
4. 每 fold fit/selection commit hash 和 validation read-once 证据
5. 三档目标与 P-A/P-B/P-C/P-D 表
6. 5/5 全正列表
7. top 10 的 symbols、groups、每腿 weight、direction、leverage
8. exact minimum principal、ann、DD、cold starts、concentration
9. FO/SO/TP/reduce/liquidation/legging counts 和全部成本
10. backtest_candidate 与 production_ready 分栏
11. raw series/trades/orders/rejections/five hashes 路径
12. 全部新失败 exact fingerprint + never-repeat reason
13. 2026-07-11+ future lock 状态
```

没有有效 candidate 时也必须输出 top diagnostic，但明确写 `not_candidate` 和首个失败 gate。

## 18. Git 和运行纪律

1. 从包含本计划的远端 commit 新建 `glm-martingale-core-round19`；
2. 每 phase 独立 commit/push，不在最后一次性补 gate stub；
3. 每个 commit body 必须包含问题描述、复现路径、修复思路；
4. checkpoint 每 30 分钟写盘，registry 每 experiment 实时 append；
5. 不提交 DB、target、cache、大 stdout；只提交 config、registry、summary、hash manifest 和必要 raw series；
6. 不覆盖/删除用户或前轮文件；correction 采用 additive authority + superseded banner；
7. handoff 前 `git status --short` 只能有报告中明确列出的外部改动。

## 19. 停止状态

Round 19 最终只能输出：

```text
TARGET_HIT_PROVISIONAL_FUTURE_LOCK
FRONTIER_PROGRESS_ONLY
VALID_SEARCH_NO_FRONTIER_PROGRESS
BLOCKED_ENGINE_DATA_OR_EXECUTION
MATERIALLY_INCOMPLETE_INVALID_RESULTS
```

Family 局部停止不阻塞其他独立 family。以下任一立即停止相应 family：

- 不是真实 loss-after-add Martingale；
- 与可信历史 fingerprint 重复；
- 参数不改变 state/order/rejection；
- 5+ 实际 legs 无法执行；
- spurious/break/fit gate 失败；
- train strict gate 0 survivor；
- 两 validation folds 负或 principal breach；
- concentration >50%；
- partial fill/legging 无法保守建模；
- production restart/reconcile 不幂等。

禁止把 `MATERIALLY_INCOMPLETE_INVALID_RESULTS` 写成“新 family 已穷尽”；只有完整执行且证据有效时才允许
`VALID_SEARCH_NO_FRONTIER_PROGRESS`。
