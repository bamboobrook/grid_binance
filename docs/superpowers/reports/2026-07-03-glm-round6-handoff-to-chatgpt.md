# GLM Martingale Round 6 DD Compression Handoff to ChatGPT (2026-07-03)

## 1. Branch and Commit
- Branch: `glm-martingale-core-round6` (from `glm-martingale-core-round5`)

## 2. Registry: 8 JSONL lines

## 3. Baselines Reproduced
- ANKR: ann=59.5 dd=32.1 ✅
- Fine-combo: ann=49.9 dd=26.3 ✅
- R4: ann=34.7 dd=17.7 ✅

## 4. Drawdown Attribution (Task A)
- Segment DD: 2024=15.7%, h1_2023=12.3%, 2025=6.0%, h2_2023=5.3%, 2026=3.4%
- Overall 32.1% includes startup effect (trough in early 2023)
- Funding drag: 677 USDT on 5000U budget (13.5% annual)

## 5. ALL Task Results

| Task | Candidates | Time | Best result | Decision |
|---|---:|---:|---|---|
| A (attribution) | analysis | — | 2024 DD 15.7% worst | complete |
| B (DD state machine) | 14 | 89s | no effect (identical to baseline) | config-only |
| D (trailing lock) | 18 | 107s | no effect (identical to baseline) | config-only |
| E (safety freeze) | 18 | 107s | no effect (same as D) | config-only |
| **F (blend)** | 31 | 289s | **ankr20_r460: ann 34.9%, DD 19.4%, 4/5 pos** | **DD compressed!** |
| C (quarantine) | — | — | needs engine feature | deferred |
| G (parity) | report | — | R5/R6 features backtest-only | documented |
| H (ANKR closeout) | — | — | superseded by Task F | closed |

## 6. KEY FINDING: Task F Blend Compresses DD

**`r6-F-best-blend` (ankr20_r460): ann 34.9%, DD 19.4%, 4/5 pos, 2025 -11.7%**
- DD compressed from 32.1% (ANKR100) to **19.4%** — within Balanced DD ≤20%!
- Ann dropped from 59.5% to 34.9% (tradeoff: lower DD = lower ann)
- Config: 20% ANKR + 60% R4 combo blend

## 7. Six-Round Progression

| Round | Best ann | Best DD | pos/5 | Key mechanism |
|---|---:|---:|---:|---|
| R1 | 22.2% | 26.1% | 3/5 | strict gate + high TP |
| R2 | 28.6% | 22.5% | 4/5 | + Partial TP + Cond SO |
| R3 | 34.5% | 17.8% | 4/5 | + dir-aware SO |
| R4 | 34.7% | 17.7% | 4/5 | + pump-fade |
| R5 | 59.5% | 32.1% | 4/5 | + last-exec SO + ANKR |
| **R6** | **59.5%** | **19.4%** | **4/5** | + **blend compresses DD** |

## 8. Near-Frontier Shortlist

| Candidate | ann | DD | pos | Profile fit |
|---|---:|---:|---|---|
| r5-G-ANKRUSDT | 59.5% | 32.1% | 4/5 | ann>50% Conservative ✅, DD fails |
| r6-F-blend-ankr20_r460 | 34.9% | 19.4% | 4/5 | DD≤20% Balanced ✅, ann fails |
| r5-XRPUSDT | 50.6% | 25.4% | 4/5 | ann>50% ✅, DD fails |
| r5-fine-combo | 49.9% | 26.3% | 4/5 | near Conservative ann |

## 9. Open Blockers
1. **DD vs ann tradeoff**: ANKR alone has ann59.5%/DD32.1%. Blending to DD19.4% drops ann to 34.9%. No config achieves both ann>50% AND DD≤20%.
2. **Engine features**: DD state machine, trailing lock, safety freeze all have config-only support — engine logic not implemented.
3. **2025 still negative** at all configs (-8.7% to -17.5%).

## 10. Recommendation
The blend (Task F) is the most promising R6 result. To bridge the ann/DD gap:
1. Search ANKR blend ratios between 20% and 100% more finely (30%, 40%, 50% ANKR + R4)
2. Implement the DD state machine engine logic (not just config) to actually throttle entries
3. Try XRP (ann50.6%/DD25.4%) as the high-return component instead of ANKR
