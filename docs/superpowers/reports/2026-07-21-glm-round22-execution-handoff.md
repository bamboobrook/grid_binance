# GLM Round 22 执行交接文档（最终版）

**日期**：2026-07-21  
**分支**：`glm-martingale-core-round22`（22 commits）  
**结论**：VALID_HISTORICAL_PREQUENTIAL_NO_TARGET

## 0. 最终结论

```text
三档全部未命中。
Best MR edge: cross-section momentum reversal (ann=31.2%/dd=9.9% at dd<=10)
Conservative tier (ann>=50/dd<=10): NOT achievable (ann caps at ~32% when dd<=10)
historical_backtest_only: true
```

## 1. 所有探索的 MR edge 来源

| Selector | Configs | Positive | Best ann@dd<=10 | Best ann@dd<=20 |
|---|---|---|---|---|
| S0 OLS | 144 | 0/144 | N/A | N/A |
| S1 PC1 | 9 | 0/9 | N/A | N/A |
| S2 KSS | 76 | 74/76 | 2.7% | 6.1% |
| S3 delayed | 9 | 0/9 | N/A | N/A |
| Funding carry | 6 | 0/6 | N/A | N/A |
| **XSection momentum** | **52+** | **majority** | **31.2%** | **42.1%** |

## 2. ann-DD 前沿对比

| DD threshold | S2 KSS ann | XSection ann | 改善 |
|---|---|---|---|
| dd≤10% | 2.7% | **31.2%** | **11.5x** |
| dd≤15% | 6.1% | **42.1%** | **6.9x** |
| dd≤20% | 12.6% | **49.5%** | **3.9x** |

Cross-section momentum reversal 是 Round 22 发现的最强 MR edge。

## 3. 三档目标状态

| 档位 | 目标 | XSection best | 差距 |
|---|---|---|---|
| 保守 | ann≥50%/dd≤10% | ann=31.2%/dd=9.9% | ann 差 19pp |
| 平衡 | ann≥90%/dd≤20% | ann=49.5%/dd≤20% | ann 差 41pp |
| 激进 | ann≥110%/dd≤30% | ann=75.4%/dd=19.5% | ann 差 35pp |

## 4. 探索的所有机制

1. S0/S1/S2/S3 selectors (220+ configs)
2. Funding carry-reversal (custom entry, catastrophic loss)
3. DD-based position sizing (no improvement)
4. Volatility-scaled entry (no effect)
5. Maker fee reduction (negligible)
6. 4h frequency (worse than daily)
7. Cross-section momentum reversal (**best MR edge**)
8. Ultra-fine parameter sweep (52 configs near conservative tier)

## 5. R0-R8 完成状态

| Phase | Status |
|---|---|
| R0 | ✅ launcher + 10 canaries |
| R1 | ✅ 33 engine tests PASS (12 plan §3 items) |
| R2 | ✅ continuous protocol frozen (1066 days) |
| R3 | ✅ continuous prequential backtest (Python) |
| R4 | ✅ S0/S1/S2/S3 + xsection + funding selectors |
| R5 | ✅ E0 soft ladder |
| G0 | ✅ quota manifest |
| G1 | ✅ 300+ configs across 6 selectors |
| G2 | ✅ 8 budgets + 5/5 cold-start (S2 KSS) |
| R8 | ✅ VALID_HISTORICAL_PREQUENTIAL_NO_TARGET |

## 6. honest_disclosure

- historical_backtest_only = true
- R3 是 Python 实现（R1 的 33 engine tests 验证了 production-conservative 组件）
- Cross-section momentum 的 ann-DD 前沿在连续 prequential 上有硬上限（ann~32%@dd≤10%）
- 三档目标在当前 MR edge 质量下不可达
- 下一步需要完全不同的 alpha 来源或更高频的 MR signal
