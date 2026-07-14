#!/usr/bin/env python3
"""Round 14 P2.3: 256 Sobol train-only screen.

Runs 256 configs covering the full parameter space:
- Family A (EMA proxy): XS lookback variation [4, 8, 12, 24, 48, 72, 96, 168] hours
- Family B (ADX proxy): SO scale variation [0.25, 0.5, 0.75, 1.0]
- Family C (VR proxy): rebalance period [1, 3, 7, 14] days
- Family D (downside vol proxy): inventory penalty [0.0, 0.25, 0.5, 1.0] × floor [0.25, 0.5]

Total: 8 × 4 × 4 × 2 = 256 configs (Sobol-like grid)
Each config gets FULL development window backtest (no fast screening).
"""
import json, os, subprocess, time, sys
from pathlib import Path
from itertools import product

REPO = Path("/home/bumblebee/Project/grid_binance")
BINARY = REPO / "target" / "release" / "r14_htf_search"
BASE = REPO / "docs/superpowers/artifacts/glm-martingale-core-round13/promising/r13-icp-trx-lp-pair.json"
OUTDIR = REPO / "docs/superpowers/artifacts/glm-martingale-core-round14"

def run_config(label, xs_lb, so_scale, rebal_days, inv_pen, inv_floor):
    env = os.environ.copy()
    env["R14_XS_FAMILY"] = "reversal"
    env["R14_XS_LOOKBACK"] = str(xs_lb)
    env["R14_XS_SKIP_RECENT"] = "0"
    env["R14_XS_REBALANCE_DAYS"] = str(rebal_days)
    env["R14_XS_ACTIVE_LONG"] = "2"
    env["R14_XS_ACTIVE_SHORT"] = "2"
    env["R14_DUAL_STATE"] = "1"
    env["R14_DUAL_SO_SCALE"] = str(so_scale)
    env["R14_DUAL_SPACING_MULT"] = "1.0"
    env["R14_INV_SCHED"] = "1"
    env["R14_INV_PENALTY"] = str(inv_pen)
    env["R14_RISK_FLOOR"] = str(inv_floor)
    cmd = [str(BINARY), str(BASE), label, "off"]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=300, env=env)
        if r.returncode != 0: return {"error": f"rc={r.returncode}"}
        try: return json.loads(r.stdout)
        except:
            s = r.stdout.strip()
            i, j = s.find('{'), s.rfind('}')
            return json.loads(s[i:j+1]) if i >= 0 else {"error": "no json"}
    except Exception as e: return {"error": str(e)}

# Parameter space: 8 × 4 × 4 × 2 = 256 configs
lookbacks = [4, 8, 12, 24, 48, 72, 96, 168]  # hours
so_scales = [0.25, 0.5, 0.75, 1.0]
rebalances = [1, 3, 7, 14]  # days
# penalty × floor = 2 combos per (lb, so, rebal)
pen_floors = [(1.0, 0.25), (0.5, 0.5)]

configs = []
for lb in lookbacks:
    for so in so_scales:
        for rebal in rebalances:
            for pen, floor in pen_floors:
                configs.append((lb, so, rebal, pen, floor))

print(f"Total configs: {len(configs)}")
assert len(configs) == 256, f"Expected 256, got {len(configs)}"

results = []
print("="*60)
print("P2.3: 256 Sobol Train-Only Screen (XS-REV + P4 + P5)")
print("="*60)
sys.stdout.flush()

for i, (lb, so, rebal, pen, floor) in enumerate(configs):
    label = f"sobol_{i:03d}_lb{lb}_so{so}_r{rebal}_p{pen}_f{floor}"
    if (i+1) % 16 == 0:
        print(f"\n  Progress: {i+1}/256")
        sys.stdout.flush()
    t0 = time.time()
    r = run_config(label, lb, so, rebal, pen, floor)
    elapsed = time.time() - t0
    if "error" not in r:
        full = r.get("full", {})
        ann = full.get("ann", -999)
        dd = full.get("dd", 999)
        pos = r.get("positive_segments", 0)
        if ann > 20:  # only print interesting results
            print(f"  [{i+1}] lb={lb} so={so} r={rebal} p={pen} f={floor}: ann={ann:.2f}% DD={dd:.2f}% pos={pos}/5 [{elapsed:.0f}s]")
            sys.stdout.flush()
        results.append({"label": label, "lb": lb, "so": so, "rebal": rebal,
                        "pen": pen, "floor": floor, "ann": ann, "dd": dd, "pos": pos,
                        "result": r})
    else:
        results.append({"label": label, "lb": lb, "so": so, "rebal": rebal,
                        "pen": pen, "floor": floor, "error": r["error"]})

    # Checkpoint every 32 configs
    if (i+1) % 32 == 0:
        with open(OUTDIR / "r14-p2-sobol-256-checkpoint.json", "w") as f:
            json.dump({"total": len(results), "results": results}, f, indent=2)
        valid = [r for r in results if "error" not in r]
        print(f"  Checkpoint: {len(results)} configs, {len(valid)} valid")
        sys.stdout.flush()

# Save final
with open(OUTDIR / "r14-p2-sobol-256-final.json", "w") as f:
    json.dump({"total": len(results), "results": results}, f, indent=2)

# Summary
print("\n" + "="*60)
print("SOBOL 256 SCREEN SUMMARY")
print("="*60)
valid = [r for r in results if "error" not in r and r.get("ann", -999) > -999]
print(f"Total: {len(results)}, Valid: {len(valid)}")

# Top 20 by Calmar
valid_with_calmar = []
for r in valid:
    calmar = r["ann"] / max(r["dd"], 0.1)
    valid_with_calmar.append((r, calmar))
valid_with_calmar.sort(key=lambda x: x[1], reverse=True)

print("\nTop 20 by Calmar (Pareto front):")
for r, calmar in valid_with_calmar[:20]:
    print(f"  lb={r['lb']} so={r['so']} r={r['rebal']} p={r['pen']} f={r['floor']}: "
          f"ann={r['ann']:.2f}% DD={r['dd']:.2f}% pos={r['pos']}/5 calmar={calmar:.2f}")

# Best by ann
best_ann = max(valid, key=lambda x: x["ann"])
print(f"\nBest ann: lb={best_ann['lb']} so={best_ann['so']} r={best_ann['rebal']} "
      f"p={best_ann['pen']} f={best_ann['floor']}: ann={best_ann['ann']:.2f}% DD={best_ann['dd']:.2f}%")

# Best by DD (with ann > 0)
positive = [r for r in valid if r["ann"] > 0]
if positive:
    best_dd = min(positive, key=lambda x: x["dd"])
    print(f"Best DD (ann>0): lb={best_dd['lb']} so={best_dd['so']} r={best_dd['rebal']} "
          f"p={best_dd['pen']} f={best_dd['floor']}: ann={best_dd['ann']:.2f}% DD={best_dd['dd']:.2f}%")

# Pattern analysis
print("\nParameter patterns (top 20):")
for param in ["lb", "so", "rebal", "pen", "floor"]:
    from collections import Counter
    vals = Counter(r[param] for r, _ in valid_with_calmar[:20])
    print(f"  {param}: {dict(vals)}")
