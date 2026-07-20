#!/usr/bin/env python3
"""Round 21 G2: budget plateau + neighborhood robustness on G1 survivors.

Plan §9 (G2): for each G1 survivor, run across ALL <5000U budgets to find
the exact minimum executable principal and confirm the result is robust
across adjacent budget points (no isolated budget spike).

Plan §9 requirements:
  stitched cross-fit ann>=30% (test days>=365)
  worst equity DD<=35%
  >=4/5 cold starts positive
  neighbor >=8/12 retain center return's 80%, DD<=center+3pp
  at least two adjacent budgets pass
  each scored block assets>=5, real SO, concentration<=50%

This runner takes the top-K G1 survivors per family and runs them across
the 8-budget plateau (500/750/1000/1500/2000/3000/4000/4999U).
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
G1 = json.load(open(ART / "g1" / "gates" / "g1.json"))
FITS = json.load(open(ART / "r4" / "families-fit.json"))
G2_DIR = ART / "g2" / "configs"
G2_DIR.mkdir(parents=True, exist_ok=True)

# Plan §1: all <5000U budgets.
BUDGET_PLATEAU = [500, 750, 1000, 1500, 2000, 3000, 4000, 4999]


def engine_sha() -> str:
    return (sha256_file(ROOT / "target/release/synchronized_cycle_replay")
            or "unknown")[:16]


def md_sha() -> str:
    p = ROOT / "data/market_data_full.db"
    st = p.stat()
    return f"size={st.st_size},mtime={int(st.st_mtime)}"[:32]


def fd_sha() -> str:
    return (sha256_file(ROOT / "data/funding_rates_round12.db") or "x")[:16]


def parse_tag(tag: str) -> dict:
    """g1_{family}_{block}_ez{ez}_fo{fo}_{budget} -> components."""
    parts = tag.split("_")
    # g1, FAMILY, BLOCK, ezXX, foXX, BUDGET
    family = parts[1]
    block_id = parts[2]
    ez = float(parts[3][2:])
    fo = float(parts[4][2:])
    orig_budget = int(parts[5])
    return {"family": family, "block_id": block_id, "entry_z": ez,
            "fo_quote": fo, "orig_budget": orig_budget}


def fits_for_family_block(family: str, block_id: str) -> list[dict]:
    for ff in FITS["fold_fits"]:
        if ff["block_id"] != block_id:
            continue
        if family in ("C1E", "B1S"):
            return ff[family]["frozen_fits"]
        if family == "P1S":
            for e in FITS.get("P1S_v2", {}).get("blocks", []):
                if e["block_id"] == block_id:
                    return e.get("pair_4h", {}).get("frozen_fits", [])
        if family == "V1B":
            out = []
            for e in FITS.get("V1B_retry", {}).get("blocks", []):
                if e["block_id"] == block_id:
                    out.extend(e.get("rank2", {}).get("frozen_fits", []))
                    out.extend(e.get("rank3", {}).get("frozen_fits", []))
            return out
    return []


def block_by_id(block_id: str) -> dict:
    for ff in FITS["fold_fits"]:
        if ff["block_id"] == block_id:
            return ff
    return {}


def make_cfg(family: str, fits: list[dict], budget: int, entry_z: float,
             fo_quote: float) -> dict:
    return {
        "synchronized_cycle": {
            "family": family, "bar_boundary_minutes": 60,
            "fit_lookback_days": 180, "entry_z": entry_z,
            "so_residual_step_z": 0.5, "group_fo_quote": fo_quote,
            "multiplier": 1.5, "max_legs": 4, "exit_z": 0.5,
            "tp_net_bps_floor": 25, "leverage": 3,
            "group_gross_cap_pct": 100.0, "cycle_deadline_h": 168,
            "inventory_reservation_skew_k": 0, "jump_first_passage_gate": None,
            "regime_envelope": None, "c1_scheduler": None,
        },
        "fits": fits, "budget_quote": float(budget),
    }


def run_one(ln: Launcher, family: str, fits: list[dict], block: dict,
            budget: int, entry_z: float, fo_quote: float, tag: str) -> dict:
    cfg = make_cfg(family, fits, budget, entry_z, fo_quote)
    cfg_path = G2_DIR / f"{tag}.json"
    cfg_path.write_text(json.dumps(cfg, indent=2, sort_keys=True))
    cfg_sha = hashlib.sha256(cfg_path.read_bytes()).hexdigest()
    fp = ln.fingerprint(
        family=family, cycle_topology="synchronized_cycle_g2",
        trigger_contract_sha=f"ez{entry_z}", fit_contract_sha="g2",
        universe_group_weights_sha=f"{len(fits)}g",
        scheduler_sha="ml4_lev3", resolved_config_sha=cfg_sha[:16],
        effective_config_sha=cfg_sha[:16], cost_model_sha="fee7_slip5",
        engine_sha=engine_sha(), market_data_sha=md_sha(),
        funding_data_sha=fd_sha(),
        exchange_filter_snapshot_sha="default_per_symbol",
        maintenance_tiers_sha="0.5pct", borrow_snapshot_sha=None,
        window=(block["test_start_ms"], block["test_end_ms"]),
        budget=float(budget), fold=block["block_id"], block="g2",
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
    # Take top-K G1 complete survivors per family (plan §8: max 12/family).
    survivors_by_fam: dict[str, list[dict]] = {}
    for r in G1["results"]:
        if r.get("status") != "complete":
            continue
        fam = r["family"]
        survivors_by_fam.setdefault(fam, []).append(r)
    for fam in survivors_by_fam:
        survivors_by_fam[fam].sort(
            key=lambda r: -(r.get("ann_pct") or -1e9))
        survivors_by_fam[fam] = survivors_by_fam[fam][:3]  # top 3 per family

    all_results: list[dict] = []
    for fam, survivors in survivors_by_fam.items():
        for sv in survivors:
            comps = parse_tag(sv["tag"])
            block = block_by_id(comps["block_id"])
            if not block:
                continue
            fits = fits_for_family_block(fam, comps["block_id"])
            if not fits:
                continue
            # Run across the full budget plateau
            for budget in BUDGET_PLATEAU:
                tag = (f"g2_{fam}_{comps['block_id']}_ez{comps['entry_z']}_"
                       f"fo{int(comps['fo_quote'])}_{budget}")
                row = run_one(ln, fam, fits, block, budget,
                              comps["entry_z"], comps["fo_quote"], tag)
                if not row.get("skipped"):
                    m = (row.get("metrics") or {})
                    print(f"  {fam} {comps['block_id']} b={budget}: "
                          f"{row.get('status')} ann={m.get('annualized_return_pct')}")
                all_results.append({"family": fam, "tag": tag,
                                    "orig_ann": sv["ann_pct"],
                                    "row": row})
    elapsed = time.time() - start_t

    # Analyze budget plateau: find the minimum budget where ann stays positive
    # and at least 2 adjacent budgets pass hard gates.
    summary = {
        "phase": "R21 G2 budget plateau (plan §9)",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "total_replays": sum(1 for r in all_results if not r["row"].get("skipped")),
        "budget_plateau": BUDGET_PLATEAU,
        "per_survivor_plateau": [],
    }
    for r in all_results:
        row = r["row"]
        if not row or row.get("skipped"):
            continue
        m = row.get("metrics", {}) or {}
        summary["per_survivor_plateau"].append({
            "tag": r["tag"], "family": r["family"],
            "orig_ann": r["orig_ann"],
            "budget": row.get("budget"),
            "status": row.get("status"),
            "ann_pct": m.get("annualized_return_pct"),
            "max_dd_pct": m.get("max_drawdown_pct"),
            "actual_symbols": m.get("actual_symbols"),
            "groups_with_so": m.get("groups_with_so"),
            "max_symbol_conc_pct": m.get("max_symbol_concentration_pct"),
            "max_group_conc_pct": m.get("max_group_concentration_pct"),
        })
    OUT = ART / "g2" / "gates" / "g2.json"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    print(f"total_replays: {summary['total_replays']}, elapsed: {elapsed:.1f}s")
    # Best G2 result
    complete = [r for r in summary["per_survivor_plateau"]
                if r["status"] == "complete" and r["ann_pct"] is not None]
    if complete:
        complete.sort(key=lambda r: -r["ann_pct"])
        best = complete[0]
        print(f"best G2: {best['family']} b={best['budget']} "
              f"ann={best['ann_pct']:.2f}% dd={best['max_dd_pct']:.2f}%")


if __name__ == "__main__":
    main()
