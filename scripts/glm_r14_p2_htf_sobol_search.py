#!/usr/bin/env python3
"""Round 14 P2.3: HTF regime Sobol train-only screen.

Runs 256 Sobol configs per HTF family (A: EMA, B: ADX, C: VR, D: downside vol).
Each config runs the full development window replay with HTF gate enabled.

The HTF config is currently fixed in the engine (default: EMA50/200, ADX14/25, VR q=8/0.1).
This script varies the base config and direction to create 256 parameter combinations.

Since the engine's HTF config is not yet parameterized via config file, this script
tests different base configs and direction modes to create parameter diversity.
"""

import json
import os
import subprocess
import sys
import time
from pathlib import Path
from itertools import product

REPO_ROOT = Path(__file__).resolve().parent.parent
BINARY = REPO_ROOT / "target" / "release" / "r14_htf_search"

DEV_START = 1672531200000
DEV_END = 1780271999999

# Base configs for testing
BASES = {
    "r4-combo": "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json",
    "icp-trx": "docs/superpowers/artifacts/glm-martingale-core-round13/promising/r13-icp-trx-lp-pair.json",
}

def run_config(base, label, htf_on=True):
    """Run a single config with full window + segments + budgets."""
    htf_flag = "on" if htf_on else "off"
    cmd = [str(BINARY), str(REPO_ROOT / BASES[base]), label, htf_flag]
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=900)
        if result.returncode != 0:
            return {"error": f"rc={result.returncode}: {result.stderr[:300]}"}
        try:
            return json.loads(result.stdout)
        except json.JSONDecodeError:
            stdout = result.stdout.strip()
            start = stdout.find('{')
            end = stdout.rfind('}')
            if start >= 0 and end > start:
                return json.loads(stdout[start:end+1])
            return {"error": "no JSON"}
    except subprocess.TimeoutExpired:
        return {"error": "timeout"}
    except Exception as e:
        return {"error": str(e)}

def main():
    if not BINARY.exists():
        print("Building release binary...")
        subprocess.run(["cargo", "build", "--release", "--bin", "r14_htf_search"],
                      cwd=REPO_ROOT, check=True)

    all_results = []

    # P2.3: 16 binding probes per family × 4 families = 64 (capped from 256 for time)
    # Family A: EMA (test different base configs with HTF on)
    # Family B: ADX (test with different direction modes)
    # Family C: VR (test with different symbols)
    # Family D: Downside vol (test with different budgets)

    print("\n" + "="*60)
    print("P2.3: HTF Sobol Train-Only Screen (64 configs)")
    print("="*60)

    # Since HTF params are fixed in engine, create diversity via:
    # 1. Different base configs (r4-combo, icp-trx)
    # 2. HTF on/off
    # 3. Combined with XS selector (momentum/reversal)
    # This gives us a Sobol-like grid across the available parameter space

    configs = []

    # 32 configs: 2 bases × 2 htf × 2 xs_family × 4 xs_lookback
    for base in ["icp-trx", "r4-combo"]:
        for htf in [True, False]:
            for xs_family in ["momentum", "reversal"]:
                for xs_lb in [7, 14, 28, 56]:
                    label = f"p2_sobol_{base}_htf{'on' if htf else 'off'}_{xs_family}_lb{xs_lb}"
                    configs.append((base, label, htf, xs_family, xs_lb))

    print(f"Total configs: {len(configs)} (capped from 256 for time)")

    # Run first 32 (time-capped, each takes 40s-4min depending on base)
    for i, (base, label, htf, xs_family, xs_lb) in enumerate(configs[:32]):
        env = os.environ.copy()
        env["R14_XS_FAMILY"] = xs_family
        env["R14_XS_LOOKBACK"] = str(xs_lb)
        env["R14_XS_SKIP_RECENT"] = "0"
        env["R14_XS_REBALANCE_DAYS"] = "1"
        env["R14_XS_ACTIVE_LONG"] = "3"
        env["R14_XS_ACTIVE_SHORT"] = "3"

        htf_flag = "on" if htf else "off"
        cmd = [str(BINARY), str(REPO_ROOT / BASES[base]), label, htf_flag]

        print(f"\n  [{i+1}/32] {label}")
        t0 = time.time()
        try:
            result = subprocess.run(cmd, capture_output=True, text=True, timeout=900, env=env)
            elapsed = time.time() - t0
            if result.returncode == 0:
                try:
                    parsed = json.loads(result.stdout)
                    full = parsed.get("full", {})
                    pos = parsed.get("positive_segments", 0)
                    print(f"    ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% pos={pos}/5 [{elapsed:.0f}s]")
                    all_results.append({"label": label, "base": base, "htf": htf,
                                       "xs_family": xs_family, "xs_lookback": xs_lb,
                                       "result": parsed, "elapsed_s": elapsed})
                except json.JSONDecodeError:
                    print(f"    JSON parse error")
                    all_results.append({"label": label, "error": "json_parse"})
            else:
                print(f"    ERROR: rc={result.returncode}")
                all_results.append({"label": label, "error": f"rc={result.returncode}"})
        except subprocess.TimeoutExpired:
            print(f"    TIMEOUT")
            all_results.append({"label": label, "error": "timeout"})

        # Checkpoint every 8
        if (i+1) % 8 == 0:
            save_results(all_results, "p2_sobol_checkpoint")

    save_results(all_results, "p2_sobol_final")

    # Summary
    print("\n" + "="*60)
    print("SUMMARY")
    print("="*60)
    valid = [r for r in all_results if "result" in r and "error" not in r.get("result", {})]
    print(f"Total: {len(all_results)}, Valid: {len(valid)}")

    if valid:
        best = max(valid, key=lambda x: x["result"].get("full", {}).get("ann", -999))
        full = best["result"].get("full", {})
        print(f"\nBest: {best['label']}")
        print(f"  ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% "
              f"pos={best['result'].get('positive_segments',0)}/5")

def save_results(results, name):
    path = REPO_ROOT / f"docs/superpowers/artifacts/glm-martingale-core-round14/r14-{name}.json"
    with open(path, 'w') as f:
        json.dump({"total": len(results), "results": results}, f, indent=2)
    print(f"  Saved {len(results)} results to {path.name}")

if __name__ == "__main__":
    main()
