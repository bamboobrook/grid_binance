#!/usr/bin/env python3
"""Round 21 EDGE EXPLORATION: search for positive-edge mechanisms.

Path A: try C1E pair fitting at 4h and daily frequency (1h is too noisy).
Path D: expand universe to all 30 funding-DB symbols (more MR candidates).

This script is DIAGNOSTIC — it computes pair stats (ADF, half_life, MR share,
in-sample Sharpe of a simple MR entry) across multiple frequencies and a
larger universe to find which (frequency, pair) combinations show real
positive edge. Only pairs that pass ADF<-2.85 + half_life<168h AND show
positive in-sample MR Sharpe proceed to backtest.

Output: r4/edge-exploration.json with the diagnostic table.
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
OUT = ART / "r4" / "edge-exploration.json"

# 30 symbols from funding_rates_round12.db (more than the 12-symbol universe)
UNIVERSE_30 = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT",
               "ADAUSDT", "TRXUSDT", "LINKUSDT", "LTCUSDT", "BCHUSDT", "DOTUSDT",
               "AVAXUSDT", "ATOMUSDT", "NEARUSDT", "APTUSDT", "AAVEUSDT",
               "ALGOUSDT", "COMPUSDT", "UNIUSDT", "CRVUSDT", "INJUSDT",
               "DASHUSDT", "ETCUSDT", "FILUSDT", "ICPUSDT", "ANKRUSDT",
               "EGLDUSDT", "HBARUSDT", "ZECUSDT"]

# Sample windows (use first 2 years of dev window for diagnostic)
DIAG_START = 1672531200000  # 2023-01-01
DIAG_END = 1704067199999    # 2023-12-31


def load_log_prices(symbol, start_ms, end_ms, freq_hours):
    conn = sqlite3.connect(f"file:{ROOT}/data/market_data_full.db?mode=ro", uri=True)
    cur = conn.cursor()
    mod = int(freq_hours * 3600 * 1000)
    cur.execute(
        "SELECT open_time, close FROM klines WHERE symbol=? "
        "AND market_type='futures_usdt_perp' AND timeframe='1m' "
        "AND open_time>=? AND open_time<=? AND open_time % ? = 0 "
        "ORDER BY open_time", (symbol, start_ms, end_ms, mod))
    rows = cur.fetchall()
    conn.close()
    return {t: math.log(float(c)) for t, c in rows if c and float(c) > 0}


def fit_pair(la, lb):
    common = sorted(set(la) & set(lb))
    n = len(common)
    if n < 100:
        return None
    xs = [lb[t] for t in common]; ys = [la[t] for t in common]
    mx = sum(xs)/n; my = sum(ys)/n
    sxx = sum((x-mx)**2 for x in xs)
    sxy = sum((xs[i]-mx)*(ys[i]-my) for i in range(n))
    beta = sxy/sxx if sxx > 0 else 1.0
    mu = my - beta*mx
    resid = [ys[i]-beta*xs[i]-mu for i in range(n)]
    mr = sum(resid)/n; vr = sum((r-mr)**2 for r in resid)/n
    sigma = vr**0.5
    if vr <= 0 or n <= 2: return None
    phi = sum((resid[i]-mr)*(resid[i-1]-mr) for i in range(1,n))/(n*vr)
    hl = (-0.693147/math.log(phi)) if 0 < phi < 1 else 999.0
    dr = [resid[i]-resid[i-1] for i in range(1,n)]
    rl = [resid[i-1]-mr for i in range(1,n)]
    sxx2 = sum(x*x for x in rl)
    alpha = (sum(dr[i]*rl[i] for i in range(len(dr)))/sxx2) if sxx2 > 0 else 0.0
    fe = [dr[i]-alpha*rl[i] for i in range(len(dr))]
    se2 = sum(e*e for e in fe)/(len(dr)-2)
    sea = math.sqrt(se2/sxx2) if sxx2 > 0 else 1e9
    adf_t = alpha/sea if sea > 0 else 0.0
    # In-sample MR Sharpe: enter when |z|>1, exit when |z|<0.5
    z = [(r-mr)/sigma for r in resid]
    pnl = []
    pos = 0
    for i in range(1, len(z)):
        if pos == 0 and abs(z[i-1]) > 1.0:
            pos = -1 if z[i-1] > 0 else 1
        elif pos != 0 and abs(z[i-1]) < 0.5:
            pos = 0
        if pos != 0:
            pnl.append(pos * (z[i] - z[i-1]) * sigma)
    sharpe = 0.0
    if pnl:
        m = sum(pnl)/len(pnl)
        v = sum((p-m)**2 for p in pnl)/len(pnl)
        sharpe = (m / v**0.5) * (len(pnl)**0.5) if v > 0 else 0
    return {"n":n, "beta":beta, "mu":mu, "sigma":sigma,
            "half_life_h":max(1.0,hl), "adf_t":adf_t, "ac1":phi,
            "mr_sharpe": sharpe, "mr_trades": len(pnl)}


def explore_frequency(symbols, freq_hours):
    print(f"  fitting {len(symbols)} symbols @ {freq_hours}h...")
    prices = {s: load_log_prices(s, DIAG_START, DIAG_END, freq_hours)
              for s in symbols}
    results = []
    for a, b in itertools.combinations(symbols, 2):
        f = fit_pair(prices[a], prices[b])
        if f is None:
            continue
        if f["adf_t"] >= -2.85 or f["half_life_h"] >= 168:
            continue
        results.append({"pair": f"{b}_{a}", "freq_h": freq_hours, **f})
    results.sort(key=lambda r: -r["mr_sharpe"])
    return results


def main():
    start_t = time.time()
    all_results = []
    for freq in (4, 24):  # 4h and daily
        r = explore_frequency(UNIVERSE_30, freq)
        all_results.extend(r)
        print(f"  @ {freq}h: {len(r)} pairs pass ADF<-2.85 + hl<168h")
        for x in r[:5]:
            print(f"    {x['pair']}: adf={x['adf_t']:.2f} hl={x['half_life_h']:.1f}h "
                  f"mr_sharpe={x['mr_sharpe']:.2f} trades={x['mr_trades']}")
    elapsed = time.time() - start_t
    # Filter for positive MR Sharpe (real edge)
    positive_edge = [r for r in all_results if r["mr_sharpe"] > 0.5]
    positive_edge.sort(key=lambda r: -r["mr_sharpe"])
    summary = {
        "phase": "R21 edge exploration (frequency + universe expansion)",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "diagnostic_window": "2023-01-01..2023-12-31 (1 year)",
        "frequencies_tried_hours": [4, 24],
        "universe_size": len(UNIVERSE_30),
        "total_passing_pairs": len(all_results),
        "positive_edge_pairs (mr_sharpe>0.5)": len(positive_edge),
        "top_positive_edge": positive_edge[:20],
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    print(f"total passing: {len(all_results)}, positive edge: {len(positive_edge)}")
    for x in positive_edge[:5]:
        print(f"  {x['pair']} @{x['freq_h']}h: adf={x['adf_t']:.2f} hl={x['half_life_h']:.1f}h "
              f"mr_sharpe={x['mr_sharpe']:.2f}")


if __name__ == "__main__":
    main()
