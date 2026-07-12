#!/usr/bin/env python3
"""GLM Round 13 Task P5: ATR Spacing + Safety-Depth TP Search (768 configs)."""
import argparse, json, os, subprocess, sys, time, copy, itertools
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),
    ("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999
BASE_PATH = "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json"

def apply_depth_tp(cfg, tp01, tp23, red23, tp4, red4):
    new_cfg = copy.deepcopy(cfg)
    for s in new_cfg["portfolio_config"]["strategies"]:
        rl = s.get("risk_limits", {})
        rl["depth_tp"] = {"depth_01_tp_bps": tp01, "depth_23_tp_bps": tp23, "depth_23_reduce_pct": red23,
                          "depth_4plus_tp_bps": tp4, "depth_4plus_reduce_pct": red4}
        s["risk_limits"] = rl
    return new_cfg

def replay(config_path, budget, s, e, pid):
    cmd = [REPLAY, "--config", config_path, "--budget", str(budget), "--start-ms", str(s), "--end-ms", str(e),
           "--market-data", "data/market_data_full.db", "--funding-data", "data/funding_rates_round12.db",
           "--profile", "aggressive", "--portfolio-id", pid, "--exchange-min-notional", "5"]
    try: pr = subprocess.run(cmd, capture_output=True, text=True, timeout=900)
    except subprocess.TimeoutExpired: return None
    if pr.returncode != 0: return None
    try: r = json.loads(pr.stdout)
    except: return None
    o = r.get("on_budget", {})
    return {"ann": o.get("annualized_return_pct", -999), "dd": o.get("max_drawdown_pct", 999), "ret": o.get("total_return_pct", -999)}

def evaluate_one(spec):
    label, cfg, tmp_path = spec
    with open(tmp_path, "w") as f: json.dump(cfg, f)
    full = replay(tmp_path, 4999, FULL_START, FULL_END, f"r13p5_{label}")
    seg = {}
    for nm, s, e in FULL_SEGMENTS: seg[nm] = replay(tmp_path, 4999, s, e, f"r13p5_{label}_{nm}")
    try: os.remove(tmp_path)
    except: pass
    if full is None: return {"label": label, "skipped": True}
    pos = sum(1 for v in seg.values() if v and v["ret"] > 0)
    target_hit = []
    if full["ann"] >= 50 and full["dd"] <= 10 and pos >= 4: target_hit.append("conservative")
    if full["ann"] >= 90 and full["dd"] <= 20 and pos >= 4: target_hit.append("balanced")
    if full["ann"] >= 110 and full["dd"] <= 30 and pos >= 3: target_hit.append("aggressive")
    return {"label": label, "full_metrics": full, "segment_metrics": seg, "positive_segments": pos, "target_hit": target_hit}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=28)
    args = ap.parse_args()
    base = json.load(open(BASE_PATH))
    specs = []
    # Grid: tp01 × tp23 × red23 × tp4 × red4 = 3×3×2×4×3 = 216 per base
    # × 4 spacing variants = 864 total (cap at 768)
    for tp01 in [80, 120, 180]:
        for tp23 in [40, 70, 100]:
            for red23 in [0, 25]:
                for tp4 in [20, 40, 70]:
                    for red4 in [25, 50]:
                        for step in [120, 150, 180, 250]:
                            label = f"t01_{tp01}_t23_{tp23}_r{red23}_t4_{tp4}_r{red4}_s{step}"
                            cfg = apply_depth_tp(base, tp01, tp23, red23, tp4, red4)
                            for s in cfg["portfolio_config"]["strategies"]:
                                s["spacing"] = {"fixed_percent": {"step_bps": step}}
                            specs.append((label, cfg, f"/tmp/r13p5_{len(specs):04d}.json"))
                            if len(specs) >= 768: break
                        if len(specs) >= 768: break
                    if len(specs) >= 768: break
                if len(specs) >= 768: break
            if len(specs) >= 768: break
        if len(specs) >= 768: break

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
            if completed % 100 == 0:
                el = time.time() - t0
                print(f"  [{completed}/{len(specs)}] el={el:.0f}s", flush=True)

    valid = [r for r in results if not r.get("skipped")]
    valid.sort(key=lambda r: r["full_metrics"]["ann"], reverse=True)
    targets = [r for r in valid if r.get("target_hit")]
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": valid[:50], "target_hits": targets, "total_specs": len(specs), "total_evaluated": len(valid)}, open(args.out, "w"), indent=2)
    el = time.time() - t0
    print(f"\n[r13P5] wrote {args.out}: {len(valid)} eval, {len(targets)} targets in {el:.0f}s")
    print(f"\n=== TOP 10 by ann ===")
    for v in valid[:10]:
        fm = v["full_metrics"]
        print(f"  {v['label']:55s} ann={fm['ann']:7.1f} dd={fm['dd']:6.1f} pos={v['positive_segments']}/5")

if __name__ == "__main__":
    main()
