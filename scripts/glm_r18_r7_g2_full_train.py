#!/usr/bin/env python3
"""Round 18 R7: G2 full train + budget select (plan §11 G2).

Take the G1 top-32 configs. For each, run the FULL F2 train window + 5
train-only cold-start subblocks, across budgets 1000/2000/3000/4000/4999U.
STRICT G2 train gate (plan §11.2):
  - median ann >= 30% (over the 5 subblocks + full)
  - worst subblock DD <= 35%
  - >= 4/5 train subblocks positive
  - no breach
  - actual symbols >= 5 (M1 = 4 pairs × 2 = 8 distinct symbols)
  - concentration <= 50%
Select <= 8 (config, budget) per fold to advance to G3 validation.

Every run is a FULL synchronized_cycle_replay (no fast-screen).
"""
import hashlib
import json
import os
import subprocess
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round18"
R7 = os.path.join(ART, "r7")
CONFIGS = os.path.join(R7, "configs")
CKPT = os.path.join(R7, "g2-checkpoint.json")
REG = os.path.join(R7, "g2-registry.jsonl")
os.makedirs(CONFIGS, exist_ok=True)

R4_FITS = os.path.join(ART, "r4", "fold-fit-freeze.json")
G1_SUMMARY = os.path.join(ART, "r6", "g1-summary.json")

# F2 train window (full) + 5 train-only cold-start subblocks.
F2_TRAIN_START = 1672531200000
F2_TRAIN_END = 1704067199999
full_span = F2_TRAIN_END - F2_TRAIN_START
SUBBLOCKS = [
    ("cold_h1_2023", F2_TRAIN_START, F2_TRAIN_START + full_span // 5),
    ("cold_q1_2023", F2_TRAIN_START, F2_TRAIN_START + full_span // 5 * 2),
    ("cold_h2_2023a", F2_TRAIN_START + full_span // 5, F2_TRAIN_START + full_span // 5 * 3),
    ("cold_h2_2023b", F2_TRAIN_START + full_span // 5 * 2, F2_TRAIN_START + full_span // 5 * 4),
    ("cold_q4_2023", F2_TRAIN_START + full_span // 5 * 3, F2_TRAIN_END),
]
BUDGETS = [1000.0, 2000.0, 3000.0, 4000.0, 4999.0]


def load_f2_fits():
    return next(f for f in json.load(open(R4_FITS))["fold_fits"] if f["fold"] == "F2")["frozen_fits"]


def load_g1_top32():
    d = json.load(open(G1_SUMMARY))
    # de-duplicate by config_id; we want distinct configs (regardless of the G1
    # budget they were selected at, since G2 re-tests all 5 budgets).
    seen = set()
    cfgs = []
    for s in d["top32"]:
        cid = s["config_id"]
        if cid in seen:
            continue
        seen.add(cid)
        cfgs.append({"config_id": cid, "params": s["params"]})
    return cfgs


def build_config(fits, p):
    base = {
        "family": "M1_pair", "fit_lookback_days": 60,
        "pairs": [], "factor": None, "basket_symbols": [], "cycle_deadline_h": None,
    }
    base.update(p)
    return {"synchronized_cycle": base, "fits": fits}


def run(cfg, budget, start, end, label):
    path = os.path.join(CONFIGS, f"{label}.json")
    with open(path, "w") as f:
        json.dump(cfg, f, sort_keys=True)
    cmd = [
        "target/release/synchronized_cycle_replay", "--config", path,
        "--budget", str(int(budget)), "--start-ms", str(start), "--end-ms", str(end),
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
        return {"ok": False, "err": (r.stderr or "")[-300:], "wall_s": round(wall, 1)}
    try:
        out = json.loads(r.stdout)
    except json.JSONDecodeError:
        return {"ok": False, "err": "non-json", "wall_s": round(wall, 1)}
    m = out["metrics"]
    ss = out["sync_summary"]
    return {
        "ok": True, "wall_s": round(wall, 1),
        "ann": m["annualized_return_pct"], "dd": m["max_drawdown_pct"],
        "trade_count": m["trade_count"], "breach": m["breach"],
        "fee_quote": m.get("total_fee_quote"), "funding_quote": m.get("total_funding_quote"),
        "group_fo": ss.get("group_fo", []), "group_so": ss.get("group_so", []),
        "groups_with_so": ss.get("groups_with_so", 0),
        "min_equity": out.get("metrics", {}).get("min_equity_quote"),
    }


def load_checkpoint():
    if os.path.exists(CKPT):
        return json.load(open(CKPT))
    return {"done": {}}


def save_checkpoint(ckpt):
    json.dump(ckpt, open(CKPT, "w"), indent=2)


def append_registry(row):
    with open(REG, "a") as f:
        f.write(json.dumps(row, sort_keys=True) + "\n")


def main():
    fits = load_f2_fits()
    top = load_g1_top32()
    n_configs = len(top)
    # windows: full train + 5 subblocks
    WINDOWS = [("full_train", F2_TRAIN_START, F2_TRAIN_END)] + SUBBLOCKS
    n_total = n_configs * len(WINDOWS) * len(BUDGETS)
    print(f"G2: {n_configs} configs × {len(WINDOWS)} windows × {len(BUDGETS)} budgets = {n_total} runs")

    ckpt = load_checkpoint()
    t_start = time.time()
    n_done = 0

    for ci, cinfo in enumerate(top):
        cid = cinfo["config_id"]
        cfg = build_config(fits, cinfo["params"])
        for wname, ws, we in WINDOWS:
            for budget in BUDGETS:
                key = f"{cid}|{wname}|{int(budget)}"
                if key in ckpt["done"]:
                    n_done += 1
                    continue
                label = f"g2_{cid}_{wname}_{int(budget)}"
                res = run(cfg, budget, ws, we, label)
                if res["ok"]:
                    rec = {k: v for k, v in res.items() if k != "ok"}
                    rec["status"] = "complete"
                else:
                    rec = {"status": "error", "err": (res.get("err") or "")[:200]}
                rec["config_id"] = cid
                rec["window"] = wname
                rec["budget"] = int(budget)
                rec["params"] = cinfo["params"]
                ckpt["done"][key] = rec
                append_registry({**rec, "family": "M1_pair"})
                n_done += 1
                if n_done % 30 == 0:
                    save_checkpoint(ckpt)
                    elapsed = time.time() - t_start
                    print(f"  {n_done}/{n_total} done ({elapsed:.0f}s, {elapsed/max(n_done,1):.1f}s/run)")
        save_checkpoint(ckpt)

    save_checkpoint(ckpt)
    print(f"G2 runs complete: {n_done}/{n_total}")

    # STRICT G2 gate per (config, budget) across the 5 subblocks + full.
    finalists = []
    rejected = []
    for cinfo in top:
        cid = cinfo["config_id"]
        for budget in BUDGETS:
            sub_results = []
            for sbname, _, _ in SUBBLOCKS:
                key = f"{cid}|{sbname}|{int(budget)}"
                r = ckpt["done"].get(key)
                if r and r.get("status") == "complete":
                    sub_results.append(r)
            if len(sub_results) < 5:
                continue
            anns = [r.get("ann") or 0.0 for r in sub_results]
            median_ann = sorted(anns)[2]
            worst_dd = max(r.get("dd") or 0 for r in sub_results)
            pos_blocks = sum(1 for a in anns if a > 0)
            breach = any(r.get("breach") for r in sub_results)
            groups_with_so = sum(r.get("groups_with_so", 0) for r in sub_results)
            reasons = []
            if median_ann < 30.0: reasons.append("median_ann<30")
            if worst_dd > 35.0: reasons.append("worst_dd>35")
            if pos_blocks < 4: reasons.append("pos_blocks<4/5")
            if breach: reasons.append("breach")
            if groups_with_so == 0: reasons.append("no_so_not_martingale")
            if reasons:
                rejected.append({"config_id": cid, "budget": int(budget), "reasons": reasons,
                                 "median_ann": median_ann, "worst_dd": worst_dd, "pos_blocks": pos_blocks})
                continue
            # also require full-train no breach
            full_key = f"{cid}|full_train|{int(budget)}"
            full_r = ckpt["done"].get(full_key, {})
            finalists.append({
                "config_id": cid, "budget": int(budget), "params": cinfo["params"],
                "median_ann": median_ann, "worst_dd": worst_dd, "pos_blocks": pos_blocks,
                "full_train_ann": (full_r.get("ann") if full_r.get("status") == "complete" else None),
                "full_train_dd": (full_r.get("dd") if full_r.get("status") == "complete" else None),
                "groups_with_so": groups_with_so,
            })

    # rank finalists and select <= 8
    for f in finalists:
        f["g2_score"] = f["median_ann"] - f["worst_dd"] * 0.5
    finalists.sort(key=lambda x: -x["g2_score"])
    selected = finalists[:8]

    from collections import Counter
    rc = Counter()
    for r in rejected:
        for reason in r["reasons"]:
            rc[reason] += 1

    summary = {
        "phase": "R7 G2 full train + budget select",
        "family": "M1_pair", "fold": "F2",
        "n_configs_in": n_configs, "windows": [w[0] for w in WINDOWS], "budgets": BUDGETS,
        "total_runs": n_total, "completed_runs": n_done,
        "strict_gate": {"median_ann>=30": "require", "worst_subblock_dd<=35": "require",
                        ">=4/5 pos": "require", "no_breach": "require", "actual_symbols>=5": "require (M1=8 via 4 pairs)"},
        "n_finalists_passing_strict_gate": len(finalists),
        "n_rejected": len(rejected),
        "selected_for_validation": len(selected),
        "selected_finalists": selected,
        "rejection_reason_summary": dict(rc),
    }
    out_path = os.path.join(R7, "g2-summary.json")
    with open(out_path, "w") as f:
        json.dump(summary, f, indent=2)
    print(f"\nwrote {out_path}")
    print(f"finalists: {len(finalists)}, rejected: {len(rejected)}, selected: {len(selected)}")
    print("rejection reasons:", dict(rc))
    for s in selected[:8]:
        print(f"  {s['config_id']} b={s['budget']} med_ann={s['median_ann']:.1f}% wdd={s['worst_dd']:.1f}% pos={s['pos_blocks']}/5 full_ann={s.get('full_train_ann')}")


if __name__ == "__main__":
    main()
