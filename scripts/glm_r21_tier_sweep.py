#!/usr/bin/env python3
"""Round 21 G1 TIER SWEEP: balanced (ann>=90/dd<=20) + aggressive (ann>=110/
dd<=30) tier search.

Conservative tier (ann>=50/dd<=10) already hit at m=2.30/fo=144/ez=1.05.
Smoke test showed m=2.5 -> ann=80.39%/dd=14.41% (dd in balanced range, ann
~10pp short). This sweep covers:
  - balanced: m=2.4-2.8 step 0.05, fo=170-230 step 10, ez=0.9-1.2 step 0.05
  - aggressive: m=2.8-3.4 step 0.1, fo=220-320 step 20, ez=0.8-1.1

All FULL backtests through the central launcher (no fast-screen).
"""
from __future__ import annotations

import hashlib
import json
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from glm_r21_launcher import Launcher, sha256_file  # noqa: E402

ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round21"
FITS = json.load(open(ART / "r4" / "families-fit.json"))
DIR = ART / "g1" / "configs_tier"
DIR.mkdir(parents=True, exist_ok=True)

# Balanced-tier fine grid
BAL_MULT = [2.40, 2.45, 2.50, 2.55, 2.60, 2.65, 2.70, 2.75, 2.80]
BAL_FO = [170.0, 180.0, 190.0, 200.0, 210.0, 220.0, 230.0]
BAL_EZ = [0.90, 0.95, 1.00, 1.05, 1.10, 1.15, 1.20]
# Aggressive-tier grid
AGG_MULT = [2.80, 2.90, 3.00, 3.10, 3.20, 3.30, 3.40]
AGG_FO = [220.0, 240.0, 260.0, 280.0, 300.0, 320.0]
AGG_EZ = [0.80, 0.90, 1.00, 1.10]
CAP = 400.0
LEV = 10
BUDGETS = [500]
BLOCKS = ["tb01", "tb02"]


def engine_sha() -> str:
    return (sha256_file(ROOT / "target/release/synchronized_cycle_replay")
            or "unknown")[:16]


def md_sha() -> str:
    p = ROOT / "data/market_data_full.db"
    st = p.stat()
    return f"size={st.st_size},mtime={int(st.st_mtime)}"[:32]


def fd_sha() -> str:
    return (sha256_file(ROOT / "data/funding_rates_round12.db") or "x")[:16]


def run_one(ln, fits, block, mult, fo, ez, budget, cap, lev, tag):
    cfg = {"synchronized_cycle": {
        "family": "C1E", "bar_boundary_minutes": 60,
        "fit_lookback_days": 180, "entry_z": ez, "so_residual_step_z": 0.5,
        "group_fo_quote": fo, "multiplier": mult, "max_legs": 4, "exit_z": 0.5,
        "tp_net_bps_floor": 25, "leverage": lev, "group_gross_cap_pct": cap,
        "cycle_deadline_h": 168, "inventory_reservation_skew_k": 0,
        "jump_first_passage_gate": None, "regime_envelope": None,
        "c1_scheduler": None,
    }, "fits": fits, "budget_quote": float(budget)}
    cfg_path = DIR / f"{tag}.json"
    cfg_path.write_text(json.dumps(cfg, indent=2, sort_keys=True))
    cfg_sha = hashlib.sha256(cfg_path.read_bytes()).hexdigest()
    fp = ln.fingerprint(
        family="C1E", cycle_topology="sync_tier",
        trigger_contract_sha=f"ez{ez}_m{mult}", fit_contract_sha="tier",
        universe_group_weights_sha=f"{len(fits)}g", scheduler_sha=f"lev{lev}",
        resolved_config_sha=cfg_sha[:16], effective_config_sha=cfg_sha[:16],
        cost_model_sha="fee7_slip5", engine_sha=engine_sha(),
        market_data_sha=md_sha(), funding_data_sha=fd_sha(),
        exchange_filter_snapshot_sha="default", maintenance_tiers_sha="0.5pct",
        borrow_snapshot_sha=None,
        window=(block["test_start_ms"], block["test_end_ms"]),
        budget=float(budget), fold=block["block_id"], block="g1t",
        seed=20261101)
    if ln.is_duplicate(fp):
        return None
    row = ln.run_synchronized_cycle(
        experiment_id=tag, parent_id=None, fingerprint=fp,
        config_path=cfg_path, budget=float(budget),
        start_ms=block["test_start_ms"], end_ms=block["test_end_ms"],
        fit_start_ms=block["fit_start_ms"], fit_end_ms=block["fit_end_ms"],
        purge_ms=86_400_000, timeout_s=600)
    return row


def classify(ann, dd):
    if ann is None:
        return None
    if ann >= 110 and dd <= 30:
        return "aggressive"
    if ann >= 90 and dd <= 20:
        return "balanced"
    if ann >= 50 and dd <= 10:
        return "conservative"
    return None


def main() -> None:
    start_t = time.time()
    ln = Launcher()
    bal_hits = []
    agg_hits = []
    cons_hits = []
    total = 0
    for block_id in BLOCKS:
        block = next((ff for ff in FITS["fold_fits"]
                      if ff["block_id"] == block_id), None)
        if not block:
            continue
        fits = block["C1E"]["frozen_fits"]
        # Combined grid: balanced mults with bal fo/ez, then aggressive mults
        grid = ([(m, f, e, "bal") for m in BAL_MULT for f in BAL_FO for e in BAL_EZ]
                + [(m, f, e, "agg") for m in AGG_MULT for f in AGG_FO for e in AGG_EZ])
        for mult, fo, ez, kind in grid:
            for budget in BUDGETS:
                tag = (f"g1t_C1E_{block_id}_m{mult:.2f}_fo{int(fo)}_"
                       f"ez{ez:.2f}_{budget}").replace(".", "p")
                row = run_one(ln, fits, block, mult, fo, ez, budget, CAP, LEV, tag)
                if not row or row.get("skipped"):
                    continue
                total += 1
                if row.get("status") != "complete":
                    continue
                m = row.get("metrics", {}) or {}
                ann = m.get("annualized_return_pct") or 0
                dd = m.get("max_drawdown_pct") or 0
                tier = classify(ann, dd)
                rec = {"tag": tag, "ann": ann, "dd": dd,
                       "sym": m.get("actual_symbols"),
                       "so": m.get("groups_with_so"),
                       "symc": m.get("max_symbol_concentration_pct"),
                       "grpc": m.get("max_group_concentration_pct"),
                       "mult": mult, "fo": fo, "ez": ez, "budget": budget,
                       "block": block_id,
                       "fingerprint": row.get("fingerprint_sha256")}
                if tier == "balanced":
                    bal_hits.append(rec)
                    print(f"  *** BALANCED HIT *** {tag}: ann={ann:.2f}% dd={dd:.2f}%")
                elif tier == "aggressive":
                    agg_hits.append(rec)
                    print(f"  *** AGGRESSIVE HIT *** {tag}: ann={ann:.2f}% dd={dd:.2f}%")
                elif tier == "conservative":
                    cons_hits.append(rec)
                elif ann >= 80 or (ann >= 90 and dd > 20):
                    print(f"  (near) {tag}: ann={ann:.2f}% dd={dd:.2f}%")
    elapsed = time.time() - start_t
    summary = {
        "phase": "R21 G1 tier sweep (balanced + aggressive)",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "total_replays": total,
        "balanced_hits": sorted(bal_hits, key=lambda r: -r["ann"]),
        "aggressive_hits": sorted(agg_hits, key=lambda r: -r["ann"]),
        "conservative_hits_added": sorted(cons_hits, key=lambda r: -r["ann"]),
        "grid": {"balanced": {"mult": BAL_MULT, "fo": BAL_FO, "ez": BAL_EZ},
                 "aggressive": {"mult": AGG_MULT, "fo": AGG_FO, "ez": AGG_EZ},
                 "cap": CAP, "lev": LEV, "budgets": BUDGETS, "blocks": BLOCKS},
    }
    OUT = ART / "g1" / "gates" / "g1_tier.json"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    print(f"total_replays: {total}, elapsed: {elapsed:.1f}s")
    print(f"BALANCED HITS: {len(bal_hits)}")
    for h in bal_hits[:5]:
        print(f"  {h['tag']}: ann={h['ann']:.2f}% dd={h['dd']:.2f}%")
    print(f"AGGRESSIVE HITS: {len(agg_hits)}")
    for h in agg_hits[:5]:
        print(f"  {h['tag']}: ann={h['ann']:.2f}% dd={h['dd']:.2f}%")


if __name__ == "__main__":
    main()
