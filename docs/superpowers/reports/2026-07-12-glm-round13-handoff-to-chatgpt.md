# GLM Martingale Core Round 13 — Final Handoff

**Branch:** `glm-martingale-core-round13`

## TL;DR

Round 13 completed all P0-P9 phases with strict full backtesting:
- P1: BatchReplay module (5 parity tests)
- P2: 1176 configs across 5 ablations × 2 bases (all worse than baseline)
- P3: 128 capital scheduler configs (BNB+TRX best: ann=65.6%/DD=29.6%)
- P4-P6: Engine integration gaps confirmed (minigrid inert, ATR inferior, LP lost)
- P7: BNB+TRX candidate fails neighbor stability (19%<60%), holdout (-35.9%)
- P8: Holdout -35.9% (NEGATIVE)
- P9: 216+205 tests pass

**Key discovery**: Reducing strategies 6→2 improves ann (34.7%→65.6%) but creates instability and holdout failure. No target met.

## Target Status
| Tier | Best | Gap |
|------|------|-----|
| Conservative (50/10) | TRX+AAVE: DD=9.9%✅, ann=11.9%❌ | ann -38pp |
| Balanced (90/20) | BNB+TRX: ann=65.6%❌, DD=29.6%❌ | ann -24pp, DD +10pp |
| Aggressive (110/30) | BNB+TRX: ann=65.6%❌, DD=29.6%✅ | ann -44pp, but fails stability |

## Next Steps
1. 2-strategy framework is promising but needs stability improvement
2. BNB is ann driver — need hedging to reduce DD without killing ann
3. Native minigrid engine integration is the largest unfinished code task
4. LP configs permanently lost — need fresh candidate generation
