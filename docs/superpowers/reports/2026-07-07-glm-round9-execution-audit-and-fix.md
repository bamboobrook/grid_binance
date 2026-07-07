# GLM Round 1-9 Execution Audit And Fix

**Date:** 2026-07-07  
**Branch:** `glm-martingale-core-round9`  
**Canonical status JSON:** `docs/superpowers/artifacts/glm-martingale-core-round9/r1-r9-corrected-status.json`

## Executive Verdict

This pass does not merely audit Round 9. It fixes the records that would otherwise mislead the next GLM run:

- Old Round1/2/3/4/5/6/7/8 handoffs now carry top-level correction notices pointing to the corrected audit status.
- Round 9 JSON artifacts were corrected so the best allocator is `live_module_ready=true` but `live_ready=false`.
- Round 9 P3 timeout language was corrected to the backfilled final state: `1512/1512` evaluated, `0` skipped, `0` promoted.
- A new machine-readable canonical status file records the valid use state of all nine rounds.

No original target is met after correction:

| Tier | Requirement | Corrected status |
|---|---|---|
| Conservative | ann >=50%, DD <=10%, <5000U, multi-symbol, live reproducible | Fail. Best research ann passes but DD is 18.21%; best fully live-ready ann is 34.72%. |
| Balanced | ann >=90%, DD <=20%, <5000U, multi-symbol, live reproducible | Fail. Best research DD passes at 18.21%, but ann is 64.42% and not fully live-ready. |
| Aggressive | ann >=110%, DD <=30%, <5000U, multi-symbol, live reproducible | Fail. Best research ann is 64.42%, far below 110%, and not fully live-ready. |

## Corrected Frontier

| Candidate | Status | Ann | DD | Pos segments | Live status |
|---|---|---:|---:|---:|---|
| `r9-P5-winner-lb60-rb7-calmar-hi0.2-lo0.2` | Best research allocator | 64.4196% | 18.2111% | 5/5 | Module ready, not fully live-ready |
| `R4-combo` | Best fully live-ready sleeve | 34.7233% | 17.6843% | 4/5 | Fully live-ready |

The Round 9 research allocator trades 8 symbols: AAVEUSDT, ANKRUSDT, BCHUSDT, BNBUSDT, DOTUSDT, SOLUSDT, TRXUSDT, XRPUSDT. BTCUSDT is indicator-only.

## Fixes Applied

| Area | Problem | Fix |
|---|---|---|
| Round 1 handoff | Missing/renamed evidence files could be missed if old handoff is read alone. | Added correction notice pointing to the Round1-6 audit. |
| Round 2 handoff | Registry claimed 8 entries; actual registry has 11. Live parity was overstated. | Added correction notice and canonical status entry. |
| Round 3 handoff | Registry claimed 8 entries; actual registry has 7. P2/P5 were not run. | Added correction notice and canonical status entry. |
| Round 4 final reports | "Final/structural impossibility" language was Round4-only and later superseded. | Added correction notices to all Round4 final handoff/report files. |
| Round 5 handoff | First ann >50 candidate had DD too high; live parity and ANKR low-DD evidence gaps remained. | Added correction notice and canonical status entry. |
| Round 6 handoff | Registry claimed 8 entries; actual registry has 13. Several tasks were partial or config-only. | Added correction notice and canonical status entry. |
| Round 7 handoff | Handoff was outdated after repair commits; ANKR label could be misread as single-symbol. | Added correction notice; canonical status says multi-symbol, backtest-valid, not live-ready. |
| Round 8 handoff | P2 allocator had timing leakage, incomplete segment validation, wrong registry/count claims. | Added correction notice; Round 9 repaired the allocator evidence. |
| Round 9 artifacts | Best allocator was incorrectly marked `live_ready=true`; handoff head commit was stale; P3 still showed initial timeout wording. | Corrected `r9-final-validation.json`, `r9-P5-winner.json`, `r9-expanded-allocator-grid.json`, `r9-live-allocator-parity.json`, handoff, and ledger. |

## Validity Rules For GLM

Use the following as the authority order:

1. `docs/superpowers/artifacts/glm-martingale-core-round9/r1-r9-corrected-status.json`
2. `docs/superpowers/reports/2026-07-07-glm-round9-execution-audit-and-fix.md`
3. Round-specific audit reports:
   - `docs/superpowers/reports/2026-07-03-glm-round1-6-execution-audit.md`
   - `docs/superpowers/reports/2026-07-07-glm-round7-execution-audit-and-recheck.md`
   - `docs/superpowers/reports/2026-07-07-glm-round8-execution-audit-and-recheck.md`
4. Original handoffs only after applying their top-level correction notice.

## Remaining Gaps To Fix In Round 10

1. Wire `AllocatorState` into `trading-engine main.rs` per-tick dispatch before any allocator can be called fully live-ready.
2. Add integration tests proving inactive sleeves cannot open new cycles while existing cycles are not force-closed.
3. Run exact live/backtest parity replay for `r9-P5-winner` after wiring.
4. Explore fundamentally different martingale sleeve architectures; do not repeat R4-style random symbol substitution, DCA reserve-only, or Round8 leaky allocator timing.

## Corrected Conclusion

The first nine rounds are now usable as a corrected research ledger. The best research result is strong but not a production target hit. The only fully live-ready candidate is still too low-return. Round 10 must first close live allocator wiring, then search new martingale-native sleeve architectures under the unchanged 5000U, multi-symbol, anti-overfit, drawdown-controlled constraints.
