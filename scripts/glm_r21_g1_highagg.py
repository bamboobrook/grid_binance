#!/usr/bin/env python3
"""Round 21 G1 HIGH-AGG: push ann toward target by raising group_gross_cap_pct
(plan §6.1 allows this as an open dim).

The extended sweep showed leverage saturates at group_gross_cap (budget*100%)
because the engine caps per-group gross notional at the budget. Raising
group_gross_cap_pct to 200-400% allows larger positions, which scales ann.

This sweep explores:
  - group_gross_cap_pct: [100, 200, 300, 400]
  - group_fo_quote: [60, 100, 150, 200]
  - leverage: [5, 10]
  - entry_z: [1.0, 1.5]
  - budget: [500, 1000, 4999]
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
DIR = ART / "g1" / "configs_highagg"
DIR.mkdir(parents=True, exist_ok=True)

CAP_GRID = [100.0, 200.0, 300.0, 400.0]
FO_GRID = [60.0, 100.0, 150.0, 200.0]
LEV_GRID = [5, 10]
EZ_GRID = [1.0, 1.5]
BUDGETS = [500, 1000, 4999]
BLOCKS = ["tb01", "tb02", "tb03"]


def engine_sha() -> str:
    return (sha256_file(ROOT / "target/release/synchronized_cycle_replay")
            or "unknown")[:16]


def md_sha() -> str:
    p = ROOT / "data/market_data_full.db"
    st = p.stat()
    return f"size={st.st_size},mtime={int(st.st_mtime)}"[:32]


def fd_sha() -> str:
    return (sha256_file(ROOT / "data/funding_rates_round12.db") or "x")[:16]


def fits_for_family_block(family: str, block_id: str) -> list[dict]:
    for ff in FITS["fold_fits"]:
        if ff["block_id"] != block_id:
            continue
        if family in ("C1E", "B1S"):
            return ff[family]["frozen_fits"]
    return []


def block_by_id(block_id: str) -> dict:
    for ff in FITS["fold_fits"]:
        if ff["block_id"] == block_id:
            return ff
    return {}


def make_cfg(family, fits, budget, entry_z, fo_quote, leverage, cap_pct):
    return {
        "synchronized_cycle": {
            "family": family, "bar_boundary_minutes": 60,
            "fit_lookback_days": 180, "entry_z": entry_z,
            "so_residual_step_z": 0.5, "group_fo_quote": fo_quote,
            "multiplier": 1.5, "max_legs": 4, "exit_z": 0.5,
            "tp_net_bps_floor": 25, "leverage": leverage,
            "group_gross_cap_pct": cap_pct, "cycle_deadline_h": 168,
            "inventory_reservation_skew_k": 0, "jump_first_passage_gate": None,
            "regime_envelope": None, "c1_scheduler": None,
        },
        "fits": fits, "budget_quote": float(budget),
    }


def run_one(ln, family, fits, block, budget, entry_z, fo_quote, leverage,
            cap_pct, tag):
    cfg = make_cfg(family, fits, budget, entry_z, fo_quote, leverage, cap_pct)
    cfg_path = DIR / f"{tag}.json"
    cfg_path.write_text(json.dumps(cfg, indent=2, sort_keys=True))
    cfg_sha = hashlib.sha256(cfg_path.read_bytes()).hexdigest()
    fp = ln.fingerprint(
        family=family, cycle_topology="synchronized_cycle_highagg",
        trigger_contract_sha=f"ez{entry_z}_cap{cap_pct}",
        fit_contract_sha="ha", universe_group_weights_sha=f"{len(fits)}g",
        scheduler_sha=f"lev{leverage}",
        resolved_config_sha=cfg_sha[:16], effective_config_sha=cfg_sha[:16],
        cost_model_sha="fee7_slip5", engine_sha=engine_sha(),
        market_data_sha=md_sha(), funding_data_sha=fd_sha(),
        exchange_filter_snapshot_sha="default_per_symbol",
        maintenance_tiers_sha="0.5pct", borrow_snapshot_sha=None,
        window=(block["test_start_ms"], block["test_end_ms"]),
        budget=float(budget), fold=block["block_id"], block="g1ha",
        seed=20261101)
    if ln.is_duplicate(fp):
        return {"skipped": True, "tag": tag}
    row = ln.run_synchronized_cycle(
        experiment_id=tag, parent_id=None, fingerprint=fp,
        config_path=cfg_path, budget=float(budget),
        start_ms=block["test_start_ms"], end_ms=block["test_end_ms"],
        fit_start_ms=block["fit_start_ms"], fit_end_ms=block["fit_end_ms"],
        purge_ms=86_400_000, timeout_s=600)
    return row or {}


def main() -> None:
    start_t = time.time()
    ln = Launcher()
    results = []
    for family in ("C1E",):
        for block_id in BLOCKS:
            block = block_by_id(block_id)
            fits = fits_for_family_block(family, block_id)
            if not fits:
                continue
            for cap in CAP_GRID:
                for lev in LEV_GRID:
                    for fo in FO_GRID:
                        for ez in EZ_GRID:
                            for budget in BUDGETS:
                                tag = (f"g1ha_{family}_{block_id}_"
                                       f"cap{int(cap)}_lev{lev}_fo{int(fo)}"
                                       f"_ez{ez}_{budget}")
                                row = run_one(ln, family, fits, block, budget,
                                              ez, fo, lev, cap, tag)
                                if not row.get("skipped"):
                                    m = (row.get("metrics") or {})
                                    if row.get("status") == "complete":
                                        print(f"  cap={cap} lev={lev} fo={fo} "
                                              f"ez={ez} b={budget}: ann={m.get('annualized_return_pct')}")
                                results.append({"family": family, "tag": tag,
                                                "row": row})
    elapsed = time.time() - start_t
    summary = {
        "phase": "R21 G1 high-agg (group_gross_cap_pct expansion)",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "total_replays": sum(1 for r in results if not r["row"].get("skipped")),
        "grid": {"cap_pct": CAP_GRID, "lev": LEV_GRID, "fo": FO_GRID,
                 "ez": EZ_GRID, "budgets": BUDGETS, "blocks": BLOCKS},
        "results": [],
        "best_per_family": {},
        "tier_hits": {"conservative": [], "balanced": [], "aggressive": []},
    }
    for r in results:
        row = r["row"]
        if not row or row.get("skipped"):
            continue
        m = row.get("metrics", {}) or {}
        rec = {
            "tag": r["tag"], "family": r["family"], "status": row.get("status"),
            "first_failed_gate": row.get("first_failed_gate"),
            "ann_pct": m.get("annualized_return_pct"),
            "max_dd_pct": m.get("max_drawdown_pct"),
            "actual_symbols": m.get("actual_symbols"),
            "groups_with_so": m.get("groups_with_so"),
            "max_symbol_conc_pct": m.get("max_symbol_concentration_pct"),
            "max_group_conc_pct": m.get("max_group_concentration_pct"),
            "budget": row.get("budget"), "fold": row.get("fold"),
        }
        summary["results"].append(rec)
        if rec["status"] == "complete" and rec["ann_pct"] is not None:
            cur = summary["best_per_family"].get(r["family"])
            if cur is None or rec["ann_pct"] > cur["ann_pct"]:
                summary["best_per_family"][r["family"]] = rec
            if rec["ann_pct"] >= 50 and rec["max_dd_pct"] <= 10:
                summary["tier_hits"]["conservative"].append(rec)
            if rec["ann_pct"] >= 90 and rec["max_dd_pct"] <= 20:
                summary["tier_hits"]["balanced"].append(rec)
            if rec["ann_pct"] >= 110 and rec["max_dd_pct"] <= 30:
                summary["tier_hits"]["aggressive"].append(rec)
    OUT = ART / "g1" / "gates" / "g1_highagg.json"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    print(f"total_replays: {summary['total_replays']}, elapsed: {elapsed:.1f}s")
    for fam, best in summary["best_per_family"].items():
        print(f"  best {fam}: ann={best['ann_pct']:.2f}% dd={best['max_dd_pct']:.2f}% tag={best['tag']}")
    print(f"  tier hits: conservative={len(summary['tier_hits']['conservative'])} "
          f"balanced={len(summary['tier_hits']['balanced'])} "
          f"aggressive={len(summary['tier_hits']['aggressive'])}")


if __name__ == "__main__":
    main()
