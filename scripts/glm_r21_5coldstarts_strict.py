#!/usr/bin/env python3
"""Round 21 STRICT 5 cold-start: each cold-start offset gets its OWN fit
window AND test window (plan §1 strict definition).

Plan §1 requires >=4/5 cold starts positive AND report 5/5. The previous
5-cold-start ran the SAME test block from 5 offsets (ann identical). The
strict definition (per plan §1 + R3 manifest cold_start_offsets) requires:
  - cold-start i has fit_window = [dev_start, cold_start_i]
  - cold-start i has test_window = [cold_start_i, cold_start_i + 90d]
  - the pairs are RE-FIT on each fit_window (different history => different
    beta/mu/sigma; this is the strict causal contract)

The 5 pre-registered cold-start offsets from R3 manifest:
  0, 30, 60, 90, 120 days from dev_start (2023-01-01).

For each tier's best config, run 5 independent (fit, test) replays.
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
MANIFEST = json.load(open(ART / "r3" / "gates" / "causal_crossfit_manifest.json"))
COLD_STARTS = MANIFEST["cold_start_offsets"]
DIR = ART / "g1" / "configs_5cs_strict"
DIR.mkdir(parents=True, exist_ok=True)

DAY_MS = 86_400_000
TEST_BLOCK_DAYS = 90
DEV_START = 1672531200000  # 2023-01-01

UNIVERSE = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT",
            "ADAUSDT", "TRXUSDT", "LINKUSDT", "LTCUSDT", "BCHUSDT", "DOTUSDT"]

BEST_CONFIGS = {
    "conservative": {"mult": 2.30, "fo": 144.0, "ez": 1.05, "cap": 300.0},
    "balanced": {"mult": 2.75, "fo": 170.0, "ez": 0.90, "cap": 400.0},
    "aggressive": {"mult": 2.85, "fo": 200.0, "ez": 0.90, "cap": 500.0},
}


def engine_sha() -> str:
    return (sha256_file(ROOT / "target/release/synchronized_cycle_replay")
            or "unknown")[:16]


def md_sha() -> str:
    p = ROOT / "data/market_data_full.db"
    st = p.stat()
    return f"size={st.st_size},mtime={int(st.st_mtime)}"[:32]


def fd_sha() -> str:
    return (sha256_file(ROOT / "data/funding_rates_round12.db") or "x")[:16]


def load_log_prices_1h(symbol, start_ms, end_ms):
    conn = sqlite3.connect(f"file:{ROOT}/data/market_data_full.db?mode=ro",
                           uri=True)
    cur = conn.cursor()
    cur.execute(
        "SELECT open_time, close FROM klines WHERE symbol=? "
        "AND market_type='futures_usdt_perp' AND timeframe='1m' "
        "AND open_time>=? AND open_time<=? AND open_time % 3600000=0 "
        "ORDER BY open_time", (symbol, start_ms, end_ms))
    rows = cur.fetchall()
    conn.close()
    return {t: math.log(float(c)) for t, c in rows if c and float(c) > 0}


def fit_pair_ols(la, lb):
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
    phi = sum((resid[i] - mr) * (resid[i - 1] - mr) for i in range(1, n)) / (n * vr)
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


def refit_pairs_for_cold_start(cold_start_ms):
    """Re-fit C1E disjoint pairs on the fit window [dev_start, cold_start-1d].
    Each cold-start offset sees a different history length, so beta/mu/sigma
    are different — this is the strict causal contract."""
    fit_end = cold_start_ms - DAY_MS  # 1-day purge
    prices = {s: load_log_prices_1h(s, DEV_START, fit_end) for s in UNIVERSE}
    pair_fits = []
    for a, b in itertools.combinations(UNIVERSE, 2):
        f = fit_pair_ols(prices[a], prices[b])
        if f is None:
            continue
        if f["adf_t"] >= -2.85 or f["half_life_h"] >= 168:
            continue
        pair_fits.append((a, b, f))
    pair_fits.sort(key=lambda x: x[2]["adf_t"])
    used = set()
    groups = []
    for a, b, f in pair_fits:
        if a in used or b in used:
            continue
        groups.append({
            "group_id": f"C1Ecs_{a}_{b}",
            "legs": [b, a], "leg_markets": [],
            "leg_direction_signs": [1, -1],
            "betas": [f["beta"]], "mus": [f["mu"]],
            "residual_sigma": f["sigma"], "half_life_h": f["half_life_h"],
            "weights": [],
            "fit_sha256": hashlib.sha256(
                json.dumps(f, sort_keys=True).encode()).hexdigest()[:16],
        })
        used.update({a, b})
        if len(groups) >= 4:
            break
    return groups


def main():
    start_t = time.time()
    ln = Launcher()
    results = {t: [] for t in BEST_CONFIGS}
    for tier, cfg_params in BEST_CONFIGS.items():
        for cs in COLD_STARTS:
            cs_ms = cs["cold_start_ms"]
            test_start = cs_ms
            test_end = cs_ms + TEST_BLOCK_DAYS * DAY_MS - 1
            # Refit pairs on [dev_start, cs-1d]
            fits = refit_pairs_for_cold_start(cs_ms)
            if not fits:
                results[tier].append({
                    "offset_days": cs["offset_days"], "status": "no_fits",
                    "positive": False})
                continue
            cfg = {"synchronized_cycle": {
                "family": "C1E", "bar_boundary_minutes": 60,
                "fit_lookback_days": (cs["offset_days"] + 180),
                "entry_z": cfg_params["ez"], "so_residual_step_z": 0.5,
                "group_fo_quote": cfg_params["fo"],
                "multiplier": cfg_params["mult"], "max_legs": 4, "exit_z": 0.5,
                "tp_net_bps_floor": 25, "leverage": 10,
                "group_gross_cap_pct": cfg_params["cap"],
                "cycle_deadline_h": 168, "inventory_reservation_skew_k": 0,
                "jump_first_passage_gate": None, "regime_envelope": None,
                "c1_scheduler": None,
            }, "fits": fits, "budget_quote": 500.0}
            tag = f"5css_{tier}_cs{cs['offset_days']}d"
            cfg_path = DIR / f"{tag}.json"
            cfg_path.write_text(json.dumps(cfg, indent=2, sort_keys=True))
            cfg_sha = hashlib.sha256(cfg_path.read_bytes()).hexdigest()
            fp = ln.fingerprint(
                family="C1E", cycle_topology="sync_5cs_strict",
                trigger_contract_sha=f"ez{cfg_params['ez']}_cs{cs['offset_days']}",
                fit_contract_sha=hashlib.sha256(
                    json.dumps(fits, sort_keys=True).encode()).hexdigest()[:16],
                universe_group_weights_sha=f"{len(fits)}g",
                scheduler_sha="lev10", resolved_config_sha=cfg_sha[:16],
                effective_config_sha=cfg_sha[:16], cost_model_sha="fee7_slip5",
                engine_sha=engine_sha(), market_data_sha=md_sha(),
                funding_data_sha=fd_sha(),
                exchange_filter_snapshot_sha="default",
                maintenance_tiers_sha="0.5pct", borrow_snapshot_sha=None,
                window=(test_start, test_end), budget=500.0,
                fold=f"cs{cs['offset_days']}", block="g1css", seed=20261101)
            if ln.is_duplicate(fp):
                # find prior row
                row = ln._last_row_for(tag) or {}
            else:
                row = ln.run_synchronized_cycle(
                    experiment_id=tag, parent_id=None, fingerprint=fp,
                    config_path=cfg_path, budget=500.0,
                    start_ms=test_start, end_ms=test_end,
                    fit_start_ms=DEV_START, fit_end_ms=cs_ms - DAY_MS,
                    purge_ms=DAY_MS, timeout_s=600)
            if row:
                m = row.get("metrics", {}) or {}
                ann = m.get("annualized_return_pct")
                results[tier].append({
                    "offset_days": cs["offset_days"],
                    "ann_pct": ann, "max_dd_pct": m.get("max_drawdown_pct"),
                    "actual_symbols": m.get("actual_symbols"),
                    "groups_with_so": m.get("groups_with_so"),
                    "status": row.get("status"),
                    "positive": ann is not None and ann > 0,
                    "fit_pairs": len(fits),
                    "test_window": [test_start, test_end],
                })
                print(f"  {tier} cs={cs['offset_days']}d: ann={ann} "
                      f"sym={m.get('actual_symbols')} fits={len(fits)} "
                      f"status={row.get('status')}")
    elapsed = time.time() - start_t
    summary = {
        "phase": "R21 STRICT 5 cold-start (independent fit+test per offset)",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "strict_definition": ("each cold-start offset has its OWN fit window "
                              "[dev_start, offset-1d] AND test window [offset, "
                              "offset+90d]; pairs RE-FIT on each fit window"),
        "cold_start_offsets": COLD_STARTS,
        "best_configs": BEST_CONFIGS,
        "results": results,
        "positivity": {},
    }
    for tier, rows in results.items():
        positive = sum(1 for r in rows if r.get("positive"))
        total = len(rows)
        summary["positivity"][tier] = {
            "positive": positive, "total": total, "rate": f"{positive}/{total}",
            "passes_4_of_5": positive >= 4 and total >= 4,
            "five_of_five": positive == 5 and total == 5,
        }
    OUT = ART / "g1" / "gates" / "five_cold_starts_strict.json"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    for tier, p in summary["positivity"].items():
        print(f"  {tier}: {p['positive']}/{p['total']} positive "
              f"(4/5: {p['passes_4_of_5']}, 5/5: {p['five_of_five']})")


if __name__ == "__main__":
    main()
