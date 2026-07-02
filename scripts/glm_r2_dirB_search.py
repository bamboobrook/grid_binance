#!/usr/bin/env python3
"""GLM Round 2 Direction B: Conditional Safety Orders — full grid with 5-seg validation.

Uses the REAL Rust engine safety_order_condition feature (implemented this session).
Tests the hypothesis that requiring an indicator confirmation (RSI oversold, BB
reentry, etc.) before adding a safety order reduces DD from one-way averaging.

Builds on the Direction A best structure (which showed 4/5 pos) and tests adding
conditional SO on top, plus standalone conditional SO on the 008 structure.

Grid per plan Direction B:
  - safety_order_condition: none, rsi(14)<45, rsi(14)<30, bb_lower(20,2)>close, adx(14)<25
  - base TP: percent 2200 (008) and the Direction-A partial TP
  - symbol sets: 008 (6sym) and A-best (6sym)

Usage:
  python3 scripts/glm_r2_dirB_search.py \
      --out docs/superpowers/artifacts/glm-martingale-core-round2/r2-B-full-grid.json
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
    p = f"/tmp/r2Bf_{os.getpid()}_{pid}.json"
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


def mk(sid, sym, d, gates, w, mult, legs, tp_model, sl, so_condition):
    tr = [{"cooldown": {"seconds": 21600}}]
    for g in gates:
        tr.append({"indicator_expression": {"expression": g}})
    fq = 30.0 if d == "short" else 35.0
    sll = 4000 if d == "short" else sl
    rl = {"max_active_cycles": None, "max_global_budget_quote": None,
          "max_symbol_budget_quote": None, "max_direction_budget_quote": None,
          "max_strategy_budget_quote": None, "max_global_drawdown_quote": None,
          "safety_skip_adx_threshold": 35}
    if so_condition:
        rl["safety_order_condition"] = so_condition
    return {"strategy_id": sid, "symbol": sym, "market": "usd_m_futures",
            "direction": d, "direction_mode": "long_and_short",
            "margin_mode": "isolated", "leverage": 10,
            "spacing": {"fixed_percent": {"step_bps": 150}},
            "sizing": {"multiplier": {"first_order_quote": str(fq),
                       "multiplier": str(mult), "max_legs": legs}},
            "take_profit": tp_model,
            "stop_loss": {"strategy_drawdown_pct": {"pct_bps": sll}},
            "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}],
            "entry_triggers": tr, "risk_limits": rl,
            "portfolio_weight_pct": str(w)}


def partial_tp(stages_bps, be_after, be_buf):
    # 30/30/40 split of original -> remaining fractions
    orig = [0.30, 0.30, 0.40]
    rem = 1.0
    stages = []
    for i, of in enumerate(orig):
        if i == len(orig) - 1:
            stages.append([1, 1, stages_bps[i]])
        else:
            rf = of / rem if rem > 0 else 1.0
            rf = min(max(rf, 0.0), 1.0)
            stages.append([int(round(rf * 1000)), 1000, stages_bps[i]])
            rem *= (1.0 - rf)
    return {"partial": {"stages": stages, "breakeven_after_stage": be_after,
                        "breakeven_buffer_bps": be_buf}}


def build(tp_kind, so_condition):
    longs = ["BNBUSDT", "TRXUSDT", "BCHUSDT"]
    shorts = ["AAVEUSDT", "SOLUSDT", "DOTUSDT"]
    if tp_kind == "pct2200":
        tp = {"percent": {"bps": 2200}}
        mult, legs, sl = 2.8, 8, 5000
    elif tp_kind == "partial_800_1600_2600":
        tp = partial_tp([800, 1600, 2600], 1, 100)
        mult, legs, sl = 2.8, 8, 5000
    else:  # partial_600_1200_2200
        tp = partial_tp([600, 1200, 2200], 1, 100)
        mult, legs, sl = 2.8, 8, 5000
    wl, ws = 13.3, 8.0
    strat = [mk(f"L{i}-{s}", s, "long", [x.format(S=s) for x in LONG_STRICT], wl, mult, legs, tp, sl, so_condition)
             for i, s in enumerate(longs)]
    strat += [mk(f"S{i}-{s}", s, "short", [x.format(S=s) for x in SHORT_MID], ws, 2.5, 8, tp, 4000, so_condition)
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

    # SO conditions per plan Direction B
    so_conditions = {
        "none": None,
        "rsi45": "rsi(14) < 45",
        "rsi30": "rsi(14) < 30",
        "bb_lower": "bb_lower(20, 2) > close",
        "adx25": "adx(14) < 25",
    }
    tp_kinds = ["pct2200", "partial_800_1600_2600", "partial_600_1200_2200"]
    jobs = []
    for tpk in tp_kinds:
        for son, soc in so_conditions.items():
            cfg = build(tpk, soc)
            lbl = f"{tpk}-so_{son}"
            jobs.append((cfg, lbl))

    print(f"[r2B-full] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
            print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):38s} "
                  f"ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 "
                  f"agg={rec.get('agg_2024_2026')}", flush=True)

    for v in results:
        if "error" in v:
            continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["frontier_improvement"] = (
            (ann > 17.5 or dd < 19.2 or v["positive_segments"] > 4)
            and v["positive_segments"] >= 3 and v["agg_2024_2026"] > 0
            and not fm.get("breached"))

    results.sort(key=lambda v: ((v.get("positive_segments") or 0),
                                (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"n_candidates": len(jobs), "results": results,
               "frontier_improvements": [v for v in results if v.get("frontier_improvement")]},
              open(args.out, "w"), indent=2, default=str)
    nfi = len([v for v in results if v.get("frontier_improvement")])
    print(f"\n[r2B-full] wrote {args.out}: {len(results)} results, "
          f"{nfi} frontier_improvement in {time.time()-t0:.0f}s", flush=True)
    print("\n=== TOP 10 by pos_segs then ann ===")
    for v in results[:10]:
        if "error" in v:
            continue
        fm = v.get("full_metrics") or {}
        print(f"  {v['label']:38s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} "
              f"pos={v['positive_segments']}/5 agg={v['agg_2024_2026']:7.1f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
