#!/usr/bin/env python3
"""Round 18 R5: G0 binding + adversarial activation (plan §11 G0).

For the M1 family, prove EVERY open parameter changes the event/trade/
rejection trace (the R17 A4 bug was that families passed binding on config-hash
change alone, with event_hash_bound=false). G0 requires:

  - 8-16 synthetic-ish traces per family (we use the frozen F2 train fits,
    varied across the full param grid => >=8 distinct configs);
  - >=4 real window replays covering bull trend / bear trend / range / jump-shock;
  - each open parameter must produce an expected state + order/rejection delta.
    Any inert parameter stops the family (plan §11 G0).

Method: for each M1 parameter, run a low and a high value across 4 adversarial
real windows and assert the event/trade/rejection stream differs in at least
one window. Use the frozen F2 fits (train-only). No fast-screen — every run is
a full synchronized_cycle_replay.
"""
import hashlib
import json
import os
import subprocess
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round18"
R5 = os.path.join(ART, "r5")
CONFIGS = os.path.join(R5, "configs")
os.makedirs(CONFIGS, exist_ok=True)

R4_FITS = os.path.join(ART, "r4", "fold-fit-freeze.json")

# Adversarial real windows (epoch ms): bull trend / bear trend / range / jump-shock
WINDOWS = [
    ("bull_trend", 1696118400000, 1704067200000),    # 2023-10-01..2024-01-01 (Q4 rally)
    ("bear_trend", 1736294400000, 1744070400000),    # 2025-01-08..2025-04-08 (drawdown)
    ("range", 1714521600000, 1719792000000),         # 2024-05-01..2024-07-01 (chop)
    ("jump_shock", 1729468800000, 1734480000000),    # 2024-10-21..2024-12-18 (post-election vol)
]


def load_f2_fits():
    """Load frozen F2 fits from R4. F2 is unblocked (4 disjoint pairs)."""
    d = json.load(open(R4_FITS))
    f2 = next(f for f in d["fold_fits"] if f["fold"] == "F2")
    return f2["frozen_fits"], f2["train_selection_commit_sha256"]


def build_config(frozen_fits, **overrides):
    base = {
        "family": "M1_pair", "bar_boundary_minutes": 5,
        "fit_lookback_days": 60, "entry_z": 1.5, "so_residual_step_z": 0.40,
        "group_fo_quote": 30.0, "multiplier": 1.35, "max_legs": 4, "exit_z": 0.25,
        "tp_net_bps_floor": 40, "leverage": 3, "group_gross_cap_pct": 18.0,
        "pairs": [], "factor": None, "basket_symbols": [], "cycle_deadline_h": None,
    }
    base.update(overrides)
    return {"synchronized_cycle": base, "fits": frozen_fits, "budget_quote": 4999.0}


def run(cfg, start, end, label):
    path = os.path.join(CONFIGS, f"{label}.json")
    with open(path, "w") as f:
        json.dump(cfg, f, sort_keys=True)
    cmd = [
        "target/release/synchronized_cycle_replay", "--config", path,
        "--budget", "4999", "--start-ms", str(start), "--end-ms", str(end),
        "--market-data", "data/market_data_full.db",
        "--funding-data", "data/funding_rates_round12.db",
    ]
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
        return {"ok": False, "err": "non-json stdout", "wall_s": round(wall, 1)}
    ss = out.get("sync_summary", {})
    digests = ss.get("trace_digests", {})
    return {
        "ok": True, "wall_s": round(wall, 1),
        "event_stream_sha": digests.get("event_stream_sha256", ""),
        "trade_stream_sha": digests.get("trade_stream_sha256", ""),
        "rejection_stream_sha": digests.get("rejection_stream_sha256", ""),
        "trade_count": out.get("metrics", {}).get("trade_count", 0),
        "group_fo": ss.get("group_fo", []), "group_so": ss.get("group_so", []),
        "groups_with_so": ss.get("groups_with_so", 0),
        "ann": out.get("metrics", {}).get("annualized_return_pct"),
        "dd": out.get("metrics", {}).get("max_drawdown_pct"),
        "breach": out.get("metrics", {}).get("breach"),
        "resolved_config_sha256": out.get("resolved_config_sha256", ""),
    }


# Each M1 open parameter with a low/high value (plan §7.3 ranges).
PARAM_SWEEPS = [
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


def differs(a, b):
    return (a.get("event_stream_sha") != b.get("event_stream_sha")
            or a.get("trade_stream_sha") != b.get("trade_stream_sha")
            or a.get("rejection_stream_sha") != b.get("rejection_stream_sha")
            or a.get("trade_count") != b.get("trade_count"))


def main():
    frozen_fits, f2_commit = load_f2_fits()
    print(f"F2 commit: {f2_commit[:16]}, {len(frozen_fits)} frozen pairs")

    base_cfg = build_config(frozen_fits)
    results = {"base": {}, "param_sweeps": {}}
    # Run base config across all 4 windows first.
    base_runs = {}
    for wname, ws, we in WINDOWS:
        res = run(base_cfg, ws, we, f"g0_base_{wname}")
        base_runs[wname] = res
        if res["ok"]:
            print(f"  base/{wname}: trade={res['trade_count']} fo={res['group_fo']} so={res['group_so']} breach={res['breach']} wall={res['wall_s']}s")
        else:
            print(f"  base/{wname}: FAIL {res.get('err','')[:80]}")
    results["base"] = {w: {k: v for k, v in r.items() if k != "ok"} for w, r in base_runs.items()}

    # For each parameter, run low and high across the 4 windows; assert the
    # trace differs from base AND low differs from high in >=1 window.
    inert_params = []
    binding_summary = {}
    total_runs = 0
    for param, low_kw, high_kw in PARAM_SWEEPS:
        param_active_windows = []
        low_runs, high_runs = {}, {}
        for wname, ws, we in WINDOWS:
            low_res = run(build_config(frozen_fits, **low_kw), ws, we, f"g0_{param}_low_{wname}")
            high_res = run(build_config(frozen_fits, **high_kw), ws, we, f"g0_{param}_high_{wname}")
            total_runs += 2
            low_runs[wname] = low_res
            high_runs[wname] = high_res
            if low_res["ok"] and high_res["ok"] and differs(low_res, high_res):
                param_active_windows.append(wname)
        # also confirm changing the param changes the trace vs base
        vs_base_active = []
        for wname, _ws, _we in WINDOWS:
            if (low_runs[wname]["ok"] and base_runs[wname]["ok"] and differs(low_runs[wname], base_runs[wname])) \
               or (high_runs[wname]["ok"] and base_runs[wname]["ok"] and differs(high_runs[wname], base_runs[wname])):
                vs_base_active.append(wname)
        binding_summary[param] = {
            "low_vs_high_active_windows": param_active_windows,
            "vs_base_active_windows": vs_base_active,
            "low": {w: {"trade_count": r.get("trade_count"), "fo": r.get("group_fo"), "so": r.get("group_so")} for w, r in low_runs.items() if r.get("ok")},
            "high": {w: {"trade_count": r.get("trade_count"), "fo": r.get("group_fo"), "so": r.get("group_so")} for w, r in high_runs.items() if r.get("ok")},
        }
        is_inert = (len(param_active_windows) == 0)
        if is_inert:
            inert_params.append(param)
        print(f"  param {param}: active_in={param_active_windows} inert={is_inert}")

    verdict = {
        "family": "M1_pair",
        "f2_train_commit_sha256": f2_commit,
        "param_sweep_count": len(PARAM_SWEEPS),
        "total_runs": total_runs + len(WINDOWS),
        "windows": [{"name": n, "start": s, "end": e} for n, s, e in WINDOWS],
        "inert_params": inert_params,
        "all_params_bound": len(inert_params) == 0,
        "binding_detail": binding_summary,
        "base_runs": results["base"],
    }
    out_path = os.path.join(R5, "g0_binding.json")
    with open(out_path, "w") as f:
        json.dump(verdict, f, indent=2)
    print(f"\nwrote {out_path}")
    print(f"all_params_bound={verdict['all_params_bound']}  inert={inert_params}")
    return verdict["all_params_bound"]


if __name__ == "__main__":
    ok = main()
    raise SystemExit(0 if ok else 1)
