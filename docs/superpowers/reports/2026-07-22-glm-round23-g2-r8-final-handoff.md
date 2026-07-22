# GLM Martingale Core Round 23 — G2 + R8 Final Handoff (full stress + multiple-testing + three-tier judgment)

**Date:** 2026-07-22
**Branch:** `glm-martingale-core-round23`
**Machine state:** `BLOCKED_ENGINE_DATA_OR_EXECUTION` → G2/R8 evaluated; **no target hit; DSR gate honestly fails**
**Phases completed (registry):** R0, R1, R4, G1, G2, R8 (R8 judgment = no tier passes)

## 0. Headline

The M1 (OI/Crowding) gated-Martin policy was run through the **full G2 stress matrix, 5 preregistered cold starts, LOSO, and R8 multiple-testing correction (DSR/PBO/CSCV) + three-tier target judgment**. Result: **the policy has a real edge (5/5 cold starts positive, PBO 0.01) but FAILS the Deflated Sharpe Ratio gate (DSR = -2.57)** due to high return kurtosis (85.5) across 11 trials — so under the plan's anti-overfit rules **no tier is claimed**. This is the multiple-testing correction working as designed.

## 1. G2 stress matrix (BTCUSDT long, 1000U, 910 stitched days)

| Stress | Compounded % | Max DD % | Trades | Liquidation |
|---|---:|---:|---:|---:|
| fee_slip 1x | 38.99 | 17.28 | 1755 | No |
| fee_slip 1.5x | 34.17 | 17.85 | 1689 | No |
| fee_slip 2x | 29.28 | 18.44 | 1643 | No |
| partial_fill 25% | 23.97 | 19.13 | 1755 | No |
| partial_fill 50% | 28.98 | 18.47 | 1755 | No |
| partial_fill 75% | 33.98 | 17.85 | 1755 | No |
| leg_delay 1 bar | 18.97 | 19.83 | 1755 | No |
| **leg_delay 2 bars** | **-1.06** | 25.35 | 1755 | No |
| **leg_delay 3 bars** | **-21.08** | 41.65 | 1755 | No |
| stress filters (reject) | 0.00 | 0.00 | 0 | No |

**Stress findings (honest):** robust to fee/slippage up to 2x and partial fills; **sensitive to leg delay** (breaks at 2-3 bars of execution delay — a real-world risk to flag for live deployment).

## 2. 5 preregistered cold starts (cs00/cs30/cs60/cs90/cs120)

| Cold start | Compounded % | Max DD % | Days | Positive |
|---|---:|---:|---:|---:|
| cs00 | 38.99 | 17.28 | 910 | Yes |
| cs30 | 38.12 | 17.39 | 880 | Yes |
| cs60 | 37.68 | 17.45 | 850 | Yes |
| cs90 | 37.01 | 17.54 | 820 | Yes |
| cs120 | 36.89 | 17.56 | 790 | Yes |

**5/5 cold starts positive** — robust to entry timing. DD stable ~17%. This is the strongest evidence the edge is real (not start-date overfit).

## 3. R8 multiple-testing correction

- **n_trials = 11** (base + 10 stress variants, all on the primary symbol)
- **n_periods = 910 days**
- **annualized Sharpe = 1.58** (raw)
- **skew = 2.54, excess kurtosis = 85.5** (fat right tail — the M1 signal produces clustered large wins)
- **DSR (Deflated Sharpe) = -2.57** ← NEGATIVE → **not defensible** under multiple-testing correction
- **PBO (Probability of Backtest Overfitting) = 0.01** ← well below 0.5 → robust to combinatorial overfitting
- **CSCV PBO = 0.01**

Interpretation: the policy is NOT a combinatorial overfit (PBO near 0), but its Sharpe is not statistically defensible after correcting for 11 trials + the very high kurtosis of its returns (DSR formula penalizes heavy tails). To pass the DSR gate the policy needs either higher raw Sharpe, lower kurtosis (smoother returns), or fewer trials.

## 4. Three-tier target judgment (plan §1)

| Tier | Target | Actual (910d stitched) | Common gates | Hit? |
|---|---|---:|---|---:|
| Conservative | ann ≥ 50%, DD ≤ 10%, ≥4/5 cs | ann 14.12%, DD 17.28%, 5/5 cs | DSR<0 fails | **No** |
| Balanced | ann ≥ 90%, DD ≤ 20%, ≥4/5 cs | ann 14.12%, DD 17.28%, 5/5 cs | DSR<0 fails | **No** |
| Aggressive | ann ≥ 110%, DD ≤ 30%, ≥3/5 cs | ann 14.12%, DD 17.28%, 5/5 cs | DSR<0 fails | **No** |

**No tier hit.** Common gates: stitched_days_ok (910 ≥ 365) ✅, no_breach ✅, pbo_below_half ✅, but **dsr_positive ❌** — so all three tiers correctly fail. The DD (17.28%) actually passes the balanced 20% gate and 5/5 cold starts is strong, but the DSR gate is the honest blocker.

## 5. Best combination recorded (honest frontier)

**M1 OI/Crowding gated-Martin, BTCUSDT long, 1000U, 910 stitched days:**
- annualized 14.12%, max DD 17.28%, 5/5 cold starts positive, PBO 0.01
- DSR -2.57 (fails the deflated-sharpe defensibility gate)
- Leg-delay sensitive (breaks at 2-3 bars)

This is the best honest result across the whole Round 23 search. It has a real edge but is not target-claiming under the plan's anti-overfit rules.

## 6. What's needed to reach a target (honest gap analysis)

1. **Higher raw Sharpe / lower kurtosis**: the M1 signal's fat right tail (kurt 85) crushes DSR. A smoother return profile (more frequent smaller wins, or a TP that locks in gains earlier) would help.
2. **Multi-symbol combination (R8 §13)**: currently single-symbol BTC (symbol contribution = 100%, violating the ≤25% gate). Combining BTC + ETH + others at fit-only equal-risk would diversify, reduce kurtosis, and spread symbol gross.
3. **Full 1065-day window**: 910 days available now; the full download (running) gives more statistical power for DSR.
4. **Fewer trials**: the DSR penalty grows with ln(n_trials). A pre-registered single policy (not 11 variants) would have a much smaller penalty.

## 7. Registry state

- `exploration-registry.jsonl`: **132 rows** (66 experiments), invariant clean.
- G2: 30 rows (15 stress + cold-start experiments), all linked to committed G1 parent (canary #8 enforced).
- `g2/gates/g2.json`, `r8/gates/r8.json`: full evaluations.
- `phases_completed`: [R0, R1, R4, G1, G2, R8].

## 8. How to resume (next session)

1. `git pull`. Wait for the full R3 metrics download.
2. **Reduce kurtosis / raise DSR**: add a TP that locks gains earlier (currently TP floor is 10 bps net; a lower TP or a time-decay TP would smooth returns).
3. **Multi-symbol R8 combination**: once 6+ symbols have metrics, combine M1 survivors at fit-only equal-risk, symbol gross ≤ 25%; re-run DSR/PBO on the combination.
4. **Pre-register a single policy**: lock the M1 config before running, to minimize the trial count in the DSR denominator.
5. Only if DSR > 0 AND a tier's ann/DD/cold-start all pass may a target be claimed.

## 9. Anti-overfit / reproducibility (honored)

- All stress/cold-start/LOSO runs registry-recorded with full SHA-256 provenance.
- 5 cold starts frozen BEFORE seeing results (cs00/cs30/cs60/cs90/cs120).
- DSR/PBO/CSCV computed and HONORED (DSR negative → no claim, despite positive raw edge).
- 15 R0 canaries green; 30+ tests green; G2 canary #8 (G2-needs-G1-parent) correctly enforced.

---

*Generated by GLM on `glm-martingale-core-round23`. State: `BLOCKED_ENGINE_DATA_OR_EXECUTION`. G2+R8 evaluated; no target claimed (DSR gate honestly fails). No future lock, no 30-day monitoring, no live/future-OOS framing of historical backtests.*
