# GLM Martingale Core Round 27：Staged Admission + Persistence Martin 唯一任务书

制定日期：2026-07-26。

状态：`READY_TO_EXECUTE`。

本文件是 Round 27 的唯一执行权威。不得另建同轮补充计划，不得等待 30 个自然日，不得把 paper trading
作为完成条件。只做历史 causal/prequential 回测。

Round 27 不承诺必然命中目标。它要完成两件此前没有被有效完成的事：

1. 修复 Round 26 被退化 CUSUM 和过度联合门清空的问题，使 source-faithful reference-Copula Martin 真正
   进入 pair、订单和共享账户回放；
2. 独立执行一个未在 Round 1-26 出现过的 PBD finite-persistence Martin arm，检验把永久冲击和微观噪声
   从可交易均值回归中分离，能否改善牛熊稳定性。

## 0. 必读权威

开始前必须读取：

```text
docs/superpowers/reports/2026-07-26-chatgpt-round26-execution-audit-and-round27-direction.md
docs/superpowers/artifacts/glm-martingale-core-round26/audit/
  round26-corrected-authority.json
  round26-independent-gate-decomposition.json
  round26-corrected-failure-ledger.jsonl
  round27-external-source-update.json
docs/superpowers/plans/2026-07-23-glm-martingale-core-round26-weekly-expanded-copula-martin-plan.md
```

Round 26 修正状态：

```text
MATERIALLY_INCOMPLETE_INVALID_FAMILY_CLOSURE
```

可继承：29-alt 数据、weekly Top-20、statsmodels EG/KPSS、Copula 公式、exact matching、filter sizing、
shared account、reserve、1m risk path、funding、partial fill/reject/restart 和原始 snapshots。

不可继承为结论：Round 26 broad family closure、8 条 `valid_failure`、零收益、收益上限或目标不可达。

## 1. 最终目标与硬约束

| 档位 | annualized return | max equity DD | cold starts | leverage cap |
|---|---:|---:|---:|---:|
| 保守 | `>=50%` | `<=10%` | `>=4/5` 正，另报 5/5 | `<=2x` |
| 平衡 | `>=90%` | `<=20%` | `>=4/5` 正，另报 5/5 | `<=3x` |
| 激进 | `>=100%` | `<=30%` | `>=3/5` 正，另报 5/5 | `<=4x` |

共同硬约束：

1. principal 严格 `<5000U`，`5000U` 不合法。
2. Martin 是唯一 PnL engine。EG/Copula/distance/convergence/PBD/break/liquidity 只能控制
   `admission/FO/SO/TP/reduce/abort/freeze`，不得建立独立仓位或 PnL sleeve。
3. 一个连续 shared cash/margin/equity/reserve account；禁止拼接、平均或叠加 finished curves。
4. 最终候选实际成交 `>=6` alts、`>=3` distinct alt-alt pairs。
5. BTC 仅为 Copula reference；BTC order/trade/position/margin/PnL 必须恒为 0。
6. PBD arm 直接使用 alt-alt spread，但所有两腿订单仍必须进入同一 Martin account。
7. completed signal 后的下一根 1m event 才可成交；1m high/low/funding/maintenance 全部进入风险路径。
8. fit、universe、pair、family、threshold、quota 和 sizing 不得读取当前或未来 outer return。
9. no-fit、no-signal、empty、reject、breach、timeout 和失败 policy 全部进入分母与 global trial ledger。
10. 收益目标只用于最终验收，不能用于选择 arm、参数、budget 或追加 trial。

## 2. 新 fingerprint 与禁止重复

### 2.1 本轮三个预注册 arm

```text
R27-C0-SOURCE-EG-COPULA-MARTIN
  29-alt -> weekly Top-20 -> 1h/5m 21d formation -> RAW EG
  -> BTC-reference residuals -> fixed-threshold Copula
  -> exact disjoint alt-alt pairs -> Martin shared account

R27-C1-STAGED-ROBUST-COPULA-MARTIN
  29-alt -> weekly Top-20 -> 14d/21d formation
  -> EG + KPSS + finite half-life admission
  -> beta drift / HAC break / convergence / distance only rank or veto
  -> fixed-threshold Copula -> exact pairs -> Martin shared account

R27-P1-PBD-FINITE-PERSISTENCE-MARTIN
  29-alt -> weekly Top-20 -> 5m 7d formation / 7d trading
  -> direct alt-alt spread = random walk + AR(1) + white noise
  -> finite-persistence component signal -> Martin shared account
```

`C0` 是论文语义 control；`C1` 是 Round 26 的 corrected staged arm；`P1` 是新机制。三者都必须做完整
return-blind activation，某一 arm 失败不能阻止其他 arm。

### 2.2 永久禁止项

- Round 26 的 `CUSUM p>=0.05` 六重联合 hard admission；
- 把 beta first-half 接近 0 时的相对 drift 直接作为 hard gate；
- 在 pair 形成前要求每个 BTC-alt residual 有 `>=20` tail-to-neutral excursions；
- 普通 multiplier/spacing/TP/EMA/ADX/RSI/Donchian 网格；
- 单币 ANKR、旧 DGT、curve allocator、finished-equity blending；
- 旧 PC1/Johansen/VECM/Kalman/partial-cointegration exact fingerprints；
- structural-break RL、DNN/LSTM、GA triple-barrier、outer-return threshold tuning；
- 按 test return 选择 symbol、pair、formation、frequency、family 或 budget；
- 把论文收益、短块 annualization 或线性杠杆外推当成目标命中。

PBD 不是旧 partial cointegration 的改名：其 exact runtime 必须同时含 random walk、finite AR(1) 和独立
white-noise 三组件，并以 full-vs-reduced BIC/likelihood canary 证明。若实现退化成 `RW+AR`、纯 AR、Kalman
hedge beta 或标签字段，写 `invalid_mechanism_binding`，不得回放。

## 3. Git 与执行协议

### 3.1 不得再次被 push 卡住

1. 从当前计划提交创建 `glm-martingale-core-round27` 分支。
2. 完成代码、测试、policy/source manifests 后提交一次 immutable replay source commit。
3. 本地 clean commit 足以运行 R0-G3。**replay 前不要求 push，不得向用户询问 push 授权。**
4. 若修复实现 bug，创建新 source commit；旧 raw root 标记 `superseded_implementation`，从受影响最早阶段重跑。
5. 只在全部 validator、authority、handoff 完成后执行一次 `git push -u origin glm-martingale-core-round27`。
6. 最终 push 失败时记录 `PUSH_PENDING`，但不得删除或停止本地完整回放。

每个 commit body 必须同时包含：

```text
问题描述:
复现路径:
修复思路:
```

### 3.2 固定路径

```text
repo artifact: docs/superpowers/artifacts/glm-martingale-core-round27/
raw artifact:  artifacts-local/round27/<source_commit>/
report:        docs/superpowers/reports/2026-07-XX-glm-round27-handoff.md
```

raw traces、完整 residuals、全部 PBD states 留在 `artifacts-local/`。仓库只提交 protocol、hash、schema、
compact rows、固定样本、first/last 和 validator outputs。单文件 `<10MB`，禁止提交 100MB blob。

每阶段先 append registry/checkpoint，再原子更新 execution state。`--resume` 只能跳过 source/data/policy/input
hash 全相同的 terminal。

## 4. 必须新增的文件

```text
scripts/r27_gate_decomposition.py
scripts/r27_fit_snapshots.py
scripts/r27_validate_models.py
crates/r24-research/src/r27.rs
crates/r24-research/src/bin/r27_execute.rs
crates/r24-research/src/bin/r27_validate.rs
```

职责：

- `r27_gate_decomposition.py`：只读 Round 26 immutable snapshots，生成单门/交集/roll activity；禁止 PnL。
- `r27_fit_snapshots.py`：C0/C1/P1 train-only snapshots；禁止账户 PnL、promotion 和 outer-return ranking。
- `r27_validate_models.py`：独立读 raw DB 和 snapshots；不得 import/call production fitter。
- `r27.rs`：fixed policy、selector state、pair/PBD model schema、matching 与 signal contract。
- `r27_execute.rs`：唯一 shared-account event replay 入口。
- `r27_validate.rs`：独立账户/trace/metric/authority 入口。

禁止复制同一函数到 validator 后称独立。生产与 validator 可以共同使用 `numpy/scipy/statsmodels`，但必须有
独立数据读取、独立公式路径和至少一组外部冻结 reference fixture。

## 5. R0：先修正 Round 26 证据链

### 5.1 精确复现 gate decomposition

命令必须生成：

```text
docs/superpowers/artifacts/glm-martingale-core-round27/r0-round26-gate-decomposition.json
```

且至少精确断言：

```text
snapshots = 608
legs = 9570
CUSUM p>=0.05 = 0
1h-14d RAW EG+KPSS+HL legs = 123; rolls>=2/4 = 23/8
1h-21d RAW EG+KPSS+HL legs = 116; rolls>=2/4 = 22/8
5m-14d RAW EG+KPSS+HL legs = 24;  rolls>=2/4 = 5/0
5m-21d RAW EG+KPSS+HL legs = 21;  rolls>=2/4 = 4/1
```

任一值不一致，先查 source snapshot/hash/data，不得直接改 expected。修复后重跑 R0。

### 5.2 closure 修正

生成：

```text
round27-historical-fingerprint-index.json
round27-reopened-fingerprint-map.json
```

旧 broad closure 映射为：

```text
closed_exact:
  R26_EXACT_EG_KPSS_HALF_LIFE_RAW_CUSUM_BETA_DRIFT_TAIL20_CONJUNCTIVE_ADMISSION

reopened_changed_mechanism:
  R27-C0-SOURCE-EG-COPULA-MARTIN
  R27-C1-STAGED-ROBUST-COPULA-MARTIN
  R27-P1-PBD-FINITE-PERSISTENCE-MARTIN
```

递归读取 Round 1-26 corrected ledgers。PBD exact fingerprint 若不存在才允许继续；只比较 label 不算 dedup。

## 6. D0：数据与 universe

沿用并重新 hash Round 26 的 BTC + 29-alt 数据：

```text
BTCUSDT
AAVEUSDT ADAUSDT ALGOUSDT APTUSDT ATOMUSDT AVAXUSDT BCHUSDT BNBUSDT
COMPUSDT CRVUSDT DASHUSDT DOGEUSDT DOTUSDT DYDXUSDT EGLDUSDT ETCUSDT
ETHUSDT FILUSDT GALAUSDT HBARUSDT ICPUSDT INJUSDT LINKUSDT NEARUSDT
SOLUSDT TRXUSDT UNIUSDT XRPUSDT ZECUSDT
```

D0 独立验证：

```text
1m first/last/count/missing/duplicate
funding first/last/expected/actual/missing/duplicate
exchangeInfo source SHA256 + tick/step/minQty/minNotional
maintenance source/version/hash
```

weekly Top-20 只用 formation 内 `close*volume`：

```text
zero-volume <=1%
p10 minute quote volume >=10000U
median daily quote volume desc
symbol asc tie-break
```

至少 12 eligible alts；否则 `VALID_DATA_GATE_NO_SEARCH`。不得联网刷新作为 blocker。

## 7. C0：source-faithful EG-Copula control

固定合同：

```text
frequency = completed 1h / completed 5m
formation = 21d
trading = next 7d
roll = weekly, 152 anchors
reference = BTCUSDT, non-tradable
admission RAW = EG MacKinnon p<=0.05
diagnostic FDR = BH q=0.05
```

C0 hard admission 只允许：100% sample coverage、finite intercept/beta/residual sigma、RAW/FDR EG decision。
KPSS、half-life、CUSUM、beta drift、tail count 全部保存但不作为 C0 hard gate。

对入选 BTC-alt residuals 的所有 alt-alt 组合：

1. 用 timestamp-aligned unsorted residual ranks 拟合 Gaussian 与 Student-t (`nu=3..30`) Copula；
2. 只按 formation AIC 选 family；
3. 固定 conditional tail alpha `0.10/0.20`，neutral band `0.35-0.65`；
4. C0 不允许用 formation pseudo-PnL 或 in-sample Sharpe 选 pair；
5. 固定 score：
   `-log10(max(EG_left,EG_right,1e-12)) + 1/max(HL_left+HL_right,1) - AIC/n + log1p(min_liquidity)/100`；
   non-finite half-life 的 speed 项为 0；
6. exact maximum-cardinality maximum-weight disjoint matching，最多 3 pairs；
7. filter-resolved minimum quantity和 gross mismatch `<=5%` 是硬门；
8. deterministic break-even move 必须覆盖 `2x` fee/slippage/funding/legging cost，否则 cost veto。

FDR 是 diagnostic control；只有其自身通过第 10 节 activity gate 才可 replay，不得 fallback 到 RAW。

## 8. C1：staged robust Copula arm

固定 population：

```text
frequency = completed 1h
formation = 14d / 21d
selector = RAW EG p<=0.05
base admission = coverage + EG + KPSS p>=0.05 + finite half-life [2,168]
```

以下字段只能进入 score、tie-break 或 runtime veto，不能再做 formation admission AND：

```text
HAC/bootstrap break score
absolute beta change normalized by max(abs(beta_full), 0.10)
actual leg tail count
causal previous-roll persistence
expected convergence time
formation normalized-price distance
```

score 必须在看 return 前冻结为 rank tuple，不拼接任意加权浮点目标：

```text
1. current base admission pass
2. previous-roll base pass (true first)
3. lower expected convergence time
4. lower HAC break statistic
5. lower robust beta drift
6. lower Copula AIC per observation
7. higher liquidity
8. pair_id asc
```

每 roll 按上述 tuple 排序后赋 ordinal edge score `edge_count-rank`；exact solver 先最大化 pair 数，再最大化
ordinal score 总和，再按 sorted pair IDs tie-break。不得自行给各 diagnostic 调浮点权重。

previous-roll persistence 很稀疏，只能排序；缺 previous pass 不拒绝 current candidate。

pair cost feasibility 使用 pair-level Copula divergence，不再使用 leg-level tail20。预注册两个独立 arms：

```text
C1-E0 deterministic break-even cost veto only
C1-E1 >=5 completed+censored formation pair excursions,
      median all-in edge / median cost >=2,
      p25 all-in edge >0,
      mismatch <=5%
```

两个 cost arms 都计入 global trials，均在 outer return 前冻结。若 `E1` 样本不足，只能作为该 arm 的 valid
rejection，不能删除；`E0` 仍继续。

## 9. P1：PBD finite-persistence arm

来源：`10.7494/manage.2019.20.2.151`。只采用模型结构，不采用论文 in-sample Sharpe selection 或收益。

固定合同：

```text
frequency = completed 5m
formation = previous 7 calendar days
trading = next 7 calendar days
roll = same 152 weekly anchors
universe = formation-only Top-20
pair prefilter = each symbol's 5 lowest normalized-price-distance partners
max raw pair fits per roll <=100, pair_id tie-break
```

每个 direct alt-alt log spread：

```text
Z_t = M_t + R_t + W_t
M_t = rho * M_(t-1) + epsilon_M,t       finite persistence
R_t = R_(t-1) + epsilon_R,t             infinite persistence
W_t = epsilon_W,t                       no persistence / microstructure noise
```

使用 `statsmodels.tsa.statespace.MLEModel` 或等价成熟 state-space library 做 constrained MLE；禁止手写简化
Kalman 近似。参数约束：

```text
0.5 < rho < 1
sigma_M > 0
sigma_R >= 0
sigma_W >= 0
```

admission：

```text
full PBD BIC < RW+noise reduced BIC
full PBD BIC < stationary AR(1)+noise reduced BIC
finite-persistence variance share R2_M >=0.5
optimizer converged and Hessian/finite diagnostics pass
filter-resolved two-leg quantities, gross mismatch <=5%
```

禁止按 in-sample Sharpe/PnL 选 pair。排序：BIC improvement、R2_M、较低 random-walk variance share、较短
finite half-life、liquidity、pair_id。exact disjoint matching 最多 3 pairs。

signal 只来自 filtered `M_t`：

```text
FO crossing = abs(M_t / sigma_M) >=2.0
TP neutral = abs(M_t / sigma_M) <=0.25
SO adverse step = 0.50 / 0.75 sigma_M from last paired fill
deadline = min(3 finite half-lives, 7d)
```

PBD 不得直接记 spread forecast PnL。每次 FO/SO/TP/abort 必须转换为真实两腿 Martin order intent，并由与
C0/C1 相同的账户引擎处理。

## 10. G0：必须证明真实 binding

### 10.1 必过 synthetic canaries

```text
raw_cusum_zero_population_does_not_block_c0_or_c1
round26_gate_decomposition_matches_frozen_reference
c0_eg_control_ignores_kpss_cusum_drift_and_leg_tail_for_admission
c1_break_drift_tail_are_rankers_not_joint_hard_gate
previous_roll_persistence_changes_rank_not_candidate_count
fixed_threshold_is_not_refit_from_outer_return
pair_cost_uses_pair_copula_excursions_not_leg_tail_count
rw_only_rejects_full_pbd
ar1_plus_noise_rejects_false_random_walk_component
pbd_three_component_fixture_recovers_ordered_variance_shares
pbd_filtered_m_component_changes_real_martin_order_hash
synthetic_loss_after_add_path_emits_fo_so_tp_and_reconciles_wallet
```

### 10.2 real-data canary

对每个 arm 自动选择**第一个按 anchor/pair_id 排序的 return-blind fitted pair**，不得人工挑窗口。至少保存：

```text
fit cutoff / raw data hash / model hash
pair score tuple / exact matching proof
first signal or explicit no-signal
filter-resolved quantities
real order/rejection delta when a signal exists
BTC zero-order proof
1m account path and final reconciliation
```

若某 arm 全 152 rolls 无 fitted pair，写该 arm 的 valid activation failure；其他 arm 继续。只要有 fitted pair
但 canary 因实现错误无 trace，必须修复，不得停止或改 gate。

## 11. G1：return-blind activation census

完整跑：

```text
C0 1h/21d RAW
C0 1h/21d FDR diagnostic
C0 5m/21d RAW
C0 5m/21d FDR diagnostic
C1 1h/14d E0
C1 1h/14d E1
C1 1h/21d E0
C1 1h/21d E1
P1 5m/7d
```

每个 config 必须 152/152 rolls。SO step 属于 G2 policy，不得在 G1 重复 P1 fit 或虚增 activation trial。

activity gate：

```text
valid weekly snapshots = 152/152
distinct fitted alts across timeline >=6
distinct exact matched pairs >=3
rolls with >=1 exact matched pair >=12
candidate FO crossings >=100
crossings distributed across >=8/12 outer blocks
no symbol contributes >50% crossings
model validator passed
```

这是 activation，不读取 account return。通过者按固定优先级最多晋级 3 个 fit configs：

```text
C0 5m RAW
C0 1h RAW
C1 21d E0
C1 14d E0
C1 21d E1
C1 14d E1
P1
C0 5m FDR
C0 1h FDR
```

P1 只算一个 activation survivor；它的 SO0.50/0.75 到 G2 才展开。若超过 3 个 survivor，不按 crossings
数量或 return 排序，严格按上表。

0 survivors 才可结束为 `VALID_ROUND27_ALL_ARMS_NO_ACTIVATION`。不得只因 C0/C1 失败而跳过 P1。

## 12. G2：Martin shared-account replay

每个 activation survivor 固定 policy population：

```text
C0/C1 entry alpha = 0.10 / 0.20
C0/C1 SO adverse step = 0.50 / 0.75 frozen sigma
P1 entry z = 2.0
P1 SO adverse step = 0.50 / 0.75 frozen sigma_M
```

最多晋级 3 个 fit configs：每个 C survivor 4 policies，P1 2 policies，总 population `<=12`。不得看结果
追加 threshold。

baseline：

```text
principal = 2000U
FO group gross = max(filter-feasible minimum, 5% principal)
relative layers = [1.00,1.25,1.55,1.90]
max active groups = 3
leverage cap = 2x
```

SO 必须同时满足：

```text
group all-in net <0
adverse distance from last executed paired fill >= frozen step
same frozen model still signals same tail/direction
next completed signal did not improve
runtime break/convergence state has not frozen additions
next resolved layer > previous resolved layer
reserve/filter/concentration pass
```

TP 只能在 all-in group net positive 且 signal neutral。model break、deadline、data stale、filter failure 或风险门
触发预注册 reduce/abort；禁止无限等回归。weekly roll 只影响新 cycle，旧 cycle 保持 open-time frozen model。

12 个 outer blocks 按时间连续 stitch；block 边界不重置 wallet、positions、groups、reserve 或 running peak。

### 12.1 P-A / P-B

P-A：model/account/trace/adapter parity 全过，无 lookahead/BTC order/unreconciled field，终态全归零。

P-B：

```text
compounded return >0
positive outer blocks >=8/12
closed Martin cycles >=30
actual traded alts >=6
distinct traded pairs >=3
loss-after-add SO groups >=2
symbol/pair/block positive contribution each <=50%
all-in cost/gross profit <=50%
no liquidation or principal breach
```

单 family 不适用 family-concentration `<=50%`；只有实际组合两个以上 PnL families 时才检查 family concentration。

0 P-B 时状态 `VALID_HISTORICAL_PREQUENTIAL_NO_PB`，全部 exact failures 写 ledger；禁止把最高 ann 失败项放大。

### 12.2 真正的多-family 组合

只有两个以上**不同 family** P-B survivor 时，允许一个组合 policy：

```text
chronological intents merged before execution
one shared account
equal ex-ante reserve quota by family
unused quota remains cash, cannot transfer after seeing PnL
same global symbol/pair/group caps
```

禁止叠加 equity curves。组合本身计入 global trials，并重新跑完整 G2/G3。

## 13. G3：三档、预算、cold starts 与实盘压力

只对最多 2 个 P-B survivor 加可选 1 个真实 shared-account ensemble 执行。

三档只改变 FO scale/leverage，不改变 selector、pair、direction、entry、exit、SO 或 family：

```text
conservative: FO=5.0%, cap=2x
balanced:     FO=7.5%, cap=3x
aggressive:   FO=10.0%, cap=4x
```

budgets：

```text
500 / 750 / 1000 / 1500 / 2000 / 3000 / 4000 / 4999U
```

每档每 budget 都重新跑账户。小资金 claim 至少两个相邻 `<5000U` budgets 通过，并给 exact minimum executable
principal。禁止缩放 2000U equity curve。

cold starts：

```text
0 / 30 / 60 / 90 / 120d
```

stress：

```text
fee/slippage/funding = 1x / 1.5x / 2x
partial fill = 25% / 50%
leg delay = 1 / 2 / 3 bars
one-leg reject + paired flatten
minNotional = 2x
tick/step = one level coarser
maintenance = +5%
signal missing/stale
kill/restart/reconcile
worst combined stress
```

anti-overfit：

```text
full frozen population CSCV/PBO
Deflated Sharpe with Round 1-27 global trial floor
SPA/reality check
stationary/block bootstrap CI
leave-one-symbol-out and leave-one-pair-out
leave-one-family-out for ensemble
neighbor stability only for already frozen alpha/SO/formation neighbors
all five cold starts
```

目标判定必须严格使用第 1 节。另报进展门但不得冒充目标：

```text
P-C progress = ann>=35%, DD<=20%, >=4/5 cold starts positive,
               actual alts>=6, pairs>=3, concentration/account/adapter/trial gates pass
```

## 14. 双 validator

### 14.1 Model validator

独立复算：

```text
Top-20 and cutoff
EG/KPSS/half-life and RAW/FDR
all gate decomposition counts
HAC/bootstrap break score and robust beta drift
Copula family/rho/nu/AIC/canaries
deterministic and empirical cost decisions
PBD constrained parameters/BIC/variance shares/filtered M state
exact matching optimum
snapshot and source hashes
```

至少 `checked_pair_count>0`；P1 fitted 时 `checked_pbd_pair_count>0`。否则 validator 不得 pass。

### 14.2 Account validator

独立从 raw traces 复算：

```text
orders/fills/rejections/partial/legging
wallet/equity/reserve/margin/funding/costs
1m DD/maintenance/liquidation
block return and local running-peak DD
annualization/Sharpe/Sortino/Calmar/DDR
symbol/pair/group/block/family concentration
final positions/groups/pending/reserve
restart/reconcile and adapter parity
```

authority 只能由两个 validator 生成；手写 handoff 不得覆盖 machine status。

## 15. 必过测试

除第 10 节 canaries 外，Round 24-26 的账户、reserve、filter、Copula、matching 和 1m path tests 全部保留。
禁止删除、改名逃避或 `ignored`。

固定命令：

```bash
python3 -m py_compile \
  scripts/r27_gate_decomposition.py \
  scripts/r27_fit_snapshots.py \
  scripts/r27_validate_models.py

cargo fmt --package r24-engine --package r24-research -- --check
cargo test -p r24-engine -p r24-research
```

## 16. 唯一执行顺序与命令

### 16.1 实现、冻结、提交

```bash
git status --porcelain
git rev-parse HEAD

# 完成全部代码、tests、protocol/policy/source manifests 后
git add scripts/r27_gate_decomposition.py scripts/r27_fit_snapshots.py scripts/r27_validate_models.py \
  crates/r24-research/src/r27.rs crates/r24-research/src/bin/r27_execute.rs \
  crates/r24-research/src/bin/r27_validate.rs crates/r24-research/src/lib.rs
git add -f docs/superpowers/artifacts/glm-martingale-core-round27
git commit -m "feat(r27): freeze staged admission and persistence Martin replay" \
  -m "问题描述: Round 26 的退化 CUSUM 与联合硬门在收益回放前清空全部模型，且 PBD finite-persistence 机制尚未执行。" \
  -m "复现路径: 读取 Round 26 的 608 snapshots 和 9570 legs，CUSUM p>=0.05 为 0，模型 validator 的 checked_pair_count 为 0。" \
  -m "修复思路: 冻结 source EG control、staged robust Copula 和 PBD 三个 return-blind arm，并统一接入 production-conservative Martin shared account。"

test -z "$(git status --porcelain)"
SOURCE_COMMIT="$(git rev-parse HEAD)"
RAW_ROOT="artifacts-local/round27/${SOURCE_COMMIT}"
```

### 16.2 R0/D0/model fit

```bash
python3 scripts/r27_gate_decomposition.py \
  --round26-root artifacts-local/round26/7a8e792696d955556a2725daf3d099c041c6397c \
  --artifact-root "$RAW_ROOT" --resume

OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 \
python3 scripts/r27_fit_snapshots.py \
  --phase d0-c0-c1 \
  --artifact-root "$RAW_ROOT" --workers 8 --resume

OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 \
python3 scripts/r27_fit_snapshots.py \
  --phase p1-pbd \
  --artifact-root "$RAW_ROOT" --workers 8 --resume
```

manifest 必须记录实际 worker 数、BLAS threads、wall time 和 peak RSS；不得改变数值或 seed。若内存不足，
只允许把 workers 从 `8` 降为 `4` 或 `2` 并记录 operational retry；fit/policy/data/seed 均不得改变。

### 16.3 G0-G3

```bash
cargo build --release -p r24-research --bin r27_execute --bin r27_validate

target/release/r27_execute \
  --phase g0 --artifact-root "$RAW_ROOT" --resume

target/release/r27_execute \
  --phase g1-activation --artifact-root "$RAW_ROOT" --resume

target/release/r27_execute \
  --phase g2-replay --artifact-root "$RAW_ROOT" --resume

# 只有 machine G2 存在 P-B survivor 时执行；否则命令写 not_applicable terminal
target/release/r27_execute \
  --phase g3 --artifact-root "$RAW_ROOT" --resume
```

### 16.4 独立验证与 finalization

```bash
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 \
python3 scripts/r27_validate_models.py \
  --artifact-root "$RAW_ROOT" --workers 8 --resume

target/release/r27_validate \
  --phase g0-g3 --artifact-root "$RAW_ROOT" --resume

git add -f docs/superpowers/artifacts/glm-martingale-core-round27
git add docs/superpowers/reports/2026-07-XX-glm-round27-handoff.md
git commit -m "docs(r27): publish validated Martin search evidence" \
  -m "问题描述: Round 27 需要从 immutable traces 生成唯一权威，避免手写摘要覆盖失败 gate。" \
  -m "复现路径: 运行 model/account validators 并核对 authority、registry、trial ledger、targets 和 final state。" \
  -m "修复思路: 提交 compact manifests、完整失败台账、双 validator 输出和机器生成 handoff，raw traces 留在 artifacts-local。"

git push -u origin glm-martingale-core-round27
```

最终 push 若失败，记录失败命令/exit/stderr 和 `PUSH_PENDING`；本地 authority/handoff 仍必须完成。

## 17. 必交 artifacts

```text
round27-authority.json
round27-protocol.json
round27-execution-state.json
round27-data-manifest.json
round27-policy-manifest.json
round27-source-manifest.json
round27-historical-fingerprint-index.json
round27-reopened-fingerprint-map.json
round27-trial-ledger.json
round27-failure-ledger.jsonl
r0-round26-gate-decomposition.json
gates/d0.json
gates/g0.json
gates/g1-activation.json
gates/g2-replay.json
gates/g3.json
model-independent-validator.json
account-independent-validator.json
fit-snapshot-manifests/c0-c1.json
fit-snapshot-manifests/p1.json
trace-manifests/*.json
replay-results/*.json
```

Handoff 必须明确回答：

1. 三个 arm 是否各自完整执行、哪一门首次失败；
2. Copula checked pairs 与 PBD checked pairs 数量；
3. 每个 P-B/P-C/失败 policy 的 ann、DD、12 blocks、5 cold starts、cost、concentration；
4. 三档每个 budget 的命中表与 exact minimum principal；
5. actual symbols、pairs、方向、权重、杠杆、FO/SO 层；
6. 所有失败 exact fingerprint 与 never-repeat reason；
7. 是否存在真实 shared-account multi-family ensemble；
8. Git source/validator/final commit 与 push 状态。

## 18. 合法终态

仅允许：

```text
VALID_DATA_GATE_NO_SEARCH
VALID_ROUND27_ALL_ARMS_NO_ACTIVATION
VALID_HISTORICAL_PREQUENTIAL_NO_PB
VALID_FRONTIER_PROGRESS_NO_TARGET
VALID_CONSERVATIVE_TARGET
VALID_BALANCED_TARGET
VALID_AGGRESSIVE_TARGET
INVALID_IMPLEMENTATION_OR_VALIDATOR_FAILURE
```

`COMPLETE`、`FINALIST`、`BREAKTHROUGH`、`PRODUCTION_READY` 不得单独作为状态。

实现错误、编译错误、运行慢、内存不足、无 pair、无 order 和 push 失败都不是随意改参数的理由。修复工程问题后
重跑相同 frozen population；有效负结果写 ledger 后继续下一预注册 arm。只有全部预注册路径形成 terminal，
Round 27 才算完整。
