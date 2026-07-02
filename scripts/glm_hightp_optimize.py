#!/usr/bin/env python3
"""GLM Martingale Core — High-TP frontier optimization (post-cliff-break).

The cliff was broken: TP=2200 + mult=2.8 strict-gate broad6 = ann 21.9%, DD 26.1%,
3/5 pos, agg24-26 +16.8%. This searches the surrounding space to maximize ann
while keeping DD<=30 and pos_segs>=3.

Search dims: TP (2000-2600), long mult (2.5-3.2), long legs (8-9), short mult
(2.0-2.8), gate style (strict/mid for long; strict/mid for short), symbol set
variations, weights. Segment-first validation (5 segments + full).

Usage:
  python3 scripts/glm_hightp_optimize.py \
      --out docs/superpowers/artifacts/glm-martingale-core/hightp-optimize.json
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
LONG_MID = ["{S}.close > {S}.ema(50)", "BTCUSDT.close > BTCUSDT.ema(50)"]
SHORT_STRICT = ["{S}.close < {S}.ema(50)", "{S}.ema(50) < {S}.ema(200)",
                "BTCUSDT.close < BTCUSDT.ema(50)"]
SHORT_MID = ["{S}.close < {S}.ema(50)", "BTCUSDT.close < BTCUSDT.ema(50)"]
LGATES = {"strict": LONG_STRICT, "mid": LONG_MID}
SGATES = {"strict": SHORT_STRICT, "mid": SHORT_MID}


def run_replay(config, budget, s, e, profile, pid):
    p = f"/tmp/glm_ho_{os.getpid()}_{pid}.json"
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
            "ret": o.get("total_return_pct"), "breached": o.get("principal_breached"),
            "trades": r.get("trade_count")}


def mk(sid, sym, direction, gates, mult, legs, fq, tp, sl, w):
    tr = [{"cooldown": {"seconds": 21600}}]
    for g in gates:
        tr.append({"indicator_expression": {"expression": g}})
    return {"strategy_id": sid, "symbol": sym, "market": "usd_m_futures",
            "direction": direction, "direction_mode": "long_and_short",
            "margin_mode": "isolated", "leverage": 10,
            "spacing": {"fixed_percent": {"step_bps": 150}},
            "sizing": {"multiplier": {"first_order_quote": str(fq),
                       "multiplier": str(mult), "max_legs": legs}},
            "take_profit": {"percent": {"bps": tp}},
            "stop_loss": {"strategy_drawdown_pct": {"pct_bps": sl}},
            "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}],
            "entry_triggers": tr,
            "risk_limits": {"max_active_cycles": None, "max_global_budget_quote": None,
                "max_symbol_budget_quote": None, "max_direction_budget_quote": None,
                "max_strategy_budget_quote": None, "max_global_drawdown_quote": None,
                "safety_skip_adx_threshold": 35},
            "portfolio_weight_pct": str(w)}


def build(longs, shorts, gl, gs, ml, ll, ms, ls_, tp, weights):
    wl, ws = weights
    strat = []
    for i, s in enumerate(longs):
        strat.append(mk(f"ho-L{i}-{s}", s, "long",
                        [x.format(S=s) for x in LGATES[gl]], ml, ll, 35.0, tp, 5000, wl))
    for i, s in enumerate(shorts):
        strat.append(mk(f"ho-S{i}-{s}", s, "short",
                        [x.format(S=s) for x in SGATES[gs]], ms, ls_, 30.0, tp, 4000, ws))
    return {"direction_mode": "long_and_short", "strategies": strat,
            "risk_limits": {"max_global_budget_quote": "5000"}}


def full_eval(args):
    config, label = args
    seg_m = {}
    for name, s, e in FULL_SEGMENTS:
        seg_m[name] = metrics(run_replay(config, 5000, s, e, "aggressive", label[:10] + name))
    full_m = metrics(run_replay(config, 5000, FULL_START, FULL_END, "aggressive", label[:10] + "fl"))
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
    ap.add_argument("--workers", type=int, default=28)
    args = ap.parse_args()

    long_sets = [["BNBUSDT", "TRXUSDT", "BCHUSDT"],
                 ["BNBUSDT", "TRXUSDT", "BCHUSDT", "ADAUSDT"]]
    short_sets = [["AAVEUSDT", "SOLUSDT", "DOTUSDT"],
                  ["AAVEUSDT", "SOLUSDT", "DOTUSDT", "NEARUSDT"],
                  ["AAVEUSDT", "DOTUSDT", "NEARUSDT"]]
    gate_pairs = [("strict", "strict"), ("strict", "mid"), ("mid", "strict")]
    tps = [2000, 2200, 2400, 2600]
    mults = [(2.5, 9, 2.0, 7), (2.8, 9, 2.5, 8), (3.0, 9, 2.5, 8), (2.8, 8, 2.5, 7)]
    weight_sets = [(13.3, 8.0), (15.0, 7.0), (12.0, 9.0)]

    jobs = []
    for ls in long_sets:
        for ss in short_sets:
            for gl, gs in gate_pairs:
                for tp in tps:
                    for ml, ll, ms, ls_n in mults:
                        for wl, ws in weight_sets:
                            cfg = build(ls, ss, gl, gs, ml, ll, ms, ls_n, tp, (wl, ws))
                            lbl = (f"L{len(ls)}S{len(ss)}-{gl[0]}{gs[0]}"
                                   f"-t{tp}-m{ml}l{ll}-{wl}w{ws}")
                            jobs.append((cfg, lbl))

    print(f"[glm-ho] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
            if done % 60 == 0 or (ann > 21 and dd <= 30 and rec.get("positive_segments",0) >= 3):
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):28s} "
                      f"ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 "
                      f"agg2426={rec.get('agg_2024_2026')}", flush=True)

    for v in results:
        if "error" in v:
            continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        # aggressive pass (relax h1_contrib to 100 since it's a metric artifact when 2024+2026 positive)
        v["pass_agg"] = (ann > 110 and dd <= 30 and v["positive_segments"] >= 3
                         and v["agg_2024_2026"] > 0 and not fm.get("breached"))
        v["near_agg"] = (ann > 20 and dd <= 30 and v["positive_segments"] >= 3
                         and v["agg_2024_2026"] > 0 and not fm.get("breached"))

    results.sort(key=lambda v: ((v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
               "n_candidates": len(jobs), "results": results,
               "passes": [v for v in results if v.get("pass_agg")]},
              open(args.out, "w"), indent=2, default=str)
    npass = len([v for v in results if v.get("pass_agg")])
    nnear = len([v for v in results if v.get("near_agg")])
    print(f"\n[glm-ho] wrote {args.out}: {len(results)} results, {npass} passes, "
          f"{nnear} near(ann>20,dd<=30,pos>=3) in {time.time()-t0:.0f}s", flush=True)
    print("\n=== TOP 15 by ann (with dd<=32 filter) ===")
    filt = [v for v in results if "error" not in v and
            ((v.get("full_metrics") or {}).get("dd") or 999) <= 32]
    filt.sort(key=lambda v: (v.get("full_metrics") or {}).get("ann") or 0, reverse=True)
    for v in filt[:15]:
        fm = v.get("full_metrics") or {}
        print(f"  {v['label']:28s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} "
              f"pos={v['positive_segments']}/5 agg2426={v['agg_2024_2026']:7.1f} h1c={v['h1_2023_contribution']:5.1f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
