#!/usr/bin/env python3
"""Round 21 G1 CORRECTED sweep: run C1E with corrected R4 fits (ADF<-2.85
enforced) across multiple blocks (tb01, tb02, tb03) + multiplier expansion.

This is the strict cross-fit: each block uses its OWN block-specific frozen
fits (R4 already refits per block). The strategy must remain positive on
>=4/5 blocks with tier-target ann/DD (plan §1 cold-start mandatory + §5
trial correction).

The previous 3-tier hits were revoked because R4 had inverted ADF gate.
With the fix, all C1E pairs now have ADF<-2.85. This sweep re-tests whether
the multiplier sweet spot produces cross-fit-validated tier hits.
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
DIR = ART / "g1" / "configs_corrected"
DIR.mkdir(parents=True, exist_ok=True)

# Multiplier grid spanning conservative (m=2.0-2.3) to aggressive (m=2.4-3.0)
MULT_GRID = [2.0, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 2.9, 3.0]
FO_GRID = [100.0, 150.0, 200.0]
EZ_GRID = [0.9, 1.0, 1.1]
CAP = 400.0
LEV = 10
BUDGET = 500


def engine_sha():
    return (sha256_file(ROOT / "target/release/synchronized_cycle_replay")
            or "unknown")[:16]


def md_sha():
    p = ROOT / "data/market_data_full.db"
    st = p.stat()
    return f"size={st.st_size},mtime={int(st.st_mtime)}"[:32]


def fd_sha():
    return (sha256_file(ROOT / "data/funding_rates_round12.db") or "x")[:16]


def main():
    start_t = time.time()
    ln = Launcher()
    # All blocks that have C1E fits
    blocks_with_fits = []
    for ff in FITS["fold_fits"]:
        if ff["C1E"]["frozen_fits"]:
            blocks_with_fits.append(ff)
    print(f"blocks with C1E fits: {[b['block_id'] for b in blocks_with_fits]}")
    results = []
    for block in blocks_with_fits:
        fits = block["C1E"]["frozen_fits"]
        for mult in MULT_GRID:
            for fo in FO_GRID:
                for ez in EZ_GRID:
                    tag = (f"g1c_C1E_{block['block_id']}_m{mult:.1f}_"
                           f"fo{int(fo)}_ez{ez:.1f}").replace(".", "p")
                    cfg = {"synchronized_cycle": {
                        "family": "C1E", "bar_boundary_minutes": 60,
                        "fit_lookback_days": 180,
                        "entry_z": ez, "so_residual_step_z": 0.5,
                        "group_fo_quote": fo, "multiplier": mult,
                        "max_legs": 4, "exit_z": 0.5, "tp_net_bps_floor": 25,
                        "leverage": LEV, "group_gross_cap_pct": CAP,
                        "cycle_deadline_h": 168,
                        "inventory_reservation_skew_k": 0,
                        "jump_first_passage_gate": None,
                        "regime_envelope": None, "c1_scheduler": None,
                    }, "fits": fits, "budget_quote": float(BUDGET)}
                    cfg_path = DIR / f"{tag}.json"
                    cfg_path.write_text(
                        json.dumps(cfg, indent=2, sort_keys=True))
                    cfg_sha = hashlib.sha256(
                        cfg_path.read_bytes()).hexdigest()
                    fp = ln.fingerprint(
                        family="C1E",
                        cycle_topology="sync_corrected",
                        trigger_contract_sha=f"ez{ez}_m{mult}",
                        fit_contract_sha=hashlib.sha256(
                            json.dumps(fits, sort_keys=True).encode()).hexdigest()[:16],
                        universe_group_weights_sha=f"{len(fits)}g",
                        scheduler_sha=f"lev{LEV}",
                        resolved_config_sha=cfg_sha[:16],
                        effective_config_sha=cfg_sha[:16],
                        cost_model_sha="fee7_slip5", engine_sha=engine_sha(),
                        market_data_sha=md_sha(), funding_data_sha=fd_sha(),
                        exchange_filter_snapshot_sha="default",
                        maintenance_tiers_sha="0.5pct",
                        borrow_snapshot_sha=None,
                        window=(block["test_start_ms"],
                                block["test_end_ms"]),
                        budget=float(BUDGET), fold=block["block_id"],
                        block="g1c", seed=20261101)
                    if ln.is_duplicate(fp):
                        continue
                    row = ln.run_synchronized_cycle(
                        experiment_id=tag, parent_id=None,
                        fingerprint=fp, config_path=cfg_path,
                        budget=float(BUDGET),
                        start_ms=block["test_start_ms"],
                        end_ms=block["test_end_ms"],
                        fit_start_ms=block["fit_start_ms"],
                        fit_end_ms=block["fit_end_ms"],
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
                           "so": m.get("groups_with_so"),
                           "symc": m.get("max_symbol_concentration_pct"),
                           "grpc": m.get("max_group_concentration_pct")}
                    results.append(rec)
                    if ann >= 50:
                        tier = ("CONS" if ann >= 50 and dd <= 10
                                else "BAL" if ann >= 90 and dd <= 20
                                else "AGG" if ann >= 110 and dd <= 30 else "")
                        print(f"  {tag}: ann={ann:.1f}% dd={dd:.2f}% "
                              f"sym={rec['sym']} so={rec['so']} {tier}")
    elapsed = time.time() - start_t
    # Per-block best + cross-fit positivity
    by_block = {}
    for r in results:
        by_block.setdefault(r["block"], []).append(r)
    summary = {
        "phase": "R21 G1 corrected sweep (strict cross-fit, fixed ADF gate)",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "blocks": list(by_block.keys()),
        "total_complete": len(results),
        "per_block_best": {},
        "cross_fit_positivity": {},
    }
    for blk, rows in by_block.items():
        rows.sort(key=lambda r: -r["ann"])
        summary["per_block_best"][blk] = rows[0] if rows else None
        positive = sum(1 for r in rows if r["ann"] > 0)
        summary["cross_fit_positivity"][blk] = {
            "positive_configs": positive, "total": len(rows)}
    OUT = ART / "g1" / "gates" / "g1_corrected.json"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    print(f"total_complete: {len(results)}, elapsed: {elapsed:.1f}s")
    for blk, best in summary["per_block_best"].items():
        if best:
            print(f"  {blk} best: ann={best['ann']:.1f}% dd={best['dd']:.2f}% "
                  f"m={best['mult']} fo={best['fo']}")


if __name__ == "__main__":
    main()
