# GLM Round 4 P1: 2025 Cycle Attribution Report

## Key Finding: Longs MISS the 2025 bull, Shorts lose in choppy crash

2025 price analysis:
| Symbol | Direction | 2025 return | Max DD | Volatility |
|---|---|---:|---:|---:|
| BNBUSDT | long | +23.2% | -40.9% | 37% |
| TRXUSDT | long | +11.5% | -26.7% | 29% |
| BCHUSDT | long | +37.8% | -29.5% | 56% |
| AAVEUSDT | short | -52.8% | -69.2% | 73% |
| SOLUSDT | short | -34.1% | -66.2% | 63% |
| DOTUSDT | short | -73.2% | -79.0% | 62% |
| BTCUSDT | (gate) | -6.3% | -34.9% | 32% |

## Root Cause Analysis

1. **Longs are BLOCKED by BTC gate**: BNB/TRX/BCH all went UP in 2025 (+11 to +38%), but the strict long gate requires `BTCUSDT.close > BTCUSDT.ema(50)`. BTC was flat (-6.3%) and oscillated around ema50, so the long gate fired rarely. **The long legs are leaving 2025 bull profit on the table.**

2. **Shorts lose in choppy crash**: AAVE/SOL/DOT crashed hard (-34 to -73%), but 2025 was CHOPPY (high volatility 62-73%, frequent short squeezes). The martingale short averages INTO squeezes, gets stopped on bounces. 4237 stops out of 8494 trades = 50% stop rate.

3. **The fix is clear**: Remove or relax the BTC gate for longs so they can capture the 2025 bull in BNB/TRX/BCH. The per-symbol gate (`close>ema50>ema200`) is sufficient — the BTC overlay is too restrictive.

## Routing to Round 4 directions
| Finding | Route |
|---|---|
| Longs blocked by BTC gate miss 2025 bull | P2 range sleeve OR relax BTC gate for longs |
| Shorts lose in choppy crash | P3 pump-fade short (enter after exhaustion, not trend) |
| 50% stop rate = too many stops in chop | P4 active exit (reduce stale cycles) or P5 rebound SO |
| High volatility 62-73% in crash coins | Reduce short leg size or use tighter SO |

## Immediate test recommendation
Test r3-P1-best with the BTC gate REMOVED from longs (keep per-symbol close>ema50>ema200 only). This should capture the 2025 BNB/TRX/BCH bull and potentially flip 2025 positive.
