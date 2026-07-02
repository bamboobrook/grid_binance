#!/usr/bin/env python3
"""GLM Martingale Core — Re-entry + ATR-adaptive TP cliff-breaker search.

Hypothesis to break the ann/DD cliff: the fixed DD stop cuts ann because the
24h cooldown misses the recovery. A SHORT cooldown (1-6h) lets the bot re-enter
when equity recovers, capturing round-trips like 2024's -45%->+127%. Combined
with ATR-adaptive TP (smaller TP in high-vol -> smaller per-cycle floating loss),
this may find ann>50% @ DD<=30%.

All mechanisms are live-parity (config fields + ATR TP model + reduceOnly stop).

Two TP modes tested:
  - percent (fixed bps) — baseline high-ann
  - ATR multiplier — TP scales with volatility; in Mixed/Trailing we can blend

Usage:
  python3 scripts/glm_reentry_atr_search.py \
      --out docs/superpowers/artifacts/glm-martingale-core/reentry-atr-search.json
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


def run_replay(config, budget, s, e, profile, pid):
    p = f"/tmp/glm_re_{os.getpid()}_{pid}.json"
    json.dump({"portfolio_config": config}, open(p, "w"))
    cmd = [REPLAY, "--config", p, "--budget", str(budget),
           "--start-ms", str(s), "--end-ms", str(e),
           "--market-data", MARKET_DB, "--funding-data", FUNDING_DB,
           "--profile", profile, "--portfolio-id", pid,
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
            "ret": o.get("total_return_pct"), "min_eq": o.get("min_equity_quote"),
            "breached": o.get("principal_breached"), "trades": r.get("trade_count")}


def mk(sid, symbol, direction, gates, p, w, tp_model, sl_bps):
    tr = [{"cooldown": {"seconds": p["cooldown"]}}]
    for g in gates:
        tr.append({"indicator_expression": {"expression": g}})
    return {"strategy_id": sid, "symbol": symbol, "market": "usd_m_futures",
            "direction": direction, "direction_mode": "long_and_short",
            "margin_mode": "isolated", "leverage": p["leverage"],
            "spacing": {"fixed_percent": {"step_bps": p["step_bps"]}},
            "sizing": {"multiplier": {"first_order_quote": str(p["first_q"]),
                       "multiplier": str(p["multiplier"]), "max_legs": p["max_legs"]}},
            "take_profit": tp_model,
            "stop_loss": {"strategy_drawdown_pct": {"pct_bps": sl_bps}},
            "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}],
            "entry_triggers": tr,
            "risk_limits": {"max_active_cycles": None, "max_global_budget_quote": None,
                "max_symbol_budget_quote": None, "max_direction_budget_quote": None,
                "max_strategy_budget_quote": None, "max_global_drawdown_quote": None,
                "safety_skip_adx_threshold": p.get("adx_skip")},
            "portfolio_weight_pct": str(w)}


LONG_STRICT = ["{S}.close > {S}.ema(50)", "{S}.ema(50) > {S}.ema(200)",
               "BTCUSDT.close > BTCUSDT.ema(50)"]
SHORT_STRICT = ["{S}.close < {S}.ema(50)", "{S}.ema(50) < {S}.ema(200)",
                "BTCUSDT.close < BTCUSDT.ema(50)"]


def tp_model(kind, value):
    """kind: 'pct' -> percent bps; 'atr' -> ATR multiplier; 'mixed' -> [atr, pct fallback]"""
    if kind == "pct":
        return {"percent": {"bps": value}}
    if kind == "atr":
        return {"atr": {"multiplier": str(value)}}
    if kind == "mixed":
        atr_m, pct_b = value
        return {"mixed": {"phases": [{"atr": {"multiplier": str(atr_m)}},
                                      {"percent": {"bps": pct_b}}]}}
    raise ValueError(kind)


def build(longs, shorts, tp_kind_long, tp_val_long, tp_kind_short, tp_val_short,
          mult_l, legs_l, mult_s, legs_s, sl_long, sl_short,
          dd_stop, cooldown_hours, weights, adx_skip):
    lp = dict(leverage=10, multiplier=mult_l, max_legs=legs_l, step_bps=150,
              cooldown=21600, first_q=35.0, adx_skip=adx_skip)
    sp = dict(leverage=10, multiplier=mult_s, max_legs=legs_s, step_bps=180,
              cooldown=21600, first_q=30.0, adx_skip=adx_skip)
    tpl = tp_model(tp_kind_long, tp_val_long)
    tps = tp_model(tp_kind_short, tp_val_short)
    strat = []
    for i, s in enumerate(longs):
        strat.append(mk(f"re-L{i}-{s}", s, "long",
                        [x.format(S=s) for x in LONG_STRICT], lp, weights[0], tpl, sl_long))
    for i, s in enumerate(shorts):
        strat.append(mk(f"re-S{i}-{s}", s, "short",
                        [x.format(S=s) for x in SHORT_STRICT], sp, weights[1], tps, sl_short))
    return {"direction_mode": "long_and_short", "strategies": strat,
            "risk_limits": {"max_global_budget_quote": "5000",
                            "portfolio_equity_stop_pct": dd_stop,
                            "portfolio_stop_cooldown_hours": cooldown_hours}}


def full_eval(args):
    config, label = args
    profile = "aggressive"
    seg_m = {}
    for name, s, e in FULL_SEGMENTS:
        seg_m[name] = metrics(run_replay(config, 5000, s, e, profile, label[:10] + name))
    full_m = metrics(run_replay(config, 5000, FULL_START, FULL_END, profile, label[:10] + "fl"))
    pos = sum(1 for v in seg_m.values() if v and v["ret"] and v["ret"] > 0)
    rets = {n: (v["ret"] if v and v["ret"] else 0.0) for n, v in seg_m.items()}
    agg = sum(rets.get(k, 0) for k in ("2024", "2025", "2026_ytd"))
    tot = sum(rets.values())
    h1c = rets.get("h1_2023", 0) / tot * 100 if tot > 0 else 0
    return {"label": label, "profile": profile, "config": config,
            "segment_metrics": seg_m, "full_metrics": full_m,
            "positive_segments": pos, "agg_2024_2026": agg,
            "h1_2023_contribution": h1c, "segment_returns": rets}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=28)
    args = ap.parse_args()

    long_sets = [["BNBUSDT", "TRXUSDT"], ["BNBUSDT", "BCHUSDT"]]
    short_sets = [["AAVEUSDT"], ["AAVEUSDT", "SOLUSDT"]]
    # TP options: fixed high (600), ATR multipliers, mixed (atr then pct fallback)
    tp_long_opts = [("pct", 600), ("atr", 2.0), ("atr", 1.5), ("mixed", (1.5, 600))]
    tp_short_opts = [("pct", 600), ("atr", 1.5), ("mixed", (1.2, 600))]
    mults = [(3.0, 9, 2.5, 8), (2.5, 8, 2.0, 7)]  # (ml, legs_l, ms, legs_s)
    # KEY: short cooldowns for re-entry + a range of DD stops
    dd_cooldowns = [(30, 1), (30, 2), (30, 4), (32, 2), (28, 2), (25, 1), (35, 2), (30, 6), (0, 0)]
    sl_opts = [(5000, 4000), (3500, 3000)]
    weight_sets = [(40, 12), (45, 10)]
    adx_skips = [35]

    jobs = []
    for ls in long_sets:
        for ss in short_sets:
            for tpl_k, tpl_v in tp_long_opts:
                for tps_k, tps_v in tp_short_opts:
                    for ml, ll, ms, ls_n in mults:
                        for sl_l, sl_s in sl_opts:
                            for dd, cd in dd_cooldowns:
                                for wl, ws in weight_sets:
                                    for adx in adx_skips:
                                        cfg = build(ls, ss, tpl_k, tpl_v, tps_k, tps_v,
                                                    ml, ll, ms, ls_n, sl_l, sl_s,
                                                    dd, cd, (wl, ws), adx)
                                        lbl = (f"L{len(ls)}S{len(ss)}"
                                               f"-tl{tpl_k[0]}{tpl_v if tpl_k!='mixed' else 'x'}"
                                               f"ts{tps_k[0]}{tps_v if tps_k!='mixed' else 'x'}"
                                               f"-m{ml}-sl{sl_l}-d{dd}c{cd}-{wl}w{ws}")
                                        jobs.append((cfg, lbl))

    print(f"[glm-re] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
            # print breakthrough candidates: ann>50 AND dd<=30, OR pos>=3 with ann>30
            ann = fm.get("ann") or 0
            dd = fm.get("dd") or 999
            if done % 60 == 0 or (ann > 50 and dd <= 30) or (rec.get("positive_segments",0)>=3 and ann>30):
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):32s} "
                      f"ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 "
                      f"agg2426={rec.get('agg_2024_2026')}", flush=True)

    for v in results:
        if "error" in v:
            continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["breakthrough"] = (ann > 50 and dd <= 30 and v["positive_segments"] >= 3
                             and v["agg_2024_2026"] > 0 and v["h1_2023_contribution"] < 60
                             and not fm.get("breached"))
        v["near"] = (ann > 40 and dd <= 32 and v["positive_segments"] >= 3
                     and v["agg_2024_2026"] > 0 and not fm.get("breached"))

    results.sort(key=lambda v: ((v.get("full_metrics") or {}).get("ann") or 0)
                 - 0.3*((v.get("full_metrics") or {}).get("dd") or 999), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
               "n_candidates": len(jobs), "results": results,
               "breakthroughs": [v for v in results if v.get("breakthrough")]},
              open(args.out, "w"), indent=2, default=str)
    nbt = len([v for v in results if v.get("breakthrough")])
    nnear = len([v for v in results if v.get("near")])
    print(f"\n[glm-re] wrote {args.out}: {len(results)} results, "
          f"{nbt} BREAKTHROUGH(ann>50,dd<=30,pos>=3), {nnear} near in {time.time()-t0:.0f}s", flush=True)
    print("\n=== TOP 10 by ann-0.3*dd ===")
    for v in results[:10]:
        if "error" in v:
            continue
        fm = v.get("full_metrics") or {}
        print(f"  {v['label']:32s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} "
              f"pos={v['positive_segments']}/5 agg2426={v['agg_2024_2026']:7.1f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
