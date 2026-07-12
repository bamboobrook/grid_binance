#!/usr/bin/env python3
"""GLM Round 13 Task P3: Capital-Aware Active-Cycle Scheduler Screen.

128-config mechanism screen testing different 2-3 strategy portfolios
with varied parameters. Explores the R13 discovery that fewer strategies
= higher ann per strategy.

Dimensions:
  pair: all C(6,2)=15 pairs from R4-combo strategies
  step_bps: 100, 130, 150, 180
  tp_bps: 80, 100, 130
  multiplier_scale: 1.0, 1.3
  budget: 4999 (fixed for screen)

Continue condition: ann>=40%, DD<=25%, >=4/5 positive segments → expand to 512.
"""
import argparse, json, os, subprocess, sys, time, copy, itertools
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates_round12.db"
FULL_START, FULL_END = 1672531200000, 1780271999999
FULL_SEGMENTS = [
    ("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),
    ("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),
    ("2026_ytd",1767225600000,1780271999999),
]
BASE_PATH = "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json"

def replay(config_path, budget, s, e, pid):
    cmd = [REPLAY, "--config", config_path, "--budget", str(budget),
           "--start-ms", str(s), "--end-ms", str(e),
           "--market-data", MARKET_DB, "--funding-data", FUNDING_DB,
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
    full = replay(tmp_path, 4999, FULL_START, FULL_END, f"r13p3_{label}")
    seg = {}
    for nm, s, e in FULL_SEGMENTS: seg[nm] = replay(tmp_path, 4999, s, e, f"r13p3_{label}_{nm}")
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
            "traded_symbols": sorted(set(s["symbol"] for s in cfg["portfolio_config"]["strategies"])),
            "symbol_count": len(set(s["symbol"] for s in cfg["portfolio_config"]["strategies"]))}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=28)
    args = ap.parse_args()

    base = json.load(open(BASE_PATH))
    strategies = base["portfolio_config"]["strategies"]

    # Generate 128+ configs: C(6,2)=15 pairs × variations
    specs = []
    for i, j in itertools.combinations(range(6), 2):
        for step in [100, 130, 150]:
            for mult_scale in [1.0, 1.3, 0.8]:
                s1 = copy.deepcopy(strategies[i])
                s2 = copy.deepcopy(strategies[j])
                for s in [s1, s2]:
                    s["portfolio_weight_pct"] = "50.0"
                    s["spacing"] = {"fixed_percent": {"step_bps": step}}
                    m = float(s["sizing"]["multiplier"]["multiplier"])
                    s["sizing"]["multiplier"]["multiplier"] = f"{m * mult_scale:.4f}"
                cfg = copy.deepcopy(base)
                cfg["portfolio_config"]["strategies"] = [s1, s2]
                cfg["portfolio_config"]["risk_limits"]["max_global_budget_quote"] = "4999.00"
                sym_pair = f"{strategies[i]['symbol'][:4]}{strategies[i]['direction'][0]}_{strategies[j]['symbol'][:4]}{strategies[j]['direction'][0]}"
                label = f"{sym_pair}_s{step}_m{mult_scale}"
                specs.append((label, cfg, f"/tmp/r13p3_{len(specs):04d}.json"))
                if len(specs) >= 128:
                    break
            if len(specs) >= 128: break
        if len(specs) >= 128: break

    print(f"Generated {len(specs)} configs. Running full + 5 segments each...", flush=True)
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
            if completed % 50 == 0:
                el = time.time() - t0
                print(f"  [{completed}/{len(specs)}] el={el:.0f}s", flush=True)

    valid = [r for r in results if not r.get("skipped")]
    valid.sort(key=lambda r: r["full_metrics"]["ann"], reverse=True)
    targets = [r for r in valid if r.get("target_hit")]
    near = [r for r in valid if r["full_metrics"]["ann"] >= 40 and r["full_metrics"]["dd"] <= 25 and r["positive_segments"] >= 4]

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": valid[:50], "target_hits": targets, "near_targets": near,
               "total_specs": len(specs), "total_evaluated": len(valid)}, open(args.out, "w"), indent=2)
    el = time.time() - t0
    print(f"\n[r13P3] wrote {args.out}: {len(valid)} eval, {len(targets)} targets, {len(near)} near in {el:.0f}s")
    print(f"\n=== TOP 15 by ann ===")
    for v in valid[:15]:
        fm = v["full_metrics"]
        print(f"  {v['label']:55s} ann={fm['ann']:7.1f} dd={fm['dd']:6.1f} pos={v['positive_segments']}/5 tgt={v.get('target_hit',[])}")
    if near:
        print(f"\n=== NEAR-TARGETS ({len(near)}) ===")
        for v in near[:10]:
            fm = v["full_metrics"]
            print(f"  {v['label']}: ann={fm['ann']:.1f}% dd={fm['dd']:.1f}% pos={v['positive_segments']}/5")

if __name__ == "__main__":
    main()
