#!/usr/bin/env python3
"""GLM Round 2 Direction A: Partial TP + Breakeven — full grid search with 5-seg validation.

Uses the REAL Rust engine Partial TP feature (implemented this session). Tests
the hypothesis that partial TP banking + breakeven stop breaks the ann/DD cliff.

Full grid per plan section 5 Direction A + section 12 Batch 1:
  - 3 symbol sets
  - TP ladders: (400,900,1600), (600,1200,2200), (800,1600,2600)
  - TP splits: 50/30/20, 40/35/25, 30/30/40
  - breakeven_after_stage: 0 (after TP1), 1 (after TP2)
  - breakeven_buffer: 0, 50, 100 bps
Each candidate: full-period + 5-segment validation.

Usage:
  python3 scripts/glm_r2_dirA_search.py \
      --out docs/superpowers/artifacts/glm-martingale-core-round2/r2-A-full-grid.json
"""
import argparse
import json
import os
import subprocess
import sys
import time
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [
    ("h1_2023", 1672531200000, 1688169599999),
    ("h2_2023", 1688169600000, 1704067199999),
    ("2024", 1704067200000, 1735689599999),
    ("2025", 1735689600000, 1767225599999),
    ("2026_ytd", 1767225600000, 1780271999999),
]
FULL_START, FULL_END = 1672531200000, 1780271999999
LONG_STRICT = ["{S}.close > {S}.ema(50)", "{S}.ema(50) > {S}.ema(200)",
               "BTCUSDT.close > BTCUSDT.ema(50)"]
SHORT_MID = ["{S}.close < {S}.ema(50)", "BTCUSDT.close < BTCUSDT.ema(50)"]


def run_replay(config, budget, s, e, pid):
    p = f"/tmp/r2Af_{os.getpid()}_{pid}.json"
    json.dump({"portfolio_config": config}, open(p, "w"))
    cmd = [REPLAY, "--config", p, "--budget", str(budget),
           "--start-ms", str(s), "--end-ms", str(e),
           "--market-data", MARKET_DB, "--funding-data", FUNDING_DB,
           "--profile", "aggressive", "--portfolio-id", pid,
           "--exchange-min-notional", "5"]
    try:
        pr = subprocess.run(cmd, capture_output=True, text=True, timeout=900)
    except subprocess.TimeoutExpired:
        return {"error": "timeout"}
    finally:
        try:
            os.remove(p)
        except OSError:
            pass
    if pr.returncode != 0:
        return {"error": pr.stderr.strip()[:500]}
    try:
        return json.loads(pr.stdout)
    except json.JSONDecodeError:
        return {"error": pr.stdout[:500]}


def metrics(r):
    if "error" in r or "on_budget" not in r:
        return None
    o = r["on_budget"]
    return {"ann": o.get("annualized_return_pct"), "dd": o.get("max_drawdown_pct"),
            "ret": o.get("total_return_pct"), "breached": o.get("principal_breached"),
            "trades": r.get("trade_count")}


def mk(sid, sym, d, gates, w, mult, legs, tp_model, sl):
    tr = [{"cooldown": {"seconds": 21600}}]
    for g in gates:
        tr.append({"indicator_expression": {"expression": g}})
    fq = 30.0 if d == "short" else 35.0
    sll = 4000 if d == "short" else sl
    return {"strategy_id": sid, "symbol": sym, "market": "usd_m_futures",
            "direction": d, "direction_mode": "long_and_short",
            "margin_mode": "isolated", "leverage": 10,
            "spacing": {"fixed_percent": {"step_bps": 150}},
            "sizing": {"multiplier": {"first_order_quote": str(fq),
                       "multiplier": str(mult), "max_legs": legs}},
            "take_profit": tp_model,
            "stop_loss": {"strategy_drawdown_pct": {"pct_bps": sll}},
            "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}],
            "entry_triggers": tr,
            "risk_limits": {"max_active_cycles": None, "max_global_budget_quote": None,
                "max_symbol_budget_quote": None, "max_direction_budget_quote": None,
                "max_strategy_budget_quote": None, "max_global_drawdown_quote": None,
                "safety_skip_adx_threshold": 35},
            "portfolio_weight_pct": str(w)}


def partial_tp_model(stages_bps, split_numden, be_after, be_buf):
    """Build a Partial TP model. stages_bps=[tp1,tp2,tp3], split_numden=[(n1,d1),(n2,d2),(n3,d3)].
    The fractions apply to REMAINING position at each stage. To make a 50/30/20 split
    of ORIGINAL position across 3 stages, convert to remaining-fractions:
      stage0 closes 50% of current(=orig) => frac0 = 0.50
      stage1 closes 30% of orig = 30/50 of remaining => frac1 = 0.60
      stage2 closes rest (20% of orig = 40% of remaining) => frac2 = 1.0 (all remaining)
    We encode each stage as (num, den, tp_bps) where num/den is the fraction of REMAINING.
    """
    # convert original-split to remaining-fractions
    orig_fracs = [n / d for n, d in split_numden]
    remaining = 1.0
    rem_fracs = []
    for i, of in enumerate(orig_fracs):
        if remaining <= 0:
            rem_fracs.append((1, 1))
            continue
        if i == len(orig_fracs) - 1:
            rem_fracs.append((1, 1))  # last stage closes all remaining
        else:
            rf = of / remaining
            rf = min(max(rf, 0.0), 1.0)
            # encode as fraction with denominator 1000 for precision
            rem_fracs.append((int(round(rf * 1000)), 1000))
        remaining *= (1.0 - (rem_fracs[-1][0] / rem_fracs[-1][1]))
    stages = [[rem_fracs[i][0], rem_fracs[i][1], stages_bps[i]] for i in range(3)]
    return {"partial": {"stages": stages, "breakeven_after_stage": be_after,
                        "breakeven_buffer_bps": be_buf}}


def build(sym_set_idx, tp_ladder, split, be_after, be_buf):
    sym_sets = [
        (["BNBUSDT", "TRXUSDT", "BCHUSDT"], ["AAVEUSDT", "SOLUSDT", "DOTUSDT"]),
        (["BNBUSDT", "TRXUSDT"], ["AAVEUSDT"]),
        (["BNBUSDT", "TRXUSDT", "BCHUSDT", "ADAUSDT"], ["AAVEUSDT", "SOLUSDT", "DOTUSDT", "NEARUSDT"]),
    ]
    longs, shorts = sym_sets[sym_set_idx]
    tp_model = partial_tp_model(tp_ladder, split, be_after, be_buf)
    wl = 40.0 / len(longs)
    ws = 24.0 / len(shorts)
    strat = [mk(f"L{i}-{s}", s, "long", [x.format(S=s) for x in LONG_STRICT], wl, 2.8, 8, tp_model, 5000)
             for i, s in enumerate(longs)]
    strat += [mk(f"S{i}-{s}", s, "short", [x.format(S=s) for x in SHORT_MID], ws, 2.5, 8, tp_model, 4000)
              for i, s in enumerate(shorts)]
    return {"direction_mode": "long_and_short", "strategies": strat,
            "risk_limits": {"max_global_budget_quote": "5000"}}


def full_eval(args):
    config, label = args
    seg_m = {}
    for name, s, e in FULL_SEGMENTS:
        seg_m[name] = metrics(run_replay(config, 5000, s, e, label[:8] + name))
    full_m = metrics(run_replay(config, 5000, FULL_START, FULL_END, label[:8] + "fl"))
    pos = sum(1 for v in seg_m.values() if v and v["ret"] and v["ret"] > 0)
    rets = {n: (v["ret"] if v and v["ret"] else 0.0) for n, v in seg_m.items()}
    agg = sum(rets.get(k, 0) for k in ("2024", "2025", "2026_ytd"))
    tot = sum(rets.values())
    h1c = rets.get("h1_2023", 0) / tot * 100 if tot > 0 else 0
    return {"label": label, "config": config, "segment_metrics": seg_m,
            "full_metrics": full_m, "positive_segments": pos,
            "agg_2024_2026": agg, "h1_2023_contribution": h1c, "segment_returns": rets}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=20)
    args = ap.parse_args()

    tp_ladders = [(400, 900, 1600), (600, 1200, 2200), (800, 1600, 2600)]
    splits = [[(50, 100), (30, 100), (20, 100)],
              [(40, 100), (35, 100), (25, 100)],
              [(30, 100), (30, 100), (40, 100)]]
    be_afters = [0, 1]
    be_bufs = [0, 50, 100]
    sym_sets = [0, 1, 2]

    jobs = []
    for si in sym_sets:
        for tl in tp_ladders:
            for sp in splits:
                for bea in be_afters:
                    for beb in be_bufs:
                        cfg = build(si, list(tl), sp, bea, beb)
                        lbl = f"s{si}-tl{tl[0]}{tl[1]}{tl[2]}-sp{sp[0][0]}{sp[1][0]}{sp[2][0]}-be{bea}b{beb}"
                        jobs.append((cfg, lbl))

    print(f"[r2A-full] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
    t0 = time.time()
    results = []
    with ProcessPoolExecutor(max_workers=args.workers) as ex:
        futs = {ex.submit(full_eval, j): j for j in jobs}
        done = 0
        for fut in as_completed(futs):
            try:
                rec = fut.result()
            except Exception as e:
                rec = {"label": "?", "error": str(e), "positive_segments": 0}
            results.append(rec)
            done += 1
            fm = rec.get("full_metrics") or {}
            ann = fm.get("ann") or 0
            dd = fm.get("dd") or 999
            if done % 30 == 0 or (ann > 25 and rec.get("positive_segments", 0) >= 3):
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):34s} "
                      f"ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 "
                      f"agg={rec.get('agg_2024_2026')}", flush=True)

    # Classify
    for v in results:
        if "error" in v:
            continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["near_agg"] = (ann > 70 and dd <= 35 and v["positive_segments"] >= 3
                         and v["agg_2024_2026"] > 0 and not fm.get("breached"))
        v["frontier_improvement"] = (
            (ann > 22.2 or dd < 26.1) and v["positive_segments"] >= 3
            and v["agg_2024_2026"] > 0 and not fm.get("breached"))

    results.sort(key=lambda v: ((v.get("positive_segments") or 0),
                                (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"n_candidates": len(jobs), "results": results,
               "near_agg": [v for v in results if v.get("near_agg")],
               "frontier_improvements": [v for v in results if v.get("frontier_improvement")]},
              open(args.out, "w"), indent=2, default=str)
    n_near = len([v for v in results if v.get("near_agg")])
    n_fi = len([v for v in results if v.get("frontier_improvement")])
    print(f"\n[r2A-full] wrote {args.out}: {len(results)} results, "
          f"{n_near} near_agg, {n_fi} frontier_improvement in {time.time()-t0:.0f}s", flush=True)
    print("\n=== TOP 10 by pos_segs then ann ===")
    for v in results[:10]:
        if "error" in v:
            continue
        fm = v.get("full_metrics") or {}
        print(f"  {v['label']:34s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} "
              f"pos={v['positive_segments']}/5 agg={v['agg_2024_2026']:7.1f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
