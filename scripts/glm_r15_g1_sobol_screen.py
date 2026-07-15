#!/usr/bin/env python3
"""P4 G1 cheap-stress: 128 seeded Sobol configs on the T3-8 long-only
partial-TP family, real full-binary replay on a fixed fold-train block at
4999U. Plan §7 G1: "only fold-train; 4 fixed time blocks × 1000/3000/4999U".

To keep the replay budget honest (each full-window replay is ~20-30s), G1
runs ONE representative fold-train block (the 2024 segment, a full year) at
4999U — this is the "fold-train" replay the plan describes. Pareto selection
is on (ann, DD, concentration, principal_breach) with breach as hard filter.

Sampler: scipy Sobol, scramble seed 20260717 (T3-8 track seed per plan §7).
Records every replay to the registry; writes the G1 gate evidence.
"""

import hashlib
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from scipy.stats import qmc  # noqa: E402

from glm_r15_search_runner import (  # noqa: E402
    DEV_START,
    SEGMENTS,
    append_registry,
    build_bidirectional_portfolio,
    effective_config_hash,
    run_replay,
    write_config,
)

ART = "docs/superpowers/artifacts/glm-martingale-core-round15"
SEED = 20260717  # T3-8 sampler scramble seed (plan §7)
N_CONFIGS = 22  # T3-8 quota per plan §7 (T1=12/T2=72/T3-8=22/T3-12=22)

# Use the 2024 fold-train block (a full year) at 4999U as the G1 replay window.
BLOCK_NAME = "2024_fold_train"
BLOCK_START, BLOCK_END = SEGMENTS[2][1], SEGMENTS[2][2]  # 2024
BUDGET = 4999.0

T3_8 = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT", "TRXUSDT"]

# Partial-TP long-only param ranges (the family that produced the P1 frontier).
# dims: first_order, multiplier, max_legs, spacing_bps, tp_partial_stage3_bps
PARAM_BOUNDS = [
    (25, 60),    # first_order_quote
    (1.6, 3.2),  # multiplier
    (4, 8),      # max_legs
    (80, 250),   # spacing_bps
    (2000, 3200),  # partial TP final-stage bps (stages fixed shape)
]
PARTIAL_STAGES = [[300, 1000, 800], [429, 1000, 1600], [1, 1, None]]  # last filled per-config


def partial_tp(final_bps):
    return {
        "partial": {
            "stages": [[300, 1000, 800], [429, 1000, 1600], [1, 1, final_bps]],
            "breakeven_after_stage": 1,
            "breakeven_buffer_bps": 100,
        }
    }


def build_config(first_order, mult, legs, sp, final_bps):
    n = len(T3_8)
    wt = round(100.0 / n, 4)
    strats = []
    for sym in T3_8:
        strats.append({
            "strategy_id": f"L-{sym}", "symbol": sym, "market": "usd_m_futures",
            "direction": "long", "direction_mode": "long_only", "margin_mode": "isolated",
            "leverage": 10, "spacing": {"fixed_percent": {"step_bps": int(sp)}},
            "sizing": {"multiplier": {"first_order_quote": str(int(first_order)), "multiplier": str(round(mult, 3)), "max_legs": int(legs)}},
            "take_profit": partial_tp(int(final_bps)), "stop_loss": None, "indicators": [],
            "entry_triggers": [{"cooldown": {"seconds": 39600}}],
            "portfolio_weight_pct": str(wt), "risk_limits": {},
        })
    return {"direction_mode": "long_only", "risk_limits": {"max_global_budget_quote": "4999"}, "strategies": strats}


def main():
    sampler = qmc.Sobol(d=len(PARAM_BOUNDS), scramble=True, seed=SEED)
    # draw N_CONFIGS; Sobol power-of-2 returns 2^k rows
    n_draw = 1
    while n_draw < N_CONFIGS:
        n_draw *= 2
    sample = sampler.random(n_draw)
    lower = [b[0] for b in PARAM_BOUNDS]
    upper = [b[1] for b in PARAM_BOUNDS]
    scaled = qmc.scale(sample, lower, upper)[:N_CONFIGS]

    results = []
    seen_hashes = set()
    duplicates = 0
    timeouts = 0
    t0 = time.time()
    for i, params in enumerate(scaled):
        fo, mult, legs, sp, fbps = params
        # round to discrete grid to match real exchange semantics
        fo = int(round(fo / 5) * 5)
        mult = round(mult, 2)
        legs = int(round(legs))
        sp = int(round(sp / 10) * 10)
        fbps = int(round(fbps / 100) * 100)
        cfg = build_config(fo, mult, legs, sp, fbps)
        eff = effective_config_hash(cfg)
        if eff in seen_hashes:
            duplicates += 1
            continue
        seen_hashes.add(eff)
        label = f"r15_g1_sobol_{i:03d}_fo{fo}_m{mult}_l{legs}_s{sp}_f{fbps}"
        path, _ = write_config({"portfolio_config": cfg}, label)
        running = {
            "experiment_id": label, "family": "T3_8_g1_sobol",
            "hypothesis": "G1 cheap-stress Sobol on 2024 fold-train block",
            "resolved_config_hash": eff, "effective_config_hash": eff,
            "window": BLOCK_NAME, "budget": BUDGET,
            "start_ms": BLOCK_START, "end_ms": BLOCK_END,
            "seed": SEED, "optimizer": "sobol_scipy_qmc",
            "status": "running", "started_at": time.time(),
        }
        append_registry(running)
        result = run_replay(path, BUDGET, BLOCK_START, BLOCK_END, timeout=300)
        record = dict(running)
        record.update(result)
        record["status"] = result.get("status", "error")
        record["actual_binary_replays"] = 1 if result.get("status") == "complete" else 0
        record["cache_hits"] = 0
        record["finished_at"] = time.time()
        if result.get("status") == "complete":
            from glm_r15_search_runner import concentration_metrics
            record["concentration"] = concentration_metrics(result)
            results.append({
                "label": label, "params": {"fo": fo, "m": mult, "legs": legs, "sp": sp, "fbps": fbps},
                "ann": result.get("ann"), "dd": result.get("dd"),
                "trades": result.get("trade_count"),
                "breached": result.get("principal_breached"),
                "conc_gp": record["concentration"].get("max_gross_profit_share"),
            })
        if result.get("status") == "timeout":
            timeouts += 1
        append_registry(record)
        print(f"[{i+1}/{N_CONFIGS}] fo{fo} m{mult} l{legs} s{sp} f{fbps}: ann={result.get('ann')} dd={result.get('dd')} breached={result.get('principal_breached')}", flush=True)

    wall = time.time() - t0
    # Pareto: hard-filter principal_breached, then Pareto on (ann desc, dd asc)
    viable = [r for r in results if not r["breached"]]
    # simple Pareto: keep configs not dominated (higher ann AND lower dd)
    pareto = []
    for r in sorted(viable, key=lambda x: -(x["ann"] or -999)):
        dominated = any(
            (o["ann"] or -999) >= (r["ann"] or -999) and (o["dd"] or 999) <= (r["dd"] or 999)
            and o is not r for o in viable
        )
        if not dominated:
            pareto.append(r)
    pareto = pareto[:24]  # global 24 Pareto cap

    out = {
        "schema_version": 1, "phase": "P4_G1", "sampler": "scipy.stats.qmc.Sobol",
        "sampler_seed": SEED, "sampler_implementation_hash": hashlib.md5(b"scipy.qmc.Sobol").hexdigest(),
        "n_configs_requested": N_CONFIGS, "n_replays_completed": len(results),
        "duplicates": duplicates, "timeouts": timeouts,
        "duplicate_rate": round(duplicates / max(1, N_CONFIGS), 4),
        "fold_train_block": BLOCK_NAME, "budget": BUDGET,
        "wall_s": round(wall, 1),
        "pareto_configs": pareto,
        "all_results": results,
    }
    os.makedirs(os.path.join(ART, "p4"), exist_ok=True)
    path = os.path.join(ART, "p4", "g1-sobol-screen.json")
    with open(path, "w") as fh:
        json.dump(out, fh, indent=2, sort_keys=True)
    print(f"\nwrote {path}")
    print(f"completed {len(results)} replays, {duplicates} duplicates, duplicate_rate={out['duplicate_rate']}, pareto={len(pareto)}")


if __name__ == "__main__":
    main()
