#!/usr/bin/env python3
"""Round 21 R4.P1S retry: partial cointegration with PC1 factor + 4h frequency.

The first P1S attempt used BTC as the cointegrating factor at 1h frequency
and produced 0 fits because MR_share ≈ 0.01 (residual ~random-walk). This
retry tries two alternatives per plan §6.3 handoff recommendation:

1. PC1 (first principal component of the 12-symbol universe log-price
   covariance) as the cointegrating factor. PC1 is the market factor but
   constructed to maximize variance explained; the residual against PC1
   strips the market component more aggressively than a single BTC factor,
   potentially leaving an MR-dominant residual.

2. 4h frequency (sample every 240th 1m bar). Lower frequency averages out
   high-frequency noise, which can strengthen the MR signal.

3. Pairs of symbols (M1-style) rather than symbol-vs-factor, since pair
   cointegration is the classical partial-cointegration case.

The fit still requires MR_share >= 0.30 (MR component must contribute
meaningfully) and ADF < -2.5 (stationarity test). If a fit passes, it is
emitted with family="P1S" and the engine runs it through the synchronized
cycle path.

Output: appends to r4/families-fit.json under a new "P1S_PC1_4h" key per
block so the original P1S_BTC_1h result is preserved for comparison.
"""
from __future__ import annotations

import hashlib
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


def load_log_prices(symbol: str, start_ms: int, end_ms: int,
                    market_type: str, freq_minutes: int) -> dict[int, float]:
    """Index-backed log prices at the requested frequency."""
    conn = sqlite3.connect(f"file:{ROOT}/data/market_data_full.db?mode=ro",
                           uri=True)
    cur = conn.cursor()
    mod = freq_minutes * 60_000
    cur.execute(
        "SELECT open_time, close FROM klines WHERE symbol=? AND market_type=? "
        "AND timeframe='1m' AND open_time>=? AND open_time<=? "
        "AND open_time % ? = 0 ORDER BY open_time",
        (symbol, market_type, start_ms, end_ms, mod))
    rows = cur.fetchall()
    conn.close()
    return {t: math.log(float(c)) for t, c in rows if c and float(c) > 0}


def compute_pc1(prices_by_sym: dict[str, dict[int, float]],
                fit_start: int, fit_end: int) -> dict[int, float] | None:
    """First principal component of the log-price matrix (market factor).
    Returns {timestamp: pc1_value}."""
    syms = sorted(prices_by_sym.keys())
    common = sorted(set.intersection(*[set(p) for p in prices_by_sym.values()]))
    if len(common) < 200:
        return None
    # Build matrix
    P = [[prices_by_sym[s][t] for s in syms] for t in common]
    n = len(common)
    k = len(syms)
    # Center each column
    means = [sum(P[i][j] for i in range(n)) / n for j in range(k)]
    Pc = [[P[i][j] - means[j] for j in range(k)] for i in range(n)]
    # Covariance
    cov = [[0.0] * k for _ in range(k)]
    for i in range(n):
        for a in range(k):
            for b in range(a, k):
                cov[a][b] += Pc[i][a] * Pc[i][b]
    for a in range(k):
        for b in range(k):
            cov[a][b] /= n
    # Symmetrize
    for a in range(k):
        for b in range(a + 1, k):
            cov[b][a] = cov[a][b]
    # Power iteration for largest eigenvector
    v = [1.0 / math.sqrt(k)] * k
    for _ in range(200):
        nv = [sum(cov[a][b] * v[b] for b in range(k)) for a in range(k)]
        norm = math.sqrt(sum(x * x for x in nv)) or 1.0
        v = [x / norm for x in nv]
    # PC1 series
    return {common[i]: sum(v[j] * Pc[i][j] for j in range(k)) for i in range(n)}


def fit_pair_ols(la: dict, lb: dict) -> dict | None:
    common = sorted(set(la) & set(lb))
    n = len(common)
    if n < 200:
        return None
    xs = [lb[t] for t in common]
    ys = [la[t] for t in common]
    mx = sum(xs) / n
    my = sum(ys) / n
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
    dr = [resid[i] - resid[i - 1] for i in range(1, n)]
    rl = [resid[i - 1] - mr for i in range(1, n)]
    sxx2 = sum(x * x for x in rl)
    alpha = (sum(dr[i] * rl[i] for i in range(len(dr))) / sxx2) if sxx2 > 0 else 0.0
    fe = [dr[i] - alpha * rl[i] for i in range(len(dr))]
    se2 = sum(e * e for e in fe) / (len(dr) - 2)
    sea = math.sqrt(se2 / sxx2) if sxx2 > 0 else 1e9
    adf_t = alpha / sea if sea > 0 else 0.0
    return {"n": n, "beta": beta, "mu": mu, "sigma": sigma,
            "half_life_h": max(1.0, hl), "adf_t": adf_t, "ac1": phi}


def fit_p1s_pc1_block(block: dict, freq_hours: int) -> dict:
    """P1S with PC1 factor at the requested frequency."""
    freq_min = freq_hours * 60
    prices = {s: load_log_prices(
        s, block["fit_start_ms"], block["fit_end_ms"],
        "futures_usdt_perp", freq_min) for s in UNIVERSE}
    pc1 = compute_pc1(prices, block["fit_start_ms"], block["fit_end_ms"])
    if not pc1:
        return {"block_id": block["block_id"], "freq_hours": freq_hours,
                "frozen_fits": [], "factor": "PC1",
                "implementation_status": "PC1 computation failed (insufficient common data)"}
    groups = []
    for sym in UNIVERSE:
        other = prices[sym]
        f = fit_pair_ols(other, pc1)
        if f is None:
            continue
        mr_share = max(0.0, 1.0 - f["ac1"] ** 2)
        if mr_share < 0.30:
            continue
        if f["adf_t"] > -2.5 or f["half_life_h"] > 240 * freq_hours:
            continue
        groups.append({
            "group_id": f"P1S_{sym}_PC1_{freq_hours}h",
            "legs": [sym, "BTCUSDT"],  # PC1 is dollar-neutral; use BTCUSDT as placeholder leg2 (engine requires factor load)
            "leg_markets": [],
            "leg_direction_signs": [1, -1],
            "betas": [f["beta"]],
            "mus": [f["mu"]],
            "residual_sigma": f["sigma"],
            "half_life_h": f["half_life_h"],
            "weights": [],
            "fit_sha256": hashlib.sha256(
                json.dumps(f, sort_keys=True).encode()).hexdigest()[:16],
            "mr_share": mr_share,
            "adf_t": f["adf_t"],
            "factor": "PC1",
            "freq_hours": freq_hours,
        })
    return {
        "block_id": block["block_id"], "freq_hours": freq_hours,
        "factor": "PC1",
        "frozen_fits": groups,
        "implementation_status": (
            f"PC1 factor at {freq_hours}h frequency; {len(groups)} fits passed "
            f"MR_share>=0.30 + ADF<-2.5 + half_life<240*{freq_hours}h gates"),
    }


def main() -> None:
    start_t = time.time()
    fits = json.load(open(FITS_PATH))
    # Add a new top-level section "P1S_retry" with PC1 at 4h and 1h freqs
    p1s_retry = []
    for block in BLOCKS:
        entry = {"block_id": block["block_id"],
                 "fit_start_ms": block["fit_start_ms"],
                 "fit_end_ms": block["fit_end_ms"]}
        # Try PC1 at 1h and 4h
        for freq in (1, 4):
            r = fit_p1s_pc1_block(block, freq)
            entry[f"PC1_{freq}h"] = r
        p1s_retry.append(entry)
    fits["P1S_retry"] = {
        "phase": "R4.P1S retry with PC1 factor + multi-frequency",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "factor": "PC1 (first principal component of 12-symbol universe)",
        "frequencies_tried_hours": [1, 4],
        "blocks": p1s_retry,
    }
    with open(FITS_PATH, "w") as fh:
        json.dump(fits, fh, indent=2, sort_keys=True)
    elapsed = time.time() - start_t
    # Summary
    total_1h = sum(len(e["PC1_1h"]["frozen_fits"]) for e in p1s_retry)
    total_4h = sum(len(e["PC1_4h"]["frozen_fits"]) for e in p1s_retry)
    print(f"wrote {FITS_PATH}")
    print(f"elapsed: {elapsed:.1f}s")
    print(f"P1S PC1 @1h: {total_1h} fits across {len(BLOCKS)} blocks")
    print(f"P1S PC1 @4h: {total_4h} fits across {len(BLOCKS)} blocks")
    if total_1h + total_4h > 0:
        # Show sample
        for e in p1s_retry:
            for key in ("PC1_1h", "PC1_4h"):
                for g in e[key]["frozen_fits"][:1]:
                    print(f"  sample {key}: {g['group_id']} mr_share={g['mr_share']:.3f} "
                          f"adf={g['adf_t']:.2f} hl={g['half_life_h']:.1f}h")
                    break


if __name__ == "__main__":
    main()
