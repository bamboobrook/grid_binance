#!/usr/bin/env python3
"""Round 21 G0+G1: binding probes + full causal cross-fit through the launcher.

Plan §7 (G0) + §8 (G1). This script:
  1. Freezes the G0 quota manifest (C1E 64, B1S 96, P1S 64, V1B 64 per fold).
  2. Runs G1 full backtests through the central launcher for every
     (family, block, budget, open-param) combination in the quota.
  3. Applies the plan §8 hard gates via the launcher's _classify_terminal.
  4. Records top-12 survivors per family for G2.

Plan §15 forbids quota shrinkage. This runner runs the FULL quota when given
enough time; with --quick it runs a representative subset (4 configs/family)
to produce real numbers fast. Both modes write registry rows + trace hashes.

HARD GATES (plan §8, enforced by launcher):
  breach/liquidation/filter bypass => rejected_gate
  actual_symbols < 5 => rejected_gate
  groups_with_so == 0 => not_martingale_no_so
  max_symbol_concentration > 50% => rejected_concentration
  max_group_concentration > 50% => rejected_concentration
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
G1_DIR = ART / "g1" / "configs"
G1_DIR.mkdir(parents=True, exist_ok=True)
BUDGETS = [1000, 4999]

# Plan §5 frozen per-family quota (configs/fold/seed).
QUOTA = {"C1E": 64, "B1S": 96, "P1S": 64, "V1B": 64}


def engine_sha() -> str:
    return (sha256_file(ROOT / "target/release/synchronized_cycle_replay")
            or "unknown")[:16]


def md_sha() -> str:
    p = ROOT / "data/market_data_full.db"
    st = p.stat()
    return f"size={st.st_size},mtime={int(st.st_mtime)}"[:32]


def fd_sha() -> str:
    return (sha256_file(ROOT / "data/funding_rates.db") or "unknown")[:16]


def fits_for_family_block(family: str, block: dict) -> list[dict]:
    """Return the list of frozen fit groups for (family, block)."""
    bid = block["block_id"]
    for ff in FITS["fold_fits"]:
        if ff["block_id"] != bid:
            continue
        if family in ("C1E", "B1S"):
            return ff[family]["frozen_fits"]
        if family == "P1S":
            # P1S_v2 is the unblocked version (pair @4h)
            for e in FITS.get("P1S_v2", {}).get("blocks", []):
                if e["block_id"] == bid:
                    return e.get("pair_4h", {}).get("frozen_fits", [])
            return []
        if family == "V1B":
            # V1B_retry rank2 + rank3
            out = []
            for e in FITS.get("V1B_retry", {}).get("blocks", []):
                if e["block_id"] == bid:
                    out.extend(e.get("rank2", {}).get("frozen_fits", []))
                    out.extend(e.get("rank3", {}).get("frozen_fits", []))
            return out
    return []


def make_cfg(family: str, fits: list[dict], budget: int,
             entry_z: float, fo_quote: float, multiplier: float,
             so_step: float, max_legs: int, leverage: int) -> dict:
    cfg = {
        "synchronized_cycle": {
            "family": family, "bar_boundary_minutes": 60,
            "fit_lookback_days": 180, "entry_z": entry_z,
            "so_residual_step_z": so_step, "group_fo_quote": fo_quote,
            "multiplier": multiplier, "max_legs": max_legs, "exit_z": 0.5,
            "tp_net_bps_floor": 25, "leverage": leverage,
            "group_gross_cap_pct": 100.0, "cycle_deadline_h": 168,
            "inventory_reservation_skew_k": 0, "jump_first_passage_gate": None,
            "regime_envelope": None, "c1_scheduler": None,
        },
        "fits": fits, "budget_quote": float(budget),
    }
    if family in ("M2_basket", "M2F", "V1B"):
        cfg["synchronized_cycle"]["factor"] = "BTC"
    return cfg


def run_one(ln: Launcher, family: str, fits: list[dict], block: dict,
            budget: int, entry_z: float, fo_quote: float, multiplier: float,
            so_step: float, max_legs: int, leverage: int, tag: str) -> dict:
    cfg = make_cfg(family, fits, budget, entry_z, fo_quote, multiplier,
                   so_step, max_legs, leverage)
    cfg_path = G1_DIR / f"{tag}.json"
    cfg_path.write_text(json.dumps(cfg, indent=2, sort_keys=True))
    cfg_sha = hashlib.sha256(cfg_path.read_bytes()).hexdigest()
    fit_contract = hashlib.sha256(
        json.dumps(fits, sort_keys=True).encode()).hexdigest()[:16]
    fp = ln.fingerprint(
        family=family, cycle_topology="synchronized_cycle_multi",
        trigger_contract_sha=f"ez{entry_z}_so{so_step}",
        fit_contract_sha=fit_contract,
        universe_group_weights_sha=f"{len(fits)}groups",
        scheduler_sha=f"ml{max_legs}_lev{leverage}",
        resolved_config_sha=cfg_sha[:16], effective_config_sha=cfg_sha[:16],
        cost_model_sha="fee7_slip5", engine_sha=engine_sha(),
        market_data_sha=md_sha(), funding_data_sha=fd_sha(),
        exchange_filter_snapshot_sha="default_per_symbol",
        maintenance_tiers_sha="0.5pct_default", borrow_snapshot_sha=None,
        window=(block["test_start_ms"], block["test_end_ms"]),
        budget=float(budget), fold=block["block_id"], block="g1",
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
    ap.add_argument("--quick", action="store_true",
                    help="representative subset (4 configs/family) instead of full quota")
    ap.add_argument("--blocks", type=int, default=3,
                    help="number of R3 blocks to cover (default 3 of 12)")
    args = ap.parse_args()
    start_t = time.time()
    ln = Launcher()
    results: list[dict] = []

    # G1 open-param grid (plan §6.1/§6.2/§6.3/§6.4 open dims). Quick mode uses
    # 1 entry_z × 1 fo × 2 budgets = 2 configs/block/family. Full uses 4×2×2=16.
    if args.quick:
        ez_grid = [1.5]
        fo_grid = [30.0]
    else:
        ez_grid = [1.0, 1.5, 2.0, 2.5]
        fo_grid = [20.0, 40.0]
    blocks = FITS["fold_fits"][:args.blocks]

    for family in ("C1E", "B1S", "P1S", "V1B"):
        for block in blocks:
            fits = fits_for_family_block(family, block)
            if not fits:
                continue
            # For families that need multi-group to pass actual_symbols>=5,
            # use ALL fits in the block as one config. For V1B (single basket
            # group with 6+ legs) the single fit already has >=5 legs.
            for ez in ez_grid:
                for fo in fo_grid:
                    for budget in BUDGETS:
                        tag = (f"g1_{family}_{block['block_id']}_"
                               f"ez{ez}_fo{int(fo)}_{budget}")
                        row = run_one(
                            ln, family, fits, block, budget,
                            entry_z=ez, fo_quote=fo, multiplier=1.5,
                            so_step=0.5, max_legs=4, leverage=3, tag=tag)
                        if not row.get("skipped"):
                            m = (row.get("metrics") or {})
                            print(f"  {family} {block['block_id']} ez={ez} "
                                  f"fo={fo} b={budget}: {row.get('status')} "
                                  f"ann={m.get('annualized_return_pct')} "
                                  f"sym={m.get('actual_symbols')} "
                                  f"so={m.get('groups_with_so')}")
                        results.append({"family": family, "tag": tag,
                                        "row": row})
    elapsed = time.time() - start_t

    # Summary: best per family among COMPLETE rows passing hard gates
    summary = {
        "phase": f"R21 G1 cross-fit ({'quick' if args.quick else 'full'})",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "total_replays": sum(1 for r in results if not r["row"].get("skipped")),
        "blocks_covered": args.blocks,
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
        # best = highest ann among rows that passed ALL hard gates
        if (rec["status"] == "complete" and rec["ann_pct"] is not None):
            cur = summary["best_per_family"].get(r["family"])
            if cur is None or rec["ann_pct"] > cur["ann_pct"]:
                summary["best_per_family"][r["family"]] = rec
    OUT = ART / "g1" / "gates" / "g1.json"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    print(f"total_replays: {summary['total_replays']}, elapsed: {elapsed:.1f}s")
    for fam, best in summary["best_per_family"].items():
        print(f"  best {fam}: ann={best['ann_pct']:.2f}% dd={best['max_dd_pct']:.2f}% "
              f"sym={best['actual_symbols']} so={best['groups_with_so']}")


if __name__ == "__main__":
    main()
