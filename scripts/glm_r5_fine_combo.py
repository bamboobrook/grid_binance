#!/usr/bin/env python3
"""GLM Round 5 Final: Fine-combo search to break 50% Conservative target.

Base: last-executed SO + vol-target (ann 45.1%). Fine-scan around the sweet spot:
  long_mult: 3.0-3.5
  long_step: 120-180
  short_mult: 1.6-2.2
  short_step: 150-250
  long_legs: 7-9
  short_legs: 4-6
  TP ladder: 600/1200/2200, 700/1400/2400, 800/1600/2600
  vol_target: 1.0, 1.5, None
  cooldown: 10h, 11h, 12h

Full 5-segment validation for every candidate.
"""
import argparse, json, os, subprocess, sys, time, random
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999
LONG_STRICT = ["{S}.close > {S}.ema(50)", "{S}.ema(50) > {S}.ema(200)", "BTCUSDT.close > BTCUSDT.ema(50)"]

def run_replay(config, budget, s, e, pid):
    p = f"/tmp/r5Fc_{os.getpid()}_{pid}.json"
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

def mk(sid, sym, d, gates, w, mult, legs, step, tp_bps, vol_fields, cd_s):
    tr = [{"cooldown": {"seconds": cd_s}}]
    for g in gates: tr.append({"indicator_expression": {"expression": g}})
    rl = {"max_active_cycles": None, "max_global_budget_quote": None, "max_symbol_budget_quote": None, "max_direction_budget_quote": None, "max_strategy_budget_quote": None, "max_global_drawdown_quote": None, "safety_skip_adx_threshold": 35, "safety_order_basis": "last_executed_order"}
    if d == "long": rl["safety_order_condition"] = "rsi(14) < 45"
    rl.update(vol_fields)
    fq = "35.0" if d == "long" else "25.0"
    sl = 5000 if d == "long" else 3000
    return {"strategy_id": sid, "symbol": sym, "market": "usd_m_futures", "direction": d, "direction_mode": "long_and_short", "margin_mode": "isolated", "leverage": 10, "spacing": {"fixed_percent": {"step_bps": step}}, "sizing": {"multiplier": {"first_order_quote": fq, "multiplier": str(mult), "max_legs": legs}}, "take_profit": ptp(tp_bps), "stop_loss": {"strategy_drawdown_pct": {"pct_bps": sl}}, "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}], "entry_triggers": tr, "risk_limits": rl, "portfolio_weight_pct": str(w)}

def build(long_mult, short_mult, long_step, short_step, long_legs, short_legs, tp_bps, vol_target, cd_h):
    longs = ["BNBUSDT", "TRXUSDT", "BCHUSDT"]; shorts = ["AAVEUSDT", "SOLUSDT", "DOTUSDT"]
    vol = {}
    if vol_target:
        vol["vol_target_atr_pct"] = vol_target
        vol["vol_target_min_scale"] = 0.5
        vol["vol_target_max_scale"] = 1.5
    cd_s = cd_h * 3600
    strat = []
    for i, s in enumerate(longs):
        gates = [x.replace("{S}", s) for x in LONG_STRICT]
        strat.append(mk(f"L{i}-{s}", s, "long", gates, 13.3, long_mult, long_legs, long_step, tp_bps, vol, cd_s))
    for i, s in enumerate(shorts):
        gates = [f"roc(720) > 18", f"rsi(14) > 65"]
        strat.append(mk(f"S{i}-{s}", s, "short", gates, 8.0, short_mult, short_legs, short_step, tp_bps, vol, cd_s))
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
    ap.add_argument("--workers", type=int, default=24)
    args = ap.parse_args()
    long_mults = [3.0, 3.1, 3.2, 3.3, 3.4, 3.5]
    short_mults = [1.6, 1.8, 2.0, 2.2]
    long_steps = [120, 150, 180]
    short_steps = [150, 180, 250]
    long_legs_opts = [7, 8, 9]
    short_legs_opts = [4, 5, 6]
    tp_ladders = [(600,1200,2200), (700,1400,2400), (800,1600,2600)]
    vol_targets = [None, 1.0]
    cds = [10, 11, 12]
    jobs = []
    for lm in long_mults:
        for sm in short_mults:
            for ls in long_steps:
                for ss in short_steps:
                    for ll in long_legs_opts:
                        for sl in short_legs_opts:
                            for tp in tp_ladders:
                                for vt in vol_targets:
                                    for cd in cds:
                                        cfg = build(lm, sm, ls, ss, ll, sl, list(tp), vt, cd)
                                        lbl = f"lm{lm}sm{sm}ls{ls}ss{ss}ll{ll}sl{sl}tp{tp[0]}vt{vt}cd{cd}"
                                        jobs.append((cfg, lbl))
    # Sample to ~500 for feasibility
    random.seed(42)
    if len(jobs) > 500:
        jobs = random.sample(jobs, 500)
    print(f"[r5Fc] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
            if done % 50 == 0 or (ann > 45 and rec.get("positive_segments",0)>=4):
                s25 = rec.get("segment_returns",{}).get("2025","?")
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):40s} ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 2025={s25}", flush=True)
    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["frontier_improvement"] = (ann > 45.1 and v["positive_segments"] >= 4 and v["agg_2024_2026"] > 0)
        v["target_pass"] = (ann > 50 and dd <= 10 and v["positive_segments"] >= 4)
    results.sort(key=lambda v: ((v.get("positive_segments") or 0), (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results, "frontier_improvements": [v for v in results if v.get("frontier_improvement")]}, open(args.out, "w"), indent=2, default=str)
    nfi = len([v for v in results if v.get("frontier_improvement")])
    ntp = len([v for v in results if v.get("target_pass")])
    print(f"\n[r5Fc] wrote {args.out}: {len(results)} results, {nfi} frontier_improvement, {ntp} target_pass in {time.time()-t0:.0f}s")
    print("\n=== TOP 15 by pos_segs then ann ===")
    for v in results[:15]:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        s25 = (v.get("segment_returns") or {}).get("2025","n/a")
        print(f"  {v['label']:40s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} pos={v['positive_segments']}/5 2025={s25}")

if __name__ == "__main__":
    main()
