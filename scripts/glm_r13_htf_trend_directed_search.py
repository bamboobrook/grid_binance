#!/usr/bin/env python3
"""GLM Round 13 Task P2: HTF Trend-Directed Martingale Ablation Search.

Uses entry_triggers with EMA/ADX conditions to gate long/short directions.
The engine already supports indicator_expression triggers — we use them to
implement the HTF state machine:
  LONG_TREND: BTC.close > BTC.ema(50) AND BTC.ema(50) > BTC.ema(200)
  SHORT_TREND: BTC.close < BTC.ema(50) AND BTC.ema(50) < BTC.ema(200)

Ablations:
1. Direction gate only (long when BTC uptrend, short when downtrend)
2. Direction + ADX-based SO scale
3. Direction + ATR-based first-order scale
4. Full combination

Uses frozen Round12 funding DB. R4-combo symbols as base.
"""
import argparse, json, os, subprocess, sys, time, copy
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

def load_base():
    return json.load(open(BASE_PATH))

def apply_htf_direction_gate(cfg, ema_fast, ema_slow, adx_threshold=None, so_scale_extreme=None):
    """Apply HTF direction gate via entry_triggers.
    Long strategies only open when BTC is in uptrend.
    Short strategies only open when BTC is in downtrend.
    """
    new_cfg = copy.deepcopy(cfg)
    pc = new_cfg["portfolio_config"]
    for s in pc["strategies"]:
        direction = s["direction"]
        triggers = [{"cooldown": {"seconds": 39600}}]

        if direction == "long":
            # Only open longs when BTC is above EMA (uptrend)
            triggers.append({"indicator_expression": {"expression": f"BTCUSDT.close > BTCUSDT.ema({ema_fast})"}})
            triggers.append({"indicator_expression": {"expression": f"BTCUSDT.ema({ema_fast}) > BTCUSDT.ema({ema_slow})"}})
        elif direction == "short":
            # Only open shorts when BTC is below EMA (downtrend)
            triggers.append({"indicator_expression": {"expression": f"BTCUSDT.close < BTCUSDT.ema({ema_fast})"}})
            triggers.append({"indicator_expression": {"expression": f"BTCUSDT.ema({ema_fast}) < BTCUSDT.ema({ema_slow})"}})

        s["entry_triggers"] = triggers

        # ADX-based SO scale for extreme trends
        rl = s.get("risk_limits", {})
        if adx_threshold is not None:
            rl["safety_skip_adx_threshold"] = adx_threshold
        if so_scale_extreme is not None:
            rl["drawdown_state_rules"] = [{
                "trigger_drawdown_pct": 10.0,
                "safety_order_scale": so_scale_extreme,
                "first_order_scale": None,
                "cooldown_multiplier": None,
                "freeze_safety_orders": None,
            }]
        s["risk_limits"] = rl
    return new_cfg

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
    full = replay(tmp_path, 4999, FULL_START, FULL_END, f"r13p2_{label}")
    seg = {}
    for nm, s, e in FULL_SEGMENTS: seg[nm] = replay(tmp_path, 4999, s, e, f"r13p2_{label}_{nm}")
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
    ap.add_argument("--workers", type=int, default=28)
    args = ap.parse_args()

    base = load_base()
    specs = []

    # Ablation 1: Direction gate only
    for ema_f, ema_s in [(50, 200), (100, 300)]:
        label = f"dir_only_ema{ema_f}_{ema_s}"
        cfg = apply_htf_direction_gate(base, ema_f, ema_s)
        specs.append((label, cfg, f"/tmp/r13p2_{label}.json"))

    # Ablation 2: Direction + ADX SO scale
    for ema_f, ema_s in [(50, 200)]:
        for adx in [25, 35, 50]:
            for so_scale in [0.5, 0.75]:
                label = f"dir_adx{adx}_so{so_scale}_ema{ema_f}"
                cfg = apply_htf_direction_gate(base, ema_f, ema_s, adx, so_scale)
                specs.append((label, cfg, f"/tmp/r13p2_{label}.json"))

    # Ablation 3: Direction + different step_bps
    for ema_f, ema_s in [(50, 200)]:
        for step in [120, 150, 180, 250]:
            label = f"dir_step{step}_ema{ema_f}"
            cfg = apply_htf_direction_gate(base, ema_f, ema_s)
            for s in cfg["portfolio_config"]["strategies"]:
                s["spacing"] = {"fixed_percent": {"step_bps": step}}
            specs.append((label, cfg, f"/tmp/r13p2_{label}.json"))

    # Ablation 4: Direction + TP variants
    for tp0 in [300, 450, 600]:
        for tp1 in [800, 1000, 1400]:
            label = f"dir_tp{tp0}_{tp1}"
            cfg = apply_htf_direction_gate(base, 50, 200)
            for s in cfg["portfolio_config"]["strategies"]:
                s["take_profit"] = {"partial": {"stages": [[300,1000,tp0],[400,1000,tp1],[1,1,2600]], "breakeven_after_stage": 1, "breakeven_buffer_bps": 100}}
            specs.append((label, cfg, f"/tmp/r13p2_{label}.json"))

    # Ablation 5: No direction gate (baseline for comparison)
    label = "baseline_no_gate"
    specs.append((label, copy.deepcopy(base), f"/tmp/r13p2_{label}.json"))

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
            if completed % 20 == 0:
                el = time.time() - t0
                print(f"  [{completed}/{len(specs)}] el={el:.0f}s", flush=True)

    valid = [r for r in results if not r.get("skipped")]
    valid.sort(key=lambda r: r["full_metrics"]["ann"], reverse=True)
    targets = [r for r in valid if r.get("target_hit")]
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": valid, "target_hits": targets, "total_specs": len(specs),
               "total_evaluated": len(valid)}, open(args.out, "w"), indent=2)
    el = time.time() - t0
    print(f"\n[r13P2] wrote {args.out}: {len(valid)} eval, {len(targets)} targets in {el:.0f}s")
    print(f"\n=== TOP 10 by ann ===")
    for v in valid[:10]:
        fm = v["full_metrics"]
        print(f"  {v['label']:50s} ann={fm['ann']:7.1f} dd={fm['dd']:6.1f} pos={v['positive_segments']}/5")
    print(f"\n=== Baseline (no gate) ===")
    for v in valid:
        if "baseline" in v["label"]:
            fm = v["full_metrics"]
            print(f"  {v['label']}: ann={fm['ann']:.1f} dd={fm['dd']:.1f} pos={v['positive_segments']}/5")

if __name__ == "__main__":
    main()
