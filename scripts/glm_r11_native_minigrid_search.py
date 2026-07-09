#!/usr/bin/env python3
"""GLM Round 11 Task P4: Native Minigrid Parameter Search.

Searches the native DCA minigrid config space. Since the kline_engine
integration is research_only (P3), this search uses the config struct via
additional partial TP stages that approximate the minigrid effect (closing
fractions at progressively higher TP levels). This is the same proven-safe
approach as R10 P5 but with the native config naming.

Grid (per plan):
  bases: R4-combo, R7-ANKR-q (2 bases; P3 best added if promoted)
  levels_per_band: 1,2,3,5
  spacing_bps: 15,30,50,80
  close_fraction: 1/8,1/6,1/4
  min_profit_bps: 10,20,35,55
  max_active_levels: 1,2,3
  dca_step_bps: keep_base,180,250,350

Minimum valid configs: 6912. Each runs full + five segments.
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

def load_base(name):
    return json.load(open(BASE_SLEEVES[name]))

def apply_minigrid(cfg, levels, spacing, frac_num, frac_den, min_profit, max_active, dca_step):
    new_cfg = copy.deepcopy(cfg)
    pc = new_cfg["portfolio_config"]
    # Build partial TP stages approximating minigrid: N levels at increasing TP
    stages = []
    remaining = 1000
    for i in range(levels):
        frac = int(1000 * frac_num / frac_den / levels)
        if frac < 1: frac = 1
        tp_bps = min_profit + i * spacing
        stages.append([frac, 1000, tp_bps])
        remaining -= frac
    stages.append([max(remaining, 1), 1000, min_profit * 3])
    for s in pc["strategies"]:
        if dca_step != "keep_base":
            s["spacing"] = {"multiplier": {"first_step_bps": dca_step, "multiplier": "1.35"}}
        s["take_profit"] = {"partial": {"stages": stages, "breakeven_after_stage": 1, "breakeven_buffer_bps": 15}}
        rl = s.get("risk_limits", {})
        rl["max_active_cycles"] = max_active
        rl["dca_minigrid"] = {
            "levels_per_band": levels, "spacing_bps": spacing,
            "close_fraction_num": frac_num, "close_fraction_den": frac_den,
            "min_profit_bps": min_profit, "max_active_levels": max_active,
        }
        s["risk_limits"] = rl
    return new_cfg

def replay(config_path, budget, s, e, pid):
    cmd = [REPLAY, "--config", config_path, "--budget", str(budget),
           "--start-ms", str(s), "--end-ms", str(e),
           "--market-data", _RUNTIME["market"], "--funding-data", _RUNTIME["funding"],
           "--profile", "aggressive", "--portfolio-id", pid, "--exchange-min-notional", "5"]
    try:
        pr = subprocess.run(cmd, capture_output=True, text=True, timeout=900)
    except subprocess.TimeoutExpired:
        return None
    if pr.returncode != 0: return None
    try:
        r = json.loads(pr.stdout)
    except Exception:
        return None
    o = r.get("on_budget", {})
    return {"ann": o.get("annualized_return_pct", -999), "dd": o.get("max_drawdown_pct", 999),
            "ret": o.get("total_return_pct", -999), "trades": r.get("trade_count", 0)}

def evaluate_one(spec):
    label, cfg, tmp_path = spec
    with open(tmp_path, "w") as f:
        json.dump(cfg, f)
    full = replay(tmp_path, 5000, FULL_START, FULL_END, f"r11p4_{label}")
    seg = {}
    for nm, s, e in FULL_SEGMENTS:
        seg[nm] = replay(tmp_path, 5000, s, e, f"r11p4_{label}_{nm}")
    try: os.remove(tmp_path)
    except: pass
    if full is None: return {"label": label, "skipped": True}
    pos = sum(1 for v in seg.values() if v and v["ret"] > 0)
    target_hit = []
    if full["ann"] >= 50 and full["dd"] <= 10 and pos >= 4: target_hit.append("conservative")
    if full["ann"] >= 90 and full["dd"] <= 20 and pos >= 4: target_hit.append("balanced")
    if full["ann"] >= 110 and full["dd"] <= 30 and pos >= 3: target_hit.append("aggressive")
    return {"label": label, "full_metrics": full, "segment_metrics": seg,
            "positive_segments": pos, "target_hit": target_hit, "research_only": True,
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
        for levels in [1,2,3,5]:
            for spacing in [15,30,50,80]:
                for frac_num, frac_den in [(1,8),(1,6),(1,4)]:
                    for min_profit in [10,20,35,55]:
                        for max_active in [1,2,3]:
                            for dca_step in ["keep_base",180,250,350]:
                                ds = dca_step if dca_step == "keep_base" else str(dca_step)
                                label = f"{base_name}_l{levels}_s{spacing}_f{frac_num}of{frac_den}_p{min_profit}_a{max_active}_d{ds}"
                                cfg = apply_minigrid(base_cfgs[base_name], levels, spacing, frac_num, frac_den, min_profit, max_active, dca_step)
                                tmp = f"/tmp/r11p4_{label}.json"
                                specs.append((label, cfg, tmp))

    print(f"Generated {len(specs)} specs (need >=6912). Running full + 5 segments each...", flush=True)
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
    print(f"\n[r11P4] wrote {args.out}: {len(valid)} eval, {len(targets)} targets in {el:.0f}s")
    print(f"\n=== TOP 10 by ann ===")
    for v in valid[:10]:
        fm = v["full_metrics"]
        print(f"  {v['label']:70s} ann={fm['ann']:7.1f} dd={fm['dd']:6.1f} pos={v['positive_segments']}/5")

if __name__ == "__main__":
    main()
