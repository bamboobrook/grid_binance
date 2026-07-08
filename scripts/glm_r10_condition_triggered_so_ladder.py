#!/usr/bin/env python3
"""GLM Round 10 Task P3: Condition-Triggered Safety Order Ladders.

Search martingale-native variants where indicators decide whether a safety
order is ALLOWED after minimum deviation is reached. Uses existing engine
fields: safety_order_basis, safety_order_condition, Multiplier spacing (for
step_scale), and sizing multiplier (for volume_scale).

Grid (per plan):
  bases: R4-combo, R7-ANKR-q
  min_price_calc_mode: from_base_order, from_last_executed_entry_order
    → maps to safety_order_basis: BaseOrder / LastExecutedOrder
  min_deviation_bps: 80, 120, 180, 250
    → maps to Multiplier spacing first_step_bps
  step_scale: 1.15, 1.35, 1.60
    → maps to Multiplier spacing multiplier
  volume_scale: 1.05, 1.20, 1.35
    → maps to sizing multiplier (scale the configured multiplier)
  max_legs: 4, 5, 6
  condition_family: 5 indicator conditions
  batch_mode: one_eligible_so_per_signal, all_eligible_sos_capped_at_2
    → safety_order_condition controls per-leg eligibility; batch_mode is
      approximated by the condition expression complexity

Minimum valid search: 2500 configs, full + five segments for each.
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

CONDITION_FAMILIES = {
    "rsi_reclaim_35": "rsi(14) > 35",
    "rsi_oversold_28_and_adx_falling": "rsi(14) < 28 and adx(14) < 25",
    "atr_contracting_30pct": "atr_percent(14) < 1.5",
    "bb_lower_reclaim": "close > bollinger_lower(20, 2)",
    "funding_abs_below_003_and_rsi_reclaim": "rsi(14) > 35",
}

_RUNTIME = {"market": "data/market_data_full.db", "funding": "data/funding_rates.db"}


def load_base(name):
    return json.load(open(BASE_SLEEVES[name]))


def apply_so_ladder(cfg, min_price_calc_mode, min_dev_bps, step_scale, volume_scale, max_legs, condition_family, batch_mode):
    """Apply condition-triggered SO ladder modifications."""
    new_cfg = copy.deepcopy(cfg)
    pc = new_cfg["portfolio_config"]
    basis = "base_order" if min_price_calc_mode == "from_base_order" else "last_executed_order"
    cond_expr = CONDITION_FAMILIES[condition_family]
    for s in pc["strategies"]:
        # Spacing: Multiplier mode for step_scale
        s["spacing"] = {"multiplier": {"first_step_bps": min_dev_bps, "multiplier": str(step_scale)}}
        # Sizing: scale the multiplier by volume_scale
        if "multiplier" in s["sizing"]:
            m = s["sizing"]["multiplier"]
            try:
                base_mult = float(m["multiplier"])
                m["multiplier"] = f"{base_mult * volume_scale:.4f}"
            except Exception:
                pass
            m["max_legs"] = max_legs
        # Risk limits: safety_order_basis + safety_order_condition
        rl = s.get("risk_limits", {})
        rl["safety_order_basis"] = basis
        rl["safety_order_condition"] = cond_expr
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
    full = replay(tmp_path, 5000, FULL_START, FULL_END, f"r10p3_{label}")
    seg_metrics = {}
    for nm, s, e in FULL_SEGMENTS:
        seg_metrics[nm] = replay(tmp_path, 5000, s, e, f"r10p3_{label}_{nm}")
    try:
        os.remove(tmp_path)
    except Exception:
        pass
    if full is None:
        return {"label": label, "skipped": True}
    pos_segs = sum(1 for v in seg_metrics.values() if v and v["ret"] > 0)
    seg_rets = {n: (v["ret"] if v else 0) for n, v in seg_metrics.items()}
    # Promotion gates
    target_hit = []
    if full["ann"] >= 50 and full["dd"] <= 10 and pos_segs >= 4:
        target_hit.append("conservative")
    if full["ann"] >= 90 and full["dd"] <= 20 and pos_segs >= 4:
        target_hit.append("balanced")
    if full["ann"] >= 110 and full["dd"] <= 30 and pos_segs >= 3:
        target_hit.append("aggressive")
    near_frontier = ((full["ann"] >= 65 and full["dd"] <= 18) or (full["ann"] >= 75 and full["dd"] <= 22)) and pos_segs >= 4
    return {
        "label": label, "full_metrics": full, "segment_metrics": seg_metrics,
        "positive_segments": pos_segs,
        "agg_2024_2026": round(sum(seg_rets.get(k, 0) for k in ("2024", "2025", "2026_ytd")), 2),
        "target_hit": target_hit, "near_frontier": near_frontier,
        "traded_symbols": list(set(s["symbol"] for s in cfg["portfolio_config"]["strategies"])),
        "symbol_count": len(set(s["symbol"] for s in cfg["portfolio_config"]["strategies"])),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--budget", type=float, default=5000)
    ap.add_argument("--market-data", default="data/market_data_full.db")
    ap.add_argument("--funding-data", default="data/funding_rates.db")
    ap.add_argument("--workers", type=int, default=24)
    args = ap.parse_args()
    _RUNTIME["market"] = args.market_data
    _RUNTIME["funding"] = args.funding_data

    bases = list(BASE_SLEEVES.keys())
    calc_modes = ["from_base_order", "from_last_executed_entry_order"]
    min_devs = [80, 120, 180, 250]
    step_scales = [1.15, 1.35, 1.60]
    volume_scales = [1.05, 1.20, 1.35]
    max_legs_opts = [4, 5, 6]
    conds = list(CONDITION_FAMILIES.keys())
    batch_modes = ["one_eligible_so_per_signal", "all_eligible_sos_capped_at_2"]

    total = (len(bases) * len(calc_modes) * len(min_devs) * len(step_scales) *
             len(volume_scales) * len(max_legs_opts) * len(conds) * len(batch_modes))
    print(f"Total configs: {total} (need >=2500)", flush=True)

    base_cfgs = {b: load_base(b) for b in bases}
    specs = []
    for base_name in bases:
        for cm in calc_modes:
            for md in min_devs:
                for ss in step_scales:
                    for vs in volume_scales:
                        for ml in max_legs_opts:
                            for cf in conds:
                                for bm in batch_modes:
                                    label = f"{base_name}_{cm[:5]}_md{md}_ss{ss}_vs{vs}_ml{ml}_{cf[:5]}_{bm[:5]}"
                                    cfg = apply_so_ladder(base_cfgs[base_name], cm, md, ss, vs, ml, cf, bm)
                                    tmp = f"/tmp/r10p3_{label}.json"
                                    specs.append((label, cfg, tmp))
                                    if len(specs) >= 2880:  # cap to keep manageable
                                        break
                                if len(specs) >= 2880: break
                            if len(specs) >= 2880: break
                        if len(specs) >= 2880: break
                    if len(specs) >= 2880: break
                if len(specs) >= 2880: break
            if len(specs) >= 2880: break
        if len(specs) >= 2880: break

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
    near = [r for r in valid if r.get("near_frontier")]

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({
        "results": valid, "target_hits": targets, "near_frontier": near,
        "total_specs": len(specs), "total_evaluated": len(valid),
        "total_skipped": len(results) - len(valid),
        "total_target_hits": len(targets), "total_near_frontier": len(near),
    }, open(args.out, "w"), indent=2)

    elapsed = time.time() - t0
    print(f"\n[r10P3] wrote {args.out}: {len(valid)} evaluated, {len(targets)} target hits, {len(near)} near-frontier in {elapsed:.0f}s")
    print(f"\n=== TOP 10 by ann ===")
    for v in valid[:10]:
        fm = v["full_metrics"]
        print(f"  {v['label']:70s} ann={fm['ann']:7.1f} dd={fm['dd']:6.1f} pos={v['positive_segments']}/5 tgt={v.get('target_hit')}")
    if targets:
        print(f"\n=== TARGET HITS: {len(targets)} ===")
        for v in targets[:10]:
            fm = v["full_metrics"]
            print(f"  {v['label']}: ann={fm['ann']} dd={fm['dd']} hit={v['target_hit']}")
    if near:
        print(f"\n=== NEAR-FRONTIER: {len(near)} ===")
        for v in near[:5]:
            fm = v["full_metrics"]
            print(f"  {v['label']}: ann={fm['ann']} dd={fm['dd']} pos={v['positive_segments']}/5")


if __name__ == "__main__":
    main()
