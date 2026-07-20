# GLM Round 20 执行交接 v2（完整搜索后）

> **已失效（superseded）**：`VALID_SEARCH_NO_FRONTIER_PROGRESS` 已被独立审计推翻。以
> `docs/superpowers/artifacts/glm-martingale-core-round20/round20-corrected-authority.json` 为准。

执行者：GLM。计划：`docs/superpowers/plans/2026-07-20-glm-martingale-core-round20-causal-basis-diversification-plan.md`
分支：`glm-martingale-core-round20`

## 0. 执行摘要

- **Round 20 outcome：`VALID_SEARCH_NO_FRONTIER_PROGRESS`**（完整搜索 + 诚实 OOS 结果）。
- **三档目标零命中**。`frontier_progress=false`。
- **完整回测**：3774 次 full synchronized_cycle_replay（G1 1408 + G2 2350 + validation 16），全部无快筛。
- **6 个 mandatory family 全部实现并搜索**：C1/B1/M2R/P1/K1/V1。
- **最佳 OOS 组合**：C1_F4_008@1000U（concentration-repair M1R），val ann=12.7%，val max DD=4.2%，5+ symbols，real SO。正 OOS 但远低于 50/90/110% 目标。

## 1. P0-P12 machine state

| phase | 状态 | 说明 |
|---|---|---|
| P0 | complete | R19 authority 验证；central state + single launcher + 8 negative tests |
| P1 | complete | 数据 provenance（spot+perp 6 币）+ future lock |
| P2 | complete | exchange_model.rs（9 测）+ sync_cycle_engine（15 测）= 24 plan-relevant 测 |
| P3 | complete | causal nested fit（leakage injection fail-closes） |
| P4 | complete | **6 mandatory family 全部实现**（C1/B1/M2R + P1/K1/V1） |
| P5 | skipped | Soft-SEL（无 parent 过 train 门后组合） |
| P6 | complete | G0 binding + G1 search（1408 replays） |
| P7 | complete | G1 causal inner-OOS（P1 4/4 folds survivors，C1 F3/F4 survivors） |
| P8 | complete | G2 full-train + subblocks + budget（2350 replays，P1+C1 finalists） |
| P9 | complete | one-shot validation（16 runs，11 survivors，0 target hit） |
| P10 | skipped | 无 >=2 parent 过 validation |
| P11 | not_applicable | 0 robustness finalist 过 target |
| P12 | complete | 本文档 |

## 2. 三档目标 + P-A/P-B/P-C/P-D

```text
保守 (>=50% ann, <=10% DD, >=4/5) : []  NOT HIT
平衡 (>=90% ann, <=20% DD, >=4/5)  : []  NOT HIT
激进 (>=110% ann,<=30% DD, >=3/5)  : []  NOT HIT
```
P-A/P-B/P-C/P-D: 全 NO。`frontier_progress=false`。

## 3. best combination（OOS 验证后）

| rank | family | config | budget | val ann | val max DD | train med ann | 备注 |
|---|---|---|---:|---:|---:|---:|---|
| 1 | C1 | C1_F4_008 | 1000U | 12.7% | 4.2% | 32.8% | best OOS survivor |
| 2 | C1 | C1_F4_003 | 1000U | 12.6% | 4.2% | — | — |
| 3 | M2R | M2R_F3_003 | 2000U | 4.4% | 2.3% | 1.1% | low DD low ann |
| 4 | P1 | P1_F3_013 | 1000U | 3.9% | 17.1% | 28.7% | train overfit |
| 5 | C1 | C1_F3_003 | 2000U | 1.6% | 15.5% | 51.7% | train overfit |

## 4. family counts

```text
G1: 1408 replays (6 family × configs × 2-3 blocks × 2 budgets × 4 folds)
G2: 2350 replays (G1 survivors × 6 windows × 5 budgets)
Validation: 16 replays (top-2 G2 finalists × folds)
合计: 3774 full synchronized_cycle_replay，0 快筛，0 cache-as-result
```

## 5. 关键诚实诊断

- **train→OOS gap 巨大**：P1_F4 train 68.8% → val -6.2%；C1_F3 train 51.7% → val 1.6%。短窗 train 信号大量过拟合。
- **最佳 OOS 是 C1_F4_008**（concentration repair）：val 12.7%/4.2% — 正向但不达目标。低 DD 是真实进步（exchange_model filter + gross cap + concentration 起作用）。
- **P1 partial-cointegration** 是最强 train family（4/4 folds survivors）但 OOS 衰减严重。
- **B1/K1** 全 fold 0 survivors（机制不产生足够 train-positive blocks）。

## 6. 工程产物

- `apps/backtest-engine/src/martingale/exchange_model.rs`（filters + liquidation + cooldown，9 测）
- `crates/shared-domain/src/martingale.rs`（C1SchedulerConfig）
- `scripts/glm_r20_launcher.py`（单一 launcher）
- `scripts/glm_r20_state_machine.py`（validator）
- `scripts/glm_r20_r3_causal_fit.py`（causal nested fit）
- `scripts/glm_r20_r4_families_fit.py` + `glm_r20_m2r_constrained_solve.py` + `glm_r20_r4b_p1_k1_v1_fit.py`（6 family fits）
- `scripts/glm_r20_r6r7_g0_g1.py`（G0+G1 search）
- `scripts/glm_r20_r8_g2.py`（G2 search）
- `scripts/glm_r20_r9_validation.py`（one-shot validation）

## 7. future lock

2026-07-11+ 继续封存（<30 天，0 finalist 过 target）。无 candidate 进 future check。

## 8. 结论

Round 20 完整执行了搜索管道：6 个 mandatory family 全部实现，3774 次 full replay（无快筛），
one-shot OOS validation 诚实报告 0 target hit。最佳 OOS 组合 C1_F4_008@1000U val 12.7%/4.2%
是正向但不达 50/90/110%。train→OOS gap 证实短窗信号过拟合仍是核心约束。
