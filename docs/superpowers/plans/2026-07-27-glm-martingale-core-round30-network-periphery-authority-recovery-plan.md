# GLM Martingale Core Round 30：Round 29 Authority Recovery + Network-Periphery Martin 唯一任务书

日期：2026-07-27
状态：`READY_TO_EXECUTE`
执行分支：`glm-martingale-core-round30`

本文件是 Round 30 唯一任务书。执行 agent 必须先恢复 Round 29 的机器权威，再执行新的 network-periphery
Martin。不得因为 Round 29 被口头标记“完成”而跳过证据门，不得根据 outer return 改 pair、图、Hurst、阈值、
预算或配额，也不得把任何 finished curve 拼成组合。

历史窗口已被多轮读取，所有结果只能称 historical prequential evidence，不是 untouched OOS。本轮直接历史
回测，不设置 30 天等待或监控任务。

## 0. 必读权威与当前边界

依次读取：

```text
docs/superpowers/reports/2026-07-27-chatgpt-round29-evidence-audit-and-round30-direction.md
docs/superpowers/reports/2026-07-27-chatgpt-round28-execution-audit-and-round1-28-authority.md
docs/superpowers/plans/2026-07-27-glm-martingale-core-round29-causal-open-phase-cumulative-mispricing-plan.md
docs/superpowers/reports/2026-07-26-glm-round28-handoff.md
docs/superpowers/artifacts/glm-martingale-core-round28/round28-data-manifest.json
docs/superpowers/artifacts/glm-martingale-core-round28/round28-policy-manifest.json
```

当前唯一可见 parent：

```text
plan commit = c90d79c506a7d0ebe47518bac6d11fcd409d9167
Round 29 execution commit = absent
Round 29 raw root = absent
Round 29 current status = MATERIALLY_INCOMPLETE_MISSING_ALL_EXECUTION_EVIDENCE
```

Round 29 后续若同步，不得直接相信 handoff。只有第 7 节全部验证通过才可导入；否则走 mandatory replay。

## 1. 用户目标与共同硬约束

本轮按用户最新明确口径恢复激进档 `110%`：

| tier | annualized return | max equity DD | cold starts | leverage cap |
|---|---:|---:|---:|---:|
| conservative | `>=50%` | `<=10%` | `>=4/5` 正，另报 5/5 | `<=2x` |
| balanced | `>=90%` | `<=20%` | `>=4/5` 正，另报 5/5 | `<=3x` |
| aggressive | `>=110%` | `<=30%` | `>=3/5` 正，另报 5/5 | `<=4x` |

共同硬约束：

1. principal 严格 `<5000U`；`5000U` 不合法。
2. Martin group 是唯一交易 PnL engine；network/Copula/z/Hurst 只能控制 FO/SO/exit/freeze/abort。
3. 所有 pair intents 必须进入一个连续 shared cash/margin/equity/reserve account。
4. 禁止拼接、平均、缩放、平移 finished curves。
5. 最终候选实际成交 `>=6` alts、`>=3` distinct alt-alt pairs。
6. BTC 只可作冻结 reference/control；BTC order/trade/position/margin/PnL 必须为 0。
7. completed signal 后下一根可用 1m open 才可成交；open decision 禁止读取当前 high/low/close。
8. 1m path、funding、fee、slippage、filter、maintenance、partial fill、legging 和 liquidation 全部入账。
9. fit、network、pair、direction、quota、budget、sizing、阈值和 family 选择不得读取当前或未来 outer return。
10. no-fit/no-edge/no-community/no-signal/reject/conflict/breach/timeout/失败 policy 全部入 ledger。
11. 收益目标只用于最终验收，不得用于选择、提前停止、追加 arm 或改参数。
12. target claim 必须由 raw trace 经双独立 validator 生成；手写报告无权覆盖 machine authority。

## 2. 冻结研究问题与试验配额

Round 30 只回答三个问题：

1. Round 29 的 16 C0 + 8 cumulative MPI 是否存在完整、有效、可复算的执行证据；
2. formation-only cointegration network 的外围结构 pair 是否降低共享账户集中失效和回撤；
3. 固定 local `H<0.5` 是否提高 Martin cycle 的回归速度/成本后稳定性，而非只减少交易。

### 2.1 Round 29 control population

必须恢复 Round 29 原 24 个 baseline：

```text
C0 = 16
MPI = 8
```

若验证后导入，不重复回测、不重复计 trial；若证据缺失或任一 P0 validator 失败，必须用修复后的同一语义重跑
完整 24 个，不得只跑原最好 policy。

若恢复结果满足 Round 29 第 14.1 节的 C0/MPI 双 P-B 条件，Round 29 自身唯一 C0+MPI shared-account
ensemble 也属于适用恢复项，必须执行或导入并重新验证；它不进入 24-baseline matrix，但计一个 visible trial。

### 2.2 Round 30 network population

固定 6 个新 baseline：

```text
selector = CLASSIC_EG / PERIPHERAL_PMFG / PERIPHERAL_PMFG_SC
hurst_gate = OFF / H_LT_0.5
frequency = 1h
formation = 28d / 672 completed bars
trading roll = 7d weekly
```

总数：

```text
3 selectors * 2 Hurst states = 6 policies
```

唯一 policy ID 与 return-blind priority：

| priority | policy_id | selector | Hurst |
|---:|---|---|---|
| 1 | `R30-NET-PMFG-SC-HLT05` | `PERIPHERAL_PMFG_SC` | `H<0.5` |
| 2 | `R30-NET-PMFG-HLT05` | `PERIPHERAL_PMFG` | `H<0.5` |
| 3 | `R30-NET-PMFG-SC-HOFF` | `PERIPHERAL_PMFG_SC` | OFF |
| 4 | `R30-NET-PMFG-HOFF` | `PERIPHERAL_PMFG` | OFF |
| 5 | `R30-NET-CLASSIC-HLT05` | `CLASSIC_EG` | `H<0.5` |
| 6 | `R30-NET-CLASSIC-HOFF` | `CLASSIC_EG` | OFF |

每个 ID 的完整 fingerprint：

```text
R30-DIRECT-EG-BONF2-BH05-<selector>-GHE<OFF|LT0.5>-Z1-SO1.25-1.50-1.75-STOP2-
OPENPHASE-ENTRYBAR-CALENDAR12-BETA
```

manifest 中 `policy_id`、`fingerprint`、selector 与 Hurst state 必须逐行匹配上表；禁止 shorthand 或运行后改名。

### 2.3 总配额与 visible trial floor

```text
Round 1-28 visible floor = 1145
Round 29 frozen attempts  = 24
Round 30 new baselines    = 6
baseline visible floor    = 1175
r29_ensemble_executed     = e29 in {0,1}
r30_ensemble_executed     = e30 in {0,1}
DSR N                     = 1175 + e29 + e30
maximum visible floor     = 1177
```

即使 Round 29 evidence 缺失，它已经作为公开研究决策暴露，24 个策略仍计入 DSR trial floor。recovery replay
不得再次加 24。条件 ensemble 只有实际满足预注册条件并执行/导入时才加 1；不得漏算 Round 29 自身 ensemble，
也不得把同一个 replay 重复计数。所有 zero-signal/negative/invalid-terminal policies 保留在统计分母。

### 2.4 永久禁止重复

```text
普通 multiplier/spacing/TP/SL 参数网格
ATR/ADX/EMA/RSI/Donchian
first-passage/hazard exact arms
Micro-Martingale/Integral TP、Soft-Martingale/SEL
depth-controlled spot Martin 42% DD preprint
funding standalone PnL、trend/breakout directional sleeve
finished-curve allocator/blending
旧 PC1/Johansen/VECM/Kalman/partial-cointegration exact fingerprints
旧 TAR/MTAR activation gate
单币 ANKR、旧 DGT
RL/DNN/LSTM/GA、outer-return threshold tuning
dynamic/mixed/vine copula expansion
Round 26-29 exact maximum matching/C0/MPI neighbors
```

本轮不得看到结果后追加更多 graph filter、community algorithm、H threshold、GHE lag、z threshold、frequency、
formation length、SO spacing 或 sector universe。新想法只进入 handoff hypothesis ledger。

## 3. Git、授权与可恢复执行

### 3.1 分支创建

从包含本任务书的 clean plan commit 开始：

```bash
git status --short --branch
test -z "$(git status --porcelain)"
PLAN_COMMIT="$(git rev-parse HEAD)"
git switch -c glm-martingale-core-round30 "$PLAN_COMMIT"
```

若分支存在：

```bash
git switch glm-martingale-core-round30
git merge-base --is-ancestor "$PLAN_COMMIT" HEAD
```

规则：

1. clean local plan/source commit 足以运行；执行前不要求 pushed parent。
2. 不得因 GitHub、remote、push 或 tool approval 停在 G-1/G0。
3. 最终 push 仅是交付步骤；失败时本地 replay/validator/evidence 仍全部完成。
4. commit body 必须包含 `问题描述:`、`复现路径:`、`修复思路:`。

### 3.2 Immutable roots

```text
repo artifact = docs/superpowers/artifacts/glm-martingale-core-round30/
raw artifact  = artifacts-local/round30/<SOURCE_COMMIT>/
handoff       = docs/superpowers/reports/2026-07-27-glm-round30-handoff.md
```

`--resume` 只可跳过 source commit/tree、data/policy/input hashes 完全相同且 status terminal 的阶段。修改
model/replay/validator 代码必须新 commit/new raw root；旧 root 保留并标记 `superseded_implementation`。

OOM 只允许 workers `8 -> 4 -> 2` 并记录；不得减少 pair、policy、budget、cold-start 或 stress 配额。

## 4. 最小实现范围

建议新增：

```text
scripts/r30_fit_network.py
scripts/r30_validate_models.py
crates/r24-research/src/r30.rs
crates/r24-research/src/bin/r30_execute.rs
crates/r24-research/src/bin/r30_validate.rs
docs/superpowers/artifacts/glm-martingale-core-round30/round30-protocol.json
docs/superpowers/artifacts/glm-martingale-core-round30/round30-policy-manifest.json
docs/superpowers/artifacts/glm-martingale-core-round30/round30-source-map.json
docs/superpowers/artifacts/glm-martingale-core-round30/round30-environment-manifest.json
```

允许修改：

```text
crates/r24-engine/src/lib.rs
crates/r24-research/src/lib.rs
```

必须复用一个 SharedAccount 和一个 causal open/path engine。不得为 network family 写简化收益引擎。Round 28
及可能后续同步的 Round 29 artifacts 均保持不可变。

## 5. 外部来源冻结

### 5.1 Network periphery

```text
title = Embedding pairs trading in market networks: a network science approach to portfolio construction
DOI = 10.1057/s41599-025-05661-7
HTML = https://www.nature.com/articles/s41599-025-05661-7
PDF = https://www.nature.com/articles/s41599-025-05661-7.pdf
PDF SHA256 = bcaacacc7de80abf95828d024ddd6672b43c82a61d74bd12539a8dea2e253d03
Table 2 = https://www.nature.com/articles/s41599-025-05661-7/tables/2
adopted = 1h, 28d, weekly, EG graph, PMFG, X+Y periphery, same-community strong ties
project adaptation = symmetric direction test, BH q=0.05, edge strength=-ln(p_sym), deterministic matching
rejected = paper returns, random 20-pair Monte Carlo, no-cost execution
```

来源同时研究 TMFG，但本轮不执行：29-node tradable sparse graph 常不存在标准 TMFG 所需 4-clique seed，容易
再次形成 label-only/zero-order arm；来源表中 PMFG 的 return/tail-risk/Sortino 也更适合作为唯一 graph filter。

### 5.2 Local Hurst

```text
title = Anti-Persistent Values of the Hurst Exponent Anticipate Mean Reversion in Pairs Trading
DOI = 10.3390/math12182911
source URL = https://www.mdpi.com/2227-7390/12/18/2911/pdf?version=1726736500
adopted = local H<0.5 as Martin action gate
rejected = paper profit claim, threshold/estimator search
```

下载失败不阻塞 replay；source manifest 必须记录 URL、expected metadata、download status。禁止从搜索摘要猜测
其他参数。本任务书第 10.2 节给出唯一可执行 GHE adaptation。

## 6. D0：冻结数据与 calendar

只使用现有本地 inputs，不联网刷新行情：

```text
data/market_data_full.db
data/funding_rates.db
Round 28 exchangeInfo/filter snapshot
Round 28 maintenance source
```

冻结 bytes/hash，不允许只比较路径：

```text
market bytes  = 119277240320
market SHA256 = 36ec45fed5d048a783e16ecc0f90ea46e0ffedededf157d25f127edf7d19baa6
funding bytes = 7417856
funding SHA256= 356e270dacce36b4703364545beeafe402f1aa55aa11caebc5708f5f3b6767a1
exchangeInfo SHA256 = aa6b21bbb04a421ffa72e8755312cdb20de684a9e016400c14fad7ee94f9be7b
maintenance source SHA256 = 00eb93bcfe82772ef28a574ff8f1aa3e6a599c10054b44f286db20c649efa687
```

流式重验：

```text
SHA256、bytes、SQLite schema/version
每 symbol first/last/count/missing/duplicate/close-time contract
funding first/last/expected/actual/missing/duplicate
filter tick/step/minQty/minNotional
maintenance source/version/hash
```

外层固定：

```text
[2023-07-01T00:00:00Z, 2026-05-29T16:00:00Z)
logical 1m rows = 1,531,680
weekly anchors = anchor_k = 2023-07-01T00:00:00Z + 7d*k, k=0..151
last anchor = 2026-05-23T00:00:00Z
earliest 28d formation start = 2023-06-03T00:00:00Z
```

12 calendar blocks 与 Round 29 相同。禁止 clamp；越界直接 fail。D0 hash 与 Round 28 冻结值不同则合法终态为
`VALID_DATA_CHANGED_NO_REPLAY`。

固定 29-alt universe，必须 29/29 在每个 formation window 有完整数据：

```text
AAVE ADA ALGO APT ATOM AVAX BCH BNB COMP CRV DASH DOGE DOT DYDX EGLD
ETC ETH FIL GALA HBAR ICP INJ LINK NEAR SOL TRX UNI XRP ZEC
```

BTC 不属于可交易 universe。

基础账户与指标语义同样冻结：

```text
fee = 4 bps per filled leg per side
slippage = 2 bps per filled leg per side
liquidation fee = 50 bps
close reserve = 10 bps
maintenance rate = 2.5%
funding = raw symbol/timestamp rate from frozen DB, no interpolation
years = (end_exclusive-effective_start)/(365.25*86400s)
ann_pct = ((final_equity/principal)^(1/years)-1)*100
DD = maximum continuous shared-account equity peak-to-trough, including intraminute adverse path
daily return = UTC end-of-day equity / prior UTC end-of-day equity - 1
```

`effective_start` 只在 cold-start rerun 中后移；baseline 固定为 outer start。任何 equity `<=0`、缺日界 equity 或
production/validator annualization 差异 `>1e-9` 都 fail-close。D0 同时记录 Python、NumPy、SciPy、statsmodels、
NetworkX、Rust 和 SQLite 版本；环境版本变化不自动改结果，但必须进入 source/input hash。

## 7. G-1：Round 29 authority recovery

这是不可跳过的第一阶段。

### 7.1 查找与 manifest

先执行一次 best-effort remote refresh：

```bash
GIT_TERMINAL_PROMPT=0 timeout 60s git fetch --all --prune
FETCH_EXIT=$?
```

不要在启用 `set -e` 的 shell 中裸跑上述命令；无论 `FETCH_EXIT` 是否为 0 都继续。fetch 成功或失败都写入
discovery；网络/授权失败不得阻塞本地查找或 mandatory replay。然后检查：

```text
refs/heads/glm-martingale-core-round29
refs/remotes/origin/glm-martingale-core-round29
all refs matching *round29* (including codex/* prefixes)
all worktrees、reflogs and unreachable commits
docs/superpowers/artifacts/glm-martingale-core-round29/
docs/superpowers/reports/2026-07-27-glm-round29-handoff.md
artifacts-local/round29/*
```

固定 discovery commands：

```bash
git for-each-ref --format='%(refname) %(objectname)' refs/heads refs/remotes
git worktree list --porcelain
git reflog --all
git fsck --no-reflogs --unreachable
```

对每个 ref/commit 用 `git ls-tree -r` 查 Round 29 handoff/artifact 路径，不能只看当前 checkout。

输出 `round29-evidence-discovery.json`，逐项记录 path/ref、exists、commit/tree、bytes、mtime、hash，不允许仅写
`not found` 文本。

### 7.2 可导入条件

只有同时满足才可导入：

```text
source commit descends from c90d79c5
source tree 与 raw root manifest 一致
data/policy/input hashes 与 Round 29 plan 一致
16 C0 + 8 MPI 全 terminal
causal open/path、family adverse、entry-minute risk、post-funding maintenance pass
daily return matrix 和完整 account/risk traces存在
model/account validators 与语义 mutants pass
G3/anti-overfit/LOSO/LOPO 适用项 terminal
final state zero、BTC zero
```

必须在当前机器重新运行 tests 和 validators；不能只读取 `passed:true`。Round 29 旧任务书的 aggressive label 是
`100%`，本轮不得继承该 target claim：可复用已验证的同一 FO/cap raw trace，但必须由当前 validator 按 `110%/30%`
重新生成 tier authority。

### 7.3 Mandatory replay 路径

任一条件不满足：

```text
round29_import_status = rejected
first_failed_condition = exact field
```

然后在当前修复 engine 上原样重放 Round 29 的 24 policies。fingerprint 保留 Round 29 exact policy ID，并增加
`R30-RECOVERY-OF-R29` implementation suffix；trial count 不重复增加。

Round 29 recovery 必须产出独立：

```text
r29-recovery/24 replay results
r29-recovery/daily matrix
r29-recovery/model validator
r29-recovery/account validator
r29-recovery/mutants
r29-recovery/corrected authority
```

Round 29 第 14.1 节 ensemble 条件适用时，还必须有独立 ensemble trace/result/validator terminal，并设置
`e29=1`；不适用时写明两个 family 的 P-B denominator 并设置 `e29=0`。0 P-B 也必须完成全部 24 terminal 和
统计输入。G-1 未 terminal，不得运行 network family。

## 8. R0：共同执行器硬门

每分钟唯一顺序：

```text
OPEN PHASE
O1 mark pre-existing positions at t.open
O2 open-time maintenance/liquidation
O3 funding for positions existing before t
O3b post-funding maintenance/liquidation before any exit/SO/FO
O4 matured one-leg delayed pending orders
O5 completed_signal_ms < t.open_ms frozen state
O6 forced/source/neutral exits
O7 active-group SO, group_id asc
O8 new FO intents, canonical sort

BAR PATH PHASE
P1 apply t.high/t.low to all positions including O4-O8 fills
P2 DD/maintenance/liquidation
P3 mark t.close
P4 account/risk trace and running peak
```

open helper 只能接收 `timestamp/open` 类型；禁止传入完整 bar。所有新增 network/Hurst 路径必须使用相同
phase contract。

必须保留 Round 24-29 reserve/filter/restart/Copula/calendar/BTC-zero canaries，并增加本计划第 12 节测试。

## 9. Network formation：唯一公式

### 9.1 Weekly direct EG graph

每周 anchor 仅使用 `[anchor-672h, anchor)` 的 completed 1h close。对 canonical pair `A < B` 同时拟合：

```text
log(A) = alpha_ab + beta_ab * log(B) + e_ab
log(B) = alpha_ba + beta_ba * log(A) + e_ba
```

对两个 residual 做 `statsmodels.adfuller(regression="c", maxlag=1, autolag=None)`，并对两个方向作固定
Bonferroni correction：

```text
p_sym = min(1, 2*min(p_adf(e_ab), p_adf(e_ba)))
selected orientation = lower raw ADF p; tie uses canonical A-on-B
```

每个 anchor 对全部 406 个 `p_sym` 运行一次 Benjamini-Hochberg，冻结 `q=0.05`：按
`(p_sym asc,pair_id asc)` 排序，取最大 `k` 满足 `p_(k)<=k*0.05/406`；前 `k` 个为 BH pass，`k=0` 合法。
三个 selector 必须共用同一 BH edge population。tradable edge iff：

```text
  BH q=0.05 pass
  selected beta>0
  selected residual sigma finite and >0
  formation coverage =672/672
```

对 label swap 必须得到相同 `p_sym`、edge eligibility 和 canonical pair ID。edge strength：

```text
strength = -ln(max(p_sym, 1e-12))
distance = 1 / strength
```

不得删除 no-fit/nonfinite pair 后缩小 BH denominator；406 永远是 denominator，非法 p-value 按 1.0。不得使用
trading-week return、future half-life 或 future Hurst 排 edge。

### 9.2 CLASSIC_EG control

从全部 tradable edges 运行 deterministic exact matching，最多 6 edges。目标 tuple：

```text
maximize cardinality
maximize sum strength
lexicographically minimize sorted pair IDs
```

这是 network selector 的 changed-mechanism control，必须与 PMFG variants 使用相同 signal/account 参数。

### 9.3 Sparse PMFG

按 `(strength desc, pair_id asc)` 遍历 tradable edges；只有新增 edge 后图仍 planar 才保留。不得在失败后换
edge score 或补 nonstationary edge。用 NetworkX `check_planarity` 的确定性布尔结果，fit 与 independent validator
分别从 edge ledger 重建；每个 connected component 独立允许 planar。输出完整 accepted/rejected-edge ledger、
piecewise planar 上界（`N<2:0, N=2:1, N>=3:3N-6`）和 Kuratowski counterexample（若库可返回）。

### 9.4 X+Y peripherality

图节点集 `V` 是至少连接一条 accepted PMFG edge 的 endpoint；29-alt 中 isolate 只记入 `isolated_nodes`，不参加
外围 rank，也不能产生 candidate edge。`N=|V|`；`N<2` 时该 anchor 生成明确 zero-network terminal。

在每个过滤后正权图上计算 weighted/unweighted：

```text
Degree D
Betweenness BC
Eccentricity E (within connected component)
Closeness C (Wasserman-Faust disconnected correction)
Eigenvector centrality EC (full V, frozen iteration below)
weighted shortest-path distance = 1/strength
```

断连图固定语义：degree/betweenness 在完整 `V` 上计算；eccentricity 只取本 component 的 finite farthest distance；
closeness 使用 Wasserman-Faust disconnected correction；eigenvector 使用全 `V`、uniform positive start 的 power
iteration（max_iter=10000、tol=1e-12），不收敛则该 anchor `centrality_nonconverged`，不得换算法。所有 normalized
centrality 采用 NetworkX 3.5 定义并在 environment manifest 绑定版本。

按来源：D/BC/EC descending，E/C ascending；ties 用 midrank。对 N 个 graph nodes：

```text
X=(CDw+CDu+CBCw+CBCu-4)/(4*(N-1))
Y=(CEw+CEu+CCw+CCu+CECw+CECu-6)/(6*(N-1))
periphery = X+Y
```

按 `(periphery desc, symbol asc)` 取前 `ceil(N/4)` nodes；边界 tie 全保留并记录实际数量。

### 9.5 Deterministic strong-tie communities

使用固定 `DET-LOUVAIN-LEX`：

```text
resolution=1.0
undirected weight=strength, no self-loop
Q=(1/(2m))*sum_ij(Aij-resolution*k_i*k_j/(2m))*I(c_i=c_j)
initial community = one node each
node visit order = symbol asc
for each candidate move recompute exact Q on the current small graph
move to neighboring community with maximum positive modularity gain >1e-12
gain tie = community minimum symbol asc
repeat passes until no move
aggregate and repeat until modularity gain <=1e-12
canonical community ID = sorted member hash
```

不得使用随机 seed 或因结果换 Louvain/Infomap/SBM。`PERIPHERAL_PMFG_SC` candidate edge 必须：

```text
edge appears in PMFG
at least one endpoint is peripheral
both endpoints share deterministic community
```

`PERIPHERAL_PMFG` 只要求前两项，作为 same-community gate 的固定对照。若不足 3 pairs，不 fallback 到
non-structural edge；`_SC` 不得 fallback 到跨社区，均生成完整 zero/partial terminal。

### 9.6 Final network pair set

从 candidate edges 做 exact constrained matching，最多 6 edges、每 community 最多 2 条 incident selected edges。
同社区 edge 消耗该 community 一个 quota；跨社区 edge 同时消耗两个 endpoint communities 各一个 quota。
`distinct community count` 是全部 selected endpoints 的 community union。目标 tuple：

```text
maximize cardinality
maximize distinct community count
maximize sum endpoint periphery
maximize sum strength
lexicographically minimize sorted pair IDs
```

active group 跨 model roll 继续使用 open-time frozen beta/mu/sigma/network/model hash；同 pair 不允许新旧 model
同时 active。active/pending group 还实行全账户 symbol exclusivity：任何新 FO 与已有 active/pending group 共用任一
symbol 都写 `active_symbol_conflict` 并拒绝；同分钟多个新 FO 按 `(eligible_open_ms,pair_id,model_hash,direction)`
排序，先成功者占用 symbol。该规则对 Round 29、Round 30 和所有 ensemble 一致。

## 10. Direct-z Martin 状态机

### 10.1 Spread 与方向

对每条 canonical pair 使用第 9.1 节 selected orientation；`L` 是 dependent leg，`R` 是 regressor leg：

```text
s_t = log(L_t) - alpha_lr - beta_lr*log(R_t)
z_t = (s_t - formation_mean(e_lr)) / formation_sigma(e_lr)
```

completed 1h bar 更新 signal；下一可用 1m open 执行。

```text
z crosses +1 from inside -> SHORT L / LONG R
z crosses -1 from inside -> LONG L / SHORT R
```

每个 successful paired fill 的 gross weights：

```text
w_L = 1/(1+beta_lr)
w_R = beta_lr/(1+beta_lr)
```

过 filters 后两腿目标 gross mismatch `<=5%`，否则 causal reject。

### 10.2 Local GHE

只对 `H_LT_0.5` arm，在每个 completed 1h signal bar，用 frozen beta 和过去 672 个 completed spreads：

```text
q=1
lags tau=1..24
K(tau)=mean(|s[t+tau]-s[t]|)
H = OLS slope of log(K(tau)) on log(tau)
```

要求 672/672 finite、全部 K>0、OLS finite；否则 `hurst_unavailable`，该 bar 不 FO。唯一门：

```text
FO requires H<0.5
SO requires current H<0.5
```

H>=0.5 只 freeze SO，不强制提前退出；zero/stop/deadline 仍执行。OFF arm 完全不计算 H 作为决策输入。

### 10.3 Martin layers 与 exits

固定：

```text
FO crossing = |z| 1.00
SO1 = adverse |z| 1.25
SO2 = adverse |z| 1.50
SO3 = adverse |z| 1.75
source stop = adverse |z| 2.00
relative gross layers = [1.00,1.25,1.55,1.90]
deadline = 7d
```

每个 SO 同时要求：

```text
group all-in net <0
z remains on original adverse side
threshold first crossing
H gate pass when enabled
paired atomic fill or paired flatten
reserve/filter/concentration/leverage pass
```

单个 group 每个 completed 1h signal 最多成功一个 SO；若 z gap 一次跨过多层，只尝试最低尚未成交层，其余层等
后续 completed signal。open-phase 优先级固定为 source stop/deadline/zero exit > SO > 新 FO；同一 signal 同时满足
强制 exit 与 SO 时只执行 exit。

退出：

```text
z crosses zero -> close next eligible open, even if net<=0
adverse |z| reaches 2 -> source_stop_abort next eligible open
age 7d -> deadline_abort
END -> end_of_data_abort
```

任何 closure 后 state reset。不得增加 neutral band、partial TP、trailing 或 wider stop。

## 11. G1：Return-blind census

每个 anchor/selector 输出：

```text
29 node coverage
812 directional EG fits / 406 canonical pairs denominator
raw p_sym<=.05 edges、components、PMFG accepted/rejected edges
BH k/406、BH pass/fail/no-fit denominators
planarity terminal counts
X/Y/periphery ranks、community membership/modularity
classic/network selected pairs and model hashes
z updates/crossings、H available/pass/fail
distinct eligible alts/pairs/communities
signals by 12 blocks
```

activity diagnostic：

```text
eligible alts >=6
eligible pairs >=3
FO crossings >=30
crossings across >=8/12 blocks
single symbol crossing share <=50%
```

G1 低 activity 不得跳过 G2；6 policies 全部进入 shared-account replay。只有 D0/R0/G-1 工程失败可停止。

## 12. G0/model canaries

至少新增并通过：

```text
r29_missing_evidence_forces_recovery_not_valid_status
r29_import_rejects_wrong_source_tree_or_missing_raw_hash
eg_edge_is_label_swap_invariant
bh_denominator_stays_406_with_invalid_p_values
bh_tie_break_is_pair_id_stable
future_shift_cannot_change_prior_week_network
pmfg_accepts_only_planar_prefix_edges
pmfg_tie_break_is_pair_id_stable
xy_periphery_matches_frozen_toy_graph
det_louvain_is_repeatable_and_canonical
cross_community_edge_is_rejected
network_matching_is_disjoint_and_community_capped
active_symbol_conflict_is_global_across_model_roll_and_family
classic_control_uses_identical_signal_and_account_parameters
positive_z_is_short_a_long_b
negative_z_is_long_a_short_b
so_adverse_direction_is_family_correct
ghe_q1_fixture_recovers_frozen_slope
hurst_future_shift_cannot_change_prior_decision
hurst_off_order_hash_ignores_hurst_values
hurst_fail_freezes_so_but_does_not_skip_forced_exit
gap_crossing_can_fill_at_most_one_so_per_completed_bar
new_fo_and_so_enter_same_minute_adverse_path
funding_debit_can_liquidate_before_exit_or_new_fill
all_network_orders_are_owned_by_martin_groups
six_alt_three_pair_synthetic_account_reconciles_to_zero
```

建立 deterministic 8-alt/4-pair synthetic graph path，包含 PMFG、两个 communities、FO/SO1/SO2、H freeze、
zero-loss exit、entry-minute DD、funding、partial fill、single-leg delay、reject/flatten、restart 和 final zero state。

## 13. G2：30 baseline shared-account matrix

Round 29 24 controls 使用导入或 recovery traces；Round 30 恰好使用第 2.2 节六个 exact policy IDs。baseline daily
matrix 恰好 30 行，Round 29 自身 ensemble 与 Round 30 cross-family ensemble 都不混入 CSCV baseline selection
matrix，但各自保留 daily series 并按第 2.3 节计 trial。

Round 30 6 policies 固定：

```text
principal=2000U
FO group gross=max(filter feasible minimum, 5% principal)
relative layers=[1.00,1.25,1.55,1.90]
max active groups=3
leverage cap=2x
one continuous account across 12 blocks
```

block 边界不重置 wallet/positions/groups/reserve/running peak。weekly model roll 只影响新 group。所有 intents
chronological merge；禁止 per-pair account。全账户 active/pending symbol exclusivity 与第 9.6 节一致。

P-A：

```text
D0/G-1/R0/model/account validators pass
trace chronology/open-phase/entry-minute path pass
logical risk rows complete
every signal onset terminal
final positions/groups/pending/reserve=0
BTC zero
no wallet/cost/margin mismatch
```

P-B：

```text
P-A
compounded return >0
positive blocks >=8/12
closed Martin cycles >=30
actual alts >=6 and pairs >=3
successful SO groups >=2 and loss-after-add groups >=2
symbol/pair/group/block positive contribution each <=50%
all-in cost/gross positive PnL <=50%
no liquidation/principal breach
```

0 P-B 仍发布全 30-policy matrix，不得只报最好 policy。

### 13.1 唯一可选 ensemble

仅当至少一个 validated Round 29 P-B 和至少一个 Round 30 network P-B 时执行一个真实 shared-account ensemble。
按 policy manifest 固定 priority 取第一个，不按收益：

```text
Round 29: MPI-1h-EMPIRICAL-EQUAL, then C0 priority from Round 29 plan
Round 30: R30-NET-PMFG-SC-HLT05, R30-NET-PMFG-HLT05,
          R30-NET-PMFG-SC-HOFF, R30-NET-PMFG-HOFF,
          R30-NET-CLASSIC-HLT05, R30-NET-CLASSIC-HOFF
```

两 family intents 先合并，各 50% ex-ante reserve quota，未用额度留现金。必须重跑，不得组合 curves。
条件不满足写 `not_applicable` 且 `e30=0`；满足后必须执行、双 validator 通过并设置 `e30=1`。Round 29 自身
C0+MPI ensemble 不能替代这里的一个 Round 29 baseline parent，也不能重复计为 e30。

## 14. G3：三档、小本金与 stress

对全部 P-B、适用 Round 29 自身 ensemble 和适用 Round 30 cross-family ensemble：

```text
conservative: FO=5.0%, cap=2x, target=50%/10%
balanced:     FO=7.5%, cap=3x, target=90%/20%
aggressive:   FO=10.0%, cap=4x, target=110%/30%

budgets: 500/750/1000/1500/2000/3000/4000/4999U
cold starts: 0/30/60/90/120d
```

每行重新跑 orders/filters/reserve/funding/1m path。target 至少两个相邻 budgets 同时通过。只报告：

```text
filter_reserve_lower_bound (necessary, not sufficient)
smallest_passing_frozen_budget
```

单个 `(policy,tier,budget)` 的 base pass 唯一定义：

```text
5/5 cold-start rows terminal and P-A/final-zero/BTC-zero
cold0 ann >= tier target and cold0 DD <= tier cap
positive cold starts >=4/5 conservative, >=4/5 balanced, >=3/5 aggressive
each cold start has no liquidation/principal breach
each row actual traded alts>=6 and pairs>=3, unless no-trade then explicit fail
policy P-B and all applicable anti-overfit/LOSO/LOPO gates pass
```

target hit 还要求两个相邻 frozen budgets 均 base pass，且两者各自的 17 项 cold0 stress 全 pass。相邻只按上述
预算数组位置定义，例如 `4000/4999` 相邻；不得把同一预算的不同 cold start 当相邻预算。

固定 stress：

```text
cost 1.5x/2x
partial fill 25%/50%
left delay 1/2/3m
right delay 1/2/3m
one-leg reject + immediate paired flatten
minNotional 2x
tick/step one level coarser
maintenance +5%
signal stale stride 3
kill/restart/reconcile
worst combined
```

恰好 `17` 个 stress IDs：2 cost + 2 partial + 3 left-delay + 3 right-delay + 1 second-leg-reject + 1
minNotional + 1 coarse-filter + 1 maintenance + 1 stride + 1 restart + 1 worst-combined。固定语义：

```text
partial25/50 = 两腿在原 open 按相同比例成交，余量取消，不放大到 minNotional
left/right delay = 仅指定腿延迟，另一腿原 open 全额成交；延迟腿按第 1/2/3 个后续可用 1m open 成交
second-leg-reject = 仅对 base trace 中按 (eligible_open_ms,pair_id,group_id) 排序的首个实际双腿入场事件注入；
                    canonical order sequence 的第二腿拒绝，已成交第一腿在同一 open 立即平掉并计双边成本
coarse-filter = tickSize*10 and stepSize*10；minQty 不变
stride3 = 全局 chronological completed-signal ordinal 满足 ordinal mod 3 == 0 的 row 丢弃
restart = outer interval 时间中点后的第一分钟序列化、清空进程内存、重新加载并逐字段 reconcile
```

worst combined 必含 cost2x、partial25%、单腿 delay3m、reject_second_leg、2x minNotional、coarse filter、
maintenance+5%、stride3、restart。所有 stress compounded return >0、无 liquidation/breach、DD 不超 tier cap；
stress ann 不要求仍达 headline。若 base trace 没有可注入的双腿事件，reject stress 为明确 fail，不得伪造 pass。
worst-combined 的 delay leg 固定为 canonical right leg；不得看结果后选择较有利的一腿。所有 stress 的 actual
alt/pair 数也必须报告；stress 可因真实 filter/拒单少于 6/3，这本身不推翻 base 的多币身份，但不能删除其
return/DD/reconciliation survival gate。

## 15. 防过拟合硬门

使用 30 baseline 的完整 daily shared-account matrix；optional ensemble 不参与 baseline selection matrix，但计 DSR
global trial floor。

### 15.1 CSCV/PBO

```text
S=16 chronological equal blocks
all C(16,8)=12,870 splits
train daily Sharpe 选 policy，test daily Sharpe rank/logit
PBO<0.50
```

16 blocks 按 daily-row ordinal 连续等分，前 `n mod 16` blocks 各多一日。全局 tie priority 是 Round 29 的
24 policies 按其 manifest 顺序在前，再接第 2.2 节六个 Round 30 policies。test rank 使用 30 policies 的
ascending midrank，
`omega=rank/(30+1)`、`logit=ln(omega/(1-omega))`，`logit<=0` 计 overfit。zero/no-trade policy 用实际 zero
daily series，不得删除。

### 15.2 PSR/DSR

使用 Round 29 标准公式、未年化 daily Sharpe、Euler-Mascheroni correction：

```text
N=1175+e29+e30, 即 1175/1176/1177
candidate DSR probability >=0.95
```

SR dispersion 输入为 30 baseline 的完整 daily Sharpe vector，并另外公开实际执行 ensemble 的 Sharpe；全局 N
按历史 visible floor 做 multiplicity correction。保存 n、mean/std/skew/kurtosis、e29/e30、N、expected max
Sharpe 与全部中间项。由于早期 1145 trials 没有统一 daily matrix，报告必须把“当前 30-row dispersion + 历史 N”
标为预注册 global-N approximation，不得声称它重建了全部历史 Sharpe，也不得把它描述为必然保守；同时以
cross-policy SR standard deviation 的 `x 0.5/1.0/1.5/2.0` 发布 sensitivity，主门仍只用 frozen 1.0。禁止旧
`sqrt(2lnN)` proxy。

### 15.3 Hansen SPA/bootstrap

```text
Hansen SPA daily excess returns, benchmark=0
stationary bootstrap 9,999, expected block=20d
SPA p<=0.10

moving-block bootstrap daily returns
9,999, block=20d
annualized return 95% lower bound >0
```

与独立 Python reference 同 frozen matrix 对齐。

### 15.4 Full leave-out reruns

每个 P-B：

```text
LOSO: 删除一个实际成交 symbol，从 scheduler 到 account 全重跑
LOPO: 删除一个实际成交 pair，从 scheduler 到 account 全重跑
>=80% LOSO and >=80% LOPO compounded return >0
all final reconciled/no liquidation
```

禁止用 contribution subtraction。

### 15.5 Network stability

每个 network candidate 另报但不按结果调参：

```text
weekly node/edge Jaccard
selected-pair turnover
community adjusted Rand index
peripheral-node turnover
actual active community concentration
```

P-C 进展门：

```text
fixed diagnostic = principal2000U / FO5% / cap2x
ann>=35%, DD<=20%, >=4/5 cold starts positive
actual alts>=6, pairs>=3
P-A/P-B/DSR/PBO/SPA/bootstrap/LOSO/LOPO/stress all pass
```

P-C 的 cold starts 与 17 stress 直接复用 G3 conservative-2000 的真实 reruns，但按诊断门 `35%/20%` 独立判定，
不新增 trial、不缩放 curve。每个 stress 仍需正收益、DD<=20%、无 liquidation/breach 和 final zero。
P-C 不是 target hit。

## 16. 双独立 validator

### 16.1 Model/network validator

`scripts/r30_validate_models.py` 从 raw DB 独立复算，不调用 production helpers：

```text
29-alt weekly coverage
bidirectional OLS/ADF/p_sym、406-denominator BH q=0.05、strength
classic exact matching
PMFG planarity and edge order
weighted/unweighted centralities、X/Y、peripheral nodes
DET-LOUVAIN-LEX memberships/modularity
same-community constrained matching
spread/z/direction/beta weights
GHE K(tau)/slope/H gate
future-shift causality
all source/data/model hashes
```

每种 selector 至少复算真实 finite anchor；无 fit 时输出 denominator/terminal，不能手写 pass。

### 16.2 Account validator

`r30_validate` 只读 raw DB、signal/order/fill/account/risk traces，从零重建：

```text
open-phase allowed fields and sequence
post-funding maintenance
entry-minute adverse path
filters/qty/side/mode/fills/rejects
pending single-leg delay and legging exposure
concurrent ownership/reserve/margin/wallet/equity
FO/SO/exit/freeze/abort counts
calendar PnL/concentrations/daily returns/DD
budget/cold-start/stress/restart/final zero
P-A/P-B/P-C/tier/anti-overfit inputs
```

不能调用 production replay/metric functions，不能信任 result JSON counts。
target validator 必须逐个确认 8 budgets x 5 cold starts、相邻 budget tuple、每个 finalist/tier/budget 恰好 17 个
stress IDs，以及 reject mutant 实际命中预冻结 event ID；缺一项即 target false。

### 16.3 Mutants

至少逐一篡改并要求非零退出：

```text
Round 29 source tree/raw hash
EG orientation p-value
BH rank/admission
one PMFG forbidden crossing edge
periphery rank
community assignment
cross-community admission
matching overlap/community cap
future bar in graph
z direction/SO threshold
H value/gate
current close in open decision
entry-minute path
post-funding maintenance
side/mode/qty/fill/funding/wallet/calendar block
worst-combined second-leg reject
```

mutant 必须命中实际对应事件；不能总改第一行或仅靠 row count。

## 17. 唯一执行顺序与命令

### 17.1 实现、测试、source commit

```bash
python3 -m py_compile scripts/r30_fit_network.py scripts/r30_validate_models.py
cargo fmt --package r24-engine --package r24-research -- --check
cargo test -p r24-engine -p r24-research
cargo build --release -p r24-research --bin r30_execute --bin r30_validate
git diff --check

git add scripts/r30_fit_network.py scripts/r30_validate_models.py \
  crates/r24-engine/src/lib.rs crates/r24-research/src/r30.rs \
  crates/r24-research/src/bin/r30_execute.rs \
  crates/r24-research/src/bin/r30_validate.rs \
  crates/r24-research/src/lib.rs
git add -f docs/superpowers/artifacts/glm-martingale-core-round30
git commit -m "feat(r30): recover authority and add network Martin" \
  -m "问题描述: Round 29 没有可审计执行证据，旧 pairwise selector 也未利用 crypto cointegration network 的外围结构。" \
  -m "复现路径: 运行 Round 29 evidence discovery、错误 source/raw import、PMFG toy graph、Hurst causality 和 shared-account canaries。" \
  -m "修复思路: fail-close 恢复 Round 29 权威，并实现 formation-only network periphery selector、固定 GHE gate 和统一 causal Martin engine。"

test -z "$(git status --porcelain)"
SOURCE_COMMIT="$(git rev-parse HEAD)"
RAW_ROOT="artifacts-local/round30/${SOURCE_COMMIT}"
mkdir -p "$RAW_ROOT"
```

### 17.2 D0/G-1/R0

```bash
target/release/r30_execute --phase d0 --artifact-root "$RAW_ROOT" --resume
target/release/r30_execute --phase r29-evidence-discovery --artifact-root "$RAW_ROOT" --resume
target/release/r30_execute --phase r29-recovery --artifact-root "$RAW_ROOT" --resume
target/release/r30_execute --phase r0-canaries --artifact-root "$RAW_ROOT" --resume

OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 \
python3 scripts/r30_validate_models.py \
  --phase r29-recovery --artifact-root "$RAW_ROOT" --workers 8 --resume

target/release/r30_validate --phase r29-recovery --artifact-root "$RAW_ROOT" --resume
```

`r29-recovery` 可合法导入已验证 evidence 或完整重跑，但必须生成明确 terminal manifest。

### 17.3 Network fit/G1/model validation

```bash
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 \
python3 scripts/r30_fit_network.py \
  --phase fit-signals --artifact-root "$RAW_ROOT" --workers 8 --resume

target/release/r30_execute --phase g1-activation --artifact-root "$RAW_ROOT" --resume

OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 \
python3 scripts/r30_validate_models.py \
  --phase all-models --artifact-root "$RAW_ROOT" --workers 8 --resume
```

### 17.4 G2/G3/final

```bash
target/release/r30_execute --phase g2-replay --artifact-root "$RAW_ROOT" --resume
target/release/r30_execute --phase g3-tiers --artifact-root "$RAW_ROOT" --resume
target/release/r30_validate --phase all --artifact-root "$RAW_ROOT" --resume
target/release/r30_validate --phase finalize --artifact-root "$RAW_ROOT" --resume
```

0 P-B 时 G3 仍输出 terminal `not_applicable` files，不能缺目录或放一个汇总占位。

### 17.5 Evidence commit/push

```bash
git add -f docs/superpowers/artifacts/glm-martingale-core-round30
git add docs/superpowers/reports/2026-07-27-glm-round30-handoff.md
git commit -m "docs(r30): publish recovered and network Martin evidence" \
  -m "问题描述: Round 30 需要用 immutable traces 给出 Round 29 修正权威及 network-periphery Martin 的完整目标判定。" \
  -m "复现路径: 核对 24 recovered controls、6 network baselines、G3、双 validator、mutants、anti-overfit 和 final zero state。" \
  -m "修复思路: 提交 compact machine authority、trial/failure ledger、trace manifests 与逐项 handoff。"

test -z "$(git status --porcelain)"
git push -u origin glm-martingale-core-round30
```

push approval 被拒时记录 `PUSH_PENDING_TOOL_APPROVAL`，不得回到 G-1 重跑。

## 18. 必交 artifacts

```text
docs/superpowers/artifacts/glm-martingale-core-round30/
  round30-protocol.json
  round30-source-map.json
  round30-data-manifest.json
  round30-environment-manifest.json
  round30-policy-manifest.json
  round30-trial-ledger.json
  round30-failure-ledger.jsonl
  round30-closed-fingerprints.json
  round30-authority.json
  round30-execution-state.json
  round29-evidence-discovery.json
  round29-recovery-authority.json
  gates/d0.json
  gates/g-minus-1-round29.json
  gates/r0-canaries.json
  gates/g1-activation.json
  gates/g2-replay.json
  gates/g3-tiers.json
  model-independent-validator.json
  account-independent-validator.json
  validator-mutants.json
  graph-manifests/
  fit-snapshot-manifests/
  signal-intent-manifests/
  replay-results/
  tier-results/
  trace-manifests/
  runtime/
```

raw root 保存 source PDFs/manifests、full graph/signal rows、account/order/fill/risk traces、daily matrix、LOSO/
LOPO、G3/stress、validator references/mutants 和 runtime logs。不得提交 `>100MB` raw files；repo 只存 hash、
bytes、rows 和 deterministic sample。

## 19. Failure ledger 与 closure

每个失败：

```text
round/family/policy_id/fingerprint
source/data/model/policy/trace hashes
trial denominator
first_failed_gate
numerator/denominator/value/threshold
causal reason
valid_implementation
replay_required_after_fix
never_repeat_rule
```

Round 29 在 recovery 前只能是 `missing_execution_evidence`。只有导入/重跑后双 validator pass，才可关闭其 exact
fingerprint。Network 负结果最多关闭本任务书冻结的 PMFG/classic + H state exact policies，禁止外推关闭
所有 graph、所有 pairs 或所有 Martin。

## 20. 合法终态与完成定义

唯一机器终态：

```text
VALID_DATA_CHANGED_NO_REPLAY
VALID_ALL_TERMINAL_NO_PB
VALID_R29_PB_NO_NETWORK_PB
VALID_NETWORK_PB_NO_PC
VALID_PC_PROGRESS_NO_TARGET
VALID_TARGET_HIT
MATERIALLY_INCOMPLETE_INVALID_RESULTS
```

判定优先级固定为 `INCOMPLETE > DATA_CHANGED > TARGET > PC > NETWORK_PB > R29_PB > NO_PB`，集合互斥：

- `VALID_DATA_CHANGED_NO_REPLAY`：D0 bytes/hash 与冻结输入不同，完整记录差异后停止，禁止比较收益。
- `VALID_ALL_TERMINAL_NO_PB`：30 baseline 全 terminal，Round 29/30 与适用 ensemble 合计 0 P-B。
- `VALID_R29_PB_NO_NETWORK_PB`：至少一个 recovered Round 29 P-B，但 6 个 network policy 为 0 P-B。
- `VALID_NETWORK_PB_NO_PC`：至少一个 network P-B，但没有 P-C。
- `VALID_PC_PROGRESS_NO_TARGET`：至少一个 P-C 全门通过，三档未命中。
- `VALID_TARGET_HIT`：至少一个 tier 在相邻 budgets、cold starts、stress、anti-overfit 和实盘门全部通过。
- 任一 mandatory stage/trace/validator/policy 缺失都只能 `MATERIALLY_INCOMPLETE_INVALID_RESULTS`。

完成必须同时满足：

```text
Round 29 24-policy authority recovered
6 network baselines terminal
适用 G3 terminal
frozen PBO/DSR-global-N/SPA/bootstrap and full LOSO/LOPO terminal
dual validators and applicable mutants pass
authority validator-generated
failure ledger/handoff complete
local evidence commit clean
```

## 21. Handoff 必须逐项回答

`docs/superpowers/reports/2026-07-27-glm-round30-handoff.md` 必须给出：

1. source commit/tree、data/policy hashes、raw root、Git/push 状态；
2. Round 29 evidence 是 imported 还是 replayed，24/24 全表与修正 authority；
3. Round 29 四项执行器修复的 real trace/tests proof；
4. 152 anchors 的 812 directional/406 canonical、Bonferroni/BH、PMFG/periphery/community/pair denominators；
5. 6 network policies 全表：ann、DD、blocks、FO/SO/exit/abort、alts/pairs/communities、cost/concentration；
6. Hurst available/pass/fail、FO veto、SO freeze 和 OFF 对照的完整计数；
7. 每个 policy overlap/reject/pending/delay/entry-minute/funding maintenance 证据；
8. P-B/P-C/target arrays；没有则明确 `[]`；
9. PBO、DSR global-N approximation 与 sensitivity、Hansen SPA、bootstrap 全公式输入/中间项，及
   `1175+e29+e30` trial floor；
10. 每个 P-B 的 full LOSO/LOPO；
11. 三档、8 budgets、5 cold starts、每个适用 tuple 的 17 stress、相邻 budget、smallest passing frozen budget；
12. 双 validator、每个 mutant、final zero 和 BTC-zero；
13. 新失败与 exact never-repeat 范围；
14. machine status、strict_valid_candidates、target_hit；
15. 明确结果是 historical prequential，不是 untouched future OOS。

完整执行后停止，不自行设计 Round 31，不在 handoff 后继续调参。
