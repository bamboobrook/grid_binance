#!/usr/bin/env python3
"""Round 20 concentrated push: best neighborhood survivor (cap=30) at higher
budgets + slightly higher risk, validated one-shot on F4."""
import json
import os
import subprocess
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round20"
PUSH = os.path.join(ART, "concentrated_push")
CONFIGS = os.path.join(PUSH, "configs")
os.makedirs(CONFIGS, exist_ok=True)

F4_VAL_START = 1767820800000
F4_VAL_END = 1780271999999


def load_c1_fits():
    d = json.load(open(os.path.join(ART, "p4", "families-fit.json")))
    ff = next(f for f in d["fold_fits"] if f["fold"] == "F4")
    return ff["C1"]["frozen_fits"]


def run(cfg, budget, start, end, label):
    path = os.path.join(CONFIGS, f"{label}.json")
    with open(path, "w") as f:
        json.dump(cfg, f, sort_keys=True)
    cmd = ["target/release/synchronized_cycle_replay", "--config", path,
           "--budget", str(int(budget)), "--start-ms", str(start), "--end-ms", str(end),
           "--market-data", "data/market_data_full.db",
           "--funding-data", "data/funding_rates_round12.db"]
    t0 = time.time()
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=900)
    except subprocess.TimeoutExpired:
        return {"ok": False, "err": "timeout"}
    wall = time.time() - t0
    if r.returncode != 0:
        return {"ok": False, "err": (r.stderr or "")[-200:], "wall_s": round(wall, 1)}
    try:
        out = json.loads(r.stdout)
    except json.JSONDecodeError:
        return {"ok": False, "err": "non-json"}
    m = out["metrics"]; ss = out["sync_summary"]
    return {"ok": True, "wall_s": round(wall, 1),
            "ann": m["annualized_return_pct"], "dd": m["max_drawdown_pct"],
            "trade_count": m["trade_count"], "breach": m["breach"],
            "groups_with_so": ss.get("groups_with_so", 0),
            "actual_symbols": ss.get("actual_symbol_count", 0),
            "max_symbol_conc": ss.get("max_symbol_abs_net_pnl_share_pct", 0),
            "max_group_conc": ss.get("max_group_abs_net_pnl_share_pct", 0)}


def build(params, fits):
    base = {"family": "M1_pair", "bar_boundary_minutes": 5, "fit_lookback_days": 60,
            "factor": None, "basket_symbols": [], "cycle_deadline_h": None, "pairs": []}
    base.update(params)
    return {"synchronized_cycle": base, "fits": fits}


def main():
    fits = load_c1_fits()
    # cap=30 was best; vary budget + multiplier + fo to push ann up
    configs = [
        # cap=30 at various budgets
        {"label": "cap30_mult135_fo60", "entry_z": 1.0, "so_residual_step_z": 0.6,
         "group_fo_quote": 60.0, "multiplier": 1.35, "max_legs": 4, "leverage": 3,
         "exit_z": 0.25, "tp_net_bps_floor": 40, "group_gross_cap_pct": 30.0},
        # higher mult
        {"label": "cap30_mult150_fo60", "entry_z": 1.0, "so_residual_step_z": 0.6,
         "group_fo_quote": 60.0, "multiplier": 1.50, "max_legs": 4, "leverage": 3,
         "exit_z": 0.25, "tp_net_bps_floor": 40, "group_gross_cap_pct": 30.0},
        {"label": "cap30_mult170_fo60", "entry_z": 1.0, "so_residual_step_z": 0.6,
         "group_fo_quote": 60.0, "multiplier": 1.70, "max_legs": 4, "leverage": 3,
         "exit_z": 0.25, "tp_net_bps_floor": 40, "group_gross_cap_pct": 30.0},
        # higher fo + mult
        {"label": "cap30_mult150_fo80", "entry_z": 1.0, "so_residual_step_z": 0.6,
         "group_fo_quote": 80.0, "multiplier": 1.50, "max_legs": 4, "leverage": 3,
         "exit_z": 0.25, "tp_net_bps_floor": 40, "group_gross_cap_pct": 30.0},
        # more legs
        {"label": "cap30_mult150_legs5", "entry_z": 1.0, "so_residual_step_z": 0.6,
         "group_fo_quote": 60.0, "multiplier": 1.50, "max_legs": 5, "leverage": 3,
         "exit_z": 0.25, "tp_net_bps_floor": 40, "group_gross_cap_pct": 30.0},
        # entry 1.5 + higher mult
        {"label": "cap30_mult150_ez15", "entry_z": 1.5, "so_residual_step_z": 0.25,
         "group_fo_quote": 60.0, "multiplier": 1.50, "max_legs": 4, "leverage": 3,
         "exit_z": 0.25, "tp_net_bps_floor": 100, "group_gross_cap_pct": 30.0},
    ]
    budgets = [1000, 2000, 3000, 4999]
    print(f"Concentrated push: {len(configs)} configs × {len(budgets)} budgets, direct F4 validation")
    results = []
    for c in configs:
        params = {k: v for k, v in c.items() if k != "label"}
        for budget in budgets:
            res = run(build(params, fits), budget, F4_VAL_START, F4_VAL_END,
                      f"push_{c['label']}_{budget}")
            if res["ok"]:
                results.append({"label": c["label"], "budget": budget, **res})
                print(f"  {c['label']} b={budget}: val ann={res['ann']:.1f}% dd={res['dd']:.1f}% breach={res['breach']} so={res['groups_with_so']} sym={res['actual_symbols']}")
            else:
                print(f"  {c['label']} b={budget}: ERROR {res.get('err','')[:60]}")
    summary = {"phase": "concentrated push F4 validation", "configs": len(configs),
               "budgets": budgets, "results": results}
    out_path = os.path.join(PUSH, "summary.json")
    with open(out_path, "w") as f:
        json.dump(summary, f, indent=2)
    print(f"\nwrote {out_path}")
    # best
    valid = [r for r in results if not r.get("breach") and r.get("groups_with_so", 0) > 0]
    valid.sort(key=lambda x: -x["ann"])
    if valid:
        b = valid[0]
        print(f"BEST: {b['label']} b={b['budget']} val ann={b['ann']:.1f}% dd={b['dd']:.1f}%")
        print(f"three-tier conservative (50%/10%): {'HIT' if b['ann']>=50 and b['dd']<=10 else 'NO'}")


if __name__ == "__main__":
    main()
