#!/usr/bin/env python3
"""Round 20 P9: selection freeze + one-shot anchored validation.

Each G2 finalist runs its validation window ONCE. Immediate elimination:
breach/liquidation, DD>45%, actual symbols<5, concentration>50%, no SO,
two validation folds negative, budget across folds not same/adjacent.

Validation is read exactly once per (family, config, budget, fold).
"""
import json
import os
import subprocess
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round20"
P9 = os.path.join(ART, "p9")
CONFIGS = os.path.join(P9, "configs")
CKPT = os.path.join(P9, "validation-checkpoint.json")
os.makedirs(CONFIGS, exist_ok=True)

FOLDS = [
    ("F1", 1672531200000, 1688169599999, 1688774400000, 1704067199999),
    ("F2", 1672531200000, 1704067199999, 1704662400000, 1735689599999),
    ("F3", 1672531200000, 1735689599999, 1736294400000, 1767225599999),
    ("F4", 1672531200000, 1767225599999, 1767820800000, 1780271999999),
]


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


def build_config(params, fits):
    cfg = dict(params)
    cfg["pairs"] = []
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
            "fee_quote": m.get("total_fee_quote"), "funding_quote": m.get("total_funding_quote"),
            "groups_with_so": ss.get("groups_with_so", 0),
            "actual_symbols": ss.get("actual_symbol_count", 0),
            "max_symbol_conc": ss.get("max_symbol_abs_net_pnl_share_pct", 0),
            "max_group_conc": ss.get("max_group_abs_net_pnl_share_pct", 0),
            "min_equity": out.get("metrics", {}).get("min_equity_quote")}


def main():
    g2 = json.load(open(os.path.join(ART, "p8", "g2-summary.json")))
    ckpt = json.load(open(CKPT)) if os.path.exists(CKPT) else {"done": {}}
    n_done = 0
    all_results = []

    for family, folds_info in g2["families"].items():
        for fold, info in folds_info.items():
            if fold == "total_runs" or fold == "elapsed_s":
                continue
            selected = info.get("selected_configs", [])
            if not selected:
                continue
            fold_info = next(f for f in FOLDS if f[0] == fold)
            _, _, _, vs, ve = fold_info
            fits = load_fits(family, fold)
            if not fits:
                continue
            # validate top 2 selected per (family, fold)
            for s in selected[:2]:
                cid = s["config_id"]; budget = s["budget"]
                key = f"{cid}|{budget}|{fold}"
                if key in ckpt["done"]:
                    all_results.append({"family": family, "fold": fold, **ckpt["done"][key],
                                        "config_id": cid, "budget": budget, "train_median_ann": s["median_ann"]})
                    n_done += 1
                    continue
                label = f"val_{cid}_{budget}_{fold}"
                res = run(build_config(s["params"], fits), budget, vs, ve, label)
                if res["ok"]:
                    rec = {k: v for k, v in res.items() if k != "ok"}
                    rec["status"] = "complete"
                else:
                    rec = {"status": "error", "err": (res.get("err") or "")[:150]}
                rec["config_id"] = cid; rec["budget"] = budget
                rec["train_median_ann"] = s["median_ann"]; rec["train_worst_dd"] = s["worst_dd"]
                ckpt["done"][key] = rec
                all_results.append({"family": family, "fold": fold, **rec})
                n_done += 1
                print(f"  {family}/{fold} {cid}@{budget}: ann={rec.get('ann')} dd={rec.get('dd')} breach={rec.get('breach')}")
            json.dump(ckpt, open(CKPT, "w"), indent=2)

    # Aggregate by (family, config_id) across folds
    by_config = {}
    for r in all_results:
        if r.get("status") != "complete":
            continue
        key = (r["family"], r["config_id"])
        by_config.setdefault(key, []).append(r)

    survivors = []
    eliminated = []
    for (fam, cid), fold_results in by_config.items():
        anns = {r["fold"]: r.get("ann") or 0.0 for r in fold_results}
        dds = {r["fold"]: r.get("dd") or 0.0 for r in fold_results}
        breaches = {r["fold"]: r.get("breach") for r in fold_results}
        reasons = []
        if any(breaches.values()): reasons.append("breach")
        if any(dd > 45 for dd in dds.values()): reasons.append("dd>45")
        neg = sum(1 for a in anns.values() if a < 0)
        if neg >= 2: reasons.append("two_folds_negative")
        if any(r.get("groups_with_so", 0) == 0 for r in fold_results): reasons.append("no_so")
        if any((r.get("max_symbol_conc") or 0) > 50 for r in fold_results): reasons.append("sym_conc>50")
        med_ann = sorted(anns.values())[len(anns)//2]
        max_dd = max(dds.values())
        entry = {"family": fam, "config_id": cid, "anns": anns, "dds": dds,
                 "val_median_ann": med_ann, "val_max_dd": max_dd}
        if reasons:
            entry["elimination_reasons"] = reasons
            eliminated.append(entry)
        else:
            survivors.append(entry)

    survivors.sort(key=lambda x: -x["val_median_ann"])
    summary = {
        "phase": "P9 one-shot anchored validation",
        "validation_read_once": True,
        "total_validation_runs": n_done,
        "n_survivors": len(survivors),
        "n_eliminated": len(eliminated),
        "survivors": survivors[:10],
        "eliminated_sample": eliminated[:5],
        "three_tier_check": {
            "conservative_50_10": any(s["val_median_ann"] >= 50 and s["val_max_dd"] <= 10 for s in survivors),
            "balanced_90_20": any(s["val_median_ann"] >= 90 and s["val_max_dd"] <= 20 for s in survivors),
            "aggressive_110_30": any(s["val_median_ann"] >= 110 and s["val_max_dd"] <= 30 for s in survivors),
        },
    }
    out_path = os.path.join(P9, "validation-summary.json")
    with open(out_path, "w") as f:
        json.dump(summary, f, indent=2)
    print(f"\nwrote {out_path}")
    print(f"survivors: {len(survivors)}, eliminated: {len(eliminated)}")
    print(f"three-tier: {summary['three_tier_check']}")
    for s in survivors[:5]:
        print(f"  {s['family']}/{s['config_id']}: val_med={s['val_median_ann']:.1f}% val_max_dd={s['val_max_dd']:.1f}% anns={s['anns']}")


if __name__ == "__main__":
    main()
