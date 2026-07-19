#!/usr/bin/env python3
"""Round 19 R5: G0 binding + adversarial activation for M1R + M2F (plan §7).

For each family, prove EVERY open parameter changes the event/trade/rejection
trace across >=4 real windows (bull/bear/range/jump). Plan §7 G0 binding.

M1R parameters (plan §7.3): entry_z, so_residual_step_z, group_fo_quote,
multiplier, max_legs, exit_z, tp_net_bps_floor, leverage, group_gross_cap_pct,
bar_boundary_minutes.

M2F parameters: entry_z, so_residual_step_z, group_fo_quote, multiplier,
max_legs, exit_z, tp_net_bps_floor, leverage, group_gross_cap_pct.

Every run is a FULL synchronized_cycle_replay (no fast-screen).
"""
import hashlib
import json
import os
import subprocess
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round19"
R5 = os.path.join(ART, "r5")
CONFIGS = os.path.join(R5, "configs")
os.makedirs(CONFIGS, exist_ok=True)
R4_FITS = os.path.join(ART, "r4", "families-fit.json")

# Adversarial real windows (epoch ms): bull/bear/range/jump-shock
WINDOWS = [
    ("bull_trend", 1696118400000, 1704067200000),    # 2023-10-01..2024-01-01
    ("bear_trend", 1736294400000, 1744070400000),    # 2025-01-08..2025-04-08
    ("range", 1714521600000, 1719792000000),         # 2024-05-01..2024-07-01
    ("jump_shock", 1729468800000, 1734480000000),    # 2024-10-21..2024-12-18
]


def load_fold_fits(fold, family):
    d = json.load(open(R4_FITS))
    ff = next(f for f in d["fold_fits"] if f["fold"] == fold)
    return ff[family]["frozen_fits"], ff["train_selection_commit_sha256"]


def build_m1r_config(fits, **kw):
    base = {"family": "M1_pair", "bar_boundary_minutes": 5, "fit_lookback_days": 60,
            "entry_z": 1.5, "so_residual_step_z": 0.40, "group_fo_quote": 30.0,
            "multiplier": 1.35, "max_legs": 4, "exit_z": 0.25, "tp_net_bps_floor": 40,
            "leverage": 3, "group_gross_cap_pct": 18.0,
            "pairs": [], "factor": None, "basket_symbols": [], "cycle_deadline_h": None}
    base.update(kw)
    return {"synchronized_cycle": base, "fits": fits, "budget_quote": 4999.0}


def build_m2f_config(fits, **kw):
    base = {"family": "M2F", "bar_boundary_minutes": 5, "fit_lookback_days": 60,
            "entry_z": 1.5, "so_residual_step_z": 0.40, "group_fo_quote": 60.0,
            "multiplier": 1.35, "max_legs": 4, "exit_z": 0.25, "tp_net_bps_floor": 40,
            "leverage": 3, "group_gross_cap_pct": 25.0,
            "pairs": [], "factor": "BTC", "basket_symbols": [], "cycle_deadline_h": None}
    base.update(kw)
    return {"synchronized_cycle": base, "fits": fits, "budget_quote": 4999.0}


def run(cfg, start, end, label):
    path = os.path.join(CONFIGS, f"{label}.json")
    with open(path, "w") as f:
        json.dump(cfg, f, sort_keys=True)
    cmd = ["target/release/synchronized_cycle_replay", "--config", path,
           "--budget", "4999", "--start-ms", str(start), "--end-ms", str(end),
           "--market-data", "data/market_data_full.db",
           "--funding-data", "data/funding_rates_round12.db"]
    t0 = time.time()
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    except subprocess.TimeoutExpired:
        return {"ok": False, "err": "timeout", "wall_s": 600.0}
    wall = time.time() - t0
    if r.returncode != 0:
        return {"ok": False, "err": (r.stderr or "")[-400:], "wall_s": round(wall, 1)}
    try:
        out = json.loads(r.stdout)
    except json.JSONDecodeError:
        return {"ok": False, "err": "non-json", "wall_s": round(wall, 1)}
    ss = out.get("sync_summary", {})
    dig = ss.get("trace_digests", {})
    return {"ok": True, "wall_s": round(wall, 1),
            "event_sha": dig.get("event_stream_sha256", ""),
            "trade_sha": dig.get("trade_stream_sha256", ""),
            "rej_sha": dig.get("rejection_stream_sha256", ""),
            "trade_count": out.get("metrics", {}).get("trade_count", 0),
            "actual_symbols": ss.get("actual_symbol_count", 0),
            "group_fo": ss.get("group_fo", []), "group_so": ss.get("group_so", []),
            "ann": out.get("metrics", {}).get("annualized_return_pct"),
            "dd": out.get("metrics", {}).get("max_drawdown_pct"),
            "breach": out.get("metrics", {}).get("breach")}


def differs(a, b):
    return (a.get("event_sha") != b.get("event_sha")
            or a.get("trade_sha") != b.get("trade_sha")
            or a.get("rej_sha") != b.get("rej_sha")
            or a.get("trade_count") != b.get("trade_count"))


M1R_PARAMS = [
    ("entry_z", {"entry_z": 1.0}, {"entry_z": 2.5}),
    ("so_residual_step_z", {"so_residual_step_z": 0.25}, {"so_residual_step_z": 0.80}),
    ("group_fo_quote", {"group_fo_quote": 20.0}, {"group_fo_quote": 60.0}),
    ("multiplier", {"multiplier": 1.20}, {"multiplier": 1.70}),
    ("max_legs", {"max_legs": 3}, {"max_legs": 5}),
    ("exit_z", {"exit_z": 0.10}, {"exit_z": 0.50}),
    ("tp_net_bps_floor", {"tp_net_bps_floor": 20}, {"tp_net_bps_floor": 100}),
    ("leverage", {"leverage": 2}, {"leverage": 5}),
    ("group_gross_cap_pct", {"group_gross_cap_pct": 12.0}, {"group_gross_cap_pct": 25.0}),
    ("bar_boundary_minutes", {"bar_boundary_minutes": 1}, {"bar_boundary_minutes": 5}),
]

M2F_PARAMS = [
    ("entry_z", {"entry_z": 1.0}, {"entry_z": 2.5}),
    ("so_residual_step_z", {"so_residual_step_z": 0.25}, {"so_residual_step_z": 0.80}),
    ("group_fo_quote", {"group_fo_quote": 40.0}, {"group_fo_quote": 100.0}),
    ("multiplier", {"multiplier": 1.20}, {"multiplier": 1.50}),
    ("max_legs", {"max_legs": 3}, {"max_legs": 5}),
    ("exit_z", {"exit_z": 0.10}, {"exit_z": 0.50}),
    ("tp_net_bps_floor", {"tp_net_bps_floor": 20}, {"tp_net_bps_floor": 100}),
    ("leverage", {"leverage": 2}, {"leverage": 5}),
    ("group_gross_cap_pct", {"group_gross_cap_pct": 18.0}, {"group_gross_cap_pct": 35.0}),
]


def sweep_family(family, fold, build_fn, param_list):
    fits, commit = load_fold_fits(fold, family)
    base_cfg = build_fn(fits)
    base_runs = {}
    for wname, ws, we in WINDOWS:
        base_runs[wname] = run(base_cfg, ws, we, f"g0_{family}_base_{wname}")
    binding = {}
    inert = []
    total_runs = len(WINDOWS)
    for param, low_kw, high_kw in param_list:
        active = []
        low_runs, high_runs = {}, {}
        for wname, ws, we in WINDOWS:
            lo = run(build_fn(fits, **low_kw), ws, we, f"g0_{family}_{param}_low_{wname}")
            hi = run(build_fn(fits, **high_kw), ws, we, f"g0_{family}_{param}_high_{wname}")
            total_runs += 2
            low_runs[wname] = lo; high_runs[wname] = hi
            if lo["ok"] and hi["ok"] and differs(lo, hi):
                active.append(wname)
        if not active:
            inert.append(param)
        binding[param] = {"active_windows": active, "inert": not active,
                          "low_sample": {w: {"trade": r.get("trade_count"), "fo": r.get("group_fo"), "actual_sym": r.get("actual_symbols")} for w, r in low_runs.items() if r.get("ok")},
                          "high_sample": {w: {"trade": r.get("trade_count"), "fo": r.get("group_fo"), "actual_sym": r.get("actual_symbols")} for w, r in high_runs.items() if r.get("ok")}}
        print(f"  {family} param {param}: active_in={active} inert={not active}")
    return {"family": family, "fold": fold, "commit": commit,
            "param_count": len(param_list), "inert_params": inert,
            "all_bound": len(inert) == 0, "binding": binding,
            "base_runs": {w: {"trade": r.get("trade_count"), "fo": r.get("group_fo"), "actual_sym": r.get("actual_symbols"), "breach": r.get("breach")} for w, r in base_runs.items() if r.get("ok")},
            "total_runs": total_runs}


def main():
    # M1R uses F2 fold fits (F2 is unblocked for M1R).
    m1r = sweep_family("M1R", "F2", build_m1r_config, M1R_PARAMS)
    # M2F uses F2 fold fits (M2F unblocked in all folds).
    m2f = sweep_family("M2F", "F2", build_m2f_config, M2F_PARAMS)

    verdict = {
        "phase": "R5 G0 binding + adversarial activation",
        "windows": [{"name": n, "start": s, "end": e} for n, s, e in WINDOWS],
        "families": [m1r, m2f],
        "all_implemented_families_bound": m1r["all_bound"] and m2f["all_bound"],
        "blocked_families_P1_K1_V1": "blocked_implementation_scope (R4); no G0 binding",
    }
    out_path = os.path.join(R5, "g0_binding.json")
    with open(out_path, "w") as f:
        json.dump(verdict, f, indent=2)
    print(f"\nwrote {out_path}")
    print(f"M1R all_bound={m1r['all_bound']} inert={m1r['inert_params']}")
    print(f"M2F all_bound={m2f['all_bound']} inert={m2f['inert_params']}")


if __name__ == "__main__":
    main()
