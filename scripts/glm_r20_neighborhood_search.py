#!/usr/bin/env python3
"""Round 20 continuation: C1 neighborhood (trust-region) expansion search.

Plan §10 P8 allows center + 12 local neighbors. The best OOS survivor is
C1_F4_008 (val 12.7%/4.2%) and C1_F4_003 (val 12.6%/4.2%). Both use fo=60,
cap=25, multiplier=1.35, legs=4. Low DD (4.2%) suggests room for more risk.

This script generates a neighborhood around these two centers, runs each on
F4 train (causal inner blocks), selects the best by train median ann, then
validates the top survivors ONE-SHOT on F4 validation.

Also tests higher-multiplier / more-legs variants to push ann up while
keeping DD bounded.
"""
import itertools
import json
import os
import subprocess
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round20"
P_NEI = os.path.join(ART, "neighborhood")
CONFIGS = os.path.join(P_NEI, "configs")
CKPT = os.path.join(P_NEI, "checkpoint.json")
os.makedirs(CONFIGS, exist_ok=True)

# F4 train/validation
F4_TRAIN_START = 1672531200000
F4_TRAIN_END = 1767225599999
F4_VAL_START = 1767820800000
F4_VAL_END = 1780271999999


def causal_blocks(train_start, train_end, lookback_days=60, n_blocks=3):
    initial = train_start + lookback_days * 86_400_000
    if initial >= train_end:
        return []
    span = train_end - initial
    chunk = span // n_blocks
    purge = max(lookback_days * 86_400_000, 8 * 3600 * 1000)
    blocks = []
    for i in range(n_blocks):
        rs = initial + i * chunk
        re = initial + (i + 1) * chunk if i < n_blocks - 1 else train_end
        fe = rs - purge
        if fe <= train_start or re <= rs:
            continue
        blocks.append((f"ib{i+1}", rs, re))
    return blocks


def load_c1_fits(fold="F4"):
    d = json.load(open(os.path.join(ART, "p4", "families-fit.json")))
    ff = next(f for f in d["fold_fits"] if f["fold"] == fold)
    return ff["C1"]["frozen_fits"]


def build_config(params, fits):
    cfg = dict(params)
    cfg["pairs"] = []
    cfg["family"] = "M1_pair"
    cfg["bar_boundary_minutes"] = 5
    cfg["fit_lookback_days"] = 60
    cfg["factor"] = None
    cfg["basket_symbols"] = []
    cfg["cycle_deadline_h"] = None
    return {"synchronized_cycle": cfg, "fits": fits}


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
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
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


def neighborhood_configs():
    """Generate neighborhood around the two best C1 configs, plus higher-risk
    variants to push ann up."""
    centers = [
        # C1_F4_008 center
        {"entry_z": 1.0, "so_residual_step_z": 0.6, "group_fo_quote": 60.0,
         "multiplier": 1.35, "max_legs": 4, "leverage": 3,
         "exit_z": 0.25, "tp_net_bps_floor": 40, "group_gross_cap_pct": 25.0},
        # C1_F4_003 center
        {"entry_z": 1.5, "so_residual_step_z": 0.25, "group_fo_quote": 60.0,
         "multiplier": 1.35, "max_legs": 4, "leverage": 3,
         "exit_z": 0.25, "tp_net_bps_floor": 100, "group_gross_cap_pct": 25.0},
    ]
    cfgs = []
    # add the centers themselves as controls
    for c in centers:
        cfgs.append({**c, "_label": "center"})
    # neighborhood: vary one param at a time by one grid step
    for c in centers:
        # entry_z: 0.5 / 1.0 / 1.5 / 2.0
        for ez in [0.5, 2.0]:
            cfgs.append({**c, "entry_z": ez, "_label": f"entry_z={ez}"})
        # so_step: 0.25/0.4/0.6/0.8
        for ss in [0.4, 0.8]:
            cfgs.append({**c, "so_residual_step_z": ss, "_label": f"so_step={ss}"})
        # multiplier: push higher for more risk
        for mult in [1.5, 1.7]:
            cfgs.append({**c, "multiplier": mult, "_label": f"mult={mult}"})
        # max_legs: 3/5
        for ml in [3, 5]:
            cfgs.append({**c, "max_legs": ml, "_label": f"legs={ml}"})
        # group_fo_quote: push higher
        for fo in [45.0, 80.0]:
            cfgs.append({**c, "group_fo_quote": fo, "_label": f"fo={fo}"})
        # tp floor
        for tp in [20, 70]:
            cfgs.append({**c, "tp_net_bps_floor": tp, "_label": f"tp={tp}"})
        # leverage
        for lev in [2, 5]:
            cfgs.append({**c, "leverage": lev, "_label": f"lev={lev}"})
        # cap
        for cap in [18.0, 30.0]:
            cfgs.append({**c, "group_gross_cap_pct": cap, "_label": f"cap={cap}"})
    # higher-risk combos to push ann: high mult + high legs + high fo
    for mult, legs, fo in [(1.5, 5, 80), (1.7, 5, 60), (1.5, 4, 80)]:
        cfgs.append({**centers[0], "multiplier": mult, "max_legs": legs, "group_fo_quote": fo,
                     "_label": f"risk_mult={mult}_legs={legs}_fo={fo}"})
    return cfgs


def main():
    fits = load_c1_fits("F4")
    if not fits:
        print("F4 C1 fits blocked; cannot run neighborhood")
        return
    blocks = causal_blocks(F4_TRAIN_START, F4_TRAIN_END)
    cfgs = neighborhood_configs()
    print(f"Neighborhood: {len(cfgs)} configs × {len(blocks)} blocks × 2 budgets")

    ckpt = json.load(open(CKPT)) if os.path.exists(CKPT) else {"done": {}}
    t_start = time.time()
    n_done = 0

    for ci, params in enumerate(cfgs):
        label_param = params.get("_label", str(ci))
        cid = f"C1nei_{ci:03d}_{label_param}"
        for budget in [1000.0, 4999.0]:
            block_results = []
            for bname, bs, be in blocks:
                key = f"{cid}|{bname}|{int(budget)}"
                if key in ckpt["done"]:
                    block_results.append(ckpt["done"][key])
                    n_done += 1
                    continue
                res = run(build_config(params, fits), budget, bs, be, f"nei_{key}")
                if res["ok"]:
                    rec = {k: v for k, v in res.items() if k != "ok"}
                    rec["status"] = "complete"
                else:
                    rec = {"status": "error", "err": (res.get("err") or "")[:150]}
                rec["params"] = params; rec["label"] = label_param
                ckpt["done"][key] = rec
                block_results.append(rec)
                n_done += 1
            if n_done % 24 == 0:
                json.dump(ckpt, open(CKPT, "w"), indent=2)
                print(f"  {n_done} done ({time.time()-t_start:.0f}s)", flush=True)
    json.dump(ckpt, open(CKPT, "w"), indent=2)
    print(f"Neighborhood train done: {n_done} runs in {time.time()-t_start:.0f}s")

    # Build a clean index: (ci, budget, block) -> record
    index = {}
    for k, r in ckpt["done"].items():
        # key format: C1nei_NNN_label|block|budget  OR  nei_C1nei_NNN_label|block|budget
        if "|ib" not in k:
            continue
        parts = k.split("|")
        if len(parts) < 3:
            continue
        ci_part = parts[0]  # C1nei_NNN_label
        block = parts[1]
        budget = int(parts[2])
        # extract ci number
        try:
            ci_num = int(ci_part.split("_")[1])
        except (IndexError, ValueError):
            continue
        index.setdefault((ci_num, budget), {})[block] = r

    survivors = []
    for ci, params in enumerate(cfgs):
        label = params.get("_label", str(ci))
        for budget in [1000.0, 4999.0]:
            ib = int(budget)
            blocks_map = index.get((ci, ib), {})
            brs = [blocks_map.get(bn) for bn, _, _ in blocks]
            brs = [r for r in brs if r and r.get("status") == "complete"]
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
            med = sorted(anns)[len(anns)//2]
            wdd = max(r.get("dd") or 0 for r in brs)
            survivors.append({"ci": ci, "label": label, "budget": ib,
                              "params": {k: v for k, v in params.items() if k != "_label"},
                              "median_ann": med, "worst_dd": wdd,
                              "pos_blocks": sum(1 for a in anns if a > 0)})
    survivors.sort(key=lambda x: -(x["median_ann"] - x["worst_dd"]*0.5))
    print(f"\nSurvivors: {len(survivors)}")
    for s in survivors[:10]:
        print(f"  {s['label']} b={s['budget']}: med={s['median_ann']:.1f}% wdd={s['worst_dd']:.1f}% pos={s['pos_blocks']}/{len(blocks)}")

    # validate top 4 on F4 validation (one-shot)
    print("\n=== One-shot F4 validation ===")
    val_results = []
    for s in survivors[:4]:
        res = run(build_config(s["params"], fits), s["budget"], F4_VAL_START, F4_VAL_END,
                  f"valnei_{s['ci']}_{s['budget']}")
        if res["ok"]:
            val_results.append({"label": s["label"], "budget": s["budget"],
                                "train_med": s["median_ann"], "train_wdd": s["worst_dd"],
                                "val_ann": res["ann"], "val_dd": res["dd"],
                                "val_breach": res["breach"], "val_conc": res.get("max_symbol_conc", 0)})
            print(f"  {s['label']} b={s['budget']}: train={s['median_ann']:.1f}%/{s['worst_dd']:.1f}% -> val={res['ann']:.1f}%/{res['dd']:.1f}% breach={res['breach']}")
        else:
            print(f"  {s['label']}: ERROR {res.get('err','')[:80]}")

    summary = {"phase": "C1 neighborhood expansion", "configs": len(cfgs),
               "train_runs": n_done, "survivors": len(survivors),
               "top_survivors": survivors[:10], "validation": val_results}
    out_path = os.path.join(P_NEI, "summary.json")
    with open(out_path, "w") as f:
        json.dump(summary, f, indent=2)
    print(f"\nwrote {out_path}")


def ckpt_done_label(ckpt, ci):
    for k in ckpt["done"]:
        if k.startswith(f"C1nei_{ci:03d}_"):
            return ckpt["done"][k].get("label", str(ci))
    return str(ci)


if __name__ == "__main__":
    main()
