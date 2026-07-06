#!/usr/bin/env python3
"""GLM Round 7 Task G: Dynamic cost/DD risk-budget blend.
Builds equity curves for all candidates, then runs lagged allocator.
"""
import argparse, json, os, subprocess, sys, time, copy, math
from collections import defaultdict
from datetime import datetime, timezone
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999

CANDIDATES = {
    "R3-lowdd": "docs/superpowers/artifacts/glm-martingale-core-round3/promising/r3-P1-best-cd11.json",
    "R4-combo": "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json",
    "R5-fine": "docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-fine-combo-best.json",
    "R5-ANKR": "docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-G-best-ANKRUSDT.json",
    "R6-XRPQ": "docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-C-best-quarantine.json",
    "R6-QB": "docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-QB-best.json",
}

def get_curve(name, config_path, budget=5000):
    """Get equity curve for a candidate."""
    cmd = [REPLAY, "--config", config_path, "--budget", str(budget),
           "--start-ms", str(FULL_START), "--end-ms", str(FULL_END),
           "--market-data", MARKET_DB, "--funding-data", FUNDING_DB,
           "--profile", "aggressive", "--portfolio-id", f"r7g_{name}",
           "--exchange-min-notional", "5", "--equity-curve-points", "5000"]
    pr = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    if pr.returncode != 0: return None
    r = json.loads(pr.stdout)
    return r.get("equity_curve", [])

def build_curves():
    curves = {}
    for name, path in CANDIDATES.items():
        if not os.path.exists(path):
            print(f"  {name}: MISSING"); continue
        ec = get_curve(name, path)
        if ec:
            curves[name] = ec
            # Convert to cum_pnl series (relative to first point)
            first = ec[0]["equity_quote"]
            for p in ec:
                p["cum_pnl"] = p["equity_quote"] - first
            print(f"  {name}: {len(ec)} points")
    return curves

def run_allocator(curves, lookback_points, rebalance_points, max_ankr, cash_dd_trigger, score_fn):
    """Run lagged dynamic allocator across curves.
    Returns the merged equity curve (budget-based).
    """
    # Align all curves by timestamp
    all_ts = sorted(set(p["timestamp_ms"] for c in curves.values() for p in c))
    ts_to_idx = {ts: i for i, ts in enumerate(all_ts)}

    # Build per-candidate cum_pnl at each timestamp
    pnl_by_cand = {}
    for name, ec in curves.items():
        pnl_map = {p["timestamp_ms"]: p["cum_pnl"] for p in ec}
        pnl_by_cand[name] = pnl_map

    budget = 5000.0
    active = "R4-combo"  # Start with R4 (lowest DD)
    allocation = {name: 0.0 for name in curves}
    allocation[active] = 1.0
    last_rebalance = 0
    merged_pnl = 0.0
    peak = budget
    max_dd = 0.0
    equity_series = []

    for i, ts in enumerate(all_ts):
        if i > 0 and i - last_rebalance >= rebalance_points:
            # Rebalance: compute lagged scores
            lookback_start = max(0, i - lookback_points)
            scores = {}
            for name in curves:
                pnl_now = pnl_by_cand[name].get(ts, 0)
                pnl_past = None
                # Find the point lookback_points ago
                past_ts = all_ts[lookback_start] if lookback_start < len(all_ts) else all_ts[0]
                pnl_past = pnl_by_cand[name].get(past_ts, 0)
                ret = pnl_now - pnl_past
                # Compute DD in lookback
                lookback_pnls = [pnl_by_cand[name].get(all_ts[j], 0) for j in range(lookback_start, i+1)]
                if lookback_pnls:
                    lb_peak = max(lookback_pnls)
                    lb_trough = min(lookback_pnls)
                    lb_dd = (lb_peak - lb_trough) / (budget + lb_peak) * 100 if (budget + lb_peak) > 0 else 0
                else:
                    lb_dd = 0
                scores[name] = score_fn(ret, lb_dd)

            # Pick best
            best = max(scores, key=scores.get)
            if scores[best] <= 0:
                active = "cash"  # Switch to cash if no positive score
            elif best == "R5-ANKR" and allocation.get("R5-ANKR", 0) >= max_ankr:
                active = best
            else:
                active = best

            # Check cash DD trigger
            current_eq = budget + merged_pnl
            dd = (peak - current_eq) / peak * 100 if peak > 0 else 0
            if cash_dd_trigger and dd > cash_dd_trigger:
                active = "cash"

            last_rebalance = i

        # Apply allocation
        if active == "cash":
            step_pnl = 0.0
        else:
            # Get delta pnl for this step
            pnl_now = pnl_by_cand.get(active, {}).get(ts, 0)
            prev_ts = all_ts[i-1] if i > 0 else ts
            pnl_prev = pnl_by_cand.get(active, {}).get(prev_ts, 0)
            step_pnl = pnl_now - pnl_prev

        merged_pnl += step_pnl
        equity = budget + merged_pnl
        if equity > peak: peak = equity
        dd = (peak - equity) / peak * 100 if peak > 0 else 0
        if dd > max_dd: max_dd = dd
        equity_series.append(equity)

    # Compute ann and DD
    if not equity_series: return None
    days = (all_ts[-1] - all_ts[0]) / 86400000
    total_ret = (equity_series[-1] - budget) / budget
    ann = ((1 + total_ret) ** (365 / days) - 1) * 100 if days > 0 and (1 + total_ret) > 0 else -999
    return {"ann": ann, "dd": max_dd, "ret": total_ret * 100, "days": days, "final_equity": equity_series[-1]}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mode", default="all")
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=1)
    args = ap.parse_args()

    print("=== Building equity curves ===", flush=True)
    curves_path = args.out.replace(".json", "-curves.json")
    curves = build_curves()
    with open(curves_path, "w") as f:
        json.dump({name: [{"ts": p["timestamp_ms"], "pnl": p["cum_pnl"]} for p in ec] for name, ec in curves.items()}, f)

    # Grid search
    lookbacks = [14*24, 30*24, 60*24]  # points (hourly)
    rebalances = [7*24, 14*24, 30*24]
    max_ankrs = [0.35, 0.50, 0.70]
    cash_dd_triggers = [None, 6, 10, 14]
    score_fns = {
        "ann_minus_dd": lambda ret, dd: ret - 1.5 * dd,
        "ret_over_dd": lambda ret, dd: ret / (dd + 1),
        "ret_minus_cost": lambda ret, dd: ret - 0.5 * dd,
    }

    print(f"=== Running allocator grid ({len(lookbacks)*len(rebalances)*len(max_ankrs)*len(cash_dd_triggers)*len(score_fns)} configs) ===", flush=True)
    t0 = time.time()
    results = []
    for lb in lookbacks:
        for rb in rebalances:
            for ma in max_ankrs:
                for cdd in cash_dd_triggers:
                    for sname, sfn in score_fns.items():
                        r = run_allocator(curves, lb, rb, ma, cdd, sfn)
                        if r:
                            results.append({"lookback": lb//24, "rebalance": rb//24, "max_ankr": ma,
                                           "cash_dd": cdd, "score": sname, **r})
    results.sort(key=lambda v: v.get("ann", -999), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results}, open(args.out, "w"), indent=2)
    print(f"\n[r7G] wrote {args.out}: {len(results)} configs in {time.time()-t0:.0f}s")
    print("\n=== TOP 10 ===")
    for v in results[:10]:
        print(f"  lb{v['lookback']}d rb{v['rebalance']}d ma{v['max_ankr']} cdd{v['cash_dd']} {v['score']:16s} ann={v['ann']:.1f} dd={v['dd']:.1f} ret={v['ret']:.1f}")

if __name__ == "__main__":
    main()
