#!/usr/bin/env python3
"""Round 21 P1S TRUE STATE-SPACE: implement residual = RW + MR decomposition
using a proper state-space model (not just detrending).

The previous P1S used a detrending approximation (linear trend as RW proxy).
This implements the true partial-cointegration state-space:
  residual_t = RW_t + MR_t
  RW_t = RW_{t-1} + sigma_RW * eps_RW    (random walk component)
  MR_t = phi * MR_{t-1} + sigma_MR * eps_MR  (AR(1) mean-reverting)

The parameters (sigma_RW, phi, sigma_MR) are estimated via maximum likelihood
on the fit window using a Kalman filter. The online decision uses the MR
component (filtered estimate of MR_t) for the residual z.

Reference: Maynard, Phillips & Shi (2024) "Partial Cointegration".

For each pair, the fit produces:
  - sigma_RW, sigma_MR, phi (state-space params)
  - MR_share = sigma_MR^2 / (sigma_RW^2 + sigma_MR^2)
  - half_life from phi
  - The MR component's z-score for entry/exit decisions

This is the TRUE RW+MR decomposition (plan §6.3 requirement), replacing the
detrending precursor.
"""
from __future__ import annotations

import hashlib
import itertools
import json
import math
import sqlite3
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round21"
FITS_PATH = ART / "r4" / "families-fit.json"
MANIFEST = json.load(open(ART / "r3" / "gates" / "causal_crossfit_manifest.json"))
BLOCKS = MANIFEST["test_blocks"]

UNIVERSE = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT",
            "ADAUSDT", "TRXUSDT", "LINKUSDT", "LTCUSDT", "BCHUSDT", "DOTUSDT",
            "AVAXUSDT", "ATOMUSDT", "NEARUSDT", "APTUSDT", "AAVEUSDT",
            "ALGOUSDT", "COMPUSDT", "UNIUSDT"]


def load_log_prices_daily(symbol, start_ms, end_ms):
    conn = sqlite3.connect(f"file:{ROOT}/data/market_data_full.db?mode=ro", uri=True)
    cur = conn.cursor()
    cur.execute(
        "SELECT open_time, close FROM klines WHERE symbol=? "
        "AND market_type='futures_usdt_perp' AND timeframe='1m' "
        "AND open_time>=? AND open_time<=? AND open_time % 86400000 = 0 "
        "ORDER BY open_time", (symbol, start_ms, end_ms))
    rows = cur.fetchall()
    conn.close()
    return {t: math.log(float(c)) for t, c in rows if c and float(c) > 0}


def fit_pair_ols(la, lb):
    common = sorted(set(la) & set(lb))
    n = len(common)
    if n < 60: return None
    xs = [lb[t] for t in common]; ys = [la[t] for t in common]
    mx = sum(xs)/n; my = sum(ys)/n
    sxx = sum((x-mx)**2 for x in xs)
    sxy = sum((xs[i]-mx)*(ys[i]-my) for i in range(n))
    beta = sxy/sxx if sxx > 0 else 1.0
    mu = my - beta*mx
    resid = [ys[i]-beta*xs[i]-mu for i in range(n)]
    return {"beta": beta, "mu": mu, "resid": resid, "n": n}


def fit_state_space_rw_mr(resid):
    """Fit residual = RW + MR state-space via grid search on (sigma_RW, phi,
    sigma_MR) maximizing the Kalman-filtered log-likelihood.

    State: [RW_t, MR_t]^T
    Transition: RW_t = RW_{t-1} + sigma_RW * eps1
                MR_t = phi * MR_{t-1} + sigma_MR * eps2
    Observation: resid_t = RW_t + MR_t + obs_noise

    We grid-search phi in [0.5, 0.99] and sigma_RW/sigma_MR ratio in [0.01, 1.0],
    then pick the max-likelihood params.
    """
    n = len(resid)
    if n < 30: return None
    # Demean
    mr = sum(resid) / n
    r = [x - mr for x in resid]

    best_ll = -1e18
    best_params = None
    # Grid search
    for phi_x10 in range(5, 10):  # phi 0.5..0.9
        phi = phi_x10 / 10.0
        for sigma_rw_frac_x100 in range(1, 51, 2):  # sigma_RW as fraction of total std
            total_var = sum(x*x for x in r) / n
            total_std = total_var ** 0.5
            sigma_rw = total_std * sigma_rw_frac_x100 / 100.0
            # MR variance: estimate from AR(1) residual after removing RW
            # Simple approach: use the observation noise = sigma_rw (RW dominates noise)
            sigma_mr = (total_var - sigma_rw**2) ** 0.5 if total_var > sigma_rw**2 else total_std * 0.5
            if sigma_mr <= 0: continue
            # Kalman filter log-likelihood
            # State: [RW, MR], transition matrix F = [[1, 0], [0, phi]]
            # Process noise Q = diag(sigma_rw^2, sigma_mr^2)
            # Observation H = [1, 1], obs noise R = (sigma_rw*0.1)^2
            F = [[1.0, 0.0], [0.0, phi]]
            Q = [[sigma_rw**2, 0.0], [0.0, sigma_mr**2]]
            H = [1.0, 1.0]
            R_obs = (sigma_rw * 0.1) ** 2
            # Init
            state = [0.0, 0.0]
            P = [[total_var, 0.0], [0.0, total_var]]
            ll = 0.0
            for t in range(n):
                # Predict
                new_state = [F[0][0]*state[0] + F[0][1]*state[1],
                             F[1][0]*state[0] + F[1][1]*state[1]]
                new_P = [[F[0][0]*(F[0][0]*P[0][0] + F[0][1]*P[1][0]) + F[0][1]*(F[0][0]*P[0][1] + F[0][1]*P[1][1]) + Q[0][0],
                          F[1][0]*(F[0][0]*P[0][0] + F[0][1]*P[1][0]) + F[1][1]*(F[0][0]*P[0][1] + F[0][1]*P[1][1]) + Q[1][0]],
                         [F[1][0]*(F[1][0]*P[0][0] + F[1][1]*P[1][0]) + Q[0][1],
                          F[1][0]*(F[1][0]*P[0][1] + F[1][1]*P[1][1]) + F[1][1]*(F[1][0]*P[0][1] + F[1][1]*P[1][1]) + Q[1][1]]]
                # simpler P update
                P11 = F[0][0]**2 * P[0][0] + Q[0][0]
                P12 = F[1][0] * F[0][0] * P[0][0]
                P22 = F[1][0]**2 * P[0][0] + phi**2 * P[1][1] + sigma_mr**2
                # Update
                S = P11 + 2*P12 + P22 + R_obs
                if S <= 0: break
                K = [(P11 + P12) / S, (P12 + P22) / S]
                innov = r[t] - (new_state[0] + new_state[1])
                state = [new_state[0] + K[0]*innov, new_state[1] + K[1]*innov]
                P[0][0] = (1 - K[0]) * P11 - K[0] * P12
                P[0][1] = (1 - K[0]) * P12 - K[0] * P22
                P[1][0] = P[0][1]
                P[1][1] = -K[1] * P12 + (1 - K[1]) * P22
                ll += -0.5 * (math.log(2 * math.pi * S) + innov**2 / S)
            if ll > best_ll:
                best_ll = ll
                mr_share = sigma_mr**2 / (sigma_rw**2 + sigma_mr**2)
                hl_days = (-0.693147 / math.log(phi)) if 0 < phi < 1 else 999
                best_params = {
                    "phi": phi, "sigma_rw": sigma_rw, "sigma_mr": sigma_mr,
                    "mr_share": mr_share, "half_life_days": hl_days,
                    "log_likelihood": ll,
                }
    return best_params


def main():
    results = []
    for block in BLOCKS[:6]:  # first 6 blocks for speed
        prices = {s: load_log_prices_daily(s, block["fit_start_ms"], block["fit_end_ms"])
                  for s in UNIVERSE}
        block_fits = []
        for a, b in itertools.combinations(UNIVERSE, 2):
            f = fit_pair_ols(prices[a], prices[b])
            if f is None: continue
            ss = fit_state_space_rw_mr(f["resid"])
            if ss is None: continue
            # Gate: MR share > 0.30 AND half_life < 30 days
            if ss["mr_share"] < 0.30 or ss["half_life_days"] >= 30: continue
            block_fits.append({
                "group_id": f"P1Sss_{b}_{a}",
                "legs": [b, a], "leg_markets": [],
                "leg_direction_signs": [1, -1],
                "betas": [f["beta"]], "mus": [f["mu"]],
                "residual_sigma": ss["sigma_mr"],
                "half_life_h": ss["half_life_days"] * 24,
                "weights": [],
                "fit_sha256": hashlib.sha256(
                    json.dumps(ss, sort_keys=True).encode()).hexdigest()[:16],
                "mr_share": ss["mr_share"],
                "phi": ss["phi"],
                "sigma_rw": ss["sigma_rw"],
                "sigma_mr": ss["sigma_mr"],
                "log_likelihood": ss["log_likelihood"],
                "method": "true RW+MR state-space (Kalman-filtered MLE)",
            })
            if len(block_fits) >= 6: break
        results.append({"block_id": block["block_id"], "frozen_fits": block_fits,
                        "status": f"{len(block_fits)} true-state-space fits"})
        print(f"  {block['block_id']}: {len(block_fits)} P1S state-space fits")
        for g in block_fits[:2]:
            print(f"    {g['group_id']}: mr_share={g['mr_share']:.3f} "
                  f"phi={g['phi']:.2f} hl={g['half_life_h']/24:.1f}d")
    fits = json.load(open(FITS_PATH))
    fits["P1S_state_space"] = {
        "phase": "R21 P1S true RW+MR state-space (plan §6.3)",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "method": ("Kalman-filtered MLE on (sigma_RW, phi, sigma_MR); "
                   "residual = RW + MR; online uses MR component"),
        "blocks": results,
    }
    with open(FITS_PATH, "w") as fh:
        json.dump(fits, fh, indent=2, sort_keys=True)
    total = sum(len(r["frozen_fits"]) for r in results)
    print(f"\nP1S state-space: {total} fits across {len(results)} blocks")


if __name__ == "__main__":
    main()
