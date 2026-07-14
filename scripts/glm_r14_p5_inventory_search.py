#!/usr/bin/env python3
"""Round 14 P5: Inventory scheduler mechanism screen.

Tests if inventory penalty + downside-vol risk budget can improve risk-adjusted
returns on the best XS-REV config (with P4 SO=0.75).

Screen matrix: inventory_penalty × risk_floor × max_cycles
"""
import json, os, subprocess, time, sys
from pathlib import Path
from itertools import product

REPO = Path("/home/bumblebee/Project/grid_binance")
BINARY = REPO / "target" / "release" / "r14_htf_search"
BASE = REPO / "docs/superpowers/artifacts/glm-martingale-core-round13/promising/r13-icp-trx-lp-pair.json"
OUTDIR = REPO / "docs/superpowers/artifacts/glm-martingale-core-round14"

def run(label, inv_penalty=0.5, risk_floor=0.5):
    env = os.environ.copy()
    env["R14_XS_FAMILY"] = "reversal"
    env["R14_XS_LOOKBACK"] = "72"
    env["R14_XS_SKIP_RECENT"] = "0"
    env["R14_XS_REBALANCE_DAYS"] = "1"
    env["R14_XS_ACTIVE_LONG"] = "2"
    env["R14_XS_ACTIVE_SHORT"] = "2"
    env["R14_DUAL_STATE"] = "1"
    env["R14_DUAL_SO_SCALE"] = "0.75"
    env["R14_DUAL_SPACING_MULT"] = "1.0"
    env["R14_INV_SCHED"] = "1"
    env["R14_INV_PENALTY"] = str(inv_penalty)
    env["R14_RISK_FLOOR"] = str(risk_floor)
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

results = []
print("="*60)
print("P5: Inventory Scheduler Screen on XS-REV + P4 SO=0.75")
print("Parent: ann=34.20%/DD=13.83%/5/5")
print("="*60)
sys.stdout.flush()

# Screen: penalty × risk_floor = 4 × 3 = 12 configs
penalties = [0.0, 0.25, 0.5, 1.0]
floors = [0.25, 0.5, 0.75]

combos = list(product(penalties, floors))
for i, (pen, floor) in enumerate(combos):
    label = f"p5_pen{pen}_floor{floor}"
    print(f"\n[{i+1}/{len(combos)}] {label}")
    sys.stdout.flush()
    t0 = time.time()
    r = run(label, inv_penalty=pen, risk_floor=floor)
    elapsed = time.time() - t0
    if "error" not in r:
        full = r.get("full", {})
        pos = r.get("positive_segments", 0)
        dd = full.get("dd", 999)
        ann = full.get("ann", -999)
        dd_imp = "DD improved" if dd < 13.83 else ""
        print(f"  ann={ann:.2f}% DD={dd:.2f}% pos={pos}/5 {dd_imp} [{elapsed:.0f}s]")
        sys.stdout.flush()
    else:
        print(f"  ERROR: {r['error'][:80]}")
        sys.stdout.flush()
    results.append({"label": label, "penalty": pen, "risk_floor": floor,
                    "result": r, "elapsed_s": elapsed})
    if (i+1) % 4 == 0:
        with open(OUTDIR / "r14-p5-inventory-checkpoint.json", "w") as f:
            json.dump({"total": len(results), "results": results}, f, indent=2)

with open(OUTDIR / "r14-p5-inventory-final.json", "w") as f:
    json.dump({"total": len(results), "results": results}, f, indent=2)

print("\n" + "="*60)
print("P5 SUMMARY (parent: ann=34.20%/DD=13.83%/5/5)")
print("="*60)
valid = [r for r in results if "error" not in r.get("result", {})]
print(f"Total: {len(results)}, Valid: {len(valid)}")
print("\nRanked by DD (ascending):")
for r in sorted(valid, key=lambda x: x["result"].get("full", {}).get("dd", 999)):
    full = r["result"].get("full", {})
    pos = r["result"].get("positive_segments", 0)
    dd = full.get("dd", 999)
    ann = full.get("ann", -999)
    better = "✓better" if dd < 13.83 else ""
    print(f"  pen={r['penalty']:.2f} floor={r['risk_floor']:.2f}: ann={ann:.2f}% DD={dd:.2f}% pos={pos}/5 {better}")
print("\nRanked by Calmar:")
for r in sorted(valid, key=lambda x: x["result"].get("full", {}).get("ann", -999)/max(x["result"].get("full", {}).get("dd", 1), 0.1), reverse=True):
    full = r["result"].get("full", {})
    pos = r["result"].get("positive_segments", 0)
    dd = full.get("dd", 999)
    ann = full.get("ann", -999)
    calmar = ann / max(dd, 0.1)
    print(f"  pen={r['penalty']:.2f} floor={r['risk_floor']:.2f}: ann={ann:.2f}% DD={dd:.2f}% pos={pos}/5 calmar={calmar:.2f}")
