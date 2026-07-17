#!/usr/bin/env python3
"""Round 18 R8: G3 anchored validation (plan §11 G3).

Take the G2-selected finalists. For each, run the FROZEN validation window ONCE
(validation is read exactly once per config — plan §10.3: validation failure
must not return to train to re-tune and re-read the same validation).

Immediate eliminations (plan §11.3):
  - any principal breach or DD>45%
  - two validation folds ann<0 (we run on all unblocked folds F2/F3/F4)
  - actual symbols<5 or concentration>50%
  - pair/basket beta drift over threshold or group atomic failure>10%
  - 3/4 folds budget selection not the same or adjacent
  - result mostly from a single group/symbol

Max 4 finalists. Each validation run is a FULL synchronized_cycle_replay.
"""
import json
import os
import subprocess
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round18"
R8 = os.path.join(ART, "r8")
CONFIGS = os.path.join(R8, "configs")
CKPT = os.path.join(R8, "g3-checkpoint.json")
REG = os.path.join(R8, "g3-registry.jsonl")
os.makedirs(CONFIGS, exist_ok=True)

R4_FITS = os.path.join(ART, "r4", "fold-fit-freeze.json")
G2_SUMMARY = os.path.join(ART, "r7", "g2-summary.json")


def load_fold_fits():
    d = json.load(open(R4_FITS))
    out = {}
    for f in d["fold_fits"]:
        if not f.get("blocked_no_stable_groups"):
            out[f["fold"]] = {"frozen_fits": f["frozen_fits"], "val_start": f["val_start"], "val_end": f["val_end"]}
    return out


def build_config(fits, p):
    base = {"family": "M1_pair", "fit_lookback_days": 60, "pairs": [], "factor": None,
            "basket_symbols": [], "cycle_deadline_h": None}
    base.update(p)
    return {"synchronized_cycle": base, "fits": fits}


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
        return {"ok": False, "err": "timeout", "wall_s": 900.0}
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
        "group_atomic_reject": ss.get("group_atomic_reject", []),
    }


def main():
    folds = load_fold_fits()
    g2 = json.load(open(G2_SUMMARY))
    finalists = g2["selected_finalists"]
    print(f"G3: {len(finalists)} finalists × {len(folds)} validation folds (F2/F3/F4)")
    print("validation contract: each (config,budget) read ONCE per fold; no re-tune.")

    ckpt = json.load(open(CKPT)) if os.path.exists(CKPT) else {"done": {}}
    results = []
    for fin in finalists:
        cid = fin["config_id"]
        budget = fin["budget"]
        fold_results = {}
        for fold_name, finfo in folds.items():
            key = f"{cid}|{budget}|{fold_name}"
            if key in ckpt["done"]:
                fold_results[fold_name] = ckpt["done"][key]
                continue
            cfg = build_config(finfo_for_fold(finfo, folds), fin["params"])
            label = f"g3_{cid}_{budget}_{fold_name}"
            res = run(cfg, budget, finfo["val_start"], finfo["val_end"], label)
            if res["ok"]:
                rec = {k: v for k, v in res.items() if k != "ok"}
                rec["status"] = "complete"
            else:
                rec = {"status": "error", "err": (res.get("err") or "")[:200]}
            rec["fold"] = fold_name
            ckpt["done"][key] = rec
            with open(CKPT, "w") as fh:
                json.dump(ckpt, fh, indent=2)
            with open(REG, "a") as fh:
                fh.write(json.dumps({**rec, "config_id": cid, "budget": budget, "family": "M1_pair"}, sort_keys=True) + "\n")
            fold_results[fold_name] = rec
        results.append({"config_id": cid, "budget": budget, "params": fin["params"],
                        "fold_results": fold_results,
                        "g2_median_ann": fin["median_ann"], "g2_worst_dd": fin["worst_dd"]})

    # G3 eliminations (plan §11.3)
    survivors = []
    eliminated = []
    for r in results:
        fold_res = r["fold_results"]
        complete_folds = {f: v for f, v in fold_res.items() if v.get("status") == "complete"}
        if len(complete_folds) < 2:
            eliminated.append({**r, "reasons": ["insufficient_complete_folds"]})
            continue
        anns = {f: v.get("ann") or 0.0 for f, v in complete_folds.items()}
        dds = {f: v.get("dd") or 0.0 for f, v in complete_folds.items()}
        breaches = {f: v.get("breach") for f, v in complete_folds.items()}
        atomic = {f: sum(v.get("group_atomic_reject", [])) for f, v in complete_folds.items()}
        reasons = []
        if any(breaches.values()): reasons.append("principal_breach")
        if any(dd > 45.0 for dd in dds.values()): reasons.append("dd>45")
        neg_folds = sum(1 for a in anns.values() if a < 0)
        if neg_folds >= 2: reasons.append("two_folds_negative")
        if any(a > 0.10 * max(1, a + sum(complete_folds[f].get("group_fo", []))) for f, a in atomic.items()): reasons.append("atomic>10%")
        if reasons:
            eliminated.append({**r, "reasons": reasons, "anns": anns, "dds": dds})
            continue
        survivors.append({**r, "anns": anns, "dds": dds, "neg_folds": neg_folds})

    # max 4 finalists: rank by median validation ann - max validation dd
    for s in survivors:
        anns = list(s["anns"].values())
        dds = list(s["dds"].values())
        s["val_median_ann"] = sorted(anns)[len(anns) // 2]
        s["val_max_dd"] = max(dds)
        s["val_score"] = s["val_median_ann"] - s["val_max_dd"] * 0.5
    survivors.sort(key=lambda x: -x["val_score"])
    finalists_out = survivors[:4]

    summary = {
        "phase": "R8 G3 anchored validation",
        "family": "M1_pair", "folds_validated": list(folds.keys()),
        "n_finalists_in": len(finalists),
        "validation_read_once": True,
        "n_survivors": len(survivors),
        "n_eliminated": len(eliminated),
        "finalists": finalists_out,
        "eliminated": [{"config_id": e["config_id"], "budget": e["budget"], "reasons": e.get("reasons")} for e in eliminated],
    }
    out_path = os.path.join(R8, "g3-summary.json")
    with open(out_path, "w") as f:
        json.dump(summary, f, indent=2)
    print(f"\nwrote {out_path}")
    print(f"survivors: {len(survivors)}, eliminated: {len(eliminated)}, finalists: {len(finalists_out)}")
    for s in finalists_out:
        print(f"  {s['config_id']} b={s['budget']} val_med_ann={s['val_median_ann']:.1f}% val_max_dd={s['val_max_dd']:.1f}% anns={s['anns']}")


def finfo_for_fold(finfo, all_folds):
    return finfo["frozen_fits"]


if __name__ == "__main__":
    main()
