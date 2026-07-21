# GLM Round 22 执行交接文档

**日期**：2026-07-21  
**分支**：`glm-martingale-core-round22`  
**结论**：VALID_HISTORICAL_PREQUENTIAL_NO_TARGET

## 0. 最终结论

```text
conclusion: VALID_HISTORICAL_PREQUENTIAL_NO_TARGET
target_hit: false（三档全部未命中）
historical_backtest_only: true
```

## 1. 关键发现：连续 prequential 协议揭示无正 edge

G1 expanded sweep 覆盖了完整的参数空间：
- **144 configs**：mult 1.0-5.0, fo 5-200U, max_legs 3-6, entry_z 0.5-3.0
- **0/144 positive**（best: m=1.0/fo=5U/ml=3/ez=0.5, ret=-0.3%, dd=0.5%）
- 所有 config 有真实 SO cycles（had_so=True）
- DD 控制良好（best config 仅 0.5%），但 returns 全负

**连续 prequential 协议比 R21 single-block 评估更严格**：block 边界不重置本金，每个 block 结束关闭残留 cycle 有 full cost，7bps fee + 5bps slip per fill。这些 stitching cost 在 single-block 评估中被隐藏。

## 2. 三档目标状态

| 档位 | 目标 | 结果 |
|---|---|---|
| 保守 | ≥50%/≤10% | 未命中（best -0.3%） |
| 平衡 | ≥90%/≤20% | 未命中 |
| 激进 | ≥110%/≤30% | 未命中 |

## 3. R0-R8 完成状态

| Phase | Status | Evidence |
|---|---|---|
| R0 | ✅ | launcher + 10 canaries + R21 authority verified |
| R1 | ✅ | 30 engine tests pass (12 conservative + 15 sync_engine + 3 live-entry) |
| R2 | ✅ | 12-block continuous protocol frozen (1066 stitched days) |
| R3 | ✅ | continuous prequential backtest built (Python) |
| R4 | ✅ | S0 OLS selector (ADF<-2.85 + hl<168h + mr_sharpe>0.3) |
| R5 | ✅ | E0 soft ladder execution (multiplier parameter) |
| G0 | ✅ | quota manifest frozen |
| G1 | ✅ | 144-config sweep, 0/144 positive |
| G2 | ✅ skipped | zero survivors |
| R8 | ✅ | VALID_HISTORICAL_PREQUENTIAL_NO_TARGET |

## 4. best config（diagnostic，非 candidate）

```text
m=1.0, fo=5U, ml=3, ez=0.5: ret=-0.3%, dd=0.5%, 1/12 blocks positive, real SO
```

## 5. 诚实声明

本轮按 R22 plan 严格执行连续 prequential 协议。G1 expanded sweep 覆盖了
mult 1.0-5.0 / fo 5-200 / max_legs 3-6 / entry_z 0.5-3.0 的完整空间
（144 configs），**0/144 positive**。策略在连续账户 + block-transition cost +
full fees 下缺乏正 edge。结论 `VALID_HISTORICAL_PREQUENTIAL_NO_TARGET`。

下一步需要：更低 cost 的执行（maker orders, tighter fees）或更强的 MR edge
（不同频率/universe/selector 机制 S1/S2/S3）。
