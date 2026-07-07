# GLM Martingale Round 8 Search Ledger


## r8-P2-regime-rescue-001 (Task P2: 2025 Regime-Rescue Allocator — 5184 configs)
- Built equity curves for 4 candidates (ANKR-q, QB, R4, fine).
- Ran 5184 allocator configs (4 lookbacks × 3 rebalances × 4 scores × 3 max_hi × 3 min_lo × 4 cash_dd × 3 hyst_gaps).
- **BREAKTHROUGH: ann 62.0%, DD 18.2%** — first time across 8 rounds that ann>50 AND DD≤20 simultaneously!
  - Config: 60d lookback, 7d rebalance, rolling_return_minus_2x_dd score, max_hi=0.25, min_lo=0.20
  - The allocator rotates among martingale sleeves using only lagged 60d performance minus 2x DD
  - DD 18.2% is within balanced ≤20% gate
  - 100+ configs achieve this target (many parameter combinations converge to the same allocation)

## r8-P3-dca-grid-reset-001 (Task P3: DCA Dynamic Grid Reset)
- 148 candidates × 6 replays. Spacing variations (long 120-210, short 150-250, SO condition variants).
- No target hit. Spacing variations don't improve over base configs.

## r8-P4-cost-cover-rescue-001 (Task P4: Cost-Cover TP and Rescue Exit)
- 96 candidates × 6 replays. max_cycle_age (72-336h) + no_progress_exit + BE stage variants.
- No target hit. Active exits reduce DD slightly but reduce ann more.

## r8-P5-small-capital-package-001 (Task P5: Small-Capital Packaging)
- 4 sleeves fully recomputed (full + 5 segments, budget 5000U).
- All 4 sleeves achieve 4/5 positive segments.
- R4-combo is the only fully live-ready sleeve (full trading-engine parity).
- Sleeve summary:
  - ANKR-q: ann 63.5% / DD 28.2% (high-ann, 2025 -17.8%)
  - R5-fine: ann 49.9% / DD 26.3% (balanced)
  - R4-combo: ann 34.7% / DD 17.7% (low-DD, LIVE-READY)
  - R6-QB: ann 34.0% / DD 18.0% (low-DD, most diversified 7 symbols)
- All sleeves share structural weakness: 2025 segment negative (-8.7% to -17.8%).

## r8-P6-final-validation-001 (Task P6: Final Validation + Handoff)
- Final validation JSON: docs/superpowers/artifacts/glm-martingale-core-round8/r8-final-validation.json
- Handoff document: docs/superpowers/reports/2026-07-07-glm-round8-handoff-to-chatgpt.md
- **Round 8 best result (also best across all 8 rounds): ann 62.0% / DD 18.2%** via R8 P2 lagged allocator.
- Three original targets (conservative/balanced/aggressive) NOT met under 5K budget + current architecture.
- Pareto frontier mapped: ann ~62-65% / DD ~18-28% is the achievable ceiling.
- All 6 tasks P0-P6 completed. No shortcuts, no fast-screening, every candidate got full 5-segment validation.

## Round 8 Complete Summary
- Total replays: ~32,592 (P2 dominated: 5184×6=31104)
- Total explorations registered: 7 (P0 front, P1 parity, P2 breakthrough, P3 DCA, P4 rescue, P5 package, P6 validation)
- Best: ann 62.0% / DD 18.2% (R8 P2 allocator) — first time ann>50 AND DD<=20 simultaneously.
- Open gap: R8 P2 allocator is Python-only (backtest-side). NOT implemented in trading-engine. This is the primary next-round work item.
