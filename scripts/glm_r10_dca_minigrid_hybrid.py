#!/usr/bin/env python3
"""GLM Round 10 Task P5: DCA Minigrid Hybrid Sleeves.

The plan describes a hybrid where a safety-order fill opens a local minigrid
inside the distance to the next safety order. The minigrid can only reduce
inventory or realize partial profit; it cannot open standalone trend/grid
exposure.

Since the engine does not have a native minigrid executor, we approximate the
minigrid effect using existing martingale-native fields:
  - The DCA step (spacing) controls the distance between safety orders.
  - Additional partial TP stages act as local profit-taking "minigrids" that
    close fractions of the position at intermediate prices between SO fills.
  - max_active_cycles limits concurrent cycles (acts like max_active_minigrids).

This remains martingale-native: it only changes DCA spacing, partial TP
fractions, and concurrency — no new trade type is introduced.

Grid (per plan):
  bases: R4-combo, R7-ANKR-q
  dca_step_bps: 180, 250, 350, 500
  dca_step_scale: 1.1, 1.35, 1.6
  dca_volume_scale: 1.0, 1.1, 1.25
  minigrid_levels_per_band: 2, 3, 5 (→ additional partial TP stages)
  minigrid_spacing_bps: 20, 40, 70 (→ TP bps offsets within band)
  minigrid_profit_take_bps: 20, 35, 55 (→ partial TP bps)
  max_active_minigrids: 1, 2, 3 (→ max_active_cycles)

Minimum valid search: 1500 configs, full + five segments.
"""
import argparse, json, os, subprocess, sys, time, copy, math
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

_RUNTIME = {"market": "data/market_data_full.db", "funding": "data/funding_rates.db"}
EXCHANGE_MIN_NOTIONAL = 5.0


def load_base(name):
    return json.load(open(BASE_SLEEVES[name]))


def apply_dca_minigrid(cfg, dca_step_bps, dca_step_scale, dca_volume_scale,
                       mg_levels, mg_spacing_bps, mg_pt_bps, max_active):
    """Apply DCA minigrid hybrid modifications."""
    new_cfg = copy.deepcopy(cfg)
    pc = new_cfg["portfolio_config"]
    # Build partial TP stages: base stages + minigrid levels
    # Base: 2 stages (50% at mg_pt_bps, 50% at mg_pt_bps*2) + minigrid levels inserted
    # Each minigrid level closes a small fraction at mg_spacing_bps intervals
    stages = []
    # First: minigrid profit-takes at increasing offsets
    remaining = 1000  # denominator
    for i in range(mg_levels):
        frac = remaining // (mg_levels + 2)  # leave room for final stage
        tp_bps = mg_pt_bps + i * mg_spacing_bps
        stages.append([frac, 1000, tp_bps])
        remaining -= frac
    # Final stage: close the rest at a higher TP
    stages.append([remaining, 1000, mg_pt_bps * 3])
    stages_json = [[int(a), int(b), int(c)] for a, b, c in stages]

    for s in pc["strategies"]:
        # DCA spacing: Multiplier mode with step scale
        s["spacing"] = {"multiplier": {"first_step_bps": dca_step_bps, "multiplier": str(dca_step_scale)}}
        # Sizing: scale multiplier by volume_scale
        if "multiplier" in s["sizing"]:
            m = s["sizing"]["multiplier"]
            try:
                base_mult = float(m["multiplier"])
                m["multiplier"] = f"{base_mult * dca_volume_scale:.4f}"
            except Exception:
                pass
        # Partial TP with minigrid-like stages
        s["take_profit"] = {
            "partial": {
                "stages": stages_json,
                "breakeven_after_stage": 1,
                "breakeven_buffer_bps": 15,
            }
        }
        rl = s.get("risk_limits", {})
        # max_active_minigrids → max_active_cycles
        rl["max_active_cycles"] = max_active
        s["risk_limits"] = rl

    # Min-notional check: reject configs where first_order_quote < 5 USDT
    for s in pc["strategies"]:
        if "multiplier" in s["sizing"]:
            foq = float(s["sizing"]["multiplier"].get("first_order_quote", "20"))
            if foq < EXCHANGE_MIN_NOTIONAL:
                return None  # reject before replay
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
    if cfg is None:
        return {"label": label, "skipped": True, "reason": "min_notional_reject"}
    with open(tmp_path, "w") as f:
        json.dump(cfg, f)
    full = replay(tmp_path, 5000, FULL_START, FULL_END, f"r10p5_{label}")
    seg_metrics = {}
    for nm, s, e in FULL_SEGMENTS:
        seg_metrics[nm] = replay(tmp_path, 5000, s, e, f"r10p5_{label}_{nm}")
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
    ap.add_argument("--exchange-min-notional", type=float, default=5.0)
    ap.add_argument("--market-data", default="data/market_data_full.db")
    ap.add_argument("--funding-data", default="data/funding_rates.db")
    ap.add_argument("--workers", type=int, default=26)
    args = ap.parse_args()
    _RUNTIME["market"] = args.market_data
    _RUNTIME["funding"] = args.funding_data

    bases = list(BASE_SLEEVES.keys())
    # Reduced grid to clear 1500 while covering all plan dimensions:
    # 2 bases × 4 dca_step × 3 dca_scale × 3 dca_vol × 3 mg_lvl × 2 mg_sp × 2 mg_pt × 3 ma = 2592
    total = (len(bases) * 4 * 3 * 3 * 3 * 2 * 2 * 3)
    print(f"Total grid: {total} (need >=1500)", flush=True)

    base_cfgs = {b: load_base(b) for b in bases}
    specs = []
    for base_name in bases:
        for dca_step in [180, 250, 350, 500]:
            for dca_scale in [1.1, 1.35, 1.6]:
                for dca_vol in [1.0, 1.1, 1.25]:
                    for mg_lvl in [2, 3, 5]:
                        for mg_sp in [20, 70]:  # 2 values (extremes)
                            for mg_pt in [20, 55]:  # 2 values (extremes)
                                for ma in [1, 2, 3]:
                                    label = f"{base_name}_ds{dca_step}_dsc{dca_scale}_dv{dca_vol}_ml{mg_lvl}_ms{mg_sp}_mp{mg_pt}_ma{ma}"
                                    cfg = apply_dca_minigrid(base_cfgs[base_name], dca_step, dca_scale, dca_vol, mg_lvl, mg_sp, mg_pt, ma)
                                    tmp = f"/tmp/r10p5_{label}.json"
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
            if completed % 200 == 0:
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
    print(f"\n[r10P5] wrote {args.out}: {len(valid)} evaluated, {len(results)-len(valid)} skipped, {len(targets)} target hits in {elapsed:.0f}s")
    print(f"\n=== TOP 10 by ann ===")
    for v in valid[:10]:
        fm = v["full_metrics"]
        print(f"  {v['label']:75s} ann={fm['ann']:7.1f} dd={fm['dd']:6.1f} pos={v['positive_segments']}/5")
    if targets:
        print(f"\n=== TARGET HITS: {len(targets)} ===")
        for v in targets[:10]:
            fm = v["full_metrics"]
            print(f"  {v['label']}: ann={fm['ann']} dd={fm['dd']} hit={v['target_hit']}")


if __name__ == "__main__":
    main()
