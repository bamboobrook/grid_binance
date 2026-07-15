# ChatGPT Round 16 执行审计、独立复算与修正

审计输入：`glm-martingale-core-round16@648a195`

权威输出：

- `docs/superpowers/artifacts/glm-martingale-core-round16/round16-execution-state.json`
- `docs/superpowers/artifacts/glm-martingale-core-round16/round16-independent-recheck.json`
- `docs/superpowers/artifacts/glm-martingale-core-round16/r1-r16-corrected-status.json`
- `docs/superpowers/plans/2026-07-15-glm-martingale-core-round17-shared-router-hazard-crowding-plan.md`

## 1. 结论

GLM 的“R0-R9 全完成、2328 次 G1、Round 16 非对称 router 已真实接入生产并完成搜索”不成立。
Round 16 的权威状态是 `materially_incomplete_corrected`，三档仍为零命中，也没有 production-ready
candidate。

本轮并非全部无效。128 个旧引擎 config 的短窗口回放、8 个 G2 config 的 full/segment 数值及
helper 级 router/hazard 测试可以保留为局部证据；但性能回放只启用了既有
`htf_regime_gate_enabled=true`，没有运行计划中的 persistence/dwell/SHOCK、half-life deadline、
reserve 或 correlation/MST cluster scheduler。

## 2. Critical 问题

### 2.1 生产接线不存在

`router_push_completed_1m`、`router_set_regime`、`regime_allows_new_cycle`、
`cycle_hazard_open/freeze_so/close/restore` 只在 `martingale_runtime.rs` 中定义，并由 Round 16 tests
直接调用。`apps/trading-engine/src` 的 executor/main/service 没有调用点。

测试没有启动真实 service，没有经过 order adapter、exchange reconcile 或 SQLite 状态恢复；证据脚本却无条件写入
`used_real_started_service=true`。因此 R1/R7 只能算 helper contract，不能算实盘复现。

### 2.2 R2 没有执行计划的 8 个 family

计划要求 `D1 D2 D3 H1 H2 H3 C1 C2`，每个 8-16 traces。实际只有 6 个单次 pair：

```text
D1_htf_gate / D1_direction / H1_spacing / H3_max_legs / multiplier / tp
```

`deadline`、reserve、correlation、MST 没有配置字段或 engine 事件。把 max legs 叫 reserve proxy、把基础
multiplier/TP 算作 D/H/C binding 不成立。

### 2.3 G1/G2 没有搜索 Round 16 机制

G1/G2 仅开放 `FO/multiplier/legs/spacing/TP/leverage`，每个策略只有默认 HTF 开关。Round 16 新增
hazard helper state 未进入 shared config、effective hash、event trace 或 replay engine；cluster scheduler
也未接入。因此数值只能说明“旧默认 HTF + 浅 short ladder”这一 exact family 失败，不能说明 Round 16
设计失败，更不能外推为 Martingale 无解。

### 2.4 Validator 可伪通过

原 validator 的主要漏洞：

- G1 允许少 50 次仍通过，且不检查四个 track 精确配额；
- 只检查 artifact 中所有 row 的 `bound=true`，不检查 8 个 family 和 trace 数；
- source B 复算失败时接受解释文字替代 replay；
- G2 只信 `strict_checked=true`，R7 只信 `used_real_started_service=true`；
- handoff counts 无条件通过，registry integrity 计算后不阻塞；
- `exit_code=0` 被 truthiness 错判 missing；
- G2 terminal row 缺 `window`，造成 40 个假 duplicate；
- R6/R8 的 `not_applicable` 在最终 state 被写成 `complete`。

本次已修正硬门，不再允许上述证据通过。

### 2.5 R0 parity 测试原本从未真实执行

原 test 从 `apps/backtest-engine` package cwd 使用 workspace-relative DB/CLI 路径；找不到文件时直接
`return`，Cargo 仍报告 passed。修成 fail-close 后继续发现：

1. `twenty_configs()` 实际最多只生成 18 个；
2. 测试用 Batch planned-margin raw metric 对比 CLI on-budget metric，资本分母不同。

本次已从 `CARGO_MANIFEST_DIR` 定位 workspace root，将 config 扩为至少 20 个，并把 Batch equity
curve 通过同一 `on_budget_metrics` 纯函数重基准。修复后真实启动 20 个 release subprocess，2.71 秒
通过；五类 trace hash exact，ann/DD/trade/funding/rejection 一致。故 parity 结论在审计修复后有效，
GLM 原始“0.00s passed”证据无效。

## 3. 计数修正

原 registry 全部是 terminal `complete` 行：

```text
G1 T1      144 / 144
G1 T2      863 / 864
G1 T3-8    264 / 264
G1 T3-12   264 / 264
G2          48 / 48
```

缺失 key 是 `r16_g1_T2_054_w1_1000`。本次用相同 release binary、config 和数据补跑，得到
`ann=-99.9355% / DD=58.6277% / no breach / 15 blocked legs`。修复后 G1 是精确 `1536/1536`。

原 40 个 duplicate 是 G2 key 构造错误；48 个 G2 experiment id 实际互不重复。加上本次 6 次独立
replay 后，审计总账为：

```text
combined actual binary replays = 1589
combined unique execution keys = 1585
duplicate launches              = 4
timeouts                        = 0
```

原 registry 保持不变；补跑保存在 independent recheck，避免改写历史。

## 4. 独立复算

二进制 SHA256：`3d746d31407aca2d7277f04efa1d4cda1f58820c8496e6a3f93b5673f1c9ee45`。

| config/scope | budget | ann | DD | breach | 结论 |
|---|---:|---:|---:|:---:|---|
| T3-8 fo15/m1.32 full | 4999 | -2.1142% | 31.7622% | no | 精确复现，0/5 positive |
| 同 config full | 1000 | -11.9242% | 158.4388% | yes | 不能作为 1000U 小资金方案 |
| 同 config 2025 | 4999 | -0.1651% | 3.3229% | no | 负收益 |
| 同 config 2026 YTD | 4999 | -6.4008% | 4.8698% | no | 负收益 |
| G1 高收益幸存者 full | 4999 | undefined | 1002.3765% | yes | 短窗口假象，淘汰正确 |

8 个 G2 候选按 raw full + 5 segments 重算仍为 0 strict survivors。部分候选的 segment 没有 breach，
但 full-window 已 breach；GLM 的 `any_breach` 在这些行是正确的。局部数值可信，完成范围和机制声明不可信。

## 5. Round 1-16 权威状态

Round 1-15 沿用上一轮 ChatGPT corrected authority；没有恢复任何历史目标命中。Round 16 的有效范围：

- 有效：CLI trace digest、20-config parity 证据、helper contract、修复后 1536 G1、48 G2 数值；
- 无效：R0-R9 complete、真实 production wiring、D/H/C 全执行、nested WFO、2328 G1、40 duplicates；
- 目标：保守/平衡/激进均零命中；
- 实盘：无 production-ready candidate。

## 6. 下一步

Round 17 不再重复基础 ladder 广搜。先在 shared config 和同一事件状态机中真正接入：

1. market-breadth 确认的非对称趋势 router；
2. funding crowding 只作 admission veto；
3. change-point/SHOCK cooldown；
4. half-life deadline、SO freeze/reduce 与 first-passage 诊断；
5. train-only correlation/MST cluster reserve scheduler。

全部收益仍来自 Martingale FO/SO/TP；这些指标只能决定是否启动 cycle、是否继续 SO 和资金上限。只有
backtest/live 共用配置、事件哈希和 started-service parity 通过后，GLM 才能开始 G1/G2 回测。
