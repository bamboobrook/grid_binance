# GLM Martingale Core Round 26：Weekly Expanded-Universe Copula Martin 唯一任务书

制定日期：2026-07-23。

本任务书是 Round 26 的唯一执行权威。目标不是继续调 Round 25R 的无效结果，而是保留已经修好的账户引擎，
替换错误的统计模型门，并执行此前没有完成过的 29-alt weekly reference-copula Martin fingerprint。

禁止等待 30 个自然日、paper trading 或人工观察。只做历史 prequential 回测。

## 0. 上游权威与状态

必须先读取：

```text
docs/superpowers/reports/2026-07-23-chatgpt-round25r-post-correction-audit-and-round26-direction.md
docs/superpowers/artifacts/glm-martingale-core-round25-corrected/audit/
  round25r-selected-fit-cache-independent-audit.json
  round25r-post-correction-failure-ledger.jsonl
```

Round 25R 权威状态：

```text
MATERIALLY_INCOMPLETE_INVALID_RESULTS
```

Round 25R 的 16 个 terminal 只计入 global trial ledger，不得作为 valid failures、frontier、baseline return、
promotion parent 或 fingerprint closure。可继承 reserve、filter sizing、1m risk path、Copula conditional CDF、
raw traces 和账户 validator 代码，但不得继承 Rust stationarity、cost gate、matching 或 authority generation。

## 1. 最终目标与不可变约束

| 档位 | annualized return | max equity DD | cold starts | leverage cap |
|---|---:|---:|---:|---:|
| 保守 | `>=50%` | `<=10%` | `>=4/5` 正，另报 5/5 | `<=2x` |
| 平衡 | `>=90%` | `<=20%` | `>=4/5` 正，另报 5/5 | `<=3x` |
| 激进 | `>=100%` | `<=30%` | `>=3/5` 正，另报 5/5 | `<=4x` |

硬约束：

1. principal 严格 `<5000U`；`5000U` 不是合法 budget。
2. 一个连续 shared account；禁止拼接、平均或叠加独立 finished equity curves。
3. 最终候选实际成交 `>=6` alts、`>=3` distinct alt-alt pairs。
4. BTC 只作 reference，不得产生 order、trade、position、margin 或 PnL。
5. Martin 是唯一交易收益引擎。selector/Copula/Hurst/cost/liquidity 只能决定
   `FO/SO/TP/reduce/abort/freeze/admission`，不得单独开仓或贡献 PnL。
6. completed signal 后的下一根 1m event 才能成交；每根 1m high/low/funding/maintenance 均进入账户。
7. fit、universe、pair、family、threshold、quota、sizing 不得读取 outer return。
8. 所有 no-fit、no-signal、拒单、空 block、失败 policy 和被截断前的尝试都进入分母与 ledger。
9. 收益目标只能作为最终验收条件，不能用于选择参数或追加 trial。

## 2. 本轮新 fingerprint 与禁止重复项

新 fingerprint：

```text
R26_WEEKLY_TOP20_REFERENCE_SPREAD_COPULA_MARTIN
  29-alt data pool
  -> train-only weekly Top-20 liquidity universe
  -> 14d/21d formation
  -> independent statsmodels EG/KPSS/half-life/break snapshots
  -> exact maximum-cardinality maximum-weight disjoint matching
  -> Gaussian/Student-t train-AIC conditional Copula
  -> filter-resolved, cost-feasible Martin shared account
```

以前的周度 lookback 出现在 DGT、breadth、trend 或其他 selector 中，不等于这个 fingerprint。Round 25/25R
使用 6 alt、季度 selector roll、60/120d formation 和错误统计门，也不等于本轮。

禁止重新运行或扩展：

- Round 25R 的 `5m/1h x 60/120d x alpha 0.05/0.10 x SO 0.50/0.75` 16-policy population；
- 单币 ANKR、旧 DGT、普通 multiplier/spacing/TP、EMA/ADX/RSI 网格；
- 已关闭的 PC1/Johansen/VECM/Kalman/partial-cointegration fingerprints；
- 按 test return 选择 symbol、lookback、frequency、pair、Copula 或 threshold；
- 把论文收益、线性杠杆外推或单次 cold start 当成目标命中。

## 3. Git、权限与 artifact 协议

### 3.1 Git 不得阻塞执行

开始前记录：

```bash
git status --porcelain
git rev-parse HEAD
git rev-parse HEAD^{tree}
```

必须从 clean local commit 启动 replay。**本地 immutable commit 足以运行 G0-G3**。`git push` 只是最终交付，
不得作为 G0/G1 前置条件，不得因 push 权限再次向用户提问或停止回测。若最终 push 不可用，记录
`PUSH_PENDING`，继续完成所有历史回测和本地提交。

每个 commit body 必须包含：

```text
问题描述:
复现路径:
修复思路:
```

### 3.2 路径与体积

```text
repo root:  docs/superpowers/artifacts/glm-martingale-core-round26/
raw root:   artifacts-local/round26/<source_commit>/
report:     docs/superpowers/reports/2026-07-XX-glm-round26-handoff.md
```

raw traces 和完整 fit observations 留在 `artifacts-local/`。仓库只提交 protocol、hash、row count、schema、
compact results、first/last row 和固定种子样本。单个提交文件 `<10MB`；禁止提交 100MB 以上 blob。

每个阶段先写 checkpoint，再原子更新 execution state。`--resume` 必须跳过 hash 完全一致的 terminal，不得重复 fit。

## 4. D0：30-symbol 数据与可执行 filter gate

reference：

```text
BTCUSDT
```

固定 29-alt data pool：

```text
AAVEUSDT ADAUSDT ALGOUSDT APTUSDT ATOMUSDT AVAXUSDT BCHUSDT BNBUSDT
COMPUSDT CRVUSDT DASHUSDT DOGEUSDT DOTUSDT DYDXUSDT EGLDUSDT ETCUSDT
ETHUSDT FILUSDT GALAUSDT HBARUSDT ICPUSDT INJUSDT LINKUSDT NEARUSDT
SOLUSDT TRXUSDT UNIUSDT XRPUSDT ZECUSDT
```

本地先验事实：每个 alt 均有 `1,795,680` 根 1m futures bars，范围
`2023-01-01T00:00:00Z` 至 `2026-05-31T23:59:59.999Z`；funding DB 对这些 symbol 完整。

D0 必须重新独立验证，不得只复制上述文字：

1. 每 symbol 的 1m row count、first/last、missing minute、duplicate minute；
2. funding expected/actual/missing/duplicate；
3. 使用已提交的
   `docs/superpowers/artifacts/glm-martingale-core-round25-corrected/audit/perp-exchangeInfo-30-symbols-2026-07-23.json`，
   验证 30 symbols、serverTime 与 source SHA256；联网刷新只作可选交付，失败不得阻塞；
4. 对每 symbol 保存 tick、step、minQty、minNotional；maintenance 使用 Round 25R 已审计 production schedule
   及其 hash，另在 G3 做 `+5%` stress，不得声称 exchangeInfo 提供 leverage bracket；
5. 任何数据不完整的 symbol fail closed，但不得删除对应 no-data 记录；
6. 至少 12 个 eligible alts 才允许 G1；否则状态为 `VALID_DATA_GATE_NO_SEARCH`，不是 blocker。

每个 weekly roll 只用 formation window 的 `close * volume` 计算流动性，在 eligible pool 中按以下规则形成 Top-20：

```text
zero-volume bars <= 1%
p10 one-minute quote volume >= 10,000U
按 formation median daily quote volume 降序
symbol 字典序作确定性 tie-break
最多 20 alts
```

该 universe 在 trading week 内冻结。未来 volume、全样本 volume 和 outer PnL 均不得参与选择。

## 5. R0：独立模型统计快照

### 5.1 权威分工

必须新增并使用固定文件名：

```text
scripts/r26_fit_snapshots.py
crates/r24-research/src/r26.rs
crates/r24-research/src/bin/r26_execute.rs
crates/r24-research/src/bin/r26_validate.rs
```

Python 只负责 train-only model snapshots，不能计算账户 PnL、DD 或 promotion。环境写入 manifest：

```text
Python version
numpy version
scipy version
statsmodels version (expected 0.14.5)
script SHA256
input DB/filter/funding SHA256
```

Rust 只消费 immutable snapshots 并负责 signal state、orders、funding、margin、liquidation、wallet、equity 和 metrics。
Rust 不得再次近似 ADF/KPSS p-value。

### 5.2 时间合同

roll anchor 固定为：

```text
2023-07-01T00:00:00Z 起，每 7 个自然日一个 trading week，直到 2026-05-30T00:00:00Z
```

formation 为 roll anchor 前完整 `14d` 或 `21d`。只使用 `close_time < roll_anchor` 的 completed bars。

signal frequencies：

```text
completed_1h
completed_5m
```

5m 只先做 activation census；没有通过第 8 节 activity gate 时不得完整 replay。

### 5.3 每个 BTC-alt spread 的统计

拟合：

```text
S_i = log(BTC) - intercept_i - beta_i * log(alt_i)
```

保存：

```text
sample_count/intercept/beta/residual_sigma
statsmodels coint statistic/MacKinnon p-value/critical values/autolag
residual adfuller statistic/p-value (diagnostic only)
KPSS statistic/p-value/lags
AR(1) phi/half-life
CUSUM statistic/p-value
first-half/second-half beta drift
actual tail crossings/neutral returns/censored failures
fit_start/fit_end/data_hash
```

共同硬门：

```text
sample coverage = 100%
EG half-life finite and 2 bars <= half-life <= one trading week
KPSS p >= 0.05
CUSUM p >= 0.05
absolute beta drift <= 25%
以固定 diagnostic alpha=0.20 计算的 actual train tail crossings >= 20
```

仅预注册两个 selector arms：

```text
RAW: EG p <= 0.05 + 共同硬门
FDR: 同一 roll/lookback/universe 内对 EG p 做 Benjamini-Hochberg q=0.05 + 共同硬门
```

两臂必须同时落盘，且在查看 trading-week return 前冻结。RAW 不是 FDR 的替代或 fallback；两者均计入 global trials。

### 5.4 真实 cost feasibility

禁止 `sigma >= constant`。对每个候选必须先按 filter 解析两腿 quantities，再用 formation-only 完整 tail excursions
估算 tail-to-neutral equal-dollar gross edge。未在一周内回到 neutral 的 excursion 作为 censored non-positive edge，
不能删除。

硬门：

```text
以 alpha=0.20 crossing、0.35-0.65 neutral band 计算的 completed/censored excursions >= 20
median all-in edge / round-trip cost >= 2.0
p25 all-in edge > 0
两腿 resolved gross mismatch <= 5%
```

round-trip cost 必须包含 entry/exit fee、现有 production slippage、formation funding p95、legging allowance 和
filter rounding。cost inputs、quantities、edge distribution 和 decision 全部落盘。

### 5.5 Copula 与 exact matching

1. 依赖估计只使用 timestamp-aligned unsorted observations；sorted copy 只供 empirical CDF。
2. Gaussian 和 Student-t (`nu=3..30`) 只按 train log-likelihood/AIC 选择。
3. 保存 family/rho/nu/loglik/AIC/sample count/Kendall tau、fit cutoff 和 Python reference values。
4. Rust 重新计算 conditional CDF canary；`independent_reference_error` 必须是真实绝对误差，容差 `<=1e-7`。
5. pair score 只允许 stationarity、half-life、break、tail count、cost ratio、Copula AIC 和 liquidity。
6. selector 先最大化 disjoint pair 数，再最大化总 score，最多 3 pairs；同分时按 pair 字典序。
7. 必须使用 exhaustive/DP exact solver。按 edge score greedy 明确禁止。
8. active Martin cycle 持有 open-time frozen model；weekly roll 只影响新 cycle。

## 6. 必过回归测试

必须先证明旧实现失败，再完成修复：

```text
normal_cdf_is_not_adf_or_engle_granger_p_value
round25r_doge_cache_matches_statsmodels_residual_adf_0_1127638
round25r_link_cache_matches_statsmodels_kpss_0_0331150
only_sol_doge_passes_selected_round25r_pairs_under_independent_eg_kpss
bh_q05_matches_frozen_reference_vector
future_shift_changes_only_snapshots_after_fit_cutoff
permuting_time_changes_copula_but_not_marginals
independent_reference_error_is_computed_not_constant
greedy_counterexample_returns_exact_two_edge_weight_18_not_one_edge_weight_10
sigma_without_positive_edge_over_cost_is_rejected
censored_non_reversion_is_included_in_cost_gate
weekly_top20_uses_formation_volume_only
active_cycle_keeps_frozen_model_across_weekly_roll
model_snapshot_hash_mismatch_fails_closed
```

Round 25R 的 reserve/filter/1m/account tests 必须继续全过，不能删除或改成 ignored。

## 7. G0：production parity gate

G0-S：

```bash
python3 -m py_compile scripts/r26_fit_snapshots.py
cargo fmt --package r24-engine --package r24-research -- --check
cargo test -p r24-engine
cargo test -p r24-research
```

G0-R 在四个固定 windows 各跑 1h RAW 与 FDR，共 8 个 real replays：

```text
range  2023-08-01..2023-08-14
bull   2024-02-15..2024-02-29
shock  2024-08-01..2024-08-15
bear   2025-02-01..2025-02-15
```

每个 window 必须证明：

- snapshot cutoff、Top-20 universe、statsmodels values、pair graph 和 exact matching 可追溯；
- 至少一条真实 filter-resolved order 或明确的 no-tail/no-fit/cost rejection；
- BTC order/trade/position/PnL 全为 0；
- reserve 每 event 可复算，close 后释放；
- active model 在 roll 后不变；
- 1m risk rows 与 window 完整对齐；
- Python model validator 与 Rust account validator 同时通过。

编译错误、无 pair、无 order、运行慢和 push 失败都不是 blocker。实现错误必须修复后重跑同一 G0，不得改门槛。

## 8. G1：return-blind activation census

完整运行固定 8 个配置：

```text
frequency: 1h / 5m
formation: 14d / 21d
selector:  RAW / FDR
```

每配置必须有全部 152 个 weekly rolls，无 step truncation。保存每 roll 的 eligible universe、stationary alts、
matched pairs、tail crossings、cost rejection、family 和模型 hash。activation 阶段不得生成或读取 account return。

一个配置只有同时满足以下 activity gate 才能进入 G2：

```text
valid weekly snapshots = 152/152
distinct stationary alts across timeline >= 12
rolls with >=2 stationary alts >= 8
rolls with >=4 stationary alts >= 3
distinct exact-matched pairs across timeline >= 6
candidate FO tail crossings >= 100
candidate FO crossings distributed across >=8/12 outer blocks
no symbol contributes >35% of candidate crossings
```

不得为了让 FDR 或 5m 通过而放宽 gate。预计 1h RAW 有机会通过；这只是 audit census 的先验，不是保证。

若 0 个配置通过，形成 `VALID_ACTIVATION_NO_REPLAY_CANDIDATE`，提交完整失败台账并停止，不得运行虚假 PnL。

## 9. G2：Martin shared-account replay

只对 G1 survivors 建立固定 population：

```text
entry alpha: 0.10 / 0.20
SO adverse step: 0.50 / 0.75 train sigma
其他 selector/frequency/formation 继承 survivor
```

如果两个 formation 都通过，则每 selector/frequency 最多 8 policies。不得在看结果后增加 alpha、SO step、TP、
spacing、multiplier 或新 indicator。

baseline principal：`2000U`。

baseline Martin sizing：

```text
FO target group gross = max(filter-feasible minimum, 5% * starting principal)
relative layers = [1.00, 1.25, 1.55, 1.90]
max active groups = 3
leverage cap = 2x
```

每层按真实两腿 filter 解析；禁止理论 50/50 quantity。SO 必须同时满足：

```text
group net after all close costs < 0
adverse distance >= frozen SO step
same frozen Copula tail
one completed signal increment不再继续恶化
rolling stationarity snapshot仍有效
next resolved layer > previous resolved layer
reserve/filter/concentration pass
```

TP 只在 all-in group net positive 且 conditional probabilities 回到 neutral band。break、7d deadline、data stale、
filter failure 或 risk breach 使用预注册 reduce/abort；不得无限等待回归。新 cycle 必须先 neutral reset 后重新 crossing。

weekly model roll 不得强平旧 cycle，也不得让旧 cycle换 beta/family/rho/nu。

## 10. P-A 与 P-B promotion

P-A：production/account/trace/model parity 全过，final state 全归零，无 lookahead、BTC order 或 unreconciled field。

P-B 还必须同时满足：

```text
compounded return > 0
positive outer blocks >= 8/12
closed Martin cycles >= 30
actual traded alts >= 6
distinct traded pairs >= 3
loss-after-add SO groups >= 2
symbol/group/block positive contribution each <= 50%
all-in cost / gross profit <= 50%
no liquidation or principal breach
```

0 个 P-B 时，所有 policies 都写 exact failure reason，状态为 `VALID_HISTORICAL_PREQUENTIAL_NO_PB`。不得根据
最高 annualized return 选一个失败项继续放大。

有 P-B 时最多晋级 2 个，排序只按预注册：

```text
positive blocks desc
DDR desc
max concentration asc
cost/gross-profit asc
policy id asc
```

收益目标不参与晋级排序。

## 11. G3：三档、budgets、cold starts 与实盘压力

三档只改变同一个 P-B survivor 的 layer scale 和 leverage cap，不改变 selector、pair、direction、entry、exit 或 SO：

```text
conservative: FO = 5.0% principal, relative layers x1.0, cap 2x
balanced:     FO = 7.5% principal, relative layers x1.0, cap 3x
aggressive:   FO = 10.0% principal, relative layers x1.0, cap 4x
```

每档必须重新跑完整账户，禁止线性缩放 equity curve。

budgets：

```text
500 / 750 / 1000 / 1500 / 2000 / 3000 / 4000 / 4999U
```

filter 不可执行的低 budget 可以 valid reject。小资金 claim 至少需要两个相邻 `<5000U` budgets 完整通过，并给出
exact minimum executable principal。

cold starts：

```text
0 / 30 / 60 / 90 / 120d
```

成本与执行压力：

```text
fee/slippage/funding = 1x / 1.5x / 2x
partial fill = 25% / 50%
leg delay = 1 / 2 / 3 bars
one-leg reject and paired flatten
minNotional = 2x
tick/step = one level coarser
maintenance = +5%
signal missing/stale
kill/restart/reconcile
worst-combined stress
```

抗过拟合：

```text
exact population CSCV/PBO
Deflated Sharpe using global trial count, not only Round 26
SPA/reality check
stationary/block bootstrap CI
leave-one-symbol-out and leave-one-pair-out
neighbor stability for alpha/SO step/formation
all five cold starts
```

三档分别按第 1 节目标判定。只命中一档只能声称该档，不能外推其他档。

## 12. C2 条件增强：local Hurst veto

只有 baseline C1 parent 通过 P-B，才允许执行一个条件增强：

```text
C2_LOCAL_HURST_ANTIPERSISTENCE_VETO
```

local Hurst 只用 formation/已完成 bars，冻结阈值 `H<0.5`。它只能 veto FO、freeze SO 或触发 abort；不得产生
独立 order/PnL、增加 leverage 或改变方向。必须与同一 parent 完整 A/B replay，且 order hash 必须真实不同。

没有 P-B parent 时 C2=`not_applicable`。禁止新增 EMA/ADX/RSI 或按收益搜索 H threshold。

## 13. 双重独立 validator

### 13.1 Model validator

独立读取 snapshots 和原始 DB，至少复算：

```text
Top-20 universe and cutoff
EG/KPSS/half-life/CUSUM/beta drift
BH decisions
tail/censored counts
cost feasibility
Copula family/rho/nu/AIC
exact matching optimum
snapshot hashes
```

不得调用 production fitter。

### 13.2 Account validator

独立读取 raw traces，至少复算：

```text
wallet/equity/reserve/margin/funding/costs
1m DD/liquidation path
block returns/positive blocks
annualization/Sharpe/Sortino/Calmar/DDR
symbol/group/block concentration
orders/fills/rejections/partial/legging
final positions/groups/pending/reserve
```

只有两个 validator 都通过，authority 才能写 `VALID_*`。

## 14. 唯一执行命令

允许使用 `--resume`，不得改变 phase 语义：

```bash
SOURCE_COMMIT="$(git rev-parse HEAD)"
RAW_ROOT="artifacts-local/round26/${SOURCE_COMMIT}"

python3 scripts/r26_fit_snapshots.py \
  --phase g0 --frequency 1h --formation-days 14,21 \
  --artifact-root "$RAW_ROOT" --resume

cargo fmt --package r24-engine --package r24-research -- --check
cargo test -p r24-engine
cargo test -p r24-research

cargo run --release -p r24-research --bin r26_execute -- \
  --phase g0 --artifact-root "$RAW_ROOT" --resume

python3 scripts/r26_fit_snapshots.py \
  --phase g1 --frequency 1h,5m --formation-days 14,21 \
  --artifact-root "$RAW_ROOT" --resume

cargo run --release -p r24-research --bin r26_execute -- \
  --phase g1-activation --artifact-root "$RAW_ROOT" --resume

cargo run --release -p r24-research --bin r26_execute -- \
  --phase g2-replay --artifact-root "$RAW_ROOT" --resume

cargo run --release -p r24-research --bin r26_validate -- \
  --phase g0-g2 --artifact-root "$RAW_ROOT"
```

仅有 P-B survivor 才执行：

```bash
SOURCE_COMMIT="$(git rev-parse HEAD)"
RAW_ROOT="artifacts-local/round26/${SOURCE_COMMIT}"

cargo run --release -p r24-research --bin r26_execute -- \
  --phase g3 --artifact-root "$RAW_ROOT" --resume

cargo run --release -p r24-research --bin r26_validate -- \
  --phase all --artifact-root "$RAW_ROOT"
```

运行时必须记录真实 argv、PID、start/end UTC、wall time、peak RSS、exit code、source/data/policy/snapshot hashes。

## 15. 必交产物

```text
round26-authority.json
round26-protocol.json
round26-execution-state.json
round26-data-manifest.json
round26-universe-manifest.json
round26-policy-manifest.json
round26-trial-ledger.json
round26-failure-ledger.jsonl
round26-closed-fingerprints.json
exploration-registry.jsonl
gates/d0.json
gates/r0.json
gates/g0.json
gates/g1-activation.json
gates/g2-replay.json
gates/g3.json (conditional)
fit-snapshot-manifests/**
trace-manifests/**
replay-results/**
model-independent-validator.json
account-independent-validator.json
2026-07-XX-glm-round26-handoff.md
```

handoff 必须先写结论，再写最佳有效候选；无有效候选时不得展示无效 headline 诱导继续放大。

## 16. 停止条件

只有以下条件可以结束本轮：

1. D0 数据门形成 valid terminal；或
2. 8 个 G1 activation configs 全部形成 terminal，且 0 个通过 activity gate；或
3. 所有 activation survivors 的 G2 policies 全部形成 terminal；或
4. 有 P-B 时，G3 三档、budgets、cold starts、stress、anti-overfit 和双 validator 全部完成；或
5. 同一个外部数据/硬件 blocker 经三次可复现修复仍无法解除。

push 失败、编译错误、运行慢、无 fit、无 order、低收益、某个 policy 失败、单文件过大都不能提前停止。
失败必须记录并继续下一预注册项。不得为了“找到目标”修改已冻结门槛、删除负结果或追加事后参数。
