#!/usr/bin/env python3
"""Round 21 EDGE G1: backtest the top positive-edge pairs (4h/daily freq,
expanded universe) across all R3 blocks with strict cross-fit.

The edge-exploration found 167 pairs with positive MR Sharpe at 4h/daily
frequency on the expanded 30-symbol universe. This script:
  1. Refits the top-K pairs on EACH R3 block's fit window (strict causal).
  2. Runs C1E Martingale backtest on each block's test window.
  3. Requires >=4/5 blocks positive for the strategy to be cross-fit-valid.

The previous C1E used 1h frequency on 12 symbols and lost money with corrected
pairs. This uses 4h frequency (stronger MR) on 30 symbols (more candidates).
"""
from __future__ import annotations

import hashlib
import itertools
import json
import math
import sqlite3
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from glm_r21_launcher import Launcher, sha256_file  # noqa: E402

ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round21"
FITS = json.load(open(ART / "r4" / "families-fit.json"))
EDGE = json.load(open(ART / "r4" / "edge-exploration.json"))
MANIFEST = json.load(open(ART / "r3" / "gates" / "causal_crossfit_manifest.json"))
BLOCKS = MANIFEST["test_blocks"]
DIR = ART / "g1" / "configs_edge"
DIR.mkdir(parents=True, exist_ok=True)

# Top pairs from edge exploration (mix of 4h and daily, top MR Sharpe)
TOP_PAIRS_4H = [r["pair"] for r in EDGE["top_positive_edge"]
                if r["freq_h"] == 4][:12]
TOP_PAIRS_24H = [r["pair"] for r in EDGE["top_positive_edge"]
                 if r["freq_h"] == 24][:12]

LEV = 10
CAP = 400.0


def engine_sha():
    return (sha256_file(ROOT / "target/release/synchronized_cycle_replay")
            or "unknown")[:16]


def md_sha():
    p = ROOT / "data/market_data_full.db"
    st = p.stat()
    return f"size={st.st_size},mtime={int(st.st_mtime)}"[:32]


def fd_sha():
    return (sha256_file(ROOT / "data/funding_rates_round12.db") or "x")[:16]


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
    return {"beta":beta, "mu":mu, "sigma":sigma,
            "half_life_h":max(1.0,hl), "adf_t":adf_t, "ac1":phi}


def refit_block_pairs(block, pairs_list, freq_hours):
    """Refit the candidate pairs on this block's fit window; keep those that
    pass ADF<-2.85 + hl<168h. Returns list of fit groups."""
    fit_start = block["fit_start_ms"]
    fit_end = block["fit_end_ms"]
    # Load all symbols involved
    symbols = set()
    for p in pairs_list:
        a, b = p.split("_")
        symbols.add(a); symbols.add(b)
    prices = {s: load_log_prices(s, fit_start, fit_end, freq_hours) for s in symbols}
    groups = []
    used = set()
    for p in pairs_list:
        a, b = p.split("_")
        if a in used or b in used:
            continue
        f = fit_pair(prices[a], prices[b])
        if f is None: continue
        if f["adf_t"] >= -2.85 or f["half_life_h"] >= 168:
            continue
        groups.append({
            "group_id": f"EDG_{a}_{b}",
            "legs": [b, a], "leg_markets": [],
            "leg_direction_signs": [1, -1],
            "betas": [f["beta"]], "mus": [f["mu"]],
            "residual_sigma": f["sigma"], "half_life_h": f["half_life_h"],
            "weights": [],
            "fit_sha256": hashlib.sha256(
                json.dumps(f, sort_keys=True).encode()).hexdigest()[:16],
        })
        used.update({a, b})
        if len(groups) >= 6: break
    return groups


def run_one(ln, fits, block, mult, fo, ez, freq_hours, tag):
    cfg = {"synchronized_cycle": {
        "family": "C1E", "bar_boundary_minutes": freq_hours * 60,
        "fit_lookback_days": 180, "entry_z": ez, "so_residual_step_z": 0.5,
        "group_fo_quote": fo, "multiplier": mult, "max_legs": 4,
        "exit_z": 0.5, "tp_net_bps_floor": 25, "leverage": LEV,
        "group_gross_cap_pct": CAP, "cycle_deadline_h": 168,
        "inventory_reservation_skew_k": 0, "jump_first_passage_gate": None,
        "regime_envelope": None, "c1_scheduler": None,
    }, "fits": fits, "budget_quote": 500.0}
    cfg_path = DIR / f"{tag}.json"
    cfg_path.write_text(json.dumps(cfg, indent=2, sort_keys=True))
    cfg_sha = hashlib.sha256(cfg_path.read_bytes()).hexdigest()
    fp = ln.fingerprint(
        family="C1E", cycle_topology="sync_edge",
        trigger_contract_sha=f"ez{ez}_m{mult}_f{freq_hours}",
        fit_contract_sha=hashlib.sha256(
            json.dumps(fits, sort_keys=True).encode()).hexdigest()[:16],
        universe_group_weights_sha=f"{len(fits)}g",
        scheduler_sha=f"lev{LEV}", resolved_config_sha=cfg_sha[:16],
        effective_config_sha=cfg_sha[:16], cost_model_sha="fee7_slip5",
        engine_sha=engine_sha(), market_data_sha=md_sha(),
        funding_data_sha=fd_sha(), exchange_filter_snapshot_sha="default",
        maintenance_tiers_sha="0.5pct", borrow_snapshot_sha=None,
        window=(block["test_start_ms"], block["test_end_ms"]),
        budget=500.0, fold=block["block_id"], block="g1e", seed=20261101)
    if ln.is_duplicate(fp):
        return None
    return ln.run_synchronized_cycle(
        experiment_id=tag, parent_id=None, fingerprint=fp,
        config_path=cfg_path, budget=500.0,
        start_ms=block["test_start_ms"], end_ms=block["test_end_ms"],
        fit_start_ms=block["fit_start_ms"], fit_end_ms=block["fit_end_ms"],
        purge_ms=86_400_000, timeout_s=600)


def main():
    start_t = time.time()
    ln = Launcher()
    results = []
    # Use top-12 daily pairs (strongest MR signal). daily = bar_boundary 1440min
    # The bar_boundary_minutes must be set to match the fit frequency so the
    # engine samples at the same rate.
    pairs = TOP_PAIRS_24H[:12]
    freq = 24
    print(f"using top-{len(pairs)} daily pairs: {pairs[:5]}...")
    for block in BLOCKS:
        fits = refit_block_pairs(block, pairs, freq)
        if not fits:
            print(f"  {block['block_id']}: no valid pairs after refit")
            continue
        print(f"  {block['block_id']}: {len(fits)} valid pairs after refit")
        # Run with conservative mult first to check edge
        for mult in [1.5, 2.0, 2.5]:
            for fo in [30.0, 60.0]:
                for ez in [1.0, 1.5]:
                    tag = (f"g1e_{block['block_id']}_m{mult:.1f}_"
                           f"fo{int(fo)}_ez{ez:.1f}_d").replace(".", "p")
                    row = run_one(ln, fits, block, mult, fo, ez, freq, tag)
                    if not row or row.get("skipped"):
                        continue
                    if row.get("status") == "complete":
                        m = row.get("metrics", {}) or {}
                        ann = m.get("annualized_return_pct") or 0
                        dd = m.get("max_drawdown_pct") or 0
                        rec = {"tag": tag, "block": block["block_id"],
                               "mult": mult, "fo": fo, "ez": ez,
                               "ann": ann, "dd": dd, "freq": freq,
                               "sym": m.get("actual_symbols"),
                               "so": m.get("groups_with_so")}
                        results.append(rec)
                        marker = "+" if ann > 0 else " "
                        print(f"    {tag}: ann={ann:>7.2f}% dd={dd:>5.2f}% "
                              f"sym={rec['sym']} so={rec['so']} {marker}")
    elapsed = time.time() - start_t
    # Cross-fit positivity per block
    by_block = {}
    for r in results:
        by_block.setdefault(r["block"], []).append(r)
    cross_fit = {}
    for blk, rows in by_block.items():
        positive = sum(1 for r in rows if r["ann"] > 0)
        cross_fit[blk] = {"positive": positive, "total": len(rows),
                          "best_ann": max((r["ann"] for r in rows), default=0)}
    summary = {
        "phase": "R21 edge G1 (daily freq, expanded universe, strict cross-fit)",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "pairs_used": pairs,
        "frequency_hours": freq,
        "total_complete": len(results),
        "cross_fit_per_block": cross_fit,
        "blocks_positive": sum(1 for v in cross_fit.values() if v["positive"] > 0),
        "blocks_total": len(cross_fit),
    }
    OUT = ART / "g1" / "gates" / "g1_edge.json"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    print(f"total_complete: {len(results)}")
    print(f"blocks with any positive: {summary['blocks_positive']}/{summary['blocks_total']}")
    for blk, cf in cross_fit.items():
        print(f"  {blk}: {cf['positive']}/{cf['total']} positive, best_ann={cf['best_ann']:.2f}%")


if __name__ == "__main__":
    main()
