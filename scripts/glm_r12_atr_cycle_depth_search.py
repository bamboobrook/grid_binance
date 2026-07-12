#!/usr/bin/env python3
"""Round 12 partial-TP stage ladder and max-age search.

Binding probes confirmed:
- ATR spacing binds but produces ann -4.74% (worse than R4-combo's 34.73%)
- Fixed-percent step binds and R4-combo's 150bps is well-tuned

Despite the historical filename, partial TP stages advance after TP fills, not
after safety-leg depth changes. This script does not implement cycle-depth TP;
it searches ordinary partial-TP stages, spacing, and max-cycle-age exits.
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

BASE_PATH = "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json"
_RUNTIME = {"market": "data/market_data_full.db", "funding": "data/funding_rates_round12.db"}

def apply_partial_tp_ladder(cfg, step_bps, stage0_bps, stage1_bps, stage2_bps, max_age_hours):
    """Apply ordinary partial-TP stage and max-cycle-age variants."""
    new_cfg = copy.deepcopy(cfg)
    pc = new_cfg["portfolio_config"]
    for s in pc["strategies"]:
        if step_bps != 150:
            s["spacing"] = {"fixed_percent": {"step_bps": step_bps}}
        rl = s.get("risk_limits", {})
        if max_age_hours is not None:
            rl["max_cycle_age_hours"] = max_age_hours
        s["risk_limits"] = rl
        s["take_profit"] = {"partial": {
            "stages": [
                [300, 1000, stage0_bps],
                [400, 1000, stage1_bps],
                [300, 1000, stage2_bps],
            ],
            "breakeven_after_stage": 1,
            "breakeven_buffer_bps": 15,
        }}
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
    full = replay(tmp_path, 4999, FULL_START, FULL_END, f"r12p6_{label}")
    seg = {}
    for nm, s, e in FULL_SEGMENTS: seg[nm] = replay(tmp_path, 4999, s, e, f"r12p6_{label}_{nm}")
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
    ap.add_argument("--workers", type=int, default=24)
    args = ap.parse_args()

    base_cfg = json.load(open(BASE_PATH))
    specs = []
    # 4 spacing x 4 stage0 x 3 stage1 x 3 stage2 x 4 ages = 576.
    for step in [120, 150, 180, 220]:
        for tp0 in [300, 450, 600, 800]:
            for tp2 in [300, 450, 600]:
                for tp4 in [200, 350, 500]:
                    for age in [None, 48, 120, 240]:
                        label = f"s{step}_p0_{tp0}_p1_{tp2}_p2_{tp4}_a{age if age else 'none'}"
                        cfg = apply_partial_tp_ladder(base_cfg, step, tp0, tp2, tp4, age)
                        specs.append((label, cfg, f"/tmp/r12p6_{label}.json"))

    print(f"Generated {len(specs)} specs. Running full + 5 segments each...", flush=True)
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
                rate = completed / el if el > 0 else 0
                print(f"  [{completed}/{len(specs)}] el={el:.0f}s rate={rate:.2f}/s", flush=True)

    valid = [r for r in results if not r.get("skipped")]
    valid.sort(key=lambda r: r["full_metrics"]["ann"], reverse=True)
    targets = [r for r in valid if r.get("target_hit")]
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"mechanism": "partial_tp_stage_ladder_plus_max_cycle_age",
               "cycle_depth_aware": False,
               "results": valid, "target_hits": targets, "total_specs": len(specs),
               "total_evaluated": len(valid), "total_skipped": len(results)-len(valid),
               "total_target_hits": len(targets)}, open(args.out, "w"), indent=2)
    el = time.time() - t0
    print(f"\n[r12P6] wrote {args.out}: {len(valid)} eval, {len(targets)} targets in {el:.0f}s")
    print(f"\n=== TOP 10 by ann ===")
    for v in valid[:10]:
        fm = v["full_metrics"]
        print(f"  {v['label']:55s} ann={fm['ann']:7.1f} dd={fm['dd']:6.1f} pos={v['positive_segments']}/5")

if __name__ == "__main__":
    main()
