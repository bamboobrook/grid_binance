# GLM Martingale Core Round 23 — Session 2 Final Handoff (R0→R1→R3→R4-M3→R4-M1)

**Date:** 2026-07-22
**Branch:** `glm-martingale-core-round23`
**Plan:** `docs/superpowers/plans/2026-07-22-glm-martingale-core-round23-orderflow-factor-repair-plan.md`
**Machine state:** `BLOCKED_ENGINE_DATA_OR_EXECUTION`
**Phases completed (registry-detected):** R0, R1, R4 (partial — mechanism attempted, no G1/G2)
**Phases remaining:** R2, R3 (full data), R4-M1 engine wiring, R4-M2, R5, G0, G1, G2, R8

## 0. Honest summary

This session advanced Round 23 across **R1, R3, R4-M3, and R4-M1**. The scored-replay infrastructure (R0+R1) is **complete and proven correct on real data**. Two Martin-family mechanisms were implemented and strictly backtested (R4-M3 basket, R4-M1 signal layer); **neither produced alpha** — an honest negative result. No target was hit and none was claimed. The full plan (15 phases) remains multi-session work.

## 1. What was delivered this session

### R1 — production-conservative scored-replay driver ✅
- `crates/r23-replay`: continuous prequential driver routing every replay through `run_synchronized_cycle_replay` (conservative gates built in at FO/SO emit sites).
- 12 §5.3 directed tests all pass; P-A gate (`gates/r1.json`) `passed: true`.
- **Real-data full continuous backtests** (BTC/ETH, 1065-day window, 8 budgets) — best control: 1000U → 1.25% ann / 5.21% DD. All recorded.
- R22 non-candidate canary: `budget500/m3/fo120` → -4.99% / 12.82% DD, proving the old 31.2%/9.9% frontier is revoked.

### R3 — Binance Vision data infrastructure ✅
- `scripts/r23_download_binance_vision.py`: sidecar-checksummed download + immutable manifest, honors plan §7.2.
- Verified pipeline (165+ files, checksums green, schema parsed). Full-window 8-symbol download launched in background.

### R4-M3 — Johansen-style BTC-factor basket Martin ✅ (negative result)
- `crates/r23-replay/src/bin/r23_m3_johansen.rs`: train-only OLS dynamic factor, 6 real legs (ETH/BNB/SOL/XRP/DOGE/LTC), 3 long / 3 short, factor-neutral.
- Backtested across entry_z 1.0–2.5: **all negative** (ann -0.6% to -1.15%). Honest: price-only factor basket has no edge in this window. 7 R4 experiment rows recorded.

### R4-M1 — OI/Crowding Exhaustion signal layer ✅ (validated; engine wiring pending)
- `crates/r23-replay/src/m1_signal.rs`: computes the §8 M1 robust state (oi_change, taker_flow, top/all crowd, price_ext via rolling median/MAD) + causal past-window activation flags.
- Validated on real Q3 metrics (43,773 rows) + aligned klines: 43,761 M1 states computed, SO-exhaustion flags fire ~50% of snapshots (deceleration causal), FO flags conservative (all-5-conditions rarely co-occur at q=0.95 — correct behavior).
- **Remaining M1 work:** wire these activation flags into the engine's single-symbol FO/SO/TP/abort decision path.

## 2. Strict-complete-backtest results (recorded, honest)

| Mechanism | Best config | Trades | Annualized | Max DD | Target hit? |
|---|---|---:|---:|---:|---|
| Pair control (R1) | 1000U, ez0.5 | 36 | 1.25% | 5.21% | No |
| M3 BTC-basket (R4) | 1000U, ez1.5 | 72 | -0.64% | 31.47% | No |
| M1 signal layer (R4) | (signal only, not yet traded) | — | — | — | — |

**Best combination recorded:** R1 pair control at 1000U → annualized 1.25%, max DD 5.21%. This is the current frontier; it does not approach any tier (50%/90%/110%).

## 3. Registry state

- `exploration-registry.jsonl`: **50 rows** (25 experiments × running+terminal), invariant clean, 0 failures.
- `failure-ledger.jsonl`: 0 rows.
- `phases_completed`: [R0, R1, R4]. Status: `BLOCKED_ENGINE_DATA_OR_EXECUTION` (R2/R3-full/R5/G0/G1/G2/R8 pending).

## 4. Three-tier target status (unchanged, no relaxation)

| Tier | Target | Status |
|---|---|---|
| Conservative | ann ≥ 50%, DD ≤ 10% | not hit |
| Balanced | ann ≥ 90%, DD ≤ 20% | not hit |
| Aggressive | ann ≥ 110%, DD ≤ 30% | not hit |

## 5. Why no target was hit (honest)

1. The R1 pair control and M3 basket are **baselines/simple mechanisms**, not the plan's edge-bearing families. They correctly produced flat/negative results — this validates the engine is not "finding" fake edge.
2. The plan's promising mechanisms (M1 OI/crowding, M2 depth/aggressor-flow) need (a) R3 full data and (b) engine-wiring of their signal flags into FO/SO. M1's signal layer is done and validated; M2 is not started.
3. Producing a target-claiming number before the mechanisms are wired + pass G0/G1 gates would repeat the Round 22 invalidity (claiming results from an incomplete engine). This session refused to do that.

## 6. How to resume (next session)

1. `git pull`. `cargo test -p r23-registry -p r23-replay` → 31+ tests green.
2. Check the background R3 download; once done, verify the full metrics manifest.
3. **M1 engine wiring**: extend `run_synchronized_cycle_replay` (or add a single-symbol Martin path) to gate FO/SO with the M1 activation flags; backtest; record.
4. **M2**: implement Depth Replenishment + Aggressor Flow (needs bookDepth + aggTrades download).
5. **R5/G0**: synthetic activation tests per family.
6. **G1**: frozen 12-block continuous replays per policy; P-B gate.
7. **G2/R8**: full budgets, 5 cold starts, stress, multiple-testing correction, target judgment.

## 7. Anti-overfit / reproducibility notes (honored)

- All fits are train-only; the engine never re-fits.
- All signal thresholds use only past-window data (causal).
- Every experiment is registry-recorded (running+terminal) with full SHA-256 provenance.
- The 15 R0 canaries stay green; the 12 R1 §5.3 directed checks stay green.
- Raw market-data archives stay out of git; only manifests are committed.

---

*Generated by GLM on `glm-martingale-core-round23`. State: `BLOCKED_ENGINE_DATA_OR_EXECUTION`. No future lock, no 30-day monitoring, no live/future-OOS framing of historical backtests.*
