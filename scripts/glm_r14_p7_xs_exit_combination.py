#!/usr/bin/env python3
"""Round 14 P7: XS selector + minigrid/depth TP combination on ICP+TRX.

Tests if adding exit mechanisms to the XS selector improves risk-adjusted returns.
Uses ICP+TRX base (2 symbols, fast ~40s/config) for quick iteration.
"""
import json, os, subprocess, time, sys
from pathlib import Path

REPO = Path("/home/bumblebee/Project/grid_binance")
BINARY = REPO / "target" / "release" / "r14_htf_search"
BASE_ICP_TRX = REPO / "docs/superpowers/artifacts/glm-martingale-core-round13/promising/r13-icp-trx-lp-pair.json"
BASE_R4 = REPO / "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json"
OUTDIR = REPO / "docs/superpowers/artifacts/glm-martingale-core-round14"

def run_with_xs_exit(label, base_config, xs_family, xs_lb, xs_active, minigrid=None, depth_tp=None):
    """Run config with XS selector + optional minigrid/depth TP via env vars + config modification."""
    import tempfile
    # Load base config
    with open(base_config) as f:
        raw = json.load(f)
    config = raw["portfolio_config"]

    # Add minigrid/depth TP if specified
    if minigrid:
        for s in config["strategies"]:
            if "risk_limits" not in s:
                s["risk_limits"] = {}
            s["risk_limits"]["dca_minigrid"] = minigrid
    if depth_tp:
        for s in config["strategies"]:
            if "risk_limits" not in s:
                s["risk_limits"] = {}
            s["risk_limits"]["depth_tp"] = depth_tp

    # Write modified config
    with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as f:
        json.dump({"portfolio_config": config}, f)
        config_path = f.name

    env = os.environ.copy()
    env["R14_XS_FAMILY"] = xs_family
    env["R14_XS_LOOKBACK"] = str(xs_lb)
    env["R14_XS_SKIP_RECENT"] = "0"
    env["R14_XS_REBALANCE_DAYS"] = "1"
    env["R14_XS_ACTIVE_LONG"] = str(xs_active)
    env["R14_XS_ACTIVE_SHORT"] = str(min(xs_active, 5))

    cmd = [str(BINARY), config_path, label, "off"]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=300, env=env)
        os.unlink(config_path)
        if r.returncode != 0: return {"error": f"rc={r.returncode}: {r.stderr[:200]}"}
        try: return json.loads(r.stdout)
        except:
            s = r.stdout.strip()
            i, j = s.find('{'), s.rfind('}')
            return json.loads(s[i:j+1]) if i >= 0 else {"error": "no json"}
    except Exception as e:
        if os.path.exists(config_path): os.unlink(config_path)
        return {"error": str(e)}

# Minigrid configs (aggregate_profit_reduce family)
minigrid_configs = {
    "mg_conservative": {"levels_per_band": 1, "spacing_bps": 30, "close_fraction_num": 1, "close_fraction_den": 2, "min_profit_bps": 20, "max_active_levels": 1},
    "mg_moderate": {"levels_per_band": 2, "spacing_bps": 30, "close_fraction_num": 1, "close_fraction_den": 3, "min_profit_bps": 15, "max_active_levels": 2},
}

# Depth TP configs
depth_tp_configs = {
    "dt_original": {"depth_01_tp_bps": 5000, "depth_23_tp_bps": 40, "depth_23_reduce_pct": 25, "depth_4plus_tp_bps": 20, "depth_4plus_reduce_pct": 50},
    "dt_wider": {"depth_01_tp_bps": 8000, "depth_23_tp_bps": 30, "depth_23_reduce_pct": 20, "depth_4plus_tp_bps": 15, "depth_4plus_reduce_pct": 40},
}

results = []
print("="*60)
print("P7: XS Selector + Exit Mechanism Combination (ICP+TRX)")
print("="*60)
sys.stdout.flush()

# Test matrix: XS config × exit mechanism
combos = [
    # (label, base, xs_family, xs_lb, xs_active, minigrid_key, depth_tp_key)
    ("p7_xs_mom_only_icp", BASE_ICP_TRX, "momentum", 7, 1, None, None),
    ("p7_xs_mom_mg_cons_icp", BASE_ICP_TRX, "momentum", 7, 1, "mg_conservative", None),
    ("p7_xs_mom_mg_mod_icp", BASE_ICP_TRX, "momentum", 7, 1, "mg_moderate", None),
    ("p7_xs_mom_dt_orig_icp", BASE_ICP_TRX, "momentum", 7, 1, None, "dt_original"),
    ("p7_xs_mom_dt_wider_icp", BASE_ICP_TRX, "momentum", 7, 1, None, "dt_wider"),
    ("p7_xs_rev_only_icp", BASE_ICP_TRX, "reversal", 72, 2, None, None),
    ("p7_xs_rev_mg_cons_icp", BASE_ICP_TRX, "reversal", 72, 2, "mg_conservative", None),
    ("p7_xs_rev_dt_orig_icp", BASE_ICP_TRX, "reversal", 72, 2, None, "dt_original"),
]

for i, (label, base, fam, lb, active, mg_key, dt_key) in enumerate(combos):
    print(f"\n[{i+1}/{len(combos)}] {label}")
    sys.stdout.flush()
    mg = minigrid_configs.get(mg_key) if mg_key else None
    dt = depth_tp_configs.get(dt_key) if dt_key else None
    t0 = time.time()
    r = run_with_xs_exit(label, base, fam, lb, active, mg, dt)
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
    results.append({"label": label, "family": fam, "lb": lb, "active": active,
                    "minigrid": mg_key, "depth_tp": dt_key, "result": r})

with open(OUTDIR / "r14-p7-xs-exit-combination-final.json", "w") as f:
    json.dump({"total": len(results), "results": results}, f, indent=2)

print("\n" + "="*60)
print("P7 XS+EXIT SUMMARY")
print("="*60)
valid = [r for r in results if "error" not in r.get("result", {})]
print(f"Total: {len(results)}, Valid: {len(valid)}")
top = sorted(valid, key=lambda x: x["result"].get("full", {}).get("ann", -999), reverse=True)
print("\nAll configs ranked by ann:")
for r in top:
    full = r["result"].get("full", {})
    pos = r["result"].get("positive_segments", 0)
    mg = f"+{r['minigrid']}" if r['minigrid'] else ""
    dt = f"+{r['depth_tp']}" if r['depth_tp'] else ""
    print(f"  {r['label']}: ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% pos={pos}/5 {mg}{dt}")
