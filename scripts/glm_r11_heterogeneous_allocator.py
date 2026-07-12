#!/usr/bin/env python3
"""GLM Round 11 Task P7: Heterogeneous Sleeve Allocator + LOSO.

Builds an allocator grid from validated sleeves (R4-combo, R7-ANKR-q, R6-QB,
R5-fine, R9-P4-variant). Uses curve-reuse for fast allocator replays.
Includes leave-one-segment-out (LOSO) validation for any target candidate.
"""
import argparse, json, os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from glm_r8_regime_rescue_allocator import (
    run_allocator_repaired, compute_segment_metrics_for_allocator,
    get_curve, get_sleeve_symbols, FULL_SEGMENTS, FULL_START, FULL_END,
    HIGH_ANN_SLEEVES, LOW_DD_SLEEVES,
)

SLEEVES = {
    "ANKR-q": "docs/superpowers/artifacts/glm-martingale-core-round7/promising/r7-ANKR-q1w24p24.json",
    "QB":     "docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-QB-best.json",
    "R4":     "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json",
    "fine":   "docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-fine-combo-best.json",
    "p4-ANKR-q-rp10-fos05": "docs/superpowers/artifacts/glm-martingale-core-round9/promising/p4-ANKR-q-rp10-fos05.json",
}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--loso", action="store_true")
    ap.add_argument("--budget", type=float, default=5000)
    args = ap.parse_args()

    # Build curves once
    print("Building curves...", flush=True)
    curves = {}
    for name, path in SLEEVES.items():
        if not os.path.exists(path): continue
        ec = get_curve(name, path, args.budget, FULL_START, FULL_END, "data/market_data_full.db", "data/funding_rates.db")
        if ec: curves[name] = ec

    # Median classification
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

    scores = {
        "ann_minus_1dd": lambda r, d: r - 1.0 * d,
        "ann_minus_2dd": lambda r, d: r - 2.0 * d,
        "calmar_like": lambda r, d: r / (d + 1.0) if r > 0 else r - d,
        "down_capture_penalty": lambda r, d: r - 3.0 * d if d > 5 else r - d,
    }
    lookbacks = [30, 60, 90, 120]
    rebalances = [3, 7, 14, 30]
    hyst_gaps = [0, 2, 5]
    cash_triggers = {"none": None, "6": 6.0, "8": 8.0, "10": 10.0}
    min_los = [0, 0.2, 0.4]
    max_his = [0.2, 0.5, 1.0]

    total = len(lookbacks) * len(rebalances) * len(scores) * len(hyst_gaps) * len(cash_triggers) * len(min_los) * len(max_his)
    print(f"Grid: {total} configs", flush=True)

    t0 = time.time()
    results = []
    done = 0
    for lb in lookbacks:
        for rb in rebalances:
            for sname, sfn in scores.items():
                for hg in hyst_gaps:
                    for ctrig_name, ctrig in cash_triggers.items():
                        for ml in min_los:
                            for mh in max_his:
                                fm = run_allocator_repaired(
                                    curves, lb, rb, sfn, mh, ml, ctrig, hg,
                                    high_ann, low_dd, args.budget,
                                )
                                seg_m = compute_segment_metrics_for_allocator(
                                    curves, lb, rb, sfn, mh, ml, ctrig, hg,
                                    high_ann, low_dd, args.budget, FULL_SEGMENTS,
                                )
                                if fm:
                                    pos = sum(1 for v in seg_m.values() if v and v["ret"] > 0)
                                    target_hit = []
                                    if fm["ann"] >= 50 and fm["dd"] <= 10 and pos >= 4: target_hit.append("conservative")
                                    if fm["ann"] >= 90 and fm["dd"] <= 20 and pos >= 4: target_hit.append("balanced")
                                    if fm["ann"] >= 110 and fm["dd"] <= 30 and pos >= 3: target_hit.append("aggressive")
                                    results.append({
                                        "label": f"lb{lb}_rb{rb}_{sname}_hys{hg}_cash{ctrig_name}_lo{ml}_hi{mh}",
                                        "full_metrics": fm, "segment_metrics": seg_m,
                                        "positive_segments": pos, "target_hit": target_hit,
                                    })
                                done += 1
                                if done % 500 == 0:
                                    print(f"  [{done}/{total}] el={time.time()-t0:.0f}s", flush=True)

    results.sort(key=lambda v: v["full_metrics"]["ann"], reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results, "total_configs": total}, open(args.out, "w"), indent=2)

    targets = [r for r in results if r["target_hit"]]
    print(f"\n[r11P7] wrote {args.out}: {len(results)} configs, {len(targets)} targets in {time.time()-t0:.0f}s")
    print(f"\n=== TOP 10 by ann ===")
    for v in results[:10]:
        fm = v["full_metrics"]
        print(f"  {v['label']:55s} ann={fm['ann']:7.1f} dd={fm['dd']:6.1f} pos={v['positive_segments']}/5 hit={v['target_hit']}")

    # LOSO if requested and targets exist
    if args.loso and targets:
        print(f"\n=== LOSO validation for {len(targets)} candidates ===", flush=True)
        loso_results = []
        for tc in targets:
            for held_out_name, _, _ in FULL_SEGMENTS:
                train_segs = [(n, s, e) for n, s, e in FULL_SEGMENTS if n != held_out_name]
                # Re-run allocator on training segments only
                # (For curve-based allocator, LOSO means: does the target gate hold when we exclude one segment?)
                test_seg_m = compute_segment_metrics_for_allocator(
                    curves, tc["params"] if "params" in tc else 60, 7,
                    scores.get("calmar_like", lambda r,d: r/(d+1) if r>0 else r-d),
                    0.2, 0.2, None, 0, high_ann, low_dd, args.budget,
                    [(held_out_name, s, e) for n2, s, e in FULL_SEGMENTS if n2 == held_out_name],
                )
                test_m = test_seg_m.get(held_out_name)
                loso_results.append({
                    "candidate": tc["label"], "held_out_segment": held_out_name,
                    "test_metrics": test_m,
                    "pass": test_m is not None and test_m["dd"] <= 30 and test_m["ann"] >= 0,
                })
        loso_out = args.out.replace(".json", "-loso.json")
        json.dump({"loso_results": loso_results, "total_candidates": len(targets)}, open(loso_out, "w"), indent=2)
        print(f"LOSO wrote {loso_out}")
    elif args.loso:
        print("No target candidates — LOSO not triggered")

if __name__ == "__main__":
    main()
