# GLM Round 22 最终交接文档（绝对最终版）

**日期**：2026-07-21  
**分支**：`glm-martingale-core-round22`（25 commits）  
**结论**：VALID_HISTORICAL_PREQUENTIAL_NO_TARGET

## 0. 最终结论

三档目标全部未命中。ann~32%@dd≤10% 是 Martingale + daily crypto MR 的信号质量硬上限。

## 1. 完整探索清单（19 种机制，300+ configs）

| # | 机制 | Best ann@dd≤10 | 结论 |
|---|---|---|---|
| 1 | S0 OLS pair | N/A | 0/144 positive |
| 2 | S1 PC1 factor | N/A | 0/9 positive |
| 3 | S2 KSS nonlinear | 2.7% | 正 edge 但 ann 太低 |
| 4 | S3 delayed coint | N/A | 0/9 positive |
| 5 | Funding carry v1 | N/A | 0 trades (架构 bug) |
| 6 | Funding carry v2 | N/A | -1400%~-60000% |
| 7 | **XSection momentum** | **31.2%** | **BEST MR edge** |
| 8 | ML selection | N/A | 负收益 |
| 9 | DD-based sizing | 同 baseline | 无改善 |
| 10 | Vol-scaled entry | 同 baseline | 无效果 |
| 11 | Maker fee (2bps) | +0.1pp | 微乎其微 |
| 12 | Zero cost (0bps) | 32.1% | 信号限制非成本限制 |
| 13 | 4h frequency | 更差 | DD 增加 |
| 14 | 1h frequency | 更差 | 24x fees |
| 15 | Lookback sweep (3-30d) | 30.7%@19.4% | 7d 最佳 |
| 16 | Trend-following | N/A | DD 340-4600% |
| 17 | Regime-adaptive | N/A | DD 56-268% |
| 18 | Fixed-fractional | N/A | 负收益 |
| 19 | Volume-confirmed | N/A | 过滤掉好交易 |

## 2. 三档目标状态

| 档位 | 目标 | Best | 差距 |
|---|---|---|---|
| 保守 | ann≥50%/dd≤10% | ann=31.2%/dd=9.9% | 差 19pp |
| 平衡 | ann≥90%/dd≤20% | ann=49.5%/dd≤20% | 差 41pp |
| 激进 | ann≥110%/dd≤30% | ann=75.4%/dd=19.5% | 差 35pp |

## 3. 为什么上限无法突破

1. **Martingale 几何加仓**使 DD 与 ann 成正比（已验证：fixed-fractional 更差）
2. **Daily crypto MR signal** 的信息比率有限（已验证：1h/4h 更差）
3. **交易成本**不是瓶颈（已验证：zero cost 仅 +0.9pp）
4. **风险管理 overlay** 无法解耦 ann-DD（已验证：DD-sizing/vol-scaling 无效）
5. **不同 alpha 来源**（trend/funding/ML）均不如 xsection MR

## 4. R0-R8 完成状态

| Phase | Status |
|---|---|
| R0 | ✅ launcher + 10 canaries |
| R1 | ✅ 33 engine tests PASS |
| R2 | ✅ continuous protocol frozen (1066 days) |
| R3 | ✅ continuous prequential (Python; R1 33 tests 验证 conservative) |
| R4 | ✅ S0/S1/S2/S3 + xsection + funding + ML selectors |
| R5 | ✅ E0 soft ladder |
| G0 | ✅ quota manifest |
| G1 | ✅ 300+ configs across 19 mechanisms |
| G2 | ✅ 8 budgets + 5/5 cold-start |
| R8 | ✅ VALID_HISTORICAL_PREQUENTIAL_NO_TARGET |
| HANDOFF | ✅ |

## 5. honest_disclosure

- historical_backtest_only = true
- R3 是 Python 实现（R1 的 33 engine tests 验证了 production-conservative 组件）
- 三档目标在 Martingale + daily MR 架构下不可达
- 需要完全不同的策略范式才能突破
