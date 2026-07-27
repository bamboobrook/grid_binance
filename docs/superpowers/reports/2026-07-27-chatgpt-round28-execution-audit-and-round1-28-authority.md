# Round 28 独立审计、Round 1-28 权威状态与 Round 29 方向

日期：2026-07-27
审计对象：`glm-martingale-core-round28@b158f67f`
原始执行 source commit：`d516d40dafbac63d0c9d607224cae8de9721a2ce`
原始 raw root：`artifacts-local/round28/d516d40dafbac63d0c9d607224cae8de9721a2ce`

## 1. 权威结论

Round 28 原报告的两个 `P-B`、全部 G2/G3 收益与回撤，以及
`VALID_FRONTIER_PROGRESS_NO_TARGET` 不能继续作为策略证据。修正状态为：

```text
MATERIALLY_INCOMPLETE_INVALID_G2_G3_AND_FRONTIER_CLOSURE
strict_valid_candidates = 0
strict_valid_5_of_5_candidates = 0
target_hit = false
round28_p_b_candidates = 0
round28_family_closed = false
```

原因不是收益未达标，而是 scored replay 同时存在三项 P0 语义错误：

1. 当前分钟先读取 `high/low/close`，再按同一分钟 `open` 执行 TP、SO 和 FO，形成前视；
2. C0 SO 的不利方向符号反了，恢复方向会触发加仓，真正继续亏损的方向反而不会；
3. 新 FO/SO 成交后没有再执行该分钟的 `high/low` 风险路径，首分钟 DD 和强平可能漏记。

因此不能诚实地说“前 28 轮结果都有效”。正确处理是：保留可复核的局部证据，隔离受污染的收益结论，
并在修复后重跑。缺失或错误的历史订单路径不能靠改报告事后变成有效回测。

在最新共同合同下，Round 1-28 仍没有一条同时满足以下条件的严格候选：

```text
Martin 为唯一收益引擎
本金严格 <5000U
真实多币共享账户
保守 50%/10%、平衡 90%/20%、激进 100%/30%
抗过拟合与实盘订单语义通过
```

## 2. 审计范围与复核

已核对：

```text
Round 28 唯一任务书、handoff、authority、D0/R0/G1/G2/G3 gates
16 个 replay summaries、240 个 tier/budget/cold-start rows、28 个 stress rows
signal/model/trace manifests、双 validator、validator mutants、failure ledger
r28_execute.rs、r28_validate.rs、r28.rs、r24-engine SharedAccount
Round 1-27 最新 ChatGPT corrected authority/audit reports
```

历史结论按以下权威链递进继承，不以较早的 GLM 自报 handoff 覆盖后续修正：

```text
Round 1-23: 2026-07-23-chatgpt-round23-execution-audit-correction-and-feasibility.md
Round 24:   2026-07-23-chatgpt-round24-execution-audit-frontier-and-round25-direction.md
Round 25:   2026-07-23-chatgpt-round25-independent-audit-and-corrective-direction.md
Round 25R:  2026-07-23-chatgpt-round25r-post-correction-audit-and-round26-direction.md
Round 26:   2026-07-26-chatgpt-round26-execution-audit-and-round27-direction.md
Round 27:   2026-07-26-chatgpt-round27-execution-audit-and-round28-direction.md
Round 28:   本报告
```

原始 raw root 约 `60G`，source tree 为
`998fa2b9745ee9c0d79118daaf92c3d21e5b6f8e`。本次没有删除或改写原始 evidence。

工程测试重新执行：

```text
cargo test -p r24-engine -p r24-research
r24-engine: 23/23 PASS
r24-research lib: 62/62 PASS
r28_execute: 1/1 PASS
r28_validate: 1/1 PASS
doc tests: PASS
```

测试通过只说明现有断言通过；测试集没有覆盖下述错误的分钟事件顺序、family-specific SO 距离和
entry-minute adverse path，不能抵消审计结论。

## 3. Round 28 阻断问题

### 3.1 当前分钟前视后按 open 成交

`crates/r24-research/src/bin/r28_execute.rs:2348-2361` 在每个 `cursor` 先读取当前分钟
`low/high/close` 并调用 `mark_adverse_bar()`。该函数在
`crates/r24-engine/src/lib.rs:694-732` 先用 adverse extreme 更新 DD/强平，再把持仓 mark 设置为当前分钟
`close`。

随后执行器却：

```text
r28_execute.rs:2539      使用已知 current close 的 group_net 判断 TP
r28_execute.rs:2567-2572 使用已知 current close 的 group_net 判断 SO
r28_execute.rs:2592-2610 处理当前 cursor 的 FO
r28_execute.rs:3647-3659 最终仍按 current open 平仓
```

这等价于先看完分钟 `t` 的高低收，再按分钟 `t` 开盘成交。正确顺序必须拆成 open phase 和 bar path phase：

```text
minute t open phase:
  只允许读取 t.open 与 signal_completed_at <= t-1
  mark existing positions at t.open
  funding -> pre-existing delayed fills -> exits -> SO -> FO

minute t path phase:
  对包括本分钟新仓在内的全部持仓应用 t.high/t.low
  maintenance/liquidation
  mark t.close
  写 risk/account trace
```

任何 open-phase 决策读取 `t.high/t.low/t.close` 都必须由测试和独立 validator fail-close。

### 3.2 C0 SO adverse 符号反向

当前实现：

```rust
let adverse = group.direction.sign() as f64 * (group.last_value - row.value);
```

C0 的 `value` 是：

```text
left_residual / sigma_left - right_residual / sigma_right
```

且 `ShortLeftLongRight.sign() = -1`。按 Table 4 方向：

```text
short-left/long-right: value 继续下降才是不利
long-left/short-right: value 继续上升才是不利
```

所以 C0 应使用：

```text
short-left/long-right: last_add_value - current_value
long-left/short-right: current_value - last_add_value
```

当前代码恰好相反。T1 z-spread 的不利方向又与 C0 不同，不能再共用一个
`direction.sign() * delta`。Round 29 必须由 family 明确返回 adverse distance。

这也解释了 Round 28 大多数 policy 的 SO 为 `0-4`：它们没有真正检验预注册的 Martin recovery path。

### 3.3 入场分钟风险遗漏

`mark_adverse_bar()` 在 FO/SO 之前运行；新订单在 `r28_execute.rs:2592` 之后才进入账户。执行器随后只把
risk row 用当前 high/low 做一次旁路权益计算，没有让 engine 对新仓执行 maintenance/liquidation 和
`max_equity_drawdown_pct` 更新。

后果：

- 新 FO/SO 的首分钟 adverse DD 可能漏记；
- 首分钟 maintenance breach/liquidation 可能漏掉；
- engine DD、risk trace DD 和 validator 重算可能描述不同路径。

所有 G2/G3 的 DD、强平和 principal survival 因此必须重跑。

## 4. Validator、防过拟合与 G3 缺口

### 4.1 Account validator 仍不完整

`r28_validate.rs` 能读取 fills、wallet 和 risk rows，但没有从 raw DB 与 signal manifests 独立重建并逐项核验：

```text
FO/SO/TP/freeze/abort counts
symbol/pair/group/block concentration
P-A/P-B/P-C 与三档 target
final active-group/pending lifecycle
真实 overlap scheduler 与 tie-break
open-before-high/low/close 的字段访问顺序
```

risk validator 又使用 risk row 自带的 positions/path wallet 复算，没有从 account trace 和 raw DB 独立重建
分钟路径。mutants 主要改第一条 trace；`delete_overlapping_intent` 实际删除第一条 intent，只证明 count mismatch，
没有证明重叠 scheduler 被独立验证。

### 4.2 Anti-overfit 是 proxy，且原值本身未过门

Round 28 报告值：

```text
CSCV PBO                    = 0.6233766  > 0.50 FAIL
deflated Sharpe probability = 0.6013139 < 0.95 FAIL
SPA-style p-value            = 0.178     > 0.10 FAIL
bootstrap ann lower 95%      = -0.6216%  < 0 FAIL
```

实现还存在以下问题：

- DSR 不是标准 Deflated Sharpe 公式；
- PBO/SPA/bootstrap 只使用 12 个季度点，统计功效很弱；
- leave-one-symbol/pair 只是 `total PnL - contribution`，没有重新运行共享账户；
- hard gate 只要求至少一个 policy 通过 leave-one，而不是每个候选分别通过。

所以即使先忽略 P0 回放错误，两个原 `P-B` 也没有通过 anti-overfit hard gates。

### 4.3 G3 其他缺项

- `worst-combined` 没有设置 `reject_second_leg=true`；
- leg-delay 同时延迟两腿，并把两腿绝对价格变化都记作损失，不是真实单腿延迟；
- `theoretical_minimum_executable_principal` 取所有 signal 的最小 filter floor，不是完整执行的本金边界；
- `tier-results/` 只有一个汇总占位文件，没有每个 terminal 的独立 compact artifact。

## 5. Round 28 结果修正

原报告两个最好 `P-B`：

| 原 policy | 原 ann | 原 DD | 正 blocks | 修正结论 |
|---|---:|---:|---:|---|
| C0-5m-Equal-A0.10-SO0.50 | 0.6795% | 1.2212% | 9/12 | invalid implementation output |
| C0-5m-Equal-A0.10-SO0.75 | 0.5980% | 1.2240% | 9/12 | invalid implementation output |

这些数字可以描述错误程序的 trace，但不能用于排名、实盘、目标判断或关闭 C0 family。

仍可继承为 scoped evidence：

```text
D0 market/funding/filter/maintenance 数据清单与完整 SHA256
12 calendar block mapping
formation-only C0/TAR/MTAR model snapshots 与 model hashes
Table 4 signal direction和 causal intent census
TAR/MTAR 152/152 snapshot、activation denominator 与明确失败门
SharedAccount reserve/filter/并发基础设施及已有单元测试
错误 trace 上 ann/DD 的机械可复算性
```

必须撤销并重跑：

```text
两个 P-B 与 VALID_FRONTIER_PROGRESS
全部 Round 28 G2/G3 收益、DD、budget、cold-start、stress 排名
P-A/P-B/P-C、minimum principal 与 target claims
16 个 C0 valid-failure 标签和 C0 family closure
```

## 6. Round 1-28 最新权威分类

下表的“可保留”只表示精确 scope 内的研究证据，不表示目标候选或实盘可用。

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
| 26 | materially incomplete invalid family closure | D0、snapshot 与 gate decomposition | Copula Martin 已回测；实际为 0 replay |
| 27 | materially incomplete invalid G2/closure | formation snapshots、错误 trace 机械复算 | 8 个负收益可关闭 C0 |
| 28 | materially incomplete invalid G2/G3/closure | 本报告第 5 节 scoped evidence | 两个 P-B、G2/G3 与 frontier 有效 |

Round 9/11 曾出现 `5/5` curve slices，Round 19 曾出现 `5/5` inner-train，Round 23 曾出现单 BTC
`5/5` cold starts；它们都不满足当前共享账户、多币、因果和抗过拟合共同合同。严格有效 `5/5` 仍为 0。

## 7. 外部检索与新方向

本次扩大检索后，最值得执行且未在仓库出现的是 cumulative Copula Mispricing Index，而不是再调普通
multiplier/spacing/TP。

### 7.1 采用：累计 Mispricing Index

来源：

```text
Wenjun Xie et al., Pairs Trading with Copulas
SSRN: 10.2139/ssrn.2383185
Journal: 10.3905/jot.2016.11.3.041
EFMA full paper:
http://www.efmaefm.org/0EFMAMEETINGS/EFMA%20ANNUAL%20MEETINGS/2014-Rome/papers/EFMA2014_0222_fullpaper.pdf
PDF SHA256: c2b3cffbb30de9fcb81f1643a1690cc871e3c1c85ba9165086ee84585b8fb7e4
```

已核到的原文规则：

```text
PDF p.11, Eq. 2.2: MI_X|Y=P(R_X<r_X | R_Y=r_Y), MI_Y|X 对称
PDF p.12, Eq. 2.3: MI_X|Y=dC(u,v)/dv, MI_Y|X=dC(u,v)/du
PDF p.14:
  FlagX += MI_X|Y - 0.5
  FlagY += MI_Y|X - 0.5
  D=0.6, S=2
  driver 回零退出，达到 +/-S 止损，平仓后双 flag 清零
```

2026-07-27 重新通过 DOI content negotiation 核对 SSRN/journal 标题、作者与出版信息，并重新下载 EFMA
PDF；实际 SHA256 与上列冻结值一致。该核验只证明来源和规则可复现，不证明其收益可迁移到 crypto。

它与 Round 28 的“同一 bar 两个 h-value 同时越过极值阈值”不同：累计 minor dislocation，可能提高独立
pair 的持续信号和真实 SO 数量。Round 29 只把 MPI 用于 Martin 的 FO/SO/exit/abort；不得产生独立仓位或收益。

### 7.2 固定低容量 ablation：Laplace margins

来源：`10.3390/math10050783`，PDF SHA256
`453d02279818342230da3d675e1fe064bbc58017e0b36840c21ad1acdc235a6e`。

论文只支持“肥尾数据下 Laplace marginal 比 normal 更合适”的建模假设。其交易演示忽略成本、只有单 pair、
一天持有，不能继承收益。Round 29 仅允许一个预注册 Laplace-margin MPI ablation，与 empirical-margin control
共同计入 trial denominator；不得据结果继续追加 skew-Laplace/GARCH neighbors。

### 7.3 已核验但本轮不采用

| 来源 | 信息 | 决定 |
|---|---|---|
| `10.3390/jrfm18090506` | 2005-2024 成本后 MPI Copula 仅 23bp/月，DM 81bp/月 | 证明稳定性可能改善，不支持高收益先验 |
| `10.4102/jef.v6i1.278` | conditional-copula 多数 gross profit 被成本消耗 | 继续严格成本门，不新增同类 instantaneous arm |
| `10.1002/asmb.70049` | 声称新的 mispricing indicator | 正文规则未取得，不猜公式、不执行 |
| `10.1016/j.amc.2024.128635` | multivariate mixture-copula | 正文与可执行规则不足，容量过高，登记待研究 |
| `10.2139/ssrn.6188418` | 500+ crypto、clustering/stability、Hummingbot live | 只有元数据/摘要，无 PDF hash 和规则，本轮不实现 |
| `10.3934/QFE.2026016` | market overlay 可增收益但 DD 极高 | directional overlay 违反 Martin-only，排除 |
| `10.48550/arxiv.2403.07998` | maximum-weight matching 改善 pair selection | R26-R28 已实现 exact disjoint matching，属于重复 |

外部资料仍没有证明 `<5000U`、多币共享账户、完整成本、长期抗过拟合和三档收益/DD 可以同时实现。
目标继续作为硬验收线，但不能预先承诺 Round 29 命中。

## 8. Round 29 决定

Round 29 先恢复可信执行器，再测试累计 MPI：

```text
R29-R0:
  open-phase causal chronology
  family-specific adverse distance
  entry-minute high/low risk
  independent raw-DB validator
  exact rerun Round 28 C0 16-policy population

R29-MPI:
  cumulative h-value flags
  fixed D=0.6 / S=2
  deterministic Martin SO levels inside [D,S]
  empirical/Laplace margins only
  same shared-account, budgets, cold starts, stress and anti-overfit gates
```

唯一任务书：

```text
docs/superpowers/plans/2026-07-27-glm-martingale-core-round29-causal-open-phase-cumulative-mispricing-plan.md
```

本次只完成审计和任务设计，不执行 Round 29。
