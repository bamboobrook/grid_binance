#!/usr/bin/env python3
"""GLM Round 9 Task P4: DCA Reserve And Dynamic Stake Buffer Search.

Inspired by Freqtrade position-adjustment / custom-stake callbacks and Gainium
safety-order volume scaling. Remains martingale-native: only changes base/safety
order sizing and capital reserve behavior.

Search dimensions (per plan):
  bases: R4-combo, R6-QB, R7-ANKR-q (P3 best sleeves added when P3 done)
  reserve_pct: 10, 20, 30, 40
  first_order_scale_when_atr_gt_2pct: 0.5, 0.7, 1.0
  late_safety_reserve_release_after_leg: 2, 3, 4
  release_condition: rsi_reclaim_35, adx_falling_below_25, volatility_contracting_30pct
  max_active_symbols: 3, 4, 5

Minimum valid search size: 720 configs. Each runs full + 5 segments.
"""
import argparse, json, os, subprocess, sys, time, math, copy
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

# Base sleeve configs (P3 best added dynamically if available)
BASE_SLEEVES = {
    "R4-combo": "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json",
    "R6-QB":    "docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-QB-best.json",
    "R7-ANKR-q": "docs/superpowers/artifacts/glm-martingale-core-round7/promising/r7-ANKR-q1w24p24.json",
}

_RUNTIME = {"market": "data/market_data_full.db", "funding": "data/funding_rates.db"}


def load_base(name):
    path = BASE_SLEEVES[name]
    return json.load(open(path))


def apply_reserve_and_stake_buffer(cfg, reserve_pct, first_order_scale_atr,
                                     release_after_leg, release_condition,
                                     max_active_symbols):
    """Apply reserve and stake-buffer modifications to a base config.

    - reserve_pct: scales first_order_quote down by (1 - reserve/100), keeping
      the unused fraction as wallet reserve (per Freqtrade position_adjustment).
    - first_order_scale_atr: sets vol_target_atr_pct=2.0 with max_scale=1.0 and
      min_scale=first_order_scale_atr — so when ATR% > 2%, first order shrinks
      to first_order_scale_atr.
    - release_after_leg: sets taper_safety_after_leg to release the reserve
      into late safety orders (scaled by 1.0 — full release).
    - release_condition: encoded as a safety_order_condition expression.
    - max_active_symbols: sets max_active_cycles on the portfolio risk limits.
    """
    new_cfg = copy.deepcopy(cfg)
    pc = new_cfg["portfolio_config"]
    rl = pc.get("risk_limits", {})

    # Reserve: scale first_order_quote down
    reserve_factor = 1.0 - reserve_pct / 100.0
    # Vol-target: ATR-gated first-order scaling
    for s in pc["strategies"]:
        if "Multiplier" in s["sizing"]:
            m = s["sizing"]["Multiplier"]
        else:
            m = s["sizing"].get("multiplier", {})
        try:
            foq = float(m.get("first_order_quote", "20"))
        except Exception:
            foq = 20.0
        m["first_order_quote"] = f"{foq * reserve_factor:.6f}"

        # Set vol_target so engine scales first order when ATR% > target
        srl = s.get("risk_limits", {})
        srl["vol_target_atr_pct"] = 2.0
        srl["vol_target_min_scale"] = first_order_scale_atr
        srl["vol_target_max_scale"] = 1.0
        # Release reserve into late safety orders via taper (release_after_leg)
        # taper_safety_after_leg with scale 1.0 = no taper, but engine sees leg>=N
        # We use taper to RELEASE: taper_safety_scale > 1.0 boosts later legs
        # (engine clamps at configured max). Use scale = 1/reserve_factor to
        # restore the reserved capital into legs after release_after_leg.
        if reserve_pct > 0:
            srl["taper_safety_after_leg"] = release_after_leg
            srl["taper_safety_scale"] = max(1.0, 1.0 / max(reserve_factor, 0.1))
        # Release condition as safety_order_condition
        if release_condition == "rsi_reclaim_35":
            srl["safety_order_condition"] = "rsi(14) > 35"
        elif release_condition == "adx_falling_below_25":
            srl["safety_order_condition"] = "adx(14) < 25"
        elif release_condition == "volatility_contracting_30pct":
            # Approximate volatility contraction via ATR percent threshold
            srl["safety_order_condition"] = "atr_percent(14) < 1.5"
        s["risk_limits"] = srl

    # max_active_symbols: cap concurrent active strategies
    # Implemented via portfolio-level max_active_cycles (each strategy = 1 cycle)
    rl["max_active_cycles"] = max_active_symbols
    pc["risk_limits"] = rl

    return new_cfg


def replay(config_path, budget, s, e, pid):
    cmd = [REPLAY, "--config", config_path, "--budget", str(budget),
           "--start-ms", str(s), "--end-ms", str(e),
           "--market-data", _RUNTIME["market"], "--funding-data", _RUNTIME["funding"],
           "--profile", "aggressive", "--portfolio-id", pid,
           "--exchange-min-notional", "5"]
    try:
        pr = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    except subprocess.TimeoutExpired:
        return None
    if pr.returncode != 0:
        return None
    try:
        r = json.loads(pr.stdout)
    except Exception:
        return None
    o = r.get("on_budget", {})
    return {
        "ann": o.get("annualized_return_pct", -999),
        "dd": o.get("max_drawdown_pct", 999),
        "ret": o.get("total_return_pct", -999),
        "trades": r.get("trade_count", 0),
    }


def evaluate_one(spec):
    label, cfg, tmp_path = spec
    with open(tmp_path, "w") as f:
        json.dump(cfg, f)
    full = replay(tmp_path, 5000, FULL_START, FULL_END, f"r9p4_{label}")
    seg_metrics = {}
    for nm, s, e in FULL_SEGMENTS:
        seg_metrics[nm] = replay(tmp_path, 5000, s, e, f"r9p4_{label}_{nm}")
    try:
        os.remove(tmp_path)
    except Exception:
        pass
    if full is None:
        return {"label": label, "skipped": True}
    pos_segs = sum(1 for v in seg_metrics.values() if v and v["ret"] > 0)
    seg_rets = {n: (v["ret"] if v else 0) for n, v in seg_metrics.items()}
    return {
        "label": label,
        "full_metrics": full,
        "segment_metrics": seg_metrics,
        "positive_segments": pos_segs,
        "agg_2024_2026": round(sum(seg_rets.get(k, 0) for k in ("2024", "2025", "2026_ytd")), 2),
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

    # Optionally include P3 promoted sleeves if available
    p3_path = "docs/superpowers/artifacts/glm-martingale-core-round9/r9-multi-symbol-sleeve-library.json"
    p3_configs = []
    if os.path.exists(p3_path):
        try:
            p3 = json.load(open(p3_path))
            # Take top 3 promoted by ann
            promoted = p3.get("promoted", [])
            for i, p in enumerate(promoted[:3]):
                # Reconstruct config from spec — but we don't have it stored.
                # Skip P3 bases for now; documented in ledger.
                pass
        except Exception:
            pass

    bases = list(BASE_SLEEVES.keys())
    reserve_pcts = [10, 20, 30, 40]
    first_order_scales = [0.5, 0.7, 1.0]
    release_after_legs = [2, 3, 4]
    release_conditions = ["rsi_reclaim_35", "adx_falling_below_25", "volatility_contracting_30pct"]
    max_active_syms = [3, 4, 5]

    total = (len(bases) * len(reserve_pcts) * len(first_order_scales) *
             len(release_after_legs) * len(release_conditions) * len(max_active_syms))
    print(f"Total configs: {total} (need >=720)", flush=True)

    # Generate specs
    specs = []
    base_cfgs = {b: load_base(b) for b in bases}
    for base_name in bases:
        for rp in reserve_pcts:
            for fos in first_order_scales:
                for ral in release_after_legs:
                    for rc in release_conditions:
                        for mas in max_active_syms:
                            label = f"{base_name}_rp{rp}_fos{fos}_ral{ral}_{rc[:5]}_mas{mas}"
                            cfg = apply_reserve_and_stake_buffer(
                                base_cfgs[base_name], rp, fos, ral, rc, mas
                            )
                            tmp_path = f"/tmp/r9p4_{label}.json"
                            specs.append((label, cfg, tmp_path))

    print(f"Generated {len(specs)} specs. Running full + 5 segments each...", flush=True)
    t0 = time.time()
    results = []
    completed = 0
    with ProcessPoolExecutor(max_workers=args.workers) as ex:
        futures = {ex.submit(evaluate_one, spec): spec[0] for spec in specs}
        for fut in as_completed(futures):
            try:
                r = fut.result()
                results.append(r)
            except Exception as e:
                results.append({"label": futures[fut], "skipped": True, "error": str(e)})
            completed += 1
            if completed % 50 == 0:
                elapsed = time.time() - t0
                rate = completed / elapsed if elapsed > 0 else 0
                eta = (len(specs) - completed) / rate if rate > 0 else 0
                print(f"  [{completed}/{len(specs)}] elapsed={elapsed:.0f}s rate={rate:.2f}/s eta={eta:.0f}s", flush=True)

    valid = [r for r in results if not r.get("skipped")]
    valid.sort(key=lambda r: r["full_metrics"]["ann"], reverse=True)

    # Promotion: DD improves by >=3pp vs base while ann drops by <=5pp
    BASE_METRICS = {"R4-combo": (34.7, 17.7), "R6-QB": (34.0, 18.0), "R7-ANKR-q": (63.5, 28.2)}
    promoted = []
    for r in valid:
        base_name = r["label"].split("_rp")[0]
        base_ann, base_dd = BASE_METRICS.get(base_name, (0, 100))
        fm = r["full_metrics"]
        dd_improves = base_dd - fm["dd"] >= 3
        ann_drops = base_ann - fm["ann"] <= 5
        if dd_improves and ann_drops:
            r["promoted"] = True
            promoted.append(r)
        else:
            r["promoted"] = False

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({
        "results": valid,
        "promoted": promoted,
        "total_specs": len(specs),
        "total_evaluated": len(valid),
        "total_skipped": len(results) - len(valid),
        "total_promoted": len(promoted),
    }, open(args.out, "w"), indent=2)

    elapsed = time.time() - t0
    print(f"\n[r9P4] wrote {args.out}: {len(valid)} evaluated, {len(promoted)} promoted in {elapsed:.0f}s")
    print(f"\n=== TOP 10 by ann ===")
    for v in valid[:10]:
        fm = v["full_metrics"]
        print(f"  {v['label']:65s} ann={fm['ann']:7.1f} dd={fm['dd']:6.1f} pos={v['positive_segments']}/5 prom={v['promoted']}")
    print(f"\n=== Top 5 promoted (DD improves >=3pp, ann drops <=5pp) ===")
    for v in promoted[:5]:
        fm = v["full_metrics"]
        print(f"  {v['label']}: ann={fm['ann']} dd={fm['dd']} pos={v['positive_segments']}/5")


if __name__ == "__main__":
    main()
