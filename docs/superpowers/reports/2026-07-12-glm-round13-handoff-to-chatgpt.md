# GLM Martingale Core Round 13 — Final Handoff

## Summary

Round 13 completed all P0-P9 with full backtesting. Key deliverables:

### New Code
1. **Event-level shadow/live allocator** (`event_level_allocator.rs`): dual-state model with 10 required tests
2. **Batch replay infrastructure** (`batch_replay.rs`): preload + parallel, 5 parity tests
3. **Native minigrid engine integration** (kline_engine.rs): dca_minigrid config now BINDS

### Search Results
| Task | Configs | Replays | Result |
|------|---------|---------|--------|
| P2 ablation | 1176 | 7056 | All worse than baseline |
| P3 scheduler | 128 | 768 | BNB+TRX ann=65.6%/DD=29.6% |
| P4 minigrid | 512 | 3072 | Binds! ann=32.0%/DD=17.7% |

### Best Event-Level Candidate
BNB+TRX (2 strategies): ann=65.6%/DD=29.6% — but fails neighbor stability (19%), holdout (-35.9%), small capital (1000U negative)

### Target Status
All 3 targets NOT MET. Conservative DD gate reached (TRX+AAVE 9.9%) but ann far below 50%.
