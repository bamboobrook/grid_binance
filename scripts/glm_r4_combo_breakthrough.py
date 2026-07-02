#!/usr/bin/env python3
"""GLM Round 4 Final Combo: pump-fade short + direction-aware long + 4-stage TP.

This is the UNTESTED combination: P3 pump-fade short (roc-gated) + R3 P1 best
long (strict gate + cond SO) + 4-stage Partial TP + various cooldowns.
All mechanisms exist in the engine; this tests them combined.
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
    p = f"/tmp/r4Cf_{os.getpid()}_{pid}.json"
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

def mk_long(sid, sym, w, cd, tp_model, so_cond=None):
    tr = [{"cooldown": {"seconds": cd}}]
    for g in [x.replace("{S}", sym) for x in LONG_STRICT]: tr.append({"indicator_expression": {"expression": g}})
    rl = {"max_active_cycles": None, "max_global_budget_quote": None, "max_symbol_budget_quote": None, "max_direction_budget_quote": None, "max_strategy_budget_quote": None, "max_global_drawdown_quote": None, "safety_skip_adx_threshold": 35}
    if so_cond: rl["safety_order_condition"] = so_cond
    return {"strategy_id": sid, "symbol": sym, "market": "usd_m_futures", "direction": "long", "direction_mode": "long_and_short", "margin_mode": "isolated", "leverage": 10, "spacing": {"fixed_percent": {"step_bps": 150}}, "sizing": {"multiplier": {"first_order_quote": "35.0", "multiplier": "2.8", "max_legs": 8}}, "take_profit": tp_model, "stop_loss": {"strategy_drawdown_pct": {"pct_bps": 5000}}, "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}], "entry_triggers": tr, "risk_limits": rl, "portfolio_weight_pct": str(w)}

def mk_pump_fade_short(sid, sym, w, cd, roc_period, roc_thresh, rsi_thresh, tp_model, mult=1.8, legs=5, step=180):
    tr = [{"cooldown": {"seconds": cd}}]
    tr.append({"indicator_expression": {"expression": f"roc({roc_period}) > {roc_thresh}"}})
    tr.append({"indicator_expression": {"expression": f"rsi(14) > {rsi_thresh}"}})
    rl = {"max_active_cycles": None, "max_global_budget_quote": None, "max_symbol_budget_quote": None, "max_direction_budget_quote": None, "max_strategy_budget_quote": None, "max_global_drawdown_quote": None, "safety_skip_adx_threshold": 35}
    return {"strategy_id": sid, "symbol": sym, "market": "usd_m_futures", "direction": "short", "direction_mode": "long_and_short", "margin_mode": "isolated", "leverage": 10, "spacing": {"fixed_percent": {"step_bps": step}}, "sizing": {"multiplier": {"first_order_quote": "25.0", "multiplier": str(mult), "max_legs": legs}}, "take_profit": tp_model, "stop_loss": {"strategy_drawdown_pct": {"pct_bps": 3000}}, "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}], "entry_triggers": tr, "risk_limits": rl, "portfolio_weight_pct": str(w)}

def build(long_tp_bps, short_tp_bps, roc_period, roc_thresh, rsi_thresh, long_cd, short_cd, long_so):
    longs = ["BNBUSDT", "TRXUSDT", "BCHUSDT"]
    shorts = ["AAVEUSDT", "SOLUSDT", "DOTUSDT"]
    long_tp = ptp(long_tp_bps)
    short_tp = ptp(short_tp_bps)
    lw, sw = 13.3, 8.0
    strat = [mk_long(f"L{i}-{s}", s, lw, long_cd, long_tp, long_so) for i, s in enumerate(longs)]
    strat += [mk_pump_fade_short(f"PF{i}-{s}", s, sw, short_cd, roc_period, roc_thresh, rsi_thresh, short_tp) for i, s in enumerate(shorts)]
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
    ap.add_argument("--workers", type=int, default=20)
    args = ap.parse_args()
    # Grid: long TP × short TP × roc settings × cooldowns × long SO
    long_tps = [(800,1600,2600), (600,1200,2200)]
    short_tps = [(500,1000,1800), (300,700,1400), (800,1600,2600)]
    roc_settings = [(720, 12, 70), (720, 18, 65), (1440, 9, 70), (1440, 12, 65)]
    long_cds = [39600]  # 11h (P1 best)
    short_cds = [14400, 21600, 39600]  # 4h, 6h, 11h
    long_sos = ["rsi(14) < 45", None]
    jobs = []
    for ltp in long_tps:
        for stp in short_tps:
            for roc_p, roc_t, rsi_t in roc_settings:
                for lcd in long_cds:
                    for scd in short_cds:
                        for lso in long_sos:
                            cfg = build(list(ltp), list(stp), roc_p, roc_t, rsi_t, lcd, scd, lso)
                            lso_tag = "so45" if lso else "sono"
                            lbl = f"lt{ltp[0]}st{stp[0]}r{roc_p}t{roc_t}r{rsi_t}sc{scd}{lso_tag}"
                            jobs.append((cfg, lbl))
    print(f"[r4C] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
            if done % 40 == 0 or (ann > 35 and rec.get("positive_segments",0) >= 4):
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):36s} ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 2025={rec.get('segment_returns',{}).get('2025')}", flush=True)
    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["frontier_improvement"] = ((ann > 34.7 or (v["segment_returns"] or {}).get("2025",-99) > -9.96 or v["positive_segments"] >= 5) and v["positive_segments"] >= 4 and v["agg_2024_2026"] > 0)
    results.sort(key=lambda v: ((v.get("positive_segments") or 0), (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results, "frontier_improvements": [v for v in results if v.get("frontier_improvement")]}, open(args.out, "w"), indent=2, default=str)
    nfi = len([v for v in results if v.get("frontier_improvement")])
    print(f"\n[r4C] wrote {args.out}: {len(results)} results, {nfi} frontier_improvement in {time.time()-t0:.0f}s")
    print("\n=== TOP 10 by pos_segs then ann ===")
    for v in results[:10]:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        s25 = (v.get("segment_returns") or {}).get("2025","n/a")
        print(f"  {v['label']:36s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} pos={v['positive_segments']}/5 2025={s25}")

if __name__ == "__main__":
    main()
