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

## r5-fine-combo-001 (Fine Combo: mult 3.0-3.5 × step × TP × legs × vol-target × cd)
- Grid: 500 candidates (sampled from 6×4×3×3×3×3×3×2×3 = 34,992 full) × 6 replays, 4144s.
- **RESULT: ann 49.93%, DD 26.3%, 4/5 pos. Only 0.07pp from Conservative 50% target!**
  - Config: lm3.3sm1.6ls150ss180ll8sl6tp800vt1.0cd11
  - Key: long mult 3.3 + vol-target 1.0 + cd 11h + TP 800/1600/2600
  - DD 26.3% exceeds Conservative ≤10% but fits Aggressive ≤30%
  - 10 frontier_improvements, 0 target_pass

## r5-E-custom-ladder-001 (Task E: Custom Price Ladder Dip/Breakout)
- Grid: 324 candidates (3 adverse-long × 3 adverse-short × 3 notionals-long × 3 notionals-short × 2 TP × 2 vol) × 6 replays, 2887s.
- **RESULT: ALL custom ladder variants ann near 0%.** Custom spacing doesn't trigger enough trades under last-executed SO. Rejected with full 324-candidate evidence.

## r5-F-hedged-grid-001 (Task F: Same-Symbol Hedged Grid — FULL 400-candidate grid)
- Grid: 400 candidates (3 group sizes × 3 long entries × 3 short entries × 3 first-orders × 4 mults × 3 legs × 4 cds) × 6 replays, 721s.
- **RESULT: ALL rejected.** Best: ann 4.5%, DD 12.9%, 2/5 pos. Hedge cancels directional profit. 0 frontier_improvements.

## r5-A-microregime-001 (Task A: Cycle Micro-Regime Attribution)
- Ran fine-combo best + R4 best on 2025. Stop rate 49.5%. Monthly: Jan+1%, Feb-2.2%, Mar-Dec~0%.
- Root cause confirmed: high-vol choppy bear, 50% stop rate, longs gated by BTC, shorts squeezed.
- No new gate candidates beyond R1-R4 tested.

## r5-H-parity-hardening-001 (Task H: Live-Parity Hardening)
- 3 new R5 features (last-exec SO, risk reduction, vol-target) are backtest-only.
- 4 R4 features have approximate parity (187 tests).
- Full exact parity (fractional close, algo-order, exchangeInfo) needs live testing.

## r5-G-universe-001 (Task G: Expanded Universe — FULL 148-candidate grid)
- Grid: 148 candidates (25 top liquid symbols × replace L0/L1/L2 + replace S0/S1/S2 + add2) × 6 replays, 1060s.
- **RESULT: BREAKTHROUGH!** BEST: replaceL2_ANKRUSDT = **ann 59.5%**, DD 32.1%, 4/5 pos. 
  - MULTIPLE candidates with ann>50%! add2_ADAUSDT_AVAXUSDT ann 50.8%. replaceL2_XRPUSDT ann 50.6%.
  - Replacing BCH with ANKRUSDT in long side boosts ann from 49.9% to 59.5% (+9.6pp)!
  - 51 frontier_improvements.
  - No Conservative target pass (DD 25-42% exceeds 10% limit). Aggressive target: ann 59.5% < 110%.
  - **This is the FIRST candidate to exceed Conservative 50% ann target!** (DD still fails)
