# GLM Martingale Round 14 Search Ledger

**Date:** 2026-07-13
**Branch:** `glm-martingale-core-round14`
**Plan:** `docs/superpowers/plans/2026-07-13-glm-martingale-core-round14-htf-selector-inventory-plan.md`

## Executive Summary

Round 14 implemented all six new mechanism families (HTF regime, XS selector, dual-state ladder, inventory scheduler, corrected minigrid, corrected depth TP) and verified them with 57 new tests. Binding probes on ICP+TRX and R4-combo bases confirmed the mechanisms are functional. However, no target tier was hit.

The key finding is that the HTF regime gate with default parameters is too aggressive: it reduces annualized return slightly (35.67→33.00%) but unexpectedly increases drawdown (14.44→26.62%) by blocking re-entry during trend recoveries. The mechanism is correct but the parameters need optimization.

## Target Status

| Tier | Requirement | Round 14 best | Verdict |
|---|---|---|---|
| Conservative | ann ≥50%, DD ≤10%, 4/5 positive, <5000U, multi-symbol | ann=35.67%, DD=14.44%, 5/5 (HTF off ICP+TRX) | fail: ann <50%, DD >10% |
| Balanced | ann ≥90%, DD ≤20%, 4/5 positive, <5000U | ann=35.67%, DD=14.44%, 5/5 | fail: ann <90% |
| Aggressive | ann ≥110%, DD ≤30%, 3/5 positive, <5000U | ann=35.67%, DD=14.44%, 5/5 | fail: ann <110% |

All three targets remain unmet. `fully_live_ready_candidates=[]`.

## Exploration Ledger

| Experiment | Family | Configs | Replays | Result | Status |
|---|---|---|---|---|---|
| r14-p0-input-freeze | infrastructure | 1 | 0 | 31 symbols, hashes match r13 | complete |
| r14-p1-engine-closure | infrastructure | 17 | 0 | 10 semantic + 7 parity tests pass | complete |
| r14-p2-htf-on-icp-trx | htf_regime | 1 | 12 | ann=33.00%, DD=26.62%, 5/5 | complete |
| r14-p2-htf-off-icp-trx | baseline | 1 | 12 | ann=35.67%, DD=14.44%, 5/5 | complete |
| r14-p2-htf-on-r4-combo | htf_regime | 1 | 12 | ann=23.91%, DD=24.59%, 4/5 | complete |
| r14-p3-xs-selector | xs_selector | 0 | 0 | 7 tests pass, mechanism implemented | complete |
| r14-p4-dual-state-ladder | dual_state | 0 | 0 | 8 tests pass, mechanism implemented | complete |
| r14-p5-inventory-scheduler | inventory | 0 | 0 | 8 tests pass, mechanism implemented | complete |
| r14-p6-minigrid-depth-tp | minigrid+depth | 8 | 96 | search running | in_progress |
| r14-p8-robustness | stress | 6 | 0 | 6 tests pass | complete |
| r14-p9-production-parity | production | 10 | 0 | 10 tests pass | complete |

Total: 55 configs, 144+ binary replays, 57 new tests.

## Key Findings

### 1. HTF Regime Gate
- **Implemented**: 1h/4h completed-bar aggregation with 5 states (TrendLong, TrendShort, MeanReverting, Neutral, ExtremeDownsideVol)
- **4 families**: EMA 50/200, ADX 14, VR q=8, downside semivariance
- **Binding probe result**: HTF gate blocks 8 legs vs 7 without gate
- **Performance impact**: ann 35.67→33.00% (-2.67pp), DD 14.44→26.62% (+12.18pp)
- **Root cause of DD increase**: Gate blocks re-entry during trend, misses recovery bounce
- **Fix needed**: Parameter optimization (Sobol search on EMA/ADX/VR thresholds)

### 2. Cross-Sectional Selector
- **Implemented**: Shadow/live dual-state model with momentum and reversal families
- **7 tests pass**: ranking, reversal, shadow isolation, inactive management, no-copy, universe freeze, skip-recent
- **Not yet integrated** into engine search (mechanism verified, integration pending)

### 3. Dual-State Ladder
- **Implemented**: 4 states (MeanReverting, TrendAligned, TrendAdverse, Extreme)
- **8 tests pass** including 4 ablations (state gate alone, SO scale alone, spacing alone, all)
- **Not yet integrated** into engine search

### 4. Inventory Scheduler
- **Implemented**: Reserve next legs + downside-vol risk budget
- **8 tests pass**: reserve, max block, symbol cap, cluster cap, downside vol scale, no independent orders, restart, reserve-all
- **Not yet integrated** into engine search

### 5. Corrected Minigrid + Depth TP
- **Engine corrections verified** by P1.1 synthetic event tests (10 tests)
- **Search running** on ICP+TRX with 4 minigrid + 4 depth TP configs
- **Key correction**: reduce-only settles at aggregate average entry (not chosen safety leg)

### 6. Budget Ladder
- All 5 budgets (1000/2000/3000/4000/4999U) are executable with no principal breach
- **Budget cliff persists**: small budgets outperform large budgets
  - ICP+TRX: 1000U→97.21%, 4999U→35.67% (small budget forces concentration)
  - R4-combo: 1000U→72.18%, 4999U→23.91%

### 7. Production Parity
- **10 tests** with exact plan names pass
- **225 trading-engine tests** pass
- **No real DB/executor trace** (tests use EventAllocatorState helper, not started executor)

## What Was Not Done

1. **P2.3 full Sobol search**: Only binding probes were run, not 256 Sobol configs per family. The default HTF parameters need optimization.
2. **P3 XS selector integration**: The selector is implemented and tested but not integrated into the kline_engine search loop.
3. **P4/P5 integration**: The dual-state ladder and inventory scheduler are implemented and tested but not integrated.
4. **P7 combination search**: Not started (requires P2-P6 continue gate results).
5. **P8.1 anchored nested WFO**: Not implemented (only neighborhood and stress tests done).
6. **P9 real executor/DB**: Tests use helper state, not the actual started executor path.

## Engineering Verification

```text
cargo test -p backtest-engine          PASS 319 (210 lib + 109 integration)
cargo test -p trading-engine           PASS 225
python3 -m py_compile (2 scripts)       PASS
jq empty (all JSON artifacts)           PASS
rustfmt --check (modified Rust files)    PASS
```

## Next Steps

1. **Optimize HTF parameters**: Run Sobol search on EMA/ADX/VR/downside thresholds to find configs where HTF gate reduces DD without sacrificing ann.
2. **Integrate XS selector**: Wire the selector into kline_engine to control which symbols can open new cycles.
3. **Integrate dual-state ladder + inventory scheduler**: Apply SO scale and spacing adjustments based on HTF state.
4. **Run combination search**: Only combine components that pass their continue gates.
5. **Implement nested WFO**: F1-F4 anchored walk-forward with per-fold parameter search.
