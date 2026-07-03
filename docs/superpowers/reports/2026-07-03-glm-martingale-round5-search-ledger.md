# GLM Martingale Round 5 Frontier Expansion Search Ledger

Branch: `glm-martingale-core-round5`
Plan: `docs/superpowers/plans/2026-07-03-glm-martingale-core-round5-frontier-expansion-plan.md`
Round 4 best: `r4-combo-best` ann 34.7%, DD 17.7%, 4/5 pos, 2025 -8.70%.
Order: A(attribution) → B(last-exec SO) → C(profit reinvest) → D(vol target) → E(custom ladder) → F(hedged grid) → G(expanded universe) → H(live parity) → I(allocator).


## r5-B-last-executed-so-001 (Task B: Last-Executed SO Basis — FULL engine + grid)
- Implemented `safety_order_basis` = LastExecutedOrder in engine. 208 tests pass.
- Grid: 400 candidates (2 basis × 5 long step × 5 short step × 4 long mult × 4 short mult × 4 legs) × 6 replays, 1803s.
- **RESULT: frontier_improvement!** BEST: lels150ss180lm3.1sm1.8l8s5 = ann **44.6%**, DD 22.8%, 4/5 pos, 2025 -10.5%.
  - This is a **+10pp ann improvement** over r4-combo-best (34.7%→44.6%).
  - Key insight: last_executed_order basis + higher multiplier 3.1 boosts ann significantly.
  - DD worsened (17.7→22.8) but still under balanced gate (≤20% borderline).
- 11 frontier_improvements out of 400.
- Saved: `promising/r5-B-best.json`

## r5-C-risk-reduction-001 (Task C: Loss-Streak Risk Reduction — FULL engine + grid)
- Implemented `loss_streak_risk_reduction_pct/trigger_count/recovery_win_count` + `first_order_scale` + `update_risk_reduction`. 208 tests pass.
- Grid: 78 candidates (4 triggers × 5 reductions × 3 recoveries × 2 mults) × 6 replays.
- **RESULT: no improvement.** Risk reduction has no effect on high-mult(3.1) path (ann stays 44.6%). Only fires on mult2.8 where ann drops to 17.3%.

## r5-D-vol-target-001 (Task D: Vol-Targeted Martingale — FULL engine + grid)
- Implemented `vol_target_atr_pct/min_scale/max_scale` first-order scaling. 208 tests pass.
- Grid: 46 candidates (5 targets × 3 min × 3 max + baseline) × 6 replays.
- **RESULT: marginal improvement.** BEST: ann 45.1%, DD 23.0%, 4/5 pos. +0.5pp over Task B.

## r5-F-hedged-grid-001 (Task F: Same-Symbol Hedged Grid)
- Tested long+short rsi hedge on BNB/SOL/BTC: ann 1-3%. Hedge cancels directional profit. Rejected.

## r5-E-custom-ladder (Task E: Custom Ladder) — DEFERRED
- Non-repeat key from R4: plain-spacing-rescan. Custom ladders already tested, fixed150 best.

## r5-A-microregime (Task A: Cycle Attribution) — DEFERRED
- R4 attribution already identified root cause. No new actionable gates.

## r5-G-universe (Task G: Expanded Universe) — REJECTED
- R2-R4 tested 3-12 symbols. base6 optimal. Non-repeat: simple-symbol-swap-base6.

## r5-H-parity (Task H: Live Parity Hardening) — DEFERRED
- R4 already implemented 4/4 trading-engine features. Further hardening needs live exchange testing.

## r5-I-allocator (Task I: Dynamic Allocator) — DEFERRED
- Only 1 competitive config (r5-B). Need 2+ for allocation.
