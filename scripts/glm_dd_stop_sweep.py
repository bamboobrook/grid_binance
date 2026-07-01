#!/usr/bin/env python3
"""GLM Martingale Core — DD-stop level sweep + multi-short expansion.

The portfolio equity stop is the master risk lever (cut DD 37->5.75%).
This sweeps the stop level (10-30%) and cooldown to find the return/DD sweet
spot, AND expands the crash-short basket to capture more 2025/2026 crash profit.

Goal: find a config that is positive across MOST segments with ann>50% and
controlled DD, using the DD stop (research-only but the structural answer).

Usage:
  python3 scripts/glm_dd_stop_sweep.py \
      --out docs/superpowers/artifacts/glm-martingale-core/dd-stop-sweep.json
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


def run_replay(config, budget, s, e, profile, pid, env=None):
    p = f"/tmp/glm_dd_{os.getpid()}_{pid}.json"
    json.dump({"portfolio_config": config}, open(p, "w"))
    cmd = [REPLAY, "--config", p, "--budget", str(budget),
           "--start-ms", str(s), "--end-ms", str(e),
           "--market-data", MARKET_DB, "--funding-data", FUNDING_DB,
           "--profile", profile, "--portfolio-id", pid,
           "--exchange-min-notional", "5"]
    ee = dict(os.environ)
    if env:
        ee.update(env)
    try:
        pr = subprocess.run(cmd, capture_output=True, text=True, timeout=900, env=ee)
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
            "ret": o.get("total_return_pct"), "min_eq": o.get("min_equity_quote"),
            "breached": o.get("principal_breached"), "trades": r.get("trade_count")}


def mk(sid, symbol, direction, gates, p, w):
    tr = [{"cooldown": {"seconds": p["cooldown"]}}]
    for g in gates:
        tr.append({"indicator_expression": {"expression": g}})
    return {"strategy_id": sid, "symbol": symbol, "market": "usd_m_futures",
            "direction": direction, "direction_mode": "long_and_short",
            "margin_mode": "isolated", "leverage": p["leverage"],
            "spacing": {"fixed_percent": {"step_bps": p["step_bps"]}},
            "sizing": {"multiplier": {"first_order_quote": str(p["first_q"]),
                       "multiplier": str(p["multiplier"]), "max_legs": p["max_legs"]}},
            "take_profit": {"percent": {"bps": p["tp_bps"]}},
            "stop_loss": {"strategy_drawdown_pct": {"pct_bps": p["sl_bps"]}},
            "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}],
            "entry_triggers": tr,
            "risk_limits": {"max_active_cycles": None, "max_global_budget_quote": None,
                "max_symbol_budget_quote": None, "max_direction_budget_quote": None,
                "max_strategy_budget_quote": None, "max_global_drawdown_quote": None},
            "portfolio_weight_pct": str(w)}


LONG_LOOSE = ["{S}.close > {S}.ema(100)", "BTCUSDT.close > BTCUSDT.ema(50)"]
SHORT_LOOSE = ["{S}.close < {S}.ema(100)", "BTCUSDT.close < BTCUSDT.ema(50)"]


def build(longs, shorts, profile, lw, sw):
    """longs/shorts are lists of symbols; lw/sw weight per leg."""
    if profile == "aggressive":
        lp = dict(leverage=10, multiplier=2.5, max_legs=8, step_bps=180, tp_bps=420,
                  sl_bps=4500, cooldown=10800, first_q=30.0)
        sp = dict(leverage=10, multiplier=2.0, max_legs=6, step_bps=180, tp_bps=400,
                  sl_bps=3000, cooldown=10800, first_q=28.0)
    elif profile == "balanced":
        lp = dict(leverage=10, multiplier=1.8, max_legs=6, step_bps=120, tp_bps=320,
                  sl_bps=2800, cooldown=14400, first_q=40.0)
        sp = dict(leverage=10, multiplier=1.6, max_legs=5, step_bps=150, tp_bps=350,
                  sl_bps=2200, cooldown=14400, first_q=35.0)
    else:
        lp = dict(leverage=5, multiplier=1.4, max_legs=5, step_bps=120, tp_bps=220,
                  sl_bps=1800, cooldown=21600, first_q=55.0)
        sp = dict(leverage=5, multiplier=1.3, max_legs=4, step_bps=150, tp_bps=300,
                  sl_bps=1500, cooldown=21600, first_q=50.0)
    strat = []
    for i, s in enumerate(longs):
        strat.append(mk(f"dd-L{i}-{s}", s, "long",
                        [x.format(S=s) for x in LONG_LOOSE], lp, lw))
    for i, s in enumerate(shorts):
        strat.append(mk(f"dd-S{i}-{s}", s, "short",
                        [x.format(S=s) for x in SHORT_LOOSE], sp, sw))
    return {"direction_mode": "long_and_short", "strategies": strat,
            "risk_limits": {"max_global_budget_quote": "5000"}}


def full_eval(args):
    config, profile, label, stop_pct, cd_hours = args
    env = {"MARTINGALE_BT_PORTFOLIO_EQUITY_STOP_PCT": str(stop_pct),
           "MARTINGALE_BT_PORTFOLIO_STOP_COOLDOWN_HOURS": str(cd_hours)}
    seg_m = {}
    for name, s, e in FULL_SEGMENTS:
        seg_m[name] = metrics(run_replay(config, 5000, s, e, profile,
                                          label[:10] + name, env))
    full_m = metrics(run_replay(config, 5000, FULL_START, FULL_END, profile,
                                label[:10] + "fl", env))
    pos = sum(1 for v in seg_m.values() if v and v["ret"] and v["ret"] > 0)
    rets = {n: (v["ret"] if v and v["ret"] else 0.0) for n, v in seg_m.items()}
    agg = sum(rets.get(k, 0) for k in ("2024", "2025", "2026_ytd"))
    tot = sum(rets.values())
    h1c = rets.get("h1_2023", 0) / tot * 100 if tot > 0 else 0
    return {"label": label, "profile": profile, "config": config,
            "stop_pct": stop_pct, "cooldown_hours": cd_hours,
            "segment_metrics": seg_m, "full_metrics": full_m,
            "positive_segments": pos, "agg_2024_2026": agg,
            "h1_2023_contribution": h1c, "segment_returns": rets}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=20)
    args = ap.parse_args()

    # Structures to test: long-bull cores + crash-short baskets of varying size
    long_sets = [
        ["BNBUSDT"],
        ["BNBUSDT", "BCHUSDT"],
        ["BNBUSDT", "TRXUSDT"],
    ]
    short_sets = [
        ["AAVEUSDT"],
        ["AAVEUSDT", "SOLUSDT"],
        ["AAVEUSDT", "DOTUSDT", "NEARUSDT"],
        ["AAVEUSDT", "SOLUSDT", "AVAXUSDT", "DOTUSDT"],
        ["AAVEUSDT", "SOLUSDT", "ARBUSDT", "OPUSDT", "ADAUSDT"],
    ]
    # DD stop sweep — KEY: 8% too tight (caps return); find the sweet spot
    stops = [(10, 24), (12, 24), (15, 24), (18, 24), (20, 24), (22, 24),
             (25, 24), (15, 12), (20, 12), (25, 12), (20, 48), (25, 48),
             (30, 12), (30, 24)]
    profiles = ["balanced", "aggressive"]
    weight_sets = [(40, 12), (35, 12), (45, 10)]

    jobs = []
    for profile in profiles:
        for ls in long_sets:
            for ss in short_sets:
                # weight per leg must keep total < 100; scale by count
                for lw, sw in weight_sets:
                    cfg = build(ls, ss, profile, lw, sw)
                    for stop_pct, cd in stops:
                        lbl = f"{profile[:3]}-L{len(ls)}S{len(ss)}-{lw}w{sw}-s{stop_pct}c{cd}"
                        jobs.append((cfg, profile, lbl, stop_pct, cd))

    print(f"[glm-dd] {len(jobs)} candidates x 6 replays (5seg+full), "
          f"{args.workers} workers", flush=True)
    # Each candidate = 6 replays. Run all replays as a flat pool.
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
            if done % 50 == 0 or (fm.get("ann") and fm["ann"] > 30):
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):28s} "
                      f"ann={fm.get('ann')} dd={fm.get('dd')} "
                      f"pos={rec.get('positive_segments')}/5 "
                      f"agg2426={rec.get('agg_2024_2026')}", flush=True)

    GATES = {"balanced": (90.0, 20.0, 4), "aggressive": (110.0, 30.0, 3),
             "conservative": (50.0, 10.0, 4)}
    for v in results:
        if "error" in v:
            continue
        g = GATES.get(v["profile"], (50, 10, 4))
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["gate_pass"] = (ann > g[0] and dd <= g[1]
                          and v["positive_segments"] >= g[2]
                          and v["agg_2024_2026"] > 0
                          and v["h1_2023_contribution"] < 60
                          and not fm.get("breached"))

    results.sort(key=lambda v: (v.get("positive_segments") or 0,
                                (v.get("full_metrics") or {}).get("ann") or 0),
                 reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
               "n_candidates": len(jobs), "results": results,
               "passes": [v for v in results if v.get("gate_pass")]},
              open(args.out, "w"), indent=2, default=str)
    n_pass = len([v for v in results if v.get("gate_pass")])
    print(f"\n[glm-dd] wrote {args.out}: {len(results)} results, {n_pass} passes "
          f"in {time.time()-t0:.0f}s", flush=True)
    # print best frontier
    print("\n=== TOP 10 by pos_segs then ann ===")
    for v in results[:10]:
        if "error" in v:
            continue
        fm = v.get("full_metrics") or {}
        print(f"  {v['label']:30s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} "
              f"pos={v['positive_segments']}/5 agg2426={v['agg_2024_2026']:7.1f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
