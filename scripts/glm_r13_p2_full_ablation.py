#!/usr/bin/env python3
"""GLM Round 13 Task P2: Complete HTF Trend-Directed Martingale Ablation.

5 ablations × up to 512 constrained configs each:
1. Direction gate only
2. SO scale only (extreme state)
3. ATR risk scale only
4. Direction + SO
5. Direction + SO + ATR

Bases: R4-combo (6 strategies) and TRX_L+AAVE_S (2 strategies, the R13 discovery).
Each config runs full + 5 segments. Pareto configs per ablation enter validation.
"""
import argparse, json, os, subprocess, sys, time, copy, hashlib, itertools
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates_round12.db"

FULL_SEGMENTS = [
    ("h1_2023", 1672531200000, 1688169599999),
    ("h2_2023", 1688169600000, 1704067199999),
    ("2024",    1704067200000, 1735689599999),
    ("2025",    1735689600000, 1767225599999),
    ("2026_ytd",1767225600000, 1780271999999),
]
FULL_START, FULL_END = 1672531200000, 1780271999999

BASE_PATH = "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json"

# WFO folds (train, validate)
WFO_FOLDS = [
    ("F1", 1672531200000, 1688169599999, 1688169600000, 1704067199999),  # h1→h2
    ("F2", 1672531200000, 1704067199999, 1704067200000, 1735689599999),  # 2023→2024
    ("F3", 1672531200000, 1735689599999, 1735689600000, 1767225599999),  # 2023-24→2025
    ("F4", 1672531200000, 1767225599999, 1767225600000, 1780271999999),  # 2023-25→2026ytd
]


def load_base():
    return json.load(open(BASE_PATH))


def make_2strat_base():
    """Create the R13-discovered 2-strategy base (TRX long + AAVE short)."""
    base = load_base()
    strategies = base["portfolio_config"]["strategies"]
    cfg = copy.deepcopy(base)
    cfg["portfolio_config"]["strategies"] = [strategies[1], strategies[3]]  # TRX_L, AAVE_S
    for s in cfg["portfolio_config"]["strategies"]:
        s["portfolio_weight_pct"] = "50.0"
    cfg["portfolio_config"]["risk_limits"]["max_global_budget_quote"] = "4999.00"
    return cfg


def apply_ablation(base_cfg, ablation, params):
    """Apply ablation-specific modifications."""
    cfg = copy.deepcopy(base_cfg)
    pc = cfg["portfolio_config"]
    ema_f = params.get("ema_fast", 50)
    ema_s = params.get("ema_slow", 200)
    adx_trend = params.get("adx_trend")
    adx_extreme = params.get("adx_extreme")
    so_scale = params.get("so_scale")
    fo_scale = params.get("fo_scale")
    atr_target = params.get("atr_target")
    step_bps = params.get("step_bps")
    tp_bps = params.get("tp_bps")

    for s in pc["strategies"]:
        triggers = [{"cooldown": {"seconds": 39600}}]

        # Direction gate (ablations 1, 4, 5)
        if ablation in ("direction", "dir_so", "dir_so_atr"):
            if s["direction"] == "long":
                triggers.append({"indicator_expression": {"expression": f"BTCUSDT.close > BTCUSDT.ema({ema_f})"}})
                triggers.append({"indicator_expression": {"expression": f"BTCUSDT.ema({ema_f}) > BTCUSDT.ema({ema_s})"}})
            elif s["direction"] == "short":
                triggers.append({"indicator_expression": {"expression": f"BTCUSDT.close < BTCUSDT.ema({ema_f})"}})
                triggers.append({"indicator_expression": {"expression": f"BTCUSDT.ema({ema_f}) < BTCUSDT.ema({ema_s})"}})

        s["entry_triggers"] = triggers

        rl = s.get("risk_limits", {})

        # SO scale (ablations 2, 4, 5)
        if ablation in ("so_scale", "dir_so", "dir_so_atr") and so_scale is not None:
            trigger_dd = params.get("dd_trigger", 10.0)
            rl["drawdown_state_rules"] = [{
                "trigger_drawdown_pct": trigger_dd,
                "safety_order_scale": so_scale,
                "first_order_scale": None,
                "cooldown_multiplier": None,
                "freeze_safety_orders": None,
            }]

        # ATR risk scale (ablations 3, 5)
        if ablation in ("atr_scale", "dir_so_atr") and atr_target is not None:
            rl["vol_target_atr_pct"] = atr_target
            rl["vol_target_min_scale"] = 0.5
            rl["vol_target_max_scale"] = 1.5

        # ADX skip for extreme (all ablations that include SO)
        if adx_extreme is not None:
            rl["safety_skip_adx_threshold"] = adx_extreme

        s["risk_limits"] = rl

        # Step/TP variants
        if step_bps is not None:
            s["spacing"] = {"fixed_percent": {"step_bps": step_bps}}
        if tp_bps is not None:
            s["take_profit"] = {"percent": {"bps": tp_bps}}

    return cfg


def replay(config_path, budget, s, e, pid):
    cmd = [REPLAY, "--config", config_path, "--budget", str(budget),
           "--start-ms", str(s), "--end-ms", str(e),
           "--market-data", MARKET_DB, "--funding-data", FUNDING_DB,
           "--profile", "aggressive", "--portfolio-id", pid,
           "--exchange-min-notional", "5"]
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
    full = replay(tmp_path, 4999, FULL_START, FULL_END, f"r13p2f_{label}")
    seg = {}
    for nm, s, e in FULL_SEGMENTS: seg[nm] = replay(tmp_path, 4999, s, e, f"r13p2f_{label}_{nm}")
    try: os.remove(tmp_path)
    except: pass
    if full is None: return {"label": label, "skipped": True}
    pos = sum(1 for v in seg.values() if v and v["ret"] > 0)
    return {"label": label, "full_metrics": full, "segment_metrics": seg,
            "positive_segments": pos, "skipped": False}


def generate_ablation_configs(base_cfg, ablation, max_configs=512):
    """Generate configs for one ablation."""
    configs = []
    params_grid = []

    if ablation == "direction":
        for ema_f, ema_s in [(50,200),(100,300)]:
            for step in [120,150,180,220,280]:
                for tp in [80,100,130,180]:
                    params_grid.append({"ema_fast":ema_f,"ema_slow":ema_s,"step_bps":step,"tp_bps":tp})

    elif ablation == "so_scale":
        for so_scale in [0.25,0.5,0.75]:
            for dd_trigger in [5.0,10.0,15.0,20.0]:
                for adx_extreme in [40.0,50.0,60.0]:
                    for step in [120,150,180]:
                        params_grid.append({"so_scale":so_scale,"dd_trigger":dd_trigger,
                                           "adx_extreme":adx_extreme,"step_bps":step})

    elif ablation == "atr_scale":
        for atr_target in [1.0,1.5,2.0,2.5,3.0]:
            for step in [120,150,180,220]:
                for tp in [80,100,130,180]:
                    for mult in [1.15,1.35,1.55,1.8]:
                        params_grid.append({"atr_target":atr_target,"step_bps":step,"tp_bps":tp})

    elif ablation == "dir_so":
        for ema_f, ema_s in [(50,200),(100,300)]:
            for so_scale in [0.25,0.5,0.75]:
                for adx_extreme in [40.0,50.0]:
                    for step in [120,150,180]:
                        for dd_trigger in [10.0,15.0]:
                            params_grid.append({"ema_fast":ema_f,"ema_slow":ema_s,"so_scale":so_scale,
                                               "adx_extreme":adx_extreme,"step_bps":step,"dd_trigger":dd_trigger})

    elif ablation == "dir_so_atr":
        for ema_f in [50,100]:
            for so_scale in [0.5,0.75]:
                for atr_target in [1.5,2.0]:
                    for step in [120,150,180]:
                        for tp in [100,130]:
                            params_grid.append({"ema_fast":ema_f,"ema_slow":200,"so_scale":so_scale,
                                               "atr_target":atr_target,"step_bps":step,"tp_bps":tp,
                                               "adx_extreme":50.0,"dd_trigger":10.0})

    # Cap at max_configs
    params_grid = params_grid[:max_configs]

    for i, params in enumerate(params_grid):
        cfg = apply_ablation(base_cfg, ablation, params)
        label = f"{ablation}_{i:04d}"
        configs.append((label, cfg, f"/tmp/r13p2f_{label}.json"))

    return configs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=28)
    ap.add_argument("--max-per-ablation", type=int, default=512)
    args = ap.parse_args()

    ablations = ["direction", "so_scale", "atr_scale", "dir_so", "dir_so_atr"]
    bases = {
        "2strat": make_2strat_base(),
        "r4combo": load_base(),
    }

    all_results = {}
    t_total = time.time()

    for base_name, base_cfg in bases.items():
        for ablation in ablations:
            configs = generate_ablation_configs(base_cfg, ablation, args.max_per_ablation)
            key = f"{base_name}_{ablation}"
            print(f"\n=== {key}: {len(configs)} configs ===", flush=True)

            t0 = time.time()
            results = []
            completed = 0
            with ProcessPoolExecutor(max_workers=args.workers) as ex:
                futures = {ex.submit(evaluate_one, spec): spec[0] for spec in configs}
                for fut in as_completed(futures):
                    try: r = fut.result()
                    except Exception as e: r = {"label": futures[fut], "skipped": True, "error": str(e)}
                    results.append(r)
                    completed += 1
                    if completed % 100 == 0:
                        el = time.time() - t0
                        rate = completed / el if el > 0 else 0
                        print(f"  [{completed}/{len(configs)}] el={el:.0f}s rate={rate:.1f}/s", flush=True)

            valid = [r for r in results if not r.get("skipped")]
            valid.sort(key=lambda r: r["full_metrics"]["ann"], reverse=True)
            all_results[key] = {
                "total_configs": len(configs),
                "evaluated": len(valid),
                "skipped": len(results) - len(valid),
                "top5": [{
                    "label": v["label"],
                    "ann": v["full_metrics"]["ann"],
                    "dd": v["full_metrics"]["dd"],
                    "pos_segs": v["positive_segments"],
                } for v in valid[:5]],
                "best": valid[0] if valid else None,
                "elapsed_s": time.time() - t0,
            }
            if valid:
                best = valid[0]
                fm = best["full_metrics"]
                print(f"  BEST: ann={fm['ann']:.1f}% dd={fm['dd']:.1f}% pos={best['positive_segments']}/5", flush=True)

            # Commit each ablation as checkpoint
            print(f"  {key} done in {time.time()-t0:.0f}s", flush=True)

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump(all_results, open(args.out, "w"), indent=2)
    print(f"\n[r13P2-full] wrote {args.out} in {time.time()-t_total:.0f}s total")
    print(f"\n=== SUMMARY ===")
    for key, data in all_results.items():
        if data["best"]:
            b = data["best"]
            print(f"  {key}: {data['evaluated']} eval, best ann={b['full_metrics']['ann']:.1f}% dd={b['full_metrics']['dd']:.1f}% pos={b['positive_segments']}/5")
        else:
            print(f"  {key}: {data['evaluated']} eval, no valid results")


if __name__ == "__main__":
    main()
