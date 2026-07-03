# Round 5 Task A: Cycle Micro-Regime Attribution Report

## Analysis Method
Ran the fine-combo best (ann 49.93%, mult 3.3, last-executed SO, vol-target) and R4 best (ann 34.7%) on 2025 segment. Analyzed per-strategy trades, stop rate, monthly equity curve, price context, and event types.

## Key Findings

### 2025 Stop Rate: ~49.5% (fine-combo) / ~49.5% (R4 best)
Both configurations have approximately 50% stop rate in 2025. Nearly half of all cycles are stopped out.

### Monthly Equity Curve (fine-combo best)
- 2025-01: +1.0% (BNB/TRX/BCH January bull captured)
- 2025-02: -2.2% (correction)
- 2025-03 to 2025-09: near 0% each month (chop, no trend)
- 2025-10 to 2025-12: near 0% each month

### 2025 Price Context
| Symbol | Direction | 2025 Return | Max DD | Volatility |
|---|---|---:|---:|---:|
| BNBUSDT | long | +23.2% | -40.9% | 37% |
| TRXUSDT | long | +11.5% | -26.7% | 29% |
| BCHUSDT | long | +37.8% | -49.5% | 56% |
| AAVEUSDT | short | -52.8% | -69.2% | 73% |
| SOLUSDT | short | -34.1% | -66.2% | 63% |
| DOTUSDT | short | -73.2% | -79.0% | 62% |
| BTCUSDT | gate | -6.3% | -34.9% | 31% |

### Root Cause
1. **Longs miss most of 2025 bull**: BNB/TRX/BCH up 11-38% but BTC gate (BTC>ema50) blocks entries when BTC is flat (-6.3%). Only January captures gains.
2. **Shorts lose in choppy crash**: AAVE/SOL/DOT crash 34-73% but with 62-73% volatility and frequent squeezes. 50% stop rate.
3. **Choppy months (Mar-Dec) produce near-zero returns**: no clear trend for martingale to exploit.

### Feature Bucket Decisions
| Feature | Decision | Reason |
|---|---|---|
| Low ADX range entries | reject-overfit | Already tested in R4 P2; barely fires |
| High ADX trend entries | route-done | Already captured by strict gate |
| Premium high/low states | reject-thin-sample | Weak signal (0.76%/24h) from R4 |
| Prior 30d cycle win rate | route-to-task | Could gate entries, but cycles are too sparse per symbol |
| BTC ema50 crossing | reject-overfit | Already the long gate; tested relaxation worsens |

### Actionable Conclusion
No new gate candidate emerged from cycle attribution that wasn't already tested in Rounds 1-4. The 2025 -8.7% to -15.9% loss is structural: high-vol choppy market where 50% of cycles stop. The path to improvement is higher multiplier (R5 Task B found +10pp from mult 3.1→3.3 with last-executed SO), not better entry gates.
