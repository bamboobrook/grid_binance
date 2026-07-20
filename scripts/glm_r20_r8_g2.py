#!/usr/bin/env python3
"""Round 20 P8: G2 full-train + subblocks + budget select.

Takes G1 survivors per (family, fold) and runs:
  full train + 5 non-overlapping train-only subblocks × budgets 1000/2000/3000/4000/4999U.
G2 strict gate: median subblock ann>=30% (or raw>0 if <180d), worst DD<=35%,
>=4/5 positive, no breach, actual symbols>=5, conc<=50%, real SO in >=4/5.
Per family per fold: max 4 config+budget finalists.
"""
import json
import os
import subprocess
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round20"
P8 = os.path.join(ART, "p8")
CONFIGS = os.path.join(P8, "configs")
CKPT = os.path.join(P8, "g2-checkpoint.json")
os.makedirs(CONFIGS, exist_ok=True)

G1_CKPT = os.path.join(ART, "p6_p7", "g1-checkpoint.json")

FOLDS = [
    ("F1", 1672531200000, 1688169599999, 1688774400000, 1704067199999),
    ("F2", 1672531200000, 1704067199999, 1704662400000, 1735689599999),
    ("F3", 1672531200000, 1735689599999, 1736294400000, 1767225599999),
    ("F4", 1672531200000, 1767225599999, 1767820800000, 1780271999999),
]
BUDGETS = [1000.0, 2000.0, 3000.0, 4000.0, 4999.0]
FAMILIES = ["P1", "C1", "M2R", "V1"]  # only families with G1 survivors


def subblocks(train_start, train_end, n=5):
    span = train_end - train_start
    chunk = span // n
    return [(f"sb{i+1}", train_start + i*chunk, train_start + (i+1)*chunk if i < n-1 else train_end) for i in range(n)]


def get_g1_survivors(family, fold):
    """Pull G1 survivors for (family, fold) from G1 checkpoint, re-applying the strict gate."""
    g1 = json.load(open(G1_CKPT))
    prefix = f"{family}_{fold}_"
    groups = {}
    for key, rec in g1["done"].items():
        if not key.startswith(prefix):
            continue
        if rec.get("status") != "complete":
            continue
        cid = rec.get("config_id") or key.split("|")[0]
        # reconstruct config_id from key: family_fold_NNN|block|budget
        parts = key.split("|")
        cid_full = parts[0]
        block = parts[1] if len(parts) > 1 else ""
        budget = int(parts[2]) if len(parts) > 2 else 0
        groups.setdefault((cid_full, budget), {})[block] = rec
    survivors = []
    for (cid, bud), blocks in groups.items():
        brs = [blocks.get(bn) for bn in ("ib1", "ib2", "ib3")]
        brs = [r for r in brs if r]
        if len(brs) < 2:
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
        median_ann = sorted(anns)[len(anns)//2]
        worst_dd = max(r.get("dd") or 0 for r in brs)
        survivors.append({"config_id": cid, "budget": bud, "params": brs[0].get("params", {}),
                          "median_ann": median_ann, "worst_dd": worst_dd,
                          "pos_blocks": sum(1 for a in anns if a > 0)})
    survivors.sort(key=lambda x: -(x["median_ann"] - x["worst_dd"]*0.5))
    # dedupe by config_id, take top 8
    seen = set(); top = []
    for s in survivors:
        if s["config_id"] in seen:
            continue
        seen.add(s["config_id"])
        top.append(s)
        if len(top) >= 8:
            break
    return top


def build_config(params, fits):
    cfg = dict(params)
    cfg["pairs"] = []
    return {"synchronized_cycle": cfg, "fits": fits}


def load_fits(family, fold):
    d = json.load(open(os.path.join(ART, "p4", "families-fit.json")))
    ff = next((f for f in d["fold_fits"] if f["fold"] == fold), None)
    if ff and family in ff and ff[family].get("frozen_fits"):
        return ff[family]["frozen_fits"]
    d2 = json.load(open(os.path.join(ART, "p4", "p1_k1_v1_fits.json")))
    ff2 = next((f for f in d2["fold_fits"] if f["fold"] == fold), None)
    if ff2 and family in ff2 and ff2[family].get("frozen_fits"):
        return ff2[family]["frozen_fits"]
    return None


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


def main():
    ckpt = json.load(open(CKPT)) if os.path.exists(CKPT) else {"done": {}}
    t_start = time.time()
    n_done = 0
    summary = {"families": {}}

    for family in FAMILIES:
        for fname, ts, te, vs, ve in FOLDS:
            fits = load_fits(family, fname)
            if not fits:
                continue
            survivors = get_g1_survivors(family, fname)
            if not survivors:
                continue
            sbs = subblocks(ts, te)
            windows = [("full", ts, te)] + sbs
            finalists = []
            for s in survivors[:8]:  # top 8 from G1
                for budget in BUDGETS:
                    sub_results = []
                    for sbname, sbs_t, sbe_t in sbs:
                        key = f"{s['config_id']}|{sbname}|{int(budget)}"
                        if key in ckpt["done"]:
                            sub_results.append(ckpt["done"][key])
                            n_done += 1
                            continue
                        label = f"g2_{s['config_id']}_{sbname}_{int(budget)}"
                        res = run(build_config(s["params"], fits), budget, sbs_t, sbe_t, label)
                        if res["ok"]:
                            rec = {k: v for k, v in res.items() if k != "ok"}
                            rec["status"] = "complete"
                        else:
                            rec = {"status": "error", "err": (res.get("err") or "")[:150]}
                        ckpt["done"][key] = rec
                        sub_results.append(rec)
                        n_done += 1
                        if n_done % 30 == 0:
                            json.dump(ckpt, open(CKPT, "w"), indent=2)
                            print(f"  {family}/{fname}: {n_done} done ({time.time()-t_start:.0f}s)", flush=True)
                    # G2 gate
                    complete = [r for r in sub_results if r.get("status") == "complete"]
                    if len(complete) < 5:
                        continue
                    anns = [r.get("ann") or 0.0 for r in complete]
                    median_ann = sorted(anns)[2]
                    worst_dd = max(r.get("dd") or 0 for r in complete)
                    pos = sum(1 for a in anns if a > 0)
                    breach = any(r.get("breach") for r in complete)
                    gws = sum(r.get("groups_with_so", 0) for r in complete)
                    if median_ann < 30 and median_ann <= 0:
                        continue
                    if worst_dd > 35:
                        continue
                    if pos < 4:
                        continue
                    if breach:
                        continue
                    if gws == 0:
                        continue
                    finalists.append({"config_id": s["config_id"], "budget": int(budget),
                                      "params": s["params"], "median_ann": median_ann,
                                      "worst_dd": worst_dd, "pos_blocks": pos})
            finalists.sort(key=lambda x: -(x["median_ann"] - x["worst_dd"]*0.5))
            selected = finalists[:4]
            summary["families"].setdefault(family, {})[fname] = {
                "g1_survivors_in": len(survivors), "g2_finalists": len(finalists),
                "selected": len(selected), "selected_configs": selected}
            print(f"{family}/{fname}: g1_in={len(survivors)} finalists={len(finalists)} selected={len(selected)}")
            if selected:
                b = selected[0]
                print(f"  best: {b['config_id']} b={b['budget']} med={b['median_ann']:.1f}% wdd={b['worst_dd']:.1f}% pos={b['pos_blocks']}/5")
            json.dump(ckpt, open(CKPT, "w"), indent=2)

    summary["total_runs"] = n_done
    summary["elapsed_s"] = round(time.time() - t_start, 1)
    out_path = os.path.join(P8, "g2-summary.json")
    with open(out_path, "w") as f:
        json.dump(summary, f, indent=2)
    print(f"\nwrote {out_path}; total runs: {n_done}")


if __name__ == "__main__":
    main()
