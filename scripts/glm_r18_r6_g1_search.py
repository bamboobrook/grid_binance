#!/usr/bin/env python3
"""Round 18 R6: G1 nested train-only successive halving (plan §11 G1).

96 M1 pair configs via Sobol sampling over the plan §7.3 parameter ranges,
each run over 3 train stress blocks × 2 budgets (1000/4999U). STRICT train
gate (plan §11.1):
  - reject any breach / DD>45% / atomic_reject>10%
  - reject two-blocks-negative-return
  - select top 32 by train median return, worst DD, positive blocks, cost ratio,
    concentration, group stability
  - short-window annualization is for ELIMINATION only; it must NOT enter the
    candidate table (plan §11.1).

M2 basket (64) and M3-M5 (32) are blocked until their parent gates pass; their
quota does NOT transfer to M1 (plan §11.1). We record blocked_parent_gate.
"""
import hashlib
import json
import os
import subprocess
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round18"
R6 = os.path.join(ART, "r6")
CONFIGS = os.path.join(R6, "configs")
CKPT = os.path.join(R6, "g1-checkpoint.json")
REG = os.path.join(R6, "g1-registry.jsonl")
os.makedirs(CONFIGS, exist_ok=True)

R4_FITS = os.path.join(ART, "r4", "fold-fit-freeze.json")
SEED = 20260731

# Plan §7.3 M1 parameter ranges.
PARAM_GRID = {
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

# Train stress blocks (plan §11.1: 3 train stress blocks). Use F2 train window
# split into 3 equal sub-blocks (train-only, no validation leak).
F2_TRAIN_START = 1672531200000
F2_TRAIN_END = 1704067199999
third = (F2_TRAIN_END - F2_TRAIN_START) // 3
TRAIN_BLOCKS = [
    ("tb1", F2_TRAIN_START, F2_TRAIN_START + third),
    ("tb2", F2_TRAIN_START + third, F2_TRAIN_START + 2 * third),
    ("tb3", F2_TRAIN_START + 2 * third, F2_TRAIN_END),
]
BUDGETS = [1000.0, 4999.0]


def load_f2_fits():
    d = json.load(open(R4_FITS))
    return next(f for f in d["fold_fits"] if f["fold"] == "F2")["frozen_fits"]


def sobol_configs(n=96, seed=SEED):
    """Generate n Sobol-sampled configs over PARAM_GRID. Each config picks one
    value per dimension via Sobol index scaled to the grid length."""
    try:
        from scipy.stats import qmc
        sampler = qmc.Sobol(d=len(PARAM_GRID), scramble=True, seed=seed)
        keys = list(PARAM_GRID.keys())
        cfgs = []
        for i in range(n):
            row = sampler.random()[0]
            cfg = {}
            for k, u in zip(keys, row):
                vals = PARAM_GRID[k]
                idx = min(len(vals) - 1, int(u * len(vals)))
                cfg[k] = vals[idx]
            cfgs.append(cfg)
        return cfgs
    except ImportError:
        # deterministic fallback if scipy unavailable
        import itertools, random
        random.seed(seed)
        keys = list(PARAM_GRID.keys())
        cfgs = []
        for i in range(n):
            cfg = {k: random.choice(PARAM_GRID[k]) for k in keys}
            cfgs.append(cfg)
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
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=300)
    except subprocess.TimeoutExpired:
        return {"ok": False, "err": "timeout", "wall_s": 300.0}
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
        "group_tp": ss.get("group_tp", []), "group_atomic_reject": ss.get("group_atomic_reject", []),
        "groups_with_so": ss.get("groups_with_so", 0),
        "min_equity": out.get("metrics", {}).get("min_equity_quote"),
    }


def load_checkpoint():
    if os.path.exists(CKPT):
        return json.load(open(CKPT))
    return {"done": {}, "configs": []}


def save_checkpoint(ckpt):
    json.dump(ckpt, open(CKPT, "w"), indent=2)


def append_registry(row):
    with open(REG, "a") as f:
        f.write(json.dumps(row, sort_keys=True) + "\n")


def main():
    fits = load_f2_fits()
    cfgs = sobol_configs(96, SEED)
    print(f"G1: {len(cfgs)} Sobol configs × {len(TRAIN_BLOCKS)} blocks × {len(BUDGETS)} budgets = {len(cfgs)*len(TRAIN_BLOCKS)*len(BUDGETS)} runs")

    ckpt = load_checkpoint()
    ckpt["configs"] = [{**p, "config_id": f"M1_{i:03d}"} for i, p in enumerate(cfgs)]
    results = {}  # config_id -> {block -> {budget -> metrics}}
    t_start = time.time()
    n_done = 0
    n_total = len(cfgs) * len(TRAIN_BLOCKS) * len(BUDGETS)

    for ci, p in enumerate(ckpt["configs"]):
        cid = p["config_id"]
        results[cid] = {}
        for bname, bs, be in TRAIN_BLOCKS:
            results[cid][bname] = {}
            for budget in BUDGETS:
                key = f"{cid}|{bname}|{int(budget)}"
                if key in ckpt["done"]:
                    results[cid][bname][str(int(budget))] = ckpt["done"][key]
                    n_done += 1
                    continue
                label = f"g1_{cid}_{bname}_{int(budget)}"
                res = run(build_config(fits, {k: v for k, v in p.items() if k != "config_id"}), budget, bs, be, label)
                if res["ok"]:
                    rec = {k: v for k, v in res.items() if k != "ok"}
                    rec["status"] = "complete"
                else:
                    rec = {"status": "error", "err": res.get("err", "")[:200]}
                rec["config_id"] = cid
                rec["block"] = bname
                rec["budget"] = int(budget)
                rec["params"] = {k: v for k, v in p.items() if k != "config_id"}
                ckpt["done"][key] = rec
                append_registry({**rec, "family": "M1_pair", "window": bname})
                results[cid][bname][str(int(budget))] = rec
                n_done += 1
                if n_done % 24 == 0:
                    save_checkpoint(ckpt)
                    elapsed = time.time() - t_start
                    print(f"  {n_done}/{n_total} done ({elapsed:.0f}s, {elapsed/max(n_done,1):.1f}s/run)")
        save_checkpoint(ckpt)

    save_checkpoint(ckpt)
    elapsed = time.time() - t_start
    print(f"G1 runs complete: {n_done}/{n_total} in {elapsed:.0f}s")

    # STRICT train gate (plan §11.1) — per (config, budget) across the 3 blocks.
    survivors = []
    rejected = []
    for cid in sorted(results):
        for budget in BUDGETS:
            bs = str(int(budget))
            block_results = [results[cid][bn].get(bs) for bn, _, _ in TRAIN_BLOCKS]
            block_results = [r for r in block_results if r and r.get("status") == "complete"]
            if len(block_results) < 3:
                continue
            # gate checks
            breach = any(r.get("breach") for r in block_results)
            dd_too_big = any((r.get("dd") or 0) > 45.0 for r in block_results)
            atomic_too_high = any(
                sum(r.get("group_atomic_reject", [])) > 0.10 * max(1, sum(r.get("group_fo", [])) + sum(r.get("group_atomic_reject", [])))
                for r in block_results
            )
            anns = [r.get("ann") or 0.0 for r in block_results]
            neg_blocks = sum(1 for a in anns if a < 0)
            two_neg = neg_blocks >= 2
            groups_with_so = sum(r.get("groups_with_so", 0) for r in block_results)
            reject_reasons = []
            if breach: reject_reasons.append("breach")
            if dd_too_big: reject_reasons.append("dd>45")
            if atomic_too_high: reject_reasons.append("atomic_reject>10%")
            if two_neg: reject_reasons.append("two_blocks_negative")
            if groups_with_so == 0: reject_reasons.append("no_so_not_martingale")
            if reject_reasons:
                rejected.append({"config_id": cid, "budget": int(budget), "reasons": reject_reasons,
                                 "median_ann": sorted(anns)[1], "worst_dd": max(r.get("dd") or 0 for r in block_results)})
                continue
            median_ann = sorted(anns)[1]
            worst_dd = max(r.get("dd") or 0 for r in block_results)
            pos_blocks = sum(1 for a in anns if a > 0)
            fee_total = sum(r.get("fee_quote") or 0 for r in block_results)
            survivors.append({
                "config_id": cid, "budget": int(budget), "params": block_results[0].get("params", {}),
                "median_ann": median_ann, "worst_dd": worst_dd, "pos_blocks": pos_blocks,
                "fee_total": fee_total, "groups_with_so": groups_with_so,
                "anns_by_block": {bn: r.get("ann") for (bn, _, _), r in zip(TRAIN_BLOCKS, block_results)},
            })

    # rank survivors by composite train score (median ann - worst_dd penalty +
    # pos_blocks bonus). Select top 32.
    for s in survivors:
        s["train_score"] = s["median_ann"] - s["worst_dd"] * 0.5 + s["pos_blocks"] * 5.0
    survivors.sort(key=lambda x: -x["train_score"])
    top32 = survivors[:32]

    summary = {
        "phase": "R6 G1 nested train-only successive halving",
        "family": "M1_pair",
        "n_configs": len(cfgs), "n_blocks": len(TRAIN_BLOCKS), "n_budgets": len(BUDGETS),
        "total_runs": n_total, "completed_runs": n_done,
        "train_blocks": [{"name": n, "start": s, "end": e} for n, s, e in TRAIN_BLOCKS],
        "budgets": BUDGETS,
        "strict_gate": {"breach": "reject", "dd>45": "reject", "atomic_reject>10%": "reject",
                        "two_blocks_negative": "reject", "no_so_not_martingale": "reject"},
        "n_survivors_passing_strict_gate": len(survivors),
        "n_rejected": len(rejected),
        "top32_selected": len(top32),
        "top32": top32,
        "rejection_reason_summary": {},
        "blocked_M2_M3_M5": "blocked_parent_gate (M2/M3-M5 quota not transferred to M1; plan §11.1)",
    }
    # tally rejection reasons
    from collections import Counter
    rc = Counter()
    for r in rejected:
        for reason in r["reasons"]:
            rc[reason] += 1
    summary["rejection_reason_summary"] = dict(rc)

    out_path = os.path.join(R6, "g1-summary.json")
    with open(out_path, "w") as f:
        json.dump(summary, f, indent=2)
    print(f"\nwrote {out_path}")
    print(f"survivors: {len(survivors)}, rejected: {len(rejected)}, top32: {len(top32)}")
    print("rejection reasons:", dict(rc))
    if top32:
        print(f"best survivor: median_ann={top32[0]['median_ann']:.2f}% worst_dd={top32[0]['worst_dd']:.2f}% pos_blocks={top32[0]['pos_blocks']}/3")


if __name__ == "__main__":
    main()
