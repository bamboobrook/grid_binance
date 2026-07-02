#!/usr/bin/env python3
"""GLM Round 4 P4: Active-Cycle Stale Exit search (full 5-seg validation).

Tests max_cycle_age and no_progress_exit on r3-P1-best structure. The 2025
attribution showed 50% stop rate — stale cycles may be wasting capital.
Active exits force-close stale cycles, potentially improving turnover.
"""
import argparse, json, os, subprocess, sys, time
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999
LONG_STRICT = ["{S}.close > {S}.ema(50)", "{S}.ema(50) > {S}.ema(200)", "BTCUSDT.close > BTCUSDT.ema(50)"]
SHORT_MID = ["{S}.close < {S}.ema(50)", "BTCUSDT.close < BTCUSDT.ema(50)"]

def run_replay(config, budget, s, e, pid):
    p = f"/tmp/r4P4f_{os.getpid()}_{pid}.json"
    json.dump({"portfolio_config": config}, open(p, "w"))
    cmd = [REPLAY, "--config", p, "--budget", str(budget), "--start-ms", str(s), "--end-ms", str(e), "--market-data", MARKET_DB, "--funding-data", FUNDING_DB, "--profile", "aggressive", "--portfolio-id", pid, "--exchange-min-notional", "5"]
    try:
        pr = subprocess.run(cmd, capture_output=True, text=True, timeout=900)
    except subprocess.TimeoutExpired:
        return {"error": "timeout"}
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

def ptp(bps):
    orig=[0.30,0.30,0.40]; rem=1.0; stages=[]
    for i,of in enumerate(orig):
        if i==2: stages.append([1,1,bps[i]])
        else:
            rf=min(max(of/rem if rem>0 else 1,0),1); stages.append([int(round(rf*1000)),1000,bps[i]]); rem*=1-rf
    return {"partial":{"stages":stages,"breakeven_after_stage":1,"breakeven_buffer_bps":100}}

def mk(sid,sym,d,gates,w,max_age,np_hours,np_mfe):
    tr=[{"cooldown":{"seconds":39600}}]  # 11h
    for g in gates: tr.append({"indicator_expression":{"expression":g}})
    fq=30.0 if d=='short' else 35.0; sl=4000 if d=='short' else 5000; ms=2.5 if d=='short' else 2.8
    rl={"max_active_cycles":None,"max_global_budget_quote":None,"max_symbol_budget_quote":None,"max_direction_budget_quote":None,"max_strategy_budget_quote":None,"max_global_drawdown_quote":None,"safety_skip_adx_threshold":35}
    if d=='long': rl["safety_order_condition"]="rsi(14) < 45"
    if max_age: rl["max_cycle_age_hours"]=max_age
    if np_hours and np_mfe:
        rl["no_progress_exit_hours"]=np_hours
        rl["no_progress_mfe_bps"]=np_mfe
    return {"strategy_id":sid,"symbol":sym,"market":"usd_m_futures","direction":d,"direction_mode":"long_and_short","margin_mode":"isolated","leverage":10,"spacing":{"fixed_percent":{"step_bps":150}},"sizing":{"multiplier":{"first_order_quote":str(fq),"multiplier":str(ms),"max_legs":8}},"take_profit":ptp([800,1600,2600]),"stop_loss":{"strategy_drawdown_pct":{"pct_bps":sl}},"indicators":[{"atr":{"period":14}},{"adx":{"period":14}}],"entry_triggers":tr,"risk_limits":rl,"portfolio_weight_pct":str(w)}

def build(max_age, np_hours, np_mfe):
    longs=["BNBUSDT","TRXUSDT","BCHUSDT"]; shorts=["AAVEUSDT","SOLUSDT","DOTUSDT"]
    strat=[mk(f"L{i}-{s}",s,"long",[x.format(S=s) for x in LONG_STRICT],13.3,max_age,np_hours,np_mfe) for i,s in enumerate(longs)]
    strat+=[mk(f"S{i}-{s}",s,"short",[x.format(S=s) for x in SHORT_MID],8.0,max_age,np_hours,np_mfe) for i,s in enumerate(shorts)]
    return {"direction_mode":"long_and_short","strategies":strat,"risk_limits":{"max_global_budget_quote":"5000"}}

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
    ap.add_argument("--workers", type=int, default=20)
    args = ap.parse_args()
    # Grid: max_cycle_age (None, 24h, 48h, 72h, 120h) × no_progress (None, 24h/200bps, 48h/200bps, 24h/400bps)
    ages = [None, 24, 48, 72, 120]
    nps = [(None, None), (24, 200), (48, 200), (24, 400), (48, 400)]
    jobs = []
    for age in ages:
        for np_h, np_m in nps:
            cfg = build(age, np_h, np_m)
            lbl = f"age{age}_np{np_h}_{np_m}"
            jobs.append((cfg, lbl))
    print(f"[r4P4] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
            print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):24s} ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 2025={rec.get('segment_returns',{}).get('2025')}", flush=True)
    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["frontier_improvement"] = ((ann > 34.5 or (v["segment_returns"] or {}).get("2025", -99) > -10.1 or v["positive_segments"] >= 5) and v["positive_segments"] >= 4 and v["agg_2024_2026"] > 0)
    results.sort(key=lambda v: ((v.get("positive_segments") or 0), (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results, "frontier_improvements": [v for v in results if v.get("frontier_improvement")]}, open(args.out, "w"), indent=2, default=str)
    print(f"\n[r4P4] wrote {args.out} in {time.time()-t0:.0f}s")
    print("\n=== ALL by pos_segs then ann ===")
    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        s25 = (v.get("segment_returns") or {}).get("2025", "n/a")
        print(f"  {v['label']:24s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} pos={v['positive_segments']}/5 2025={s25}")

if __name__ == "__main__":
    main()
