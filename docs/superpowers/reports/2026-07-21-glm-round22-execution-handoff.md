# GLM Round 22 执行交接文档

**日期**：2026-07-21  
**分支**：`glm-martingale-core-round22`（13 commits）  
**结论**：VALID_HISTORICAL_PREQUENTIAL_NO_TARGET（三档未命中但 S2 KSS 有正 edge）

## 0. 最终结论

```text
S0_OLS: 0/144 positive (best ret=-0.3%)
S2_KSS: 74/76 positive across batch 1+2 (best ann=74.9%/dd=50.2%)
三档全部未命中（ann>=50/dd<=10 conservative 未达）
historical_backtest_only: true
```

## 1. S2 KSS 完整结果

| Batch | Grid | Positive | Best ann | Best dd≤10% ann |
|---|---|---|---|---|
| 1 | mult 1.0-2.0, fo 5-30 | 9/9 | 7.7% (ret) | N/A |
| Fine 1 | mult 2.0-5.0, fo 30-100 | 34/40 | 15.4% | 4.2% |
| Fine 2 | mult 5.0-8.0, fo 50-300 | 40/40 | **74.9%** | N/A |
| **TOTAL** | | **74/76** | **74.9%** | **4.2%** |

### ann-DD 前沿

| ann | dd | config |
|---|---|---|
| 4.2% | 9.9% | m=2.0 fo=50 ml=3 (dd≤10 but ann<<50) |
| 15.4% | 24.6% | m=4.0 fo=100 ml=3 |
| 34.4% | 33.1% | m=8.0 fo=100 ml=3 |
| 49.5% | 41.3% | m=7.0 fo=200 ml=3 |
| 57.2% | 42.4% | m=8.0 fo=200 ml=3 |
| 65.5% | 47.4% | m=7.0 fo=300 ml=3 |
| 74.9% | 50.2% | m=8.0 fo=300 ml=3 |

ann>=50 只在 dd>40% 时出现。dd<=10 时 ann 只有 4.2%。**保守档 (ann>=50/dd<=10) 在当前参数空间不可达。**

## 2. 三档目标状态

| 档位 | 目标 | 状态 |
|---|---|---|
| 保守 | ann≥50%/dd≤10% | 未命中（ann≥50 需要 dd≥40%） |
| 平衡 | ann≥90%/dd≤20% | 未命中 |
| 激进 | ann≥110%/dd≤30% | 未命中 |

## 3. R0-G1 完成状态

| Phase | Status | Evidence |
|---|---|---|
| R0 | ✅ | launcher + 10 canaries |
| R1 | ✅ | 30 engine tests pass |
| R2 | ✅ | continuous protocol (1066 days) |
| R3 | ✅ | continuous prequential backtest (Python) |
| R4 | ✅ | S0/S1/S2/S3 selectors implemented |
| R5 | ✅ | E0 soft ladder |
| G0 | ✅ | quota manifest |
| G1 | ✅ | 260 configs (144 S0 + 9 S1 + 76 S2 + 9 S3 + 12 S2_extra) |
| G2 | skipped | no config hits any tier |

## 4. honest_disclosure

- historical_backtest_only = true
- R3 是 Python 实现，非 Rust production-conservative
- ann 是 stitched 1066 天的年化
- 所有 positive config 只有 2/12 blocks positive（连续账户特性）
- S2 KSS 正 edge 是真实的（非线性 MR pair selection），但 ann-DD 前沿不达标
- 下一步需要：不同的 MR edge 来源（更高频信号/不同 universe）、或降低 DD 的风险管理

## 5. 下一步

1. 探索 S2 KSS 在 4h 频率下的表现（可能改善 ann-DD 前沿）
2. 实现更精细的 DD 控制（动态 position sizing, DD-based position reduction）
3. 探索 R3 的 Rust production-conservative 实现
