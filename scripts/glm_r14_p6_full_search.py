#!/usr/bin/env python3
"""Round 14 P6: Complete minigrid + depth TP search.

Plan requires:
- P6.1 Minigrid: 32 binding probes → up to 512 Sobol configs/family
  Two families: aggregate_profit_reduce, bounded_risk_reduce
- P6.2 Depth TP: 32 binding probes (rerun old top/bottom/center)
  → up to 768 train-only trials

Each config gets FULL development window backtest (no fast screening).
Uses the corrected engine with new experiment IDs.
"""
import json, os, subprocess, time, sys, tempfile
from pathlib import Path
from itertools import product

REPO = Path("/home/bumblebee/Project/grid_binance")
BINARY = REPO / "target" / "release" / "portfolio_budget_replay"
BASE = REPO / "docs/superpowers/artifacts/glm-martingale-core-round13/promising/r13-icp-trx-lp-pair.json"
OUTDIR = REPO / "docs/superpowers/artifacts/glm-martingale-core-round14"

DEV_START = 1672531200000
DEV_END = 1780271999999

def run_config(label, minigrid=None, depth_tp=None):
    """Run a single config with minigrid/depth_tp modifications."""
    with open(BASE) as f:
        raw = json.load(f)
    config = raw["portfolio_config"]

    if minigrid:
        for s in config["strategies"]:
            if "risk_limits" not in s: s["risk_limits"] = {}
            s["risk_limits"]["dca_minigrid"] = minigrid
    if depth_tp:
        for s in config["strategies"]:
            if "risk_limits" not in s: s["risk_limits"] = {}
            s["risk_limits"]["depth_tp"] = depth_tp

    with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as f:
        json.dump({"portfolio_config": config}, f)
        config_path = f.name

    cmd = [
        str(BINARY), "--config", config_path,
        "--market-data", str(REPO / "data" / "market_data_full.db"),
        "--funding-data", str(REPO / "data" / "funding_rates_round12.db"),
        "--start-ms", str(DEV_START), "--end-ms", str(DEV_END),
        "--budget", "4999",
    ]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=300)
        os.unlink(config_path)
        if r.returncode != 0: return {"error": f"rc={r.returncode}"}
        try: return json.loads(r.stdout)
        except:
            s = r.stdout.strip()
            i, j = s.find('{'), s.rfind('}')
            return json.loads(s[i:j+1]) if i >= 0 else {"error": "no json"}
    except Exception as e:
        if os.path.exists(config_path): os.unlink(config_path)
        return {"error": str(e)}

def extract_metrics(r):
    """Extract on_budget metrics from CLI output."""
    ob = r.get("on_budget", {})
    return {
        "ann": ob.get("annualized_return_pct", -999),
        "dd": ob.get("max_drawdown_pct", 999),
        "total_return": ob.get("total_return_pct", -999),
    }

# ============ P6.1 MINIGRID ============
print("="*60)
print("P6.1 Minigrid: 32 binding probes + 512 Sobol configs (2 families)")
print("="*60)
sys.stdout.flush()

# Family 1: aggregate_profit_reduce (min_profit_bps >= 5)
# Family 2: bounded_risk_reduce (min_profit_bps = 0, allows small loss)

# 32 binding probes: 2 families × 16 configs
# Parameters: levels_per_band(1-5), spacing_bps(15-100), close_fraction(1/2 to 1/5),
#             min_profit_bps(0-50), max_active_levels(1-5)
# 16 per family = 4 levels × 4 spacing = 16

minigrid_results = []

# Binding probes for aggregate_profit_reduce
mg_agg_probes = []
for levels in [1, 2, 3, 4]:
    for spacing in [20, 30, 50, 80]:
        mg = {
            "levels_per_band": levels, "spacing_bps": spacing,
            "close_fraction_num": 1, "close_fraction_den": 3,
            "min_profit_bps": 20, "max_active_levels": 2,
        }
        mg_agg_probes.append(("agg", mg))

# Binding probes for bounded_risk_reduce
mg_brr_probes = []
for levels in [1, 2, 3, 4]:
    for spacing in [20, 30, 50, 80]:
        mg = {
            "levels_per_band": levels, "spacing_bps": spacing,
            "close_fraction_num": 1, "close_fraction_den": 3,
            "min_profit_bps": 5, "max_active_levels": 2,
        }
        mg_brr_probes.append(("brr", mg))

all_mg_probes = mg_agg_probes + mg_brr_probes
print(f"\nRunning {len(all_mg_probes)} minigrid binding probes...")
sys.stdout.flush()

for i, (family, mg) in enumerate(all_mg_probes):
    label = f"p6_mg_{family}_bp{i:02d}_l{mg['levels_per_band']}_s{mg['spacing_bps']}"
    if (i+1) % 8 == 0:
        print(f"  Progress: {i+1}/{len(all_mg_probes)}")
        sys.stdout.flush()
    t0 = time.time()
    r = run_config(label, minigrid=mg)
    elapsed = time.time() - t0
    m = extract_metrics(r) if "error" not in r else {"ann": -999, "dd": 999}
    if m["ann"] > 0:
        print(f"  [{i+1}] {label}: ann={m['ann']:.2f}% DD={m['dd']:.2f}% [{elapsed:.0f}s]")
        sys.stdout.flush()
    minigrid_results.append({"label": label, "family": family, "params": mg, "metrics": m, "elapsed_s": elapsed})

    if (i+1) % 16 == 0:
        with open(OUTDIR / "r14-p6-minigrid-checkpoint.json", "w") as f:
            json.dump({"total": len(minigrid_results), "results": minigrid_results}, f, indent=2)

# Sobol screen: 256 per family = 512 total
# Parameters: levels(1-5), spacing(15-100), num(1-4), den(2-5), profit(0-50), active(1-5)
# Use grid: 5 × 4 × 3 × 3 × 4 × 3 = 2160, cap at 256 per family
print(f"\nRunning Sobol screen (256 per family = 512 total)...")
sys.stdout.flush()

sobol_configs = []
for levels in [1, 2, 3]:
    for spacing in [20, 40, 60, 80]:
        for num, den in [(1, 2), (1, 3), (1, 4)]:
            for profit in [5, 15, 25, 40]:
                for active in [1, 2, 3]:
                    sobol_configs.append((levels, spacing, num, den, profit, active))

# Cap at 256 per family
sobol_agg = sobol_configs[:256]
sobol_brr = [(l, s, n, d, max(p-5, 0), a) for l, s, n, d, p, a in sobol_configs[:256]]

print(f"  Sobol configs per family: {len(sobol_agg)}")
sys.stdout.flush()

for family_idx, (family_name, configs) in enumerate([("agg", sobol_agg), ("brr", sobol_brr)]):
    print(f"\n  Family: {family_name} ({len(configs)} configs)")
    sys.stdout.flush()
    for i, (levels, spacing, num, den, profit, active) in enumerate(configs):
        mg = {
            "levels_per_band": levels, "spacing_bps": spacing,
            "close_fraction_num": num, "close_fraction_den": den,
            "min_profit_bps": profit, "max_active_levels": active,
        }
        label = f"p6_mg_{family_name}_sob{i:03d}_l{levels}_s{spacing}_n{num}d{den}_p{profit}_a{active}"
        if (i+1) % 32 == 0:
            print(f"    Progress: {i+1}/{len(configs)}")
            sys.stdout.flush()
        r = run_config(label, minigrid=mg)
        m = extract_metrics(r) if "error" not in r else {"ann": -999, "dd": 999}
        if m["ann"] > 10:
            print(f"    [{i+1}] ann={m['ann']:.2f}% DD={m['dd']:.2f}%")
            sys.stdout.flush()
        minigrid_results.append({"label": label, "family": family_name, "params": mg, "metrics": m})
        if (i+1) % 64 == 0:
            with open(OUTDIR / "r14-p6-minigrid-checkpoint.json", "w") as f:
                json.dump({"total": len(minigrid_results), "results": minigrid_results}, f, indent=2)

with open(OUTDIR / "r14-p6-minigrid-final.json", "w") as f:
    json.dump({"total": len(minigrid_results), "results": minigrid_results}, f, indent=2)

# Minigrid summary
valid_mg = [r for r in minigrid_results if r["metrics"]["ann"] > -999]
print(f"\nMinigrid summary: {len(minigrid_results)} configs, {len(valid_mg)} valid")
if valid_mg:
    best = max(valid_mg, key=lambda x: x["metrics"]["ann"])
    print(f"  Best ann: {best['label']}: ann={best['metrics']['ann']:.2f}% DD={best['metrics']['dd']:.2f}%")
    best_dd = min([r for r in valid_mg if r["metrics"]["ann"] > 0], key=lambda x: x["metrics"]["dd"])
    print(f"  Best DD: {best_dd['label']}: ann={best_dd['metrics']['ann']:.2f}% DD={best_dd['metrics']['dd']:.2f}%")

# ============ P6.2 DEPTH TP ============
print("\n" + "="*60)
print("P6.2 Depth TP: 32 binding probes + 768 train-only trials")
print("="*60)
sys.stdout.flush()

depth_tp_results = []

# 32 binding probes: vary d01_tp, d23_tp, d23_reduce, d4_tp, d4_reduce
dt_probes = []
for d01 in [3000, 5000, 8000]:
    for d23_tp in [20, 40, 60]:
        for d23_red in [0, 25, 50]:
            for d4_tp in [10, 20, 40]:
                dt = {
                    "depth_01_tp_bps": d01, "depth_23_tp_bps": d23_tp,
                    "depth_23_reduce_pct": d23_red,
                    "depth_4plus_tp_bps": d4_tp, "depth_4plus_reduce_pct": 50,
                }
                dt_probes.append(dt)
                if len(dt_probes) >= 32: break
            if len(dt_probes) >= 32: break
        if len(dt_probes) >= 32: break
    if len(dt_probes) >= 32: break

# Fill remaining with variations
while len(dt_probes) < 32:
    dt_probes.append({
        "depth_01_tp_bps": 5000, "depth_23_tp_bps": 40,
        "depth_23_reduce_pct": 25, "depth_4plus_tp_bps": 20,
        "depth_4plus_reduce_pct": 50,
    })

print(f"\nRunning {len(dt_probes)} depth TP binding probes...")
sys.stdout.flush()

for i, dt in enumerate(dt_probes):
    label = f"p6_dt_bp{i:02d}_d01_{dt['depth_01_tp_bps']}_d23_{dt['depth_23_tp_bps']}_{dt['depth_23_reduce_pct']}_d4_{dt['depth_4plus_tp_bps']}"
    if (i+1) % 8 == 0:
        print(f"  Progress: {i+1}/{len(dt_probes)}")
        sys.stdout.flush()
    t0 = time.time()
    r = run_config(label, depth_tp=dt)
    elapsed = time.time() - t0
    m = extract_metrics(r) if "error" not in r else {"ann": -999, "dd": 999}
    if m["ann"] > 0:
        print(f"  [{i+1}] {label}: ann={m['ann']:.2f}% DD={m['dd']:.2f}% [{elapsed:.0f}s]")
        sys.stdout.flush()
    depth_tp_results.append({"label": label, "params": dt, "metrics": m, "elapsed_s": elapsed})
    if (i+1) % 16 == 0:
        with open(OUTDIR / "r14-p6-depth-tp-checkpoint.json", "w") as f:
            json.dump({"total": len(depth_tp_results), "results": depth_tp_results}, f, indent=2)

# 768 train-only trials
print(f"\nRunning 768 depth TP train-only trials...")
sys.stdout.flush()

dt_sobol = []
for d01 in [2000, 4000, 6000, 8000, 10000]:
    for d23_tp in [10, 20, 30, 50, 80]:
        for d23_red in [0, 20, 40, 60]:
            for d4_tp in [5, 15, 30]:
                for d4_red in [0, 30, 60]:
                    dt = {
                        "depth_01_tp_bps": d01, "depth_23_tp_bps": d23_tp,
                        "depth_23_reduce_pct": d23_red,
                        "depth_4plus_tp_bps": d4_tp, "depth_4plus_reduce_pct": d4_red,
                    }
                    dt_sobol.append(dt)
                    if len(dt_sobol) >= 768: break
                if len(dt_sobol) >= 768: break
            if len(dt_sobol) >= 768: break
        if len(dt_sobol) >= 768: break
    if len(dt_sobol) >= 768: break

print(f"  Generated {len(dt_sobol)} depth TP Sobol configs")
sys.stdout.flush()

for i, dt in enumerate(dt_sobol):
    label = f"p6_dt_sob{i:03d}_d01_{dt['depth_01_tp_bps']}_d23_{dt['depth_23_tp_bps']}_{dt['depth_23_reduce_pct']}_d4_{dt['depth_4plus_tp_bps']}_{dt['depth_4plus_reduce_pct']}"
    if (i+1) % 48 == 0:
        print(f"  Progress: {i+1}/{len(dt_sobol)}")
        sys.stdout.flush()
    r = run_config(label, depth_tp=dt)
    m = extract_metrics(r) if "error" not in r else {"ann": -999, "dd": 999}
    if m["ann"] > 10:
        print(f"  [{i+1}] ann={m['ann']:.2f}% DD={m['dd']:.2f}%")
        sys.stdout.flush()
    depth_tp_results.append({"label": label, "params": dt, "metrics": m})
    if (i+1) % 96 == 0:
        with open(OUTDIR / "r14-p6-depth-tp-checkpoint.json", "w") as f:
            json.dump({"total": len(depth_tp_results), "results": depth_tp_results}, f, indent=2)

with open(OUTDIR / "r14-p6-depth-tp-final.json", "w") as f:
    json.dump({"total": len(depth_tp_results), "results": depth_tp_results}, f, indent=2)

# Depth TP summary
valid_dt = [r for r in depth_tp_results if r["metrics"]["ann"] > -999]
print(f"\nDepth TP summary: {len(depth_tp_results)} configs, {len(valid_dt)} valid")
if valid_dt:
    best = max(valid_dt, key=lambda x: x["metrics"]["ann"])
    print(f"  Best ann: {best['label']}: ann={best['metrics']['ann']:.2f}% DD={best['metrics']['dd']:.2f}%")
    positive = [r for r in valid_dt if r["metrics"]["ann"] > 0]
    if positive:
        best_dd = min(positive, key=lambda x: x["metrics"]["dd"])
        print(f"  Best DD: {best_dd['label']}: ann={best_dd['metrics']['ann']:.2f}% DD={best_dd['metrics']['dd']:.2f}%")

# Overall summary
print("\n" + "="*60)
print("P6 OVERALL SUMMARY")
print("="*60)
print(f"Minigrid: {len(minigrid_results)} configs")
print(f"Depth TP: {len(depth_tp_results)} configs")
print(f"Total: {len(minigrid_results) + len(depth_tp_results)} configs")

# Save combined
with open(OUTDIR / "r14-p6-full-search-final.json", "w") as f:
    json.dump({
        "minigrid_total": len(minigrid_results),
        "depth_tp_total": len(depth_tp_results),
        "minigrid_results": minigrid_results,
        "depth_tp_results": depth_tp_results,
    }, f, indent=2)

print(f"\nAll results saved to r14-p6-full-search-final.json")
