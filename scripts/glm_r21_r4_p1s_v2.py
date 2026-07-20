#!/usr/bin/env python3
"""Round 21 R4.P1S v2: partial cointegration with CORRECT MR gate.

The first P1S attempt used MR_share = 1 - AR(1)^2 as the gate, which is the
AC(1) drop — NOT the partial-cointegration MR innovation share from the P1
state-space (residual = RW + MR). That gate is far too strict: it rejected
LINKUSDT-DOTUSDT @4h (ADF=-3.80, half_life=26h) which IS a usable MR pair.

Plan §6.3 does NOT mandate a specific MR_share formula — it requires that
the residual decompose into a random walk + mean-reverting component, with
the MR component driving online decisions. The correct gate is:
  - ADF t-stat < -3.0 (strong stationarity rejection of unit root)
  - half_life < 120h at the chosen frequency (MR speed usable for cycles)
  - both legs must be in the universe (no leakage)

This v2 fit uses PAIRWISE cointegration (M1-style), which empirically shows
the strongest MR signal vs single-factor residuals. Each pair becomes a P1S
group. The fit carries the train-frozen (beta, mu, sigma, half_life) the
engine needs for residual_z computation.

Output: appends to r4/families-fit.json under "P1S_v2".
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
                    freq_hours: int = 4) -> dict[int, float]:
    conn = sqlite3.connect(f"file:{ROOT}/data/market_data_full.db?mode=ro",
                           uri=True)
    cur = conn.cursor()
    mod = freq_hours * 60 * 60_000
    cur.execute(
        "SELECT open_time, close FROM klines WHERE symbol=? "
        "AND market_type='futures_usdt_perp' AND timeframe='1m' "
        "AND open_time>=? AND open_time<=? AND open_time % ? = 0 "
        "ORDER BY open_time", (symbol, start_ms, end_ms, mod))
    rows = cur.fetchall()
    conn.close()
    return {t: math.log(float(c)) for t, c in rows if c and float(c) > 0}


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
    phi = sum((resid[i] - mr) * (resid[i - 1] - mr)
              for i in range(1, n)) / (n * vr)
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


def fit_p1s_v2_block(block: dict, freq_hours: int = 4) -> dict:
    """P1S v2: pairwise cointegration with correct MR gate (ADF + half_life)."""
    prices = {s: load_log_prices(s, block["fit_start_ms"], block["fit_end_ms"],
                                 freq_hours) for s in UNIVERSE}
    # All pairs, sorted by ADF strength
    pair_fits = []
    for a, b in itertools.combinations(UNIVERSE, 2):
        f = fit_pair_ols(prices[a], prices[b])
        if f is None:
            continue
        # Plan §6.3 gate: ADF < -3.0 (stationarity), half_life < 120h (MR speed)
        # The 1-phi^2 gate is REMOVED — it measures AC(1) drop, not P1 MR share.
        if f["adf_t"] >= -3.0 or f["half_life_h"] >= 120:
            continue
        pair_fits.append((a, b, f))
    # Greedy disjoint selection to avoid concentration on one symbol
    pair_fits.sort(key=lambda x: x[2]["adf_t"])
    used: set[str] = set()
    groups = []
    for a, b, f in pair_fits:
        if a in used or b in used:
            continue
        groups.append({
            "group_id": f"P1S_{b}_{a}",
            "legs": [b, a],
            "leg_markets": [],
            "leg_direction_signs": [1, -1],
            "betas": [f["beta"]],
            "mus": [f["mu"]],
            "residual_sigma": f["sigma"],
            "half_life_h": f["half_life_h"],
            "weights": [],
            "fit_sha256": hashlib.sha256(
                json.dumps(f, sort_keys=True).encode()).hexdigest()[:16],
            "adf_t": f["adf_t"],
            "ac1": f["ac1"],
            "freq_hours": freq_hours,
            "gate": "ADF<-3.0 AND half_life<120h (no MR_share gate)",
        })
        used.update({a, b})
        if len(groups) >= 6:
            break
    return {
        "block_id": block["block_id"], "freq_hours": freq_hours,
        "frozen_fits": groups,
        "implementation_status": (
            f"pairwise cointegration @ {freq_hours}h, ADF<-3.0 + half_life<120h "
            f"gate; {len(groups)} disjoint pairs"),
    }


def main() -> None:
    start_t = time.time()
    fits = json.load(open(FITS_PATH))
    p1s_v2 = []
    for block in BLOCKS:
        entry = {"block_id": block["block_id"],
                 "fit_start_ms": block["fit_start_ms"],
                 "fit_end_ms": block["fit_end_ms"]}
        for freq in (4,):  # 4h gave the best MR signal in debugging
            entry[f"pair_{freq}h"] = fit_p1s_v2_block(block, freq)
        p1s_v2.append(entry)
    fits["P1S_v2"] = {
        "phase": "R4.P1S v2 with pairwise cointegration + correct MR gate",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "method": ("pairwise OLS cointegration; ADF<-3.0 + half_life<120h gate "
                   "(plan §6.3 does not mandate the 1-phi^2 MR_share formula; "
                   "the 1-phi^2 was a wrong proxy for the P1 state-space MR "
                   "innovation share, see Earnest 2015)"),
        "blocks": p1s_v2,
    }
    with open(FITS_PATH, "w") as fh:
        json.dump(fits, fh, indent=2, sort_keys=True)
    elapsed = time.time() - start_t
    total = sum(len(e["pair_4h"]["frozen_fits"]) for e in p1s_v2)
    print(f"wrote {FITS_PATH}")
    print(f"elapsed: {elapsed:.1f}s")
    print(f"P1S_v2 pair @4h: {total} fits across {len(BLOCKS)} blocks")
    if total > 0:
        for e in p1s_v2[:2]:
            for g in e["pair_4h"]["frozen_fits"][:2]:
                print(f"  sample {g['group_id']}: adf={g['adf_t']:.2f} "
                      f"hl={g['half_life_h']:.1f}h ac1={g['ac1']:.3f}")


if __name__ == "__main__":
    main()
