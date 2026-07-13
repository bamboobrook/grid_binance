#!/usr/bin/env python3
"""Round 14 P3: Constrained trials on gate-passing configs.

Runs neighborhood search around the 4 configs that passed the continue gate:
1. p46: XS-REVERSAL lb72h s0 r1 a5 (ann=38.02%/DD=26.24%/4/5)
2. p01: XS-MOM lb7 s0 r1 a3 (ann=35.18%/DD=16.75%/4/5)
3. p40: XS-REVERSAL lb12h s0 r7 a5 (ann=33.27%/DD=18.35%/4/5)
4. p42: XS-REVERSAL lb24h s0 r1 a5 (ann=33.21%/DD=18.34%/4/5)

Each neighborhood config gets FULL 5-segment + budget ladder validation.
"""

import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BINARY = REPO_ROOT / "target" / "release" / "r14_htf_search"
BASE_CONFIG = REPO_ROOT / "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json"

def run_config(label, xs_family, xs_lookback, xs_skip, xs_rebalance_days,
               xs_active_long, xs_active_short, htf_on=False):
    env = os.environ.copy()
    env["R14_XS_FAMILY"] = xs_family
    env["R14_XS_LOOKBACK"] = str(xs_lookback)
    env["R14_XS_SKIP_RECENT"] = str(xs_skip)
    env["R14_XS_REBALANCE_DAYS"] = str(xs_rebalance_days)
    env["R14_XS_ACTIVE_LONG"] = str(xs_active_long)
    env["R14_XS_ACTIVE_SHORT"] = str(xs_active_short)

    htf_flag = "on" if htf_on else "off"
    cmd = [str(BINARY), str(BASE_CONFIG), label, htf_flag]
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=900, env=env)
        if result.returncode != 0:
            return {"error": f"rc={result.returncode}"}
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
        subprocess.run(["cargo", "build", "--release", "--bin", "r14_htf_search"],
                      cwd=REPO_ROOT, check=True)

    all_results = []

    # Gate-passing configs and their neighborhoods
    # For each, vary: lookback ±20%, skip ±1, rebalance ±2d, active ±1
    base_configs = [
        # (family, lookback, skip, rebalance, active_long, active_short, label)
        ("reversal", 72, 0, 1, 5, 5, "p46_rev_lb72"),      # ann=38.02% best
        ("momentum", 7, 0, 1, 3, 3, "p01_mom_lb7"),         # ann=35.18% best Calmar
        ("reversal", 12, 0, 7, 5, 5, "p40_rev_lb12"),       # ann=33.27% good Calmar
        ("reversal", 24, 0, 1, 5, 5, "p42_rev_lb24"),       # ann=33.21% good Calmar
    ]

    print("="*60)
    print("Constrained Trials: Neighborhood search on 4 gate-passing configs")
    print("="*60)

    trial_idx = 0
    for family, lb, skip, rebal, a_long, a_short, base_label in base_configs:
        print(f"\n--- Base: {base_label} ({family} lb={lb} skip={skip} rebal={rebal} active={a_long}) ---")

        # Center config
        trial_idx += 1
        label = f"ct_{trial_idx:03d}_{base_label}_center"
        print(f"  [{trial_idx}] {label}")
        t0 = time.time()
        result = run_config(label, family, lb, skip, rebal, a_long, a_short)
        elapsed = time.time() - t0
        if "error" not in result:
            full = result.get("full", {})
            print(f"    ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% pos={result.get('positive_segments',0)}/5 [{elapsed:.0f}s]")
        all_results.append({"label": label, "base": base_label, "phase": "center", "result": result})

        # Neighborhood: vary lookback
        lb_variants = []
        if family == "reversal":
            # Hours: ±20%
            lb_variants = [max(2, int(lb * 0.8)), int(lb * 1.2)]
        else:
            # Days: ±1 day
            lb_variants = [max(3, lb - 1), lb + 1]

        for new_lb in lb_variants:
            trial_idx += 1
            label = f"ct_{trial_idx:03d}_{base_label}_lb{new_lb}"
            print(f"  [{trial_idx}] {label}")
            result = run_config(label, family, new_lb, skip, rebal, a_long, a_short)
            if "error" not in result:
                full = result.get("full", {})
                print(f"    ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% pos={result.get('positive_segments',0)}/5")
            all_results.append({"label": label, "base": base_label, "phase": "vary_lb", "new_lb": new_lb, "result": result})

        # Neighborhood: vary skip
        for new_skip in [0, 1]:
            if new_skip == skip:
                continue
            trial_idx += 1
            label = f"ct_{trial_idx:03d}_{base_label}_skip{new_skip}"
            print(f"  [{trial_idx}] {label}")
            result = run_config(label, family, lb, new_skip, rebal, a_long, a_short)
            if "error" not in result:
                full = result.get("full", {})
                print(f"    ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% pos={result.get('positive_segments',0)}/5")
            all_results.append({"label": label, "base": base_label, "phase": "vary_skip", "new_skip": new_skip, "result": result})

        # Neighborhood: vary rebalance
        for new_rebal in [1, 3, 7]:
            if new_rebal == rebal:
                continue
            trial_idx += 1
            label = f"ct_{trial_idx:03d}_{base_label}_rebal{new_rebal}"
            print(f"  [{trial_idx}] {label}")
            result = run_config(label, family, lb, skip, new_rebal, a_long, a_short)
            if "error" not in result:
                full = result.get("full", {})
                print(f"    ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% pos={result.get('positive_segments',0)}/5")
            all_results.append({"label": label, "base": base_label, "phase": "vary_rebal", "new_rebal": new_rebal, "result": result})

        # Neighborhood: vary active
        for new_active in [3, 5, 7]:
            if new_active == a_long:
                continue
            trial_idx += 1
            label = f"ct_{trial_idx:03d}_{base_label}_act{new_active}"
            print(f"  [{trial_idx}] {label}")
            result = run_config(label, family, lb, skip, rebal, new_active, min(new_active, 5))
            if "error" not in result:
                full = result.get("full", {})
                print(f"    ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% pos={result.get('positive_segments',0)}/5")
            all_results.append({"label": label, "base": base_label, "phase": "vary_active", "new_active": new_active, "result": result})

        # Checkpoint after each base
        save_results(all_results, "constrained_trials_checkpoint")

    save_results(all_results, "constrained_trials_final")

    # Summary
    print("\n" + "="*60)
    print("CONSTRAINED TRIALS SUMMARY")
    print("="*60)
    valid = [r for r in all_results if "error" not in r.get("result", {})]
    print(f"Total: {len(all_results)}, Valid: {len(valid)}")

    # Top by ann
    top = sorted(valid, key=lambda x: x["result"].get("full", {}).get("ann", -999), reverse=True)[:10]
    print("\nTop 10 by ann:")
    for r in top:
        full = r["result"].get("full", {})
        pos = r["result"].get("positive_segments", 0)
        print(f"  {r['label']}: ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% pos={pos}/5")

    # Best Calmar (ann/DD)
    calmar_sorted = sorted(valid, key=lambda x: x["result"].get("full", {}).get("ann", -999) / max(x["result"].get("full", {}).get("dd", 1), 0.1), reverse=True)
    print("\nTop 5 by Calmar:")
    for r in calmar_sorted[:5]:
        full = r["result"].get("full", {})
        pos = r["result"].get("positive_segments", 0)
        calmar = full.get("ann", -999) / max(full.get("dd", 1), 0.1)
        print(f"  {r['label']}: ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% pos={pos}/5 calmar={calmar:.2f}")

def save_results(results, name):
    path = REPO_ROOT / f"docs/superpowers/artifacts/glm-martingale-core-round14/r14-{name}.json"
    with open(path, 'w') as f:
        json.dump({"total": len(results), "results": results}, f, indent=2)
    print(f"  Saved {len(results)} results to {path.name}")

if __name__ == "__main__":
    main()
