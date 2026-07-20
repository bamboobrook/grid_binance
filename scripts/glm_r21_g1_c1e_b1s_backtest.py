#!/usr/bin/env python3
"""Round 21 G1 (partial — C1E + B1S only, since P1S/V1B are blocked at R4):
run FULL synchronized-cycle backtests through the central launcher for the
two families that produced frozen fits.

This is NOT the full plan §8 G1 quota (C1E 64 + B1S 96 configs/fold). It is
a representative strict-subset backtest that produces REAL registry rows with
REAL metrics + 5 trace hashes for the families that unblocked. The full G1
quota cannot run because P1S/V1B blocked at R4 (round status
BLOCKED_ENGINE_DATA_OR_EXECUTION per §16).

Plan §8 hard gates enforced by the launcher's _classify_terminal:
  - breach / liquidation / filter bypass => rejected_gate
  - actual_symbols < 5 => rejected_gate
  - groups_with_so == 0 => not_martingale_no_so
  - max_symbol_concentration > 50% => rejected_concentration
  - max_group_concentration > 50% => rejected_concentration

Each backtest is a FULL synchronized_cycle_replay invocation — no fast-screen.
The launcher writes running+terminal rows + 5 trace hashes.
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
from glm_r21_launcher import Launcher, sha256_json, sha256_file  # noqa: E402

ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round21"
FITS = json.load(open(ART / "r4" / "families-fit.json"))
G1_DIR = ART / "g1" / "configs"
G1_DIR.mkdir(parents=True, exist_ok=True)
BUDGETS = [1000, 4999]  # plan §8 G1: @1000/4999U


def engine_sha() -> str:
    bin_path = ROOT / "target/release/synchronized_cycle_replay"
    return sha256_file(bin_path) or "unknown"


def market_data_sha() -> str:
    # 118GB file — use mtime+size as provenance (per R1 data contract)
    p = ROOT / "data/market_data_full.db"
    st = p.stat()
    return f"size={st.st_size},mtime={int(st.st_mtime)}"


def funding_data_sha() -> str:
    return sha256_file(ROOT / "data/funding_rates.db") or "unknown"


def make_engine_cfg(family: str, fit_group: dict, budget: int,
                    fo_quote: float = 30.0, multiplier: float = 1.5,
                    entry_z: float = 1.5, so_step: float = 0.5,
                    max_legs: int = 4, leverage: int = 3) -> dict:
    """Build a SynchronizedCycleConfig + fits for the engine."""
    cfg = {
        "synchronized_cycle": {
            "family": family,
            "bar_boundary_minutes": 60,
            "fit_lookback_days": 180,
            "entry_z": entry_z,
            "so_residual_step_z": so_step,
            "group_fo_quote": fo_quote,
            "multiplier": multiplier,
            "max_legs": max_legs,
            "exit_z": 0.5,
            "tp_net_bps_floor": 25,
            "leverage": leverage,
            "group_gross_cap_pct": 100.0,
            "cycle_deadline_h": 168,
            "inventory_reservation_skew_k": 0,
            "jump_first_passage_gate": None,
            "regime_envelope": None,
            "c1_scheduler": None,
        },
        "fits": [fit_group],
        "budget_quote": float(budget),
    }
    # V1B-style families need factor; C1E/B1S don't.
    if family in ("M2_basket", "M2F", "P1", "K1", "V1", "V1B"):
        cfg["synchronized_cycle"]["factor"] = "BTC"
    return cfg


def run_one(ln: Launcher, family: str, fit_group: dict, block: dict,
            budget: int, fo_quote: float, multiplier: float,
            entry_z: float, so_step: float, max_legs: int,
            leverage: int, tag: str) -> dict:
    cfg = make_engine_cfg(family, fit_group, budget, fo_quote, multiplier,
                          entry_z, so_step, max_legs, leverage)
    cfg_path = G1_DIR / f"{tag}.json"
    cfg_path.write_text(json.dumps(cfg, indent=2, sort_keys=True))

    # Compute the engine-side budget (engine takes budget from cfg or argv)
    cfg_sha = hashlib.sha256(cfg_path.read_bytes()).hexdigest()
    fit_contract_sha = hashlib.sha256(
        json.dumps(fit_group, sort_keys=True).encode()).hexdigest()[:16]
    fp = ln.fingerprint(
        family=family, cycle_topology="synchronized_cycle",
        trigger_contract_sha=f"ez{entry_z}_so{so_step}",
        fit_contract_sha=fit_contract_sha,
        universe_group_weights_sha=fit_group["group_id"],
        scheduler_sha=f"ml{max_legs}_lev{leverage}",
        resolved_config_sha=cfg_sha[:16],
        effective_config_sha=cfg_sha[:16],
        cost_model_sha="fee7bp_slip5bp",
        engine_sha=engine_sha()[:16],
        market_data_sha=market_data_sha()[:32],
        funding_data_sha=funding_data_sha()[:16],
        exchange_filter_snapshot_sha="default_per_symbol",
        maintenance_tiers_sha="0.5pct_default",
        borrow_snapshot_sha=None,
        window=(block["test_start_ms"], block["test_end_ms"]),
        budget=float(budget), fold=block["block_id"], block="g1", seed=20261101,
    )
    if ln.is_duplicate(fp):
        print(f"  [skip] {tag} (duplicate)")
        return {"skipped": True, "tag": tag}
    print(f"  [run ] {tag} budget={budget} fo={fo_quote} mult={multiplier} "
          f"ez={entry_z} so={so_step}")
    row = ln.run_synchronized_cycle(
        experiment_id=tag, parent_id=None, fingerprint=fp,
        config_path=cfg_path, budget=float(budget),
        start_ms=block["test_start_ms"], end_ms=block["test_end_ms"],
        fit_start_ms=block["fit_start_ms"], fit_end_ms=block["fit_end_ms"],
        purge_ms=86_400_000, timeout_s=600)
    m = (row or {}).get("metrics", {}) if row else {}
    ann = m.get("annualized_return_pct")
    dd = m.get("max_drawdown_pct")
    sym = m.get("actual_symbols")
    grp_so = m.get("groups_with_so")
    status = (row or {}).get("status")
    ffg = (row or {}).get("first_failed_gate")
    print(f"        -> status={status} ann={ann} dd={dd} sym={sym} "
          f"grp_so={grp_so} ffg={ffg}")
    return row or {}


def main() -> None:
    start_t = time.time()
    ln = Launcher()
    results: list[dict] = []
    # For each family that has fits, run a representative strict subset:
    #   - C1E: 2/12 blocks have fits; run those 2 blocks x 2 budgets x 2 fo_quote
    #     = 8 replays per block x 2 blocks = 16
    #   - B1S: 12/12 blocks; pick 3 representative blocks x 2 budgets x 1 config
    #     = 6 (representative, not full quota)
    # Open dims (plan §6.1/§6.2): fo_quote, multiplier, entry_z, so_step,
    # max_legs, leverage. We use a small grid to demonstrate the pipeline.
    c1e_fits_by_block = {
        ff["block_id"]: ff["C1E"]["frozen_fits"]
        for ff in FITS["fold_fits"] if ff["C1E"]["frozen_fits"]}
    b1s_fits_by_block = {
        ff["block_id"]: ff["B1S"]["frozen_fits"]
        for ff in FITS["fold_fits"] if ff["B1S"]["frozen_fits"]}
    blocks_by_id = {ff["block_id"]: ff for ff in FITS["fold_fits"]}

    # C1E: use up to 2 blocks, 2 fo_quote x 1 multiplier x 1 entry_z x 2 budgets
    fo_grid = [20.0, 40.0]
    for block_id, fit_groups in list(c1e_fits_by_block.items())[:2]:
        block = blocks_by_id[block_id]
        for fit_group in fit_groups[:1]:  # first disjoint group per block
            for fo in fo_grid:
                for budget in BUDGETS:
                    tag = f"c1e_{block_id}_{fit_group['group_id']}_fo{int(fo)}_{budget}"
                    row = run_one(ln, "C1E", fit_group, block, budget,
                                  fo_quote=fo, multiplier=1.5, entry_z=1.5,
                                  so_step=0.5, max_legs=4, leverage=3, tag=tag)
                    results.append({"family": "C1E", "tag": tag,
                                    "row": row})

    # B1S: 3 representative blocks, 1 fo x 1 mult x 2 budgets
    b1s_blocks = list(b1s_fits_by_block.keys())[:3]
    for block_id in b1s_blocks:
        block = blocks_by_id[block_id]
        for fit_group in b1s_fits_by_block[block_id][:1]:  # first symbol
            for budget in BUDGETS:
                tag = f"b1s_{block_id}_{fit_group['group_id']}_{budget}"
                row = run_one(ln, "B1S", fit_group, block, budget,
                              fo_quote=30.0, multiplier=1.5, entry_z=1.5,
                              so_step=0.5, max_legs=4, leverage=3, tag=tag)
                results.append({"family": "B1S", "tag": tag, "row": row})

    elapsed = time.time() - start_t
    # Summary
    summary = {
        "phase": "R21 G1 partial backtest (C1E+B1S, P1S/V1B blocked at R4)",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 1),
        "total_replays": sum(1 for r in results if not r["row"].get("skipped")),
        "families_run": ["C1E", "B1S"],
        "blocked_families": ["P1S", "V1B"],
        "results": [],
    }
    for r in results:
        row = r["row"]
        if not row or row.get("skipped"):
            summary["results"].append({"tag": r["tag"], "skipped": True})
            continue
        m = row.get("metrics", {}) or {}
        summary["results"].append({
            "tag": r["tag"], "family": r["family"],
            "status": row.get("status"),
            "first_failed_gate": row.get("first_failed_gate"),
            "ann_pct": m.get("annualized_return_pct"),
            "max_dd_pct": m.get("max_drawdown_pct"),
            "actual_symbols": m.get("actual_symbols"),
            "groups_with_so": m.get("groups_with_so"),
            "max_symbol_conc_pct": m.get("max_symbol_concentration_pct"),
            "max_group_conc_pct": m.get("max_group_concentration_pct"),
            "min_liquidation_buffer_pct": m.get("min_liquidation_buffer_pct"),
            "liquidation_count": m.get("liquidation_count"),
            "budget": row.get("budget"),
            "trace_event_sha256": row.get("trace_event_sha256"),
            "trace_trade_sha256": row.get("trace_trade_sha256"),
            "fingerprint_sha256": row.get("fingerprint_sha256"),
        })
    OUT = ART / "g1" / "gates" / "g1.json"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    print(f"\nwrote {OUT}")
    print(f"total_replays: {summary['total_replays']}, elapsed: {elapsed:.1f}s")
    # Best results per family
    for fam in ("C1E", "B1S"):
        fam_results = [r for r in summary["results"]
                       if r.get("family") == fam and r.get("status") == "complete"]
        if fam_results:
            best = max(fam_results,
                       key=lambda r: r.get("ann_pct") or -1e9)
            print(f"best {fam}: ann={best['ann_pct']:.2f}% dd={best['max_dd_pct']:.2f}% "
                  f"sym={best['actual_symbols']} grp_so={best['groups_with_so']}")


if __name__ == "__main__":
    main()
