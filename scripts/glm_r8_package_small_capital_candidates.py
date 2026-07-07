#!/usr/bin/env python3
"""GLM Round 8 Task P5: Package small-capital live-executable candidates."""
import json, os, subprocess, sys

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999

def replay(config_path, budget, s, e, pid):
    cmd = [REPLAY, "--config", config_path, "--budget", str(budget),
           "--start-ms", str(s), "--end-ms", str(e),
           "--market-data", MARKET_DB, "--funding-data", FUNDING_DB,
           "--profile", "aggressive", "--portfolio-id", pid, "--exchange-min-notional", "5"]
    pr = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    if pr.returncode != 0: return None
    r = json.loads(pr.stdout)
    o = r["on_budget"]
    return {"ann": o["annualized_return_pct"], "dd": o["max_drawdown_pct"], "ret": o["total_return_pct"],
            "max_cap": r.get("on_max_capital_used",{}).get("max_capital_used_quote",0),
            "blocked": r.get("budget_blocked_legs",0),
            "trades": r.get("trade_count",0)}

# Promoted candidates: R7-ANKR-q (best ann), R6-QB (best DD), R4-combo (live-ready)
CANDIDATES = [
    {"label": "R8-ANKR-q", "config": "docs/superpowers/artifacts/glm-martingale-core-round7/promising/r7-ANKR-q1w24p24.json"},
    {"label": "R8-QB-lowdd", "config": "docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-QB-best.json"},
    {"label": "R8-R4-combo", "config": "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json"},
    {"label": "R8-R5-fine", "config": "docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-fine-combo-best.json"},
]

out = []
for c in CANDIDATES:
    if not os.path.exists(c["config"]): continue
    cfg = json.load(open(c["config"]))["portfolio_config"]
    symbols = list(set(s["symbol"] for s in cfg["strategies"]))
    weights = {s["symbol"]: float(s["portfolio_weight_pct"]) for s in cfg["strategies"]}
    max_w = max(weights.values()) if weights else 0
    
    full = replay(c["config"], 5000, FULL_START, FULL_END, f"r8pkg_{c['label']}")
    seg_results = {}
    for nm, s, e in FULL_SEGMENTS:
        seg_results[nm] = replay(c["config"], 5000, s, e, f"r8pkg_{c['label']}_{nm}")
    
    pos_segs = sum(1 for v in seg_results.values() if v and v["ret"] > 0)
    seg_rets = {n: (v["ret"] if v else 0) for n, v in seg_results.items()}
    agg = sum(seg_rets.get(k, 0) for k in ("2024", "2025", "2026_ytd"))
    
    pkg = {
        "label": c["label"],
        "config_path": c["config"],
        "budget": 5000,
        "full_metrics": full,
        "segment_metrics": seg_results,
        "positive_segments": pos_segs,
        "agg_2024_2026": agg,
        "traded_symbols": symbols,
        "symbol_count": len(symbols),
        "max_symbol_budget_pct": max_w,
        "portfolio_candidate": len(symbols) >= 5 and max_w <= 35,
        "live_ready": c["label"] == "R8-R4-combo",  # only R4 has full parity
        "near_targets": [],
    }
    if full and full["ann"] >= 45 and full["dd"] <= 12:
        pkg["near_targets"].append("conservative_near")
    if full and full["ann"] >= 50 and full["dd"] <= 20:
        pkg["near_targets"].append("NF-conservative-dd")
    
    out.append(pkg)
    print(f"  {c['label']}: ann={full['ann']:.1f}% dd={full['dd']:.1f}% pos={pos_segs}/5 syms={len(symbols)} max_w={max_w:.1f}% near={pkg['near_targets']}")

os.makedirs("docs/superpowers/artifacts/glm-martingale-core-round8", exist_ok=True)
json.dump({"packages": out}, open("docs/superpowers/artifacts/glm-martingale-core-round8/r8-small-capital-packages.json", "w"), indent=2)
print(f"\nWrote {len(out)} packages")
