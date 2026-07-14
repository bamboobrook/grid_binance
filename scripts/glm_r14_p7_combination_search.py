#!/usr/bin/env python3
"""Round 14 P7: Controlled combination search - XS selector + HTF gate."""
import json, os, subprocess, time, sys
from pathlib import Path

REPO = Path("/home/bumblebee/Project/grid_binance")
BINARY = REPO / "target" / "release" / "r14_htf_search"
BASE = REPO / "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json"
OUTDIR = REPO / "docs/superpowers/artifacts/glm-martingale-core-round14"

def run(label, xs_family, xs_lb, xs_skip, xs_rebal, xs_active, htf_on):
    env = os.environ.copy()
    env["R14_XS_FAMILY"] = xs_family
    env["R14_XS_LOOKBACK"] = str(xs_lb)
    env["R14_XS_SKIP_RECENT"] = str(xs_skip)
    env["R14_XS_REBALANCE_DAYS"] = str(xs_rebal)
    env["R14_XS_ACTIVE_LONG"] = str(xs_active)
    env["R14_XS_ACTIVE_SHORT"] = str(min(xs_active, 5))
    cmd = [str(BINARY), str(BASE), label, "on" if htf_on else "off"]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=900, env=env)
        if r.returncode != 0: return {"error": f"rc={r.returncode}: {r.stderr[:200]}"}
        try: return json.loads(r.stdout)
        except:
            s = r.stdout.strip()
            i, j = s.find('{'), s.rfind('}')
            return json.loads(s[i:j+1]) if i >= 0 else {"error": "no json"}
    except Exception as e: return {"error": str(e)}

results = []
combos = [
    ("p7_mom_lb7_xs_only", "momentum", 7, 0, 1, 3, False),
    ("p7_mom_lb7_xs_htf", "momentum", 7, 0, 1, 3, True),
    ("p7_rev_lb72_xs_only", "reversal", 72, 0, 1, 5, False),
    ("p7_rev_lb72_xs_htf", "reversal", 72, 0, 1, 5, True),
    ("p7_rev_lb12_xs_only", "reversal", 12, 0, 7, 5, False),
    ("p7_rev_lb12_xs_htf", "reversal", 12, 0, 7, 5, True),
    ("p7_rev_lb24_xs_only", "reversal", 24, 0, 1, 5, False),
    ("p7_rev_lb24_xs_htf", "reversal", 24, 0, 1, 5, True),
    ("p7_rev_lb72_act7_xs_only", "reversal", 72, 0, 1, 7, False),
    ("p7_rev_lb72_act7_xs_htf", "reversal", 72, 0, 1, 7, True),
    ("p7_mom_lb7_act7_xs_only", "momentum", 7, 0, 1, 7, False),
    ("p7_mom_lb7_act7_xs_htf", "momentum", 7, 0, 1, 7, True),
]

print("="*60)
print("P7: XS Selector + HTF Gate Combination (12 configs)")
print("="*60)
sys.stdout.flush()

for i, (label, fam, lb, skip, rebal, active, htf) in enumerate(combos):
    print(f"\n[{i+1}/12] {label}")
    sys.stdout.flush()
    t0 = time.time()
    r = run(label, fam, lb, skip, rebal, active, htf)
    elapsed = time.time() - t0
    if "error" not in r:
        full = r.get("full", {})
        pos = r.get("positive_segments", 0)
        print(f"  ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% pos={pos}/5 [{elapsed:.0f}s]")
        sys.stdout.flush()
        for b in r.get("budgets", []):
            if b["budget"] in [3000.0, 4999.0]:
                print(f"    {b['budget']}U: ann={b['ann']:.2f}% DD={b['dd']:.2f}%")
        sys.stdout.flush()
    else:
        print(f"  ERROR: {r['error'][:80]}")
        sys.stdout.flush()
    results.append({"label": label, "family": fam, "lb": lb, "skip": skip,
                    "rebal": rebal, "active": active, "htf_on": htf, "result": r})
    if (i+1) % 4 == 0:
        with open(OUTDIR / "r14-p7-combination-checkpoint.json", "w") as f:
            json.dump({"total": len(results), "results": results}, f, indent=2)

with open(OUTDIR / "r14-p7-combination-final.json", "w") as f:
    json.dump({"total": len(results), "results": results}, f, indent=2)

print("\n" + "="*60)
print("P7 SUMMARY")
print("="*60)
valid = [r for r in results if "error" not in r.get("result", {})]
print(f"Total: {len(results)}, Valid: {len(valid)}")
top = sorted(valid, key=lambda x: x["result"].get("full", {}).get("ann", -999), reverse=True)
print("\nAll configs ranked by ann:")
for r in top:
    full = r["result"].get("full", {})
    pos = r["result"].get("positive_segments", 0)
    htf = "HTF" if r["htf_on"] else "   "
    print(f"  {htf} {r['label']}: ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% pos={pos}/5")
