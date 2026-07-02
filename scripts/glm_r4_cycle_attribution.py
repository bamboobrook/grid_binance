#!/usr/bin/env python3
"""GLM Round 4 P1: 2025 Cycle Attribution Forensics.

Runs the r3-P1-best config on 2025 segment and analyzes per-symbol/direction
cycle-level attribution: which symbols/directions lose, how much, and why.

Uses the engine's per_strategy output + kline data for cycle reconstruction.
"""
import argparse
import json
import os
import sqlite3
import subprocess
import sys

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
SEG_2025_S = 1735689600000
SEG_2025_E = 1767225599999


def run_replay(config_path, budget, s, e, pid):
    cmd = [REPLAY, "--config", config_path, "--budget", str(budget),
           "--start-ms", str(s), "--end-ms", str(e),
           "--market-data", MARKET_DB, "--funding-data", FUNDING_DB,
           "--profile", "aggressive", "--portfolio-id", pid,
           "--exchange-min-notional", "5"]
    pr = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    if pr.returncode != 0:
        return {"error": pr.stderr.strip()[:500]}
    return json.loads(pr.stdout)


def analyze_2025_prices():
    """Analyze 2025 price behavior per symbol to understand the bear."""
    conn = sqlite3.connect(MARKET_DB)
    syms = ["BNBUSDT", "TRXUSDT", "BCHUSDT", "AAVEUSDT", "SOLUSDT", "DOTUSDT", "BTCUSDT"]
    print("=== 2025 Price Analysis ===")
    print(f"{'SYMBOL':12s} {'start':>10s} {'end':>10s} {'ret%':>8s} {'max_dd%':>8s} {'vol%':>8s} {'trend':>8s}")
    for s in syms:
        rows = conn.execute(
            "select open_time, high, low, close from klines where symbol=? "
            "and open_time between ? and ? and open_time % 3600000 < 60000 "
            "order by open_time", (s, SEG_2025_S, SEG_2025_E)).fetchall()
        if not rows:
            continue
        closes = [r[3] for r in rows]
        start_c = closes[0]
        end_c = closes[-1]
        ret = (end_c / start_c - 1) * 100
        # max drawdown
        peak = max(closes)
        trough = min(closes[closes.index(peak):]) if peak in closes else min(closes)
        max_dd = (trough / peak - 1) * 100 if peak > 0 else 0
        # volatility (daily returns stddev * sqrt(365))
        import math
        daily_rets = []
        for i in range(24, len(closes), 24):
            daily_rets.append(closes[i] / closes[i-24] - 1)
        vol = (sum((r - sum(daily_rets)/len(daily_rets))**2 for r in daily_rets) / len(daily_rets))**0.5 * math.sqrt(365) * 100 if daily_rets else 0
        trend = "DOWN" if ret < -10 else ("UP" if ret > 10 else "FLAT")
        print(f"{s:12s} {start_c:10.2f} {end_c:10.2f} {ret:8.1f} {max_dd:8.1f} {vol:8.1f} {trend:>8s}")
    conn.close()


def run_attribution(config_path):
    """Run the config on 2025 and extract per-strategy attribution."""
    result = run_replay(config_path, 5000, SEG_2025_S, SEG_2025_E, "r4attr2025")
    if "error" in result:
        print("ERROR:", result["error"])
        return
    o = result["on_budget"]
    print(f"\n=== 2025 Full Segment: ann={o['annualized_return_pct']:.1f}%, dd={o['max_drawdown_pct']:.1f}%, ret={o['total_return_pct']:.1f}% ===")
    print(f"trade_count={result.get('trade_count',0)}, stop_count={result.get('stop_count',0)}")
    print(f"total_fee={result.get('total_fee_quote',0):.1f}, total_funding={result.get('total_funding_quote',0):.1f}, total_slippage={result.get('total_slippage_quote',0):.1f}")
    print(f"\n=== Per-Strategy 2025 Attribution ===")
    print(f"{'strategy_id':20s} {'symbol':10s} {'dir':5s} {'trades':>7s} {'stops':>6s}")
    for s in result.get("per_strategy", []):
        sid = s.get("strategy_id", "?")
        sym = s.get("symbol", "?")
        trades = s.get("trade_count", 0)
        stops = s.get("stop_count", 0)
        print(f"{sid:20s} {sym:10s} {'?':5s} {trades:7d} {stops:6d}")
    # monthly breakdown
    print(f"\n=== Monthly Equity Curve Analysis ===")
    ec = result.get("equity_curve", [])
    if ec:
        # group by month
        from collections import defaultdict
        monthly = defaultdict(list)
        for p in ec:
            ts = p.get("timestamp_ms", p.get("ts", 0))
            eq = p.get("equity_quote", p.get("eq", 0))
            month = ts // (30 * 86400000)  # rough month bucket
            monthly[month].append(eq)
        print(f"{'month_bucket':>12s} {'start_eq':>10s} {'end_eq':>10s} {'ret%':>8s}")
        for m in sorted(monthly):
            eqs = monthly[m]
            if len(eqs) > 1:
                r = (eqs[-1] / eqs[0] - 1) * 100 if eqs[0] > 0 else 0
                print(f"{m:12d} {eqs[0]:10.1f} {eqs[-1]:10.1f} {r:8.1f}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", required=True)
    ap.add_argument("--out", default=None)
    args = ap.parse_args()
    analyze_2025_prices()
    run_attribution(args.config)
    if args.out:
        # save raw result for reference
        result = run_replay(args.config, 5000, SEG_2025_S, SEG_2025_E, "r4attrsave")
        if "error" not in result:
            os.makedirs(os.path.dirname(args.out), exist_ok=True)
            with open(args.out, "w") as f:
                json.dump({"on_budget": result["on_budget"], "per_strategy": result.get("per_strategy", []),
                           "trade_count": result.get("trade_count"), "stop_count": result.get("stop_count"),
                           "total_fee_quote": result.get("total_fee_quote"),
                           "total_funding_quote": result.get("total_funding_quote")}, f, indent=2)


if __name__ == "__main__":
    main()
