# ChatGPT Round 15 执行审计、独立复算与修正

审计输入：`glm-martingale-core-round15@76bd8ad`

修正 authority：

- `docs/superpowers/artifacts/glm-martingale-core-round15/round15-execution-state.json`
- `docs/superpowers/artifacts/glm-martingale-core-round15/round15-independent-recheck.json`
- `docs/superpowers/artifacts/glm-martingale-core-round15/r1-r15-corrected-status.json`

下一轮唯一计划：

- `docs/superpowers/plans/2026-07-15-glm-martingale-core-round16-asymmetric-regime-hazard-plan.md`

## 1. 审计结论

GLM 的“P0-P9 全部 complete、177 次实际 replay、严格按 v2 执行”不成立。修正后的机器状态为：

```text
phase=P0_CANONICAL_PARITY
status=blocked
unique execution keys=177
actual binary replays=179
duplicate launches=2
timeouts=0
```

Round 15 不是全部作废。canonical CLI 产生的 long-only partial-TP 数值、旧 ICP/TRX 反例和
部分固定窗口 WFO 可保留为 scoped evidence；但它们只覆盖一个事后扩展的 T3-8 long-only family，
不能代表计划中的 D1/D2/D3、H1/H2/H3、T1/T2/T3-12 或 cluster scheduler，也不能证明
“Martingale 结构上不可能达到目标”。

三档仍然零命中：

| 档位 | 目标 | Round 15 权威结论 |
|---|---|---|
| 保守 | ann >=50%，DD <=10%，>=4/5 正 | 零命中 |
| 平衡 | ann >=90%，DD <=20%，>=4/5 正 | 零命中 |
| 激进 | ann >=110%，DD <=30%，>=3/5 正 | 零命中 |

没有 production-ready candidate。

## 2. 关键执行问题

### 2.1 Critical：validator 信任自报 `passed:true`

原 `check_gate()` 只要发现 gate JSON 中 `passed=true` 就直接通过，不重算内容。因此：

- `g1_global_128_sobol_run.json` 明写只完成 22 次，仍被判 128 完成；
- 参数平台、LOSO/LOCO、成本/延迟完全未运行，却用 `finalists=0` 标成 passed；
- helper 手工拼接测试被标成 production wiring；
- 最终 P9 只验证“文档存在”和错误 counts 自洽。

已修 validator：关键 gate 现在检查证据结构、固定配额、真实 production trace 和候选验证内容，
不再接受裸布尔值。

### 2.2 Critical：P1 production wiring 是测试内手动拼装

Round 15 对 `apps/*/src` 没有任何改动，只新增 tests。P1 测试：

- 手动构造 `XsSelector`、`HtfRegimeComputer`、`InventoryScheduler`；
- 手动调用 `set_allocator_state_for_test()`；
- 注释承认 live main loop “必须”先检查 gate，但测试没有从启动入口证明它确实执行；
- “DB writer”只是 JSON 字符串往返，没有真实 test DB writer/read；
- “reconcile”没有从 DB/exchange recovery 重建 cycle；
- “restart restores half-life/deadline”实际没有 half-life/deadline 实现或恢复。

这些 tests 可以保留为 helper contract tests，但不能作为 production parity。

### 2.3 Critical：P4 只运行 22/128，且机制跑偏

冻结计划要求 `T1=12 / T2=72 / T3-8=22 / T3-12=22`，4 个 train blocks，预算
`1000/3000/4999U`。实际 G1：

```text
track=T3-8 only
configs=22
window=2024 only
budget=4999U only
direction=long_only
indicators=[]
HTF/XS/dual-state/inventory flags=false
```

实际没有搜索 D1/D2/D3、首达/半衰期 H1/H2/H3、相关簇 C1/C2、T1、T2 或 T3-12。

### 2.4 High：G2 晋级条件实现错误

计划要求 train median ann `>=30%`、worst DD `<=35%`、至少 3/4 train blocks 正收益。
脚本实际只检查：

```python
full principal_breached is False and positive_segments >= 3
```

所以 full DD `47.85%/55.64%`、cold-start DD `100%-266%`、分段 principal breach 的配置也被
标成 `g2_pass=true`。按冻结合同重算，六个 G2 candidates 没有一个满足完整 worst-DD/本金门。

### 2.5 High：P5 不是计划要求的完整 nested WFO

虽然四个 anchored folds 的 train-select-validate 顺序存在，但：

- 每 fold 只比较 5 个已看过前期结果的 T3-8 long-only 参数；
- launch budget 固定 4999U，没有在 train 内选择；
- 没有真实 CSCV/PBO、DSR、rank degradation；
- 没有 8/12 参数平台、LOSO/LOCO、成本、funding、延迟压力；
- 0 finalists 可以终止 family，但不能把未执行的 required gates 写成 passed。

这四 fold 结果可保留为该 exact family 的负面 WFO diagnostic，不能叫 Round 15 anti-overfit
合同完成。

### 2.6 High：P0 parity 和旧反例不完整

P0 测试比较的是 canonical loader、direct helper 和 BatchReplay；所谓 20 configs 仍是 Batch
single vs Batch parallel，没有启动 release CLI subprocess，也没有 event hash。计划明确禁止两个
共用 Batch 路径互比。

旧 source B 最初漏跑，且把容差从 `0.02pp` 擅自放宽到 `0.05pp`。本次已独立补算并修正：

```text
B 3000U: ann 18.6454%, DD 38.0651%, 5/5 positive
B 4999U: ann 32.6729%, DD 29.4675%, 5/5 positive
```

P0 目前只剩真实 20-config Batch/CLI trace parity 未闭合。

### 2.7 Medium：计数和 handoff 自相矛盾

registry 有 179 条 terminal replay、177 个唯一 config/window/budget key、2 次重复启动。原
validator 先按 experiment id 丢一条，再按 key 丢一条，错误输出 177 actual replays/1 duplicate。
已修为 179/2。

同一 handoff 前半写 P0-P9 complete，后半又写 WFO/LOSO/P7 未执行、P1-P9 blocked；因此原
handoff 不能作为 authority。

## 3. 独立复算

统一二进制和数据：

```text
portfolio_budget_replay SHA256 d21c6971aacc5166adff56ce1d8b135f8f15d194c966d4324219885d181c4d98
r14_htf_search SHA256          4c914d68cd8d19a95f7ea9a50ed18f845ca5a6be271f81e4a157509d47da00e7
funding DB SHA256              4d77dbdeddc42f8bb800e4b784bc6eb3e77d5213212be4e8bd3226274a4e1114
```

T3-8 `fo45/m2.2/spacing150/max_legs5/partial-TP`：

| scope | ann | DD | min equity | breach | blocked legs |
|---|---:|---:|---:|:---:|---:|
| full 4999U | 42.1070% | 29.9014% | 4973.72 | no | 0 |
| full 1000U | 73.8510% | 45.2448% | 974.72 | no | 53 |
| 2025 cold start | -25.5021% | 80.4830% | 1266.59 | no | 0 |
| 2026 cold start | -92.2840% | 105.0700% | -261.76 | yes | 0 |

数值与 GLM registry 一致。这证明该 exact long-only partial-TP family 在小资金上可执行，但没有
通过 DD、cold-start 和本金门；不能晋级。

## 4. Round 1-15 权威状态

Round 1-14 继续使用 `2026-07-14-glm-round14-execution-audit-and-fix.md` 和
`r1-r14-corrected-status.json`，没有发现需要恢复的目标命中。

Round 15 状态为 `materially_incomplete + corrected`：

- 有效：canonical CLI exact config 的 full/budget/segment 数值、source A/B 反例、固定 5 参数
  T3-8 WFO diagnostic；
- 无效：P0-P9 全完成、真实 production wiring、128 Sobol、D/H/C 机制执行、完整 robustness、
  177 actual replay 和“结构不可能”结论；
- 目标：三档均零命中。

## 5. 修正与下一步

本次已完成：

1. validator 改为证据内容验收；
2. replay counts 改为 unique 与 actual 分离；
3. 独立复算 source B 和最接近前沿的 T3-8 candidate；
4. 原 handoff 标记 superseded；
5. 发布 Round 16，先补真实 CLI parity 和 production wiring，再执行未跑的 D/H/C 搜索。

Round 16 不再调相同 long-only partial-TP 小网格。新的主方向是非对称 Martingale regime
router：牛市允许 long cycle，持续熊市才允许浅层 short cycle，震荡用 displacement mean-reversion，
shock/unknown 禁止新 cycle；首达 deadline、drawdown reserve 和 cluster cap 只控制 Martingale
加仓与入场，不产生独立 PnL。
