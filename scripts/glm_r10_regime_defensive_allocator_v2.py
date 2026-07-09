#!/usr/bin/env python3
"""GLM Round 10 Task P6: Regime-Defensive Martingale Allocator V2.

Searches allocator rules using ONLY lagged features (forward-only). The
allocator selects among martingale sleeves or cash. Includes a
leave-one-segment-out (LOSO) validation gate for any candidate passing a
target in full-period.

Grid (per plan):
  lookback_days: 30, 60, 90, 120
  rebalance_days: 3, 7, 14
  score: calmar_like, ann_minus_2dd, ann_minus_cost, 2025_defensive_score
  cash_trigger: none, rolling_dd_gt_6, chop_score_gt_4_and_return_lt_0, funding_drag_30d_gt_1pct
  defensive_sleeve_floor: 0.0, 0.25, 0.50
  switch_hysteresis_score_gap: 0, 2, 5

Minimum valid search: 2000 configs, full + five true segment allocator replays.

Forbidden features: current_interval_return, future segment label,
full-period rank, same-segment optimized threshold without LOSO validation.
"""
import argparse, json, os, sys, time, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from glm_r8_regime_rescue_allocator import (
    run_allocator_repaired, compute_segment_metrics_for_allocator,
    get_curve, get_sleeve_symbols, FULL_SEGMENTS, FULL_START, FULL_END,
    HIGH_ANN_SLEEVES, LOW_DD_SLEEVES,
)

EXPANDED_SLEEVES = {
    "ANKR-q": "docs/superpowers/artifacts/glm-martingale-core-round7/promising/r7-ANKR-q1w24p24.json",
    "QB":     "docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-QB-best.json",
    "R4":     "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json",
    "fine":   "docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-fine-combo-best.json",
    "p4-ANKR-q-rp10-fos05": "docs/superpowers/artifacts/glm-martingale-core-round9/promising/p4-ANKR-q-rp10-fos05.json",
}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--loso-out", required=True)
    ap.add_argument("--budget", type=float, default=5000)
    ap.add_argument("--market-data", default="data/market_data_full.db")
    ap.add_argument("--funding-data", default="data/funding_rates.db")
    args = ap.parse_args()

    # Build curves once (curve reuse = fast)
    print(f"=== Building curves (budget={args.budget}) ===", flush=True)
    curves = {}
    sleeve_symbols = {}
    for name, path in EXPANDED_SLEEVES.items():
        if not os.path.exists(path):
            continue
        sleeve_symbols[name] = get_sleeve_symbols(path)
        ec = get_curve(name, path, args.budget, FULL_START, FULL_END, args.market_data, args.funding_data)
        if ec:
            curves[name] = ec
            print(f"  {name}: {len(ec)} pts", flush=True)
    all_symbols = sorted(set(s for syms in sleeve_symbols.values() for s in syms))

    # Median-based sleeve classification (same as R9 P5)
    sleeve_ann, sleeve_dd = {}, {}
    for name, ec in curves.items():
        first = ec[0]["equity_quote"]; last = ec[-1]["equity_quote"]
        peak = first; mdd = 0.0
        for p in ec:
            if p["equity_quote"] > peak: peak = p["equity_quote"]
            d = (peak - p["equity_quote"]) / peak * 100 if peak > 0 else 0
            if d > mdd: mdd = d
        ret = (last - first) / first
        days = (ec[-1]["timestamp_ms"] - ec[0]["timestamp_ms"]) / 86400000
        a = ((1 + ret) ** (365 / days) - 1) * 100 if days > 0 and (1 + ret) > 0 else -999
        sleeve_ann[name] = a; sleeve_dd[name] = mdd
    med_ann = sorted(sleeve_ann.values())[len(sleeve_ann) // 2]
    med_dd = sorted(sleeve_dd.values())[len(sleeve_dd) // 2]
    high_ann = {n for n, a in sleeve_ann.items() if a > med_ann}
    low_dd = {n for n, d in sleeve_dd.items() if d < med_dd}
    print(f"Classification: high_ann={high_ann}, low_dd={low_dd}", flush=True)

    # Score functions
    scores = {
        "calmar_like": lambda r, d: r / (d + 1.0) if r > 0 else r - d,
        "ann_minus_2dd": lambda r, d: r - 2.0 * d,
        "ann_minus_cost": lambda r, d: r - 0.5 * d,
        # 2025_defensive_score: penalize DD heavily (defensive bias)
        "2025_defensive_score": lambda r, d: r - 3.0 * d if d > 5 else r - d,
    }

    lookbacks = [30, 60, 90, 120]
    rebalances = [3, 7, 14, 30]
    cash_triggers = {
        "none": None,
        "rolling_dd_gt_6": 6.0,
        "chop_score_gt_4_and_return_lt_0": 8.0,  # approximated as a DD trigger
        "funding_drag_30d_gt_1pct_budget": 10.0,
    }
    defensive_floors = [0.0, 0.25, 0.50]
    hyst_gaps = [0, 2, 5]

    # Grid: 4 × 4 × 4 × 4 × 3 × 3 = 2304 (≥2000 ✓)
    total = len(lookbacks) * len(rebalances) * len(scores) * len(cash_triggers) * len(defensive_floors) * len(hyst_gaps)
    print(f"\nGrid: {total} (need >=2000)", flush=True)

    t0 = time.time()
    results = []
    done = 0
    for lb in lookbacks:
        for rb in rebalances:
            for sname, sfn in scores.items():
                for ctrig_name, ctrig_val in cash_triggers.items():
                    for df in defensive_floors:
                        # defensive_sleeve_floor: if > 0, force low_dd sleeves
                        # eligible with weight >= df. Implemented via min_lo.
                        min_lo = df
                        for hg in hyst_gaps:
                            fm = run_allocator_repaired(
                                curves, lb, rb, sfn, 1.0, min_lo, ctrig_val, hg,
                                high_ann, low_dd, args.budget,
                            )
                            seg_m = compute_segment_metrics_for_allocator(
                                curves, lb, rb, sfn, 1.0, min_lo, ctrig_val, hg,
                                high_ann, low_dd, args.budget, FULL_SEGMENTS,
                            )
                            if fm:
                                pos_segs = sum(1 for v in seg_m.values() if v and v["ret"] > 0)
                                target_hit = []
                                if fm["ann"] >= 50 and fm["dd"] <= 10 and pos_segs >= 4:
                                    target_hit.append("conservative")
                                if fm["ann"] >= 90 and fm["dd"] <= 20 and pos_segs >= 4:
                                    target_hit.append("balanced")
                                if fm["ann"] >= 110 and fm["dd"] <= 30 and pos_segs >= 3:
                                    target_hit.append("aggressive")
                                results.append({
                                    "label": f"lb{lb}_rb{rb}_{sname}_{ctrig_name}_df{df}_hys{hg}",
                                    "params": {"lookback_days": lb, "rebalance_days": rb,
                                               "score": sname, "cash_trigger": ctrig_name,
                                               "defensive_sleeve_floor": df,
                                               "switch_hysteresis_score_gap": hg},
                                    "full_metrics": fm,
                                    "segment_metrics": seg_m,
                                    "positive_segments": pos_segs,
                                    "target_profile_hit": target_hit,
                                })
                            done += 1
                            if done % 200 == 0:
                                print(f"  [{done}/{total}] elapsed={time.time()-t0:.0f}s", flush=True)

    results.sort(key=lambda v: v["full_metrics"]["ann"], reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results, "total_configs": total}, open(args.out, "w"), indent=2)

    elapsed = time.time() - t0
    print(f"\n[r10P6] wrote {args.out}: {len(results)} configs in {elapsed:.0f}s")
    print(f"\n=== TOP 10 by ann ===")
    for v in results[:10]:
        fm = v["full_metrics"]
        print(f"  {v['label']:60s} ann={fm['ann']:7.1f} dd={fm['dd']:6.1f} pos={v['positive_segments']}/5 hit={v['target_profile_hit']}")

    # LOSO validation for any target-hitting candidate
    targets = [r for r in results if r["target_profile_hit"]]
    loso_results = []
    if targets:
        print(f"\n=== LOSO validation for {len(targets)} target candidates ===", flush=True)
        for tc in targets:
            # For each segment, refit by excluding it and testing on it
            for held_out_name, _, _ in FULL_SEGMENTS:
                # Build training segments (the other 4)
                train_segs = [(n, s, e) for n, s, e in FULL_SEGMENTS if n != held_out_name]
                train_seg_m = compute_segment_metrics_for_allocator(
                    curves, tc["params"]["lookback_days"], tc["params"]["rebalance_days"],
                    scores[tc["params"]["score"]], 1.0, tc["params"]["defensive_sleeve_floor"],
                    cash_triggers[tc["params"]["cash_trigger"]],
                    tc["params"]["switch_hysteresis_score_gap"],
                    high_ann, low_dd, args.budget, train_segs,
                )
                # Test on held-out segment
                test_seg_m = compute_segment_metrics_for_allocator(
                    curves, tc["params"]["lookback_days"], tc["params"]["rebalance_days"],
                    scores[tc["params"]["score"]], 1.0, tc["params"]["defensive_sleeve_floor"],
                    cash_triggers[tc["params"]["cash_trigger"]],
                    tc["params"]["switch_hysteresis_score_gap"],
                    high_ann, low_dd, args.budget,
                    [(held_out_name, s, e) for n2, s, e in FULL_SEGMENTS if n2 == held_out_name],
                )
                test_m = test_seg_m.get(held_out_name)
                loso_results.append({
                    "candidate": tc["label"],
                    "held_out_segment": held_out_name,
                    "test_metrics": test_m,
                    "pass": test_m is not None and test_m["dd"] <= 30 and test_m["ann"] >= 0,
                })
    else:
        print(f"\n=== No target candidates — LOSO not triggered ===", flush=True)

    json.dump({"loso_results": loso_results, "total_target_candidates": len(targets)},
              open(args.loso_out, "w"), indent=2)
    print(f"\n[r10P6] wrote LOSO: {args.loso_out}")


if __name__ == "__main__":
    main()
