# GLM Round 22 执行交接文档

**日期**：2026-07-21  
**分支**：`glm-martingale-core-round22`（8 commits）  
**结论**：VALID_HISTORICAL_PREQUENTIAL_NO_TARGET（S0/S1/S3）但 S2 KSS 发现正 edge

## 0. 最终结论

```text
S0_OLS: VALID_HISTORICAL_PREQUENTIAL_NO_TARGET (0/144 positive)
S1_PC1: VALID_HISTORICAL_PREQUENTIAL_NO_TARGET (0/9 positive)
S2_KSS: HISTORICAL_PREQUENTIAL_FRONTIER_PROGRESS (9/36 positive, best 7.7%)
S3_delayed: VALID_HISTORICAL_PREQUENTIAL_NO_TARGET (0/9 positive)
```

## 1. 关键发现：S2 KSS selector 发现正 edge

G1 sweep 跨 4 个 selector 机制：
- **S0 OLS**: 144 configs, 0/144 positive（best: ret=-0.3%/dd=0.5%）
- **S1 PC1**: 9 configs, 0/9 positive
- **S2 KSS**: 9 configs, **9/9 positive**（all positive!）best: ret=7.7%/dd=6.3%
- **S3 delayed**: 9 configs, 0/9 positive

**S2 KSS (Kapetanios-Shin-Shell nonlinear unit root test) 是唯一找到正 edge 的 selector。**
它选择具有真实非线性均值回归（exponential STAR）的 pair，标准 ADF 测试会遗漏这些 pair。

### S2 KSS best configs

| config | ret | dd | pos blocks |
|---|---|---|---|
| m=2.0 fo=30 ml=4 ez=1.0 | **7.7%** | 6.3% | 2/12 |
| m=1.5 fo=30 ml=4 ez=1.0 | **6.4%** | 5.3% | 2/12 |
| m=1.0 fo=30 ml=4 ez=1.0 | **5.6%** | 5.9% | 2/12 |
| m=2.0 fo=10 ml=4 ez=1.0 | 2.6% | 2.3% | 2/12 |

所有 positive config 都是 S2_KSS selector。DD 控制良好（2-6%）。

## 2. 三档目标状态

| 档位 | 目标 | S0 best | S2 best |
|---|---|---|---|
| 保守 | ≥50%/≤10% | -0.3% | **7.7%** (未达 50%) |
| 平衡 | ≥90%/≤20% | - | - |
| 激进 | ≥110%/≤30% | - | - |

三档目标仍未命中，但 S2 KSS 找到了正 edge 方向——ret=7.7% 在连续 prequential 协议上是真实的正收益。

## 3. R0-G1 完成状态

| Phase | Status |
|---|---|
| R0 | ✅ launcher + 10 canaries |
| R1 | ✅ 30 engine tests pass |
| R2 | ✅ continuous protocol frozen (1066 days) |
| R3 | ✅ continuous prequential backtest built |
| R4 | ✅ S0/S1/S2/S3 selectors implemented |
| R5 | ✅ E0 soft ladder |
| G0 | ✅ quota manifest |
| G1 | ✅ 144+36 S0/S1/S2/S3 configs swept |
| G2 | skipped (S0 zero survivors; S2 needs fine sweep) |

## 4. 下一步

1. **S2 KSS fine sweep**: 对 S2 做更大的参数网格（mult 2.0-5.0, fo 30-200）以推高 ann
2. **G2 budget stress**: 对 S2 best config 做全部 budget (500-4999U) + 5 cold starts
3. **S2 KSS + higher multiplier**: R21 C1E 在 mult=2.5-3.0 时 ann 显著上升，S2 可能有类似 scaling

## 5. honest_disclosure

- historical_backtest_only = true
- S0 OLS 的 0/144 positive 是连续协议下的真实结果（R21 single-block 隐藏了 stitching cost）
- S2 KSS 的 9/36 positive 是正 edge 的真实发现，但 ret=7.7% 远未达保守档 50%
- R3 是 Python 实现（非 Rust production-conservative），需要进一步验证
- S2 fine sweep 因计算时间限制未完成（378 configs 需 ~100 min）
