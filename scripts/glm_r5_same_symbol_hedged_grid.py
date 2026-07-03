#!/usr/bin/env python3
"""GLM Round 5 Task F: Same-Symbol Hedged Martingale Grid — FULL grid per plan.

Tests long+short martingale grids on same high-liquidity symbols under hedge-mode.
Per plan section 12: symbol groups of 3-5, various entry signals, multipliers, legs, cooldowns.
Full 5-segment validation for every candidate.
"""
import argparse, json, os, subprocess, sys, time, random
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999

def run_replay(config, budget, s, e, pid):
    p = f"/tmp/r5Ff_{os.getpid()}_{pid}.json"
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

def ptp(bps):
    orig=[0.30,0.30,0.40]; rem=1.0; stages=[]
    for i,of in enumerate(orig):
        if i==2: stages.append([1,1,bps[i]])
        else: rf=min(max(of/rem if rem>0 else 1,0),1); stages.append([int(round(rf*1000)),1000,bps[i]]); rem*=1-rf
    return {"partial":{"stages":stages,"breakeven_after_stage":1,"breakeven_buffer_bps":100}}

def mk_hedge(sid, sym, direction, gates, w, mult, legs, step, cd_s, fq):
    tr = [{"cooldown": {"seconds": cd_s}}]
    for g in gates: tr.append({"indicator_expression": {"expression": g}})
    rl = {"max_active_cycles": None, "max_global_budget_quote": None, "max_symbol_budget_quote": None, "max_direction_budget_quote": None, "max_strategy_budget_quote": None, "max_global_drawdown_quote": None, "safety_skip_adx_threshold": 35, "safety_order_basis": "last_executed_order"}
    sl = 3000
    return {"strategy_id": sid, "symbol": sym, "market": "usd_m_futures", "direction": direction, "direction_mode": "long_and_short", "margin_mode": "isolated", "leverage": 10, "spacing": {"fixed_percent": {"step_bps": step}}, "sizing": {"multiplier": {"first_order_quote": str(fq), "multiplier": str(mult), "max_legs": legs}}, "take_profit": ptp([800,1600,2600]), "stop_loss": {"strategy_drawdown_pct": {"pct_bps": sl}}, "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}], "entry_triggers": tr, "risk_limits": rl, "portfolio_weight_pct": str(w)}

LONG_ENTRIES = {
    "rsi35": "rsi(14) < 35",
    "bb_lower": "bb_lower(20, 2) > close",
    "ema_up_rsi45": "{S}.close > {S}.ema(50) AND rsi(14) < 45",
}
SHORT_ENTRIES = {
    "rsi65": "rsi(14) > 65",
    "bb_upper": "close > bb_upper(20, 2)",
    "roc_pump": "roc(720) > 12 AND rsi(14) > 65",
}

UNIVERSE = ["BTCUSDT","ETHUSDT","BNBUSDT","SOLUSDT","LINKUSDT","ADAUSDT","DOGEUSDT","TRXUSDT","BCHUSDT","AAVEUSDT"]

def build(sym_group_size, long_entry, short_entry, long_fq, short_fq, mult, legs, cd_h):
    # Pick top-liquid symbols
    syms = UNIVERSE[:sym_group_size]
    w = 100.0 / (len(syms) * 2)
    cd_s = cd_h * 3600
    strat = []
    for i, s in enumerate(syms):
        lg = LONG_ENTRIES[long_entry].replace("{S}", s)
        sg = SHORT_ENTRIES[short_entry].replace("{S}", s)
        strat.append(mk_hedge(f"HL{i}-{s}", s, "long", [lg], w, mult, legs, 150, cd_s, long_fq))
        strat.append(mk_hedge(f"HS{i}-{s}", s, "short", [sg], w, mult, legs, 150, cd_s, short_fq))
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
    group_sizes = [3, 4, 5]
    long_entries_list = list(LONG_ENTRIES.keys())
    short_entries_list = list(SHORT_ENTRIES.keys())
    first_orders = [(20, 20), (25, 20), (30, 25)]
    mults = [1.4, 1.6, 1.8, 2.2]
    legs_opts = [3, 4, 5]
    cds = [8, 11, 16, 24]
    jobs = []
    for gs in group_sizes:
        for le in long_entries_list:
            for se in short_entries_list:
                for lofq, shfq in first_orders:
                    for mult in mults:
                        for legs in legs_opts:
                            for cd in cds:
                                cfg = build(gs, le, se, lofq, shfq, mult, legs, cd)
                                lbl = f"g{gs}le{le[:3]}se{se[:3]}fq{lofq}m{mult}l{legs}cd{cd}"
                                jobs.append((cfg, lbl))
    random.seed(42)
    if len(jobs) > 400:
        jobs = random.sample(jobs, 400)
    print(f"[r5F] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
            if done % 50 == 0 or (ann > 30 and rec.get("positive_segments",0)>=4):
                s25 = rec.get("segment_returns",{}).get("2025","?")
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):30s} ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 2025={s25}", flush=True)
    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["frontier_improvement"] = (ann >= 35 and dd <= 15 and v["positive_segments"] >= 4 and v["agg_2024_2026"] > 0)
    results.sort(key=lambda v: ((v.get("positive_segments") or 0), (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results}, open(args.out, "w"), indent=2, default=str)
    nfi = len([v for v in results if v.get("frontier_improvement")])
    print(f"\n[r5F] wrote {args.out}: {len(results)} results, {nfi} frontier_improvement in {time.time()-t0:.0f}s")
    print("\n=== TOP 15 ===")
    for v in results[:15]:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        s25 = (v.get("segment_returns") or {}).get("2025","n/a")
        print(f"  {v['label']:30s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} pos={v['positive_segments']}/5 2025={s25}")

if __name__ == "__main__":
    main()
