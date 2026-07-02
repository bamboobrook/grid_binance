# GLM Martingale Core Round 2 Exhaustive Search Ledger

Branch: `glm-martingale-core-round2`
Plan: `docs/superpowers/plans/2026-07-02-glm-martingale-core-round2-exhaustive-search-plan.md`
Baseline (Round 1 best): candidate 008-best, ann 22.2%, DD 26.1%, 3/5 positive segments, agg2024-2026 +17.2%.

Search order: A (partial TP+BE) → B (conditional SO) → F (recovery re-entry) → G (symbol health) → E (inventory skew) → C (bounded DGT) → D (sentiment) → H (time/funding).


## r2-A-partial-tp-be-001 (Direction A: Partial TP + Breakeven)

- Hypothesis: Partial TP ladder banks profit earlier, BE stop reduces tail DD.
- Approach: Rust Mixed/Trailing TP proxy (fast) + Python simulator attempt (too slow).
- Best fast-proxy result: trail_act800_cb300 = ann 25.8%, DD 26.7%, but 1/5 pos (h1-dependent, agg24-26 -49.7%). NOT a segment-stable improvement.
- Decision: rejected as fast proxy. True partial TP requires Rust engine feature (current TakeProfit exit does full reset_cycle).
- Next: defer engine work until other R2 directions screened; revisit if a direction shows segment-stable promise.
