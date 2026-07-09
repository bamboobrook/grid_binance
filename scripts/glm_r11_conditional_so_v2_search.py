#!/usr/bin/env python3
"""GLM Round 11 Task P6: Conditional Safety Order V2 Without Strict Blocking.

Avoids Round10's strict safety_order_condition all-or-nothing gate. Uses
existing martingale-native risk controls instead: rebound_bps, basis, ADX
skip, drawdown-state scale, late-leg cap.

Minimum valid configs: 4000.
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

BASE_SLEEVES = {
    "R4-combo": "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json",
    "R7-ANKR-q": "docs/superpowers/artifacts/glm-martingale-core-round7/promising/r7-ANKR-q1w24p24.json",
    "R6-QB": "docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-QB-best.json",
}
_RUNTIME = {"market": "data/market_data_full.db", "funding": "data/funding_rates.db"}

def load_base(name): return json.load(open(BASE_SLEEVES[name]))

def apply_so_v2(cfg, rebound_bps, basis, adx_skip, dd_scale, late_leg_cap, step_bps=None, foq=None):
    new_cfg = copy.deepcopy(cfg)
    pc = new_cfg["portfolio_config"]
    for s in pc["strategies"]:
        rl = s.get("risk_limits", {})
        rl["safety_order_rebound_bps"] = rebound_bps
        rl["safety_order_basis"] = basis
        rl["safety_skip_adx_threshold"] = adx_skip
        if dd_scale < 1.0:
            rl["drawdown_state_rules"] = [{"trigger_drawdown_pct": 10.0, "safety_order_scale": dd_scale, "first_order_scale": None, "cooldown_multiplier": None, "freeze_safety_orders": None}]
        if late_leg_cap is not None:
            rl["taper_safety_after_leg"] = late_leg_cap
            rl["taper_safety_scale"] = 0.7
        s["risk_limits"] = rl
        if step_bps is not None:
            s["spacing"] = {"multiplier": {"first_step_bps": step_bps, "multiplier": "1.35"}}
        if foq is not None and "multiplier" in s["sizing"]:
            s["sizing"]["multiplier"]["first_order_quote"] = str(foq)
    return new_cfg

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
    full = replay(tmp_path, 5000, FULL_START, FULL_END, f"r11p6_{label}")
    seg = {}
    for nm, s, e in FULL_SEGMENTS: seg[nm] = replay(tmp_path, 5000, s, e, f"r11p6_{label}_{nm}")
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

    bases = list(BASE_SLEEVES.keys())
    base_cfgs = {b: load_base(b) for b in bases}
    specs = []
    for base_name in bases:
        for rebound in [0, 20, 40, 70]:
            for basis in ["base_order", "last_executed_order"]:
                for adx in [35, 45, 55, 70]:
                    for dd_scale in [0.5, 0.7, 0.9, 1.0]:
                        for late_cap in [None, 4]:
                            for step in [None, 250]:
                                for foq in [None, 15, 30]:
                                    if len(specs) >= 4608: break
                                    label = f"{base_name}_rb{rebound}_{basis[:3]}_adx{adx}_dds{dd_scale}_lc{late_cap if late_cap else 'n'}_s{step if step else 'b'}_f{foq if foq else 'b'}"
                                    cfg = apply_so_v2(base_cfgs[base_name], rebound, basis, adx, dd_scale, late_cap, step, foq)
                                    specs.append((label, cfg, f"/tmp/r11p6_{label}.json"))
                            if len(specs) >= 4608: break
                        if len(specs) >= 4608: break
                    if len(specs) >= 4608: break
                if len(specs) >= 4608: break
            if len(specs) >= 4608: break
        if len(specs) >= 4608: break

    print(f"Generated {len(specs)} specs (need >=4000). Running full + 5 segments each...", flush=True)
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
    print(f"\n[r11P6] wrote {args.out}: {len(valid)} eval, {len(targets)} targets in {el:.0f}s")
    print(f"\n=== TOP 10 by ann ===")
    for v in valid[:10]:
        fm = v["full_metrics"]
        print(f"  {v['label']:70s} ann={fm['ann']:7.1f} dd={fm['dd']:6.1f} pos={v['positive_segments']}/5")

if __name__ == "__main__":
    main()
