#!/usr/bin/env python3
"""GLM Round 3 P3: Partial TP + Cooldown fine search (segment-first pruning).

P1 found cd11h optimal at ann 34.5%/DD 17.8%. This fine-scans around it with
separate long/short cooldowns, more TP ladders, and TP splits. Segment-first:
runs 2025+2026 first, only survivors get full 5-seg validation.

Grid per plan section 9.
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
SEG_2025 = (1735689600000, 1767225599999)
SEG_2026 = (1767225600000, 1780271999999)


def run_replay(config, budget, s, e, pid):
    p = f"/tmp/r3P3_{os.getpid()}_{pid}.json"
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


def partial_tp(stages_bps, split_orig, be_after, be_buf):
    rem = 1.0
    stages = []
    for i, of in enumerate(split_orig):
        if i == len(split_orig) - 1:
            stages.append([1, 1, stages_bps[i]])
        else:
            rf = of / rem if rem > 0 else 1.0
            rf = min(max(rf, 0.0), 1.0)
            stages.append([int(round(rf * 1000)), 1000, stages_bps[i]])
            rem *= (1.0 - rf)
    return {"partial": {"stages": stages, "breakeven_after_stage": be_after,
                        "breakeven_buffer_bps": be_buf}}


def mk(sid, sym, d, gates, w, cooldown_s, tp_model):
    tr = [{"cooldown": {"seconds": cooldown_s}}]
    for g in gates:
        tr.append({"indicator_expression": {"expression": g}})
    fq = 30.0 if d == "short" else 35.0
    sl = 4000 if d == "short" else 5000
    mult = 2.5 if d == "short" else 2.8
    rl = {"max_active_cycles": None, "max_global_budget_quote": None,
          "max_symbol_budget_quote": None, "max_direction_budget_quote": None,
          "max_strategy_budget_quote": None, "max_global_drawdown_quote": None,
          "safety_skip_adx_threshold": 35, "safety_order_condition": "rsi(14) < 45"}
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


def build(long_cd, short_cd, long_tp_bps, short_tp_bps, split, be_after, be_buf):
    longs = ["BNBUSDT", "TRXUSDT", "BCHUSDT"]
    shorts = ["AAVEUSDT", "SOLUSDT", "DOTUSDT"]
    long_tp = partial_tp(long_tp_bps, split, be_after, be_buf)
    short_tp = partial_tp(short_tp_bps, split, be_after, be_buf)
    wl, ws = 13.3, 8.0
    strat = [mk(f"L{i}-{s}", s, "long", [x.format(S=s) for x in LONG_STRICT], wl, long_cd, long_tp)
             for i, s in enumerate(longs)]
    strat += [mk(f"S{i}-{s}", s, "short", [x.format(S=s) for x in SHORT_MID], ws, short_cd, short_tp)
              for i, s in enumerate(shorts)]
    return {"direction_mode": "long_and_short", "strategies": strat,
            "risk_limits": {"max_global_budget_quote": "5000"}}


def stage1_eval(args):
    """Run 2025 + 2026 only for pruning."""
    config, label = args
    m25 = metrics(run_replay(config, 5000, SEG_2025[0], SEG_2025[1], label[:8] + "s25"))
    m26 = metrics(run_replay(config, 5000, SEG_2026[0], SEG_2026[1], label[:8] + "s26"))
    survives = bool(
        m25 and m26 and (m25["ret"] or 0) > -15 and (m26["ret"] or 0) > 0
        and (m25["dd"] or 999) <= 25 and not m25.get("breached"))
    return {"label": label, "config": config, "m25": m25, "m26": m26, "survives": survives}


def full_eval(rec):
    config, label = rec["config"], rec["label"]
    seg_m = {}
    for name, s, e in FULL_SEGMENTS:
        seg_m[name] = metrics(run_replay(config, 5000, s, e, label[:8] + name))
    full_m = metrics(run_replay(config, 5000, FULL_START, FULL_END, label[:8] + "fl"))
    pos = sum(1 for v in seg_m.values() if v and v["ret"] and v["ret"] > 0)
    rets = {n: (v["ret"] if v and v["ret"] else 0.0) for n, v in seg_m.items()}
    agg = sum(rets.get(k, 0) for k in ("2024", "2025", "2026_ytd"))
    return {"label": label, "config": config, "segment_metrics": seg_m,
            "full_metrics": full_m, "positive_segments": pos, "agg_2024_2026": agg}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=20)
    args = ap.parse_args()

    long_cds = [9, 10, 10.5, 11, 11.5, 12, 13]
    short_cds = [8, 10, 11, 12, 14]
    long_tps = [(700, 1400, 2400), (800, 1600, 2600), (900, 1800, 3000)]
    short_tps = [(600, 1200, 2200), (800, 1600, 2600)]
    splits = [[0.25, 0.35, 0.40], [0.30, 0.30, 0.40], [0.35, 0.35, 0.30], [0.40, 0.30, 0.30]]
    be_afters = [0, 1]
    be_bufs = [50, 100, 150]

    jobs = []
    for lc in long_cds:
        for sc in short_cds:
            for ltp in long_tps:
                for stp in short_tps:
                    for sp in splits:
                        for bea in be_afters:
                            for beb in be_bufs:
                                cfg = build(int(lc*3600), int(sc*3600), list(ltp), list(stp), sp, bea, beb)
                                lbl = f"lc{lc}sc{sc}-lt{ltp[0]}st{stp[0]}sp{int(sp[0]*100)}be{bea}b{beb}"
                                jobs.append((cfg, lbl))

    print(f"[r3P3] stage1: {len(jobs)} candidates x 2 replays (2025+2026)", flush=True)
    t0 = time.time()
    stage1 = []
    with ProcessPoolExecutor(max_workers=args.workers) as ex:
        futs = {ex.submit(stage1_eval, j): j for j in jobs}
        done = 0
        for fut in as_completed(futs):
            try:
                rec = fut.result()
            except Exception as e:
                rec = {"label": "?", "error": str(e), "survives": False}
            stage1.append(rec)
            done += 1
            if done % 100 == 0:
                surv = sum(1 for r in stage1 if r.get("survives"))
                print(f"  [stage1 {done}/{len(jobs)}] survivors so far: {surv}", flush=True)

    survivors = [r for r in stage1 if r.get("survives")]
    print(f"\n[r3P3] stage1 done: {len(survivors)}/{len(jobs)} survived in {time.time()-t0:.0f}s", flush=True)

    # Full validation of survivors
    print(f"[r3P3] stage2: full-validating {len(survivors)} survivors (5 segs + full)", flush=True)
    full_results = []
    for i, rec in enumerate(survivors):
        v = full_eval(rec)
        full_results.append(v)
        fm = v.get("full_metrics") or {}
        if (i + 1) % 20 == 0 or ((fm.get("ann") or 0) > 30):
            print(f"  [stage2 {i+1}/{len(survivors)}] {v['label']:38s} "
                  f"ann={fm.get('ann')} dd={fm.get('dd')} pos={v['positive_segments']}/5", flush=True)

    for v in full_results:
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["near_target"] = (ann >= 50 and dd <= 30 and v["positive_segments"] >= 3
                            and v["agg_2024_2026"] > 0)
        v["frontier_improvement"] = ((ann > 34.5 or dd < 17.8)
                                     and v["positive_segments"] >= 4 and v["agg_2024_2026"] > 0)

    full_results.sort(key=lambda v: ((v.get("positive_segments") or 0),
                                     (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"stage1_candidates": len(jobs), "stage1_survivors": len(survivors),
               "full_validated": len(full_results), "results": full_results,
               "near_targets": [v for v in full_results if v.get("near_target")],
               "promoted": [v for v in full_results if v.get("frontier_improvement")]},
              open(args.out, "w"), indent=2, default=str)
    nt = len([v for v in full_results if v.get("near_target")])
    print(f"\n[r3P3] wrote {args.out}: {len(full_results)} validated, {nt} near_target in {time.time()-t0:.0f}s")
    print("\n=== TOP 10 by pos_segs then ann ===")
    for v in full_results[:10]:
        if "error" in v:
            continue
        fm = v.get("full_metrics") or {}
        print(f"  {v['label']:38s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} "
              f"pos={v['positive_segments']}/5 agg={v['agg_2024_2026']:7.1f}")


if __name__ == "__main__":
    main()
