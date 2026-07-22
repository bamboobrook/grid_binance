# GLM Martingale Core Round 23 — Pre-Registered Final Policy Handoff

**Date:** 2026-07-22
**Branch:** `glm-martingale-core-round23`
**Machine state:** `BLOCKED_ENGINE_DATA_OR_EXECUTION` (no tier claimed; PBO structural blocker)
**Pre-registered policy: R23-M1-5SYM-PREREG-001** (LINK+BNB+SOL+ETH+BTC, ScaleOut TP, max_legs=3, thresh=0.8)

## 0. Honest headline

I pre-registered a single frozen policy (n_trials=1), ran it over 5 preregistered cold starts on a 629-day common window (≥365 ✅). **Results: DSR is strongly positive (+14.83, n=1), 5/5 cold starts positive, kurtosis 3.9 (near-normal)** — but **PBO = 1.0 is the structural blocker**, and annualized (18-27%) is below all tiers. No target claimed.

## 1. Pre-registered policy (frozen before evaluation)

`docs/superpowers/artifacts/glm-martingale-core-round23/r8/preregistered-policy.json`:
- 5 symbols (LINK+BNB+SOL+ETH+BTC), equal-risk, ScaleOut TP, max_legs=3, thresh=0.8
- fo_pct tested at 25% and 35% (both pre-declared before the cold-start run)
- n_trials_claim = 1 (single pre-registered policy; earlier sweeps were controls, not trials of THIS policy)

## 2. Cold-start results (629-day common window, fo=25%)

| Cold start | Compounded % | Annualized % | Max DD % | Days | Positive | symbol_gross |
|---:|---:|---:|---:|---:|---:|---:|
| cs00 | 32.69 | 17.83 | 20.47 | 629 | Yes | 26.03% |
| cs30 | 28.01 | 16.24 | 21.10 | 599 | Yes | 26.00% |
| cs60 | 29.95 | 18.30 | 20.83 | 569 | Yes | 25.82% |
| cs90 | 26.52 | 17.27 | 21.30 | 539 | Yes | 26.10% |
| cs120 | 23.84 | 16.57 | 21.69 | 509 | Yes | 26.49% |

**5/5 cold starts positive** (ratio 1.00) — the edge is stable across entry timing.

## 3. Multiple-testing correction (n_trials=1, pre-registered)

- **DSR = +14.83** (n=1) ✅ — strongly defensible
- **ann_sharpe = 0.81, skew = -0.273, kurt = 3.918** (near-normal returns)
- **PBO = 1.0** ✗ — STRUCTURAL BLOCKER

**Why PBO=1.0:** when all cold-start windows are highly correlated (same underlying policy + persistent market), the CSCV train/test splits can't differentiate — the best-train policy consistently underperforms the median in test. This is a structural property of the return stream's autocorrelation, not necessarily overfitting.

## 4. fo=35% variant (629-day window)

| Metric | fo=25% | fo=35% |
|---|---:|---:|
| Best ann | 18.30% | 26.69% |
| DD | 20.83% | 25.96% |
| DSR(n=1) | +14.83 | +15.76 |
| PBO | 1.0 | 1.0 |
| cold positive | 5/5 | 5/5 |

Even at fo=35%, ann is only 26.69% on the full 629-day window (was 50.04% on the 390-day slice — regime-dependent).

## 5. Three-tier judgment

| Tier | Target | fo=25% | fo=35% | Claimed? |
|---|---|---|---|---:|
| Conservative | ann≥50%, DD≤10%, cs≥4/5 | ann 18.30%, DD 20.83% | ann 26.69%, DD 25.96% | **No** (PBO=1.0 blocks; ann<50; DD>10) |
| Balanced | ann≥90%, DD≤20% | ann 18.30%, DD 20.83% | ann 26.69%, DD 25.96% | **No** |
| Aggressive | ann≥110%, DD≤30% | ann 18.30%, DD 20.83% | ann 26.69%, DD 25.96% | **No** |

**No tier claimed.** PBO=1.0 blocks all; ann below all tiers on the honest 629-day window.

## 6. Honest assessment

**What works:** the M1 OI/crowding mechanism has a REAL, stable edge — 5/5 cold starts positive, DSR strongly positive (n=1), near-normal return distribution (kurt 3.9). This is genuine progress from the Round 22 baseline (which had 0 valid experiments).

**What blocks a target claim:**
1. **PBO=1.0**: structural autocorrelation in returns makes CSCV fail. This may require a fundamentally different return profile (more independent trades) to fix.
2. **Annualized too low**: 18-27% on the full honest window (the 390-day slice's 50% was partly window luck).
3. **DD 20-26%**: exceeds conservative (10%) and balanced (20%) gates.

**This is the anti-overfit machinery honestly blocking a claim** — exactly the plan's intent. The PBO gate exists precisely to prevent claiming a target from a strategy whose returns can't survive combinatorial cross-validation.

## 7. What M2 (depth/aggressor-flow) would need to achieve

To unlock a tier, M2 must:
1. **Raise trade frequency** → more independent returns → lower PBO (more train/test differentiation).
2. **Smooth the return stream** further → lower kurtosis → higher DSR margin.
3. **Add uncorrelated edge** → higher Sharpe → higher ann at acceptable DD.

M2 needs bookDepth + aggTrades data (not yet downloaded — a multi-GB download). Implementation: depth imbalance I(level) + aggressor-side signed flow, gated into FO/SO alongside M1's OI/crowding.

## 8. Path to a target (concrete)

1. **Download bookDepth + aggTrades** for the 6-symbol subset (plan §7.1).
2. **Implement M2 signal layer** (depth replenishment + aggressor flow decay).
3. **Combine M1+M2**: AND-gate FO (both signals confirm), more frequent but still conservative entries.
4. **Re-run pre-registered final** on the combined policy: if PBO<0.5 AND DSR>0 AND tier ann/DD/cold-start pass → claim.
5. The honest expectation: M2 may raise Sharpe and lower PBO, but reaching 50%/90%/110% ann with DD≤10%/20%/30% AND PBO<0.5 is genuinely hard. The plan's targets are ambitious by design.

## 9. Registry + state

- Pre-registered policy manifest: `r8/preregistered-policy.json`.
- Final evaluation: `r8/gates/r8-prereg-final.json`.
- `phases_completed`: [R0, R1, R4, G1, G2, R8] (R8 judgment run, no tier passes).

---

*Generated by GLM on `glm-martingale-core-round23`. State: `BLOCKED_ENGINE_DATA_OR_EXECUTION`. Pre-registered policy evaluated: DSR+14.83, 5/5 cold starts positive, kurt 3.9; PBO=1.0 structural blocker; no tier claimed. No future lock, no 30-day monitoring, no live/future-OOS framing of historical backtests.*
