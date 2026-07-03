#!/usr/bin/env python3
"""GLM Round 5 Task E: Custom Price Ladder with Dip and Breakout Legs.

Tests custom adverse ladders (dip averaging) using CustomSequence spacing+sizing.
The engine already supports CustomSequence — no new engine feature needed.
Per plan: adverse-only first, then bounded favorable-add if promising.

Grid per plan section 11:
  - adverse steps long: 3 ladder shapes
  - adverse steps short: 3 ladder shapes
  - notionals long: 3 sizing sequences
  - notionals short: 3 sizing sequences
  - TP ladder: 600/1200/2200, 800/1600/2600
  - With last-executed SO basis + vol-target from Task B/D best
"""
import argparse, json, os, subprocess, sys, time
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999
LONG_STRICT = ["{S}.close > {S}.ema(50)", "{S}.ema(50) > {S}.ema(200)", "BTCUSDT.close > BTCUSDT.ema(50)"]

def run_replay(config, budget, s, e, pid):
    p = f"/tmp/r5Ef_{os.getpid()}_{pid}.json"
    json.dump({"portfolio_config": config}, open(p, "w"))
    cmd = [REPLAY, "--config", p, "--budget", str(budget), "--start-ms", str(s), "--end-ms", str(e), "--market-data", MARKET_DB, "--funding-data", FUNDING_DB, "--profile", "aggressive", "--portfolio-id", pid, "--exchange-min-notional", "5"]
    try: pr = subprocess.run(cmd, capture_output=True, text=True, timeout=900)
    except subprocess.TimeoutExpired: return {"error": "timeout"}
    finally:
        try: os.remove(p)
        except: pass
    if pr.returncode != 0: return {"error": pr.stderr.strip()[:500]}
    try: return json.loads(pr.stdout)
    except: return {"error": pr.stdout[:500]}

def metrics(r):
    if "error" in r or "on_budget" not in r: return None
    o = r["on_budget"]
    return {"ann": o.get("annualized_return_pct"), "dd": o.get("max_drawdown_pct"), "ret": o.get("total_return_pct"), "breached": o.get("principal_breached")}

def ptp(bps, be_after=1, be_buf=100):
    orig=[0.30,0.30,0.40]; rem=1.0; stages=[]
    for i,of in enumerate(orig):
        if i==2: stages.append([1,1,bps[i]])
        else: rf=min(max(of/rem if rem>0 else 1,0),1); stages.append([int(round(rf*1000)),1000,bps[i]]); rem*=1-rf
    return {"partial":{"stages":stages,"breakeven_after_stage":be_after,"breakeven_buffer_bps":be_buf}}

def mk_custom(sid, sym, d, gates, w, spacing_steps, sizing_notionals, tp_bps, vol_target):
    tr = [{"cooldown": {"seconds": 39600}}]
    for g in gates: tr.append({"indicator_expression": {"expression": g}})
    rl = {"max_active_cycles": None, "max_global_budget_quote": None, "max_symbol_budget_quote": None, "max_direction_budget_quote": None, "max_strategy_budget_quote": None, "max_global_drawdown_quote": None, "safety_skip_adx_threshold": 35, "safety_order_basis": "last_executed_order"}
    if d == "long": rl["safety_order_condition"] = "rsi(14) < 45"
    if vol_target:
        rl["vol_target_atr_pct"] = vol_target
        rl["vol_target_min_scale"] = 0.5
        rl["vol_target_max_scale"] = 1.5
    sl = 5000 if d == "long" else 3000
    spacing = {"custom_sequence": {"steps_bps": spacing_steps}}
    sizing = {"custom_sequence": {"notionals": [str(n) for n in sizing_notionals]}}
    return {"strategy_id": sid, "symbol": sym, "market": "usd_m_futures", "direction": d, "direction_mode": "long_and_short", "margin_mode": "isolated", "leverage": 10, "spacing": spacing, "sizing": sizing, "take_profit": ptp(tp_bps), "stop_loss": {"strategy_drawdown_pct": {"pct_bps": sl}}, "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}], "entry_triggers": tr, "risk_limits": rl, "portfolio_weight_pct": str(w)}

# Ladder shapes per plan
ADVERSE_LONG = {
    "tight": [120,240,420,700,1050],
    "medium": [150,300,600,900,1300],
    "wide": [200,450,800,1250,1800],
}
ADVERSE_SHORT = {
    "tight": [150,350,650,1000],
    "medium": [220,500,900,1400],
    "wide": [300,700,1200,1900],
}
NOTIONALS_LONG = {
    "mild": [25,45,80,140,220],
    "base": [30,55,95,160,260],
    "heavy": [35,70,135,240,420],
}
NOTIONALS_SHORT = {
    "mild": [20,32,52,85],
    "base": [25,42,70,115],
    "heavy": [30,50,85,140],
}
TP_LADDERS = [(600,1200,2200), (800,1600,2600)]

def build(long_ladder, short_ladder, long_notional, short_notional, tp_bps, vol_target):
    longs = ["BNBUSDT", "TRXUSDT", "BCHUSDT"]; shorts = ["AAVEUSDT", "SOLUSDT", "DOTUSDT"]
    strat = []
    for i, s in enumerate(longs):
        gates = [x.replace("{S}", s) for x in LONG_STRICT]
        strat.append(mk_custom(f"L{i}-{s}", s, "long", gates, 13.3, ADVERSE_LONG[long_ladder], NOTIONALS_LONG[long_notional], tp_bps, vol_target))
    for i, s in enumerate(shorts):
        gates = [f"roc(720) > 18", f"rsi(14) > 65"]
        strat.append(mk_custom(f"S{i}-{s}", s, "short", gates, 8.0, ADVERSE_SHORT[short_ladder], NOTIONALS_SHORT[short_notional], tp_bps, vol_target))
    return {"direction_mode": "long_and_short", "strategies": strat, "risk_limits": {"max_global_budget_quote": "5000"}}

def full_eval(args):
    config, label = args
    seg_m = {}
    for name, s, e in FULL_SEGMENTS:
        seg_m[name] = metrics(run_replay(config, 5000, s, e, label[:8] + name))
    full_m = metrics(run_replay(config, 5000, FULL_START, FULL_END, label[:8] + "fl"))
    pos = sum(1 for v in seg_m.values() if v and v["ret"] and v["ret"] > 0)
    rets = {n: (v["ret"] if v and v["ret"] else 0.0) for n, v in seg_m.items()}
    agg = sum(rets.get(k, 0) for k in ("2024", "2025", "2026_ytd"))
    return {"label": label, "config": config, "segment_metrics": seg_m, "full_metrics": full_m, "positive_segments": pos, "agg_2024_2026": agg, "segment_returns": rets}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=18)
    args = ap.parse_args()
    vol_targets = [None, 1.0]
    jobs = []
    for ll_name in ADVERSE_LONG:
        for sl_name in ADVERSE_SHORT:
            for ln_name in NOTIONALS_LONG:
                for sn_name in NOTIONALS_SHORT:
                    for tp in TP_LADDERS:
                        for vt in vol_targets:
                            cfg = build(ll_name, sl_name, ln_name, sn_name, list(tp), vt)
                            lbl = f"al{ll_name[:3]}as{sl_name[:3]}nl{ln_name[:3]}ns{sn_name[:3]}tp{tp[0]}vt{vt}"
                            jobs.append((cfg, lbl))
    print(f"[r5E] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
    t0 = time.time()
    results = []
    with ProcessPoolExecutor(max_workers=args.workers) as ex:
        futs = {ex.submit(full_eval, j): j for j in jobs}
        done = 0
        for fut in as_completed(futs):
            try: rec = fut.result()
            except Exception as e: rec = {"label": "?", "error": str(e), "positive_segments": 0}
            results.append(rec)
            done += 1
            fm = rec.get("full_metrics") or {}
            ann = fm.get("ann") or 0; dd = fm.get("dd") or 999
            if done % 10 == 0 or (ann > 40 and rec.get("positive_segments",0)>=4):
                s25 = rec.get("segment_returns",{}).get("2025","?")
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):36s} ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 2025={s25}", flush=True)
    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["frontier_improvement"] = (ann > 45.1 and v["positive_segments"] >= 4 and v["agg_2024_2026"] > 0)
    results.sort(key=lambda v: ((v.get("positive_segments") or 0), (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results}, open(args.out, "w"), indent=2, default=str)
    nfi = len([v for v in results if v.get("frontier_improvement")])
    print(f"\n[r5E] wrote {args.out}: {len(results)} results, {nfi} frontier_improvement in {time.time()-t0:.0f}s")
    print("\n=== TOP 15 by pos_segs then ann ===")
    for v in results[:15]:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        s25 = (v.get("segment_returns") or {}).get("2025","n/a")
        print(f"  {v['label']:36s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} pos={v['positive_segments']}/5 2025={s25}")

if __name__ == "__main__":
    main()
