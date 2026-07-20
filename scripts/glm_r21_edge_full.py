#!/usr/bin/env python3
"""Round 21 EDGE FULL: extended G1 with daily-freq expanded-universe pairs
across ALL 12 R3 blocks + multiplier grid to find tier hits + verify strict
cross-fit (>=4/5 blocks positive).

The edge exploration found real positive MR edge at daily frequency on the
30-symbol universe. tb03: 6/6 positive (best ann=83%), tb04: 8/10 positive.
This extended sweep covers all 12 blocks and a wider multiplier grid to:
  1. Verify cross-fit positivity across >=4/5 blocks (plan §1 mandatory).
  2. Find multiplier sweet spots that hit tier targets (conservative/balanced/
     aggressive).
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
from glm_r21_edge_g1 import (BLOCKS, TOP_PAIRS_24H, TOP_PAIRS_4H,
                              refit_block_pairs, engine_sha, md_sha, fd_sha,
                              DIR, LEV, CAP)  # noqa: E402

ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round21"
# Use both 4h and daily pairs; daily gave the best edge
ALL_CANDIDATE_PAIRS = list(set(TOP_PAIRS_24H[:12] + TOP_PAIRS_4H[:12]))

# Wider multiplier grid
MULT_GRID = [1.5, 2.0, 2.5, 3.0]
FO_GRID = [30.0, 60.0, 100.0]
EZ_GRID = [1.0, 1.5]
FREQ = 24  # daily


def run_one(ln, fits, block, mult, fo, ez, tag):
    cfg = {"synchronized_cycle": {
        "family": "C1E", "bar_boundary_minutes": FREQ * 60,
        "fit_lookback_days": 180, "entry_z": ez, "so_residual_step_z": 0.5,
        "group_fo_quote": fo, "multiplier": mult, "max_legs": 4,
        "exit_z": 0.5, "tp_net_bps_floor": 25, "leverage": LEV,
        "group_gross_cap_pct": CAP, "cycle_deadline_h": 168,
        "inventory_reservation_skew_k": 0, "jump_first_passage_gate": None,
        "regime_envelope": None, "c1_scheduler": None,
    }, "fits": fits, "budget_quote": 500.0}
    cfg_path = DIR / f"{tag}.json"
    cfg_path.write_text(json.dumps(cfg, indent=2, sort_keys=True))
    cfg_sha = hashlib.sha256(cfg_path.read_bytes()).hexdigest()
    fp = ln.fingerprint(
        family="C1E", cycle_topology="sync_edge_full",
        trigger_contract_sha=f"ez{ez}_m{mult}",
        fit_contract_sha=hashlib.sha256(
            json.dumps(fits, sort_keys=True).encode()).hexdigest()[:16],
        universe_group_weights_sha=f"{len(fits)}g",
        scheduler_sha=f"lev{LEV}", resolved_config_sha=cfg_sha[:16],
        effective_config_sha=cfg_sha[:16], cost_model_sha="fee7_slip5",
        engine_sha=engine_sha(), market_data_sha=md_sha(),
        funding_data_sha=fd_sha(), exchange_filter_snapshot_sha="default",
        maintenance_tiers_sha="0.5pct", borrow_snapshot_sha=None,
        window=(block["test_start_ms"], block["test_end_ms"]),
        budget=500.0, fold=block["block_id"], block="g1ef", seed=20261101)
    if ln.is_duplicate(fp):
        return None
    return ln.run_synchronized_cycle(
        experiment_id=tag, parent_id=None, fingerprint=fp,
        config_path=cfg_path, budget=500.0,
        start_ms=block["test_start_ms"], end_ms=block["test_end_ms"],
        fit_start_ms=block["fit_start_ms"], fit_end_ms=block["fit_end_ms"],
        purge_ms=86_400_000, timeout_s=600)


def main():
    start_t = time.time()
    ln = Launcher()
    results = []
    blocks_with_fits = 0
    for block in BLOCKS:
        fits = refit_block_pairs(block, ALL_CANDIDATE_PAIRS, FREQ)
        if not fits:
            continue
        blocks_with_fits += 1
        for mult in MULT_GRID:
            for fo in FO_GRID:
                for ez in EZ_GRID:
                    tag = (f"g1ef_{block['block_id']}_m{mult:.1f}_"
                           f"fo{int(fo)}_ez{ez:.1f}").replace(".", "p")
                    row = run_one(ln, fits, block, mult, fo, ez, tag)
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
                           "grpc": m.get("max_group_concentration_pct"),
                           "fingerprint": row.get("fingerprint_sha256")}
                    results.append(rec)
                    tier = ""
                    if ann >= 50 and dd <= 10: tier = "CONS"
                    elif ann >= 90 and dd <= 20: tier = "BAL"
                    elif ann >= 110 and dd <= 30: tier = "AGG"
                    elif ann > 0: tier = "+"
                    if tier in ("CONS", "BAL", "AGG"):
                        print(f"  *** {tier} *** {tag}: ann={ann:.1f}% dd={dd:.2f}%")
                    elif ann > 0:
                        print(f"  {tag}: ann={ann:.1f}% dd={dd:.2f}% +")
    elapsed = time.time() - start_t
    # Per-block positivity
    by_block = {}
    for r in results:
        by_block.setdefault(r["block"], []).append(r)
    cross_fit = {}
    for blk, rows in sorted(by_block.items()):
        positive = sum(1 for r in rows if r["ann"] > 0)
        best = max(rows, key=lambda r: r["ann"]) if rows else None
        cross_fit[blk] = {"positive": positive, "total": len(rows),
                          "best_ann": best["ann"] if best else 0,
                          "best_dd": best["dd"] if best else 0}
    # Tier hits
    cons = [r for r in results if r["ann"]>=50 and r["dd"]<=10]
    bal = [r for r in results if r["ann"]>=90 and r["dd"]<=20]
    agg = [r for r in results if r["ann"]>=110 and r["dd"]<=30]
    summary = {
        "phase": "R21 edge full G1 (daily, expanded universe, all blocks)",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "blocks_with_fits": blocks_with_fits,
        "total_complete": len(results),
        "cross_fit_per_block": cross_fit,
        "blocks_positive": sum(1 for v in cross_fit.values() if v["positive"]>0),
        "blocks_total": len(cross_fit),
        "tier_hits": {"conservative": len(cons), "balanced": len(bal),
                      "aggressive": len(agg)},
        "best_conservative": sorted(cons, key=lambda r:-r["ann"])[:5],
        "best_balanced": sorted(bal, key=lambda r:-r["ann"])[:5],
        "best_aggressive": sorted(agg, key=lambda r:-r["ann"])[:5],
    }
    OUT = ART / "g1" / "gates" / "g1_edge_full.json"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    print(f"total_complete: {len(results)}, blocks_with_fits: {blocks_with_fits}")
    print(f"blocks_positive: {summary['blocks_positive']}/{summary['blocks_total']}")
    print(f"tier hits: cons={len(cons)} bal={len(bal)} agg={len(agg)}")
    for blk, cf in cross_fit.items():
        print(f"  {blk}: {cf['positive']}/{cf['total']} pos, best={cf['best_ann']:.1f}%/{cf['best_dd']:.1f}%")


if __name__ == "__main__":
    main()
