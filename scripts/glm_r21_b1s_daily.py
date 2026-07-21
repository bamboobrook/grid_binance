#!/usr/bin/env python3
"""Round 21 B1S DAILY: block-specific spot/perp basis at daily frequency.

The C1E block-specific daily approach found real edge (8/11 blocks positive,
3-tier hit). B1S at 1h frequency was break-even. This script applies the SAME
block-specific daily-frequency approach to B1S:
  1. For each block, compute spot/perp basis for all 6 basis symbols at daily freq.
  2. Select the best MR basis pairs (ADF<-2.85 + hl<168h + positive MR Sharpe).
  3. Backtest with multiplier expansion.

If B1S turns positive on enough blocks, C1E+B1S combination becomes possible
(plan §12: >=2 families required).
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
DIR = ART / "g1" / "configs_b1s_daily"
DIR.mkdir(parents=True, exist_ok=True)

BASIS_SYMBOLS = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT",
                 "ADAUSDT", "TRXUSDT", "LINKUSDT", "LTCUSDT", "BCHUSDT", "DOTUSDT",
                 "AVAXUSDT", "ATOMUSDT", "NEARUSDT", "APTUSDT", "AAVEUSDT",
                 "ALGOUSDT", "COMPUSDT", "UNIUSDT"]

FREQ = 24  # daily
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


def load_basis_daily(symbol, start_ms, end_ms):
    """Load spot and perp log prices at daily frequency, compute basis."""
    conn = sqlite3.connect(f"file:{ROOT}/data/market_data_full.db?mode=ro", uri=True)
    cur = conn.cursor()
    mod = FREQ * 3600 * 1000
    cur.execute(
        "SELECT open_time, close FROM klines WHERE symbol=? "
        "AND market_type='spot' AND timeframe='1m' "
        "AND open_time>=? AND open_time<=? AND open_time % ? = 0 "
        "ORDER BY open_time", (symbol, start_ms, end_ms, mod))
    spot = {t: math.log(float(c)) for t, c in cur.fetchall() if c and float(c) > 0}
    cur.execute(
        "SELECT open_time, close FROM klines WHERE symbol=? "
        "AND market_type='futures_usdt_perp' AND timeframe='1m' "
        "AND open_time>=? AND open_time<=? AND open_time % ? = 0 "
        "ORDER BY open_time", (symbol, start_ms, end_ms, mod))
    perp = {t: math.log(float(c)) for t, c in cur.fetchall() if c and float(c) > 0}
    conn.close()
    common = sorted(set(spot) & set(perp))
    # basis = perp - spot (positive = perp premium)
    return {t: perp[t] - spot[t] for t in common}


def fit_basis_mr(basis_series):
    """Fit MR stats on basis series. Returns (mu, sigma, half_life_h, adf_t, mr_sharpe)."""
    vals = list(basis_series.values())
    n = len(vals)
    if n < 30:
        return None
    mu = sum(vals) / n
    resid = [v - mu for v in vals]
    vr = sum((r) ** 2 for r in resid) / n
    sigma = vr ** 0.5
    if vr <= 0:
        return None
    phi = sum((resid[i]) * (resid[i - 1]) for i in range(1, n)) / (n * vr)
    hl = (-0.693147 / math.log(phi)) if 0 < phi < 1 else 999.0
    # ADF
    dr = [resid[i] - resid[i - 1] for i in range(1, n)]
    rl = [resid[i - 1] for i in range(1, n)]
    sxx2 = sum(x * x for x in rl)
    if sxx2 <= 0:
        return None
    alpha = sum(dr[i] * rl[i] for i in range(len(dr))) / sxx2
    fe = [dr[i] - alpha * rl[i] for i in range(len(dr))]
    se2 = sum(e * e for e in fe) / (len(dr) - 2)
    sea = math.sqrt(se2 / sxx2)
    adf_t = alpha / sea if sea > 0 else 0.0
    # MR Sharpe
    z = [(v - mu) / sigma for v in vals]
    pnl = []
    pos = 0
    for i in range(1, len(z)):
        if pos == 0 and abs(z[i - 1]) > 1.0:
            pos = -1 if z[i - 1] > 0 else 1
        elif pos != 0 and abs(z[i - 1]) < 0.5:
            pos = 0
        if pos != 0:
            pnl.append(pos * (z[i] - z[i - 1]) * sigma)
    sharpe = 0.0
    if len(pnl) > 5:
        m = sum(pnl) / len(pnl)
        v = sum((p - m) ** 2 for p in pnl) / len(pnl)
        sharpe = (m / v ** 0.5) * (len(pnl) ** 0.5) if v > 0 else 0
    return {"mu": mu, "sigma": sigma, "half_life_h": hl * 24,
            "adf_t": adf_t, "mr_sharpe": sharpe}


def select_block_basis_pairs(block):
    """For each block, select the best basis symbols by MR Sharpe."""
    basis_by_sym = {s: load_basis_daily(s, block["fit_start_ms"], block["fit_end_ms"])
                    for s in BASIS_SYMBOLS}
    candidates = []
    for sym in BASIS_SYMBOLS:
        f = fit_basis_mr(basis_by_sym[sym])
        if f is None:
            continue
        if f["adf_t"] >= -2.85 or f["half_life_h"] >= 168:
            continue
        if f["mr_sharpe"] < 0.3:
            continue
        candidates.append((sym, f))
    candidates.sort(key=lambda x: -x[1]["mr_sharpe"])
    # Select top-6 (each symbol is one group)
    groups = []
    for sym, f in candidates[:6]:
        groups.append({
            "group_id": f"B1Sd_{block['block_id']}_{sym}",
            "legs": [sym, sym],
            "leg_markets": [
                {"venue": "binance", "market_type": "spot", "symbol": sym},
                {"venue": "binance", "market_type": "futures_usdt_perp", "symbol": sym},
            ],
            "leg_direction_signs": [1, -1],
            "betas": [1.0, -1.0],
            "mus": [f["mu"], -f["mu"]],
            "residual_sigma": f["sigma"],
            "half_life_h": f["half_life_h"],
            "weights": [0.5, 0.5],
            "fit_sha256": hashlib.sha256(
                json.dumps(f, sort_keys=True).encode()).hexdigest()[:16],
        })
    return groups


def main():
    start_t = time.time()
    ln = Launcher()
    results = []
    for block in BLOCKS:
        groups = select_block_basis_pairs(block)
        if not groups:
            print(f"  {block['block_id']}: no valid basis pairs")
            continue
        print(f"  {block['block_id']}: {len(groups)} basis groups")
        for mult in [1.5, 2.0, 2.5, 3.0]:
            for fo in [30.0, 60.0, 100.0]:
                for ez in [1.0, 1.5]:
                    tag = (f"g1bd_{block['block_id']}_m{mult:.1f}_"
                           f"fo{int(fo)}_ez{ez:.1f}").replace(".", "p")
                    cfg = {"synchronized_cycle": {
                        "family": "B1S", "bar_boundary_minutes": FREQ * 60,
                        "fit_lookback_days": 180, "entry_z": ez,
                        "so_residual_step_z": 0.5, "group_fo_quote": fo,
                        "multiplier": mult, "max_legs": 4, "exit_z": 0.5,
                        "tp_net_bps_floor": 25, "leverage": LEV,
                        "group_gross_cap_pct": CAP, "cycle_deadline_h": 168,
                        "inventory_reservation_skew_k": 0,
                        "jump_first_passage_gate": None,
                        "regime_envelope": None, "c1_scheduler": None,
                    }, "fits": groups, "budget_quote": 500.0}
                    cfg_path = DIR / f"{tag}.json"
                    cfg_path.write_text(json.dumps(cfg, indent=2, sort_keys=True))
                    cfg_sha = hashlib.sha256(cfg_path.read_bytes()).hexdigest()
                    fp = ln.fingerprint(
                        family="B1S", cycle_topology="sync_b1s_daily",
                        trigger_contract_sha=f"ez{ez}_m{mult}",
                        fit_contract_sha=hashlib.sha256(
                            json.dumps(groups, sort_keys=True).encode()).hexdigest()[:16],
                        universe_group_weights_sha=f"{len(groups)}g",
                        scheduler_sha=f"lev{LEV}",
                        resolved_config_sha=cfg_sha[:16],
                        effective_config_sha=cfg_sha[:16],
                        cost_model_sha="fee7_slip5",
                        engine_sha=engine_sha(), market_data_sha=md_sha(),
                        funding_data_sha=fd_sha(),
                        exchange_filter_snapshot_sha="default",
                        maintenance_tiers_sha="0.5pct",
                        borrow_snapshot_sha=None,
                        window=(block["test_start_ms"], block["test_end_ms"]),
                        budget=500.0, fold=block["block_id"],
                        block="g1bd", seed=20261102)
                    if ln.is_duplicate(fp):
                        continue
                    row = ln.run_synchronized_cycle(
                        experiment_id=tag, parent_id=None, fingerprint=fp,
                        config_path=cfg_path, budget=500.0,
                        start_ms=block["test_start_ms"], end_ms=block["test_end_ms"],
                        fit_start_ms=block["fit_start_ms"], fit_end_ms=block["fit_end_ms"],
                        purge_ms=86_400_000, timeout_s=600)
                    if not row or row.get("skipped"):
                        continue
                    if row.get("status") != "complete":
                        continue
                    m = row.get("metrics", {}) or {}
                    ann = m.get("annualized_return_pct") or 0
                    dd = m.get("max_drawdown_pct") or 0
                    rec = {"tag": tag, "block": block["block_id"],
                           "mult": mult, "fo": fo, "ez": ez,
                           "ann": ann, "dd": dd,
                           "sym": m.get("actual_symbols"),
                           "so": m.get("groups_with_so")}
                    results.append(rec)
                    if ann > 0:
                        print(f"    {tag}: ann={ann:.1f}% dd={dd:.2f}% +")
    elapsed = time.time() - start_t
    by_block = {}
    for r in results:
        by_block.setdefault(r["block"], []).append(r)
    cross_fit = {}
    for blk, rows in sorted(by_block.items()):
        positive = sum(1 for r in rows if r["ann"] > 0)
        best = max(rows, key=lambda r: r["ann"]) if rows else None
        cross_fit[blk] = {"positive": positive, "total": len(rows),
                          "best_ann": best["ann"] if best else 0}
    summary = {
        "phase": "R21 B1S daily block-specific (push B1S above break-even)",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "total_complete": len(results),
        "cross_fit_per_block": cross_fit,
        "blocks_positive": sum(1 for v in cross_fit.values() if v["positive"] > 0),
        "blocks_total": len(cross_fit),
    }
    OUT = ART / "g1" / "gates" / "g1_b1s_daily.json"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    print(f"total_complete: {len(results)}")
    print(f"blocks_positive: {summary['blocks_positive']}/{summary['blocks_total']}")
    for blk, cf in cross_fit.items():
        print(f"  {blk}: {cf['positive']}/{cf['total']} pos, best={cf['best_ann']:.1f}%")


if __name__ == "__main__":
    main()
