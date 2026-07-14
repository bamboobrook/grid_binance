#!/usr/bin/env python3
"""Round 14 P4: Dual-state ladder ablation on XS-REV best config.

Tests if TREND_ADVERSE state (SO scale + spacing adjustment) can reduce DD
from 14.44% to <=10% conservative gate.

Ablation matrix:
- state_gate alone (no SO/spacing change, just block adverse)
- SO scale alone (0.25/0.5/0.75)
- spacing alone (1.25/1.5/2.0)
- SO + spacing combined
"""
import json, os, subprocess, time, sys
from pathlib import Path

REPO = Path("/home/bumblebee/Project/grid_binance")
BINARY = REPO / "target" / "release" / "r14_htf_search"
BASE = REPO / "docs/superpowers/artifacts/glm-martingale-core-round13/promising/r13-icp-trx-lp-pair.json"
OUTDIR = REPO / "docs/superpowers/artifacts/glm-martingale-core-round14"

def run(label, xs_family="reversal", xs_lb=72, xs_active=2,
        dual_state=False, so_scale=0.5, spacing_mult=1.5):
    env = os.environ.copy()
    env["R14_XS_FAMILY"] = xs_family
    env["R14_XS_LOOKBACK"] = str(xs_lb)
    env["R14_XS_SKIP_RECENT"] = "0"
    env["R14_XS_REBALANCE_DAYS"] = "1"
    env["R14_XS_ACTIVE_LONG"] = str(xs_active)
    env["R14_XS_ACTIVE_SHORT"] = str(xs_active)
    if dual_state:
        env["R14_DUAL_STATE"] = "1"
        env["R14_DUAL_SO_SCALE"] = str(so_scale)
        env["R14_DUAL_SPACING_MULT"] = str(spacing_mult)
    cmd = [str(BINARY), str(BASE), label, "off"]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=300, env=env)
        if r.returncode != 0: return {"error": f"rc={r.returncode}: {r.stderr[:200]}"}
        try: return json.loads(r.stdout)
        except:
            s = r.stdout.strip()
            i, j = s.find('{'), s.rfind('}')
            return json.loads(s[i:j+1]) if i >= 0 else {"error": "no json"}
    except Exception as e: return {"error": str(e)}

results = []
print("="*60)
print("P4: Dual-State Ladder Ablation on XS-REV (ICP+TRX)")
print("Target: DD 14.44% -> <=10% conservative gate")
print("="*60)
sys.stdout.flush()

# Baseline (no dual-state)
combos = [
    # (label, dual_state, so_scale, spacing_mult)
    ("p4_baseline_xs_rev", False, 1.0, 1.0),
    # SO scale ablations
    ("p4_so_025", True, 0.25, 1.0),
    ("p4_so_050", True, 0.50, 1.0),
    ("p4_so_075", True, 0.75, 1.0),
    # Spacing ablations
    ("p4_sp_125", True, 1.0, 1.25),
    ("p4_sp_150", True, 1.0, 1.50),
    ("p4_sp_200", True, 1.0, 2.00),
    # Combined SO + spacing
    ("p4_so050_sp150", True, 0.50, 1.50),
    ("p4_so025_sp200", True, 0.25, 2.00),
    ("p4_so075_sp125", True, 0.75, 1.25),
]

for i, (label, ds, so, sp) in enumerate(combos):
    print(f"\n[{i+1}/{len(combos)}] {label} (so={so}, sp={sp})")
    sys.stdout.flush()
    t0 = time.time()
    r = run(label, dual_state=ds, so_scale=so, spacing_mult=sp)
    elapsed = time.time() - t0
    if "error" not in r:
        full = r.get("full", {})
        pos = r.get("positive_segments", 0)
        dd = full.get("dd", 999)
        dd_gate = "DD<=10%✓" if dd <= 10 else "DD>10%✗"
        print(f"  ann={full.get('ann',-999):.2f}% DD={dd:.2f}% pos={pos}/5 {dd_gate} [{elapsed:.0f}s]")
        sys.stdout.flush()
        for b in r.get("budgets", []):
            if b["budget"] in [3000.0, 4999.0]:
                print(f"    {b['budget']}U: ann={b['ann']:.2f}% DD={b['dd']:.2f}%")
        sys.stdout.flush()
    else:
        print(f"  ERROR: {r['error'][:80]}")
        sys.stdout.flush()
    results.append({"label": label, "dual_state": ds, "so_scale": so,
                    "spacing_mult": sp, "result": r, "elapsed_s": elapsed})

with open(OUTDIR / "r14-p4-dual-state-ablation-final.json", "w") as f:
    json.dump({"total": len(results), "results": results}, f, indent=2)

print("\n" + "="*60)
print("P4 DUAL-STATE ABLATION SUMMARY")
print("="*60)
valid = [r for r in results if "error" not in r.get("result", {})]
print(f"Total: {len(results)}, Valid: {len(valid)}")
print("\nAll configs ranked by DD (ascending):")
for r in sorted(valid, key=lambda x: x["result"].get("full", {}).get("dd", 999)):
    full = r["result"].get("full", {})
    pos = r["result"].get("positive_segments", 0)
    dd = full.get("dd", 999)
    ann = full.get("ann", -999)
    gate = "✓" if dd <= 10 else "✗"
    print(f"  {r['label']}: ann={ann:.2f}% DD={dd:.2f}% pos={pos}/5 {gate}")
