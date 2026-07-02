#!/usr/bin/env python3
"""GLM Round 3 P6: Cross-Section Breadth Regime — research simulator.

The P1-best (ann 34.5%) still has 2025 at -10.1%. 2025 is a broad bear where
longs lose. This research simulator computes cross-sectional breadth (fraction
of universe above ema50/ema200) and filters the P1-best equity curve: when
breadth is low (broad bear), pause long entries. This tests whether a breadth
gate can reduce the 2025 loss.

Approach: re-run the P1-best config via the engine BUT with the long entry
gate augmented by a breadth condition. Since the engine can't compute breadth
cross-sectionally, this simulates by post-processing: run the engine with a
per-symbol gate that approximates breadth (BTC ema as proxy), then measure.

Simpler valid test: use BTC breadth as the gate. When BTC.close < BTC.ema(200)
(broad bear confirmed), the long-bull gate is already strict (requires
ema50>ema200), so longs are mostly paused. The test: does adding a STRONGER
bear filter (e.g. require BTC.atr_percent < 4, or BTC.close > BTC.ema(100))
reduce 2025 loss?

This script tests long-gate variations that proxy breadth regime.
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


def run_replay(config, budget, s, e, pid):
    p = f"/tmp/r3P6_{os.getpid()}_{pid}.json"
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
            "ret": o.get("total_return_pct"), "breached": o.get("principal_breached")}


def partial_tp(stages_bps, be_after=1, be_buf=100):
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


def mk(sid, sym, d, gates, w, cooldown_s, tp_model, so_condition):
    tr = [{"cooldown": {"seconds": cooldown_s}}]
    for g in gates:
        tr.append({"indicator_expression": {"expression": g}})
    fq = 30.0 if d == "short" else 35.0
    sl = 4000 if d == "short" else 5000
    mult = 2.5 if d == "short" else 2.8
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
                       "multiplier": str(mult), "max_legs": 8}},
            "take_profit": tp_model,
            "stop_loss": {"strategy_drawdown_pct": {"pct_bps": sl}},
            "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}],
            "entry_triggers": tr, "risk_limits": rl,
            "portfolio_weight_pct": str(w)}


# Breadth-proxy long gates (the key variable). Short gate stays the same.
LONG_GATES = {
    "base": ["{S}.close > {S}.ema(50)", "{S}.ema(50) > {S}.ema(200)", "BTCUSDT.close > BTCUSDT.ema(50)"],
    "btc_ema200": ["{S}.close > {S}.ema(50)", "{S}.ema(50) > {S}.ema(200)", "BTCUSDT.close > BTCUSDT.ema(200)"],
    "btc_ema100_strict": ["{S}.close > {S}.ema(50)", "{S}.ema(50) > {S}.ema(200)", "BTCUSDT.close > BTCUSDT.ema(100)"],
    "btc_atr_low": ["{S}.close > {S}.ema(50)", "{S}.ema(50) > {S}.ema(200)", "BTCUSDT.close > BTCUSDT.ema(50)", "BTCUSDT.atr_percent(14) < 4"],
    "btc_adx_strong": ["{S}.close > {S}.ema(50)", "{S}.ema(50) > {S}.ema(200)", "BTCUSDT.close > BTCUSDT.ema(50)", "adx(14) > 20"],
    "no_btc_gate": ["{S}.close > {S}.ema(50)", "{S}.ema(50) > {S}.ema(200)"],
}
SHORT_MID = ["{S}.close < {S}.ema(50)", "BTCUSDT.close < BTCUSDT.ema(50)"]
# Short gates that might capture 2025 better
SHORT_GATES = {
    "base": SHORT_MID,
    "loose": ["{S}.close < {S}.ema(100)", "BTCUSDT.close < BTCUSDT.ema(50)"],
    "btc_ema200": ["{S}.close < {S}.ema(50)", "BTCUSDT.close < BTCUSDT.ema(200)"],
    "no_btc": ["{S}.close < {S}.ema(50)"],
}


def build(long_gate_name, short_gate_name, cooldown_h):
    longs = ["BNBUSDT", "TRXUSDT", "BCHUSDT"]
    shorts = ["AAVEUSDT", "SOLUSDT", "DOTUSDT"]
    cd_s = cooldown_h * 3600
    long_tp = partial_tp([800, 1600, 2600])
    short_tp = partial_tp([800, 1600, 2600])
    lg = LONG_GATES[long_gate_name]
    sg = SHORT_GATES[short_gate_name]
    wl, ws = 13.3, 8.0
    strat = [mk(f"L{i}-{s}", s, "long", [x.format(S=s) for x in lg], wl, cd_s, long_tp, "rsi(14) < 45")
             for i, s in enumerate(longs)]
    strat += [mk(f"S{i}-{s}", s, "short", [x.format(S=s) for x in sg], ws, cd_s, short_tp, None)
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
    ap.add_argument("--workers", type=int, default=18)
    args = ap.parse_args()

    cooldowns = [11, 12]  # P1 found 11h best; include 12h for comparison
    jobs = []
    for cd in cooldowns:
        for lg in LONG_GATES:
            for sg in SHORT_GATES:
                cfg = build(lg, sg, cd)
                lbl = f"cd{cd}-L{lg[:5]}S{sg[:5]}"
                jobs.append((cfg, lbl))

    print(f"[r3P6] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
            print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):30s} "
                  f"ann={fm.get('ann')} dd={fm.get('dd')} pos={rec.get('positive_segments')}/5 "
                  f"2025={rec.get('segment_returns',{}).get('2025')}", flush=True)

    for v in results:
        if "error" in v:
            continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["frontier_improvement"] = (
            (ann > 34.5 or (v["segment_returns"] or {}).get("2025", -99) > -10.1
             or v["positive_segments"] >= 5)
            and v["positive_segments"] >= 4 and v["agg_2024_2026"] > 0)

    results.sort(key=lambda v: ((v.get("positive_segments") or 0),
                                (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results,
               "frontier_improvements": [v for v in results if v.get("frontier_improvement")]},
              open(args.out, "w"), indent=2, default=str)
    print(f"\n[r3P6] wrote {args.out} in {time.time()-t0:.0f}s")
    print("\n=== TOP 10 by pos_segs then ann ===")
    for v in results[:10]:
        if "error" in v:
            continue
        fm = v.get("full_metrics") or {}
        s25 = (v.get("segment_returns") or {}).get("2025", "n/a")
        print(f"  {v['label']:30s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} "
              f"pos={v['positive_segments']}/5 2025={s25}")


if __name__ == "__main__":
    main()
