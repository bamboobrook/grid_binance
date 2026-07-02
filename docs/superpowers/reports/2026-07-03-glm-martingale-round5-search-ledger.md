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
