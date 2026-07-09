#!/usr/bin/env python3
"""GLM Round 10 Task P4: Multi-TP With Final Trailing Runner.

Search TP stage ladders where the final stage activates a trailing runner
that lets profits run beyond the fixed TP target. Uses the existing
trailing_lock_* fields (R6 Task D) which arm a trailing profit lock after
a configured partial TP stage.

Grid (per plan):
  bases: R4-combo, R7-ANKR-q
  tp_stage_sets: 3 ladder shapes
  final_trailing_activation_bps: 80, 120, 180, 250
  final_trailing_deviation_bps: 25, 40, 65, 90
  breakeven_after_stage: 1, 2
  breakeven_buffer_bps: 5, 15, 30
  time_limit_days: none, 3, 7, 14 (approximated via no_progress_exit_hours)

Minimum valid search: 1200 configs, full + five segments.
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
}

# 3 TP ladder shapes (num, den, tp_bps) per stage; final stage is trailing
TP_STAGE_SETS = {
    "30_40_30_final_trail": [[300, 1000, 450], [400, 1000, 900], [300, 1000, 0]],  # final=trailing
    "25_25_25_25_final_trail": [[250, 1000, 400], [250, 1000, 750], [250, 1000, 1100], [250, 1000, 0]],
    "50_25_25_final_trail": [[500, 1000, 550], [250, 1000, 1000], [250, 1000, 0]],
}

_RUNTIME = {"market": "data/market_data_full.db", "funding": "data/funding_rates.db"}


def load_base(name):
    return json.load(open(BASE_SLEEVES[name]))


def apply_multi_tp_trailing(cfg, tp_set_name, trail_act_bps, trail_dev_bps, be_after_stage, be_buffer_bps, time_limit_days):
    """Apply multi-TP with final trailing runner."""
    new_cfg = copy.deepcopy(cfg)
    pc = new_cfg["portfolio_config"]
    stages = TP_STAGE_SETS[tp_set_name]
    # Convert stages to tuples (json arrays)
    stages_json = [[int(a), int(b), int(c)] for a, b, c in stages]
    # time_limit_days → no_progress_exit_hours
    no_progress_hours = None if time_limit_days == "none" else time_limit_days * 24
    for s in pc["strategies"]:
        # Partial TP with the chosen ladder
        s["take_profit"] = {
            "partial": {
                "stages": stages_json,
                "breakeven_after_stage": be_after_stage,
                "breakeven_buffer_bps": be_buffer_bps,
            }
        }
        rl = s.get("risk_limits", {})
        # Final trailing runner via trailing_lock fields
        rl["trailing_lock_after_stage"] = len(stages) - 1  # arm on final stage
        rl["trailing_lock_activation_bps"] = trail_act_bps
        rl["trailing_lock_callback_bps"] = trail_dev_bps
        rl["trailing_lock_floor_bps"] = be_buffer_bps  # floor at breakeven
        # time limit
        if no_progress_hours is not None:
            rl["no_progress_exit_hours"] = no_progress_hours
        s["risk_limits"] = rl
    return new_cfg


def replay(config_path, budget, s, e, pid):
    cmd = [REPLAY, "--config", config_path, "--budget", str(budget),
           "--start-ms", str(s), "--end-ms", str(e),
           "--market-data", _RUNTIME["market"], "--funding-data", _RUNTIME["funding"],
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
    return {"ann": o.get("annualized_return_pct", -999), "dd": o.get("max_drawdown_pct", 999),
            "ret": o.get("total_return_pct", -999), "trades": r.get("trade_count", 0)}


def evaluate_one(spec):
    label, cfg, tmp_path = spec
    with open(tmp_path, "w") as f:
        json.dump(cfg, f)
    full = replay(tmp_path, 5000, FULL_START, FULL_END, f"r10p4_{label}")
    seg_metrics = {}
    for nm, s, e in FULL_SEGMENTS:
        seg_metrics[nm] = replay(tmp_path, 5000, s, e, f"r10p4_{label}_{nm}")
    try:
        os.remove(tmp_path)
    except Exception:
        pass
    if full is None:
        return {"label": label, "skipped": True}
    pos_segs = sum(1 for v in seg_metrics.values() if v and v["ret"] > 0)
    target_hit = []
    if full["ann"] >= 50 and full["dd"] <= 10 and pos_segs >= 4:
        target_hit.append("conservative")
    if full["ann"] >= 90 and full["dd"] <= 20 and pos_segs >= 4:
        target_hit.append("balanced")
    if full["ann"] >= 110 and full["dd"] <= 30 and pos_segs >= 3:
        target_hit.append("aggressive")
    return {
        "label": label, "full_metrics": full, "segment_metrics": seg_metrics,
        "positive_segments": pos_segs, "target_hit": target_hit,
        "traded_symbols": list(set(s["symbol"] for s in cfg["portfolio_config"]["strategies"])),
        "symbol_count": len(set(s["symbol"] for s in cfg["portfolio_config"]["strategies"])),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--budget", type=float, default=5000)
    ap.add_argument("--market-data", default="data/market_data_full.db")
    ap.add_argument("--funding-data", default="data/funding_rates.db")
    ap.add_argument("--workers", type=int, default=26)
    args = ap.parse_args()
    _RUNTIME["market"] = args.market_data
    _RUNTIME["funding"] = args.funding_data

    bases = list(BASE_SLEEVES.keys())
    total = (len(bases) * len(TP_STAGE_SETS) * 4 * 4 * 2 * 3 * 4)
    print(f"Total configs: {total} (need >=1200)", flush=True)

    base_cfgs = {b: load_base(b) for b in bases}
    specs = []
    for base_name in bases:
        for tp_set in TP_STAGE_SETS:
            for trail_act in [80, 120, 180, 250]:
                for trail_dev in [25, 40, 65, 90]:
                    for be_after in [1, 2]:
                        for be_buf in [5, 15, 30]:
                            for tl in ["none", 3, 7, 14]:
                                tl_str = "none" if tl == "none" else f"{tl}d"
                                label = f"{base_name}_{tp_set[:8]}_ta{trail_act}_td{trail_dev}_be{be_after}_bb{be_buf}_tl{tl_str}"
                                cfg = apply_multi_tp_trailing(base_cfgs[base_name], tp_set, trail_act, trail_dev, be_after, be_buf, tl)
                                tmp = f"/tmp/r10p4_{label}.json"
                                specs.append((label, cfg, tmp))

    print(f"Generated {len(specs)} specs. Running full + 5 segments each...", flush=True)
    t0 = time.time()
    results = []
    completed = 0
    with ProcessPoolExecutor(max_workers=args.workers) as ex:
        futures = {ex.submit(evaluate_one, spec): spec[0] for spec in specs}
        for fut in as_completed(futures):
            try:
                r = fut.result()
            except Exception as e:
                r = {"label": futures[fut], "skipped": True, "error": str(e)}
            results.append(r)
            completed += 1
            if completed % 100 == 0:
                elapsed = time.time() - t0
                rate = completed / elapsed if elapsed > 0 else 0
                eta = (len(specs) - completed) / rate if rate > 0 else 0
                print(f"  [{completed}/{len(specs)}] elapsed={elapsed:.0f}s rate={rate:.2f}/s eta={eta:.0f}s", flush=True)

    valid = [r for r in results if not r.get("skipped")]
    valid.sort(key=lambda r: r["full_metrics"]["ann"], reverse=True)
    targets = [r for r in valid if r.get("target_hit")]

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({
        "results": valid, "target_hits": targets,
        "total_specs": len(specs), "total_evaluated": len(valid),
        "total_skipped": len(results) - len(valid),
        "total_target_hits": len(targets),
    }, open(args.out, "w"), indent=2)

    elapsed = time.time() - t0
    print(f"\n[r10P4] wrote {args.out}: {len(valid)} evaluated, {len(targets)} target hits in {elapsed:.0f}s")
    print(f"\n=== TOP 10 by ann ===")
    for v in valid[:10]:
        fm = v["full_metrics"]
        print(f"  {v['label']:70s} ann={fm['ann']:7.1f} dd={fm['dd']:6.1f} pos={v['positive_segments']}/5")
    if targets:
        print(f"\n=== TARGET HITS: {len(targets)} ===")
        for v in targets[:10]:
            fm = v["full_metrics"]
            print(f"  {v['label']}: ann={fm['ann']} dd={fm['dd']} hit={v['target_hit']}")


if __name__ == "__main__":
    main()
