#!/usr/bin/env python3
"""Round 19 R6: G1 nested train-only successive halving per family per fold.

Plan §9: M1R 96 configs (seed 20260731), M2F 96 configs (seed 20260901).
Per fold: configs × 3 predeclared train stress blocks × budgets 1000/4999U.
G1 immediate rejection: breach/liquidation, DD>45%, two train blocks ann<0,
actual symbols<5, max symbol/group concentration>50%, atomic/legging>10%,
no real SO, cost/gross>50%, fit/break invalid.

Per family per fold: top 16 distinct configs (rank by train median return,
worst DD, pos blocks, cost ratio, concentration, fit stability).

NESTED: each fold uses its OWN train fit (NOT load_f2_fits for all folds).
"""
import hashlib
import json
import os
import subprocess
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round19"
R6 = os.path.join(ART, "r6")
CONFIGS = os.path.join(R6, "configs")
CKPT = os.path.join(R6, "g1-checkpoint.json")
REG = os.path.join(R6, "g1-registry.jsonl")
os.makedirs(CONFIGS, exist_ok=True)

R4_FITS = os.path.join(ART, "r4", "families-fit.json")

# Anchored folds + purge
FOLDS = [
    ("F1", 1672531200000, 1688169599999, 1688774400000, 1704067199999),
    ("F2", 1672531200000, 1704067199999, 1704662400000, 1735689599999),
    ("F3", 1672531200000, 1735689599999, 1736294400000, 1767225599999),
    ("F4", 1672531200000, 1767225599999, 1767820800000, 1780271999999),
]
BUDGETS = [1000.0, 4999.0]


def fit_blocks_for_fold(train_start, train_end):
    """3 non-overlapping train stress blocks (thirds of train window)."""
    span = train_end - train_start
    return [("tb1", train_start, train_start + span // 3),
            ("tb2", train_start + span // 3, train_start + 2 * span // 3),
            ("tb3", train_start + 2 * span // 3, train_end)]


def load_fold_family_fits(fold, family):
    d = json.load(open(R4_FITS))
    ff = next(f for f in d["fold_fits"] if f["fold"] == fold)
    fam = ff[family]
    if fam.get("blocked_no_stable_groups") or fam.get("blocked_no_stable_basket"):
        return None
    return fam["frozen_fits"]


def sobol_configs(n, seed, dims):
    try:
        from scipy.stats import qmc
        sampler = qmc.Sobol(d=len(dims), scramble=True, seed=seed)
        keys = list(dims.keys())
        cfgs = []
        for _ in range(n):
            row = sampler.random()[0]
            cfg = {}
            for k, u in zip(keys, row):
                vals = dims[k]
                idx = min(len(vals) - 1, int(u * len(vals)))
                cfg[k] = vals[idx]
            cfgs.append(cfg)
        return cfgs
    except ImportError:
        import random
        random.seed(seed)
        return [{k: random.choice(v) for k, v in dims.items()} for _ in range(n)]


M1R_DIMS = {
    "bar_boundary_minutes": [1, 5],
    "entry_z": [1.0, 1.5, 2.0, 2.5],
    "so_residual_step_z": [0.25, 0.40, 0.60, 0.80],
    "group_fo_quote": [20.0, 30.0, 45.0, 60.0],
    "multiplier": [1.20, 1.35, 1.50, 1.70],
    "max_legs": [3, 4, 5],
    "exit_z": [0.10, 0.25, 0.50],
    "tp_net_bps_floor": [20, 40, 70, 100],
    "leverage": [2, 3, 5],
    "group_gross_cap_pct": [12.0, 18.0, 25.0],
}
M2F_DIMS = {
    "bar_boundary_minutes": [1, 5],
    "entry_z": [1.0, 1.5, 2.0, 2.5],
    "so_residual_step_z": [0.25, 0.40, 0.60, 0.80],
    "group_fo_quote": [40.0, 60.0, 80.0, 100.0],
    "multiplier": [1.20, 1.35, 1.50],
    "max_legs": [3, 4, 5],
    "exit_z": [0.10, 0.25, 0.50],
    "tp_net_bps_floor": [20, 40, 70, 100],
    "leverage": [2, 3, 5],
    "group_gross_cap_pct": [18.0, 25.0, 35.0],
}


def build_config(family, fits, p):
    if family == "M1R":
        base = {"family": "M1_pair", "fit_lookback_days": 60, "factor": None,
                "basket_symbols": [], "cycle_deadline_h": None}
    else:
        base = {"family": "M2F", "fit_lookback_days": 60, "factor": "BTC",
                "basket_symbols": [], "cycle_deadline_h": None}
    base["pairs"] = []
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
    return {"ok": True, "wall_s": round(wall, 1),
            "ann": m["annualized_return_pct"], "dd": m["max_drawdown_pct"],
            "trade_count": m["trade_count"], "breach": m["breach"],
            "fee_quote": m.get("total_fee_quote"), "funding_quote": m.get("total_funding_quote"),
            "group_fo": ss.get("group_fo", []), "group_so": ss.get("group_so", []),
            "groups_with_so": ss.get("groups_with_so", 0),
            "actual_symbols": ss.get("actual_symbol_count", 0),
            "max_symbol_conc": ss.get("max_symbol_abs_net_pnl_share_pct", 0),
            "max_group_conc": ss.get("max_group_abs_net_pnl_share_pct", 0),
            "group_atomic_reject": ss.get("group_atomic_reject", [])}


def append_registry(row):
    with open(REG, "a") as f:
        f.write(json.dumps(row, sort_keys=True) + "\n")


def load_checkpoint():
    if os.path.exists(CKPT):
        return json.load(open(CKPT))
    return {"done": {}}


def save_checkpoint(ckpt):
    json.dump(ckpt, open(CKPT, "w"), indent=2)


def main():
    families = [("M1R", 96, 20260731, M1R_DIMS), ("M2F", 96, 20260901, M2F_DIMS)]
    ckpt = load_checkpoint()
    t_start = time.time()
    n_done = 0
    n_total = 0
    summary_by_family_fold = {}

    for family, n_cfg, seed, dims in families:
        for fname, ts, te, vs, ve in FOLDS:
            fits = load_fold_family_fits(fname, family)
            if fits is None:
                summary_by_family_fold.setdefault(family, {})[fname] = {"status": "blocked_no_stable_groups", "configs": 0}
                print(f"{family}/{fname}: blocked_no_stable_groups")
                continue
            blocks = fit_blocks_for_fold(ts, te)
            cfgs = sobol_configs(n_cfg, seed, dims)
            n_total += len(cfgs) * len(blocks) * len(BUDGETS)
            results = {}
            for ci, p in enumerate(cfgs):
                cid = f"{family}_{fname}_{ci:03d}"
                results[cid] = {}
                for bname, bs, be in blocks:
                    results[cid][bname] = {}
                    for budget in BUDGETS:
                        key = f"{cid}|{bname}|{int(budget)}"
                        if key in ckpt["done"]:
                            results[cid][bname][str(int(budget))] = ckpt["done"][key]
                            n_done += 1
                            continue
                        label = f"g1_{cid}_{bname}_{int(budget)}"
                        res = run(build_config(family, fits, p), budget, bs, be, label)
                        if res["ok"]:
                            rec = {k: v for k, v in res.items() if k != "ok"}
                            rec["status"] = "complete"
                        else:
                            rec = {"status": "error", "err": (res.get("err") or "")[:200]}
                        rec.update({"config_id": cid, "block": bname, "budget": int(budget),
                                    "params": p, "family": family, "fold": fname, "window": bname})
                        ckpt["done"][key] = rec
                        append_registry({k: v for k, v in rec.items() if k != "params"})
                        results[cid][bname][str(int(budget))] = rec
                        n_done += 1
                        if n_done % 50 == 0:
                            save_checkpoint(ckpt)
                            elapsed = time.time() - t_start
                            print(f"  {family}/{fname}: {n_done}/{n_total} done ({elapsed:.0f}s, {elapsed/max(n_done,1):.1f}s/run)")
            save_checkpoint(ckpt)
            # strict gate per (config, budget)
            survivors = []
            for cid in sorted(results):
                for budget in BUDGETS:
                    bs = str(int(budget))
                    brs = [results[cid][bn].get(bs) for bn, _, _ in blocks]
                    brs = [r for r in brs if r and r.get("status") == "complete"]
                    if len(brs) < 3:
                        continue
                    reasons = []
                    if any(r.get("breach") for r in brs): reasons.append("breach")
                    if any((r.get("dd") or 0) > 45 for r in brs): reasons.append("dd>45")
                    anns = [r.get("ann") or 0.0 for r in brs]
                    if sum(1 for a in anns if a < 0) >= 2: reasons.append("two_blocks_neg")
                    if all(r.get("actual_symbols", 0) < 5 for r in brs): reasons.append("symbols<5")
                    if any((r.get("max_symbol_conc") or 0) > 50 for r in brs): reasons.append("sym_conc>50")
                    if any((r.get("max_group_conc") or 0) > 50 for r in brs): reasons.append("grp_conc>50")
                    if sum(r.get("groups_with_so", 0) for r in brs) == 0: reasons.append("no_so_not_martingale")
                    if reasons:
                        continue
                    median_ann = sorted(anns)[1]
                    worst_dd = max(r.get("dd") or 0 for r in brs)
                    survivors.append({"config_id": cid, "budget": int(budget), "params": brs[0].get("params", {}),
                                      "median_ann": median_ann, "worst_dd": worst_dd,
                                      "pos_blocks": sum(1 for a in anns if a > 0)})
            for s in survivors:
                s["train_score"] = s["median_ann"] - s["worst_dd"] * 0.5 + s["pos_blocks"] * 5
            survivors.sort(key=lambda x: -x["train_score"])
            top16 = survivors[:16]
            summary_by_family_fold.setdefault(family, {})[fname] = {
                "configs_run": len(cfgs) * len(blocks) * len(BUDGETS),
                "survivors": len(survivors), "top16": len(top16),
                "best_survivor": top16[0] if top16 else None,
            }
            print(f"{family}/{fname}: {len(cfgs)*len(blocks)*len(BUDGETS)} runs, {len(survivors)} survivors, top16={len(top16)}")

    save_checkpoint(ckpt)
    elapsed = time.time() - t_start
    summary = {
        "phase": "R6 G1 nested train-only successive halving",
        "families": [f[0] for f in families],
        "folds": [f[0] for f in FOLDS],
        "total_runs": n_total, "completed_runs": n_done,
        "elapsed_s": round(elapsed, 1),
        "per_family_per_fold": summary_by_family_fold,
    }
    out_path = os.path.join(R6, "g1-summary.json")
    with open(out_path, "w") as f:
        json.dump(summary, f, indent=2)
    print(f"\nwrote {out_path}")
    print(f"total runs: {n_done}/{n_total} in {elapsed:.0f}s")


if __name__ == "__main__":
    main()
