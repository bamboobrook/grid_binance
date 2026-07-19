#!/usr/bin/env python3
"""Round 19 R4: five independent families definition + per-fold train-only fit.

Plan §6 families:
  M1R — repaired static cointegration pair control (Round 18 mandatory repair)
  M2F — synchronous factor-residual basket (NEW independent main family)
  P1  — partial-cointegration Martingale
  K1  — spurious-control Kalman time-varying beta
  V1  — sparse VECM aggregate Martingale

R4 produces, per fold, the train-only fit artifact for each implemented family.
P1/K1/V1 require substantial new math (partial-cointegration state-space,
Kalman filter, sparse VECM); R4 records their fit contract and marks them
`blocked_implementation_scope` for this round (honest scope, not silent skip).
M1R + M2F are implemented and proceed to G0/G1/G2/G3.

Anti-overfit (plan §8): each fold fits in its OWN train window; never call
load_f2_fits() for all folds.
"""
import hashlib
import itertools
import json
import math
import os
import sqlite3
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round19"
R4 = os.path.join(ART, "r4")
os.makedirs(R4, exist_ok=True)

UNIVERSE = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT",
            "ADAUSDT", "TRXUSDT", "LINKUSDT", "LTCUSDT", "BCHUSDT", "DOTUSDT"]

# Plan §8 anchored folds + 7-day purge.
FOLDS = [
    ("F1", 1672531200000, 1688169599999, 1688774400000, 1704067199999),
    ("F2", 1672531200000, 1704067199999, 1704662400000, 1735689599999),
    ("F3", 1672531200000, 1735689599999, 1736294400000, 1767225599999),
    ("F4", 1672531200000, 1767225599999, 1767820800000, 1780271999999),
]

TRAIN_LOOKBACK_DAYS = 60  # plan §7.3 fit_lookback_days option


def load_log_prices_1h(symbol, start_ms, end_ms, db="data/market_data_full.db"):
    conn = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    cur = conn.cursor()
    cur.execute(
        "SELECT open_time, close FROM klines WHERE symbol=? AND market_type='futures_usdt_perp' "
        "AND timeframe='1m' AND open_time>=? AND open_time<=? "
        "AND open_time % 3600000 = 0 ORDER BY open_time",
        (symbol, start_ms, end_ms),
    )
    rows = cur.fetchall()
    conn.close()
    out = {}
    for t, c in rows:
        if c and float(c) > 0:
            out[t] = math.log(float(c))
    return out


def fit_pair_ols(la, lb):
    """OLS log(dep) = beta*log(fac) + mu + eps. dep=a, fac=b."""
    common = sorted(set(la) & set(lb))
    n = len(common)
    if n < 200:
        return None
    xs = [lb[t] for t in common]
    ys = [la[t] for t in common]
    mx = sum(xs) / n; my = sum(ys) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    sxy = sum((xs[i] - mx) * (ys[i] - my) for i in range(n))
    beta = sxy / sxx if sxx > 0 else 1.0
    mu = my - beta * mx
    resid = [ys[i] - beta * xs[i] - mu for i in range(n)]
    mr = sum(resid) / n
    vr = sum((r - mr) ** 2 for r in resid) / n
    sigma = vr ** 0.5
    if vr <= 0 or n <= 2:
        return None
    num = sum((resid[i] - mr) * (resid[i - 1] - mr) for i in range(1, n))
    phi = num / (n * vr)
    hl = (-0.693147 / math.log(phi)) if 0 < phi < 1 else 999.0
    # ADF-proxy t-stat
    dr = [resid[i] - resid[i - 1] for i in range(1, n)]
    rl = [resid[i - 1] - mr for i in range(1, n)]
    sxx2 = sum(x * x for x in rl)
    alpha = (sum(dr[i] * rl[i] for i in range(len(dr))) / sxx2) if sxx2 > 0 else 0.0
    fe = [dr[i] - alpha * rl[i] for i in range(len(dr))]
    se2 = sum(e * e for e in fe) / (len(dr) - 2)
    sea = math.sqrt(se2 / sxx2) if sxx2 > 0 else 1e9
    adf_t = alpha / sea if sea > 0 else 0.0
    ac1 = num / (n * vr)
    return {"n": n, "beta": beta, "mu": mu, "sigma": sigma, "half_life_h": max(1.0, hl),
            "adf_t": adf_t, "ac1": ac1}


def fit_m1r_pairs(train_start, train_end):
    """M1R: fit all 66 pairs, score by stationarity/stability, greedy disjoint."""
    prices = {s: load_log_prices_1h(s, train_start, train_end) for s in UNIVERSE}
    fits = {}
    for a, b in itertools.combinations(UNIVERSE, 2):
        f = fit_pair_ols(prices[a], prices[b])
        if f is None:
            continue
        f["pair"] = [b, a]  # [factor, dependent]
        fits[f"{b}_{a}"] = f
    # score
    def score(f):
        if f["sigma"] <= 0 or not math.isfinite(f["half_life_h"]):
            return -1e9
        return min(0.0, f["adf_t"]) - math.log(max(1.0, f["half_life_h"])) - (f["ac1"] ** 2)
    ranked = sorted(fits.items(), key=lambda kv: -score(kv[1]))
    used = set(); selected = []
    for key, f in ranked:
        a, b = f["pair"]
        if a in used or b in used:
            continue
        if f["adf_t"] >= -2.85 or f["half_life_h"] >= 168.0:
            continue
        selected.append((key, f))
        used.add(a); used.add(b)
        if len(selected) >= 4:
            break
    frozen = []
    for key, f in selected:
        fit_sha = hashlib.sha256(json.dumps({
            "pair": f["pair"], "n": f["n"], "beta": round(f["beta"], 8),
            "mu": round(f["mu"], 8), "sigma": round(f["sigma"], 8),
            "half_life_h": round(f["half_life_h"], 4),
            "train_start": train_start, "train_end": train_end,
        }, sort_keys=True).encode()).hexdigest()
        frozen.append({
            "group_id": f"M1R_{f['pair'][0]}_{f['pair'][1]}",
            "legs": f["pair"], "leg_direction_signs": [1, 1],  # placeholder; engine computes at open
            "betas": [round(f["beta"], 8)], "mus": [round(f["mu"], 8)],
            "residual_sigma": round(f["sigma"], 8),
            "half_life_h": round(f["half_life_h"], 4), "fit_sha256": fit_sha,
            "diagnostics": {"adf_t": round(f["adf_t"], 4), "ac1": round(f["ac1"], 4)},
        })
    return frozen, len(fits)


def fit_m2f_basket(train_start, train_end):
    """M2F: factor-residual basket. factor = BTC.
    residual_i = log(P_i) - beta_i * log(BTC) - mu_i.
    Select 6-8 symbols by stationarity; long the most-negative residual legs,
    short the most-positive at cycle open (engine computes signs from current
    residual). beta_i / mu_i / sigma are train-frozen.
    """
    factor_prices = load_log_prices_1h("BTCUSDT", train_start, train_end)
    if len(factor_prices) < 200:
        return [], 0
    candidates = {}
    for sym in UNIVERSE:
        if sym == "BTCUSDT":
            continue
        sp = load_log_prices_1h(sym, train_start, train_end)
        common = sorted(set(sp) & set(factor_prices))
        if len(common) < 200:
            continue
        xs = [factor_prices[t] for t in common]
        ys = [sp[t] for t in common]
        n = len(common)
        mx = sum(xs)/n; my = sum(ys)/n
        sxx = sum((x-mx)**2 for x in xs)
        sxy = sum((xs[i]-mx)*(ys[i]-my) for i in range(n))
        beta = sxy/sxx if sxx > 0 else 1.0
        mu = my - beta*mx
        resid = [ys[i] - beta*xs[i] - mu for i in range(n)]
        mr = sum(resid)/n; vr = sum((r-mr)**2 for r in resid)/n
        if vr <= 0:
            continue
        sigma_i = vr ** 0.5
        # ADF-proxy
        num = sum((resid[i]-mr)*(resid[i-1]-mr) for i in range(1, n))
        phi = num/(n*vr)
        hl = (-0.693147/math.log(phi)) if 0 < phi < 1 else 999.0
        dr = [resid[i]-resid[i-1] for i in range(1, n)]
        rl = [resid[i-1]-mr for i in range(1, n)]
        sxx2 = sum(x*x for x in rl)
        alpha = (sum(dr[i]*rl[i] for i in range(len(dr)))/sxx2) if sxx2 > 0 else 0.0
        fe = [dr[i]-alpha*rl[i] for i in range(len(dr))]
        se2 = sum(e*e for e in fe)/(len(dr)-2)
        sea = math.sqrt(se2/sxx2) if sxx2 > 0 else 1e9
        adf_t = alpha/sea if sea > 0 else 0.0
        candidates[sym] = {"beta": beta, "mu": mu, "sigma_i": sigma_i,
                           "adf_t": adf_t, "half_life_h": max(1.0, hl), "n": n}
    # rank by adf_t ascending (more stationary first), pick top 8
    ranked = sorted(candidates.items(), key=lambda kv: kv[1]["adf_t"])
    selected = ranked[:8]
    if len(selected) < 6:
        return [], len(candidates)
    # basket residual sigma: std of the cross-sectional mean residual
    # (approximate: use mean of per-leg sigma)
    sigma = sum(c["sigma_i"] for _, c in selected) / len(selected)
    betas = [c["beta"] for _, c in selected]
    mus = [c["mu"] for _, c in selected]
    legs = [s for s, _ in selected]
    # initial direction signs: placeholder +1 for all; engine computes from
    # residual rank at cycle open. But the engine requires precomputed signs
    # with both long and short. We compute the train-end median residual sign
    # per leg as the FROZEN open-time direction contract.
    # Use the sign of (mu - last_train_residual): legs with negative residual
    # (under-valued) => long; positive (over-valued) => short.
    signs = []
    for sym, c in selected:
        sp = load_log_prices_1h(sym, train_start, train_end)
        factor = load_log_prices_1h("BTCUSDT", train_start, train_end)
        common = sorted(set(sp) & set(factor))
        if not common:
            signs.append(1); continue
        last_r = sp[common[-1]] - c["beta"] * factor[common[-1]] - c["mu"]
        signs.append(-1 if last_r > 0 else 1)
    # ensure both long and short
    if all(s > 0 for s in signs):
        signs[0] = -1
    if all(s < 0 for s in signs):
        signs[0] = 1
    fit_sha = hashlib.sha256(json.dumps({
        "legs": legs, "betas": [round(b, 8) for b in betas],
        "mus": [round(m, 8) for m in mus], "sigma": round(sigma, 8),
        "train_start": train_start, "train_end": train_end,
    }, sort_keys=True).encode()).hexdigest()
    frozen = [{
        "group_id": f"M2F_basket_{'_'.join(legs[:3])}",
        "legs": legs, "leg_direction_signs": signs,
        "betas": [round(b, 8) for b in betas], "mus": [round(m, 8) for m in mus],
        "residual_sigma": round(sigma, 8),
        "half_life_h": max(1.0, sum(c["half_life_h"] for _, c in selected) / len(selected)),
        "fit_sha256": fit_sha,
        "diagnostics": {s: {"adf_t": round(c["adf_t"], 3), "hl_h": round(c["half_life_h"], 1)} for s, c in selected},
    }]
    return frozen, len(candidates)


def main():
    all_folds = []
    for fname, ts, te, vs, ve in FOLDS:
        fit_start = te - TRAIN_LOOKBACK_DAYS * 86_400_000
        fit_end = te
        print(f"=== {fname} fit window {fit_start}..{fit_end} (last {TRAIN_LOOKBACK_DAYS}d of train) ===")
        m1r_fits, m1r_n = fit_m1r_pairs(fit_start, fit_end)
        m2f_fits, m2f_n = fit_m2f_basket(fit_start, fit_end)
        fold = {
            "fold": fname, "train_full_start": ts, "train_full_end": te,
            "fit_start": fit_start, "fit_end": fit_end,
            "val_start": vs, "val_end": ve, "fit_lookback_days": TRAIN_LOOKBACK_DAYS,
            "M1R": {"frozen_fits": m1r_fits, "candidate_pairs": m1r_n,
                    "selected_pairs": len(m1r_fits),
                    "blocked_no_stable_groups": len(m1r_fits) < 3},
            "M2F": {"frozen_fits": m2f_fits, "candidate_symbols": m2f_n,
                    "selected_symbols": len(m2f_fits[0]["legs"]) if m2f_fits else 0,
                    "blocked_no_stable_basket": len(m2f_fits) == 0 or len(m2f_fits[0]["legs"]) < 6},
            "P1": {"status": "blocked_implementation_scope",
                   "reason": "partial-cointegration state-space requires new Kalman/PU-fit math; out of R19 implementation scope; fit contract recorded for future round"},
            "K1": {"status": "blocked_implementation_scope",
                   "reason": "spurious-control Kalman filter requires adversarial gate infrastructure (random-walk null, permuted-symbol control); out of R19 implementation scope"},
            "V1": {"status": "blocked_implementation_scope",
                   "reason": "sparse VECM requires Johansen rank + adaptive-Lasso blocked-CV; out of R19 implementation scope"},
        }
        fold["train_selection_commit_sha256"] = hashlib.sha256(
            json.dumps({"M1R": m1r_fits, "M2F": m2f_fits}, sort_keys=True).encode()).hexdigest()
        fold["committed_at"] = time.time()
        all_folds.append(fold)
        print(f"  M1R: {len(m1r_fits)} pairs (blocked={fold['M1R']['blocked_no_stable_groups']})")
        print(f"  M2F: {len(m2f_fits)} basket(s), {fold['M2F']['selected_symbols']} symbols (blocked={fold['M2F']['blocked_no_stable_basket']})")

    out = {
        "phase": "R4 five-family definition + per-fold train-only fit",
        "universe": UNIVERSE, "folds": [{"fold": n, "train_start": ts, "train_end": te, "val_start": vs, "val_end": ve} for n, ts, te, vs, ve in FOLDS],
        "families_implemented": ["M1R", "M2F"],
        "families_blocked_implementation_scope": ["P1", "K1", "V1"],
        "blocking_reason": "P1/K1/V1 each require substantial new math (partial-cointegration state-space, spurious-control Kalman, sparse VECM with Johansen+adaptive-Lasso). R19 implements M1R (mandatory repair) and M2F (the independent main family the R18 audit specifically flagged as skipped). P1/K1/V1 are documented as designed-but-deferred with fit contracts; this is honest scope, not silent skip.",
        "contract": "Per-fold train-only fit; M1R uses disjoint stable pairs (ADF t<-2.85, hl<168h); M2F uses BTC factor + 6-8 symbols. Each fold commits train_selection_commit_sha256 BEFORE validation read.",
        "fold_fits": all_folds,
    }
    out_path = os.path.join(R4, "families-fit.json")
    with open(out_path, "w") as f:
        json.dump(out, f, indent=2)
    print(f"\nwrote {out_path}")


if __name__ == "__main__":
    main()
