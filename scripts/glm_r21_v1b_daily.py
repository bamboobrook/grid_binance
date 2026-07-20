#!/usr/bin/env python3
"""Round 21 V1B DAILY: re-implement V1B at daily frequency with rank>1 VECM.
The 1h-frequency V1B had half_life 675h (never traded). C1E showed daily
frequency gives much stronger MR (half_life 3-16h vs 100-160h). This script
tests whether V1B rank-2/rank-3 baskets at daily frequency are tradeable.

Method:
  1. For each R3 block, compute the top-k principal components of the
     30-symbol universe log-price covariance at daily frequency.
  2. Project pc2/pc3/pc2+pc3 onto the budget-feasible set (sum|w|=1,
     max|w|<=0.25, long/short gross>=0.35, non-zero legs>=5).
  3. Compute the basket residual half_life at daily frequency.
  4. If half_life < 30 days (tradeable), emit the V1B fit.
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


def top_k_eigenvectors(cov, k):
    n = len(cov)
    vecs = []
    c = [row[:] for row in cov]
    for _ in range(k):
        v = [1.0 / math.sqrt(n)] * n
        for _ in range(200):
            nv = [sum(c[a][b] * v[b] for b in range(n)) for a in range(n)]
            norm = math.sqrt(sum(x * x for x in nv)) or 1.0
            v = [x / norm for x in nv]
        av = [sum(c[a][b] * v[b] for b in range(n)) for a in range(n)]
        lam = sum(v[i] * av[i] for i in range(n))
        vecs.append(v[:])
        for a in range(n):
            for b in range(n):
                c[a][b] -= lam * v[a] * v[b]
    return vecs


def project_to_budget(w):
    k = len(w)
    w_c = [max(-0.25, min(0.25, x)) for x in w]
    idx_by_mag = sorted(range(k), key=lambda i: -abs(w_c[i]))
    keep = set(idx_by_mag[:6])
    w_s = [w_c[i] if i in keep else 0.0 for i in range(k)]
    positives = sum(1 for x in w_s if x > 0)
    negatives = sum(1 for x in w_s if x < 0)
    if positives == 0 or negatives == 0:
        kept_idx = [i for i in range(k) if i in keep]
        kept_idx.sort(key=lambda i: abs(w_s[i]))
        if kept_idx:
            i = kept_idx[0]
            w_s[i] = -math.copysign(0.10, w_s[i]) if w_s[i] != 0 else 0.10
    sw = sum(abs(x) for x in w_s)
    if sw <= 0: return None
    w_n = [x / sw for x in w_s]
    w_n = [max(-0.25, min(0.25, x)) for x in w_n]
    sw2 = sum(abs(x) for x in w_n)
    if sw2 > 0: w_n = [x / sw2 for x in w_n]
    long_gross = sum(x for x in w_n if x > 0)
    short_gross = sum(-x for x in w_n if x < 0)
    nz = sum(1 for x in w_n if abs(x) > 1e-9)
    max_w = max(abs(x) for x in w_n)
    net = abs(sum(w_n))
    if (nz >= 5 and long_gross >= 0.35 and short_gross >= 0.35
            and max_w <= 0.25 + 1e-9 and net <= 0.10 + 1e-9):
        return w_n
    return None


def fit_v1b_daily_block(block):
    prices = {s: load_log_prices_daily(s, block["fit_start_ms"], block["fit_end_ms"])
              for s in UNIVERSE}
    common = sorted(set.intersection(*[set(p) for p in prices.values()]))
    n = len(common)
    if n < 60:
        return {"block_id": block["block_id"], "frozen_fits": [],
                "status": "insufficient daily data"}
    syms = UNIVERSE
    k = len(syms)
    P = [[prices[s][t] for s in syms] for t in common]
    means = [sum(P[i][j] for i in range(n)) / n for j in range(k)]
    Pc = [[P[i][j] - means[j] for j in range(k)] for i in range(n)]
    cov = [[0.0] * k for _ in range(k)]
    for i in range(n):
        for a in range(k):
            for b in range(a, k):
                cov[a][b] += Pc[i][a] * Pc[i][b]
    for a in range(k):
        for b in range(k):
            cov[a][b] /= n
    for a in range(k):
        for b in range(a + 1, k):
            cov[b][a] = cov[a][b]
    vecs = top_k_eigenvectors(cov, 3)
    # Try rank-2/rank-3 combinations
    candidates = []
    for vi, v in enumerate(vecs):
        candidates.append((f"pc{vi+1}", v[:]))
        candidates.append((f"-pc{vi+1}", [-x for x in v]))
    for (i, j) in itertools.combinations(range(3), 2):
        candidates.append((f"pc{i+1}-pc{j+1}",
                           [vecs[i][m] - vecs[j][m] for m in range(k)]))
        candidates.append((f"pc{i+1}+pc{j+1}",
                           [vecs[i][m] + vecs[j][m] for m in range(k)]))
    best = None
    best_hl = 999
    for name, w_raw in candidates:
        w_proj = project_to_budget(w_raw)
        if w_proj is None: continue
        # Compute basket residual half_life at daily freq
        resid = [sum(w_proj[j] * Pc[i][j] for j in range(k)) for i in range(n)]
        mr = sum(resid) / n
        vr = sum((r - mr) ** 2 for r in resid) / n
        if vr <= 0: continue
        sigma = vr ** 0.5
        phi = sum((resid[i] - mr) * (resid[i - 1] - mr)
                  for i in range(1, n)) / (n * vr)
        hl_days = (-0.693147 / math.log(phi)) if 0 < phi < 1 else 999
        if hl_days < best_hl:
            best_hl = hl_days
            best = (name, w_proj, sigma, hl_days, phi)
    if best is None or best_hl >= 30:  # 30 days = tradeable threshold
        return {"block_id": block["block_id"], "frozen_fits": [],
                "status": f"no rank>1 candidate with hl<30d (best={best_hl:.1f}d)"}
    name, w, sigma, hl, phi = best
    signs = [1 if x > 0 else (-1 if x < 0 else 0) for x in w]
    group = {
        "group_id": f"V1Bd_{block['block_id']}_{name}",
        "legs": list(syms), "leg_markets": [],
        "leg_direction_signs": signs,
        "betas": w, "mus": [0.0] * k,
        "residual_sigma": sigma, "half_life_h": hl * 24,
        "weights": w,
        "fit_sha256": hashlib.sha256(
            json.dumps({"w": w, "name": name}).encode()).hexdigest()[:16],
        "source": name, "freq": "daily", "half_life_days": hl,
    }
    return {"block_id": block["block_id"], "frozen_fits": [group],
            "status": f"daily rank>1 {name} hl={hl:.1f}d sigma={sigma:.4f}"}


def main():
    results = []
    for block in BLOCKS:
        r = fit_v1b_daily_block(block)
        results.append(r)
        print(f"  {r['block_id']}: {r['status']}")
    fits = json.load(open(FITS_PATH))
    fits["V1B_daily"] = {
        "phase": "R21 V1B daily frequency (tradeability fix)",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "blocks": results,
    }
    with open(FITS_PATH, "w") as fh:
        json.dump(fits, fh, indent=2, sort_keys=True)
    total = sum(len(r["frozen_fits"]) for r in results)
    tradeable = sum(1 for r in results if r["frozen_fits"])
    print(f"\nV1B daily: {total} fits, {tradeable}/{len(BLOCKS)} blocks tradeable (hl<30d)")


if __name__ == "__main__":
    main()
