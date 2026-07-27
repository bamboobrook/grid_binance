# Round 29 证据审计、Round 1-29 权威状态与 Round 30 方向

日期：2026-07-27
审计基线：`codex/glm-martingale-core-round29-plan@c90d79c5`
Round 29 唯一任务书：
`docs/superpowers/plans/2026-07-27-glm-martingale-core-round29-causal-open-phase-cumulative-mispricing-plan.md`

## 1. 权威结论

当前工作区、全部本地 worktree、Git object/reflog、`artifacts-local` 和 GitHub `origin` 都没有 Round 29
执行产物。能够确认的最新提交只有 Round 29 **计划**提交 `c90d79c5`，不能确认 Round 29 已经执行。

因此 Round 29 当前唯一合法状态是：

```text
MATERIALLY_INCOMPLETE_MISSING_ALL_EXECUTION_EVIDENCE
round29_source_commit = null
round29_source_tree = null
round29_raw_root = null
round29_baseline_terminal_count = 0 / 24 verifiable
round29_g3_terminal_count = 0 verifiable
round29_model_validator = missing
round29_account_validator = missing
round29_mutants = missing
round29_strict_valid_candidates = 0
round29_verified_target_hit = false
round29_target_outcome = UNKNOWN_MISSING_EVIDENCE
```

这不等于断言 GLM 没有在其他未同步环境运行；它只表示没有任何可审计证据能支持“已完成”。收益、回撤、
P-B/P-C、三档、anti-overfit 或目标命中均不得根据口头状态推断。这里的 `false` 只表示“没有已验证命中”，
不把缺失证据误写成已验证负结果。

也不能诚实地说“前 29 轮结果全部有效”。Round 1-28 继续服从上一份逐轮修正权威；Round 29 在证据同步并
通过独立复核前没有有效结果。在最新共同合同下：

```text
strict valid multi-coin Martin candidates = 0
strict valid 5/5 candidates = 0
conservative target hits = 0
balanced target hits = 0
aggressive target hits = 0
```

## 2. 实际检查

### 2.1 Git 与远端

执行并核对：

```text
git status --short --branch
git fetch --all --prune
git branch -a --list '*round29*'
git worktree list --porcelain
git reflog --all
git fsck --no-reflogs --unreachable
git ls-remote --heads origin '*round29*'
```

结果：

```text
local branch  = codex/glm-martingale-core-round29-plan@c90d79c5
remote branch = origin/codex/glm-martingale-core-round29-plan@c90d79c5
glm-martingale-core-round29 execution branch = absent
newer Round 29 commit = absent
Round 29 dangling commit = absent
worktree = clean
```

### 2.2 文件与 raw evidence

以下任务书必交项全部不存在：

```text
docs/superpowers/reports/2026-07-27-glm-round29-handoff.md
docs/superpowers/artifacts/glm-martingale-core-round29/
artifacts-local/round29/<SOURCE_COMMIT>/
scripts/r29_fit_mpi.py
scripts/r29_validate_models.py
crates/r24-research/src/r29.rs
crates/r24-research/src/bin/r29_execute.rs
crates/r24-research/src/bin/r29_validate.rs
```

全项目与 `/tmp` 也没有其他 Round 29 handoff、artifact 或 raw root。Round 29 执行中应产生的进程当前不存在。

### 2.3 计划合规矩阵

| Round 29 mandatory item | 可验证状态 | 权威结论 |
|---|---|---|
| causal open/path recovery | 无代码/trace | missing |
| family-specific adverse | 无代码/fixture | missing |
| entry-minute risk | 无代码/fixture | missing |
| funding 后 maintenance recheck | 无代码/fixture | missing |
| 16 个 corrected C0 | 0/16 artifact | missing |
| 8 个 cumulative MPI | 0/8 artifact | missing |
| D0/R0/G1/G2/G3 | 0 gate files | missing |
| 24-policy daily matrix | missing | missing |
| PBO/DSR/SPA/bootstrap | missing | missing |
| full LOSO/LOPO | missing | missing |
| 8 budgets/5 cold starts/stress | missing | missing |
| model/account validators | missing | missing |
| semantic mutants | missing | missing |
| handoff/authority/failure ledger | missing | missing |
| evidence commit/push | missing | missing |

Round 29 不是 `VALID_REPLAY_NO_PB`，因为连有效 replay 是否存在都无法确认。

## 3. Round 1-29 最新权威分类

“可保留”只表示精确 scope 的研究证据，不表示目标候选或实盘可用。

| Round | 最新状态 | 可保留范围 | 不得继续声称 |
|---:|---|---|---|
| 1 | baseline only | 初始 Martin baseline | 完整计划、`<5000U` 通过 |
| 2 | partial research | 有 artifact 的精确失败分支 | 全方向闭合、live-ready |
| 3 | partial research | 已执行的局部分支 | 全计划完整 |
| 4 | corrected baseline | canonical event-level 诊断 | 低预算、集中度、目标通过 |
| 5 | stale frontier | 历史配置定位 | 旧收益仍为当前前沿 |
| 6 | partial DD research | 精确风险归因 | 所有 state/freeze 语义闭合 |
| 7 | corrected event-level | 六币配置与修复后 replay | 小资金稳定、live-ready |
| 8 | superseded | 单 sleeve 窄范围诊断 | 无泄漏 allocator |
| 9 | curve diagnostic | finished-curve 分散线索 | shared-account strategy |
| 10 | scoped negatives | 实际运行的 exact scope | 完整 production family |
| 11 | zero target + corrected scope | 精确负面诊断 | 所有计数均为 binary replay |
| 12 | corrected partial | R4/static merge/普通 partial grid | dynamic allocator 完成 |
| 13 | materially incomplete + corrected | 60 个 canonical diagnostics | P0-P9 全完成 |
| 14 | invalid old search + corrected diagnostics | R4/R7 与少量 corrected binding | 50.70%/20.29%、35.07%/13.56% 可恢复 |
| 15 | materially incomplete + scoped | exact long-only diagnostics | D/H/C 全 family 完成 |
| 16 | materially incomplete corrected | parity、旧 ladder 局部结果 | 新 router/hazard 已进入 scored path |
| 17 | invalid mechanism search | 旧 ladder scoped negatives | 10 family 已绑定或穷尽 |
| 18 | materially incomplete invalid results | 修复后的局部 forensic rows | synchronized residual family 闭合 |
| 19 | materially incomplete invalid results | M1R 理想化 diagnostic | 五 family 完成、41.43%/6.27% 有效 |
| 20 | materially incomplete invalid results | 可复算的局部 rows | 3946 条严格有效 |
| 21 | materially incomplete invalid results | 90d 局部机制线索 | 三档命中、有效 5/5 |
| 22 | materially incomplete invalid results | 31.19%/9.87% 错误引擎诊断 | 21 机制穷尽、多币有效 |
| 23 | materially incomplete invalid results | 修正 BTC forensic result | 单币 5/5 可晋级 |
| 24 | materially incomplete invalid results | 文件证据链、失效 F1 trace | causal residual family 失败 |
| 25 | materially incomplete invalid results | invalid C1 traces | 0.12%-0.40% 是有效上限 |
| 25R | account recovery only, model invalid | reserve/filter/1m path 工程证据 | Copula 统计与收益有效 |
| 26 | materially incomplete invalid closure | D0、snapshot 与 gate decomposition | Copula Martin 已回测；实际为 0 replay |
| 27 | materially incomplete invalid G2/closure | formation snapshots、错误 trace 机械复算 | 8 个负收益可关闭 C0 |
| 28 | materially incomplete invalid G2/G3/closure | D0、模型快照、signal census | 两个 P-B、G2/G3 与 frontier 有效 |
| 29 | missing all execution evidence | 仅唯一任务书和外部来源冻结 | 已执行、任意收益/DD、C0/MPI closure |

Round 9/11 的 curve-slice `5/5`、Round 19 的 inner-train `5/5`、Round 23 的单 BTC `5/5` 都不满足
当前共享账户、多币、因果、成本和抗过拟合共同合同。严格有效 `5/5` 仍为 0。

## 4. 目标与现实边界

用户本次明确恢复激进档 `110%`，因此最新统一目标是：

| tier | annualized return | equity DD | implied minimum Calmar |
|---|---:|---:|---:|
| conservative | `>=50%` | `<=10%` | `>=5.00` |
| balanced | `>=90%` | `<=20%` | `>=4.50` |
| aggressive | `>=110%` | `<=30%` | `>=3.67` |

并同时要求本金严格 `<5000U`、真实多币 shared account、Martin 为主要且唯一交易收益引擎、长期成本后可复现。
这些是非常高的联合门槛。前 28 轮的可信证据没有显示已接近该联合边界；公开文献也没有证明这一组合必然
可达。高收益展示通常至少缺少长期 OOS、equity DD、完整成本、小本金 filters 或共享账户中的一项。

目标继续作为硬门，不下调；但 Round 30 不能承诺命中。更合理的研究目标是先得到一个通过全部真实性和
anti-overfit 门的 P-C，再判断放大到三档时是否仍在 DD cap 内。

## 5. 外部扩大检索

### 5.1 采用：cointegration network 的外围结构 pair

新来源：

```text
Mar Grande, Javier Borondo
Embedding pairs trading in market networks: a network science approach to portfolio construction
DOI = 10.1057/s41599-025-05661-7
HTML = https://www.nature.com/articles/s41599-025-05661-7
PDF  = https://www.nature.com/articles/s41599-025-05661-7.pdf
PDF SHA256 = bcaacacc7de80abf95828d024ddd6672b43c82a61d74bd12539a8dea2e253d03
Table 2 = https://www.nature.com/articles/s41599-025-05661-7/tables/2
```

已核原文：

```text
Binance 472 crypto（排除稳定币），2021-01 至 2025-01，1h bars
28d / 672h rolling formation，weekly rebalance
OLS + ADF 构建 cointegration network
PMFG/TMFG 过滤网络
X+Y centrality/peripherality ranking
外围结构 pair 优于中心 pair；跨社区 bridge pairs 收益更低、风险更高
外围 PMFG same-community 的 Sortino 1.2444，对照 cointegration 0.4289
```

这与 Round 26-29 的“逐 pair p-value 后 exact matching”不同。网络拓扑在看到交易期收益前识别共同失效、
中心拥挤和跨群脆弱性，可能降低多个 Martin group 同时向坏方向加仓造成的 shared-account DD。Round 30
选择来源中 tail-risk/Sortino 更强且适合 29-node 稀疏图的 PMFG；不执行需要 tradable 4-clique seed、在小图上
容易退化为零的 sparse TMFG。

原文只把 `p<0.05` 当筛选而非协整证明。Round 30 因此额外冻结每周 406 个 canonical pair 的
`Benjamini-Hochberg q=0.05`，先控制周内多重检验，再让三个 selector 共用同一 edge population；这属于本项目
的 anti-overfit adaptation。论文使用的“cointegration coefficient”权重定义不足以无歧义复刻，本项目明确改用
对称 `strength=-ln(p_sym)`；两项 adaptation 都不继承论文收益。

论文的收益不能继承：它随机抽 20 pairs、做 20 次 Monte Carlo 平均，未计交易成本/流动性，也不是小本金
共享账户。Round 30 只采用 formation-only PMFG 外围结构 selector；订单、成本和收益全部由本项目的
Martin engine 产生。

### 5.2 低容量条件增强：local Hurst anti-persistence

来源：

```text
Anti-Persistent Values of the Hurst Exponent Anticipate Mean Reversion in Pairs Trading
DOI = 10.3390/math12182911
```

来源使用 crypto local `H<0.5` 识别更快均值回归。Round 26 曾把它写成条件 C2，但 Round 26 在 G1 前被错误
CUSUM 联合门清空，没有形成有效 scored replay。因此不是重复一个已闭合结果。

Round 30 只允许固定一个 `H<0.5` arm，作为 FO veto/SO freeze；source stop/zero/deadline 独立强制执行。
Hurst 不得产生独立订单或收益，不得搜索 H 阈值、lag 或 estimator neighbors。

### 5.3 已核验但不采用

2026-07-27 另以 Crossref 对 `cryptocurrency martingale pairs trading`、`cryptocurrency network pairs trading
cointegration`、`cryptocurrency Hurst pairs trading mean reversion` 各取前 40 条去重。新增高相关结果仍未提供一个
同时具备 Martin、多币共享账户、`<5000U`、完整成本和长期低 DD 的可继承策略。

| 来源 | 核验信息 | 决定 |
|---|---|---|
| `10.2139/ssrn.5935414` | 2026 posted-content，题名即为 depth-controlled spot Martin 吸收 42% DD；SSRN 正文受限 | 42% DD 已越三档 cap，且未验证多币/成本/小本金，不采用 |
| `10.3389/fams.2026.1749337` | dynamic Johansen + DNN/LSTM ensemble，5 币含 USDT；摘要没有可继承的成本后 Martin 规则 | DNN/LSTM 与旧 Johansen 路径重复且容量过高，不采用 |
| `10.1080/00036846.2025.2512152` | cryptocurrency pairs + NSGA-II | 属 GA 多目标调参，违反冻结低容量门，不采用 |
| `10.1007/s10614-025-11149-y` | cryptocurrency pairs 题名可核，但 Crossref 无摘要/规则 | 无法冻结可执行新机制，不从题名猜规则 |
| `10.2139/ssrn.7065258` | 6 个 Payment-Coin DRL 最高约 32.26%，不同 universe 差异大 | DRL/seed tuning 违反低容量可解释门；只登记 universe 风险 |
| `10.2139/ssrn.7052115` | 15m-4h 的 360 个 DRL runs，平均收益各频率均负 | 不追加 timeframe/DRL 搜索 |
| `10.1108/SEF-12-2020-0497` | dynamic crypto cointegration、OU half-life、真实 quote availability | Round 22 已规划/实现 KSS 路径，属于重复 |
| `10.1186/s40854-024-00702-7` | BTC-reference Copula crypto pairs | Round 25-29 已采用，不能重新包装 |
| `10.1016/j.najef.2020.101295` | multi-asset GARCH/tolerance statistical learning | 完整规则未取得且与 TAR/MTAR/tolerance 路径重叠 |
| `10.2139/ssrn.5895159` | Micro-Martingale/Integral TP | 历史已登记并禁止重复 |
| `10.48550/arxiv.2403.07998` | maximum-weight pair matching | Round 26-29 已实现 exact matching |

## 6. Round 30 决定

Round 30 分两段，均不可跳过：

```text
R30-A Round 29 authority recovery:
  若证据后续同步：验证 commit/tree/raw hash、24 terminal、双 validator 和 mutants 后导入
  若仍无证据：原样补跑 Round 29 的 16 C0 + 8 MPI，不重复计入新 trial

R30-B Network-embedded Martin:
  formation-only direct alt-alt cointegration graph
  bidirectional Bonferroni + within-anchor BH q=0.05
  classic EG / peripheral PMFG / peripheral PMFG same-community controls
  deterministic strong-tie filtering and disjoint multi-pair selection
  fixed z-spread Martin with one H<0.5 ablation
  same causal open/path engine, shared account, costs, budgets, stress and anti-overfit
```

全局 visible trial floor 冻结为：`1145 + 24 + 6 = 1175`；若 Round 29 自身 C0+MPI ensemble 实际适用并执行，
加 1；若 Round 30 cross-family ensemble 实际适用并执行，再加 1。因此本轮 DSR 的合法 `N` 是
`1175/1176/1177`，不能固定写死为 1176。

唯一任务书：

```text
docs/superpowers/plans/2026-07-27-glm-martingale-core-round30-network-periphery-authority-recovery-plan.md
```

本次只完成审计、外部检索和任务设计，不执行 Round 29/30 回测。
