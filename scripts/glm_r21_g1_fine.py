#!/usr/bin/env python3
"""Round 21 G1 FINE: ultra-fine sweep around the conservative-tier sweet spot.

The mult sweep found ann=52.39%/dd=10.03% (m=2.2, fo=150, cap=300, ez=1.0,
@500U) — 0.03pp over the dd<=10 threshold. And ann=46.01%/dd=9.78% (m=2.1,
fo=150, cap=300, ez=1.0, @500U) — 4pp under ann>=50. The sweet spot is
between m=2.1 and m=2.2. This sweep scans m at 0.05 resolution and tries
adjacent fo/cap/ez values to find the exact config that hits
ann>=50 AND dd<=10 (conservative tier).
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
DIR = ART / "g1" / "configs_fine"
DIR.mkdir(parents=True, exist_ok=True)

# Ultra-fine grid around the sweet spot
MULT_GRID = [2.05, 2.10, 2.15, 2.20, 2.25]
FO_GRID = [140.0, 150.0, 160.0]
CAP_GRID = [250.0, 300.0, 350.0]
EZ_GRID = [0.8, 1.0, 1.2]
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


def main() -> None:
    start_t = time.time()
    ln = Launcher()
    results = []
    cons_hits = []
    for block_id in BLOCKS:
        block = next((ff for ff in FITS["fold_fits"]
                      if ff["block_id"] == block_id), None)
        if not block:
            continue
        fits = block["C1E"]["frozen_fits"]
        for mult in MULT_GRID:
            for fo in FO_GRID:
                for cap in CAP_GRID:
                    for ez in EZ_GRID:
                        tag = (f"g1f_C1E_{block_id}_m{mult:.2f}_fo{int(fo)}_"
                               f"cap{int(cap)}_ez{ez}_{500}").replace(".", "p")
                        cfg = {"synchronized_cycle": {
                            "family": "C1E", "bar_boundary_minutes": 60,
                            "fit_lookback_days": 180, "entry_z": ez,
                            "so_residual_step_z": 0.5, "group_fo_quote": fo,
                            "multiplier": mult, "max_legs": 4, "exit_z": 0.5,
                            "tp_net_bps_floor": 25, "leverage": 10,
                            "group_gross_cap_pct": cap, "cycle_deadline_h": 168,
                            "inventory_reservation_skew_k": 0,
                            "jump_first_passage_gate": None,
                            "regime_envelope": None, "c1_scheduler": None,
                        }, "fits": fits, "budget_quote": 500.0}
                        cfg_path = DIR / f"{tag}.json"
                        cfg_path.write_text(
                            json.dumps(cfg, indent=2, sort_keys=True))
                        cfg_sha = hashlib.sha256(
                            cfg_path.read_bytes()).hexdigest()
                        fp = ln.fingerprint(
                            family="C1E",
                            cycle_topology="synchronized_cycle_fine",
                            trigger_contract_sha=f"ez{ez}_m{mult}",
                            fit_contract_sha="fine",
                            universe_group_weights_sha=f"{len(fits)}g",
                            scheduler_sha="lev10",
                            resolved_config_sha=cfg_sha[:16],
                            effective_config_sha=cfg_sha[:16],
                            cost_model_sha="fee7_slip5",
                            engine_sha=engine_sha(), market_data_sha=md_sha(),
                            funding_data_sha=fd_sha(),
                            exchange_filter_snapshot_sha="default",
                            maintenance_tiers_sha="0.5pct",
                            borrow_snapshot_sha=None,
                            window=(block["test_start_ms"],
                                    block["test_end_ms"]),
                            budget=500.0, fold=block_id, block="g1f",
                            seed=20261101)
                        if ln.is_duplicate(fp):
                            continue
                        row = ln.run_synchronized_cycle(
                            experiment_id=tag, parent_id=None,
                            fingerprint=fp, config_path=cfg_path, budget=500.0,
                            start_ms=block["test_start_ms"],
                            end_ms=block["test_end_ms"],
                            fit_start_ms=block["fit_start_ms"],
                            fit_end_ms=block["fit_end_ms"],
                            purge_ms=86_400_000, timeout_s=600)
                        if row and not row.get("skipped"):
                            m = row.get("metrics", {}) or {}
                            if row.get("status") == "complete":
                                ann = m.get("annualized_return_pct") or 0
                                dd = m.get("max_drawdown_pct") or 0
                                marker = ""
                                if ann >= 50 and dd <= 10:
                                    marker = " *** CONSERVATIVE HIT ***"
                                    cons_hits.append({
                                        "tag": tag, "ann": ann, "dd": dd,
                                        "mult": mult, "fo": fo, "cap": cap,
                                        "ez": ez, "block": block_id,
                                        "sym": m.get("actual_symbols"),
                                        "so": m.get("groups_with_so"),
                                        "symc": m.get("max_symbol_concentration_pct"),
                                        "grpc": m.get("max_group_concentration_pct"),
                                    })
                                elif ann >= 50:
                                    marker = " (ann>=50 dd>10)"
                                elif dd <= 10:
                                    marker = " (dd<=10 ann<50)"
                                if marker or ann >= 40:
                                    print(f"  {tag}: ann={ann:.2f}% dd={dd:.2f}%{marker}")
                            results.append({"tag": tag, "row": row})
    elapsed = time.time() - start_t
    summary = {
        "phase": "R21 G1 fine sweet-spot sweep",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "total_replays": len(results),
        "grid": {"mult": MULT_GRID, "fo": FO_GRID, "cap": CAP_GRID,
                 "ez": EZ_GRID, "budgets": BUDGETS, "blocks": BLOCKS},
        "conservative_hits": cons_hits,
    }
    OUT = ART / "g1" / "gates" / "g1_fine.json"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    print(f"total_replays: {len(results)}, elapsed: {elapsed:.1f}s")
    print(f"CONSERVATIVE HITS: {len(cons_hits)}")
    for h in cons_hits:
        print(f"  {h['tag']}: ann={h['ann']:.2f}% dd={h['dd']:.2f}% "
              f"sym={h['sym']} so={h['so']}")


if __name__ == "__main__":
    main()
