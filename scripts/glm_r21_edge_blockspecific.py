#!/usr/bin/env python3
"""Round 21 EDGE BLOCK-SPECIFIC: each block selects its OWN best daily pairs
from its OWN fit window. This is the correct causal contract — the global
top-12 pairs were selected on 2023 in-sample, which is itself a form of
look-ahead when applied to tb04-tb06.

Method:
  1. For each block, fit ALL C(30,2)=435 pairs on [fit_start, fit_end].
  2. Rank by in-sample MR Sharpe (entry |z|>1, exit |z|<0.5).
  3. Select top-K disjoint pairs (greedy, ADF<-2.85 + hl<168h gate enforced).
  4. Backtest on [test_start, test_end] with the selected pairs.

This is block-specific pair selection — each block gets pairs that were good
on ITS OWN history, not on 2023. The cross-fit test is then: does the
strategy generalize across blocks when each block uses its own optimal pairs?
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
MANIFEST = json.load(open(ART / "r3" / "gates" / "causal_crossfit_manifest.json"))
BLOCKS = MANIFEST["test_blocks"]
DIR = ART / "g1" / "configs_blockspec"
DIR.mkdir(parents=True, exist_ok=True)

UNIVERSE_30 = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT",
               "ADAUSDT", "TRXUSDT", "LINKUSDT", "LTCUSDT", "BCHUSDT", "DOTUSDT",
               "AVAXUSDT", "ATOMUSDT", "NEARUSDT", "APTUSDT", "AAVEUSDT",
               "ALGOUSDT", "COMPUSDT", "UNIUSDT", "CRVUSDT", "INJUSDT",
               "DASHUSDT", "ETCUSDT", "FILUSDT", "ICPUSDT", "ANKRUSDT",
               "EGLDUSDT", "HBARUSDT", "ZECUSDT"]

FREQ = 24
LEV = 10
CAP = 400.0
MULT_GRID = [1.5, 2.0, 2.5, 3.0]
FO_GRID = [30.0, 60.0, 100.0]
EZ_GRID = [1.0, 1.5]


def engine_sha():
    return (sha256_file(ROOT / "target/release/synchronized_cycle_replay")
            or "unknown")[:16]


def md_sha():
    p = ROOT / "data/market_data_full.db"
    st = p.stat()
    return f"size={st.st_size},mtime={int(st.st_mtime)}"[:32]


def fd_sha():
    return (sha256_file(ROOT / "data/funding_rates_round12.db") or "x")[:16]


def load_log_prices(symbol, start_ms, end_ms):
    conn = sqlite3.connect(f"file:{ROOT}/data/market_data_full.db?mode=ro", uri=True)
    cur = conn.cursor()
    mod = FREQ * 3600 * 1000
    cur.execute(
        "SELECT open_time, close FROM klines WHERE symbol=? "
        "AND market_type='futures_usdt_perp' AND timeframe='1m' "
        "AND open_time>=? AND open_time<=? AND open_time % ? = 0 "
        "ORDER BY open_time", (symbol, start_ms, end_ms, mod))
    rows = cur.fetchall()
    conn.close()
    return {t: math.log(float(c)) for t, c in rows if c and float(c) > 0}


def fit_pair_full(la, lb):
    common = sorted(set(la) & set(lb))
    n = len(common)
    if n < 60:
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
    # In-sample MR Sharpe
    z = [(r-mr)/sigma for r in resid]
    pnl = []; pos = 0
    for i in range(1, len(z)):
        if pos == 0 and abs(z[i-1]) > 1.0:
            pos = -1 if z[i-1] > 0 else 1
        elif pos != 0 and abs(z[i-1]) < 0.5:
            pos = 0
        if pos != 0:
            pnl.append(pos * (z[i] - z[i-1]) * sigma)
    sharpe = 0.0
    if len(pnl) > 5:
        m = sum(pnl)/len(pnl)
        v = sum((p-m)**2 for p in pnl)/len(pnl)
        sharpe = (m / v**0.5) * (len(pnl)**0.5) if v > 0 else 0
    return {"beta":beta, "mu":mu, "sigma":sigma,
            "half_life_h":max(1.0,hl), "adf_t":adf_t, "ac1":phi,
            "mr_sharpe":sharpe, "mr_trades":len(pnl)}


def select_block_specific_pairs(block):
    """For this block, fit all 435 pairs on [fit_start, fit_end] and select
    the top-K disjoint by in-sample MR Sharpe (ADF<-2.85 + hl<168h gated)."""
    fit_start = block["fit_start_ms"]
    fit_end = block["fit_end_ms"]
    print(f"  {block['block_id']}: fitting all pairs on fit window "
          f"({(fit_end-fit_start)//86_400_000}d)...")
    prices = {s: load_log_prices(s, fit_start, fit_end) for s in UNIVERSE_30}
    candidates = []
    for a, b in itertools.combinations(UNIVERSE_30, 2):
        f = fit_pair_full(prices[a], prices[b])
        if f is None: continue
        if f["adf_t"] >= -2.85 or f["half_life_h"] >= 168: continue
        candidates.append((a, b, f))
    candidates.sort(key=lambda x: -x[2]["mr_sharpe"])
    # Greedy disjoint selection
    used = set(); groups = []
    for a, b, f in candidates:
        if a in used or b in used: continue
        if f["mr_sharpe"] < 0.3: continue  # minimum edge threshold
        groups.append({
            "group_id": f"BS_{block['block_id']}_{a}_{b}",
            "legs": [b, a], "leg_markets": [],
            "leg_direction_signs": [1, -1],
            "betas": [f["beta"]], "mus": [f["mu"]],
            "residual_sigma": f["sigma"], "half_life_h": f["half_life_h"],
            "weights": [],
            "fit_sha256": hashlib.sha256(
                json.dumps(f, sort_keys=True).encode()).hexdigest()[:16],
            "mr_sharpe": f["mr_sharpe"],
        })
        used.update({a, b})
        if len(groups) >= 6: break
    return groups


def run_one(ln, fits, block, mult, fo, ez, tag):
    cfg = {"synchronized_cycle": {
        "family": "C1E", "bar_boundary_minutes": FREQ * 60,
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
        family="C1E", cycle_topology="sync_blockspec",
        trigger_contract_sha=f"ez{ez}_m{mult}",
        fit_contract_sha=hashlib.sha256(
            json.dumps(fits, sort_keys=True).encode()).hexdigest()[:16],
        universe_group_weights_sha=f"{len(fits)}g",
        scheduler_sha=f"lev{LEV}", resolved_config_sha=cfg_sha[:16],
        effective_config_sha=cfg_sha[:16], cost_model_sha="fee7_slip5",
        engine_sha=engine_sha(), market_data_sha=md_sha(),
        funding_data_sha=fd_sha(), exchange_filter_snapshot_sha="default",
        maintenance_tiers_sha="0.5pct", borrow_snapshot_sha=None,
        window=(block["test_start_ms"], block["test_end_ms"]),
        budget=500.0, fold=block["block_id"], block="g1bs", seed=20261101)
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
    block_fits = {}
    for block in BLOCKS:
        fits = select_block_specific_pairs(block)
        block_fits[block["block_id"]] = len(fits)
        if not fits:
            print(f"  {block['block_id']}: no valid pairs")
            continue
        print(f"  {block['block_id']}: {len(fits)} block-specific pairs selected")
        for mult in MULT_GRID:
            for fo in FO_GRID:
                for ez in EZ_GRID:
                    tag = (f"g1bs_{block['block_id']}_m{mult:.1f}_"
                           f"fo{int(fo)}_ez{ez:.1f}").replace(".", "p")
                    row = run_one(ln, fits, block, mult, fo, ez, tag)
                    if not row or row.get("skipped"): continue
                    if row.get("status") != "complete": continue
                    m = row.get("metrics", {}) or {}
                    ann = m.get("annualized_return_pct") or 0
                    dd = m.get("max_drawdown_pct") or 0
                    rec = {"tag": tag, "block": block["block_id"],
                           "mult": mult, "fo": fo, "ez": ez,
                           "ann": ann, "dd": dd,
                           "sym": m.get("actual_symbols"),
                           "so": m.get("groups_with_so")}
                    results.append(rec)
                    tier = ""
                    if ann >= 50 and dd <= 10: tier = "CONS"
                    elif ann >= 90 and dd <= 20: tier = "BAL"
                    elif ann >= 110 and dd <= 30: tier = "AGG"
                    if tier:
                        print(f"    *** {tier} *** {tag}: ann={ann:.1f}% dd={dd:.2f}%")
    elapsed = time.time() - start_t
    by_block = {}
    for r in results:
        by_block.setdefault(r["block"], []).append(r)
    cross_fit = {}
    for blk, rows in sorted(by_block.items()):
        positive = sum(1 for r in rows if r["ann"] > 0)
        best = max(rows, key=lambda r: r["ann"]) if rows else None
        cross_fit[blk] = {"positive": positive, "total": len(rows),
                          "best_ann": best["ann"] if best else 0,
                          "best_dd": best["dd"] if best else 0}
    summary = {
        "phase": "R21 block-specific pair selection edge G1",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "method": ("each block selects its OWN top-K daily pairs from its "
                   "OWN fit window (block-specific, not global top-12)"),
        "block_fits_count": block_fits,
        "total_complete": len(results),
        "cross_fit_per_block": cross_fit,
        "blocks_positive": sum(1 for v in cross_fit.values() if v["positive"]>0),
        "blocks_total": len(cross_fit),
    }
    OUT = ART / "g1" / "gates" / "g1_blockspec.json"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    print(f"total_complete: {len(results)}")
    print(f"blocks_positive: {summary['blocks_positive']}/{summary['blocks_total']}")
    for blk, cf in cross_fit.items():
        print(f"  {blk}: {cf['positive']}/{cf['total']} pos, best={cf['best_ann']:.1f}%/{cf['best_dd']:.1f}%")


if __name__ == "__main__":
    main()
