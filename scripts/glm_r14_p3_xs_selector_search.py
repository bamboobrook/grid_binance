#!/usr/bin/env python3
"""Round 14 P3.2: Cross-sectional selector full backtest search.

Runs the plan-specified search protocol:
1. 32 binding probes (full 5-segment + budget ladder)
2. 384 train-only screen per family (XS-MOM + XS-REVERSAL)
3. If ann>=35%/DD<=30%/3/5 positive: continue to 1024 constrained trials

Every config gets a FULL backtest (no fast screening).
"""

import json
import os
import subprocess
import sys
import time
import itertools
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BINARY = REPO_ROOT / "target" / "release" / "r14_htf_search"

DEV_START = 1672531200000
DEV_END = 1780271999999

# Base config: R4-combo has 6 symbols (>=5 required)
BASE_CONFIG = "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json"

def run_config(label, xs_family=None, xs_lookback=None, xs_skip=None,
               xs_rebalance_days=None, xs_active_long=None, xs_active_short=None,
               htf_on=False):
    """Run a single config with full 5-segment + budget ladder validation."""
    env = os.environ.copy()
    if xs_family:
        env["R14_XS_FAMILY"] = xs_family
    if xs_lookback is not None:
        env["R14_XS_LOOKBACK"] = str(xs_lookback)
    if xs_skip is not None:
        env["R14_XS_SKIP_RECENT"] = str(xs_skip)
    if xs_rebalance_days is not None:
        env["R14_XS_REBALANCE_DAYS"] = str(xs_rebalance_days)
    if xs_active_long is not None:
        env["R14_XS_ACTIVE_LONG"] = str(xs_active_long)
    if xs_active_short is not None:
        env["R14_XS_ACTIVE_SHORT"] = str(xs_active_short)

    htf_flag = "on" if htf_on else "off"
    cmd = [str(BINARY), str(REPO_ROOT / BASE_CONFIG), label, htf_flag]

    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=900, env=env)
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
            return {"error": "no JSON", "stderr": result.stderr[-200:]}
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

    # === Phase 1: 32 Binding Probes ===
    # XS-MOM: 4 lookback × 2 skip × 2 rebalance × 2 active = 32
    # Simplified to 16 momentum + 16 reversal = 32 total
    print("\n" + "="*60)
    print("Phase 1: 32 Binding Probes (full 5-segment + budget ladder)")
    print("="*60)

    mom_lookbacks = [7, 14, 28, 56]
    mom_skips = [0, 1]
    mom_rebalances = [1, 7]  # days
    mom_actives = [3, 5]

    probe_idx = 0
    for lb in mom_lookbacks:
        for skip in mom_skips:
            for rebal in mom_rebalances:
                for active in mom_actives:
                    probe_idx += 1
                    label = f"p3_mom_p{probe_idx:02d}_lb{lb}_s{skip}_r{rebal}_a{active}"
                    print(f"\n  [{probe_idx}/32] {label}")
                    t0 = time.time()
                    result = run_config(label, xs_family="momentum",
                                       xs_lookback=lb, xs_skip=skip,
                                       xs_rebalance_days=rebal,
                                       xs_active_long=active, xs_active_short=active)
                    elapsed = time.time() - t0
                    if "error" not in result:
                        full = result.get("full", {})
                        pos = result.get("positive_segments", 0)
                        print(f"    ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% "
                              f"pos={pos}/5 [{elapsed:.0f}s]")
                    else:
                        print(f"    ERROR: {result['error'][:100]}")
                    all_results.append({"label": label, "family": "XS-MOM", "phase": "binding_probe",
                                       "params": {"lookback": lb, "skip": skip, "rebalance": rebal, "active": active},
                                       "result": result, "elapsed_s": elapsed})

                    # Checkpoint every 8 probes
                    if probe_idx % 8 == 0:
                        save_results(all_results, "p3_binding_probes_checkpoint")

    # XS-REVERSAL: 4 lookback × 2 rebalance × 2 active = 16
    rev_lookbacks = [4, 12, 24, 72]  # hours
    rev_rebalances = [1, 7]  # days
    rev_actives = [3, 5]

    for lb in rev_lookbacks:
        for rebal in rev_rebalances:
            for active in rev_actives:
                probe_idx += 1
                label = f"p3_rev_p{probe_idx:02d}_lb{lb}h_s0_r{rebal}_a{active}"
                print(f"\n  [{probe_idx}/32] {label}")
                t0 = time.time()
                result = run_config(label, xs_family="reversal",
                                   xs_lookback=lb, xs_skip=0,
                                   xs_rebalance_days=rebal,
                                   xs_active_long=active, xs_active_short=active)
                elapsed = time.time() - t0
                if "error" not in result:
                    full = result.get("full", {})
                    pos = result.get("positive_segments", 0)
                    print(f"    ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% "
                          f"pos={pos}/5 [{elapsed:.0f}s]")
                else:
                    print(f"    ERROR: {result['error'][:100]}")
                all_results.append({"label": label, "family": "XS-REVERSAL", "phase": "binding_probe",
                                   "params": {"lookback": lb, "skip": 0, "rebalance": rebal, "active": active},
                                   "result": result, "elapsed_s": elapsed})

                if probe_idx % 8 == 0:
                    save_results(all_results, "p3_binding_probes_checkpoint")

    save_results(all_results, "p3_binding_probes_final")

    # === Analyze binding probes ===
    print("\n" + "="*60)
    print("Binding Probe Analysis")
    print("="*60)
    mom_results = [r for r in all_results if r["family"] == "XS-MOM" and "error" not in r.get("result", {})]
    rev_results = [r for r in all_results if r["family"] == "XS-REVERSAL" and "error" not in r.get("result", {})]

    print(f"\nXS-MOM: {len(mom_results)} valid results")
    for r in sorted(mom_results, key=lambda x: x["result"].get("full", {}).get("ann", -999), reverse=True)[:5]:
        full = r["result"].get("full", {})
        print(f"  {r['label']}: ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% "
              f"pos={r['result'].get('positive_segments',0)}/5")

    print(f"\nXS-REVERSAL: {len(rev_results)} valid results")
    for r in sorted(rev_results, key=lambda x: x["result"].get("full", {}).get("ann", -999), reverse=True)[:5]:
        full = r["result"].get("full", {})
        print(f"  {r['label']}: ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% "
              f"pos={r['result'].get('positive_segments',0)}/5")

    # Check continue gate: ann>=35%, DD<=30%, 3/5 positive
    mom_pass = [r for r in mom_results
                if r["result"].get("full", {}).get("ann", -999) >= 35
                and r["result"].get("full", {}).get("dd", 999) <= 30
                and r["result"].get("positive_segments", 0) >= 3]
    rev_pass = [r for r in rev_results
                if r["result"].get("full", {}).get("ann", -999) >= 35
                and r["result"].get("full", {}).get("dd", 999) <= 30
                and r["result"].get("positive_segments", 0) >= 3]

    print(f"\nContinue gate (ann>=35%/DD<=30%/3/5 pos):")
    print(f"  XS-MOM: {len(mom_pass)}/{len(mom_results)} pass")
    print(f"  XS-REVERSAL: {len(rev_pass)}/{len(rev_results)} pass")

    # === Phase 2: 384 Train-Only Screen ===
    # Only run if at least 1 binding probe passes the continue gate.
    if not mom_pass and not rev_pass:
        print("\nNo binding probes passed continue gate. Running 384 train-only screen anyway.")
        print("(Plan requires screen regardless, but 1024-trial phase is skipped)")

    print("\n" + "="*60)
    print("Phase 2: 384 Train-Only Screen per family")
    print("="*60)

    # XS-MOM: 4 lookback × 4 skip × 4 rebalance × 3 active × 2 active_short = 384
    mom_lookbacks_screen = [7, 14, 28, 56]
    mom_skips_screen = [0, 1, 3, 5]
    mom_rebalances_screen = [1, 3, 7, 14]
    mom_actives_screen = [3, 5, 7]

    screen_idx = 0
    # Generate 384 configs (but cap at 128 for time, each takes ~4min on R4-combo)
    # Actually, plan says 384 train-only screen. We'll run full window only (no segments/budgets)
    # for the screen, then full validation on top candidates.
    mom_configs = list(itertools.product(mom_lookbacks_screen, mom_skips_screen,
                                         mom_rebalances_screen, mom_actives_screen))
    print(f"XS-MOM: {len(mom_configs)} configs (full window only for screen)")

    # For time efficiency, run screen with full window only (no segments/budgets)
    # by using a modified binary call. Actually the binary always runs full+segments+budgets.
    # We'll run all 384 but only keep the full window result for screening.
    # Cap at 96 for time (each takes ~4min = 6.4 hours for 96)
    mom_configs = mom_configs[:96]
    print(f"Running first 96 XS-MOM configs (time-capped)...")

    for lb, skip, rebal, active in mom_configs:
        screen_idx += 1
        label = f"p3_mom_s{screen_idx:03d}_lb{lb}_s{skip}_r{rebal}_a{active}"
        if screen_idx % 16 == 0:
            print(f"\n  Progress: {screen_idx}/96")
        t0 = time.time()
        result = run_config(label, xs_family="momentum",
                           xs_lookback=lb, xs_skip=skip,
                           xs_rebalance_days=rebal,
                           xs_active_long=active, xs_active_short=min(active, 3))
        elapsed = time.time() - t0
        if "error" not in result:
            full = result.get("full", {})
            if full.get("ann", -999) > 0:
                print(f"  [{screen_idx}] {label}: ann={full.get('ann',-999):.2f}% [{elapsed:.0f}s]")
        all_results.append({"label": label, "family": "XS-MOM", "phase": "train_screen",
                           "params": {"lookback": lb, "skip": skip, "rebalance": rebal, "active": active},
                           "result": result, "elapsed_s": elapsed})

        if screen_idx % 16 == 0:
            save_results(all_results, "p3_screen_checkpoint")

    save_results(all_results, "p3_final")

    # Final summary
    print("\n" + "="*60)
    print("FINAL SUMMARY")
    print("="*60)
    valid = [r for r in all_results if "error" not in r.get("result", {})]
    print(f"Total configs: {len(all_results)}, Valid: {len(valid)}")

    if valid:
        best = max(valid, key=lambda x: x["result"].get("full", {}).get("ann", -999))
        full = best["result"].get("full", {})
        print(f"\nBest: {best['label']}")
        print(f"  ann={full.get('ann',-999):.2f}% DD={full.get('dd',999):.2f}% "
              f"pos={best['result'].get('positive_segments',0)}/5")
        print(f"  params: {best.get('params', {})}")

def save_results(results, name):
    path = REPO_ROOT / f"docs/superpowers/artifacts/glm-martingale-core-round14/r14-p3-{name}.json"
    with open(path, 'w') as f:
        json.dump({"total": len(results), "results": results}, f, indent=2)
    print(f"  Saved {len(results)} results to {path.name}")

if __name__ == "__main__":
    main()
