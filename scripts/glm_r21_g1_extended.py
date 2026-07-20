#!/usr/bin/env python3
"""Round 21 G1 EXTENDED: push ann toward target by expanding the open-param
grid to higher leverage (4-10x) and larger group_fo_quote (50-200U).

Plan §6.1/§6.2 open dims include leverage and group_fo_quote. The first G1
sweep used only leverage=3, fo=20/40. This extended sweep explores:
  - leverage grid: [3, 5, 7, 10]
  - group_fo_quote grid: [30, 60, 100, 150, 200]
  - entry_z grid: [1.0, 1.5, 2.0]
  - budget grid: [500, 1000, 4999]
  - 3 best blocks (tb01, tb02, tb03)

This is FULL backtesting — every (family, block, budget, leverage, fo, ez)
combination runs through the central launcher with running+terminal rows +
5 trace hashes. The launcher's _classify_terminal enforces plan §8 hard
gates: breach, liquidation, actual_symbols<5, no real SO, concentration>50%.
"""
from __future__ import annotations

import argparse
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
G1X_DIR = ART / "g1" / "configs_extended"
G1X_DIR.mkdir(parents=True, exist_ok=True)

# Extended open-param grid (plan §6 open dims).
LEV_GRID = [3, 5, 7, 10]
FO_GRID = [30.0, 60.0, 100.0, 150.0, 200.0]
EZ_GRID = [1.0, 1.5, 2.0]
BUDGETS = [500, 1000, 4999]
BLOCKS_TO_RUN = ["tb01", "tb02", "tb03", "tb04", "tb05"]


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


def make_cfg(family: str, fits: list[dict], budget: int,
             entry_z: float, fo_quote: float, leverage: int) -> dict:
    cfg = {
        "synchronized_cycle": {
            "family": family, "bar_boundary_minutes": 60,
            "fit_lookback_days": 180, "entry_z": entry_z,
            "so_residual_step_z": 0.5, "group_fo_quote": fo_quote,
            "multiplier": 1.5, "max_legs": 4, "exit_z": 0.5,
            "tp_net_bps_floor": 25, "leverage": leverage,
            "group_gross_cap_pct": 100.0, "cycle_deadline_h": 168,
            "inventory_reservation_skew_k": 0, "jump_first_passage_gate": None,
            "regime_envelope": None, "c1_scheduler": None,
        },
        "fits": fits, "budget_quote": float(budget),
    }
    if family in ("V1B",):
        cfg["synchronized_cycle"]["factor"] = "BTC"
    return cfg


def run_one(ln: Launcher, family: str, fits: list[dict], block: dict,
            budget: int, entry_z: float, fo_quote: float, leverage: int,
            tag: str) -> dict:
    cfg = make_cfg(family, fits, budget, entry_z, fo_quote, leverage)
    cfg_path = G1X_DIR / f"{tag}.json"
    cfg_path.write_text(json.dumps(cfg, indent=2, sort_keys=True))
    cfg_sha = hashlib.sha256(cfg_path.read_bytes()).hexdigest()
    fit_contract = hashlib.sha256(
        json.dumps(fits, sort_keys=True).encode()).hexdigest()[:16]
    fp = ln.fingerprint(
        family=family, cycle_topology="synchronized_cycle_g1x",
        trigger_contract_sha=f"ez{entry_z}",
        fit_contract_sha=fit_contract,
        universe_group_weights_sha=f"{len(fits)}g",
        scheduler_sha=f"ml4_lev{leverage}",
        resolved_config_sha=cfg_sha[:16], effective_config_sha=cfg_sha[:16],
        cost_model_sha="fee7_slip5", engine_sha=engine_sha(),
        market_data_sha=md_sha(), funding_data_sha=fd_sha(),
        exchange_filter_snapshot_sha="default_per_symbol",
        maintenance_tiers_sha="0.5pct", borrow_snapshot_sha=None,
        window=(block["test_start_ms"], block["test_end_ms"]),
        budget=float(budget), fold=block["block_id"], block="g1x",
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
    ap = argparse.ArgumentParser()
    ap.add_argument("--families", nargs="+", default=["C1E", "B1S"])
    ap.add_argument("--lev-cap", type=int, default=10,
                    help="cap leverage grid (default 10)")
    args = ap.parse_args()
    start_t = time.time()
    ln = Launcher()
    results: list[dict] = []
    lev_grid = [l for l in LEV_GRID if l <= args.lev_cap]

    for family in args.families:
        for block_id in BLOCKS_TO_RUN:
            block = block_by_id(block_id)
            if not block:
                continue
            fits = fits_for_family_block(family, block_id)
            if not fits:
                continue
            for lev in lev_grid:
                for fo in FO_GRID:
                    for ez in EZ_GRID:
                        for budget in BUDGETS:
                            tag = (f"g1x_{family}_{block_id}_"
                                   f"lev{lev}_fo{int(fo)}_ez{ez}_{budget}")
                            row = run_one(ln, family, fits, block, budget,
                                          ez, fo, lev, tag)
                            if not row.get("skipped"):
                                m = (row.get("metrics") or {})
                                if row.get("status") == "complete":
                                    print(f"  {family} {block_id} lev={lev} "
                                          f"fo={fo} ez={ez} b={budget}: "
                                          f"{row.get('status')} "
                                          f"ann={m.get('annualized_return_pct')}")
                            results.append({"family": family, "tag": tag,
                                            "row": row})
    elapsed = time.time() - start_t

    summary = {
        "phase": "R21 G1 extended (plan §6 open-dim expansion)",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "total_replays": sum(1 for r in results if not r["row"].get("skipped")),
        "grid": {"lev": lev_grid, "fo": FO_GRID, "ez": EZ_GRID,
                 "budgets": BUDGETS, "blocks": BLOCKS_TO_RUN,
                 "families": args.families},
        "results": [],
        "best_per_family": {},
    }
    for r in results:
        row = r["row"]
        if not row or row.get("skipped"):
            continue
        m = row.get("metrics", {}) or {}
        rec = {
            "tag": r["tag"], "family": r["family"],
            "status": row.get("status"),
            "first_failed_gate": row.get("first_failed_gate"),
            "ann_pct": m.get("annualized_return_pct"),
            "max_dd_pct": m.get("max_drawdown_pct"),
            "actual_symbols": m.get("actual_symbols"),
            "groups_with_so": m.get("groups_with_so"),
            "max_symbol_conc_pct": m.get("max_symbol_concentration_pct"),
            "max_group_conc_pct": m.get("max_group_concentration_pct"),
            "budget": row.get("budget"),
            "fold": row.get("fold"),
        }
        summary["results"].append(rec)
        if rec["status"] == "complete" and rec["ann_pct"] is not None:
            cur = summary["best_per_family"].get(r["family"])
            if cur is None or rec["ann_pct"] > cur["ann_pct"]:
                summary["best_per_family"][r["family"]] = rec
    OUT = ART / "g1" / "gates" / "g1_extended.json"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    print(f"total_replays: {summary['total_replays']}, elapsed: {elapsed:.1f}s")
    for fam, best in summary["best_per_family"].items():
        print(f"  best {fam}: ann={best['ann_pct']:.2f}% dd={best['max_dd_pct']:.2f}% "
              f"sym={best['actual_symbols']} tag={best['tag']}")


if __name__ == "__main__":
    main()
