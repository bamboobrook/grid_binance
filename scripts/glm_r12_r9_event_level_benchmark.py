#!/usr/bin/env python3
"""Round 12 diagnostic: static shared-account merge of the R9 sleeves.

The replay binary has no shadow/live sleeve state or dynamic rotation. All
sleeve strategies run simultaneously. This tests only the static 36-strategy
merge and cannot close the R9 dynamic allocator family.

Output:
- Full development window metrics
- 5 cold-start segments
- Budget ladder (1000, 2000, 3000, 4000, 4999)
- Comparison to R9 curve diagnostic (62.7845/18.3840 with corrected funding)
"""
import argparse, json, os, subprocess, sys, time, copy

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates_round12.db"

DEV_START = 1672531200000
DEV_END = 1780271999999

COLD_START_SEGMENTS = [
    ("h1_2023", 1672531200000, 1688169599999),
    ("h2_2023", 1688169600000, 1704067199999),
    ("2024",    1704067200000, 1735689599999),
    ("2025",    1735689600000, 1767225599999),
    ("2026_ytd",1767225600000, 1780271999999),
]

BUDGET_LADDER = [1000, 2000, 3000, 4000, 4999]

# R9 5 sleeves — merge all strategies into one portfolio
R9_SLEEVE_CONFIGS = {
    "ANKR-q": "docs/superpowers/artifacts/glm-martingale-core-round7/promising/r7-ANKR-q1w24p24.json",
    "QB":     "docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-QB-best.json",
    "R4":     "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json",
    "fine":   "docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-fine-combo-best.json",
    "p4-ANKR-q-rp10-fos05": "docs/superpowers/artifacts/glm-martingale-core-round9/promising/p4-ANKR-q-rp10-fos05.json",
}


def build_combined_portfolio(budget=4999):
    """Merge all R9 sleeve strategies into one portfolio config."""
    all_strategies = []
    for name, path in R9_SLEEVE_CONFIGS.items():
        if not os.path.exists(path):
            continue
        cfg = json.load(open(path))
        pc = cfg.get("portfolio_config", cfg)
        for s in pc.get("strategies", []):
            # Ensure unique strategy IDs by prefixing with sleeve name
            s["strategy_id"] = f"{name}_{s['strategy_id']}"
            all_strategies.append(s)

    # Normalize portfolio weights to sum to 100
    total_weight = sum(float(s.get("portfolio_weight_pct", 0)) for s in all_strategies)
    if total_weight > 0:
        scale = 100.0 / total_weight
        for s in all_strategies:
            w = float(s.get("portfolio_weight_pct", 0)) * scale
            s["portfolio_weight_pct"] = f"{w:.4f}"

    return {
        "portfolio_config": {
            "direction_mode": "long_and_short",
            "risk_limits": {"max_global_budget_quote": str(budget) + ".00"},
            "strategies": all_strategies,
        }
    }


def replay(config_path, budget, start_ms, end_ms, pid):
    cmd = [REPLAY, "--config", config_path, "--budget", str(budget),
           "--start-ms", str(start_ms), "--end-ms", str(end_ms),
           "--market-data", MARKET_DB, "--funding-data", FUNDING_DB,
           "--profile", "aggressive", "--portfolio-id", pid,
           "--exchange-min-notional", "5"]
    try:
        pr = subprocess.run(cmd, capture_output=True, text=True, timeout=900)
    except subprocess.TimeoutExpired:
        return None
    if pr.returncode != 0:
        return None
    try:
        r = json.loads(pr.stdout)
    except Exception:
        return None
    o = r.get("on_budget", {})
    return {
        "ann": o.get("annualized_return_pct", -999),
        "dd": o.get("max_drawdown_pct", 999),
        "ret": o.get("total_return_pct", -999),
        "trades": r.get("trade_count", 0),
        "max_cap": r.get("on_max_capital_used", {}).get("max_capital_used_quote", 0),
        "blocked": r.get("budget_blocked_legs", 0),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--budget", type=int, default=4999)
    args = ap.parse_args()

    print("=== R9 Static Shared-Account Merge Diagnostic ===", flush=True)
    print(f"Budget: {args.budget}U", flush=True)
    print(f"Funding DB: {FUNDING_DB} (Round12 frozen with ANKR/LTC补齐)", flush=True)

    # Build combined portfolio
    combined = build_combined_portfolio(args.budget)
    n_strategies = len(combined["portfolio_config"]["strategies"])
    symbols = sorted(set(s["symbol"] for s in combined["portfolio_config"]["strategies"]))
    print(f"Combined portfolio: {n_strategies} strategies, {len(symbols)} symbols: {symbols}", flush=True)

    # Write config
    config_path = f"/tmp/r12_r9_event_level_{args.budget}.json"
    with open(config_path, "w") as f:
        json.dump(combined, f)

    result = {
        "candidate": "r9-static-36-strategy-merge-diagnostic",
        "budget": args.budget,
        "shared_account_event_level": True,
        "static_portfolio": True,
        "dynamic_sleeve_allocator": False,
        "scope": "Five R9 sleeve configs merged and active simultaneously; no shadow/live state or rotation.",
        "n_strategies": n_strategies,
        "traded_symbols": symbols,
        "symbol_count": len(symbols),
        "full_metrics": None,
        "cold_start_segments": {},
        "budget_ladder": {},
        "r9_curve_diagnostic_corrected": {"ann": 62.7845, "dd": 18.3840},
    }

    # Full development window
    print(f"\n=== Full development window replay ===", flush=True)
    t0 = time.time()
    full = replay(config_path, args.budget, DEV_START, DEV_END, "r12r9el_full")
    result["full_metrics"] = full
    print(f"  Full: {full} ({time.time()-t0:.0f}s)", flush=True)

    if full is None:
        print("FAIL: full replay returned None", file=sys.stderr)
        result["research_threshold_hits"] = []
    else:
        result["research_threshold_hits"] = []
        if full["ann"] >= 50 and full["dd"] <= 10:
            result["research_threshold_hits"].append("conservative")
        if full["ann"] >= 90 and full["dd"] <= 20:
            result["research_threshold_hits"].append("balanced")
        if full["ann"] >= 110 and full["dd"] <= 30:
            result["research_threshold_hits"].append("aggressive")

    # Cold-start segments
    print(f"\n=== Cold-start segments ===", flush=True)
    for seg_name, s, e in COLD_START_SEGMENTS:
        m = replay(config_path, args.budget, s, e, f"r12r9el_cs_{seg_name}")
        result["cold_start_segments"][seg_name] = m
        if m:
            print(f"  {seg_name}: ann={m['ann']:.1f}% dd={m['dd']:.1f}% ret={m['ret']:.1f}%", flush=True)
    pos_segs = sum(1 for v in result["cold_start_segments"].values() if v and v["ret"] > 0)
    result["positive_segments"] = pos_segs

    # Budget ladder
    print(f"\n=== Budget ladder ===", flush=True)
    for b in BUDGET_LADDER:
        m = replay(config_path, b, DEV_START, DEV_END, f"r12r9el_bl_{b}")
        result["budget_ladder"][str(b)] = m
        if m:
            print(f"  {b}U: ann={m['ann']:.1f}% dd={m['dd']:.1f}%", flush=True)

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w") as f:
        json.dump(result, f, indent=2)
    print(f"\nWrote {args.out}")
    print(f"Research threshold hits: {result['research_threshold_hits']}")
    print(f"Positive segments: {pos_segs}/5")

    # This threshold closes only the static-merge diagnostic. The dynamic
    # allocator still requires a dedicated event-level implementation.
    if full and (full["ann"] < 45 or full["dd"] > 30):
        print("\nSTATIC MERGE FAILURE: ann < 45% or DD > 30%", file=sys.stderr)
        print("Dynamic allocator family remains untested", file=sys.stderr)


if __name__ == "__main__":
    main()
