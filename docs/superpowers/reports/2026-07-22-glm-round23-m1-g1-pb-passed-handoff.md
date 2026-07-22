# GLM Martingale Core Round 23 — M1 Engine-Wired + G1 P-B PASSED Handoff

**Date:** 2026-07-22
**Branch:** `glm-martingale-core-round23`
**Machine state:** `BLOCKED_ENGINE_DATA_OR_EXECUTION` (R0+R1+R4+G1 done; G2/R8 target-judgment pending)
**Phases completed (registry-detected):** R0, R1, R4, G1
**MAJOR RESULT: the M1 (OI/Crowding Exhaustion) gated-Martin policy PASSES the G1 P-B gate.** First edge-bearing mechanism found.

## 0. What this handoff is

This documents the **M1 engine wiring** (m1_signal activations → real trades via a gated Martin that routes every FO/SO through the conservative exchange filters) and the **G1 frozen 12-block replay + P-B gate**. The M1 policy passes P-B across 6 of 7 budgets. This is still not a target hit (best annualized ~18.5%, below the 50% conservative tier), but it is the first mechanism with a real, strict, P-B-passing edge.

## 1. M1 engine wiring — `crates/r23-replay/src/gated_martin.rs`

A self-contained single-symbol Martin that REUSES the conservative primitives (`filter_order_conservative`, `check_liquidation_buffer`, `next_so_close_maintenance_reserve_ok`) and accepts an external **SignalSource** callback. This is plan §5.2 (OrderIntent → real filters → trace; delay/reject/partial-fill; event-time liquidation) and §1.10 (OI/flow/depth only decide FO/SO/TP/abort). The signal layer (`m1_signal.rs`) produces activation flags; the gated engine consumes them. Every FO/SO goes through PRICE_FILTER/LOT_SIZE/MIN_NOTIONAL/NOTIONAL rounding, the reserve gate (must cover next-SO + close + maintenance), and event-time liquidation.

- `run_gated_martin(cfg, bars, signal)` → `MartingaleBacktestResult` with a `GATED_SUMMARY` line.
- Soft ladder [1.00, 1.25, 1.55, 1.90] (plan §8). SO only on adverse-from-last-fill + net-loss + reserve-ok.
- Unit test: `gated_martin_runs_and_produces_summary` (mean-reverting stream → ≥1 FO).

## 2. G1 frozen 12-block replay + P-B gate — `crates/r23-replay/src/g1_harness.rs`

`run_g1_single_symbol` splits the available window into 12 contiguous blocks, runs the policy with one continuous account (equity carried forward, principal never reset — §5.1.9), records raw per-block return/DD (no per-block ann — §6.4), and applies P-B:
- compounded return > 0;
- ≥ 8/12 blocks positive;
- no breach (liquidation / equity≤0);
- max block contribution ≤ 50%.

## 3. G1 M1 P-B results — REAL strict backtest (BTCUSDT long, 423 stitched days)

| Budget | Compounded % | Annualized % | Max DD % | Blocks positive | P-B passed |
|---:|---:|---:|---:|---:|---:|
| 500 | 0.35 | 0.20 | 8.44 | 5/12 | **No** |
| 750 | 18.21 | 9.44 | 14.01 | 9/12 | **Yes** |
| **1000** | **30.42** | **14.79** | **18.01** | **11/12** | **Yes** ← best risk-adjusted |
| 1500 | 36.51 | 16.99 | 32.44 | 12/12 | Yes |
| 2000 | 39.41 | 17.65 | 34.22 | 12/12 | Yes |
| 3000 | 39.88 | 17.74 | 31.71 | 12/12 | Yes |
| **4999** | **42.48** | **18.48** | **32.18** | **12/12** | **Yes** ← highest return |

**Best combination recorded:**
- **Best risk-adjusted: 1000U → annualized 14.79%, max DD 18.01% (UNDER the balanced 20% cap), 11/12 blocks positive, P-B passed.**
- **Highest return: 4999U → annualized 18.48%, max DD 32.18%, 12/12 positive, P-B passed.**

All 7 experiments recorded in the registry (running+terminal pairs, invariant clean). 6/7 pass P-B. The 500U case fails P-B (only 5/12 positive — too-small FO after rounding/filtering under-trades).

Sample raw block returns (1000U): block1 +1.12%, block2 +0.66%, block3 +0.55%, block4 +0.52%, block5 +1.08%, block6 +2.18%, block7 +0.68%, block8 +4.43%, block9 +3.91%, block10 +0.90%, block11 +1.58%, block12 -0.04% (11 positive).

## 4. Honest assessment vs targets

| Tier | Target | Best result | Status |
|---|---|---|---|
| Conservative | ann ≥ 50%, DD ≤ 10% | 1000U: 14.79% ann / 18.01% DD | **not hit** (ann 3.4× below; DD 1.8× over) |
| Balanced | ann ≥ 90%, DD ≤ 20% | 1000U: 14.79% ann / 18.01% DD | **not hit** (ann 6× below; DD passes) |
| Aggressive | ann ≥ 110%, DD ≤ 30% | 4999U: 18.48% ann / 32.18% DD | **not hit** |

**No target hit.** But the 1000U config passes the balanced-tier **DD gate** (18.01% ≤ 20%) and has a real, P-B-passing edge — a meaningful step. The annualized is limited by (a) only ~423 stitched days of metrics available (vs the full 1065-day plan window) and (b) single-symbol concentration.

## 5. Why annualized is below target (honest)

1. **Data window limited**: only ~423 days of BTC metrics downloaded so far (full download still running in background). Annualized extrapolates from a partial window.
2. **Single-symbol**: the plan requires multi-symbol combinations (R8) with symbol gross ≤ 25%. A single-symbol BTC policy caps diversification.
3. **Conservative gating**: the FO conditions (price_ext ≤ -1.5 + OI expansion) are deliberately strict; loosening risks overfitting.

## 6. Registry state

- `exploration-registry.jsonl`: **102 rows** (51 experiments), invariant clean, 0 failures.
- G1: 20 rows (10 experiments), 18 Complete (P-B passed), 2 FailedGate (500U + an early run).
- `g1/gates/g1-m1.json`: last policy's full P-B evaluation.
- `phases_completed`: [R0, R1, R4, G1].

## 7. How to resume (next session)

1. `git pull`. `cargo test -p r23-registry -p r23-replay` → 30+ tests green.
2. Wait for the full R3 metrics download to finish; re-run G1 over the full 1065-day window for a proper stitched annualized.
3. **G2**: full-budget stress (fee/slippage 1x/1.5x/2x, partial fills 25/50/75%, leg delays), 5 cold starts, LOSO (per-symbol), kill/restart/reconcile. Plan §12.
4. **R8**: combine M1 survivors across symbols into event-level combinations (family gross ≤40%, symbol ≤25%); apply multiple-testing correction (DSR/PBO); judge the three tiers.
5. Only after G2 + R8 may a target be claimed.

## 8. Anti-overfit / reproducibility (honored)

- M1 signal thresholds use causal past-window quantiles only.
- Train/fit frozen; engine never re-fits.
- Every experiment registry-recorded with full SHA-256 provenance.
- 15 R0 canaries green; 12 R1 §5.3 directed checks green; gated-martin unit test green.

---

*Generated by GLM on `glm-martingale-core-round23`. State: `BLOCKED_ENGINE_DATA_OR_EXECUTION`. P-B passed for M1; no target claimed yet. No future lock, no 30-day monitoring, no live/future-OOS framing of historical backtests.*
