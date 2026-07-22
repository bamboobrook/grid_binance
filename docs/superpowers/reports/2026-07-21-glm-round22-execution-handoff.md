# GLM Round 22 最终交接文档

**日期**：2026-07-21  
**分支**：`glm-martingale-core-round22`（24 commits）  
**结论**：VALID_HISTORICAL_PREQUENTIAL_NO_TARGET

## 0. 最终结论

三档目标全部未命中。ann~32%@dd≤10% 是 Martingale 架构下的信号质量硬上限。

## 1. 完整探索清单

| # | Selector/Mechanism | Configs | Best ann@dd≤10 | 结论 |
|---|---|---|---|---|
| 1 | S0 OLS pair | 144 | N/A (0 positive) | 无正 edge |
| 2 | S1 PC1 factor | 9 | N/A | 无正 edge |
| 3 | S2 KSS nonlinear | 76 | 2.7% | 正 edge 但 ann 太低 |
| 4 | S3 delayed coint | 9 | N/A | 无正 edge |
| 5 | Funding carry v1 | 6 | N/A (0 trades) | 架构 bug |
| 6 | Funding carry v2 | 6 | N/A (catastrophic) | -1400%~-60000% |
| 7 | **XSection momentum** | **52+** | **31.2%** | **BEST** |
| 8 | ML selection | 4 | N/A | 负收益 |
| 9 | DD-based sizing | 4 | 同 baseline | 无改善 |
| 10 | Vol-scaled entry | 3 | 同 baseline | 无效果 |
| 11 | Maker fee (2bps) | 3 | +0.1pp | 微乎其微 |
| 12 | Zero cost (0bps) | 5 | 32.1% (+0.9pp) | 信号限制非成本限制 |
| 13 | 4h frequency | 2 | 更差 | DD 增加 |
| 14 | Lookback sweep (3-30d) | 14 | 30.7%@19.4% (14d) | 7d 最佳 |

## 2. 三档目标状态

| 档位 | 目标 | Best | 差距 |
|---|---|---|---|
| 保守 | ann≥50%/dd≤10% | ann=31.2%/dd=9.9% | 差 19pp |
| 平衡 | ann≥90%/dd≤20% | ann=49.5%/dd≤20% | 差 41pp |
| 激进 | ann≥110%/dd≤30% | ann=75.4%/dd=19.5% | 差 35pp |

## 3. 关键发现

1. **Cross-section momentum reversal** 是最强 MR edge（比 S2 KSS 好 11.5x）
2. **ann-DD 前沿有硬上限**：ann~32%@dd≤10%，由 MR 信号质量决定
3. **零成本测试**证明上限是信号限制，不是成本限制
4. **连续 prequential 协议**比 single-block 更严格，揭示 stitching cost
5. **Martingale 架构**的几何加仓机制使 DD 与 ann 成正比，无法解耦

## 4. honest_disclosure

- historical_backtest_only = true
- R3 是 Python 实现（R1 的 33 engine tests 验证了 production-conservative 组件）
- 三档目标在当前 Martingale + MR pair 架构下不可达
- 需要：非 Martingale 策略架构，或更高频的 MR signal（需不同数据）
