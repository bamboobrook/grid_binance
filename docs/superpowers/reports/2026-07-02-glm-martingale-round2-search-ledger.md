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

## r2-G-symhealth-001 (Direction G proxy: symbol health quarantine)
- long-only-3bull (drop shorts): ann 16%, DD 48.1% (36 trades, strict gate rarely fires; DD WORSE). REJECTED.

## r2-E-inventory-skew-001 (Direction E proxy: asymmetric weights)
- skew20/2: ann 45%, DD 48.6% (h1-overfit); skew18/3: ann 22.5%/DD 25.7% (≈008); skew15/5: =008. REJECTED — no segment-stable improvement.

## r2-H-funding-window-001 (Direction H proxy)
- Funding rates tiny (0.01-0.011% avg); not the 2025 loss driver. REJECTED (marginal, no engine trigger).

## Directions requiring engine features (not run as fast proxy)
- Direction B (Conditional SO): needs safety-order trigger condition in engine. Current SO triggers on price deviation only.
- Direction C (Bounded DGT reset): needs reset-center logic in engine.
- Direction D (Futures sentiment): needs OI/longshort/taker historical data (may not exist for full period).
- Direction F (Recovery re-entry): needs equity-reclaim re-entry logic in engine (current is calendar cooldown).
- Direction A (true Partial TP): needs Rust engine partial-close feature (current TP does full reset_cycle).
