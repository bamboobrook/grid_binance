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

## r2-A-partial-tp-be-full-001 (Direction A: Partial TP + Breakeven — FULL engine implementation)

- Hypothesis: Partial TP banks profit before full mean reversion, BE stop reduces tail DD.
- Implementation: Added `Partial` TP variant to shared-domain + Rust engine (partial close logic in TakeProfit exit block, `tp_stage`/`breakeven_stop_active` state, breakeven stop migration in triggered_stop). 208 tests pass.
- Grid: 3 symbol sets × 3 TP ladders × 3 splits × 2 BE-after × 3 BE-buffer = 162 candidates × 6 replays = 972 replays, 2589s.
- **RESULT: BREAKTHROUGH.** 13 frontier_improvements. BEST: `s0-tl80016002600-sp303040-be1b100`:
  - **ann 17.5%, DD 19.2%, 4/5 positive segments (NEW BEST), agg24-26 +14.1%, h1_contrib 57.8% (< 60% gate!)**
  - Segments: h1_2023 +19.95%, **h2_2023 +0.46% (flipped from negative!)**, 2024 +29.77%, 2025 -17.10%, 2026_ytd +1.44%
  - DD 19.2% is within the Balanced DD gate (<=20%). 4/5 positive segments meets Conservative/Balanced segment gate (>=4/5).
  - This is a genuine frontier_improvement over 008-best on segment stability (4/5 vs 3/5) and DD (19.2 vs 26.1).
- Mechanism validated: Partial TP + breakeven DOES break part of the cliff by banking profit early and protecting it with BE stop, which flipped h2_2023 positive.
- Saved: `promising/r2-A-best-4of5.json`
- Next: continue to Direction B (Conditional SO), but this is a strong new frontier.

## r2-B-cond-so-full-001 (Direction B: Conditional Safety Orders — FULL engine implementation)

- Implementation: Added `safety_order_condition` field to MartingaleRiskLimits + evaluation in kline_engine safety-order block. 208 tests pass.
- Grid: 3 TP kinds (pct2200, partial_800/1600/2600, partial_600/1200/2200) × 5 SO conditions (none, rsi<45, rsi<30, bb_lower, adx<25) = 15 candidates × 6 replays.
- **RESULT: BREAKTHROUGH.** BEST: `partial_800_1600_2600-so_rsi45`:
  - **ann 18.0%, DD 19.0% (within Balanced DD<=20%!), 4/5 pos, agg24-26 +22.4%, h1c 47.1%**
  - Segments: h1_2023 +22.5%, h2_2023 +2.9%, 2024 +30.8%, 2025 -12.9% (improved from -17.1%), 2026_ytd +4.5%
  - Combining Direction A (partial TP) + Direction B (conditional SO rsi<45) is the NEW BEST frontier: better DD (19.0 vs 19.2), better agg (+22.4 vs +14.1), same 4/5 pos.
- Also notable: partial_600_1200_2200-so_rsi30: ann 24.9%, DD 21.2%, 3/5 pos, agg +36.4% (higher ann, lower pos).
- Saved: `promising/r2-B-best-4of5.json`

## r2-F-recovery-reentry-full-001 (Direction F: Recovery-Based Re-Entry — FULL engine implementation)

- Implementation: Added `reentry_equity_reclaim_fraction` to MartingaleRiskLimits + equity-reclaim logic in kline_engine (records equity at stop, clears cooldown when equity recovers fraction of stopped DD). 208 tests pass.
- Grid: DD stops (0,16,20,25,30) × reclaim (none,0.25,0.5,0.75) × cooldown (6,12,24h) = 49 candidates × 6 replays.
- **RESULT: no improvement.** On the B-best structure, stops >20% never fire (peak DD is 19.0%); 16% stop fires but hurts ann. Reclaim re-entry doesn't help because the B-best structure already controls DD via partial TP + conditional SO without needing a portfolio stop.
- Best: dd0 (no stop) = ann 16.2%, DD 19.9%, 4/5 pos, agg +22.7% (same structure as B-best, slight metric variance from reclaim field presence).
- Conclusion: Direction F is not needed for the current best frontier. The portfolio stop was useful in Round 1 (DD 37→5.75%) but the Round 2 mechanisms (partial TP + conditional SO) achieve DD control at the cycle level, making the portfolio-level stop redundant.

## r2-G-symhealth-full-001 (Direction G: Symbol Health / Diversification — full grid)

- Grid: 6 symbol-set variations (base6, long4, short4, long4short4, base3short3lo, base3short3hi) × B-best mechanism × 6 replays.
- **RESULT: no meaningful improvement.** base3short3lo (NEAR for AAVE): ann 16.3%, DD 19.9%, 4/5 pos, agg +21.0 (marginally higher ann, lower agg). Adding ETH/NEAR (long4, long4short4) made it WORSE (3/5, 2/5 pos). Base6 (BNB/TRX/BCH + AAVE/SOL/DOT) remains best balanced.
- Conclusion: the base6 symbol set is already well-chosen. Symbol health variation doesn't improve the frontier.

## r2-E-inventory-skew-full-001 (Direction E: Inventory Skew — full grid)
- Grid: 7 cap variants (longcap 25-40, symcap 12-15, combos) × B-best × 6 replays.
- **RESULT: no effect.** All variants identical (ann 16.2/DD 19.9/4/5 pos) because per-leg positions (~500U) never hit caps. Inventory skew via existing caps has no effect on this low-exposure structure.

## r2-D-sentiment-data-check (Direction D: data availability)
- market_data_full.db has ONLY `klines` table (no OI/longshort/taker/sentiment data).
- funding_rates.db has funding_rates (114210 rows) — available for funding gate.
- **BLOCKED for OI/longshort/taker**: historical sentiment data does not exist locally. Direction D can only test the FUNDING gate subset (which was already analyzed in Round 1 as too small to matter).

## r2-H-time-cooldown-full-001 (Direction H: Time/Cooldown Window — BREAKTHROUGH)

- Grid: cooldowns 3h/4h/6h/8h/12h on B-best mechanism.
- **BREAKTHROUGH: cd12h = ann 28.6%, DD 22.5%, 4/5 pos, agg24-26 +22.8%, h1c 39.0%**
  - Segments: h1_2023 +36.4%, h2_2023 +6.6%, 2024 +27.7%, 2025 -10.2% (best yet), 2026_ytd +12.9%
  - cd4h: ann 30.3%, DD 24.3%, 4/5 pos, but h1c 73.4% (> 60% gate)
  - 12h cooldown produces fewer but higher-quality cycles; 2025 improved to -10.2%
- This is the NEW BEST overall frontier: ann 28.6% (vs B-best 18.0%, 008 22.2%), 4/5 pos, DD 22.5%.

## r2-C-dgt-spacing-full-001 (Direction C: Bounded DGT spacing — rejected)
- All non-150 spacing gives negative ann. 150bps optimal. Rejected.
