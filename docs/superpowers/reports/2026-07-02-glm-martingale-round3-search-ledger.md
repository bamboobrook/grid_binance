# GLM Martingale Round 3 Target Breakthrough Search Ledger

Branch: `glm-martingale-core-round3`
Plan: `docs/superpowers/plans/2026-07-02-glm-martingale-core-round3-target-breakthrough-plan.md`
Round 2 best: `r2-H-best-cd12h` ann 28.6%, DD 22.5%, 4/5 pos, agg24-26 +22.8%, h1c 39%.
Order: P0 (parity audit) → P1 (dir-aware SO) → P2 (rebound SO) → P3 (TP/cooldown fine) → P4 (custom ladder) → P5 (walkforward) → P6 (breadth regime) → P7 (premium data) → P8 (core-satellite).


## r3-P1-dir-aware-so-001 (Direction-Aware Safety Order — FULL grid, 352 candidates)

- Hypothesis: Direction-specific SO (long rsi<45, short rsi>55/none) + cooldown fine-scan.
- Grid: 2 TP kinds × 5 cooldowns (10-14h) × 6 long SO × 6 short SO = 352 candidates × 6 replays, 3882s.
- **NEW BEST: `sym_8-cd11-LrsiSnone` = ann 34.5%, DD 17.8%, 4/5 pos, agg +29.2%, h1c 43.7%**
  - Segments: h1_2023 +25.9%, h2_2023 +4.2%, 2024 +32.1%, 2025 -10.1%, 2026_ytd +7.2%
  - Beats r2-H-best-cd12h on ALL metrics: ann 34.5 vs 28.6, DD 17.8 vs 22.5, agg +29.2 vs +22.8, h1c 43.7 vs 39
  - DD 17.8% is within the Balanced DD gate (<=20%). 4/5 pos meets Conservative/Balanced segment gate.
- 60 frontier_improvements, 0 near_target (no candidate hit 50% ann yet).
- Key: 11h cooldown (not 12h) is the sweet spot. Long RSI<45 SO + short no-condition.
- Saved: `promising/r3-P1-best-cd11.json`
