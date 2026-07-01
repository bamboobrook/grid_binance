#!/usr/bin/env python3
"""GLM Martingale Core — ATR/ADX-adaptive efficiency search (Task 3).

Goal: raise per-cycle profit (ann ceiling) via:
  1. ATR-based take-profit (Atr multiplier model — live-parity) instead of fixed
     bps, so TP scales with volatility (bigger TP in volatile regimes).
  2. Higher multipliers + more legs but FEWER entries (strict regime gate) so
     each cycle is a high-conviction martingale ladder.
  3. ADX safety-skip (already live-parity) to stop averaging in strong trends.
  4. Wider cooldown to reduce churn.

The current regime-gated approach trades 3000-4000 times per segment for tiny
per-cycle profit (0.008%/trade). High-ann martingale needs fewer, bigger cycles.

Structures tested: the breakthrough BNB-long + crash-short combo, with
ATR-adaptive TP and aggressive multipliers, + DD stop (config-structured).

Usage:
  python3 scripts/glm_atr_adx_efficiency_search.py \
      --out docs/superpowers/artifacts/glm-martingale-core/atr-adx-efficiency.json
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
    p = f"/tmp/glm_at_{os.getpid()}_{pid}.json"
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


def mk(sid, symbol, direction, gates, p, w, tp_model):
    """tp_model: either ('percent', bps) or ('atr', multiplier)."""
    tr = [{"cooldown": {"seconds": p["cooldown"]}}]
    for g in gates:
        tr.append({"indicator_expression": {"expression": g}})
    if tp_model[0] == "atr":
        tp = {"atr": {"multiplier": str(tp_model[1])}}
    else:
        tp = {"percent": {"bps": tp_model[1]}}
    indicators = [{"atr": {"period": p["atr_period"]}}, {"adx": {"period": 14}}]
    return {"strategy_id": sid, "symbol": symbol, "market": "usd_m_futures",
            "direction": direction, "direction_mode": "long_and_short",
            "margin_mode": "isolated", "leverage": p["leverage"],
            "spacing": {"fixed_percent": {"step_bps": p["step_bps"]}},
            "sizing": {"multiplier": {"first_order_quote": str(p["first_q"]),
                       "multiplier": str(p["multiplier"]), "max_legs": p["max_legs"]}},
            "take_profit": tp,
            "stop_loss": {"strategy_drawdown_pct": {"pct_bps": p["sl_bps"]}},
            "indicators": indicators, "entry_triggers": tr,
            "risk_limits": {"max_active_cycles": None, "max_global_budget_quote": None,
                "max_symbol_budget_quote": None, "max_direction_budget_quote": None,
                "max_strategy_budget_quote": None, "max_global_drawdown_quote": None,
                "safety_skip_adx_threshold": p.get("adx_skip")},
            "portfolio_weight_pct": str(w)}


# Strict gates (fewer, higher-conviction entries)
LONG_STRICT = ["{S}.close > {S}.ema(50)", "{S}.ema(50) > {S}.ema(200)",
               "BTCUSDT.close > BTCUSDT.ema(50)"]
SHORT_STRICT = ["{S}.close < {S}.ema(50)", "{S}.ema(50) < {S}.ema(200)",
                "BTCUSDT.close < BTCUSDT.ema(50)"]
LONG_MID = ["{S}.close > {S}.ema(50)", "BTCUSDT.close > BTCUSDT.ema(50)"]
SHORT_MID = ["{S}.close < {S}.ema(50)", "BTCUSDT.close < BTCUSDT.ema(50)"]


def build(longs, shorts, profile, lw, sw, tp_long, tp_short, dd_stop, cd_hours,
          gate_style, adx_skip):
    lg = LONG_STRICT if gate_style == "strict" else LONG_MID
    sg = SHORT_STRICT if gate_style == "strict" else SHORT_MID
    # Aggressive martingale params: high multiplier, more legs, bigger first order
    if profile == "aggressive":
        lp = dict(leverage=10, multiplier=3.0, max_legs=9, step_bps=150,
                  sl_bps=5000, cooldown=21600, first_q=35.0, atr_period=14,
                  adx_skip=adx_skip)
        sp = dict(leverage=10, multiplier=2.5, max_legs=8, step_bps=180,
                  sl_bps=4000, cooldown=21600, first_q=30.0, atr_period=14,
                  adx_skip=adx_skip)
    else:  # balanced
        lp = dict(leverage=10, multiplier=2.0, max_legs=7, step_bps=120,
                  sl_bps=3000, cooldown=21600, first_q=40.0, atr_period=14,
                  adx_skip=adx_skip)
        sp = dict(leverage=10, multiplier=1.8, max_legs=6, step_bps=150,
                  sl_bps=2500, cooldown=21600, first_q=35.0, atr_period=14,
                  adx_skip=adx_skip)
    strat = []
    for i, s in enumerate(longs):
        strat.append(mk(f"at-L{i}-{s}", s, "long",
                        [x.format(S=s) for x in lg], lp, lw, tp_long))
    for i, s in enumerate(shorts):
        strat.append(mk(f"at-S{i}-{s}", s, "short",
                        [x.format(S=s) for x in sg], sp, sw, tp_short))
    return {"direction_mode": "long_and_short", "strategies": strat,
            "risk_limits": {"max_global_budget_quote": "5000",
                            "portfolio_equity_stop_pct": dd_stop,
                            "portfolio_stop_cooldown_hours": cd_hours}}


def full_eval(args):
    config, profile, label = args
    seg_m = {}
    for name, s, e in FULL_SEGMENTS:
        seg_m[name] = metrics(run_replay(config, 5000, s, e, profile,
                                          label[:10] + name))
    full_m = metrics(run_replay(config, 5000, FULL_START, FULL_END, profile,
                                label[:10] + "fl"))
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
    ap.add_argument("--workers", type=int, default=22)
    args = ap.parse_args()

    long_sets = [
        ["BNBUSDT"],
        ["BNBUSDT", "TRXUSDT"],
        ["BNBUSDT", "BCHUSDT"],
    ]
    short_sets = [
        ["AAVEUSDT"],
        ["AAVEUSDT", "SOLUSDT"],
        ["AAVEUSDT", "DOTUSDT", "NEARUSDT"],
    ]
    # ATR TP multipliers (vs fixed-percent baseline). Bigger = bigger per-cycle profit.
    tp_options = [("atr", 1.5), ("atr", 2.0), ("atr", 2.5), ("atr", 3.0),
                  ("percent", 420), ("percent", 600)]
    dd_stops = [(15, 12), (20, 24), (25, 24), (30, 24)]
    profiles = ["aggressive", "balanced"]
    gate_styles = ["strict", "mid"]
    adx_skips = [35, 45, 55]
    weight_sets = [(40, 12), (35, 15), (45, 10)]

    jobs = []
    for profile in profiles:
        for ls in long_sets:
            for ss in short_sets:
                for gs in gate_styles:
                    for adx in adx_skips:
                        for tpl in tp_options:
                            for tps in tp_options:
                                for dd, cd in dd_stops:
                                    for lw, sw in weight_sets:
                                        cfg = build(ls, ss, profile, lw, sw,
                                                    tpl, tps, dd, cd, gs, adx)
                                        lbl = (f"{profile[:3]}-{gs[0]}-L{len(ls)}S{len(ss)}"
                                               f"-t{tpl[0][0]}{tpl[1]}{tps[0][0]}{tps[1]}"
                                               f"-d{dd}a{adx}-{lw}w{sw}")
                                        jobs.append((cfg, profile, lbl))

    print(f"[glm-at] {len(jobs)} candidates x 6 replays, {args.workers} workers",
          flush=True)
    # If too many, sample down. Each candidate = 6 replays.
    MAX = 1800
    if len(jobs) > MAX:
        import random
        random.seed(42)
        jobs = random.sample(jobs, MAX)
        print(f"[glm-at] sampled down to {len(jobs)}", flush=True)

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
            if done % 50 == 0 or (fm.get("ann") and fm["ann"] > 20):
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):36s} "
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
    print(f"\n[glm-at] wrote {args.out}: {len(results)} results, {n_pass} passes "
          f"in {time.time()-t0:.0f}s", flush=True)
    print("\n=== TOP 10 by pos_segs then ann ===")
    for v in results[:10]:
        if "error" in v:
            continue
        fm = v.get("full_metrics") or {}
        print(f"  {v['label']:36s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} "
              f"pos={v['positive_segments']}/5 agg2426={v['agg_2024_2026']:7.1f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
