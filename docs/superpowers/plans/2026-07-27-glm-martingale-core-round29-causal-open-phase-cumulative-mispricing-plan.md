# GLM Martingale Core Round 29：Causal Open-Phase + Cumulative Mispricing 唯一任务书

日期：2026-07-27
状态：`READY_TO_EXECUTE`
执行分支：`glm-martingale-core-round29`

本文件是 Round 29 的唯一任务书。执行 agent 不得根据 outer return 增删参数、放宽门槛、追加 arm，
不得把 Round 28 的失效 G2/G3 当 parent，也不得因收益低、运行慢或 push 未授权提前结束适用阶段。

本轮不等待未来数据、不创建 30 天监控，直接完成历史 prequential replay。历史窗口已被多轮读取，最终结果只能
称 historical prequential evidence，不能包装成 untouched OOS。

## 0. 必读权威与继承边界

依次读取：

```text
docs/superpowers/reports/2026-07-27-chatgpt-round28-execution-audit-and-round1-28-authority.md
docs/superpowers/reports/2026-07-26-glm-round28-handoff.md
docs/superpowers/plans/2026-07-26-glm-martingale-core-round28-corrected-concurrent-asymmetric-plan.md
docs/superpowers/reports/2026-07-26-chatgpt-round27-execution-audit-and-round28-direction.md
docs/superpowers/artifacts/glm-martingale-core-round28/round28-data-manifest.json
docs/superpowers/artifacts/glm-martingale-core-round28/round28-policy-manifest.json
```

Round 28 原始证据：

```text
source commit = d516d40dafbac63d0c9d607224cae8de9721a2ce
source tree   = 998fa2b9745ee9c0d79118daaf92c3d21e5b6f8e
raw root      = artifacts-local/round28/d516d40dafbac63d0c9d607224cae8de9721a2ce
```

允许继承：

```text
D0 数据 bytes/hash/schema/coverage 与 exchange filters/maintenance source
12 calendar block mapping
formation-only C0/TAR/MTAR snapshots、model hashes 与 signal denominator
TAR/MTAR activation census 的 exact scoped failure
SharedAccount reserve/filter/pending/restart 基础设施及已通过的单元测试
Round 28 原始 traces 作为 invalid implementation evidence
```

禁止继承：

```text
VALID_FRONTIER_PROGRESS_NO_TARGET
Round 28 两个 P-B
Round 28 全部 G2/G3 ann/DD/budget/cold-start/stress 排名
Round 28 P-A/P-B/P-C、minimum principal、target claim
Round 28 16 个 valid failure 标签或 C0 family closure
```

Round 28 的唯一修正状态：

```text
MATERIALLY_INCOMPLETE_INVALID_G2_G3_AND_FRONTIER_CLOSURE
strict_valid_candidates = 0
target_hit = false
```

## 1. 用户目标与共同硬约束

| 档位 | annualized return | max equity DD | cold starts | leverage cap |
|---|---:|---:|---:|---:|
| 保守 | `>=50%` | `<=10%` | `>=4/5` 正，另报 5/5 | `<=2x` |
| 平衡 | `>=90%` | `<=20%` | `>=4/5` 正，另报 5/5 | `<=3x` |
| 激进 | `>=100%` | `<=30%` | `>=3/5` 正，另报 5/5 | `<=4x` |

共同硬约束：

1. principal 严格 `<5000U`；`5000U` 不合法。
2. Martin group 是唯一 PnL engine；C0/MPI 只能控制 `FO/SO/exit/freeze/abort`。
3. 所有 pair intents 必须进入一个连续 shared cash/margin/equity/reserve account。
4. 禁止拼接、平均、缩放、平移或叠加 finished equity curves。
5. 最终候选实际成交 `>=6` alts、`>=3` distinct alt-alt pairs。
6. BTC 只能作 reference；BTC order/trade/position/margin/PnL 必须恒为 0。
7. completed signal 后下一根可用 1m open 才可成交；同分钟未来字段不得进入 open decision。
8. 1m high/low、funding、fee、slippage、filter、maintenance、partial fill、legging 与 liquidation 全部入账。
9. fit、pair、direction、quota、budget、sizing、阈值和 family 选择不得读取当前或未来 outer return。
10. no-fit/no-signal/censored/reject/conflict/breach/timeout/失败 policy 全部进入 trial/failure ledger。
11. 收益目标只用于最终验收，绝不用于选择、提前停止或追加试验。
12. 任何 target claim 必须由 raw trace 经独立 validator 生成；手写报告无权覆盖 machine authority。

## 2. 冻结研究问题、fingerprint 与试验配额

Round 29 只回答两个问题：

1. 修复分钟事件语义后，Round 28 C0 16-policy population 的真实结果是什么；
2. 固定来源规则的 cumulative Copula Mispricing Index 能否让多 pair Martin 获得更持续、分散的 recovery cycles。

### 2.1 R29-C0 mandatory recovery

原样重跑 Round 28 的 16 个 C0 baseline：

```text
frequency = 1h / 5m
formation = 21d
trading roll = 7d, weekly, 152 anchors
selector = RAW EG p<=0.05
entry alpha = 0.10 / 0.20
SO adverse step = 0.50 / 0.75 frozen sigma
weighting = BETA / EQUAL
```

总数固定：

```text
2 frequency * 2 alpha * 2 SO * 2 weighting = 16 policies
```

fingerprint 必须包含：

```text
R29-OPENPHASE-FAMILY-ADVERSE-ENTRYBAR-CALENDAR12-<weighting>
```

不得删除负 policy，不得只重跑 Round 28 的两个原 P-B，也不得复用失效 trace。

### 2.2 R29-MPI cumulative mispricing

固定 8 个 baseline：

```text
frequency = 1h / 5m
marginal  = EMPIRICAL / LAPLACE
weighting = EQUAL / BETA
D = 0.60
S = 2.00
Martin relative layers = [1.00,1.25,1.55,1.90]
```

总数固定：

```text
2 frequency * 2 marginal * 2 weighting = 8 policies
```

fingerprint 必须包含：

```text
R29-CUMULATIVE-MPI-D0.60-S2.00-OPENPHASE-ENTRYBAR-<marginal>-<weighting>
```

MPI 使用与 C0 相同的 formation-only weekly pair set，不重新按收益选择 pair。1h/5m 是明确的 crypto
time-scale ablation，不得冒充论文的 daily source-faithful 复现。

### 2.3 总配额与 visible trial floor

```text
C0 baseline G2 = 16
MPI baseline G2 = 8
optional shared-account C0+MPI ensemble <=1
G2 total <=25
```

Round 1-28 已公开 trial floor 固定为 `1145`。Round 29 baseline 后：

```text
visible trial floor = 1145 + 24 = 1169
若执行唯一 ensemble，则 =1170
```

所有 24 个 baseline，包括 no-signal、负收益和 invalid-terminal policies，均进入 PBO/DSR/SPA 的可见试验
分母。budget/cold-start/stress 是同一 policy 的验证路径，不可伪装成独立 alpha trials。

### 2.4 永久禁止重复

```text
普通 multiplier/spacing/TP/SL 参数网格
ATR/ADX/EMA/RSI/Donchian
first-passage/hazard
Micro-Martingale/Integral TP
Soft-Martingale/SEL
funding standalone PnL
trend/breakout directional sleeve
finished-curve allocator/blending
旧 PC1/Johansen/VECM/Kalman/partial-cointegration exact fingerprints
旧 TAR/MTAR exact activation gate
单币 ANKR、旧 DGT
RL/DNN/LSTM/GA 或 outer-return threshold tuning
dynamic/mixed/vine copula family expansion
```

本轮不得在看到结果后追加 skew-Laplace、GARCH、更多 copula family、更多 D/S、更多 frequency 或新的 SO
spacing。新想法只写 handoff hypothesis ledger。

## 3. Git、授权与可恢复执行协议

### 3.1 分支创建

从包含本任务书的 plan commit 开始：

```bash
git status --short --branch
test -z "$(git status --porcelain)"
PLAN_COMMIT="$(git rev-parse HEAD)"
git switch -c glm-martingale-core-round29 "$PLAN_COMMIT"
```

若执行分支已存在，只允许：

```bash
git switch glm-martingale-core-round29
git merge-base --is-ancestor "$PLAN_COMMIT" HEAD
```

规则：

1. clean local plan/source commit 足以执行全部 replay；**执行前不要求 push，也不要求 pushed parent**。
2. 不得因 GitHub、remote、push 授权或网络问题停在 R0/G1。
3. 最终 push 是交付步骤；平台若弹出 tool-level approval，正常请求。
4. 若审批被拒绝，仍完成全部本地 replay、validator、authority、handoff 和 evidence commit，记录
   `PUSH_PENDING_TOOL_APPROVAL`。
5. 不得把 push 失败写成数据门、策略门或回测失败。

### 3.2 Commit log

每个 commit body 必须同时包含：

```text
问题描述:
复现路径:
修复思路:
```

### 3.3 Immutable raw root 与 resume

```text
repo artifact = docs/superpowers/artifacts/glm-martingale-core-round29/
raw artifact  = artifacts-local/round29/<SOURCE_COMMIT>/
handoff       = docs/superpowers/reports/2026-07-27-glm-round29-handoff.md
```

`--resume` 只可跳过 source commit、source tree、data hash、policy hash、input hash 全相同且 status 为 terminal
的阶段。修改 replay/model/validator 代码必须新建 source commit 与 raw root；旧 root 保留并标记
`superseded_implementation`。

内存不足只允许 workers `8 -> 4 -> 2` 并记录 retry；不得改试验配额、频率或模型。运行慢不是提前停止理由。

## 4. 最小实现范围

建议新增：

```text
scripts/r29_fit_mpi.py
scripts/r29_validate_models.py
crates/r24-research/src/r29.rs
crates/r24-research/src/bin/r29_execute.rs
crates/r24-research/src/bin/r29_validate.rs
docs/superpowers/artifacts/glm-martingale-core-round29/round29-protocol.json
docs/superpowers/artifacts/glm-martingale-core-round29/round29-policy-manifest.json
docs/superpowers/artifacts/glm-martingale-core-round29/round29-source-map.json
```

允许修改：

```text
crates/r24-engine/src/lib.rs
crates/r24-research/src/lib.rs
```

优先抽取/复用 Round 28 通用模型、filters、account 与 trace API，不复制第二套简化收益引擎。Round 28 代码和
artifacts 保持不可变历史证据；不要原地重写 Round 28 authority/report。

## 5. 外部来源冻结

### 5.1 Cumulative MPI

```text
title = Pairs Trading with Copulas
SSRN DOI = 10.2139/ssrn.2383185
journal DOI = 10.3905/jot.2016.11.3.041
source URL = http://www.efmaefm.org/0EFMAMEETINGS/EFMA%20ANNUAL%20MEETINGS/2014-Rome/papers/EFMA2014_0222_fullpaper.pdf
PDF SHA256 = c2b3cffbb30de9fcb81f1643a1690cc871e3c1c85ba9165086ee84585b8fb7e4
formula pages = PDF p.11 Eq.2.2, p.12 Eq.2.3
rule page = PDF p.14
```

本任务书已冻结完整公式和规则。执行时下载源失败不阻塞历史 replay，但 source manifest 必须记录 URL、预期
hash、下载状态和本任务书页码，禁止凭记忆改公式。

### 5.2 Laplace ablation

```text
DOI = 10.3390/math10050783
source URL = https://www.mdpi.com/2227-7390/10/5/783/pdf?version=1646113708
PDF SHA256 = 453d02279818342230da3d675e1fe064bbc58017e0b36840c21ad1acdc235a6e
adopted scope = formation-only Laplace marginal MLE + PIT
rejected scope = single-pair return、negligible cost、one-day hold、paper performance
```

下载失败按 5.1 的同一规则处理：记录 URL、预期 hash 和失败状态，使用本任务书冻结公式继续执行，不得改用
搜索结果中的其他 Laplace 变体。

## 6. D0：冻结数据与 calendar

使用与 Round 28 完全相同的 local inputs，不联网刷新行情：

```text
data/market_data_full.db
data/funding_rates.db
Round 28 exchangeInfo/filter snapshot
Round 28 maintenance source
```

重新流式验证实际 bytes，不只复制旧字段：

```text
SHA256、byte count、SQLite schema/version
每 symbol first/last/count/missing/duplicate/close-time contract
funding first/last/expected/actual/missing/duplicate
filter tick/step/minQty/minNotional
maintenance source/version/hash
```

外层固定：

```text
[2023-07-01T00:00:00Z, 2026-05-29T16:00:00Z)
logical 1m rows = 1,531,680
```

12 blocks：

```text
0=2023Q3  1=2023Q4  2=2024Q1  3=2024Q2
4=2024Q3  5=2024Q4  6=2025Q1  7=2025Q2
8=2025Q3  9=2025Q4 10=2026Q1 11=2026Q2(partial)
```

禁止 clamp；越界 timestamp 直接 validator failure。D0 hash 与 Round 28 不同则合法终态为
`VALID_DATA_CHANGED_NO_REPLAY`，不得继续比较收益。

## 7. R0-A：强制 open-phase / bar-path chronology

R0 未通过，不得 fit MPI、不得执行任何 scored replay。

### 7.1 类型隔离

open decision 使用不含未来字段的类型，例如：

```rust
struct OpenSlice {
    timestamp: i64,
    open: f64,
}

struct CompletedMinutePath {
    timestamp: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
}
```

TP/SO/FO/deadline/pending-fill 函数只接受 `OpenSlice`。禁止把完整 `MinuteBar` 传入 open-phase helper 后靠
约定“不读取”未来字段。

### 7.2 每分钟唯一顺序

对 minute `t`：

```text
OPEN PHASE
O1. 读取 t.open；把进入本分钟以前已存在的仓位 mark 到 t.open
O2. 检查 open-time maintenance；若 breach，按 t.open 强平并终止
O3. 对进入 t 前持有的仓位结算 timestamp=t 的 funding
O3b. funding 改变 wallet 后立即重查 maintenance；若 breach，必须在任何 exit/SO/FO 前按 t.open 强平
O4. 处理此前提交、在 t 到期的 delayed pending orders
O5. 读取 completed_signal_ms < t.open_ms 的 frozen signal/model state
O6. forced deadline/source stop/source zero exit/neutral TP
O7. active group SO；同 timestamp 按 group_id asc
O8. new FO intents；按 (eligible_open_ms,pair_id,model_hash,direction) asc

BAR PATH PHASE
P1. 对 O4-O8 后全部持仓应用 t.high/t.low adverse path
P2. 更新 intraminute equity DD、maintenance 与 liquidation
P3. 若未终止，mark t.close
P4. 写 account/risk trace 与连续 running peak
```

同一 timestamp 新成交不得被 timestamp funding 追溯收费；funding 只作用于 O1 时已有仓位。所有订单成交价是
对应 leg 的 `t.open` 加冻结 slippage，不得使用 `t.close`。

### 7.3 Signal causality

每个 signal 保存：

```text
source_bar_open_ms
source_bar_close_ms
completed_signal_ms
eligible_open_ms
model_fit_cutoff_ms
```

必须满足：

```text
model_fit_cutoff_ms <= source_bar_open_ms
source_bar_close_ms == completed_signal_ms
eligible_open_ms > completed_signal_ms
fill_timestamp >= eligible_open_ms
```

对 1h/5m signal，只有整根 signal bar 完成后才能产生 intent；下一根可用 1m open 才可执行。

## 8. R0-B：family-specific adverse distance

禁止再次使用全局：

```rust
direction.sign() * (last_value - current_value)
```

每个 family 单独实现并由 independent fixtures 验证。

### 8.1 C0 residual-difference

```text
value = left_residual/sigma_left - right_residual/sigma_right

SHORT_LEFT_LONG_RIGHT adverse = last_add_value - current_value
LONG_LEFT_SHORT_RIGHT adverse = current_value - last_add_value
```

SO 仍要求 `adverse >= frozen so_step`，且 `last_add_value` 只在成功 paired SO fill 后更新；不能每根 bar 更新。

### 8.2 T1 z-spread regression fixture

T1 本轮不进入 scored G2，但必须保留正确公式防止共享代码回归：

```text
SHORT_LEFT_LONG_RIGHT at positive z: adverse = current_z - last_add_z
LONG_LEFT_SHORT_RIGHT at negative z: adverse = last_add_z - current_z
```

### 8.3 MPI driver flag

```text
positive driver: adverse progress = current_driver - last_add_driver
negative driver: adverse progress = last_add_driver - current_driver
```

driver 改符号或先回零时不得 SO；按第 12 节执行 source exit/reset。

## 9. R0-C：entry-minute adverse risk

O4-O8 产生的所有新仓必须进入同一分钟 P1/P2。至少验证：

```text
new long fill at open -> same-minute low 可触发 DD/maintenance/liquidation
new short fill at open -> same-minute high 可触发 DD/maintenance/liquidation
new paired fill -> 两腿各自 adverse extreme 同时进入 conservative path
SO fill -> 新增 quantity 在同分钟 extreme 上计价
one-leg delayed/rejected -> 裸腿同分钟 high/low 真实计价
```

engine `max_equity_drawdown_pct`、risk trace DD 与 independent validator 必须一致到 `1e-9`。禁止只在 risk writer
旁路重算而不更新账户强平状态。

## 10. R0-D：trace 与 phase contract

每个事件至少保存：

```text
event_id, sequence, timestamp, phase, event_type
policy_id, family, pair_id, group_id, model_hash, driver_id
completed_signal_ms, eligible_open_ms, source_bar_open_ms, source_bar_close_ms
leg_id, symbol, direction, side, position_mode, level
requested_qty, filled_qty, open_price, fill_price, mark_price
fee, slippage, funding, legging_loss
wallet_before/after, equity, running_peak, drawdown
reserve_before/after, initial_margin, maintenance
calendar_block, rejection_reason, close_reason
decision_input_hash, position_state_hash
```

phase 只允许：

```text
open_mark, funding, pending, exit, so, fo, adverse_path, close_mark, idle
```

idle RLE 展开后必须恰好覆盖 `1,531,680` 个连续 1m points。所有 signal onset 必须有一个 terminal intent outcome；
静默 `continue` 非法。

## 11. G0：不可跳过的 recovery canaries

### 11.1 必过单元/集成测试

至少新增：

```text
open_phase_type_exposes_no_high_low_close
current_close_mutation_cannot_change_same_minute_open_decisions
current_high_low_mutation_changes_risk_only_after_open_fills
tp_decision_uses_open_mark_not_current_close
so_decision_uses_open_mark_not_current_close
new_long_can_liquidate_on_entry_minute_low
new_short_can_liquidate_on_entry_minute_high
new_so_quantity_enters_entry_minute_adverse_path
c0_short_left_adverse_is_value_decrease
c0_long_left_adverse_is_value_increase
t1_short_left_adverse_is_z_increase
t1_long_left_adverse_is_z_decrease
mpi_positive_driver_adverse_is_further_positive
mpi_negative_driver_adverse_is_further_negative
last_add_value_changes_only_after_successful_paired_fill
funding_does_not_charge_order_filled_at_same_timestamp
funding_debit_can_liquidate_before_exit_or_new_fill
three_overlapping_pairs_remain_concurrent
deleting_actual_overlapping_intent_fails_validator
worst_combined_sets_second_leg_reject
leg_delay_delays_exactly_one_leg
entry_minute_risk_mutant_fails_validator
```

Round 24-28 既有 reserve/filter/restart/Copula/matching/calendar/BTC-zero tests 全部保留，不得删除、改名、
`ignored` 或放宽断言。

### 11.2 Synthetic canary

建立一个确定性 6-symbol/3-pair concurrent path，必须包含：

```text
3 overlapping FO
至少 1 C0 loss-after-add SO
至少 1 MPI FO -> SO1 -> SO2 -> zero exit
entry-minute adverse DD
timestamp funding
partial fill
single-leg delay
one-leg reject + paired flatten
deadline abort
restart/reconcile
final positions/groups/pending/reserve = 0
```

production executor 与 independent validator 对 FO/SO/exit/abort、wallet、cost、DD 和 final state 完全一致。

### 11.3 Real-data canary

使用按 `(anchor,frequency,pair_id)` 排序的第一个 C0 fitted pair，分别运行：

```text
C0-1h-BETA-A0.10-SO0.50
MPI-1h-EMPIRICAL-EQUAL
MPI-1h-LAPLACE-EQUAL
```

保存 first signal 或 explicit no-signal、open-phase field proof、direction/adverse proof、filter quantities、
entry-minute risk 与 BTC-zero proof。真实无 signal 不是工程失败；synthetic 或 chronology 失败必须修代码、创建新
source commit、从 D0 重跑。

## 12. MPI：冻结公式与 Martin 状态机

### 12.1 Formation 与 h-values

MPI 使用 C0 已冻结的 weekly pair IDs、fit cutoffs 和 BTC-reference residual定义：

```text
left_residual  = log(BTC) - a_left  - beta_left  * log(left)
right_residual = log(BTC) - a_right - beta_right * log(right)
```

`EMPIRICAL`：使用 formation-only sorted residual ECDF。
`LAPLACE`：对每条 formation residual 独立做固定 MLE：

```text
mu = sample median
b  = mean(abs(x-mu))
u  = LaplaceCDF(x;mu,b)
clip PIT to [1/(n+1), n/(n+1)]
```

`b<=0`、non-finite 或 coverage 不足写 no-fit。Laplace arm 在同一 formation 上只比较现有
Gaussian/Student-t copula AIC，tie 按 `gaussian < student_t`；不新增 copula family。pair set 不因 Laplace fit 或
outer return重新 matching。

对 completed signal bar：

```text
MI_left_t  = dC(u_t,v_t)/dv = h_left_t
MI_right_t = dC(u_t,v_t)/du = h_right_t
```

model validator 以独立 SciPy CDF/copula implementation 复算，不能调用 production helper。

### 12.2 Inactive flags

每个 `(pair_id,model_hash,marginal)` 保存：

```text
FlagLeft_0 = 0
FlagRight_0 = 0
FlagLeft_t  = FlagLeft_(t-1)  + (h_left_t  - 0.5)
FlagRight_t = FlagRight_(t-1) + (h_right_t - 0.5)
```

更新只发生在 completed 1h/5m signal bar。model roll 时：

- 无 active group：旧 flags 终止，新 model flags 从 0 开始；
- 有 active group：继续用 open-time frozen model 更新到 group 关闭/deadline，不切换 model；
- 同一 pair 不允许新旧 model 同时开 group。

### 12.3 FO 方向与冲突

从 threshold 内侧首次 crossing `D=0.60` 时：

```text
FlagLeft  crosses +D -> SHORT left / LONG right
FlagLeft  crosses -D -> LONG left  / SHORT right
FlagRight crosses +D -> LONG left  / SHORT right
FlagRight crosses -D -> SHORT left / LONG right
```

规则：

1. 同一 bar 只有一个 crossing：它成为 frozen `driver_id`。
2. 多个 crossing 给出同方向：固定 `FlagLeft` 优先，不能按 overshoot/PnL 选择。
3. 同一 bar 给出相反方向：不成交，写 `mpi_conflicting_crossing`，双 flags 立即 reset 为 0。
4. FO atomic reject：写因果 reason，双 flags reset 为 0，避免每根 bar 重复同一 stale intent。
5. 成功 FO 后，非 driver flag 继续记录但不控制该 active cycle。

### 12.4 固定 Martin SO 与 source exits

来源给出 `D=0.60`、`S=2.00`，但没有 Martin layers。Round 29 只允许一个确定性、无自由参数的映射：

```text
delta = (S-D)/4 = 0.35
FO threshold  = 0.60
SO1 threshold = 0.95
SO2 threshold = 1.30
SO3 threshold = 1.65
source stop   = 2.00
relative gross layers = [1.00,1.25,1.55,1.90]
```

正 driver 使用正 thresholds，负 driver 使用负 thresholds。每个 SO 同时要求：

```text
group all-in net <0
driver 未回零、未改符号
本层 threshold 首次越过
paired fill 原子成功或执行 paired flatten
reserve/filter/concentration/leverage pass
```

退出：

```text
driver crosses zero:
  close all legs at next eligible 1m open
  net>0 -> source_zero_tp
  net<=0 -> source_zero_loss_exit

abs(driver) reaches S on adverse side:
  source_stop_abort

age reaches 7d or END:
  deadline_abort / end_of_data_abort
```

所有 closure 后双 flags reset 为 0。source zero exit 即使 net<=0 也必须执行，不能只保留盈利退出。MPI 不得产生
任何非 Martin order。

### 12.5 Weighting

```text
EQUAL = 50/50 source control
BETA  = abs(beta_left) : abs(beta_right), normalized crypto adaptation
```

两者均过真实 filters。任何 resolved gross mismatch `>5%` 因果 reject，不允许扩大 group gross 到突破
leverage/reserve cap。

## 13. G1：Return-blind activation census

C0 和 MPI 分别输出：

```text
152/152 anchors terminal count
fitted/no-fit pair denominator
finite h-value rows
flag updates、D crossings、conflicts、S stops、zero exits
distinct eligible alts/pairs
signals by 12 blocks
max symbol/pair signal share
first failed activation gate
```

MPI activity diagnostic gate：

```text
distinct eligible alts >=6
distinct pairs >=3
causal unambiguous FO crossings >=30
crossings distributed across >=8/12 blocks
single symbol crossing share <=50%
```

**G1 不得因低 activity 跳过 G2。** 8 个 MPI policy 和 16 个 C0 policy 都必须进入 G2；零 signal policy 生成
完整 zero-order terminal/account trace。只有 D0 或 R0 工程 gate 失败才可停止本轮。

## 14. G2：24 个 baseline shared-account replay

每个 baseline 固定：

```text
principal = 2000U
FO group gross = max(filter-feasible minimum, 5% principal)
relative layers = [1.00,1.25,1.55,1.90]
max active groups = 3
leverage cap = 2x
one continuous account across all 12 blocks
```

block 边界绝不重置 wallet、positions、groups、reserve、flags 或 running equity peak；只有 MPI model roll 按
第 12.2 节处理。所有 intents 在执行前 chronological merge，禁止 per-pair 分账户或完整跑完一条 excursion
再处理下一条。

P-A：

```text
D0/R0/model/account validators pass
trace chronology/open-phase/entry-minute path pass
每个 signal onset 有 terminal intent/order/reject
logical risk rows 完整
final positions/groups/pending/reserve = 0
BTC-zero
no unreconciled wallet/cost/margin
```

P-B：

```text
P-A
compounded return >0
positive calendar blocks >=8/12
closed Martin cycles >=30
actual traded alts >=6
actual distinct pairs >=3
groups with successful SO >=2
loss-after-add groups >=2
symbol/pair/group/block positive contribution each <=50%
all-in cost/gross positive PnL <=50%
no liquidation/principal breach
```

0 P-B 仍必须完整发布 24 个结果。不能只列最好 policy，也不能把低 SO 的首单 pair trade称为 Martin family
完成。

### 14.1 唯一可选 ensemble

仅当 C0 与 MPI 各至少一个 P-B 时执行一个 shared-account ensemble。按 return-blind priority 取第一个：

```text
C0:
  1h-BETA-A0.10-SO0.50
  1h-BETA-A0.10-SO0.75
  1h-BETA-A0.20-SO0.50
  1h-BETA-A0.20-SO0.75
  then 5m BETA, then EQUAL in same order

MPI:
  1h-EMPIRICAL-EQUAL
  1h-EMPIRICAL-BETA
  1h-LAPLACE-EQUAL
  1h-LAPLACE-BETA
  then 5m in same order
```

两 family intents 在执行前合并；各占 50% ex-ante reserve quota；未用额度留现金。必须重新跑 G2/G3，禁止
组合 finished curves。

## 15. G3：三档、budgets、cold starts 与真实压力

G3 必须在 G2 前由 synthetic P-B fixture 证明可运行。对全部 P-B survivors 和适用 ensemble 执行：

```text
conservative: FO=5.0%,  cap=2x
balanced:     FO=7.5%,  cap=3x
aggressive:   FO=10.0%, cap=4x

budgets: 500 / 750 / 1000 / 1500 / 2000 / 3000 / 4000 / 4999U
cold starts: 0 / 30 / 60 / 90 / 120d
```

每行重新执行真实 orders、filters、reserve、funding 和 1m path，禁止缩放 curve。目标命中至少要求两个相邻
固定 budgets 同时通过。

不再发布虚假的 `theoretical_minimum_executable_principal`。只允许报告：

```text
filter_reserve_lower_bound = 明确标注为必要但不充分的数学下界
smallest_passing_frozen_budget = 上述固定预算中最小通过项
```

若没有通过项，写 null；不得扫描看收益后挑任意本金。

### 15.1 Stress matrix

每个 finalist/tier/budget 固定执行：

```text
combined fee+slippage+funding = 1.5x / 2x
partial fill = 25% / 50%
left-leg delay = 1 / 2 / 3 minutes
right-leg delay = 1 / 2 / 3 minutes
one-leg reject + immediate paired flatten
minNotional = 2x
tick/step = one level coarser
maintenance = +5%
signal missing/stale = deterministic stride 3
kill/restart/reconcile
worst combined
```

leg delay 只能延迟指定一腿；另一腿在原 open 成交，裸腿期间按每个真实 1m path 计 PnL、DD、margin、funding
和 liquidation。禁止同时延迟两腿后把绝对价格变化手写成 legging loss。

`worst combined` 必须至少包含：

```text
cost=2x, partial=25%, one selected leg delay=3m,
reject_second_leg=true, minNotional=2x, coarse filter,
maintenance=+5%, signal stride=3, restart=true
```

target stress survival：

```text
每个单项和 worst combined 最终状态 reconciled
无 liquidation/principal breach
每个 stress compounded return >0
worst combined DD <= 对应 tier DD cap
```

base ann 必须达到 tier return；stress ann 不要求仍达到 headline，但必须满足 survival。

## 16. 标准防过拟合验收

禁止继续使用 Round 28 proxy。所有统计使用完整 daily shared-account returns matrix；不得只用 12 个季度点。

### 16.1 CSCV/PBO

```text
chronological equal blocks S=16
all C(16,8)=12,870 train/test splits
每个 split 以 train daily Sharpe 选择最佳 policy，在对应 test 以 daily Sharpe 计算相对 rank/logit
PBO = fraction(test logit <=0)
hard gate: PBO <0.50
```

24 baseline 全进入 matrix；failed-gate/no-trade policy 以其实际 daily return series进入，不可删除。若 implementation
validator 失败，则整轮只能是 invalid，禁止对污染 matrix 发布 PBO。

### 16.2 Standard PSR/DSR

使用 Bailey/Lopez de Prado 标准形式：

```text
PSR(SR*) = Phi(
  (SR_hat-SR*)*sqrt(n-1) /
  sqrt(1-skew*SR_hat+((kurtosis-1)/4)*SR_hat^2)
)
```

`SR*` 使用 visible trial floor `N=1169`（ensemble 时 1170）对应的 expected maximum Sharpe；必须包含
Euler-Mascheroni correction，不得再使用 `sqrt(2 ln N)/sqrt(12)` proxy。全部量使用未年化 daily Sharpe：

```text
gamma_E = 0.5772156649015329
SR* = mean(SR_trials) + sqrt(var(SR_trials)) * (
        (1-gamma_E)*Phi^-1(1-1/N)
        + gamma_E*Phi^-1(1-1/(N*e))
      )
```

trial Sharpe 必须来自同一 daily matrix；零波动且零收益 policy 的 Sharpe 固定为 0，零波动正收益视为数据/计算
错误并 fail-close，不允许产生 infinity。每个候选分别计算：

```text
hard gate: DSR probability >=0.95
```

保存 sample count、frequency、mean/std/skew/kurtosis、N、SR* 和全部中间项。

### 16.3 Hansen SPA 与 bootstrap

```text
Hansen SPA on daily excess returns
benchmark return = 0，studentized performance statistic
stationary bootstrap replications = 9,999
expected block length = 20 calendar trading days
deterministic seed = first 64 bits SHA256(source_commit||"SPA")
hard gate: SPA p<=0.10

moving-block bootstrap daily returns
replications = 9,999
block length = 20 days
hard gate: annualized return 95% lower bound >0
```

实现必须与一个独立 Python reference 对相同 frozen matrix 一致；字段名不得写 `spa_style`。

### 16.4 Full shared-account leave-out reruns

对每个 P-B candidate：

```text
删除一个实际成交 symbol -> 从 signal scheduler 到 account 全量重跑
删除一个实际成交 pair   -> 从 signal scheduler 到 account 全量重跑
```

禁止 `total PnL - contribution`。至少 80% LOSO 和 80% LOPO runs compounded return >0，且每次仍满足 final
reconciliation/no liquidation。每个候选必须独立通过，不能用“至少一个 policy pass”。

### 16.5 其他稳定性门

```text
positive calendar blocks >=8/12
所有 frozen neighbor 公开，不得只选正者
五个 cold starts 按第 1 节通过
symbol/pair/group/block concentration <=50%
cost ratio <=50%
```

进展门 P-C，不得冒充目标：

```text
ann>=35%, DD<=20%, >=4/5 cold starts positive,
actual alts>=6, pairs>=3,
P-A/P-B/DSR/PBO/SPA/bootstrap/LOSO/LOPO/stress 全通过
```

## 17. 双独立 validator

### 17.1 Model validator

`scripts/r29_validate_models.py` 从 raw DB、fit cutoffs 和 source formulas 独立复算，不调用 fit/production statistic
helpers：

```text
weekly pair IDs 与 fit cutoffs
BTC-reference residuals、ECDF
Laplace mu/b、CDF/PIT clipping
Gaussian/Student-t likelihood/AIC 与 h-values
FlagLeft/FlagRight 每步累计、roll/reset
D crossing、direction、driver、conflict、SO thresholds、zero/S exits
future-shift causality canary
model/source/data hashes
```

至少检查每种 frequency/marginal 的真实 finite pair；如果不存在，输出完整 denominator 与 no-fit terminal，不能
手写 passed。

### 17.2 Account validator

`r29_validate` 只读 raw DB、signal/order/fill/account/risk traces，从零重建：

```text
open-phase sequence 与 allowed fields
same-minute entry adverse path
side/mode/filter quantity/order/fill/reject
pending single-leg delay 与 legging exposure
concurrent group ownership、reserve/margin、wallet/equity
fee/slippage/funding/partial/reject flatten
funding 后 maintenance 重查及其先于 exit/SO/FO 的顺序
FO/SO/exit/freeze/abort counts
calendar block PnL 与 all concentrations
daily returns、DD、maintenance/liquidation、annualization
budget/cold-start/stress、restart、final zero state
P-A/P-B/P-C、三档、anti-overfit inputs
```

不能调用 production replay/metric functions，不能信任 result JSON 自报 counts。

### 17.3 Mutants

至少逐一篡改并要求 validator 非零退出：

```text
side, mode, qty, fill_price, funding, wallet, timestamp, calendar_block
current close injected into open TP
current close injected into open SO
C0 adverse sign
MPI driver sign/SO threshold
delete a specifically identified overlapping intent
delete entry-minute adverse path
swap one-leg delay to both-leg delay
worst-combined second-leg reject removal
delete post-funding maintenance recheck
```

mutant 必须作用于实际包含对应事件的 trace；不能总改第一行或靠 row-count 差异假装验证业务语义。

authority 只能在双 validator 和全部适用 mutants pass 后生成。

## 18. 唯一执行顺序与命令

### 18.1 实现、测试与 source commit

```bash
python3 -m py_compile \
  scripts/r29_fit_mpi.py \
  scripts/r29_validate_models.py

cargo fmt --package r24-engine --package r24-research -- --check
cargo test -p r24-engine -p r24-research
cargo build --release -p r24-research --bin r29_execute --bin r29_validate
git diff --check

git add scripts/r29_fit_mpi.py scripts/r29_validate_models.py \
  crates/r24-engine/src/lib.rs \
  crates/r24-research/src/r29.rs \
  crates/r24-research/src/bin/r29_execute.rs \
  crates/r24-research/src/bin/r29_validate.rs \
  crates/r24-research/src/lib.rs
git add -f docs/superpowers/artifacts/glm-martingale-core-round29
git commit -m "fix(r29): restore causal minute-open Martin replay" \
  -m "问题描述: Round 28 先读当前分钟 high/low/close 再按 open 成交，C0 SO adverse 符号反向，并遗漏新仓首分钟风险。" \
  -m "复现路径: 重放 current-close mutation、C0 双方向、entry-minute liquidation、single-leg delay 和 overlapping intents canaries。" \
  -m "修复思路: 类型隔离 open/path phases，实现 family-specific adverse distance，并让所有新仓进入同分钟 high/low maintenance path。"

test -z "$(git status --porcelain)"
SOURCE_COMMIT="$(git rev-parse HEAD)"
RAW_ROOT="artifacts-local/round29/${SOURCE_COMMIT}"
mkdir -p "$RAW_ROOT"
```

source commit 后修改任何 model/replay/validator 代码，必须新 commit/new raw root。

### 18.2 D0 与 R0

```bash
target/release/r29_execute \
  --phase d0 --artifact-root "$RAW_ROOT" --resume

target/release/r29_execute \
  --phase r0-recovery --artifact-root "$RAW_ROOT" --resume

OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 \
python3 scripts/r29_validate_models.py \
  --phase recovery --artifact-root "$RAW_ROOT" --workers 8 --resume

target/release/r29_validate \
  --phase recovery --artifact-root "$RAW_ROOT" --resume
```

R0 非 PASS 时只能修工程错误并从新 source commit 重跑；不得 fit MPI 或发布收益。

### 18.3 MPI fit、G1 与 model validation

```bash
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 \
python3 scripts/r29_fit_mpi.py \
  --phase fit-signals --artifact-root "$RAW_ROOT" --workers 8 --resume

target/release/r29_execute \
  --phase g1-activation --artifact-root "$RAW_ROOT" --resume

OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 \
python3 scripts/r29_validate_models.py \
  --phase all-models --artifact-root "$RAW_ROOT" --workers 8 --resume
```

### 18.4 G2、G3 与 final validators

```bash
target/release/r29_execute \
  --phase g2-replay --artifact-root "$RAW_ROOT" --resume

# 总是执行；0 P-B 写 terminal not_applicable，不能省略文件
target/release/r29_execute \
  --phase g3-tiers --artifact-root "$RAW_ROOT" --resume

target/release/r29_validate \
  --phase all --artifact-root "$RAW_ROOT" --resume

target/release/r29_validate \
  --phase finalize --artifact-root "$RAW_ROOT" --resume
```

每个阶段保存 argv、PID、start/end UTC、wall seconds、peak RSS、workers、BLAS threads、exit code、resume、
source/data/policy/input hashes。

### 18.5 Evidence commit 与 push

```bash
git add -f docs/superpowers/artifacts/glm-martingale-core-round29
git add docs/superpowers/reports/2026-07-27-glm-round29-handoff.md
git commit -m "docs(r29): publish causal MPI Martin evidence" \
  -m "问题描述: Round 29 需要由 immutable traces 给出 corrected C0 与 cumulative MPI 的完整目标判定和失败边界。" \
  -m "复现路径: 核对 D0/R0、24 baseline、G3、双 validator、mutants、anti-overfit、12 blocks 和 final zero state。" \
  -m "修复思路: 提交 compact manifests、机器 authority、完整 trial/failure ledger 与 handoff，raw traces 保留在 source-commit root。"

test -z "$(git status --porcelain)"
git push -u origin glm-martingale-core-round29
```

若 push tool approval 被拒，记录 `PUSH_PENDING_TOOL_APPROVAL` 后仍视本地执行完成；不要回到 G0 重跑。

## 19. 必交 artifacts

仓库 compact evidence：

```text
docs/superpowers/artifacts/glm-martingale-core-round29/
  round29-protocol.json
  round29-source-map.json
  round29-data-manifest.json
  round29-policy-manifest.json
  round29-trial-ledger.json
  round29-failure-ledger.jsonl
  round29-closed-fingerprints.json
  round29-authority.json
  round29-execution-state.json
  gates/d0.json
  gates/r0-recovery.json
  gates/g1-activation.json
  gates/g2-replay.json
  gates/g3-tiers.json
  model-independent-validator.json
  account-independent-validator.json
  validator-mutants.json
  fit-snapshot-manifests/
  signal-intent-manifests/
  replay-results/                 # 24 baseline + applicable ensemble
  tier-results/                   # 每个 terminal 独立文件，不得占位
  trace-manifests/
  runtime/
```

raw root：

```text
source PDF/source manifest
fit snapshots 与 full signal rows
完整 account/order/fill traces
完整 risk RLE
daily return matrix
LOSO/LOPO full rerun traces
G3/cold-start/stress full traces
validator reference outputs 与 mutants
stdout/stderr/runtime manifests
```

不得提交 `>100MB` raw trace 到 Git；仓库提交 path、bytes、SHA256、row count 和 deterministic sample。

## 20. Failure ledger 与禁止错误 closure

每个失败记录：

```text
round, family, policy_id, canonical fingerprint
source/data/model/policy/trace hashes
exact trial denominator
first_failed_gate
numerator/denominator/value/threshold
causal reason
valid_implementation boolean
replay_required_after_fix boolean
never_repeat_rule
```

Round 28 的 16 个 C0 只能标记：

```text
invalid_implementation_evidence
replay_required_after_fix=true
```

只有 Round 29 修复后的 exact fingerprint 经双 validator pass 后，才可关闭该 exact policy。禁止外推关闭：

```text
所有 Copula
所有 cumulative signals
所有 Martin
所有多币组合
```

MPI 负结果最多关闭本任务书冻结的 `D=.6/S=2/frequency/marginal/weighting` exact fingerprints。

## 21. 合法终态与完成定义

唯一合法机器终态：

```text
VALID_DATA_CHANGED_NO_REPLAY
VALID_REPLAY_NO_PB
VALID_PB_NO_PC
VALID_PC_PROGRESS_NO_TARGET
VALID_TARGET_HIT
MATERIALLY_INCOMPLETE_INVALID_RESULTS
```

含义：

- `VALID_DATA_CHANGED_NO_REPLAY`：D0 bytes 与冻结输入不同；不得比较收益。
- `VALID_REPLAY_NO_PB`：24 baseline、双 validator、anti-overfit inputs 全 terminal，0 P-B。
- `VALID_PB_NO_PC`：有 P-B，但 anti-overfit/stress/P-C 未通过。
- `VALID_PC_PROGRESS_NO_TARGET`：至少一个 P-C 全门通过，但三档未命中。
- `VALID_TARGET_HIT`：至少一个 tier 在相邻 budgets、cold starts、stress、anti-overfit 和实盘门全部通过。
- `MATERIALLY_INCOMPLETE_INVALID_RESULTS`：任何适用 mandatory stage、validator、trace 或配额缺失。

禁止使用含糊的 `VALID_FRONTIER_PROGRESS`。`strict_valid_candidates` 只统计通过全部适用共同门的 P-C/target，
不能统计普通正收益 row。

Round 29 完成必须同时满足：

```text
R0 全部修复 canary pass
16 C0 + 8 MPI baseline 全 terminal
适用 G3 全 terminal
标准 PBO/DSR/SPA/bootstrap 与 LOSO/LOPO 完成
model/account validators 与全部适用 mutants pass
authority 由 validator 生成
failure ledger 完整
handoff 完整
local evidence commit clean
```

## 22. Handoff 必须逐项回答

`docs/superpowers/reports/2026-07-27-glm-round29-handoff.md` 必须逐项给出：

1. source commit/tree、data/policy hashes、raw root、Git/push 状态；
2. Round 28 三个 P0 是否逐项修复，给测试与 real trace proof；
3. 24 baseline 全表：ann、equity DD、12 blocks、FO/SO/exit/abort、actual alts/pairs、cost、concentration；
4. C0 两方向与 MPI 四种 flag trigger 的实际 denominator；
5. MPI conflicts、resets、SO1/SO2/SO3、zero-loss exits 和 S stops；
6. 每个 policy 的 overlap/reject/pending/single-leg delay 与 entry-minute liquidation证据；
7. P-B/P-C/target candidates 完整数组；没有则明确 `[]`；
8. PBO、standard DSR、Hansen SPA、bootstrap 的公式输入、中间项、trial floor 与结果；
9. 每个 P-B 的真实 LOSO/LOPO full rerun结果；
10. 三档、8 budgets、5 cold starts、所有 stress 和 smallest passing frozen budget；
11. 双 validator、每个 mutant、final zero state 与 BTC-zero proof；
12. 本轮新失败及 exact never-repeat 范围；
13. 最终 machine status、`strict_valid_candidates`、`target_hit`；
14. 明确说明结果是 historical prequential，不是 untouched future OOS。

执行完整后停止，不自行设计 Round 30，也不在 handoff 后继续调参。
