# GLM Round 22 执行交接文档

**日期**：2026-07-21  
**分支**：`glm-martingale-core-round22`  
**权威**：`r8/selected-configs.json`

## 结论：VALID_HISTORICAL_PREQUENTIAL_NO_TARGET

```text
conclusion: VALID_HISTORICAL_PREQUENTIAL_NO_TARGET
target_hit: false（三档全部未命中）
historical_backtest_only: true
```

## 关键发现

连续 prequential 协议（plan §4）揭示了 single-block 评估隐藏的 stitching cost：
- G1 sweep 18 configs：0/18 positive（best: -2.0%/dd=3.0%）
- 策略在连续账户+block-transition cost+full fees 下缺乏正 edge
- DD 很低（3-9%）但 returns 全负
- 所有 configs 有真实 SO cycles（had_so=True）

## R0-R8 完成状态

| Phase | Status |
|---|---|
| R0 | complete (launcher + 10 canaries) |
| R1 | complete (30 engine tests pass) |
| R2 | complete (12-block continuous protocol, 1066 days) |
| R3 | complete (continuous prequential backtest built) |
| R4-R5 | complete (S0 OLS selector + E0 soft ladder) |
| G0 | complete (quota manifest frozen) |
| G1 | complete (18-config sweep, 0/18 positive) |
| G2 | skipped (zero survivors) |
| R8 | complete (VALID_HISTORICAL_PREQUENTIAL_NO_TARGET) |

## 三档目标

| 档位 | 目标 | 结果 |
|---|---|---|
| 保守 | ≥50%/≤10% | 未命中（best -2.0%） |
| 平衡 | ≥90%/≤20% | 未命中 |
| 激进 | ≥110%/≤30% | 未命中 |

## best config

```text
m=1.5, fo=10U, ez=0.8/1.5: ret=-2.0%, dd=3.0%, 2/12 positive, real SO
```

## 下一步

连续 prequential 协议比 single-block 更严格。要达成正 edge 需要：
1. 更低 cost 的执行（maker orders, tighter fees）
2. 更强 MR edge（不同频率/universe/selector）
3. 更好的 pair rotation 策略（减少 transition cost）
