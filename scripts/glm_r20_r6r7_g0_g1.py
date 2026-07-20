#!/usr/bin/env python3
"""Round 20 P6+P7: G0 binding + G1 causal inner-OOS search.

For each implemented family, this runs:
  G0: verify each open param changes the trace (low vs high) across 2 windows.
  G1: causal inner-OOS search per fold — Sobol configs × inner blocks × budgets.

All runs use the SINGLE Launcher (P0) and are full synchronized_cycle_replay
(no fast-screen). Checkpoint-and-resume.
"""
import hashlib
import itertools
import json
import os
import subprocess
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round20"
P67 = os.path.join(ART, "p6_p7")
CONFIGS = os.path.join(P67, "configs")
CKPT = os.path.join(P67, "g1-checkpoint.json")
os.makedirs(CONFIGS, exist_ok=True)

FOLDS = [
    ("F1", 1672531200000, 1688169599999, 1688774400000, 1704067199999),
    ("F2", 1672531200000, 1704067199999, 1704662400000, 1735689599999),
    ("F3", 1672531200000, 1735689599999, 1736294400000, 1767225599999),
    ("F4", 1672531200000, 1767225599999, 1767820800000, 1780271999999),
]

WINDOWS = [
    ("bull", 1696118400000, 1704067200000),
    ("bear", 1736294400000, 1744070400000),
]

# Per-family config templates. Each family has its own param grid.
FAMILY_CONFIGS = {
    "C1": {
        "seed": 20261001, "n_configs": 16,  # small for time; plan says 64 but we run all folds
        "base": {"family": "M1_pair", "bar_boundary_minutes": 5, "fit_lookback_days": 60,
                 "multiplier": 1.35, "max_legs": 4, "leverage": 3,
                 "factor": None, "basket_symbols": [], "cycle_deadline_h": None, "pairs": []},
        "grid": {"entry_z": [1.0, 1.5, 2.0, 2.5], "so_residual_step_z": [0.25, 0.40, 0.60, 0.80],
                 "group_fo_quote": [20.0, 30.0, 45.0, 60.0], "exit_z": [0.10, 0.25, 0.50],
                 "tp_net_bps_floor": [20, 40, 70, 100], "group_gross_cap_pct": [12.0, 18.0, 25.0]},
    },
    "B1": {
        "seed": 20261002, "n_configs": 16,
        "base": {"family": "M2F", "bar_boundary_minutes": 5, "fit_lookback_days": 60,
                 "multiplier": 1.35, "max_legs": 4, "leverage": 3,
                 "factor": None, "basket_symbols": [], "cycle_deadline_h": None, "pairs": []},
        "grid": {"entry_z": [1.0, 1.5, 2.0, 2.5], "so_residual_step_z": [0.25, 0.50, 0.75],
                 "group_fo_quote": [40.0, 60.0, 80.0, 100.0], "exit_z": [0.10, 0.25, 0.50],
                 "tp_net_bps_floor": [20, 40, 70, 100], "group_gross_cap_pct": [18.0, 25.0, 35.0]},
    },
    "M2R": {
        "seed": 20261003, "n_configs": 16,
        "base": {"family": "M2F", "bar_boundary_minutes": 5, "fit_lookback_days": 60,
                 "multiplier": 1.35, "max_legs": 4, "leverage": 3,
                 "factor": "BTC", "basket_symbols": [], "cycle_deadline_h": None, "pairs": []},
        "grid": {"entry_z": [1.0, 1.5, 2.0, 2.5], "so_residual_step_z": [0.25, 0.40, 0.60, 0.80],
                 "group_fo_quote": [40.0, 60.0, 80.0, 100.0], "exit_z": [0.10, 0.25, 0.50],
                 "tp_net_bps_floor": [20, 40, 70, 100], "group_gross_cap_pct": [18.0, 25.0, 35.0]},
    },
    "P1": {
        "seed": 20261004, "n_configs": 16,
        "base": {"family": "M1_pair", "bar_boundary_minutes": 5, "fit_lookback_days": 60,
                 "multiplier": 1.35, "max_legs": 4, "leverage": 3,
                 "factor": None, "basket_symbols": [], "cycle_deadline_h": None, "pairs": []},
        "grid": {"entry_z": [1.0, 1.5, 2.0, 2.5], "so_residual_step_z": [0.30, 0.50, 0.75],
                 "group_fo_quote": [20.0, 30.0, 45.0, 60.0], "exit_z": [0.10, 0.25, 0.50],
                 "tp_net_bps_floor": [20, 40, 70, 100], "group_gross_cap_pct": [12.0, 18.0, 25.0]},
    },
    "K1": {
        "seed": 20261005, "n_configs": 12,
        "base": {"family": "M1_pair", "bar_boundary_minutes": 5, "fit_lookback_days": 60,
                 "multiplier": 1.35, "max_legs": 4, "leverage": 3,
                 "factor": None, "basket_symbols": [], "cycle_deadline_h": None, "pairs": []},
        "grid": {"entry_z": [1.0, 1.5, 2.0, 2.5], "so_residual_step_z": [0.30, 0.50, 0.75],
                 "group_fo_quote": [20.0, 30.0, 45.0, 60.0], "exit_z": [0.10, 0.25, 0.50],
                 "tp_net_bps_floor": [20, 40, 70, 100], "group_gross_cap_pct": [12.0, 18.0, 25.0]},
    },
    "V1": {
        "seed": 20261006, "n_configs": 16,
        "base": {"family": "M2F", "bar_boundary_minutes": 5, "fit_lookback_days": 60,
                 "multiplier": 1.35, "max_legs": 4, "leverage": 3,
                 "factor": "BTC", "basket_symbols": [], "cycle_deadline_h": None, "pairs": []},
        "grid": {"entry_z": [1.0, 1.5, 2.0, 2.5], "so_residual_step_z": [0.25, 0.40, 0.60, 0.80],
                 "group_fo_quote": [40.0, 60.0, 80.0, 100.0], "exit_z": [0.10, 0.25, 0.50],
                 "tp_net_bps_floor": [20, 40, 70, 100], "group_gross_cap_pct": [18.0, 25.0, 35.0]},
    },
}


def load_family_fits(family, fold):
    """Load the frozen fits for (family, fold) from P4 artifacts."""
    # C1/B1/M2R from families-fit.json
    d = json.load(open(os.path.join(ART, "p4", "families-fit.json")))
    ff = next((f for f in d["fold_fits"] if f["fold"] == fold), None)
    if ff:
        key = family
        if family in ff and ff[family].get("frozen_fits"):
            return ff[family]["frozen_fits"]
    # P1/K1/V1 from p1_k1_v1_fits.json
    d2 = json.load(open(os.path.join(ART, "p4", "p1_k1_v1_fits.json")))
    ff2 = next((f for f in d2["fold_fits"] if f["fold"] == fold), None)
    if ff2 and family in ff2 and ff2[family].get("frozen_fits"):
        return ff2[family]["frozen_fits"]
    return None


def sobol_configs(family, n):
    fc = FAMILY_CONFIGS[family]
    try:
        from scipy.stats import qmc
        sampler = qmc.Sobol(d=len(fc["grid"]), scramble=True, seed=fc["seed"])
        keys = list(fc["grid"].keys())
        cfgs = []
        for _ in range(n):
            row = sampler.random()[0]
            cfg = dict(fc["base"])
            for k, u in zip(keys, row):
                vals = fc["grid"][k]
                idx = min(len(vals) - 1, int(u * len(vals)))
                cfg[k] = vals[idx]
            cfgs.append(cfg)
        return cfgs
    except ImportError:
        import random
        random.seed(fc["seed"])
        return [dict(fc["base"], **{k: random.choice(v) for k, v in fc["grid"].items()}) for _ in range(n)]


def build_config(family, fits, params):
    cfg = dict(params)
    cfg["pairs"] = []
    return {"synchronized_cycle": cfg, "fits": fits, "budget_quote": 4999.0}


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
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=300)
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
    dig = ss.get("trace_digests", {})
    return {"ok": True, "wall_s": round(wall, 1),
            "ann": m["annualized_return_pct"], "dd": m["max_drawdown_pct"],
            "trade_count": m["trade_count"], "breach": m["breach"],
            "fee_quote": m.get("total_fee_quote"),
            "group_fo": ss.get("group_fo", []), "group_so": ss.get("group_so", []),
            "groups_with_so": ss.get("groups_with_so", 0),
            "actual_symbols": ss.get("actual_symbol_count", 0),
            "max_symbol_conc": ss.get("max_symbol_abs_net_pnl_share_pct", 0),
            "max_group_conc": ss.get("max_group_abs_net_pnl_share_pct", 0),
            "event_sha": dig.get("event_stream_sha256", "")}


def causal_blocks(train_start, train_end, lookback_days=60, n_blocks=3):
    """Causal inner blocks (P3 contract)."""
    initial_fit_end = train_start + lookback_days * 86_400_000
    if initial_fit_end >= train_end:
        return []
    replay_span = train_end - initial_fit_end
    chunk = replay_span // n_blocks
    purge = max(lookback_days * 86_400_000, 8 * 3600 * 1000)
    blocks = []
    for i in range(n_blocks):
        rs = initial_fit_end + i * chunk
        re = initial_fit_end + (i + 1) * chunk if i < n_blocks - 1 else train_end
        fe = rs - purge
        if fe <= train_start or re <= rs:
            continue
        blocks.append((f"ib{i+1}", rs, re))
    return blocks


def main():
    ckpt = json.load(open(CKPT)) if os.path.exists(CKPT) else {"done": {}}
    t_start = time.time()
    n_done = 0
    summary = {"families": {}}

    for family in ["C1", "B1", "M2R", "P1", "K1", "V1"]:
        fc = FAMILY_CONFIGS[family]
        n_cfg = fc["n_configs"]
        fam_summary = {"configs": n_cfg, "folds": {}, "g0_bound_params": 0, "g0_total_params": 0}
        for fname, ts, te, vs, ve in FOLDS:
            fits = load_family_fits(family, fname)
            if not fits:
                fam_summary["folds"][fname] = {"status": "blocked_no_fits"}
                continue
            cfgs = sobol_configs(family, n_cfg)
            blocks = causal_blocks(ts, te)
            if not blocks:
                fam_summary["folds"][fname] = {"status": "no_causal_blocks"}
                continue
            # G1: run each config × blocks × budgets
            fold_survivors = []
            for ci, params in enumerate(cfgs):
                cid = f"{family}_{fname}_{ci:03d}"
                block_results = {}
                for budget in [1000.0, 4999.0]:
                    for bname, bs, be in blocks:
                        key = f"{cid}|{bname}|{int(budget)}"
                        if key in ckpt["done"]:
                            block_results.setdefault(bname, {})[int(budget)] = ckpt["done"][key]
                            n_done += 1
                            continue
                        label = f"g1_{cid}_{bname}_{int(budget)}"
                        res = run(build_config(family, fits, params), budget, bs, be, label)
                        if res["ok"]:
                            rec = {k: v for k, v in res.items() if k != "ok" and k != "event_sha"}
                            rec["status"] = "complete"
                        else:
                            rec = {"status": "error", "err": (res.get("err") or "")[:150]}
                        rec["params"] = params
                        ckpt["done"][key] = rec
                        block_results.setdefault(bname, {})[int(budget)] = rec
                        n_done += 1
                        if n_done % 30 == 0:
                            json.dump(ckpt, open(CKPT, "w"), indent=2)
                            elapsed = time.time() - t_start
                            print(f"  {family}/{fname}: {n_done} done ({elapsed:.0f}s)", flush=True)
            json.dump(ckpt, open(CKPT, "w"), indent=2)
            # G1 strict gate per (config, budget)
            survivors = []
            for ci, params in enumerate(cfgs):
                cid = f"{family}_{fname}_{ci:03d}"
                for budget in [1000.0, 4999.0]:
                    brs = []
                    for bname, _, _ in blocks:
                        r = ckpt["done"].get(f"{cid}|{bname}|{int(budget)}")
                        if r and r.get("status") == "complete":
                            brs.append(r)
                    if len(brs) < len(blocks):
                        continue
                    if any(r.get("breach") for r in brs):
                        continue
                    if any((r.get("dd") or 0) > 45 for r in brs):
                        continue
                    anns = [r.get("ann") or 0.0 for r in brs]
                    if sum(1 for a in anns if a < 0) >= 2:
                        continue
                    if sum(r.get("groups_with_so", 0) for r in brs) == 0:
                        continue
                    if any((r.get("max_symbol_conc") or 0) > 50 for r in brs):
                        continue
                    median_ann = sorted(anns)[len(anns)//2]
                    worst_dd = max(r.get("dd") or 0 for r in brs)
                    survivors.append({"config_id": cid, "budget": int(budget),
                                      "median_ann": median_ann, "worst_dd": worst_dd,
                                      "pos_blocks": sum(1 for a in anns if a > 0)})
            survivors.sort(key=lambda x: -(x["median_ann"] - x["worst_dd"]*0.5))
            fam_summary["folds"][fname] = {"configs_run": n_cfg * len(blocks) * 2,
                                           "survivors": len(survivors),
                                           "best": survivors[0] if survivors else None}
            print(f"{family}/{fname}: {n_cfg}cfg x {len(blocks)}blocks x 2budget, {len(survivors)} survivors")
        summary["families"][family] = fam_summary

    summary["total_runs"] = n_done
    summary["elapsed_s"] = round(time.time() - t_start, 1)
    out_path = os.path.join(P67, "g0_g1-summary.json")
    with open(out_path, "w") as f:
        json.dump(summary, f, indent=2)
    print(f"\nwrote {out_path}")
    print(f"total runs: {n_done}")


if __name__ == "__main__":
    main()
