# GLM Martingale Core Round 28：Corrected Concurrent + Asymmetric Martin 唯一任务书

日期：2026-07-26  
状态：`READY_TO_EXECUTE`  
执行分支：`glm-martingale-core-round28`

本文件是 Round 28 的唯一任务书。执行 agent 不得根据 outer return 增删参数、放宽门槛、追加 arm，
也不得用“收益仍为负”提前停止尚未完成的适用阶段。本轮不做 30 天监控，直接完成历史 prequential replay。

## 0. 必读权威与本轮边界

依次读取：

```text
docs/superpowers/reports/2026-07-26-chatgpt-round27-execution-audit-and-round28-direction.md
docs/superpowers/reports/2026-07-26-glm-round27-handoff.md
docs/superpowers/plans/2026-07-26-glm-martingale-core-round27-staged-admission-persistence-recovery-plan.md
docs/superpowers/artifacts/glm-martingale-core-round27/round27-data-manifest.json
docs/superpowers/artifacts/glm-martingale-core-round27/round27-policy-manifest.json
```

必须继承：

```text
Round 27 source commit = aaa02fda4e588ced0359318465ee682c677ae00c
Round 27 source tree   = 42c605916e4419e4cd556125e82dc0609205f95f
Round 27 raw root      = artifacts-local/round27/aaa02fda4e588ced0359318465ee682c677ae00c
Round 27 D0 数据、152 weekly anchors、formation-only Top-20、模型快照和失败原始证据
```

不得继承：

```text
VALID_HISTORICAL_PREQUENTIAL_NO_PB
Round 27 G2/G3/account-validator PASS
Round 27 block PnL/concentration
Round 27 Copula/PBD family closure
Round 27 的 8 条 valid G2 failure 标签
```

Round 27 的修正审计状态固定为：

```text
MATERIALLY_INCOMPLETE_INVALID_G2_AND_FAMILY_CLOSURE
```

Round 28 只有两个研究目标：

1. 完整修复并重跑 source Copula Martin control；
2. 在修复后的同一账户引擎上测试 train-only TAR/MTAR 非对称 error-correction Martin。

## 1. 最终目标与共同硬约束

| 档位 | annualized return | max equity DD | cold starts | leverage cap |
|---|---:|---:|---:|---:|
| 保守 | `>=50%` | `<=10%` | `>=4/5` 正，另报 5/5 | `<=2x` |
| 平衡 | `>=90%` | `<=20%` | `>=4/5` 正，另报 5/5 | `<=3x` |
| 激进 | `>=100%` | `<=30%` | `>=3/5` 正，另报 5/5 | `<=4x` |

共同硬约束：

1. principal 严格 `<5000U`；`5000U` 不合法。
2. Martin group 是唯一 PnL engine。Copula/TAR/MTAR 只能控制 `FO/SO/TP/freeze/reduce/abort`。
3. 所有 pair intents 必须进入一个连续 shared cash/margin/equity/reserve account。
4. 禁止拼接、平均、缩放或叠加 finished equity curves。
5. 最终候选实际成交 `>=6` alts、`>=3` distinct alt-alt pairs。
6. BTC 仅是 C0 reference；BTC order/trade/position/margin/PnL 必须恒为 0。
7. completed signal 后的下一根可用 1m open 才可成交；1m high/low、funding、fee、slippage、maintenance
   和 legging 全部进入路径。
8. fit、universe、pair、direction、threshold、quota、budget 和 sizing 不得读取当前或未来 outer return。
9. no-fit、no-signal、censored、reject、cap conflict、breach、timeout 和失败 policy 全部进入 trial/failure ledger。
10. 收益目标只用于最终验收，不用于选择或追加试验。

## 2. 冻结 fingerprint、试验配额与禁止重复

### 2.1 R28-C0 corrected recovery

固定信号 population：

```text
frequency = 1h / 5m
formation = 21d
trading roll = 7d, weekly, 152 anchors
selector = RAW EG p<=0.05
entry alpha = 0.10 / 0.20
SO adverse step = 0.50 / 0.75 frozen sigma
weighting = BETA / EQUAL diagnostic
```

总计 `2 frequencies * 2 alpha * 2 SO * 2 weighting = 16` 个 baseline policies，全部预注册并重跑。
`BETA` 是 source-faithful control；`EQUAL` 是独立 ablation，仍计入 trial 分母，不得冒充来源实现。

每个 fingerprint 必须包含：

```text
R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-<weighting>
```

### 2.2 R28-T1 TAR/MTAR asymmetric error-correction

固定 model variants：`TAR`、`MTAR`。每个通过 G1 的 variant 只展开 SO step `0.50/0.75`，最多 4 个
baseline policies。entry、formation、lag、bootstrap、threshold 和 TP 没有其他邻居。

### 2.3 总配额

```text
C0 baseline trials = 16
T1 baseline trials <= 4
cross-family shared-account ensemble <= 1
G2 total <= 21
```

所有 G2 policies 都进入 DSR/PBO/SPA 的 visible trial population。G3 对**所有** P-B survivors 执行，
不得按 outer return 只挑最好者。

### 2.4 永久禁止重复

```text
普通 multiplier/spacing/TP 参数网格
ATR/ADX/EMA/RSI/Donchian
first-passage/hazard
Micro-Martingale/Integral TP
Soft-Martingale
funding standalone PnL
trend/breakout directional sleeve
curve allocator / finished-equity blending
旧 PC1/Johansen/VECM/Kalman/partial-cointegration exact fingerprints
单币 ANKR、旧 DGT
RL/DNN/LSTM/GA 或 outer-return threshold tuning
```

本轮不得新增 CMI、delayed-cointegration 或其他临时 control；未执行的新想法写入 handoff，不在看到本轮
收益后追加。

## 3. Git、授权与可恢复执行协议

### 3.1 分支与 push

从包含本任务书的计划提交开始：

```bash
git status --porcelain
PLAN_COMMIT="$(git rev-parse HEAD)"
git switch -c glm-martingale-core-round28 "$PLAN_COMMIT"
```

若分支已由同一计划提交创建，只允许 `git switch glm-martingale-core-round28` 并验证：

```bash
git merge-base --is-ancestor "$PLAN_COMMIT" HEAD
```

规则：

1. 本地 clean source commit 足以执行全部历史 replay；**replay 前不要求 push**。
2. 不得因 remote、GitHub 或 push 授权停在 G0/G1。
3. 用户已要求把本任务交给指定 agent 执行，最终单次 push 属于任务步骤；不要在聊天中再次询问口头授权。
4. 若平台弹出 tool-level approval，正常提交该审批；若被拒绝，仍完成所有本地阶段并记录 `PUSH_PENDING`。
5. 全部 validator、authority、handoff 和 final evidence commit 完成后，才执行一次：

```bash
git push -u origin glm-martingale-core-round28
```

### 3.2 Commit log

每个 commit body 必须同时包含：

```text
问题描述:
复现路径:
修复思路:
```

### 3.3 Immutable raw root 与 resume

```text
repo artifact = docs/superpowers/artifacts/glm-martingale-core-round28/
raw artifact  = artifacts-local/round28/<SOURCE_COMMIT>/
handoff       = docs/superpowers/reports/2026-07-26-glm-round28-handoff.md
```

`--resume` 只能跳过 source commit、source tree、data hash、policy hash、input hash 全相同且 terminal 的阶段。
任何实现 bug 修复都必须新建 source commit 和新 raw root；旧 root 保留并标记 `superseded_implementation`。

运行慢、内存不足或 push 失败不是改策略参数的理由。内存不足只允许 workers `8 -> 4 -> 2`，记录 retry。

## 4. 必须新增或修改的实现

建议最小文件集合：

```text
scripts/r28_fit_threshold.py
scripts/r28_validate_models.py
crates/r24-research/src/r28.rs
crates/r24-research/src/bin/r28_execute.rs
crates/r24-research/src/bin/r28_validate.rs
crates/r24-research/src/lib.rs
docs/superpowers/artifacts/glm-martingale-core-round28/round28-protocol.json
docs/superpowers/artifacts/glm-martingale-core-round28/round28-policy-manifest.json
docs/superpowers/artifacts/glm-martingale-core-round28/round28-source-map.json
```

保留 Round 27 代码和 artifacts 作为不可变失败证据；不要原地改写 Round 27 authority/report。允许抽取已有通用
engine API，但不允许复制一个与 production account 不同的简化收益模拟器。

## 5. D0：冻结数据与 calendar

使用同一个 `data/market_data_full.db`、funding DB、exchangeInfo 和 maintenance source，不得联网刷新。

必须独立验证并记录：

```text
market DB full SHA256、byte count、SQLite schema/version
每个 symbol 的 first/last/count/missing/duplicate/close-time contract
funding first/last/expected/actual/missing/duplicate
exchangeInfo source SHA256 + tick/step/minQty/minNotional
maintenance source/version/hash
```

若 Round 27 manifest 的 `sha256_bound_to_bytes=false`，Round 28 必须真正流式读取文件字节计算 SHA256，不能
只复制旧字段。

外层时间仍为 `[2023-07-01T00:00:00Z, 2026-05-29T16:00:00Z)`。12 blocks 固定为：

```text
0=2023Q3  1=2023Q4  2=2024Q1  3=2024Q2
4=2024Q3  5=2024Q4  6=2025Q1  7=2025Q2
8=2025Q3  9=2025Q4 10=2026Q1 11=2026Q2(partial)
```

实现公式：

```text
block = (year - 2023) * 4 + zero_based_quarter - 2
```

禁止 `.min(11)`/clamp；任何 `[0,11]` 外 timestamp 直接 validator failure。

weekly Top-20 继续只用 formation 内 completed `close*volume`，同分按 symbol asc。至少 12 eligible alts；
否则合法终态为 `VALID_DATA_GATE_NO_SEARCH`。

## 6. R0：先修复 Round 27 回放器

R0 recovery 未通过，不得进入 T1 fit 或发布收益结论。

### 6.1 Source-faithful Copula direction

固定映射：

```text
h_left <= alpha && h_right >= 1-alpha -> SHORT left / LONG right
h_left >= 1-alpha && h_right <= alpha -> LONG left / SHORT right
otherwise                              -> no new FO
```

neutral 仍为两个 conditional probability 都在 `[0.35,0.65]`。signal timestamp、`h_left/h_right`、alpha、
model hash、最终 side/mode 必须出现在 trace，account validator 由这些原始字段独立复算方向。

`BETA` 两腿以 frozen formation beta 的绝对值分配 gross：

```text
left gross  = group_gross * abs(beta_left)  / (abs(beta_left)+abs(beta_right))
right gross = group_gross * abs(beta_right) / (abs(beta_left)+abs(beta_right))
```

beta 必须 finite 且 `>0`；否则该 pair return-blind reject。filters resolve 后 gross mismatch `<=5%`。
`EQUAL` 固定 50/50，单独标识，不得共享结果字段。

### 6.2 删除 future-completed excursion

禁止先构造含未来 exit 的 `Excursion` 再决定入场。改为两个因果对象：

```text
SignalIntent { signal_ms, eligible_open_ms, pair_id, model_hash, direction, weights, deadline_ms }
ActiveMartinGroup { open-time frozen fields, fills, last_add_state, current deadline }
```

signal onset 一出现就写 intent。无论未来是否 neutral、是否跨 weekly roll、是否到数据末端，都不得删除。
open group 跨 roll 时继续使用 open-time frozen model；新 roll 只影响新 cycle。数据末端统一执行带成本强平并标记
`end_of_data_abort`。

### 6.3 Chronological concurrent scheduler

所有 pair/snapshot intents 在执行前按以下 key 合并：

```text
(eligible_open_ms, pair_id, model_hash, direction)
```

账户逐 1m 时间推进，禁止 per-excursion 完整回放。每个 minute 的固定顺序：

1. 对 minute open 前已有仓位应用 adverse high/low path 和 maintenance/liquidation；
2. 应用该 timestamp funding；
3. 处理 forced reduce/abort 和满足 all-in positive 的 neutral TP；
4. 处理 active groups 的 SO intents；
5. 处理新 FO intents；同 timestamp 按上述 key 排序；
6. 应用真实 filter/partial/legging/fee/slippage，更新 wallet/equity/reserve/margin；
7. 写完整 risk/account trace。

baseline `max_active_groups=3`。同一 pair 同时最多一个 active group；第四个 group、symbol/pair cap、reserve 或
margin 不足必须写 explicit rejection，不能静默跳过。严禁 `entry_ms < last_processed -> continue`。

### 6.4 完整 trace schema

每个 signal/order/fill/funding/mark/TP/SO/abort/reject 至少保存：

```text
event_id, timestamp, completed_signal_ms, eligible_open_ms, event_type,
policy_id, pair_id, group_id, model_hash, leg_id, symbol,
direction, side, position_mode, level, requested_qty, filled_qty,
open/fill/mark price, fee, slippage, funding,
wallet_before/after, equity, reserve_before/after, margin, maintenance,
calendar_block, rejection/close_reason
```

idle RLE 允许压缩，但展开后必须覆盖恰好 `1,531,680` 个连续 1m 风险点。任何缺分钟、重叠或倒序为 invalid。

## 7. T1：TAR/MTAR 非对称 error-correction Martin

来源：[Enders and Siklos, 2001](https://doi.org/10.1198/073500101316970395)。只继承 threshold
error-correction 机制，不继承其市场、收益或参数。

### 7.1 Formation 与确定性 orientation

```text
bars = completed 1h
formation = 42d (1008 bars when complete)
trading = next 7d
roll = weekly, same 152 anchors
universe = formation-only weekly Top-20 alts
pair orientation = y=min(symbol), x=max(symbol), lexical and never flipped by fit quality
```

对每个 alt-alt pair 在 formation 内拟合：

```text
log(y_t) = a + beta * log(x_t) + e_t
```

要求 100% coverage、finite `a/beta/e`、`beta>0`、非零 robust scale 和 filter feasibility。outer return
不能参与 orientation、fit 或 pair ranking。

### 7.2 冻结 TAR/MTAR 公式

固定一个 augmented lag：

```text
Delta e_t = rho_1 * I_t * e_(t-1)
          + rho_2 * (1-I_t) * e_(t-1)
          + gamma * Delta e_(t-1) + epsilon_t

TAR:  I_t = 1[e_(t-1) >= tau_e]
MTAR: I_t = 1[Delta e_(t-1) >= tau_d]
```

`tau_e/tau_d` 只从对应 formation variable 的 15%-85% trimmed distinct values 中选择最小 RSS；同 RSS 取
数值较小 threshold。禁止用 outer path 重估 threshold 或 lag。

对每个 finite fit 使用 formation-only moving-block bootstrap：

```text
replications = 199
block length = 24 completed 1h bars
seed = first 64 bits of SHA256(snapshot_id || pair_id || TAR/MTAR)
null = rho_1 = rho_2 = 0
```

每个 snapshot/variant 对 bootstrap p-values 做 BH `q=0.05`。候选至少满足：

```text
joint-null BH pass
至少一个 regime 的 rho in (-1,0)
该 regime bootstrap sign support P(rho<0) >=0.90
该 regime half-life = -ln(2)/ln(1+rho) in [2h,168h]
另一 regime rho <=0.05；若 rho>=0 则标记 unreliable，绝不允许该 regime FO/SO
```

asymmetry Wald p-value 只用于 formation rank，不作额外 hard AND。每个 roll/variant 用 exact
maximum-cardinality maximum-weight disjoint matching，最多 3 pairs；rank tuple 固定为：

```text
1. reliable regimes 数量降序
2. joint bootstrap p-value 升序
3. reliable regime 最慢 half-life 升序
4. asymmetry Wald p-value 升序
5. projected edge/all-in cost 降序
6. minimum formation liquidity 降序
7. pair_id 升序
```

projected edge 只用 formation prices、frozen beta、从 entry `|z|=2.0` 到 neutral `|z|=0.25` 的确定性
move 和 2x fee/slippage/funding/legging estimate；要求 edge/cost `>=2`。禁止 formation pseudo-PnL。

### 7.3 Martin order semantics

formation residual center 和 scale 固定为 median 与 `1.4826*MAD`：

```text
z_t = (e_t - formation_median) / frozen_scale
upper entry: z>=+2.0 -> SHORT y / LONG x
lower entry: z<=-2.0 -> LONG y / SHORT x
neutral: abs(z)<=0.25
```

TAR 使用当前 residual threshold regime；MTAR 使用当前 momentum threshold regime。只有当前 regime 满足
第 7.2 节 reliability 才可 FO。

两腿 raw gross weights 为 `1 : abs(beta)`，normalize 后过 exchange filters。每个 SO 必须同时满足：

```text
group all-in net <0
距 last executed paired fill 又 adverse 0.50/0.75 frozen scale
当前仍是同一 tail direction
当前 TAR/MTAR regime 仍 reliable
next layer > previous resolved layer
reserve/filter/concentration/leverage pass
```

若进入 unreliable regime，立即 `freeze_so=true`；不得反向开独立仓位。TP 仅在 neutral 且 all-in group net >0。
deadline 固定为 `min(7d, ceil(3 * entry-regime half-life))`，到期带成本 abort。

## 8. G0：不可跳过的 recovery 与机制 canaries

所有 canary 在任何完整收益 replay 前通过。

### 8.1 必过 synthetic/unit tests

```text
copula_table4_low_high_is_short_left_long_right
copula_table4_high_low_is_long_left_short_right
beta_weighted_gross_normalizes_and_filter_mismatch_is_bounded
last_bar_open_signal_is_not_dropped_without_future_neutral
future_points_after_signal_do_not_change_prior_intent
three_overlapping_pairs_create_three_active_groups_and_fourth_is_rejected
concurrent_scheduler_order_hash_differs_from_serial_scheduler
calendar_blocks_map_2023q3_to_0_and_2026q2_to_11_without_clamp
trace_contains_reconstructable_side_mode_qty_price_and_wallet_fields
mutating_side_mode_qty_price_funding_wallet_or_block_fails_validator
synthetic_pb_survivor_executes_real_g3_tier_budget_matrix_without_bail
tar_fixture_recovers_upper_lower_adjustment
mtar_fixture_recovers_momentum_regimes
future_shift_changes_only_snapshots_after_fit_cutoff
unreliable_threshold_regime_emits_no_fo_and_freezes_so
all_t1_orders_are_owned_by_martin_groups
btc_reference_generates_zero_orders_positions_margin_and_pnl
```

现有 Round 24-27 engine/reserve/filter/Copula/matching/restart tests 全部保留；不得删除、改名、`ignored` 或
放宽 assertion 逃避。

### 8.2 Real-data canary

对 C0 1h beta alpha 0.10 和每个 T1 variant，自动使用第一个按 `(anchor,pair_id)` 排序的 fitted pair。
保存 first signal 或 explicit no-signal、完整 causal intent、next-1m orders、filter quantities、direction proof、
账户 reconciliation 和 BTC-zero proof。

真实 canary 无 signal 是该 config 的显式 activation evidence，不是实现 blocker；synthetic canary 或 trace
schema 失败才是工程错误，必须修复后用新 source commit 重跑。

### 8.3 Recovery gate

生成 `gates/r0-recovery.json`，同时满足才可继续：

```text
所有 8.1 tests pass
Round 27 C0 snapshots/hash 可读取且没有被改写
两个 direction cases 独立 validator pass
open-censored intent count >= completed-excursion intent count
每个原 signal onset 要么成为 intent，要么有因果 reject reason
synthetic concurrent max_active_groups = 3
calendar 12-block canary pass
G3 survivor fixture terminal
```

## 9. G1：Return-blind activation census

C0 两个 frequency 的 RAW configs 已因实现污染而强制重开，16 个 policy 全部进入 G2，不按活动数量挑选。

T1 TAR/MTAR 各自完成 152/152 snapshots，并分别检查：

```text
distinct fitted alts across timeline >=6
distinct exact matched pairs >=3
rolls with >=1 exact pair >=12
causal signal onsets including censored >=60
signals distributed across >=8/12 calendar blocks
no symbol contributes >50% signal onsets
model validator passed
```

门槛只读 fit/signal activity，不读 account return。variant 未通过时写首次失败门和完整 denominator；另一个
variant 继续。不得在看到无 activation 后改 formation、entry z、bootstrap q 或 threshold trim。

## 10. G2：Baseline Martin shared-account replay

每个 policy 使用：

```text
principal = 2000U
FO group gross = max(filter-feasible minimum, 5% principal)
relative layers = [1.00,1.25,1.55,1.90]
max active groups = 3
leverage cap = 2x
one continuous account over all 12 blocks
```

block 边界绝不重置 wallet、positions、groups、reserve 或 running equity peak。每个 policy 的 signal intents
先 chronological merge，再由同一个 engine 执行；禁止按 pair 分账户后合并。

P-A：

```text
model/recovery/account validators pass
trace 可独立重建，无 lookahead/BTC order/unreconciled field
所有 signal onset 都有 intent/order/reject terminal
终态 positions/groups/pending/reserve = 0
```

P-B：

```text
compounded return >0
positive calendar blocks >=8/12
closed Martin cycles >=30
actual traded alts >=6
actual distinct pairs >=3
loss-after-add SO groups >=2
symbol/pair/group/block positive contribution each <=50%
all-in cost/gross positive PnL <=50%
no liquidation or principal breach
```

0 P-B 仍必须完整记录所有 16 C0 和适用 T1 policies，不得只汇报最好项。

### 10.1 唯一可选 cross-family ensemble

只有 C0 与 T1 各至少一个 P-B 时，生成一个 ensemble。每个 family survivor 按下列 return-blind priority 取
第一个，不按收益排序：

```text
C0: 1h-BETA-A0.10-SO0.50, 1h-BETA-A0.10-SO0.75,
    1h-BETA-A0.20-SO0.50, 1h-BETA-A0.20-SO0.75,
    5m-BETA..., then EQUAL in same order
T1: MTAR-SO0.50, TAR-SO0.50, MTAR-SO0.75, TAR-SO0.75
```

两个 family intents 在执行前合并；一个 shared account；各占 50% ex-ante reserve quota；未用额度保留现金，
不能在看到 PnL 后转移。ensemble 重新跑全部 G2/G3，不允许组合 finished curves。

## 11. G3：三档、budgets、cold starts 与实盘压力

**G3 必须在 G2 前实现并由 synthetic P-B fixture 证明。** 不允许再次保留 `bail!(requires tier replay)`。

对所有 P-B survivors 和适用 ensemble 执行：

```text
conservative: FO=5.0%,  cap=2x
balanced:     FO=7.5%,  cap=3x
aggressive:   FO=10.0%, cap=4x

budgets: 500 / 750 / 1000 / 1500 / 2000 / 3000 / 4000 / 4999U
cold starts: 0 / 30 / 60 / 90 / 120d
```

每个 tier/budget/cold-start 都重新执行 orders、filters、reserve、funding 和 1m risk path，禁止缩放 2000U
curve。目标命中至少要求两个相邻固定 budgets 通过。另报：

```text
filter/reserve 解析得到的 theoretical minimum executable principal
固定 budget 网格中命中该 tier 的最小 principal
```

不得通过看收益后扫描任意本金来制造“精确最低本金”。

stress 固定为：

```text
fee/slippage/funding = 1.5x and 2x
partial fill = 25% and 50%
leg delay = 1/2/3 bars
one-leg reject + immediate paired flatten
minNotional = 2x
tick/step = one level coarser
maintenance = +5%
signal missing/stale
kill/restart/reconcile
worst combined stress
```

任一 target claim 必须给 base、每个单项 stress 和 worst combined stress 的 ann/DD/final reconciliation。

## 12. 防过拟合验收

对完整冻结 trial population，而不是 selected subset，计算：

```text
CSCV/PBO，hard gate PBO <0.50
Deflated/Probabilistic Sharpe，使用 Round 1-28 可见 trial floor，hard gate probability >=0.95
White/SPA-style reality check，hard gate p<=0.10
stationary moving-block bootstrap ann 95% lower bound >0
12 calendar-block returns，>=8 positive
leave-one-symbol-out 与 leave-one-pair-out：至少 80% runs compounded return >0
SO 0.50/0.75 frozen neighbor 均不得出现符号翻转后只选正者
五个 cold starts 按第 1 节通过
```

TAR 与 MTAR、C0 BETA 与 EQUAL、所有 no-fit/no-signal/reject/failed policies 都计入 trial denominator。
不得把 symbols、pairs、cold starts 当成独立“成功试验”扩大样本量。

另报进展门，但不得冒充目标：

```text
P-C = ann>=35%, DD<=20%, >=4/5 cold starts positive,
      actual alts>=6, pairs>=3, P-A/P-B/anti-overfit/stress/account gates pass
```

## 13. 双独立 validator

### 13.1 Model validator

`scripts/r28_validate_models.py` 必须从 raw DB 和 frozen cutoffs 独立复算，不调用 fit script 的 statistic
函数：

```text
Top-20、coverage、cutoff、RAW EG/Copula marginals/family/AIC
Table 4 direction mapping 与 beta weights
TAR/MTAR OLS residual、trim threshold、rho/gamma、half-life
bootstrap seeds/p-values、BH、sign support、Wald、cost gate
exact matching optimum、snapshot/model/source hashes
future-shift causality canary
```

至少 `checked_copula_pairs>0`；T1 有 fitted pair 时 `checked_threshold_pairs>0`。容差和独立 reference
implementation 写入 manifest，不能把 production output 原样比较自身。

### 13.2 Account validator

`r28_validate` 只读 raw signal/order/fill/risk traces，从零重建：

```text
direction/side/mode、filter-resolved quantity、order/fill/reject
concurrent group ownership、reserve/margin、wallet/equity
fee/slippage/funding/legging/partial/reject flatten
FO/SO/TP/freeze/abort、calendar block PnL
symbol/pair/group/block/family concentration
1m adverse DD、maintenance/liquidation、annualization
budget/cold-start/stress、restart/reconcile、final zero state
P-A/P-B/P-C 与三档 target
```

validator 不能调用 production metric/replay functions。至少建立 8 个 deterministic trace mutants，分别篡改
`side`、`mode`、`qty`、`fill price`、`funding`、`wallet`、`timestamp`、`calendar block`；每个 mutant 都必须
令 validator 非零退出。删除一个 overlapping intent 也必须失败。

authority 只能由两个 validator 和 immutable traces 生成；手写 report 不得覆盖 machine status。

## 14. 唯一执行顺序与命令

### 14.1 实现、测试、冻结 source commit

```bash
python3 -m py_compile \
  scripts/r28_fit_threshold.py \
  scripts/r28_validate_models.py

cargo fmt --package r24-engine --package r24-research -- --check
cargo test -p r24-engine -p r24-research
cargo build --release -p r24-research --bin r28_execute --bin r28_validate
git diff --check

git add scripts/r28_fit_threshold.py scripts/r28_validate_models.py \
  crates/r24-research/src/r28.rs \
  crates/r24-research/src/bin/r28_execute.rs \
  crates/r24-research/src/bin/r28_validate.rs \
  crates/r24-research/src/lib.rs
git add -f docs/superpowers/artifacts/glm-martingale-core-round28
git commit -m "fix(r28): restore causal concurrent Martin replay" \
  -m "问题描述: Round 27 反转 Copula 方向、按未来退出筛选入场并串行跳过重叠 pair，且 validator/G3 不完整。" \
  -m "复现路径: 对照 Table 4 检查 r27 signal-to-side 映射，并重放周末 open signal、重叠 intents、12 calendar blocks 和 synthetic P-B survivor。" \
  -m "修复思路: 冻结正确方向与 beta sizing，改为逐分钟并发 scheduler，补全可重建 trace、独立 validator、TAR/MTAR 和真实 G3。"

test -z "$(git status --porcelain)"
SOURCE_COMMIT="$(git rev-parse HEAD)"
RAW_ROOT="artifacts-local/round28/${SOURCE_COMMIT}"
mkdir -p "$RAW_ROOT"
```

若 source commit 后修改任何 replay/model/validator 代码，不得继续使用旧 `RAW_ROOT`。

### 14.2 D0 与 recovery gate

```bash
target/release/r28_execute \
  --phase d0 --artifact-root "$RAW_ROOT" --resume

target/release/r28_execute \
  --phase r0-recovery-g0 --artifact-root "$RAW_ROOT" --resume

OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 \
python3 scripts/r28_validate_models.py \
  --phase recovery-c0 --artifact-root "$RAW_ROOT" --workers 8 --resume

target/release/r28_validate \
  --phase recovery-g0 --artifact-root "$RAW_ROOT" --resume
```

`gates/r0-recovery.json` 非 PASS 时只能修工程错误、创建新 source commit 并从 D0 重跑；不得进入 T1。

### 14.3 T1 fit 与 activation

```bash
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 \
python3 scripts/r28_fit_threshold.py \
  --phase tar-mtar --artifact-root "$RAW_ROOT" --workers 8 --resume

target/release/r28_execute \
  --phase g1-activation --artifact-root "$RAW_ROOT" --resume

OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 \
python3 scripts/r28_validate_models.py \
  --phase all-models --artifact-root "$RAW_ROOT" --workers 8 --resume
```

### 14.4 G2/G3 与 final validators

```bash
target/release/r28_execute \
  --phase g2-replay --artifact-root "$RAW_ROOT" --resume

# 命令总是执行：0 P-B 时写 not_applicable terminal；有 P-B 时必须真实跑完整 matrix
target/release/r28_execute \
  --phase g3-tiers --artifact-root "$RAW_ROOT" --resume

target/release/r28_validate \
  --phase all --artifact-root "$RAW_ROOT" --resume

target/release/r28_validate \
  --phase finalize --artifact-root "$RAW_ROOT" --resume
```

执行过程中每个阶段写 `runtime/*.json`：argv、PID、start/end UTC、wall seconds、peak RSS、workers、BLAS
threads、exit code、resume、source/data/policy/input hashes。

### 14.5 Evidence commit 与最终 push

```bash
git add -f docs/superpowers/artifacts/glm-martingale-core-round28
git add docs/superpowers/reports/2026-07-26-glm-round28-handoff.md
git commit -m "docs(r28): publish corrected concurrent Martin evidence" \
  -m "问题描述: Round 28 需要从 immutable traces 给出 corrected Copula 与 TAR/MTAR 的完整目标判定和失败边界。" \
  -m "复现路径: 依次核对 recovery、activation、G2、G3、双 validator、mutants、trial ledger、12 blocks 和 final zero state。" \
  -m "修复思路: 提交 compact manifests、机器 authority、全量失败台账和 handoff，raw traces 保留在 source-commit root。"

test -z "$(git status --porcelain)"
git push -u origin glm-martingale-core-round28
```

## 15. 必交 artifacts

```text
round28-authority.json
round28-execution-state.json
round28-protocol.json
round28-policy-manifest.json
round28-source-map.json
round28-data-manifest.json
round28-trial-ledger.json
round28-failure-ledger.jsonl
round28-closed-fingerprints.json
round28-reopened-round27-fingerprints.json
gates/d0.json
gates/r0-recovery.json
gates/g1-activation.json
gates/g2-replay.json
gates/g3-tiers.json
model-independent-validator.json
account-independent-validator.json
validator-mutants.json
fit-snapshot-manifests/c0-recovery.json
fit-snapshot-manifests/tar.json
fit-snapshot-manifests/mtar.json
signal-intent-manifests/*.json
trace-manifests/*.json
replay-results/*.json
tier-results/*.json
runtime/*.json
```

raw traces、完整 bootstrap samples 和逐分钟展开数据留在 `artifacts-local/`。仓库只提交 compact rows、hash、
schema、first/last、metrics 和 validator outputs；单文件 `<10MB`，禁止提交大 blob。

## 16. Failure ledger 与禁止错误 closure

每个失败记录至少包含：

```text
failure_id, round, phase, exact fingerprint, source/data/policy hash,
first_failed_gate, numerator, denominator, metrics, trace hash,
causal reason, valid/invalid implementation status, never-repeat rule
```

规则：

1. Round 27 的 8 个错误 G2 policies 保留为 `invalid_implementation_evidence`，不删除、不改写为有效负结果。
2. 只有 Round 28 recovery validator 全过后，corrected exact fingerprint 的负结果才可记 `valid_failure`。
3. TAR/MTAR 某一 variant 失败只能关闭其完整 frozen fingerprint，不能关闭所有 threshold cointegration。
4. 0 P-B 只能写“本轮预注册 population 无 P-B”，不能写“所有 Martin 可能性耗尽”。
5. 任何 implementation/validator failure 都必须修复并重跑，不能伪装成策略失败。

## 17. 合法终态与完成定义

只允许：

```text
VALID_DATA_GATE_NO_SEARCH
VALID_RECOVERY_AND_T1_NO_ACTIVATION
VALID_HISTORICAL_PREQUENTIAL_NO_PB
VALID_FRONTIER_PROGRESS_NO_TARGET
VALID_CONSERVATIVE_TARGET
VALID_BALANCED_TARGET
VALID_AGGRESSIVE_TARGET
INVALID_IMPLEMENTATION_OR_VALIDATOR_FAILURE
```

`INVALID_IMPLEMENTATION_OR_VALIDATOR_FAILURE` 不是可交付完成状态；可修复时必须继续修复。Round 28 只有同时
满足以下条件才算完整：

```text
D0 terminal
R0 recovery PASS
C0 16/16 baseline policy terminals
TAR/MTAR 各 152/152 activation terminals
所有适用 T1 G2 terminals
G3 code 已由 survivor fixture 验证，真实 P-B 的全部 tier/budget/cold-start/stress terminals
model/account validators PASS
全部 validator mutants 被拒绝
authority、failure ledger、handoff、source/final commits 完整
push succeeded 或在所有本地工作完成后明确 PUSH_PENDING
```

## 18. Handoff 必须逐项回答

1. Round 27 七个阻断问题是否逐项修复，测试和 raw proof 在哪里；
2. corrected C0 的 16 个 policy 全表：ann、DD、12 blocks、FO/SO/TP/abort/reject、cost、concentration；
3. concurrent scheduler 的 eligible intents、executed、各类 reject、max active groups 和 overlap 统计；
4. TAR/MTAR 的每阶段 denominator、BH pass、matched pairs、signals、首次失败门；
5. 每个 P-B/P-C/target candidate 的实际币种、pair、方向规则、beta/weight、杠杆和 Martin layers；
6. 三档每个 budget、5 cold starts、每项 stress、anti-overfit 和 exact executable principal；
7. 全部失败 fingerprint 与 never-repeat reason；
8. 是否存在真实 chronological shared-account cross-family ensemble；
9. source/data/policy/trace/validator hashes、source/final commit 和 push 状态；
10. 明确写 `strict_valid_candidates`、`target_hit`，不得用“最佳”“完成”替代机器门槛。
