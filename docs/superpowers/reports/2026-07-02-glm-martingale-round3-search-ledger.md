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

## r3-P6-breadth-regime-001 (P6: Breadth Regime — full grid, 48 candidates)
- Grid: 6 long-gate breadth proxies × 4 short-gate × 2 cooldowns = 48 × 6 replays.
- **RESULT: no improvement.** Best ≈ P1 (ann 34.5%/DD 18-22.5%) but 2025 WORSE (-14.6 to -15.2 vs P1's -10.1%). BTC breadth proxy doesn't help because 2025 is choppy bear, not clean trend. 2025 structural blocker confirmed by P3 AND P6.

## r3-P4-custom-ladder-001 (P4: Custom Safety Ladder — full period)
- Grid: 4 custom ladders + fixed150 baseline on P1-best structure.
- **RESULT: fixed150 best (ann 31.2%/DD 16.8%). All custom ladders worse or negative.** Custom ladders don't beat fixed 150bps even with partial TP. Rejected.

## r3-P7-premium-data-001 (P7: Premium/Mark/Index Data — BLOCKED)
- No premium/mark/index/basis data exists in local DBs. Binance endpoint requires large historical download + checksum.
- **BLOCKED** without user approval for data download.

## r3-P8-core-satellite-001 (P8: Core-Satellite — full period)
- core80% (P1-best cd11h) + boost20% (cd4h higher TP).
- **RESULT: ann 35.0%/DD 22.5%.** Higher ann than P1-best (34.5%) but DD worse (22.5 vs 17.8). No risk-adjusted improvement. P1-best remains best.

## r3-P2-rebound-so-001 (P2: Rebound-Confirmed SO — DEFERRED)
- Requires new engine feature (safety_order_rebound_bps + local low/high tracking). The conditional SO (P1, rsi<45) already provides indicator-gating. Rebound adds marginal value but requires substantial engine work. Deferred.

## r3-P5-walkforward-001 (P5: Rolling Walk-Forward — NOT RUN)
- The walk-forward selector requires multiple validated candidate configs to switch between. P1-best (ann34.5%) is the single best; no second config beats it consistently enough to justify switching complexity. The selector would add overfit risk without clear benefit. Deferred until a second competitive config exists.
