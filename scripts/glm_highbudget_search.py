#!/usr/bin/env python3
"""GLM Martingale Core — High-budget search (user-authorized relaxed capital constraint).

User authorized relaxing the <5000U constraint to test higher budgets. The
segment-stable structure (008-best: strict long + mid short, TP2200) at 5000U
gives ann 22.2%/DD 26.1%/3pos. Higher budget scales first_order_quote (via weight
caps), which can change the ann/DD/segment balance.

This searches budgets [5k,10k,20k,30k,50k] x weight-allocations x structure
variants to find the best generalizable frontier at higher budget.

Usage:
  python3 scripts/glm_highbudget_search.py \
      --out docs/superpowers/artifacts/glm-martingale-core/highbudget-search.json
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
    p = f"/tmp/glm_hb2_{os.getpid()}_{pid}.json"
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


def mk(sid, sym, direction, gates, w, mult, legs, tp, sl):
    tr = [{"cooldown": {"seconds": 21600}}]
    for g in gates:
        tr.append({"indicator_expression": {"expression": g}})
    fq = 30.0 if direction == "short" else 35.0
    sll = 4000 if direction == "short" else 5000
    return {"strategy_id": sid, "symbol": sym, "market": "usd_m_futures",
            "direction": direction, "direction_mode": "long_and_short",
            "margin_mode": "isolated", "leverage": 10,
            "spacing": {"fixed_percent": {"step_bps": 150}},
            "sizing": {"multiplier": {"first_order_quote": str(fq),
                       "multiplier": str(mult), "max_legs": legs}},
            "take_profit": {"percent": {"bps": tp}},
            "stop_loss": {"strategy_drawdown_pct": {"pct_bps": sll}},
            "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}],
            "entry_triggers": tr,
            "risk_limits": {"max_active_cycles": None, "max_global_budget_quote": None,
                "max_symbol_budget_quote": None, "max_direction_budget_quote": None,
                "max_strategy_budget_quote": None, "max_global_drawdown_quote": None,
                "safety_skip_adx_threshold": 35},
            "portfolio_weight_pct": str(w)}


def build(ml, ll, ms, ls_, tp, wl, ws, dd_stop, cd):
    longs = ["BNBUSDT", "TRXUSDT", "BCHUSDT"]
    shorts = ["AAVEUSDT", "SOLUSDT", "DOTUSDT"]
    strat = [mk(f"L{i}-{s}", s, "long", [x.format(S=s) for x in LONG_STRICT], wl, ml, ll, tp, 5000)
             for i, s in enumerate(longs)]
    strat += [mk(f"S{i}-{s}", s, "short", [x.format(S=s) for x in SHORT_MID], ws, ms, ls_, tp, 4000)
              for i, s in enumerate(shorts)]
    rl = {"max_global_budget_quote": "5000"}
    if dd_stop > 0:
        rl["portfolio_equity_stop_pct"] = dd_stop
        rl["portfolio_stop_cooldown_hours"] = cd
    return {"direction_mode": "long_and_short", "strategies": strat, "risk_limits": rl}


def full_eval(args):
    config, budget, label = args
    seg_m = {}
    for name, s, e in FULL_SEGMENTS:
        seg_m[name] = metrics(run_replay(config, budget, s, e, label[:8] + name))
    full_m = metrics(run_replay(config, budget, FULL_START, FULL_END, label[:8] + "fl"))
    pos = sum(1 for v in seg_m.values() if v and v["ret"] and v["ret"] > 0)
    rets = {n: (v["ret"] if v and v["ret"] else 0.0) for n, v in seg_m.items()}
    agg = sum(rets.get(k, 0) for k in ("2024", "2025", "2026_ytd"))
    tot = sum(rets.values())
    h1c = rets.get("h1_2023", 0) / tot * 100 if tot > 0 else 0
    return {"label": label, "budget": budget, "config": config,
            "segment_metrics": seg_m, "full_metrics": full_m,
            "positive_segments": pos, "agg_2024_2026": agg,
            "h1_2023_contribution": h1c, "segment_returns": rets}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=24)
    args = ap.parse_args()

    budgets = [5000, 10000, 20000, 30000, 50000]
    tps = [1800, 2200, 2600]
    mults = [(2.8, 8, 2.5, 8), (3.0, 9, 2.5, 8), (2.5, 8, 2.0, 7)]
    weights = [(13.3, 8.0), (16.0, 6.0), (10.0, 10.0)]
    dd_stops = [(0, 0), (30, 12)]

    jobs = []
    for budget in budgets:
        for tp in tps:
            for ml, ll, ms, ls_n in mults:
                for wl, ws in weights:
                    for dd, cd in dd_stops:
                        cfg = build(ml, ll, ms, ls_n, tp, wl, ws, dd, cd)
                        lbl = (f"b{budget}-t{tp}-m{ml}l{ll}-{wl}w{ws}-d{dd}")
                        jobs.append((cfg, budget, lbl))

    print(f"[glm-hb] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
            if done % 80 == 0 or (ann > 30 and dd <= 30 and rec.get("positive_segments",0) >= 3):
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):30s} "
                      f"ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 "
                      f"agg2426={rec.get('agg_2024_2026')}", flush=True)

    for v in results:
        if "error" in v:
            continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["pass_agg"] = (ann > 50 and dd <= 30 and v["positive_segments"] >= 3
                         and v["agg_2024_2026"] > 0 and not fm.get("breached"))

    results.sort(key=lambda v: (v.get("positive_segments") or 0,
                                (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
               "n_candidates": len(jobs), "results": results,
               "passes": [v for v in results if v.get("pass_agg")]},
              open(args.out, "w"), indent=2, default=str)
    npass = len([v for v in results if v.get("pass_agg")])
    # best by ann among pos>=3, dd<=30
    filt = [v for v in results if "error" not in v and (v.get("positive_segments",0) >= 3)
            and ((v.get("full_metrics") or {}).get("dd") or 999) <= 30]
    filt.sort(key=lambda v: (v.get("full_metrics") or {}).get("ann") or 0, reverse=True)
    print(f"\n[glm-hb] wrote {args.out}: {len(results)} results, {npass} passes "
          f"(ann>50,dd<=30,pos>=3) in {time.time()-t0:.0f}s", flush=True)
    print("\n=== TOP 10 by ann (pos>=3, dd<=30) ===")
    for v in filt[:10]:
        fm = v.get("full_metrics") or {}
        print(f"  {v['label']:30s} b={v['budget']:>6} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} "
              f"pos={v['positive_segments']}/5 agg2426={v['agg_2024_2026']:7.1f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
