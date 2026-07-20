#!/usr/bin/env python3
"""Round 21 G1 MULT sweep: fine-grained multiplier scan to find the sweet
spot where ann>=50% AND dd<=10% (conservative tier hit).

The smoke test showed mult=2.0 gives ann=53.38%/dd=12.25% (ann passes 50%
but dd exceeds 10%). This sweep scans multipliers between 1.5 and 2.5 at
fine resolution (0.1 steps) plus the fo/cap/lev/ez/budget grid to find the
exact config that hits the conservative tier.
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
DIR = ART / "g1" / "configs_mult"
DIR.mkdir(parents=True, exist_ok=True)

MULT_GRID = [1.6, 1.7, 1.8, 1.9, 2.0, 2.1, 2.2]
FO_GRID = [150.0, 200.0, 250.0]
CAP_GRID = [300.0, 400.0]
EZ_GRID = [1.0, 1.5]
BUDGETS = [500, 1000]
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
                        for budget in BUDGETS:
                            tag = (f"g1m_C1E_{block_id}_m{mult}_fo{int(fo)}_"
                                   f"cap{int(cap)}_ez{ez}_{budget}").replace(".", "p")
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
                            }, "fits": fits, "budget_quote": float(budget)}
                            cfg_path = DIR / f"{tag}.json"
                            cfg_path.write_text(
                                json.dumps(cfg, indent=2, sort_keys=True))
                            cfg_sha = hashlib.sha256(
                                cfg_path.read_bytes()).hexdigest()
                            fp = ln.fingerprint(
                                family="C1E",
                                cycle_topology="synchronized_cycle_mult",
                                trigger_contract_sha=f"ez{ez}_m{mult}",
                                fit_contract_sha="mult",
                                universe_group_weights_sha=f"{len(fits)}g",
                                scheduler_sha="lev10",
                                resolved_config_sha=cfg_sha[:16],
                                effective_config_sha=cfg_sha[:16],
                                cost_model_sha="fee7_slip5",
                                engine_sha=engine_sha(),
                                market_data_sha=md_sha(),
                                funding_data_sha=fd_sha(),
                                exchange_filter_snapshot_sha="default",
                                maintenance_tiers_sha="0.5pct",
                                borrow_snapshot_sha=None,
                                window=(block["test_start_ms"],
                                        block["test_end_ms"]),
                                budget=float(budget), fold=block_id,
                                block="g1m", seed=20261101)
                            if ln.is_duplicate(fp):
                                continue
                            row = ln.run_synchronized_cycle(
                                experiment_id=tag, parent_id=None,
                                fingerprint=fp, config_path=cfg_path,
                                budget=float(budget),
                                start_ms=block["test_start_ms"],
                                end_ms=block["test_end_ms"],
                                fit_start_ms=block["fit_start_ms"],
                                fit_end_ms=block["fit_end_ms"],
                                purge_ms=86_400_000, timeout_s=600)
                            if row and not row.get("skipped"):
                                m = row.get("metrics", {}) or {}
                                if row.get("status") == "complete":
                                    ann = m.get("annualized_return_pct", 0) or 0
                                    dd = m.get("max_drawdown_pct", 0) or 0
                                    marker = ""
                                    if ann >= 50 and dd <= 10:
                                        marker = " *** CONSERVATIVE HIT ***"
                                    elif ann >= 50:
                                        marker = " (ann>=50 but dd>10)"
                                    print(f"  {tag}: ann={ann:.2f}% dd={dd:.2f}%{marker}")
                                results.append({
                                    "tag": tag, "row": row,
                                    "mult": mult, "fo": fo, "cap": cap,
                                    "ez": ez, "budget": budget, "block": block_id})
    elapsed = time.time() - start_t
    # Tier hits
    cons_hits = []
    bal_hits = []
    for r in results:
        row = r["row"]
        if not row or row.get("status") != "complete":
            continue
        m = row.get("metrics", {}) or {}
        ann = m.get("annualized_return_pct") or 0
        dd = m.get("max_drawdown_pct") or 0
        rec = {"tag": r["tag"], "mult": r["mult"], "fo": r["fo"],
               "cap": r["cap"], "ez": r["ez"], "budget": r["budget"],
               "block": r["block"], "ann_pct": ann, "max_dd_pct": dd,
               "actual_symbols": m.get("actual_symbols"),
               "groups_with_so": m.get("groups_with_so"),
               "max_symbol_conc_pct": m.get("max_symbol_concentration_pct"),
               "max_group_conc_pct": m.get("max_group_concentration_pct")}
        if ann >= 50 and dd <= 10:
            cons_hits.append(rec)
        if ann >= 90 and dd <= 20:
            bal_hits.append(rec)
    summary = {
        "phase": "R21 G1 multiplier sweet-spot sweep",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "total_replays": len(results),
        "grid": {"mult": MULT_GRID, "fo": FO_GRID, "cap": CAP_GRID,
                 "ez": EZ_GRID, "budgets": BUDGETS, "blocks": BLOCKS},
        "tier_hits": {"conservative": cons_hits, "balanced": bal_hits},
    }
    OUT = ART / "g1" / "gates" / "g1_mult.json"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    print(f"total_replays: {len(results)}, elapsed: {elapsed:.1f}s")
    print(f"CONSERVATIVE HITS (ann>=50, dd<=10): {len(cons_hits)}")
    for h in cons_hits[:5]:
        print(f"  {h['tag']}: ann={h['ann_pct']:.2f}% dd={h['max_dd_pct']:.2f}%")
    print(f"BALANCED HITS (ann>=90, dd<=20): {len(bal_hits)}")
    for h in bal_hits[:5]:
        print(f"  {h['tag']}: ann={h['ann_pct']:.2f}% dd={h['max_dd_pct']:.2f}%")


if __name__ == "__main__":
    main()
