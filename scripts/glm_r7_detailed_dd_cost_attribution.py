#!/usr/bin/env python3
"""GLM Round 7 Task B: Detailed Drawdown and Cost Attribution."""
import argparse, json, os, subprocess, sys, sqlite3, math
from collections import defaultdict
from datetime import datetime, timezone

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
PREMIUM_DB = "data/premium_index.db"
FULL_S, FULL_E = 1672531200000, 1780271999999

def run_replay(config_path, budget, s, e, pid):
    cmd = [REPLAY, "--config", config_path, "--budget", str(budget),
           "--start-ms", str(s), "--end-ms", str(e),
           "--market-data", MARKET_DB, "--funding-data", FUNDING_DB,
           "--profile", "aggressive", "--portfolio-id", pid,
           "--exchange-min-notional", "5", "--equity-curve-points", "5000"]
    pr = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    if pr.returncode != 0: return {"error": pr.stderr[:500]}
    return json.loads(pr.stdout)

def analyze_candidate(label, config_path, budget, out_data):
    result = run_replay(config_path, budget, FULL_S, FULL_E, f"r7attr_{label}")
    if "error" in result:
        print(f"  {label}: ERROR {result['error'][:100]}")
        return None
    o = result["on_budget"]
    ec = result.get("equity_curve", [])
    print(f"  {label}: ann={o['annualized_return_pct']:.1f}% dd={o['max_drawdown_pct']:.1f}% trades={result.get('trade_count',0)}")

    # Find max DD window from equity curve
    dd_windows = []
    if ec:
        peak = ec[0]["equity_quote"]; peak_ts = ec[0]["timestamp_ms"]
        running_dd = 0; trough_eq = peak; trough_ts = peak_ts
        for p in ec:
            eq = p["equity_quote"]; ts = p["timestamp_ms"]
            if eq > peak:
                if running_dd > 0:
                    dd_windows.append({"peak_ts": peak_ts, "trough_ts": trough_ts, "dd_pct": running_dd})
                peak = eq; peak_ts = ts; running_dd = 0; trough_eq = eq; trough_ts = ts
            dd = (peak - eq) / peak * 100 if peak > 0 else 0
            if dd > running_dd:
                running_dd = dd; trough_eq = eq; trough_ts = ts
        if running_dd > 0:
            dd_windows.append({"peak_ts": peak_ts, "trough_ts": trough_ts, "dd_pct": running_dd})

    dd_windows.sort(key=lambda w: w["dd_pct"], reverse=True)

    # Cost attribution
    total_fee = result.get("total_fee_quote", 0)
    total_funding = result.get("total_funding_quote", 0)
    total_slippage = result.get("total_slippage_quote", 0)
    gross_profit = o.get("total_return_pct", 0) * budget / 100  # approximate
    funding_pct_budget = total_funding / budget * 100 if budget > 0 else 0
    fee_pct_budget = total_fee / budget * 100 if budget > 0 else 0
    cost_share = (total_fee + abs(total_funding)) / (gross_profit + 1) * 100 if gross_profit > 0 else 999

    # Segment DD
    segs = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
    seg_dd = {}
    for nm,s,e in segs:
        seg_ec = [p for p in ec if s <= p["timestamp_ms"] <= e]
        if seg_ec:
            seg_eqs = [p["equity_quote"] for p in seg_ec]
            seg_peak = max(seg_eqs); seg_trough = min(seg_eqs)
            seg_dd[nm] = (seg_peak - seg_trough) / seg_peak * 100 if seg_peak > 0 else 0

    entry = {
        "candidate": label,
        "full": {"ann": o["annualized_return_pct"], "dd": o["max_drawdown_pct"],
                 "ret": o["total_return_pct"], "min_eq": o["min_equity_quote"]},
        "dd_windows": dd_windows[:3],
        "cost_attribution": {
            "total_fee_quote": total_fee, "total_funding_quote": total_funding,
            "total_slippage_quote": total_slippage,
            "funding_pct_of_budget": round(funding_pct_budget, 2),
            "fee_pct_of_budget": round(fee_pct_budget, 2),
            "cost_share_of_gross_pct": round(cost_share, 1)
        },
        "segment_dd": seg_dd,
        "trade_count": result.get("trade_count", 0),
        "stop_count": result.get("stop_count", 0),
    }

    # Route decision
    routes = []
    if funding_pct_budget > 5: routes.append("r7-C-cost-gate")
    if cost_share > 25: routes.append("r7-D-turnover-quality")
    if entry["stop_count"] > 0 and entry["stop_count"] / max(entry["trade_count"],1) > 0.3:
        routes.append("r7-F-safety-freeze")
    if any(dd > 15 for dd in seg_dd.values()):
        routes.append("r7-G-dynamic-blend")
    entry["routes"] = routes
    out_data.append(entry)
    return entry

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--configs", nargs="+", required=True)
    ap.add_argument("--budget", type=float, default=5000)
    ap.add_argument("--out", required=True)
    ap.add_argument("--report", default=None)
    args = ap.parse_args()

    all_data = []
    print("=== Detailed DD and Cost Attribution ===")
    for cfg_spec in args.configs:
        label, path = cfg_spec.split("=", 1)
        if not os.path.exists(path):
            print(f"  {label}: CONFIG MISSING ({path})")
            continue
        analyze_candidate(label, path, args.budget, all_data)

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w") as f:
        json.dump({"candidates": all_data}, f, indent=2)

    if args.report:
        with open(args.report, "w") as f:
            f.write("# Round 7 Task B: Detailed DD and Cost Attribution\n\n")
            f.write("| candidate | ann | DD | funding %budget | fee %budget | cost share % | routes |\n")
            f.write("|---|---:|---:|---:|---:|---:|---|\n")
            for c in all_data:
                ca = c.get("cost_attribution", {})
                f.write(f"| {c['candidate']} | {c['full']['ann']:.1f}% | {c['full']['dd']:.1f}% | "
                       f"{ca.get('funding_pct_of_budget',0)}% | {ca.get('fee_pct_of_budget',0)}% | "
                       f"{ca.get('cost_share_of_gross_pct',0)}% | {', '.join(c.get('routes',[]))} |\n")
            f.write("\n## Segment DD\n")
            f.write("| candidate | h1_2023 | h2_2023 | 2024 | 2025 | 2026_ytd |\n|---|")
            for c in all_data:
                sd = c.get("segment_dd", {})
                f.write(f"| {c['candidate']} | {sd.get('h1_2023',0):.1f}% | {sd.get('h2_2023',0):.1f}% | "
                       f"{sd.get('2024',0):.1f}% | {sd.get('2025',0):.1f}% | {sd.get('2026_ytd',0):.1f}% |\n")

    print(f"\nWrote {args.out} with {len(all_data)} candidates")

if __name__ == "__main__":
    main()
