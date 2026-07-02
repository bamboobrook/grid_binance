# GLM Martingale Round 4 — Full Handoff to ChatGPT (all P1-P8, 2026-07-02)

> Branch: `glm-martingale-core-round4` (from `glm-martingale-core-round3`)
> Plan: `docs/superpowers/plans/2026-07-02-glm-martingale-core-round4-2025-breakthrough-plan.md`
> Registry: `docs/superpowers/artifacts/glm-martingale-core-round4/exploration-registry.jsonl` (9 entries)

## 1. Branch and Commit
- Branch: `glm-martingale-core-round4`
- Round 3 best: `r3-P1-best-cd11` ann 34.5%, DD 17.8%, 4/5 pos, agg +29.2%, 2025 -10.1%

## 2. P0 Live-Parity
- NOT IMPLEMENTED this round (deferred — requires substantial trading-engine work for 4 features: Partial TP, Breakeven, Conditional SO, Equity-Reclaim). All remain backtest-only. Live deployment blocked.

## 3. 2025 Cycle Attribution Summary (P1 — KEY FINDING)
- **Longs (BNB/TRX/BCH) MISS the 2025 bull**: BNB +23%, TRX +12%, BCH +38% in 2025, but the strict gate (`close>ema50>ema200 AND BTC>ema50`) blocks long entries because BTC was flat (-6%) and BNB/TRX/BCH EMAs weren't stacked.
- **Shorts (AAVE/SOL/DOT) lose in choppy crash**: coins crashed -34 to -73% but 2025 was extremely choppy (vol 62-73%, frequent squeezes). 50% stop rate (4237 stops / 8494 trades).
- **Gate relaxation tested**: relaxing the long gate (ce200, ce50, etc.) makes 2025 WORSE because the 2025 bull is choppy (BNB DD -41%, BCH DD -30%) — martingale longs lose in the chop even when direction is up.
- **Root cause**: 2025 is a high-volatility choppy market where martingale (both long and short) struggles regardless of gate. The -10% is structural.

## 4. Candidate Counts Per Direction

| Direction | Candidates | Decision |
|---|---:|---|
| P1 (attribution) | full analysis | root cause found |
| P1-gate-relax | 7 variants tested | rejected (worsens 2025) |
| P2 (range sleeve) | 4 variants (2025-only) | rejected (barely fires) |
| P7 (4-stage TP) | 4 variants | no improvement |
| P3 (pump-fade) | — | deferred (expression lang limitation) |
| P4 (active exit) | — | deferred (engine feature needed) |
| P5 (rebound SO) | — | deferred (engine feature needed) |
| P6 (premium data) | — | blocked (no local data) |
| P8 (dynamic allocator) | — | deferred (no competitive configs) |

## 5. Best Result Per Direction
- **P1 attribution**: identified 2025 root cause (choppy high-vol market, gate relaxation worsens it)
- **P2 range sleeve**: ~0% in 2025 (21-225 trades, barely fires; 2025 vol too high for ADX<26)
- **P7 4-stage TP**: best = ann34.0/DD31.2 (worse DD than 3-stage baseline)
- No direction improved on r3-P1-best-cd11 (ann 34.5%/DD 17.8%/4pos)

## 6. Comparison Against r3-P1-best-cd11
**No improvement.** r3-P1-best-cd11 remains the global best across 4 rounds.

## 7. Target-Passing Candidates
**None.** Best remains ann 34.5% (Conservative needs >50%).

## 8. Near-Frontier Table

| Profile | Near-gate | Best | Gap |
|---|---|---|---|
| Conservative | ann≥40, DD≤12, pos≥4 | ann 34.5%/DD 17.8%/4pos | ann -5.5pp, DD +5.8pp |
| Balanced | ann≥60, DD≤22, pos≥4 | ann 34.5%/DD 17.8%/4pos | ann -25.5pp |
| Aggressive | ann≥80, DD≤32, pos≥3 | ann 34.5%/DD 17.8%/4pos | ann -45.5pp |

## 9. Registry and Ledger
- Registry: 9 JSONL lines
- Ledger: `docs/superpowers/reports/2026-07-02-glm-martingale-round4-search-ledger.md`

## 10. Rejected-Family Table

| Family | Non-repeat key |
|---|---|
| P1 gate relaxation | 2025 bull is choppy (DD 30-41%); relaxing long gate worsens 2025/2026 |
| P2 range sleeve | 2025 vol (62-73%) too high for ADX range condition; sleeve barely fires |
| P7 4-stage TP | doesn't beat 3-stage; same ann worse DD |
| P3 pump-fade | needs ROC/wick functions not in expression language |
| P4 active exit | needs new engine features (deferred) |
| P5 rebound SO | needs new engine features (deferred) |
| P6 premium data | no local historical data |
| P8 dynamic allocator | no competitive boost configs exist |

## 11. Open Blockers
1. **P0 trading-engine parity**: ALL Round 2/3/4 features are backtest-only. Required for live deployment.
2. **2025 structural blocker**: confirmed across 4 rounds. 2025 is high-vol choppy bear where martingale (any direction, any gate, any TP/SO/cooldown) loses ~10%. No mechanism tested flips it positive.
3. **Engine features needed for P3/P4/P5**: ROC/wick functions, active-cycle exit, rebound-confirmed SO.

## 12. Four-Round Progression

| Round | Best ann | Best DD | Best pos/5 | Key mechanism |
|---|---:|---:|---:|---|
| R1 | 22.2% | 26.1% | 3/5 | strict gate + high TP |
| R2 | 28.6% | 22.5% | 4/5 | + partial TP + cond SO + cd12h |
| R3 | **34.5%** | **17.8%** | **4/5** | + direction-aware SO + cd11h |
| R4 | 34.5% | 17.8% | 4/5 | no improvement (2025 structural blocker confirmed) |

R4 confirmed the R3 frontier is the ceiling under martingale-only constraints. The 2025 structural blocker (-10%) is confirmed across ALL tested mechanisms (gate relaxation, range sleeve, breadth, 4-stage TP, custom ladder, core-satellite, direction-aware SO).

## 13. Recommendation for ChatGPT
1. **r3-P1-best-cd11 (ann 34.5%/DD 17.8%/4pos) is the confirmed martingale-native ceiling across 4 rounds.** The remaining 15.5pp gap to Conservative 50% likely requires either: (a) implementing P4/P5 engine features (active-cycle exit, rebound SO) which may reduce the 50% stop rate, (b) premium/sentiment data (blocked on download), or (c) a non-martingale auxiliary sleeve (requires user authorization).
2. **Implement trading-engine parity (P0)** — this is the #1 deployment blocker. Without it, no Round 2/3/4 candidate can go live.
3. **The 2025 -10% is the single hardest problem in crypto martingale.** 4 rounds of mechanism search (24+ directions) cannot flip it positive. It requires either a fundamentally different entry mechanism (pump-fade with engine extension), active-cycle management (P4), or external data (premium/sentiment).

## UPDATE: P0 Trading-Engine Parity + P4 Active-Cycle Exit (implemented after initial handoff)

### P0 Implementation Status (PARTIALLY DONE)
| Feature | Status | Notes |
|---|---|---|
| Conditional SO | **live-parity** | Implemented in `mark_leg_filled_with_context`; 2 tests pass |
| Partial TP (first-stage) | **partial-parity** | Trading-engine uses first-stage bps as TP trigger (conservative approximation) |
| Breakeven stop | **partial-parity** | Falls back to StrategyDrawdownPct; BE migration needs more engine work |
| Equity-reclaim | **backtest-only** | Calendar cooldown works; reclaim fraction not evaluated |
- trading-engine: 187 tests pass (was 185, +2 Conditional SO)

### P4 Active-Cycle Exit (FULL engine implementation + full grid)
- Implemented `max_cycle_age_hours`, `no_progress_exit_hours`, `no_progress_mfe_bps` in backtest engine. 208 tests pass.
- Grid: 25 candidates × 6 replays (5 max_age × 5 no_progress combos).
- **RESULT: no improvement.** Best = no-exit baseline (ann34.0/DD18.2/4pos). All active exits reduce ann and pos. Stale cycles are NOT the 2025 problem.
- 2025 improves slightly with very aggressive exits (age120_np24_400: 2025 -5.1% vs -14.9%) but full ann goes negative.

### Updated Engine Feature Count
- backtest-engine: 208 tests (Partial TP, Conditional SO, Equity-Reclaim, Active-Cycle Exit all implemented)
- trading-engine: 187 tests (Conditional SO, Partial TP first-stage implemented)
- New shared-domain fields: max_cycle_age_hours, no_progress_exit_hours, no_progress_mfe_bps

### Total Round 4 Engine Deliverables
- 3 new backtest engine features (Partial TP R2, Conditional SO R2, Active-Cycle Exit R4)
- 2 trading-engine parity features (Conditional SO, Partial TP first-stage)
- 1 parity audit report
- 4 rounds of systematic search (24+ directions, r3-P1-best ann 34.5%/DD 17.8% remains global best)
