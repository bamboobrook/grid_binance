#!/usr/bin/env python3
"""P4 G2 full-development: take the G1 Pareto survivors and run full-window
dev-window replays + 5 cold-start train segments. Plan §7 G2: "complete train,
5 cold-start train segments, account constraints and concentration; output
<=8 per fold".

G2 train median ann / worst DD / >=3/4 train blocks positive decides who
passes to G3.
"""

import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from glm_r15_search_runner import (  # noqa: E402
    DEV_START, DEV_END, SEGMENTS, append_registry, concentration_metrics,
    effective_config_hash, run_replay, write_config,
)

ART = "docs/superpowers/artifacts/glm-martingale-core-round15"
T3_8 = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT", "TRXUSDT"]


def partial_tp(final_bps):
    return {"partial": {"stages": [[300, 1000, 800], [429, 1000, 1600], [1, 1, final_bps]], "breakeven_after_stage": 1, "breakeven_buffer_bps": 100}}


def build_config(fo, mult, legs, sp, fbps):
    n = len(T3_8); wt = round(100.0 / n, 4)
    strats = []
    for sym in T3_8:
        strats.append({
            "strategy_id": f"L-{sym}", "symbol": sym, "market": "usd_m_futures",
            "direction": "long", "direction_mode": "long_only", "margin_mode": "isolated",
            "leverage": 10, "spacing": {"fixed_percent": {"step_bps": int(sp)}},
            "sizing": {"multiplier": {"first_order_quote": str(int(fo)), "multiplier": str(mult), "max_legs": int(legs)}},
            "take_profit": partial_tp(int(fbps)), "stop_loss": None, "indicators": [],
            "entry_triggers": [{"cooldown": {"seconds": 39600}}],
            "portfolio_weight_pct": str(wt), "risk_limits": {},
        })
    return {"direction_mode": "long_only", "risk_limits": {"max_global_budget_quote": "4999"}, "strategies": strats}


def run_one(label, cfg, budget, start, end, window):
    path, eff = write_config({"portfolio_config": cfg}, label)
    running = {
        "experiment_id": label, "family": "T3_8_g2_fulldev",
        "hypothesis": "G2 full-dev + segments", "resolved_config_hash": eff,
        "effective_config_hash": eff, "window": window, "budget": budget,
        "start_ms": start, "end_ms": end, "seed": None, "optimizer": "g2_pareto_survivor",
        "status": "running", "started_at": time.time(),
    }
    append_registry(running)
    result = run_replay(path, budget, start, end, timeout=600)
    record = dict(running)
    record.update(result)
    record["status"] = result.get("status", "error")
    record["actual_binary_replays"] = 1 if result.get("status") == "complete" else 0
    record["cache_hits"] = 0
    record["finished_at"] = time.time()
    if result.get("status") == "complete":
        record["concentration"] = concentration_metrics(result)
    append_registry(record)
    return result


def main():
    g1 = json.load(open(os.path.join(ART, "p4", "g1-sobol-screen.json")))
    survivors = g1["pareto_configs"][:6]  # G2 input (<=8 per fold)
    out = []
    for s in survivors:
        p = s["params"]
        cfg = build_config(p["fo"], p["m"], p["legs"], p["sp"], p["fbps"])
        label_base = f"r15_g2_{p['fo']}_{p['m']}_{p['legs']}_{p['sp']}_{p['fbps']}"
        # full dev
        full = run_one(label_base + "_full", cfg, 4999.0, DEV_START, DEV_END, "full_dev")
        seg_anns = []
        seg_pos = 0
        seg_results = []
        for name, st, en in SEGMENTS:
            r = run_one(label_base + "_" + name, cfg, 4999.0, st, en, "seg_" + name)
            a = r.get("ann")
            seg_anns.append(a)
            seg_results.append({"segment": name, "ann": a, "dd": r.get("dd"), "breached": r.get("principal_breached")})
            if a is not None and a > 0:
                seg_pos += 1
        full_ann = full.get("ann")
        full_dd = full.get("dd")
        conc = concentration_metrics(full) if full.get("status") == "complete" else {}
        entry = {
            "params": p, "full_ann": full_ann, "full_dd": full_dd,
            "conc_gp": conc.get("max_gross_profit_share"),
            "breached": full.get("principal_breached"),
            "segments": seg_results, "positive_segments": seg_pos,
            "train_median_ann": sorted([a for a in seg_anns if a is not None])[len([a for a in seg_anns if a is not None]) // 2] if any(a is not None for a in seg_anns) else None,
            "g2_pass": (full.get("principal_breached") is False and seg_pos >= 3),
        }
        out.append(entry)
        print(f"G2 {p}: full ann={full_ann} dd={full_dd} breached={full.get('principal_breached')} pos={seg_pos}/5 median={entry['train_median_ann']} g2_pass={entry['g2_pass']}", flush=True)

    out_path = os.path.join(ART, "p4", "g2-full-dev.json")
    json.dump({"schema_version": 1, "phase": "P4_G2", "survivors_in": len(survivors), "results": out}, open(out_path, "w"), indent=2, sort_keys=True)
    print(f"\nwrote {out_path}")


if __name__ == "__main__":
    main()
