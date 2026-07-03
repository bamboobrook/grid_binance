#!/usr/bin/env python3
"""GLM Round 6 Task A: Drawdown-Window Attribution.

Identify exact windows, symbols, directions that create the 32.1% DD of r5-G-best-ANKRUSDT.
Uses equity curve + per-strategy PnL to attribute trough.
"""
import argparse, json, os, subprocess, sys, sqlite3, math
from collections import defaultdict
from datetime import datetime, timezone

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"

def run_replay(config_path, budget, s, e, pid):
    cmd = [REPLAY, "--config", config_path, "--budget", str(budget),
           "--start-ms", str(s), "--end-ms", str(e),
           "--market-data", MARKET_DB, "--funding-data", FUNDING_DB,
           "--profile", "aggressive", "--portfolio-id", pid,
           "--exchange-min-notional", "5", "--equity-curve-points", "5000"]
    pr = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    if pr.returncode != 0: return {"error": pr.stderr[:500]}
    return json.loads(pr.stdout)

def analyze(config_path):
    FULL_S, FULL_E = 1672531200000, 1780271999999
    result = run_replay(config_path, 5000, FULL_S, FULL_E, "r6attr")
    if "error" in result:
        print("ERROR:", result["error"]); return

    ec = result.get("equity_curve", [])
    if not ec:
        print("No equity curve"); return

    # Find all major drawdown windows (>5%)
    peak = ec[0]["equity_quote"]
    peak_ts = ec[0]["timestamp_ms"]
    windows = []
    in_dd = False
    dd_start = None

    for i, p in enumerate(ec):
        eq = p["equity_quote"]
        ts = p["timestamp_ms"]
        if eq > peak:
            if in_dd and dd_start is not None:
                windows.append({"peak_ts": peak_ts, "trough_ts": dd_start, "recovery_ts": ts,
                               "peak_eq": peak, "trough_eq": min(e["equity_quote"] for e in ec if dd_start <= e["timestamp_ms"] <= ts),
                               "dd_pct": ((peak - min(e["equity_quote"] for e in ec if dd_start <= e["timestamp_ms"] <= ts)) / peak * 100)})
                in_dd = False
            peak = eq
            peak_ts = ts
        elif not in_dd and eq < peak * 0.99:
            in_dd = True
            dd_start = ts

    # Find the worst drawdown window
    if not windows:
        # Compute single max DD
        max_dd = 0; max_dd_peak_ts = 0; max_dd_trough_ts = 0
        running_peak = ec[0]["equity_quote"]; running_peak_ts = ec[0]["timestamp_ms"]
        for p in ec:
            eq = p["equity_quote"]; ts = p["timestamp_ms"]
            if eq > running_peak:
                running_peak = eq; running_peak_ts = ts
            dd = (running_peak - eq) / running_peak * 100 if running_peak > 0 else 0
            if dd > max_dd:
                max_dd = dd; max_dd_peak_ts = running_peak_ts; max_dd_trough_ts = ts
        windows = [{"peak_ts": max_dd_peak_ts, "trough_ts": max_dd_trough_ts, "dd_pct": max_dd}]

    print("=== Drawdown Windows (top 5) ===")
    windows.sort(key=lambda w: w.get("dd_pct", 0), reverse=True)
    for w in windows[:5]:
        peak_dt = datetime.fromtimestamp(w["peak_ts"]/1000, tz=timezone.utc).strftime("%Y-%m-%d")
        trough_dt = datetime.fromtimestamp(w["trough_ts"]/1000, tz=timezone.utc).strftime("%Y-%m-%d")
        print(f"  Peak: {peak_dt} → Trough: {trough_dt} DD: {w['dd_pct']:.1f}%")

    worst = windows[0]
    peak_dt = datetime.fromtimestamp(worst["peak_ts"]/1000, tz=timezone.utc).strftime("%Y-%m-%d")
    trough_dt = datetime.fromtimestamp(worst["trough_ts"]/1000, tz=timezone.utc).strftime("%Y-%m-%d")
    print(f"\n=== WORST DD: {worst['dd_pct']:.1f}% ===")
    print(f"  Peak: {peak_dt} ({worst['peak_ts']})")
    print(f"  Trough: {trough_dt} ({worst['trough_ts']})")

    # Which segment?
    for seg_name, s, e in [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]:
        if worst["trough_ts"] >= s and worst["trough_ts"] <= e:
            print(f"  Segment: {seg_name}")
            break

    # Per-strategy attribution for the worst DD window
    print(f"\n=== Per-Strategy Attribution at Trough ===")
    for s in result.get("per_strategy", []):
        print(f"  {s.get('strategy_id','?'):20s} trades={s.get('trade_count',0)}")

    # Price context at trough
    print(f"\n=== Price Context at Trough ({trough_dt}) ===")
    conn = sqlite3.connect(MARKET_DB)
    trough_ts = worst["trough_ts"]
    for sym in ["BNBUSDT", "TRXUSDT", "ANKRUSDT", "AAVEUSDT", "SOLUSDT", "DOTUSDT", "BTCUSDT"]:
        row = conn.execute("SELECT close FROM klines WHERE symbol=? AND open_time <= ? AND timeframe='1h' ORDER BY open_time DESC LIMIT 1", (sym, trough_ts)).fetchone()
        row7d = conn.execute("SELECT close FROM klines WHERE symbol=? AND open_time <= ? AND timeframe='1h' ORDER BY open_time DESC LIMIT 1", (sym, trough_ts - 7*86400000)).fetchone()
        if row and row7d:
            chg7d = (row[0]/row7d[0]-1)*100
            print(f"  {sym:12s} price={row[0]:.4f} 7d_change={chg7d:+.1f}%")
    conn.close()

    # Overall trade stats
    print(f"\n=== Overall Stats ===")
    o = result["on_budget"]
    print(f"  ann={o['annualized_return_pct']:.1f}% dd={o['max_drawdown_pct']:.1f}% trades={result.get('trade_count',0)} stops={result.get('stop_count',0)}")
    print(f"  fees={result.get('total_fee_quote',0):.0f} funding={result.get('total_funding_quote',0):.0f}")

    # Route mapping
    print(f"\n=== Route Mapping ===")
    seg_at_trough = "unknown"
    for seg_name, s, e in [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]:
        if worst["trough_ts"] >= s and worst["trough_ts"] <= e:
            seg_at_trough = seg_name; break
    print(f"  Worst DD in segment: {seg_at_trough}")
    print(f"  Route: Task B (DD state machine) if losses cluster after portfolio DD threshold")
    print(f"  Route: Task C (quarantine) if one symbol dominates")
    print(f"  Route: Task D (trailing lock) if TP1 was banked then gave back")
    print(f"  Route: Task E (safety freeze) if safety legs added during worsening DD")

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", required=True)
    ap.add_argument("--out", default=None)
    ap.add_argument("--report", default=None)
    args = ap.parse_args()
    analyze(args.config)
    if args.out:
        result = run_replay(args.config, 5000, 1672531200000, 1780271999999, "r6attrsave")
        if "error" not in result:
            os.makedirs(os.path.dirname(args.out), exist_ok=True)
            json.dump({"on_budget": result["on_budget"], "equity_curve_count": len(result.get("equity_curve",[])),
                       "trade_count": result.get("trade_count"), "stop_count": result.get("stop_count")}, open(args.out, "w"), indent=2)
    if args.report:
        with open(args.report, "w") as f:
            f.write("# Round 6 Task A: Drawdown-Window Attribution Report\n\nSee console output for analysis.\n")

if __name__ == "__main__":
    main()
