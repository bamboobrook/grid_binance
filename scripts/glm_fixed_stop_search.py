#!/usr/bin/env python3
"""GLM Martingale Core — Post-fix DD-stop optimization search.

With the DD-stop equity-base bug fixed, the stop now correctly caps budget-based
DD. This searches the high-ann structure + variants for a config that maximizes
segment stability under the FIXED stop, targeting positive-segments >=3 with
the best ann/DD.

Uses the live-parity config fields (portfolio_equity_stop_pct) — no env needed.
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
    p = f"/tmp/glm_fx_{os.getpid()}_{pid}.json"
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


def mk(sid, symbol, direction, gates, p, w, tp_bps, sl_bps):
    tr = [{"cooldown": {"seconds": p["cooldown"]}}]
    for g in gates:
        tr.append({"indicator_expression": {"expression": g}})
    return {"strategy_id": sid, "symbol": symbol, "market": "usd_m_futures",
            "direction": direction, "direction_mode": "long_and_short",
            "margin_mode": "isolated", "leverage": p["leverage"],
            "spacing": {"fixed_percent": {"step_bps": p["step_bps"]}},
            "sizing": {"multiplier": {"first_order_quote": str(p["first_q"]),
                       "multiplier": str(p["multiplier"]), "max_legs": p["max_legs"]}},
            "take_profit": {"percent": {"bps": tp_bps}},
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
LONG_LOOSE = ["{S}.close > {S}.ema(100)", "BTCUSDT.close > BTCUSDT.ema(50)"]
SHORT_STRICT = ["{S}.close < {S}.ema(50)", "{S}.ema(50) < {S}.ema(200)",
                "BTCUSDT.close < BTCUSDT.ema(50)"]
SHORT_LOOSE = ["{S}.close < {S}.ema(100)", "BTCUSDT.close < BTCUSDT.ema(50)"]


def build(longs, shorts, gate_l, gate_s, p_longs, p_shorts, dd_stop, cd_hours, weights):
    strat = []
    for i, s in enumerate(longs):
        strat.append(mk(f"fx-L{i}-{s}", s, "long",
                        [x.format(S=s) for x in gate_l], p_longs, weights[0],
                        p_longs["tp_bps"], p_longs["sl_bps"]))
    for i, s in enumerate(shorts):
        strat.append(mk(f"fx-S{i}-{s}", s, "short",
                        [x.format(S=s) for x in gate_s], p_shorts, weights[1],
                        p_shorts["tp_bps"], p_shorts["sl_bps"]))
    return {"direction_mode": "long_and_short", "strategies": strat,
            "risk_limits": {"max_global_budget_quote": "5000",
                            "portfolio_equity_stop_pct": dd_stop,
                            "portfolio_stop_cooldown_hours": cd_hours}}


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

    long_sets = [["BNBUSDT", "TRXUSDT"], ["BNBUSDT", "BCHUSDT"],
                 ["BNBUSDT", "TRXUSDT", "BCHUSDT"]]
    short_sets = [["AAVEUSDT"], ["AAVEUSDT", "SOLUSDT"],
                  ["AAVEUSDT", "DOTUSDT", "NEARUSDT"]]
    gate_pairs = [("strict", "strict"), ("strict", "loose"),
                  ("loose", "loose"), ("loose", "strict")]
    # param packs: vary TP, multiplier, SL
    param_packs = [
        (dict(leverage=10, multiplier=2.5, max_legs=8, step_bps=150, tp_bps=600,
              sl_bps=5000, cooldown=21600, first_q=35.0, adx_skip=35),
         dict(leverage=10, multiplier=2.0, max_legs=7, step_bps=180, tp_bps=600,
              sl_bps=4000, cooldown=21600, first_q=30.0, adx_skip=35)),
        (dict(leverage=10, multiplier=3.0, max_legs=9, step_bps=150, tp_bps=600,
              sl_bps=5000, cooldown=21600, first_q=35.0, adx_skip=35),
         dict(leverage=10, multiplier=2.5, max_legs=8, step_bps=180, tp_bps=600,
              sl_bps=4000, cooldown=21600, first_q=30.0, adx_skip=35)),
        (dict(leverage=10, multiplier=2.5, max_legs=8, step_bps=150, tp_bps=450,
              sl_bps=4000, cooldown=21600, first_q=35.0, adx_skip=45),
         dict(leverage=10, multiplier=2.0, max_legs=7, step_bps=180, tp_bps=450,
              sl_bps=3500, cooldown=21600, first_q=30.0, adx_skip=45)),
    ]
    dd_stops = [(25, 12), (28, 12), (30, 24), (32, 24), (35, 24)]
    weight_sets = [(40, 12), (45, 10), (38, 14)]

    GATES = {"strict": LONG_STRICT, "loose": LONG_LOOSE}
    SGATES = {"strict": SHORT_STRICT, "loose": SHORT_LOOSE}
    jobs = []
    for ls in long_sets:
        for ss in short_sets:
            for gl, gs in gate_pairs:
                for pl, ps in param_packs:
                    for dd, cd in dd_stops:
                        for wl, ws in weight_sets:
                            cfg = build(ls, ss, GATES[gl], SGATES[gs], pl, ps,
                                        dd, cd, (wl, ws))
                            lbl = (f"L{len(ls)}S{len(ss)}-{gl[0]}{gs[0]}"
                                   f"-m{pl['multiplier']}{ps['multiplier']}"
                                   f"-t{pl['tp_bps']}-d{dd}c{cd}-{wl}w{ws}")
                            jobs.append((cfg, lbl))

    print(f"[glm-fx] {len(jobs)} candidates x 6 replays, {args.workers} workers",
          flush=True)
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
            if done % 40 == 0 or (rec.get("positive_segments", 0) >= 3 and
                                  (fm.get("ann") or 0) > 30):
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):30s} "
                      f"ann={fm.get('ann')} dd={fm.get('dd')} "
                      f"pos={rec.get('positive_segments')}/5 "
                      f"agg2426={rec.get('agg_2024_2026')}", flush=True)

    for v in results:
        if "error" in v:
            continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["gate_agg"] = (ann > 110 and dd <= 30 and v["positive_segments"] >= 3
                         and v["agg_2024_2026"] > 0 and v["h1_2023_contribution"] < 60
                         and not fm.get("breached"))
        v["near_agg"] = (ann > 50 and v["positive_segments"] >= 3
                         and v["agg_2024_2026"] > 0 and not fm.get("breached"))

    results.sort(key=lambda v: (v.get("positive_segments") or 0,
                                (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
               "n_candidates": len(jobs), "results": results,
               "passes": [v for v in results if v.get("gate_agg")]},
              open(args.out, "w"), indent=2, default=str)
    npass = len([v for v in results if v.get("gate_agg")])
    nnear = len([v for v in results if v.get("near_agg")])
    print(f"\n[glm-fx] wrote {args.out}: {len(results)} results, {npass} passes, "
          f"{nnear} near(ann>50,pos>=3) in {time.time()-t0:.0f}s", flush=True)
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
