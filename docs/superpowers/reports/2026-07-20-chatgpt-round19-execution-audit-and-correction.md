# ChatGPT Round 19 完整执行审计、独立复算与权威修正

审计日期：2026-07-20。

审计输入：

- `docs/superpowers/plans/2026-07-17-glm-martingale-core-round19-valid-residual-recovery-plan.md`
- `docs/superpowers/reports/2026-07-20-glm-round19-execution-handoff.md`
- `docs/superpowers/artifacts/glm-martingale-core-round19/`
- `scripts/glm_r19_*.py`
- `apps/backtest-engine/src/martingale/sync_cycle_engine.rs`

权威机器产物：

- `docs/superpowers/artifacts/glm-martingale-core-round19/round19-corrected-authority.json`
- `docs/superpowers/artifacts/glm-martingale-core-round19/audit/round19-independent-audit.json`
- `docs/superpowers/artifacts/glm-martingale-core-round19/audit/round19-corrected-failure-ledger.jsonl`

下一轮唯一任务书：

- `docs/superpowers/plans/2026-07-20-glm-martingale-core-round20-causal-basis-diversification-plan.md`

## 1. 权威结论

GLM 的 `VALID_SEARCH_NO_FRONTIER_PROGRESS` 不成立。Round 19 修正状态为：

```text
materially_incomplete_invalid_results
```

这不是“6612 次回放全部算错”。其中 M1R 的理想化成交路径可以确定性复现，但 Round 19 没有满足搜索
开始前的引擎、family、causality、registry 和 validator 合同，所以严格有效 search row 为 `0`。这些行
只能保留为 research diagnostic，不能证明已完整搜索，更不能证明 `<5000U` 可实盘。

三档仍为零命中：

| 档位 | ann | DD | 稳定性 | 权威结论 |
|---|---:|---:|---:|---|
| 保守 | >=50% | <=10% | >=4/5 正 | 未命中 |
| 平衡 | >=90% | <=20% | >=4/5 正 | 未命中 |
| 激进 | >=110% | <=30% | >=3/5 正 | 未命中 |

没有有效 finalist、exact minimum executable principal 或 production-ready candidate。

## 2. Critical：五个计划 family 只执行两个

Round 19 计划把 `M1R/M2F/P1/K1/V1` 定义为五个独立 family。实际：

```text
M1R implemented
M2F implemented-but-contract-invalid
P1  0 implementation / 0 replay
K1  0 implementation / 0 replay
V1  0 implementation / 0 replay
```

`blocked_implementation_scope` 是执行阻塞，不是“完整搜索后零命中”。因此 R4 起就不能继续宣告完整。

## 3. Critical：R2 fail-close 是伪通过

R2 evidence 自己承认把以下任务延期到 finalist/R11：

- next-SO reserve、费用和 maintenance buffer；
- `minQty/stepSize/tickSize/minNotional`；
- maintenance margin 和 liquidation；
- partial fill、reject、legging loss。

但计划把它们列为搜索前 hard gate。当前引擎仍输出/省略 `min_liquidation_buffer_pct=None`，也没有 filter、
partial-fill 或 legging state，因此不能证明 1000-4999U 可生存。

所谓 cooldown 没有永久冻结/cooldown state；独立复算中：

```text
M1R_F3_044 group_atomic_reject = [42060, 503220, 182599]
M1R_F3_028 group_atomic_reject = [101, 211973, 104559]
```

`no_so_row_is_marked_groups_with_so_zero` 的核心断言是恒真式
`g_so == 0 || g_so >= 1`，不能证明 no-SO 行被 runner 淘汰。

审计实际运行 `cargo test -p backtest-engine sync_cycle_engine --lib` 时还复现了 Round 19 声称已修的
determinism bug：process-global `CYCLE_SEQ` 在并发 replay 间互相 reset/递增，导致同输入 event hash 漂移。
本次已改为每次 replay 私有 counter，并把测试升级为 `8` 线程各 `20` 次并发 replay；修复后 `15/15`
通过。该修复只恢复并行回放确定性，不使缺少交易所模型的 6612 行自动有效。

## 4. Critical：M2F 不符合计划机器定义

实现仍然：

- basket notional 等权，没有 constrained factor-neutral solve；
- 持仓方向来自 train-end 静态 `leg_direction_signs`，不是 cycle-open residual rank；
- signal 是所有单腿 residual 的无符号均值，不是实际 signed-weight basket residual；
- BTC factor 不要求和所有腿同 timestamp fresh；
- PC1 返回 `None`。

等权 signed beta exposure 独立重算如下：

| fold | exposure | `abs<=0.10` |
|---|---:|:--:|
| F1 | 0.107893 | FAIL |
| F2 | -0.216878 | FAIL |
| F3 | 0.804682 | FAIL |
| F4 | 0.411812 | FAIL |

四个 fold 全部违反计划约束，M2F 的 4032/2580 局部行不能作为该 family 的有效穷尽证据。

## 5. High：G0 失败后仍搜索

权威 G0 JSON 明确写出：

```text
M1R all_bound=false: tp_net_bps_floor, group_gross_cap_pct inert
M2F all_bound=false: group_gross_cap_pct inert
```

状态机却只检查另一个 `all_implemented_families_conditionally_bound=true`。计划要求开放参数 inert 时立即
停止 family，因此 R6/R7 是 predecessor 失败后的诊断回放。

## 6. High：registry 和 validator 不能证明执行合同

独立统计：

| 项目 | 行数 |
|---|---:|
| central `exploration-registry.jsonl` | 0 |
| R6 local registry | 4032 |
| R7 local registry | 2580 |
| `running` 行 | 0 |
| canonical fingerprint | 0 |

6612 个 local terminal 行全部缺 raw command、exit code、engine/data/fit/cost hashes 和五类 trace hashes。
原 state JSON 自己显示 `actual_binary_replays=0`、`unique_terminal_fingerprints=0`，却把 R0-R12 全部判
complete。

状态机还有以下 permissive gate：

- five-family gate 只要求 `M1R/M2F`；
- G0 不要求 `all_bound=true`；
- G1/G2 只要求 summary `total_runs>0`；
- handoff 只要求文件存在；
- 不读取 local registries，也不验证 running-terminal/hash 合同。

## 7. High：G1/G2 选择链断裂且 inner train 泄漏

G1 权威 summary 的严格 survivors 是：

```text
M1R F4: M1R_F4_025@1000, M1R_F4_037@1000
其他 family/fold: 0
```

R7 的 `get_g1_top16()` 重新从 checkpoint 选参时省略 symbol/concentration hard gate，结果又向 G2 输入
M1R F2/F3/F4 各 16 个、M2F F1=6、F3/F4 各 16 个配置。G2 输入因此不是 G1 survivor artifact。

此外，每个 outer fold 的 fit 都在完整 train 末端生成，R7 却把同一个 fit 用于此前五个 train 子块。
独立检查四个 fold 均为 `0/5 causal`、`5/5 leaky`。这没有偷读 validation，但属于 inner-train
look-ahead，足以使短块 5/5 只能作线索。

## 8. 独立复算与可保留的近前沿

以下命令只读 F3 train，不读取 validation：

```text
python3 scripts/chatgpt_r19_independent_audit.py
```

两条完整 replay 的 ann、DD、trade、SO、symbols 与 GLM checkpoint 精确一致，说明局部输出可复现：

| config | budget | full train ann | DD | symbols | groups with SO | max symbol/group |
|---|---:|---:|---:|---:|---:|---:|
| M1R_F3_044 | 1000 | 41.4309% | 6.2723% | 6 | 3 | 56.07% / 56.46% |
| M1R_F3_028 | 1000 | 40.4103% | 6.1091% | 6 | 3 | 52.53% / 52.67% |

五个 train 子块诊断：

| config | median ann | worst DD | 正区间 | real SO | concentration pass |
|---|---:|---:|---:|---:|---:|
| M1R_F3_044@1000 | 48.1879% | 6.8108% | 5/5 | 5/5 | 3/5 |
| M1R_F3_028@1000 | 42.4371% | 6.1091% | 5/5 | 5/5 | 4/5 |
| M1R_F3_082@2000 | 45.8971% | 7.0665% | 5/5 | 5/5 | 3/5 |

它们全部是经过大量选择、inner-fit 泄漏、没有 validation、没有 liquidation/filter/legging 的诊断，不能
称 candidate。可保留的信息只有：**低 DD 的主要剩余缺口是 event-level group/symbol contribution
concentration，下一轮应先做 causal 分散调度，而不是再盲调普通 ladder。**

## 9. Round 19 阶段修正

| phase | 修正状态 |
|---|---|
| R0 | invalid_empty_central_registry |
| R1 | partial_index_only_not_connected_to_runners |
| R2 | materially_incomplete_failclose |
| R3 | partial_research_cycle_only |
| R4 | materially_incomplete_three_families_missing_m2_contract_invalid |
| R5 | failed_inert_parameters |
| R6 | research_diagnostic_only_blocked_predecessor |
| R7 | research_diagnostic_only_inconsistent_g1_input_and_inner_lookahead |
| R8-R11 | blocked_predecessor |
| R12 | superseded_by_chatgpt_audit |

## 10. 2026-07-20 外部扩大检索

外部收益声明不能代替本地 replay。检索得到的新机制与限制：

1. `10.3390/a19060442` 报告 Soft Martingale + Strategy-Embedded Labeling 在单一 EUR/USD、1:500 杠杆
   下年化 `442.6%`；但最大 equity DD 为 `79.97%`，balance DD 仅 `13.98%`，DDR=`5.72x`。论文自己
   声明是方法演示，不是普适收益。可转化内容是完整 Martingale path label、次线性逐层仓位和同时报告
   equity/balance DD，不可复制其收益宣传。
2. `10.3390/risks11050093` 提出 multiple-pairs 分散和基于 spread 统计特征的资金分配。Round 20 只采用
   causal train-frozen equal-risk/activity quota，不允许按 validation 收益配权。
3. `10.1111/mafi.70018` 说明 perpetual 通过 funding 把 futures 锚定 spot。本地数据库同时具备 2023 起
   spot/perp 1m、32 币 funding 和 6 币 premium，可首次原生测试多币 spot-perp basis Martingale。
4. `10.1007/s10100-021-00763-4`、`10.1016/j.ejor.2022.07.037` 分别提供 bounded-risk pairs 与自动强平/
   杠杆选择的风险建模依据；只作为 abort/liquidation gate，不产生独立 PnL。
5. `10.1016/j.ifacol.2017.08.167` 的 drawdown feedback 只作为共享账户 reserve/size 上限；前轮已测试过
   普通 DD scaling，不再把它包装成新收益 family。

此前公开案例（Pionex、Phemex、3Commas 等）仍没有一条同时披露多币、`<5000U`、2023-2026 分段、
低 DD 和 Binance live parity。网上确有高收益 Martingale，但已披露案例要么 DD 极高，要么参数/成交
证据不足；不能据此降低本任务 gate。

## 11. 修正后的方向

Round 20 不重复普通 multiplier/spacing/HTF/funding sleeve 搜索，集中执行：

- 对 `M1R_F3_028/044` 做 changed-engine、causal、event-balanced concentration repair；
- 新增真正 loss-after-add 的多币 spot-perp basis aggregate Martingale；
- 修正 M2F 动态 rank、constrained weights 和 actual basket residual；
- 强制补实现 P1/K1/V1，禁止再次写 scope block 后继续；
- 仅在父 Martingale 过 train 门后测试论文的 exact soft schedule + SEL；
- 先补完整 Binance filters/liquidation/partial-fill/legging 和中央 registry，再运行搜索。

目标保持不变，但没有证据可以诚实承诺 Round 20 必然命中。
