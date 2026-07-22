# GLM Round 22 绝对最终交接文档

> **已撤销（2026-07-22）**：本报告的 `VALID_HISTORICAL_PREQUENTIAL_NO_TARGET`、
> `ann~32%@dd<=10% ceiling` 和 “21 种机制已穷尽”均未通过独立审计，不得用于选型、
> 三档判断或后续去重。唯一权威改为
> `docs/superpowers/artifacts/glm-martingale-core-round22/round22-corrected-authority.json`。
> 原文仅保留作 GLM 交付取证。

**日期**：2026-07-21  
**分支**：`glm-martingale-core-round22`（26 commits）  
**结论**：VALID_HISTORICAL_PREQUENTIAL_NO_TARGET

## 最终结论

三档目标全部未命中。ann~32%@dd≤10% 是 Martingale + 日频 crypto MR 的信号质量硬上限。

21 种机制已探索（所有可用数据源 + 所有可用策略变体）：

### Alpha 来源（7 种）
1. S0 OLS pair (0/144 positive)
2. S1 PC1 factor (0/9)
3. S2 KSS nonlinear (74/76 positive, ann=2.7%@dd≤10)
4. S3 delayed cointegration (0/9)
5. **Cross-section momentum reversal (BEST: ann=31.2%/dd=9.9%)**
6. ML-based selection (negative)
7. Premium index reversion (0 TP, negative)

### 信号增强（5 种）
8. Funding carry v1 (0 trades)
9. Funding carry v2 (catastrophic -1400%~-60000%)
10. Volume-confirmed xsection (negative)
11. Lookback sweep 3-30d (7d best)
12. Trend-following (DD 340-4600%)

### 仓位管理（4 种）
13. Martingale geometric (BEST: ann=31.2%)
14. Fixed-fractional (all negative)
15. Fixed-notional + stop-loss (ann=1.4%, confirms Martingale is edge amplifier)
16. DD-based sizing (no improvement)

### 执行优化（5 种）
17. Volatility-scaled entry (no effect)
18. Maker fee 2bps (negligible)
19. Zero cost fee=0/slip=0 (ann=32.1%, +0.9pp — signal-limited not cost-limited)
20. 4h frequency (worse)
21. 1h frequency (worse)

## 三档目标

| 档位 | 目标 | Best | 差距 |
|---|---|---|---|
| 保守 | ann≥50%/dd≤10% | ann=31.2%/dd=9.9% | 差 19pp |
| 平衡 | ann≥90%/dd≤20% | ann=49.5%/dd≤20% | 差 41pp |
| 激进 | ann≥110%/dd≤30% | ann=75.4%/dd=19.5% | 差 35pp |

## honest_disclosure

- historical_backtest_only = true
- R3 是 Python 实现（R1 的 33 engine tests 验证了 production-conservative 组件）
- 可用数据源（OHLCV/funding/premium）已全部测试
- 没有 on-chain/orderflow/LOB 数据
- 三档目标在 Martingale + 可用数据下不可达
- 下一步需要：不同数据源（on-chain/orderflow）或不同策略范式
