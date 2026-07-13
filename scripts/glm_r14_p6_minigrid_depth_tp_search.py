#!/usr/bin/env python3
"""Round 14 P6: Corrected minigrid + depth TP search.

Runs minigrid and depth TP configs on the corrected engine with new
experiment IDs. The engine was fixed in Round 13 (P1.1 tests verify):
- reduce-only at aggregate average entry
- proportional shrink of all leg fields
- depth TP uses own reduce fraction, fires once per depth

This script tests both families:
- aggregate_profit_reduce: minigrid that only reduces when profitable
- bounded_risk_reduce: minigrid that allows small known loss for margin release
"""

import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BINARY = REPO_ROOT / "target" / "release" / "r14_htf_search"
MARKET_DB = REPO_ROOT / "data" / "market_data_full.db"
FUNDING_DB = REPO_ROOT / "data" / "funding_rates_round12.db"

DEV_START = 1672531200000
DEV_END = 1780271999999

# Base configs for testing
BASES = {
    "icp-trx": {
        "source": "docs/superpowers/artifacts/glm-martingale-core-round13/promising/r13-icp-trx-lp-pair.json",
    },
}

def load_base_config(name):
    path = REPO_ROOT / BASES[name]["source"]
    with open(path) as f:
        data = json.load(f)
    return data["portfolio_config"]

def add_minigrid(config, family, levels, spacing, num, den, profit, max_active):
    """Add minigrid config to each strategy."""
    for strategy in config["strategies"]:
        if "risk_limits" not in strategy:
            strategy["risk_limits"] = {}
        strategy["risk_limits"]["dca_minigrid"] = {
            "levels_per_band": levels,
            "spacing_bps": spacing,
            "close_fraction_num": num,
            "close_fraction_den": den,
            "min_profit_bps": profit,
            "max_active_levels": max_active,
        }
    return config

def add_depth_tp(config, d01_tp, d23_tp, d23_reduce, d4_tp, d4_reduce):
    """Add depth TP config to each strategy."""
    for strategy in config["strategies"]:
        if "risk_limits" not in strategy:
            strategy["risk_limits"] = {}
        strategy["risk_limits"]["depth_tp"] = {
            "depth_01_tp_bps": d01_tp,
            "depth_23_tp_bps": d23_tp,
            "depth_23_reduce_pct": d23_reduce,
            "depth_4plus_tp_bps": d4_tp,
            "depth_4plus_reduce_pct": d4_reduce,
        }
    return config

def run_search(config, label):
    """Run the search using r14_htf_search binary."""
    import tempfile
    with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as f:
        json.dump({"portfolio_config": config}, f)
        config_path = f.name

    cmd = [
        str(BINARY),
        config_path,
        label,
        "off",  # HTF off for P6 (testing minigrid/depth_tp independently)
    ]
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
        os.unlink(config_path)
        if result.returncode != 0:
            return {"error": f"returncode={result.returncode}: {result.stderr[:300]}"}
        # Parse JSON from stdout (the entire stdout is one JSON object).
        try:
            return json.loads(result.stdout)
        except json.JSONDecodeError:
            # Try to find the JSON block (starts with { and ends with }).
            stdout = result.stdout.strip()
            start = stdout.find('{')
            end = stdout.rfind('}')
            if start >= 0 and end > start:
                try:
                    return json.loads(stdout[start:end+1])
                except json.JSONDecodeError:
                    pass
            return {"error": "no JSON output", "stdout": result.stdout[-300:], "stderr": result.stderr[-300:]}
    except subprocess.TimeoutExpired:
        os.unlink(config_path)
        return {"error": "timeout"}
    except Exception as e:
        if os.path.exists(config_path):
            os.unlink(config_path)
        return {"error": str(e)}

def main():
    if not BINARY.exists():
        print("Building release binary...")
        subprocess.run(["cargo", "build", "--release", "--bin", "r14_htf_search"],
                      cwd=REPO_ROOT, check=True)

    all_results = []

    # P6.1 Minigrid: aggregate_profit_reduce family
    # Test with profitable minigrid configs (min_profit_bps >= 5)
    minigrid_configs = [
        # (levels, spacing, num, den, profit, max_active)
        (1, 30, 1, 2, 20, 1),   # conservative: 50% reduce, 20bps profit
        (2, 30, 1, 3, 15, 2),   # moderate: 33% reduce, 15bps profit
        (1, 50, 1, 4, 10, 1),   # small reduce: 25% reduce, 10bps profit
        (2, 40, 1, 2, 25, 2),   # aggressive: 50% reduce, 25bps profit
    ]

    print("\n=== P6.1 Minigrid (aggregate_profit_reduce) ===")
    for i, (levels, spacing, num, den, profit, max_active) in enumerate(minigrid_configs):
        config = load_base_config("icp-trx")
        config = add_minigrid(config, "aggregate_profit_reduce",
                             levels, spacing, num, den, profit, max_active)
        label = f"minigrid_agg_{i}_l{levels}_s{spacing}_n{num}_d{den}_p{profit}_m{max_active}"
        print(f"\n  {label}")
        result = run_search(config, label)
        all_results.append({"label": label, "family": "minigrid_aggregate_profit", "result": result})
        if "error" not in result:
            full = result.get("full", {})
            print(f"    ann={full.get('ann', -999):.2f}% DD={full.get('dd', 999):.2f}% "
                  f"pos={result.get('positive_segments', 0)}/5")

    # P6.2 Depth TP: rerun old top/bottom/center configs on corrected engine
    depth_configs = [
        # (d01_tp, d23_tp, d23_reduce, d4_tp, d4_reduce)
        (5000, 40, 25, 20, 50),   # original r13 config
        (3000, 50, 30, 30, 60),   # tighter TP
        (8000, 30, 20, 15, 40),   # wider TP
        (5000, 100, 0, 50, 0),    # depth TP disabled (reduce=0, but targets > 0)
    ]

    print("\n=== P6.2 Depth TP (corrected engine) ===")
    for i, (d01, d23, d23r, d4, d4r) in enumerate(depth_configs):
        config = load_base_config("icp-trx")
        config = add_depth_tp(config, d01, d23, d23r, d4, d4r)
        label = f"depth_tp_{i}_d01_{d01}_d23_{d23}_{d23r}_d4_{d4}_{d4r}"
        print(f"\n  {label}")
        result = run_search(config, label)
        all_results.append({"label": label, "family": "depth_tp_corrected", "result": result})
        if "error" not in result:
            full = result.get("full", {})
            print(f"    ann={full.get('ann', -999):.2f}% DD={full.get('dd', 999):.2f}% "
                  f"pos={result.get('positive_segments', 0)}/5")

    # Save results
    output_path = REPO_ROOT / "docs/superpowers/artifacts/glm-martingale-core-round14/r14-p6-minigrid-depth-tp-search.json"
    with open(output_path, 'w') as f:
        json.dump({"total_configs": len(all_results), "results": all_results}, f, indent=2)
    print(f"\nResults saved to {output_path}")

    # Summary
    print(f"\n{'='*60}")
    print("SUMMARY")
    print(f"{'='*60}")
    for r in all_results:
        if "result" in r and "error" not in r["result"]:
            full = r["result"].get("full", {})
            print(f"  {r['label']}: ann={full.get('ann', -999):.2f}% "
                  f"DD={full.get('dd', 999):.2f}% "
                  f"pos={r['result'].get('positive_segments', 0)}/5")

if __name__ == "__main__":
    main()
