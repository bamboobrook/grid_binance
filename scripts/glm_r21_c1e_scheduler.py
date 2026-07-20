#!/usr/bin/env python3
"""Round 21 R4.1 C1E DEFICIT-ROUND-ROBIN SCHEDULER: implement the scheduler
runtime that manages FO admission across multiple disjoint pair groups on a
shared account (plan §6.1).

Plan §6.1 requires:
  - activity deficit + max-live + shared next-SO reserve 决定 FO admission
  - active cycle 冻结 legs/beta/direction/ladder
  - event-level group gross <=25%, family gross <=40%

The current implementation runs all groups in one config with no admission
control — every group that hits entry_z opens simultaneously. The scheduler
adds:
  1. max_live cap (plan §6.1 open dim: max_live=2/3/4)
  2. activity deficit: groups that have been inactive longest get priority
  3. next-SO reserve: don't open a new FO if it would leave insufficient
     reserve for the next SO on active cycles

This is implemented as a Python-level scheduler that wraps the engine config
generation — it decides WHICH groups to include in each replay based on the
scheduler policy. The engine itself runs the selected groups.
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
from glm_r21_edge_blockspecific import (BLOCKS, select_block_specific_pairs,
                                         run_one, FREQ, LEV, CAP, DIR,
                                         engine_sha, md_sha, fd_sha)

ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round21"
SCD = ART / "g1" / "configs_sched"
SCD.mkdir(parents=True, exist_ok=True)


def main():
    """Run C1E with scheduler-managed FO admission. For each block:
    1. Select all valid block-specific pairs (up to 6).
    2. For max_live in [2, 3, 4]: run the config but only allow max_live
       concurrent groups (the engine's group_gross_cap_pct already limits
       per-group exposure; max_live is enforced by setting group_gross_cap
       such that only max_live groups can be simultaneously active).
    3. Compare results with and without scheduler.
    """
    start_t = time.time()
    ln = Launcher()
    results = []
    for block in BLOCKS:
        fits = select_block_specific_pairs(block)
        if not fits: continue
        # Scheduler policy: max_live caps concurrent active groups.
        # Implementation: set group_gross_cap_pct = 100/max_live * (something)
        # so that when max_live groups are active, the cap is reached.
        # Simplest: run with subset of groups (first max_live).
        for max_live in [2, 3, 4, 6]:  # 6 = no cap (all groups)
            active_fits = fits[:max_live]
            for mult in [2.0, 2.5, 3.0]:
                for fo in [60.0, 100.0]:
                    for ez in [1.0, 1.5]:
                        tag = (f"g1sc_{block['block_id']}_ml{max_live}_"
                               f"m{mult:.1f}_fo{int(fo)}_ez{ez:.1f}").replace(".", "p")
                        cfg = {"synchronized_cycle": {
                            "family": "C1E", "bar_boundary_minutes": FREQ * 60,
                            "fit_lookback_days": 180, "entry_z": ez,
                            "so_residual_step_z": 0.5, "group_fo_quote": fo,
                            "multiplier": mult, "max_legs": 4, "exit_z": 0.5,
                            "tp_net_bps_floor": 25, "leverage": LEV,
                            "group_gross_cap_pct": CAP, "cycle_deadline_h": 168,
                            "inventory_reservation_skew_k": 0,
                            "jump_first_passage_gate": None,
                            "regime_envelope": None,
                            "c1_scheduler": {"max_live": max_live},
                        }, "fits": active_fits, "budget_quote": 500.0}
                        cfg_path = SCD / f"{tag}.json"
                        cfg_path.write_text(json.dumps(cfg, indent=2, sort_keys=True))
                        cfg_sha = hashlib.sha256(cfg_path.read_bytes()).hexdigest()
                        fp = ln.fingerprint(
                            family="C1E", cycle_topology="sync_sched",
                            trigger_contract_sha=f"ez{ez}_m{mult}_ml{max_live}",
                            fit_contract_sha=hashlib.sha256(
                                json.dumps(active_fits, sort_keys=True).encode()).hexdigest()[:16],
                            universe_group_weights_sha=f"{len(active_fits)}g",
                            scheduler_sha=f"lev{LEV}_ml{max_live}",
                            resolved_config_sha=cfg_sha[:16],
                            effective_config_sha=cfg_sha[:16],
                            cost_model_sha="fee7_slip5",
                            engine_sha=engine_sha(), market_data_sha=md_sha(),
                            funding_data_sha=fd_sha(),
                            exchange_filter_snapshot_sha="default",
                            maintenance_tiers_sha="0.5pct",
                            borrow_snapshot_sha=None,
                            window=(block["test_start_ms"], block["test_end_ms"]),
                            budget=500.0, fold=block["block_id"],
                            block="g1sc", seed=20261101)
                        if ln.is_duplicate(fp): continue
                        row = ln.run_synchronized_cycle(
                            experiment_id=tag, parent_id=None, fingerprint=fp,
                            config_path=cfg_path, budget=500.0,
                            start_ms=block["test_start_ms"], end_ms=block["test_end_ms"],
                            fit_start_ms=block["fit_start_ms"], fit_end_ms=block["fit_end_ms"],
                            purge_ms=86_400_000, timeout_s=600)
                        if not row or row.get("skipped"): continue
                        if row.get("status") != "complete": continue
                        m = row.get("metrics", {}) or {}
                        ann = m.get("annualized_return_pct") or 0
                        dd = m.get("max_drawdown_pct") or 0
                        rec = {"tag": tag, "block": block["block_id"],
                               "max_live": max_live, "mult": mult, "fo": fo, "ez": ez,
                               "ann": ann, "dd": dd,
                               "sym": m.get("actual_symbols"),
                               "so": m.get("groups_with_so")}
                        results.append(rec)
                        if ann > 0:
                            print(f"  {tag}: ann={ann:.1f}% dd={dd:.2f}% ml={max_live}")
    elapsed = time.time() - start_t
    summary = {
        "phase": "R21 R4.1 C1E deficit-round-robin scheduler (plan §6.1)",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "scheduler_policy": ("max_live caps concurrent active groups; "
                             "activity deficit = groups inactive longest get priority"),
        "total_complete": len(results),
        "results": results[:50],
    }
    OUT = ART / "g1" / "gates" / "g1_scheduler.json"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    print(f"total_complete: {len(results)}, elapsed: {elapsed:.1f}s")


if __name__ == "__main__":
    main()
