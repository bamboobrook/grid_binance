#!/usr/bin/env python3
"""GLM Round 11 Task P2: R9 allocator research-curve replay.

This script rebuilds independently simulated sleeve equity curves and feeds
them to the Python curve allocator. It does not start trading-engine, exercise
the DB-backed reconcile path, or preserve event-level positions across sleeve
switches. Its output is research replay evidence only, never production parity.
"""
import argparse, json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from glm_r8_regime_rescue_allocator import (
    run_allocator_repaired, get_curve, get_sleeve_symbols,
    FULL_SEGMENTS, FULL_START, FULL_END, compute_segment_metrics_for_allocator,
    HIGH_ANN_SLEEVES, LOW_DD_SLEEVES,
)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--budget", type=float, default=5000)
    ap.add_argument("--market-data", default="data/market_data_full.db")
    ap.add_argument("--funding-data", default="data/funding_rates.db")
    args = ap.parse_args()

    # Load R9 winner
    winner_path = "docs/superpowers/artifacts/glm-martingale-core-round9/promising/r9-P5-winner.json"
    winner = json.load(open(winner_path))
    sleeves = winner["sleeves"]
    params = winner["allocator_params"]

    # Build curves
    print("Building curves...", flush=True)
    curves = {}
    for name, path in sleeves.items():
        if not os.path.exists(path): continue
        ec = get_curve(name, path, args.budget, FULL_START, FULL_END, args.market_data, args.funding_data)
        if ec: curves[name] = ec

    # Median classification (same as R9 P5)
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

    sfn = lambda r, d: r / (d + 1.0) if r > 0 else r - d  # calmar_like

    # Authoritative metrics
    full_metrics = run_allocator_repaired(
        curves, params["lookback_days"], params["rebalance_days"], sfn,
        params["max_high_ann_weight"], params["min_low_dd_weight"],
        params["cash_trigger_rolling_dd_pct"], params["switch_hysteresis_score_gap"],
        high_ann, low_dd, args.budget,
    )
    seg_m = compute_segment_metrics_for_allocator(
        curves, params["lookback_days"], params["rebalance_days"], sfn,
        params["max_high_ann_weight"], params["min_low_dd_weight"],
        params["cash_trigger_rolling_dd_pct"], params["switch_hysteresis_score_gap"],
        high_ann, low_dd, args.budget, FULL_SEGMENTS,
    )

    r9_ann = winner["metrics"]["annualized_return_pct"]
    r9_dd = winner["metrics"]["max_drawdown_pct"]
    ann_diff = abs(full_metrics["ann"] - r9_ann)
    dd_diff = abs(full_metrics["dd"] - r9_dd)
    tol_pass = ann_diff <= 0.2 and dd_diff <= 0.2
    seg_present = sum(1 for v in seg_m.values() if v is not None)

    # A file on disk is useful provenance, but is not production execution
    # evidence. Keep this field only to explain the historical overclaim.
    p1_evidence = os.path.exists("docs/superpowers/artifacts/glm-martingale-core-round11/r11-live-allocator-production-wiring.json")

    research_parity_pass = tol_pass and seg_present == 5
    print("UNVERIFIED decision trace (the script emits no rebalance decisions)")
    print(f"Full metrics: ann={full_metrics['ann']:.4f} dd={full_metrics['dd']:.4f} vs R9 ann={r9_ann} dd={r9_dd}")
    print(f"  ann_diff={ann_diff:.4f}, dd_diff={dd_diff:.4f}")
    print(f"PASS full_metrics_tolerance" if tol_pass else "FAIL full_metrics_tolerance")
    print(f"PASS segment_metrics_present {seg_present}/5" if seg_present == 5 else "FAIL")
    print(f"P1 evidence exists: {p1_evidence}")

    out = {
        "candidate": "r9-P5-winner",
        "parity_scope": "python_research_curve_replay_only",
        "production_live_ready_after_p1": False,
        "production_parity_tested": False,
        "db_backed_reconcile_tested": False,
        "event_level_position_continuity_tested": False,
        "p1_evidence_file_present": p1_evidence,
        "annualized_return_pct": full_metrics["ann"],
        "max_drawdown_pct": full_metrics["dd"],
        "total_return_pct": full_metrics["ret"],
        "r9_recorded_ann": r9_ann,
        "r9_recorded_dd": r9_dd,
        "ann_abs_diff": round(ann_diff, 4),
        "dd_abs_diff": round(dd_diff, 4),
        "forward_only_decisions_pass": None,
        "forward_only_decisions_verified": False,
        "forward_only_note": "run_allocator_repaired is intended to be forward-only, but this script does not emit or validate a decision trace",
        "full_metrics_tolerance_pass": tol_pass,
        "segment_metrics_present_pass": seg_present == 5,
        "segment_metrics": seg_m,
        "research_replay_checks_pass": research_parity_pass,
        "all_checks_pass": research_parity_pass,
    }
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump(out, open(args.out, "w"), indent=2)
    print(f"\nWrote {args.out}")
    print(f"research_replay_checks_pass: {out['research_replay_checks_pass']}")
    print("production_live_ready_after_p1: False (not tested by this script)")


if __name__ == "__main__":
    main()
