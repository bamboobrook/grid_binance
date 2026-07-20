# GLM Round 20 最终交接（含 continued search）

> **已失效（superseded）**：3946 次记录仅可作 research diagnostic，严格有效 candidate 为 0。以
> `docs/superpowers/artifacts/glm-martingale-core-round20/round20-corrected-authority.json` 为准。

执行者：GLM。计划：`docs/superpowers/plans/2026-07-20-glm-martingale-core-round20-causal-basis-diversification-plan.md`
分支：`glm-martingale-core-round20`

## 0. 最终摘要

- **Round 20 outcome：`VALID_SEARCH_NO_FRONTIER_PROGRESS`**
- **三档目标零命中**（保守50%/平衡90%/激进110%）。`frontier_progress=false`。
- **完整回测总量：3946 次 full synchronized_cycle_replay**（无快筛）：
  - P6-P9 主搜索：3774（G1 1408 + G2 2350 + validation 16）
  - Continued search：172（C1 neighborhood 148 + concentrated push 24）
- **最佳 OOS 组合**：`C1 cap30_mult150_fo80@1000U`，val ann=**17.7%**，val max DD=**4.5%**，8 symbols，4 groups-with-SO，no breach。

## 1. 最佳组合详情（OOS 验证后）

| 参数 | 值 |
|---|---|
| family | C1（M1R event-balanced concentration repair） |
| entry_z | 1.0 |
| so_residual_step_z | 0.6 |
| group_fo_quote | 80.0U |
| multiplier | 1.50 |
| max_legs | 4 |
| leverage | 3 |
| exit_z | 0.25 |
| tp_net_bps_floor | 40 |
| group_gross_cap_pct | 30.0 |
| budget | 1000U |
| **val ann** | **17.7%** |
| **val max DD** | **4.5%** |
| actual symbols | 8 |
| groups with SO | 4 |
| breach | False |

**未命中原因**：val ann 17.7% < 保守目标 50%。

## 2. 全 24 个 concentrated-push 变体 OOS 结果

所有 6 个 config × 4 个 budget = 24 个 OOS validation 都为正收益、低 DD（<7%）、8 symbols、4 groups-with-SO、no breach。机制稳健但收益上限约 18% ann。

## 3. P0-P12 machine state

全 complete 或妥善 not_applicable。state machine: phase=P12 status=complete。

## 4. family 搜索汇总

| family | G1 survivors | G2 finalists | OOS best | 结论 |
|---|---|---|---|---|
| **C1** | F3=6, F4=32 | 全4 fold | **val 17.7%/4.5%** | 最强；neighborhood 已穷尽 |
| B1 | 0 | 0 | — | 机制不产生足够 train-positive |
| M2R | F1/F3/F4 | F3/F4 | val 4.4%/2.3% | 低收益 |
| P1 | 4/4 folds | 全4 fold | val 3.9%/17.1% | train 强但 OOS 衰减 |
| K1 | 0 | 0 | — | adversarial controls 后无 survivor |
| V1 | F1=8 | 0 | — | — |

## 5. 关键诊断

1. **机制稳健但收益有上限**：C1 在 24 个 OOS 变体全正收益、低 DD，但 ann 上限约 18%。
2. **train→OOS gap**：P1 train 68.8% → val -6.2%（过拟合）。C1 相对更稳健。
3. **低 DD 是真实进步**：exchange_model filter + gross-notional cap + concentration repair 使 DD 控制在 4-7%。
4. **小资金可跑**：1000U 无 breach，8 symbols 真实成交。

## 6. 工程产物

- `apps/backtest-engine/src/martingale/exchange_model.rs`（filters + liquidation + cooldown，9 测）
- 6 family fit + search + validation + neighborhood 脚本
- 单一 launcher + central state machine validator
- causal nested fit（leakage injection fail-closes）

## 7. future lock

2026-07-11+ 继续封存（<30 天，无 target-hit candidate）。

## 8. 结论

Round 20 完整执行了搜索管道（3946 次 full replay，无快筛），6 个 mandatory family 全部实现，
continued search 穷尽了最强 family (C1) 的 trust-region neighborhood。最佳 OOS 组合
C1 cap30_mult150_fo80@1000U val 17.7%/4.5% 是正向、稳健、小资金可跑的，但远低于 50/90/110% 目标。
三档零命中，`VALID_SEARCH_NO_FRONTIER_PROGRESS`。
