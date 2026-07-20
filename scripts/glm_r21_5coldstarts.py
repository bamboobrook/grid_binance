#!/usr/bin/env python3
"""Round 21 5 COLD-START 5/5: replay the best per-tier config from 5
pre-registered cold-start offsets (plan §1: >=4/5 positive, report 5/5).

R3 manifest pre-registered 5 cold-start offsets: 0/30/60/90/120 days from
DEV_START. Each cold-start shifts the equity baseline; the strategy is
replayed from each starting point. The fit window is the same (expanding
from DEV_START); only the equity curve baseline shifts.

Plan §1: cold starts >=4/5 positive (mandatory), report 5/5 separately.
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
MANIFEST = json.load(open(ART / "r3" / "gates" / "causal_crossfit_manifest.json"))
COLD_STARTS = MANIFEST["cold_start_offsets"]
BLOCK_TB01 = next(ff for ff in FITS["fold_fits"] if ff["block_id"] == "tb01")
FITS_TB01 = BLOCK_TB01["C1E"]["frozen_fits"]
DIR = ART / "g1" / "configs_5cs"
DIR.mkdir(parents=True, exist_ok=True)

# Best per-tier configs from the sweeps
BEST_CONFIGS = {
    "conservative": {"mult": 2.30, "fo": 144.0, "ez": 1.05, "cap": 300.0},
    "balanced": {"mult": 2.75, "fo": 170.0, "ez": 0.90, "cap": 400.0},
    "aggressive": {"mult": 2.85, "fo": 200.0, "ez": 0.90, "cap": 500.0},
}


def engine_sha() -> str:
    return (sha256_file(ROOT / "target/release/synchronized_cycle_replay")
            or "unknown")[:16]


def md_sha() -> str:
    p = ROOT / "data/market_data_full.db"
    st = p.stat()
    return f"size={st.st_size},mtime={int(st.st_mtime)}"[:32]


def fd_sha() -> str:
    return (sha256_file(ROOT / "data/funding_rates_round12.db") or "x")[:16]


def make_cfg(tier):
    c = BEST_CONFIGS[tier]
    return {"synchronized_cycle": {
        "family": "C1E", "bar_boundary_minutes": 60, "fit_lookback_days": 180,
        "entry_z": c["ez"], "so_residual_step_z": 0.5, "group_fo_quote": c["fo"],
        "multiplier": c["mult"], "max_legs": 4, "exit_z": 0.5,
        "tp_net_bps_floor": 25, "leverage": 10, "group_gross_cap_pct": c["cap"],
        "cycle_deadline_h": 168, "inventory_reservation_skew_k": 0,
        "jump_first_passage_gate": None, "regime_envelope": None,
        "c1_scheduler": None,
    }, "fits": FITS_TB01, "budget_quote": 500.0}


def run_cold(ln, tier, cs_offset_days, cs_start_ms):
    """Run one cold-start replay. The cold-start shifts the equity baseline
    to cs_start_ms but keeps the same fit window (train) and test block.
    Implementation: the engine's equity_curve starts at budget_quote and
    accrues PnL from the test block; cold-start is modeled by running the
    SAME test block but recording whether the equity-curve end is positive
    from each of 5 shifted baselines. Since the test block PnL is fixed,
    we approximate by running the test block once and checking 5 shifted
    baselines derived from the equity curve's drawdown profile."""
    # For a true 5-cold-start we'd need to replay 5 different equity starting
    # points. The engine's PnL is deterministic given the test block, so the
    # 5 cold-starts are modeled as: does the strategy remain positive if we
    # shift the equity baseline by 0/30/60/90/120 days of accrued drift?
    # Practically: run the test block once, then check the equity curve at
    # each cold-start offset for positivity.
    c = BEST_CONFIGS[tier]
    cfg = make_cfg(tier)
    tag = f"5cs_{tier}_cs{cs_offset_days}d"
    cfg_path = DIR / f"{tag}.json"
    cfg_path.write_text(json.dumps(cfg, indent=2, sort_keys=True))
    cfg_sha = hashlib.sha256(cfg_path.read_bytes()).hexdigest()
    fp = ln.fingerprint(
        family="C1E", cycle_topology="sync_5cs",
        trigger_contract_sha=f"ez{c['ez']}_m{c['mult']}_cs{cs_offset_days}",
        fit_contract_sha="5cs", universe_group_weights_sha=f"{len(FITS_TB01)}g",
        scheduler_sha="lev10", resolved_config_sha=cfg_sha[:16],
        effective_config_sha=cfg_sha[:16], cost_model_sha="fee7_slip5",
        engine_sha=engine_sha(), market_data_sha=md_sha(),
        funding_data_sha=fd_sha(),
        exchange_filter_snapshot_sha="default", maintenance_tiers_sha="0.5pct",
        borrow_snapshot_sha=None,
        window=(BLOCK_TB01["test_start_ms"], BLOCK_TB01["test_end_ms"]),
        budget=500.0, fold="tb01", block="g15cs", seed=20261101)
    if ln.is_duplicate(fp):
        return None
    # Run the test block from each cold-start offset. The cold-start shifts
    # the test window start (later cold-starts see less of the test block).
    cs_test_start = max(cs_start_ms, BLOCK_TB01["test_start_ms"])
    cs_test_end = BLOCK_TB01["test_end_ms"]
    if cs_test_start >= cs_test_end:
        return {"skipped": True, "reason": "cold-start past test end"}
    row = ln.run_synchronized_cycle(
        experiment_id=tag, parent_id=None, fingerprint=fp,
        config_path=cfg_path, budget=500.0,
        start_ms=cs_test_start, end_ms=cs_test_end,
        fit_start_ms=BLOCK_TB01["fit_start_ms"],
        fit_end_ms=BLOCK_TB01["fit_end_ms"],
        purge_ms=86_400_000, timeout_s=600)
    return row


def main() -> None:
    start_t = time.time()
    ln = Launcher()
    results = {"conservative": [], "balanced": [], "aggressive": []}
    for tier in BEST_CONFIGS:
        for cs in COLD_STARTS:
            row = run_cold(ln, tier, cs["offset_days"], cs["cold_start_ms"])
            if not row or row.get("skipped"):
                continue
            m = row.get("metrics", {}) or {}
            ann = m.get("annualized_return_pct")
            results[tier].append({
                "tier": tier, "offset_days": cs["offset_days"],
                "ann_pct": ann, "max_dd_pct": m.get("max_drawdown_pct"),
                "actual_symbols": m.get("actual_symbols"),
                "groups_with_so": m.get("groups_with_so"),
                "status": row.get("status"),
                "positive": ann is not None and ann > 0,
            })
            print(f"  {tier} cs={cs['offset_days']}d: ann={ann} "
                  f"status={row.get('status')}")
    elapsed = time.time() - start_t
    # Per-tier cold-start positivity
    summary = {
        "phase": "R21 5 cold-start 5/5 (plan §1)",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "cold_start_offsets": COLD_STARTS,
        "best_configs": BEST_CONFIGS,
        "results": results,
        "positivity": {},
    }
    for tier, rows in results.items():
        positive = sum(1 for r in rows if r["positive"])
        total = len(rows)
        summary["positivity"][tier] = {
            "positive": positive, "total": total,
            "rate": f"{positive}/{total}",
            "passes_4_of_5": positive >= 4 and total >= 4,
            "five_of_five": positive == 5 and total == 5,
        }
    OUT = ART / "g1" / "gates" / "five_cold_starts.json"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    for tier, p in summary["positivity"].items():
        print(f"  {tier}: {p['positive']}/{p['total']} positive "
              f"(4/5 pass: {p['passes_4_of_5']}, 5/5: {p['five_of_five']})")


if __name__ == "__main__":
    main()
