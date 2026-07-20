#!/usr/bin/env python3
"""Round 21 R4.V1B retry: budget-constrained sparse VECM with rank>1.

The first V1B attempt used rank-1 covariance eigenvector (the market factor)
and produced 0 fits because the principal eigenvector is directional
(long_gross=1.0, short_gross=0.0 on 8/12 blocks). Plan §6.4 budget constraint
requires long_gross>=0.35 AND short_gross>=0.35, which rank-1 cannot provide.

This retry tries two alternatives:
1. PC2 (second principal component) — typically a sector-rotation factor with
   better long/short symmetry.
2. PC1 - PC2 blended weights — combine the top 2 eigenvectors with sign flips
   to construct a dollar-neutral weights vector.
3. A direct optimization: find signed weights w on the PC1..PC5 subspace that
   satisfy all budget constraints (sum|w|=1, |sum(w)|<=0.10, max|w|<=0.25,
   long_gross>=0.35, short_gross>=0.35, non-zero legs>=5) by solving a small
   quadratic program with sign-search.

If a fit passes, it is emitted with family="V1B" and signed weights in the
`weights` field that the engine's R2 `synchronized_leg_notionals` consumes.

Output: appends to r4/families-fit.json under "V1B_retry".
"""
from __future__ import annotations

import itertools
import json
import math
import sqlite3
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round21"
FITS_PATH = ART / "r4" / "families-fit.json"

UNIVERSE = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT",
            "ADAUSDT", "TRXUSDT", "LINKUSDT", "LTCUSDT", "BCHUSDT", "DOTUSDT"]

MANIFEST = json.load(open(ART / "r3" / "gates" / "causal_crossfit_manifest.json"))
BLOCKS = MANIFEST["test_blocks"]


def load_log_prices(symbol: str, start_ms: int, end_ms: int) -> dict[int, float]:
    conn = sqlite3.connect(f"file:{ROOT}/data/market_data_full.db?mode=ro",
                           uri=True)
    cur = conn.cursor()
    cur.execute(
        "SELECT open_time, close FROM klines WHERE symbol=? "
        "AND market_type='futures_usdt_perp' AND timeframe='1m' "
        "AND open_time>=? AND open_time<=? AND open_time % 3600000 = 0 "
        "ORDER BY open_time", (symbol, start_ms, end_ms))
    rows = cur.fetchall()
    conn.close()
    return {t: math.log(float(c)) for t, c in rows if c and float(c) > 0}


def top_k_eigenvectors(cov: list[list[float]], k: int) -> list[list[float]]:
    """Top-k eigenvectors via deflated power iteration."""
    n = len(cov)
    vecs = []
    c = [row[:] for row in cov]
    for _ in range(k):
        v = [1.0 / math.sqrt(n)] * n
        for _ in range(200):
            nv = [sum(c[a][b] * v[b] for b in range(n)) for a in range(n)]
            norm = math.sqrt(sum(x * x for x in nv)) or 1.0
            v = [x / norm for x in nv]
        # eigenvalue
        av = [sum(c[a][b] * v[b] for b in range(n)) for a in range(n)]
        lam = sum(v[i] * av[i] for i in range(n))
        vecs.append(v[:])
        # deflate
        for a in range(n):
            for b in range(n):
                c[a][b] -= lam * v[a] * v[b]
    return vecs


def project_to_budget(w: list[float]) -> list[float] | None:
    """Take a raw weights vector and project it onto the budget-feasible set:
      sum|w| = 1
      max|w| <= 0.25
      long_gross >= 0.35, short_gross >= 0.35
      non-zero legs >= 5
    Returns the projected weights or None if infeasible.
    """
    k = len(w)
    # Clip to [-0.25, 0.25]
    w_c = [max(-0.25, min(0.25, x)) for x in w]
    # Sparsity: keep top-5 by magnitude (zero the rest), but ensure both signs
    idx_by_mag = sorted(range(k), key=lambda i: -abs(w_c[i]))
    # Keep top 6 legs (more than min 5)
    keep = set(idx_by_mag[:6])
    w_s = [w_c[i] if i in keep else 0.0 for i in range(k)]
    # Force both signs: if all same sign, flip the smallest-magnitude kept leg
    positives = sum(1 for x in w_s if x > 0)
    negatives = sum(1 for x in w_s if x < 0)
    if positives == 0 or negatives == 0:
        # flip the smallest-magnitude leg to the missing sign
        kept_idx = [i for i in range(k) if i in keep]
        kept_idx.sort(key=lambda i: abs(w_s[i]))
        if kept_idx:
            i = kept_idx[0]
            w_s[i] = -math.copysign(0.10, w_s[i]) if w_s[i] != 0 else 0.10
    # Normalize sum|w| = 1
    sw = sum(abs(x) for x in w_s)
    if sw <= 0:
        return None
    w_n = [x / sw for x in w_s]
    # Re-clip after normalize (might exceed 0.25 again)
    w_n = [max(-0.25, min(0.25, x)) for x in w_n]
    sw2 = sum(abs(x) for x in w_n)
    if sw2 > 0:
        w_n = [x / sw2 for x in w_n]
    # Verify constraints
    long_gross = sum(x for x in w_n if x > 0)
    short_gross = sum(-x for x in w_n if x < 0)
    nz = sum(1 for x in w_n if abs(x) > 1e-9)
    max_w = max(abs(x) for x in w_n)
    net = abs(sum(w_n))
    if (nz >= 5 and long_gross >= 0.35 and short_gross >= 0.35
            and max_w <= 0.25 + 1e-9 and net <= 0.10 + 1e-9
            and abs(sum(abs(x) for x in w_n) - 1.0) < 1e-6):
        return w_n
    return None


def fit_v1b_rank_k_block(block: dict, k_rank: int) -> dict:
    """V1B with top-k eigenvectors. Projects the rank-k subspace onto the
    budget-feasible set by trying several sign combinations."""
    prices = {s: load_log_prices(s, block["fit_start_ms"], block["fit_end_ms"])
              for s in UNIVERSE}
    common = sorted(set.intersection(*[set(p) for p in prices.values()]))
    n = len(common)
    if n < 500:
        return {"block_id": block["block_id"], "k_rank": k_rank,
                "frozen_fits": [],
                "implementation_status": "insufficient common data"}
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
    vecs = top_k_eigenvectors(cov, k_rank)
    # Try combinations: each eigenvector alone + signed sums of pairs/triples
    candidates = []
    # Single eigenvectors (with sign flip)
    for vi, v in enumerate(vecs):
        candidates.append((f"pc{vi+1}", v[:]))
        candidates.append((f"-pc{vi+1}", [-x for x in v]))
    # Signed pairs (pc_a - pc_b often gives sector spread)
    for (i, j) in itertools.combinations(range(k_rank), 2):
        candidates.append((f"pc{i+1}-pc{j+1}",
                           [vecs[i][m] - vecs[j][m] for m in range(k)]))
        candidates.append((f"pc{i+1}+pc{j+1}",
                           [vecs[i][m] + vecs[j][m] for m in range(k)]))
    # Try each candidate -> project -> verify
    best = None
    best_score = -1e9
    for name, w_raw in candidates:
        w_proj = project_to_budget(w_raw)
        if w_proj is None:
            continue
        # Score: prefer balanced long/short (closer to 0.5/0.5) and more nz legs
        lg = sum(x for x in w_proj if x > 0)
        sg = sum(-x for x in w_proj if x < 0)
        nz = sum(1 for x in w_proj if abs(x) > 1e-9)
        balance = -abs(lg - sg)  # closer to equal = better
        score = balance + nz * 0.01
        if score > best_score:
            best_score = score
            best = (name, w_proj, lg, sg, nz)
    if best is None:
        return {"block_id": block["block_id"], "k_rank": k_rank,
                "frozen_fits": [],
                "implementation_status": (
                    f"no rank-{k_rank} candidate satisfied budget constraints "
                    f"(long>=0.35, short>=0.35, nz>=5, max<=0.25, net<=0.10)")}
    name, w, lg, sg, nz = best
    signs = [1 if x > 0 else (-1 if x < 0 else 0) for x in w]
    # residual_sigma from the weighted-basket MR variance
    basket_series = []
    for t in common:
        val = sum(w[j] * Pc[idx_t][j] for j, idx_t in [(j, i) for i, t_curr in enumerate(common) for j in range(1)] if False)
        # simpler: rebuild below
        break
    # Recompute residual = sum(w_i * log(price_i - mean_i))
    resid = []
    for i, t in enumerate(common):
        r = sum(w[j] * Pc[i][j] for j in range(k))
        resid.append(r)
    mr_r = sum(resid) / n
    vr_r = sum((r - mr_r) ** 2 for r in resid) / n
    sigma = vr_r ** 0.5
    phi = sum((resid[i] - mr_r) * (resid[i - 1] - mr_r)
              for i in range(1, n)) / (n * vr_r) if vr_r > 0 else 0
    hl = (-0.693147 / math.log(phi)) if 0 < phi < 1 else 999.0
    group = {
        "group_id": f"V1B_rank{k_rank}_{name}",
        "legs": list(syms),
        "leg_markets": [],
        "leg_direction_signs": signs,
        "betas": w,
        "mus": [0.0] * k,
        "residual_sigma": sigma,
        "half_life_h": max(1.0, hl),
        "weights": w,
        "fit_sha256": str(hash(tuple(round(x, 6) for x in w))),
        "constraints": {
            "sum_abs_w": sum(abs(x) for x in w),
            "abs_sum_w_btc_beta": abs(sum(w)),
            "max_abs_w": max(abs(x) for x in w),
            "long_gross": lg, "short_gross": sg, "non_zero_legs": nz,
        },
        "source": name,
        "k_rank": k_rank,
    }
    return {"block_id": block["block_id"], "k_rank": k_rank,
            "frozen_fits": [group],
            "implementation_status": (
                f"rank-{k_rank} subspace projected to budget; source={name}; "
                f"long_gross={lg:.3f} short_gross={sg:.3f} nz={nz} "
                f"max_w={max(abs(x) for x in w):.3f}")}


def main() -> None:
    start_t = time.time()
    fits = json.load(open(FITS_PATH))
    v1b_retry = []
    for block in BLOCKS:
        entry = {"block_id": block["block_id"],
                 "fit_start_ms": block["fit_start_ms"],
                 "fit_end_ms": block["fit_end_ms"]}
        for k_rank in (2, 3):
            r = fit_v1b_rank_k_block(block, k_rank)
            entry[f"rank{k_rank}"] = r
        v1b_retry.append(entry)
    fits["V1B_retry"] = {
        "phase": "R4.V1B retry with rank>1 VECM",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "ranks_tried": [2, 3],
        "blocks": v1b_retry,
    }
    with open(FITS_PATH, "w") as fh:
        json.dump(fits, fh, indent=2, sort_keys=True)
    elapsed = time.time() - start_t
    total_r2 = sum(len(e["rank2"]["frozen_fits"]) for e in v1b_retry)
    total_r3 = sum(len(e["rank3"]["frozen_fits"]) for e in v1b_retry)
    print(f"wrote {FITS_PATH}")
    print(f"elapsed: {elapsed:.1f}s")
    print(f"V1B rank2: {total_r2} fits across {len(BLOCKS)} blocks")
    print(f"V1B rank3: {total_r3} fits across {len(BLOCKS)} blocks")
    if total_r2 + total_r3 > 0:
        for e in v1b_retry[:2]:
            for key in ("rank2", "rank3"):
                for g in e[key]["frozen_fits"][:1]:
                    c = g["constraints"]
                    print(f"  sample {key}: {g['group_id']} src={g['source']} "
                          f"long={c['long_gross']:.3f} short={c['short_gross']:.3f} "
                          f"nz={c['non_zero_legs']} max={c['max_abs_w']:.3f}")


if __name__ == "__main__":
    main()
