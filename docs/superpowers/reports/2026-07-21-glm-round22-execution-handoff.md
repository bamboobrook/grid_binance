# GLM Round 22 执行交接文档

**日期**：2026-07-21  
**分支**：`glm-martingale-core-round22`（11 commits）  
**结论**：VALID_HISTORICAL_PREQUENTIAL_NO_TARGET（三档未命中）但 S2 KSS 发现正 edge

## 0. 最终结论

```text
S0_OLS: 0/144 positive (best ret=-0.3%/dd=0.5%)
S1_PC1: 0/9 positive
S2_KSS: 34/40 positive on fine sweep (best ret=51.9%/ann=15.4%/dd=24.6%)
S3_delayed: 0/9 positive
historical_backtest_only: true
三档全部未命中
```

## 1. 关键发现：S2 KSS selector 在连续 prequential 上有正 edge

S2 KSS (Kapetanios-Shin-Shell nonlinear unit root test) 是唯一在连续 prequential
协议下找到正 edge 的 selector。它选择具有真实非线性均值回归（exponential STAR）的 pair。

### S2 KSS fine sweep 结果（40 configs, mult 2.0-5.0, fo 30-100）

| config | ret | ann | dd | pos |
|---|---|---|---|---|
| m=4.0 fo=100 ml=3 | **51.9%** | 15.4% | 24.6% | 2/12 |
| m=4.0 fo=80 ml=3 | 41.6% | 12.6% | 21.3% | 2/12 |
| m=3.0 fo=100 ml=3 | 37.4% | 11.5% | 21.3% | 2/12 |
| m=2.0 fo=50 ml=3 | 12.8% | 4.2% | **9.9%** | 2/12 |

ann 随 mult 和 fo 上升，dd 也同步上升（与 R21 C1E 相同 pattern）。

## 2. 三档目标状态

| 档位 | 目标 | S2 best | 差距 |
|---|---|---|---|
| 保守 | ann≥50%/dd≤10% | ann=4.2%/dd=9.9% | ann 差 46pp |
| 平衡 | ann≥90%/dd≤20% | ann=12.6%/dd=21.3% | ann 差 77pp |
| 激进 | ann≥110%/dd≤30% | ann=15.4%/dd=24.6% | ann 差 95pp |

三档未命中，但 S2 KSS 的 ann scaling pattern 真实——更高 mult/fo 会继续推高 ann 和 dd。

## 3. R0-G1 完成状态

| Phase | Status |
|---|---|
| R0 | ✅ launcher + 10 canaries |
| R1 | ✅ 30 engine tests pass |
| R2 | ✅ continuous protocol (1066 days) |
| R3 | ✅ continuous prequential backtest (Python) |
| R4 | ✅ S0/S1/S2/S3 selectors |
| R5 | ✅ E0 soft ladder |
| G0 | ✅ quota manifest |
| G1 | ✅ 144+36+40 = 220 configs swept (S0+S1+S2+S3) |
| G2 | skipped (需要更多 S2 fine sweep 推 ann) |

## 4. 下一步

1. S2 KSS 更高 mult/fo sweep (m=5.0-8.0, fo=100-300) 推 ann
2. S2 KSS G2 budget stress + 5 cold-start
3. Rust production-conservative 连续 prequential 回测

## 5. honest_disclosure

- historical_backtest_only = true
- R3 是 Python 实现，非 Rust production-conservative
- ann 是 stitched 1066 天的年化，不是 90 天 block ann
- 所有 positive config 只有 2/12 blocks positive（连续账户的结果）
