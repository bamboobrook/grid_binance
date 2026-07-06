# GLM Martingale Round 7 Cost-Aware Gap Repair Handoff to ChatGPT

## 1. Branch and Commit
- Branch: `glm-martingale-core-round7`

## 2. Registry: 10 JSONL lines

## 3. Round 1-6 Gaps Repaired
- Task A: 29 do_not_repeat entries merged from R1-R6 registries
- Task B: 6 candidates fully attributed (funding 8-17% of budget, segment DD mapped)

## 4. Task B Attribution Summary
| candidate | ann | DD | funding%budget | routes |
|---|---:|---:|---:|---|
| R5-ANKR | 59.5% | 32.1% | 13.5% | C,G |
| R6-XRPQ | 55.8% | 25.0% | 17.1% | C,F |
| R5-fine | 49.9% | 26.3% | 14.1% | C,F |
| R4-combo | 34.7% | 17.7% | 8.1% | C,F,G |
| R3-lowdd | 34.5% | 17.8% | 7.9% | C,F |
| R6-QB | 34.0% | 18.0% | 8.2% | C,F |

## 5. Task C-G Results

| Task | Candidates | Time | Best | Decision |
|---|---:|---:|---|---|
| C (cost gate) | 176 | 971s | config-only (no effect) | deferred |
| D (turnover) | — | — | depends on C | deferred |
| E (quarantine) | — | — | R6 already done | deferred |
| F (safety freeze) | — | — | config-only | deferred |
| **G (dynamic blend)** | **324** | **2s** | **ann 53.9%, DD 24.3%** | **frontier improvement** |

## 6. Task H: Live-Parity Status
- R5/R6/R7 features all backtest-only (last-exec SO, vol-target, quarantine, DD state machine, trailing lock, cost gate, safety taper)
- Trading-engine has 4/4 R4 parity features (187 tests)

## 7. Original Target Status
| Target | Status |
|---|---|
| Conservative ann>50% | ✅ (ANKR 59.5%, blend 53.9%) |
| Conservative DD≤10% | ❌ (best 17.7%) |
| Balanced ann>90% | ❌ |
| Balanced DD≤20% | ✅ (QB blend 18.0%) |
| Aggressive ann>110% | ❌ |
| Aggressive DD≤30% | ✅ (ANKR 32.1% borderline, XRPQ 25.0%) |

**No candidate passes ALL conditions of any single target.**

## 8. Seven-Round Final Frontier

| Round | Best ann | Best DD | Key mechanism |
|---|---:|---:|---|
| R1 | 22.2% | 26.1% | strict gate + high TP |
| R2 | 28.6% | 22.5% | + Partial TP + Cond SO |
| R3 | 34.5% | 17.8% | + dir-aware SO |
| R4 | 34.7% | 17.7% | + pump-fade |
| R5 | 59.5% | 32.1% | + last-exec SO + ANKR |
| R6 | 59.5% | 18.0% | + blend + quarantine |
| **R7** | **59.5%** | **18.0%** | + dynamic blend (ann53.9%/DD24.3%) |

## 9. Near-Frontier Shortlist
| Candidate | ann | DD | pos | Config |
|---|---:|---:|---:|---|
| R5-ANKR | 59.5% | 32.1% | 4/5 | ANKR + last-exec SO |
| R7-G blend | 53.9% | 24.3% | — | 30d lagged allocator |
| R6-XRPQ | 55.8% | 25.0% | 4/5 | XRP + quarantine |
| R6-QB | 34.0% | 18.0% | 4/5 | XRPq20 + R4 blend |

## 10. Open Blockers
1. ann>50% AND DD≤20% not achievable simultaneously (structural cliff)
2. Engine logic for R5/R6/R7 features (cost gate, safety taper, DD state machine) not fully implemented
3. 2025 remains negative at all configs (-8.7% to -17.5%)
