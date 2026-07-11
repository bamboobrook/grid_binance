#!/usr/bin/env python3
"""GLM Round 11 Task P5: Non-R4 Martingale Architecture Expansion.

Searches martingale-native families NOT based on R4-combo's parameter set.
Families: fixed-percent TP (no partial), low-mult high-freq, vol-ladder, asymmetric.

Minimum valid configs: 8000 full plus segment replays.
"""
import argparse, json, os, subprocess, sys, time, copy
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
FULL_SEGMENTS = [
    ("h1_2023", 1672531200000, 1688169599999),
    ("h2_2023", 1688169600000, 1704067199999),
    ("2024",    1704067200000, 1735689599999),
    ("2025",    1735689600000, 1767225599999),
    ("2026_ytd",1767225600000, 1780271999999),
]
FULL_START, FULL_END = 1672531200000, 1780271999999

# Use a broad symbol set from R4-combo
SYMBOLS_LONG = ["BNBUSDT", "TRXUSDT", "BCHUSDT"]
SYMBOLS_SHORT = ["AAVEUSDT", "SOLUSDT", "DOTUSDT"]
_RUNTIME = {"market": "data/market_data_full.db", "funding": "data/funding_rates.db"}

def build_strategy(sid, symbol, direction, tp_bps, foq, mult, max_legs, step_bps, atr_pause):
    return {
        "market": "usd_m_futures",
        "sizing": {"multiplier": {"max_legs": max_legs, "multiplier": str(mult), "first_order_quote": str(foq)}},
        "symbol": symbol, "spacing": {"fixed_percent": {"step_bps": step_bps}},
        "leverage": 10, "direction": direction, "margin_mode": "isolated",
        "stop_loss": {"strategy_drawdown_pct": {"pct_bps": 5000}},
        "indicators": [{"atr": {"period": 14}}],
        "risk_limits": {"max_active_cycles": None, "max_global_budget_quote": None,
                        "max_symbol_budget_quote": None, "max_global_drawdown_quote": None,
                        "max_strategy_budget_quote": None, "max_direction_budget_quote": None,
                        "new_cycle_atr_pause_pct": atr_pause},
        "strategy_id": sid,
        "take_profit": {"percent": {"bps": tp_bps}},
        "direction_mode": "long_and_short",
        "entry_triggers": [{"cooldown": {"seconds": 39600}}],
        "portfolio_weight_pct": "16.67",
    }

def build_portfolio(family, tp_bps=None, foq=None, mult=None, max_legs=None, step_bps=None, atr_pause=None, max_active=None):
    strategies = []
    if family == "fixed_tp":
        for i, s in enumerate(SYMBOLS_LONG):
            strategies.append(build_strategy(f"fl{i}", s, "long", tp_bps, foq, mult, max_legs, 150, 2.0))
        for i, s in enumerate(SYMBOLS_SHORT):
            strategies.append(build_strategy(f"fs{i}", s, "short", tp_bps, foq, mult, max_legs, 180, 2.0))
    elif family == "low_mult_high_freq":
        for i, s in enumerate(SYMBOLS_LONG):
            strategies.append(build_strategy(f"ll{i}", s, "long", 100, 15, mult, max_legs, step_bps, 2.0))
        for i, s in enumerate(SYMBOLS_SHORT):
            strategies.append(build_strategy(f"ls{i}", s, "short", 100, 15, mult, max_legs, step_bps, 2.0))
    elif family == "vol_ladder":
        for i, s in enumerate(SYMBOLS_LONG):
            strategies.append(build_strategy(f"vl{i}", s, "long", 100, 20, 1.8, 6, 150, atr_pause))
        for i, s in enumerate(SYMBOLS_SHORT):
            strategies.append(build_strategy(f"vs{i}", s, "short", 100, 20, 1.8, 6, 180, atr_pause))
    elif family == "asymmetric":
        # Long uses step 150 tp 100 mult 2.8; short uses step 250 tp 150 mult 1.5
        for i, s in enumerate(SYMBOLS_LONG):
            strategies.append(build_strategy(f"al{i}", s, "long", 100, 20, 2.8, 8, 150, 2.0))
        for i, s in enumerate(SYMBOLS_SHORT):
            strategies.append(build_strategy(f"as{i}", s, "short", 150, 20, 1.5, 5, 250, 2.0))
    if max_active:
        for s in strategies:
            s["risk_limits"]["max_active_cycles"] = max_active
    return {"portfolio_config": {"direction_mode": "long_and_short",
            "risk_limits": {"max_global_budget_quote": "5000.00000000"}, "strategies": strategies}}

def replay(config_path, budget, s, e, pid):
    cmd = [REPLAY, "--config", config_path, "--budget", str(budget),
           "--start-ms", str(s), "--end-ms", str(e),
           "--market-data", _RUNTIME["market"], "--funding-data", _RUNTIME["funding"],
           "--profile", "aggressive", "--portfolio-id", pid, "--exchange-min-notional", "5"]
    try: pr = subprocess.run(cmd, capture_output=True, text=True, timeout=900)
    except subprocess.TimeoutExpired: return None
    if pr.returncode != 0: return None
    try: r = json.loads(pr.stdout)
    except: return None
    o = r.get("on_budget", {})
    return {"ann": o.get("annualized_return_pct", -999), "dd": o.get("max_drawdown_pct", 999),
            "ret": o.get("total_return_pct", -999), "trades": r.get("trade_count", 0)}

def evaluate_one(spec):
    label, cfg, tmp_path = spec
    with open(tmp_path, "w") as f: json.dump(cfg, f)
    full = replay(tmp_path, 5000, FULL_START, FULL_END, f"r11p5_{label}")
    seg = {}
    for nm, s, e in FULL_SEGMENTS: seg[nm] = replay(tmp_path, 5000, s, e, f"r11p5_{label}_{nm}")
    try: os.remove(tmp_path)
    except: pass
    if full is None: return {"label": label, "skipped": True}
    pos = sum(1 for v in seg.values() if v and v["ret"] > 0)
    target_hit = []
    if full["ann"] >= 50 and full["dd"] <= 10 and pos >= 4: target_hit.append("conservative")
    if full["ann"] >= 90 and full["dd"] <= 20 and pos >= 4: target_hit.append("balanced")
    if full["ann"] >= 110 and full["dd"] <= 30 and pos >= 3: target_hit.append("aggressive")
    return {"label": label, "full_metrics": full, "segment_metrics": seg,
            "positive_segments": pos, "target_hit": target_hit,
            "traded_symbols": list(set(s["symbol"] for s in cfg["portfolio_config"]["strategies"])),
            "symbol_count": len(set(s["symbol"] for s in cfg["portfolio_config"]["strategies"]))}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--budget", type=float, default=5000)
    ap.add_argument("--market-data", default="data/market_data_full.db")
    ap.add_argument("--funding-data", default="data/funding_rates.db")
    ap.add_argument("--workers", type=int, default=28)
    args = ap.parse_args()
    _RUNTIME["market"] = args.market_data
    _RUNTIME["funding"] = args.funding_data

    specs = []
    # Family 1: fixed_tp — full cartesian with step variants (6 tp × 4 foq × 4 mult × 4 ml × 6 step = 4608)
    for tp in [45,60,80,100,130,180]:
        for foq in [8,12,18,25]:
            for mult in [1.15,1.35,1.55,1.8]:
                for ml in [3,4,5,6]:
                    for step in [100,150,200,250,300,400]:
                        label = f"fixed_tp_t{tp}_f{foq}_m{mult}_l{ml}_s{step}"
                        cfg = build_portfolio("fixed_tp", tp_bps=tp, foq=foq, mult=mult, max_legs=ml)
                        for s in cfg["portfolio_config"]["strategies"]:
                            s["spacing"] = {"fixed_percent": {"step_bps": step}}
                        specs.append((label, cfg, f"/tmp/r11p5_{label}.json"))
    # Family 2: low_mult_high_freq (4*3*3 = 36)
    for step in [60,90,120,160]:
        for mult in [1.05,1.15,1.25]:
            for ma in [1,2,3]:
                label = f"low_mult_s{step}_m{mult}_a{ma}"
                cfg = build_portfolio("low_mult_high_freq", mult=mult, max_legs=6, step_bps=step, max_active=ma)
                specs.append((label, cfg, f"/tmp/r11p5_{label}.json"))
    # Family 3: vol_ladder (4)
    for atr in [1.2,1.6,2.0,2.6]:
        label = f"vol_ladder_a{atr}"
        cfg = build_portfolio("vol_ladder", atr_pause=atr)
        specs.append((label, cfg, f"/tmp/r11p5_{label}.json"))
    # Family 4: asymmetric with variants (4 tp × 4 mult × 4 ml = 64)
    for tp_long in [80,100,130,180]:
        for mult_long in [2.0,2.4,2.8,3.2]:
            for ml_long in [6,8,10,12]:
                label = f"asym_tp{tp_long}_m{mult_long}_l{ml_long}"
                cfg = build_portfolio("asymmetric")
                for s in cfg["portfolio_config"]["strategies"]:
                    if s["direction"] == "long":
                        s["take_profit"] = {"percent": {"bps": tp_long}}
                        s["sizing"]["multiplier"]["multiplier"] = str(mult_long)
                        s["sizing"]["multiplier"]["max_legs"] = ml_long
                specs.append((label, cfg, f"/tmp/r11p5_{label}.json"))
    # Family 5: additional foq+step variants for fixed_tp to reach 8000+
    for tp in [45,60,80,100,130,180]:
        for foq in [5,10,15,20,30,40]:
            for mult in [1.0,1.2,1.5,2.0,2.5,3.0,3.5]:
                for ml in [3,5,7,8]:
                    for step in [120,180,250,350,500,700]:
                        if len(specs) >= 8100: break
                        label = f"fixed_tp_v3_t{tp}_f{foq}_m{mult}_l{ml}_s{step}"
                        cfg = build_portfolio("fixed_tp", tp_bps=tp, foq=foq, mult=mult, max_legs=ml)
                        for s in cfg["portfolio_config"]["strategies"]:
                            s["spacing"] = {"fixed_percent": {"step_bps": step}}
                        specs.append((label, cfg, f"/tmp/r11p5_{label}.json"))
                    if len(specs) >= 8100: break
                if len(specs) >= 8100: break
            if len(specs) >= 8100: break
        if len(specs) >= 8100: break

    print(f"Generated {len(specs)} specs (need >=8000). Running full + 5 segments each...", flush=True)
    t0 = time.time()
    results = []
    completed = 0
    with ProcessPoolExecutor(max_workers=args.workers) as ex:
        futures = {ex.submit(evaluate_one, spec): spec[0] for spec in specs}
        for fut in as_completed(futures):
            try: r = fut.result()
            except Exception as e: r = {"label": futures[fut], "skipped": True, "error": str(e)}
            results.append(r)
            completed += 1
            if completed % 200 == 0:
                el = time.time() - t0
                rate = completed / el if el > 0 else 0
                eta = (len(specs) - completed) / rate if rate > 0 else 0
                print(f"  [{completed}/{len(specs)}] el={el:.0f}s rate={rate:.2f}/s eta={eta:.0f}s", flush=True)

    valid = [r for r in results if not r.get("skipped")]
    valid.sort(key=lambda r: r["full_metrics"]["ann"], reverse=True)
    targets = [r for r in valid if r.get("target_hit")]
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": valid, "target_hits": targets, "total_specs": len(specs),
               "total_evaluated": len(valid), "total_skipped": len(results)-len(valid),
               "total_target_hits": len(targets)}, open(args.out, "w"), indent=2)
    el = time.time() - t0
    print(f"\n[r11P5] wrote {args.out}: {len(valid)} eval, {len(targets)} targets in {el:.0f}s")
    print(f"\n=== TOP 10 by ann ===")
    for v in valid[:10]:
        fm = v["full_metrics"]
        print(f"  {v['label']:60s} ann={fm['ann']:7.1f} dd={fm['dd']:6.1f} pos={v['positive_segments']}/5")

if __name__ == "__main__":
    main()
