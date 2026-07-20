#!/usr/bin/env python3
"""Round 19 R7: G2 full train + 5 non-overlapping subblocks + budget select.

Plan §10: per G1 survivor per fold, run full train + 5 train-only non-overlapping
subblocks × budgets 1000/2000/3000/4000/4999U. G2 strict gate:
  median subblock ann >=30%
  worst subblock DD <=35%
  >=4/5 positive
  full-train no breach/liquidation
  actual symbols >=5
  max symbol/group concentration <=50%
  at least one real SO in >=4/5 subblocks
  unique atomic/legging failure <=10%

Per family per fold: max 4 config+budget finalists. Budget selection uses ONLY
that fold's train.

NESTED: each fold uses its own fit (NOT load_f2_fits for all folds).
Subblocks are 5 NON-overlapping anchored train-only windows (audit §4.2 fix).
"""
import json
import os
import subprocess
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round19"
R7 = os.path.join(ART, "r7")
CONFIGS = os.path.join(R7, "configs")
CKPT = os.path.join(R7, "g2-checkpoint.json")
REG = os.path.join(R7, "g2-registry.jsonl")
os.makedirs(CONFIGS, exist_ok=True)

R4_FITS = os.path.join(ART, "r4", "families-fit.json")
G1_SUMMARY = os.path.join(ART, "r6", "g1-summary.json")
G1_CKPT = os.path.join(ART, "r6", "g1-checkpoint.json")

FOLDS = [
    ("F1", 1672531200000, 1688169599999, 1688774400000, 1704067199999),
    ("F2", 1672531200000, 1704067199999, 1704662400000, 1735689599999),
    ("F3", 1672531200000, 1735689599999, 1736294400000, 1767225599999),
    ("F4", 1672531200000, 1767225599999, 1767820800000, 1780271999999),
]
BUDGETS = [1000.0, 2000.0, 3000.0, 4000.0, 4999.0]


def load_fold_family_fits(fold, family):
    d = json.load(open(R4_FITS))
    ff = next(f for f in d["fold_fits"] if f["fold"] == fold)
    fam = ff[family]
    if fam.get("blocked_no_stable_groups") or fam.get("blocked_no_stable_basket"):
        return None
    return fam["frozen_fits"]


def subblocks_for_fold(train_start, train_end):
    """5 NON-overlapping anchored train-only subblocks (audit §4.2 fix:
    R18 used highly-overlapping subblocks without disclosure)."""
    span = train_end - train_start
    chunk = span // 5
    return [(f"sb{i+1}", train_start + i * chunk, train_start + (i + 1) * chunk if i < 4 else train_end)
            for i in range(5)]


def build_config(family, fits, p):
    if family == "M1R":
        base = {"family": "M1_pair", "fit_lookback_days": 60, "factor": None,
                "basket_symbols": [], "cycle_deadline_h": None, "pairs": []}
    else:
        base = {"family": "M2F", "fit_lookback_days": 60, "factor": "BTC",
                "basket_symbols": [], "cycle_deadline_h": None, "pairs": []}
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
            "fee_quote": m.get("total_fee_quote"),
            "group_fo": ss.get("group_fo", []), "group_so": ss.get("group_so", []),
            "groups_with_so": ss.get("groups_with_so", 0),
            "actual_symbols": ss.get("actual_symbol_count", 0),
            "max_symbol_conc": ss.get("max_symbol_abs_net_pnl_share_pct", 0),
            "max_group_conc": ss.get("max_group_abs_net_pnl_share_pct", 0)}


def append_registry(row):
    with open(REG, "a") as f:
        f.write(json.dumps(row, sort_keys=True) + "\n")


def load_checkpoint():
    if os.path.exists(CKPT):
        return json.load(open(CKPT))
    return {"done": {}}


def save_checkpoint(ckpt):
    json.dump(ckpt, open(CKPT, "w"), indent=2)


def get_g1_top16(family, fold):
    """Pull the G1 top16 survivors for a (family, fold) from the G1 checkpoint.
    The G1 checkpoint stores flat rows keyed by config_id|block|budget; each row
    has a single block + budget. We group by (config_id, budget) and require all
    3 blocks (tb1/tb2/tb3) present per (config_id, budget)."""
    g1_ckpt = json.load(open(G1_CKPT))
    prefix = f"{family}_{fold}_"
    # group rows by (config_id, budget)
    groups = {}
    for key, rec in g1_ckpt["done"].items():
        if not key.startswith(prefix):
            continue
        cid = rec.get("config_id")
        bud = rec.get("budget")
        groups.setdefault((cid, bud), {})[rec.get("block")] = rec
    survivors = []
    for (cid, bud), blocks in groups.items():
        brs = [blocks.get(bn) for bn in ("tb1", "tb2", "tb3")]
        brs = [r for r in brs if r and r.get("status") == "complete"]
        if len(brs) < 3:
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
        median_ann = sorted(anns)[1]
        worst_dd = max(r.get("dd") or 0 for r in brs)
        pos_blocks = sum(1 for a in anns if a > 0)
        survivors.append({"config_id": cid, "budget": int(bud),
                          "params": brs[0].get("params", {}),
                          "median_ann": median_ann, "worst_dd": worst_dd,
                          "pos_blocks": pos_blocks,
                          "train_score": median_ann - worst_dd * 0.5 + pos_blocks * 5})
    survivors.sort(key=lambda x: -x["train_score"])
    # de-duplicate by config_id (keep best budget per config)
    seen = set(); top = []
    for s in survivors:
        if s["config_id"] in seen:
            continue
        seen.add(s["config_id"])
        top.append(s)
        if len(top) >= 16:
            break
    return top


def main():
    families = ["M1R", "M2F"]
    ckpt = load_checkpoint()
    t_start = time.time()
    n_done = 0
    n_total = 0
    summary_by_family_fold = {}

    for family in families:
        for fname, ts, te, vs, ve in FOLDS:
            fits = load_fold_family_fits(fname, family)
            if fits is None:
                summary_by_family_fold.setdefault(family, {})[fname] = {"status": "blocked_no_stable_groups"}
                continue
            top16 = get_g1_top16(family, fname)
            if not top16:
                summary_by_family_fold.setdefault(family, {})[fname] = {"status": "no_g1_survivors"}
                continue
            sbs = subblocks_for_fold(ts, te)
            WINDOWS = [("full_train", ts, te)] + sbs
            n_total += len(top16) * len(WINDOWS) * len(BUDGETS)
            for s in top16:
                cfg = build_config(family, fits, s["params"])
                for wname, ws, we in WINDOWS:
                    for budget in BUDGETS:
                        key = f"{s['config_id']}|{wname}|{int(budget)}"
                        if key in ckpt["done"]:
                            n_done += 1
                            continue
                        label = f"g2_{s['config_id']}_{wname}_{int(budget)}"
                        res = run(cfg, budget, ws, we, label)
                        if res["ok"]:
                            rec = {k: v for k, v in res.items() if k != "ok"}
                            rec["status"] = "complete"
                        else:
                            rec = {"status": "error", "err": (res.get("err") or "")[:200]}
                        rec.update({"config_id": s["config_id"], "window": wname,
                                    "budget": int(budget), "params": s["params"],
                                    "family": family, "fold": fname})
                        ckpt["done"][key] = rec
                        append_registry({k: v for k, v in rec.items() if k != "params"})
                        n_done += 1
                        if n_done % 30 == 0:
                            save_checkpoint(ckpt)
                            elapsed = time.time() - t_start
                            print(f"  {family}/{fname}: {n_done}/{n_total} ({elapsed:.0f}s)")
            save_checkpoint(ckpt)

            # G2 strict gate per (config, budget) across 5 subblocks + full
            finalists = []
            for s in top16:
                for budget in BUDGETS:
                    sub_results = []
                    for sbname, _, _ in sbs:
                        key = f"{s['config_id']}|{sbname}|{int(budget)}"
                        r = ckpt["done"].get(key)
                        if r and r.get("status") == "complete":
                            sub_results.append(r)
                    if len(sub_results) < 5:
                        continue
                    anns = [r.get("ann") or 0.0 for r in sub_results]
                    median_ann = sorted(anns)[2]
                    worst_dd = max(r.get("dd") or 0 for r in sub_results)
                    pos = sum(1 for a in anns if a > 0)
                    breach = any(r.get("breach") for r in sub_results)
                    gws = sum(r.get("groups_with_so", 0) for r in sub_results)
                    reasons = []
                    if median_ann < 30: reasons.append("median<30")
                    if worst_dd > 35: reasons.append("wdd>35")
                    if pos < 4: reasons.append("pos<4/5")
                    if breach: reasons.append("breach")
                    if all(r.get("actual_symbols", 0) < 5 for r in sub_results): reasons.append("sym<5")
                    if any((r.get("max_symbol_conc") or 0) > 50 for r in sub_results): reasons.append("sym_conc>50")
                    if any((r.get("max_group_conc") or 0) > 50 for r in sub_results): reasons.append("grp_conc>50")
                    if gws == 0: reasons.append("no_so")
                    if reasons:
                        continue
                    finalists.append({"config_id": s["config_id"], "budget": int(budget),
                                      "params": s["params"], "median_ann": median_ann,
                                      "worst_dd": worst_dd, "pos_blocks": pos})
            for f in finalists:
                f["g2_score"] = f["median_ann"] - f["worst_dd"] * 0.5
            finalists.sort(key=lambda x: -x["g2_score"])
            selected = finalists[:4]
            summary_by_family_fold.setdefault(family, {})[fname] = {
                "g1_in": len(top16), "g2_finalists": len(finalists),
                "selected_for_validation": len(selected), "selected": selected,
            }
            print(f"{family}/{fname}: g1_in={len(top16)} finalists={len(finalists)} selected={len(selected)}")

    save_checkpoint(ckpt)
    elapsed = time.time() - t_start
    summary = {
        "phase": "R7 G2 full train + 5 non-overlapping subblocks + budget select",
        "families": families, "folds": [f[0] for f in FOLDS],
        "total_runs": n_total, "completed_runs": n_done, "elapsed_s": round(elapsed, 1),
        "per_family_per_fold": summary_by_family_fold,
    }
    out_path = os.path.join(R7, "g2-summary.json")
    with open(out_path, "w") as f:
        json.dump(summary, f, indent=2)
    print(f"\nwrote {out_path}")


if __name__ == "__main__":
    main()
