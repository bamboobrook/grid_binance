#!/usr/bin/env python3
"""GLM Round 5 Task G: Expanded Universe With Anti-Survivor Rules.
Per plan section 13: prefilter top liquid symbols, then test replacements for base6.
Full 5-segment validation. Split discovery 2023-2024, validation 2025-2026.
"""
import argparse, json, os, subprocess, sys, time, sqlite3
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999
LONG_STRICT = ["{S}.close > {S}.ema(50)", "{S}.ema(50) > {S}.ema(200)", "BTCUSDT.close > BTCUSDT.ema(50)"]

def run_replay(config, budget, s, e, pid):
    p = f"/tmp/r5Gf_{os.getpid()}_{pid}.json"
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

def mk(sid, sym, d, gates, w, mult, legs, step):
    tr = [{"cooldown": {"seconds": 39600}}]
    for g in gates: tr.append({"indicator_expression": {"expression": g}})
    rl = {"max_active_cycles": None, "max_global_budget_quote": None, "max_symbol_budget_quote": None, "max_direction_budget_quote": None, "max_strategy_budget_quote": None, "max_global_drawdown_quote": None, "safety_skip_adx_threshold": 35, "safety_order_basis": "last_executed_order", "vol_target_atr_pct": 1.0, "vol_target_min_scale": 0.5, "vol_target_max_scale": 1.5}
    if d == "long": rl["safety_order_condition"] = "rsi(14) < 45"
    fq = "35.0" if d == "long" else "25.0"
    sl = 5000 if d == "long" else 3000
    return {"strategy_id": sid, "symbol": sym, "market": "usd_m_futures", "direction": d, "direction_mode": "long_and_short", "margin_mode": "isolated", "leverage": 10, "spacing": {"fixed_percent": {"step_bps": step}}, "sizing": {"multiplier": {"first_order_quote": fq, "multiplier": str(mult), "max_legs": legs}}, "take_profit": ptp([800,1600,2600]), "stop_loss": {"strategy_drawdown_pct": {"pct_bps": sl}}, "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}], "entry_triggers": tr, "risk_limits": rl, "portfolio_weight_pct": str(w)}

def build(longs, shorts):
    strat = [mk(f"L{i}-{s}", s, "long", [x.replace("{S}", s) for x in LONG_STRICT], 13.3, 3.3, 8, 150) for i, s in enumerate(longs)]
    strat += [mk(f"S{i}-{s}", s, "short", ["roc(720) > 18", "rsi(14) > 65"], 8.0, 1.6, 6, 180) for i, s in enumerate(shorts)]
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
    # Prefilter: get top liquid symbols with full coverage
    conn = sqlite3.connect(MARKET_DB)
    rows = conn.execute("""
        SELECT symbol, COUNT(*) as n, MIN(open_time), MAX(open_time)
        FROM klines WHERE open_time BETWEEN 1672531200000 AND 1780271999999
        GROUP BY symbol HAVING MIN(open_time) <= 1672531200000+86400000 AND MAX(open_time) >= 1780271999999-86400000
        ORDER BY n DESC LIMIT 25
    """).fetchall()
    conn.close()
    top_syms = [r[0] for r in rows]
    print(f"Top {len(top_syms)} liquid full-period symbols: {top_syms[:12]}...", flush=True)

    base_longs = ["BNBUSDT", "TRXUSDT", "BCHUSDT"]
    base_shorts = ["AAVEUSDT", "SOLUSDT", "DOTUSDT"]
    # Test replacing each long with a top symbol
    jobs = []
    # baseline
    jobs.append((build(base_longs, base_shorts), "base6"))
    # Replace each long with each candidate
    for cand in top_syms:
        if cand in base_longs or cand in base_shorts: continue
        for idx in range(3):
            new_longs = list(base_longs); new_longs[idx] = cand
            jobs.append((build(new_longs, base_shorts), f"replaceL{idx}_{cand}"))
    # Replace each short with a candidate
    for cand in top_syms:
        if cand in base_longs or cand in base_shorts: continue
        for idx in range(3):
            new_shorts = list(base_shorts); new_shorts[idx] = cand
            jobs.append((build(base_longs, new_shorts), f"replaceS{idx}_{cand}"))
    # Add two new symbols at 5% each
    for cand1 in top_syms[:8]:
        for cand2 in top_syms[:8]:
            if cand1 >= cand2 or cand1 in base_longs+base_shorts or cand2 in base_longs+base_shorts: continue
            # 2 new at 5% each, reduce base proportionally
            cfg = build(base_longs + [cand1], base_shorts + [cand2])
            # Adjust weights: base gets ~90%, new gets ~10%
            for s in cfg["strategies"][:3]: s["portfolio_weight_pct"] = "12.0"
            for s in cfg["strategies"][3:6]: s["portfolio_weight_pct"] = "7.2"
            for s in cfg["strategies"][6:]: s["portfolio_weight_pct"] = "5.0"
            jobs.append((cfg, f"add2_{cand1}_{cand2}"))

    print(f"[r5G] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
            ann = fm.get("ann") or 0
            if done % 30 == 0 or ann > 45:
                s25 = rec.get("segment_returns",{}).get("2025","?")
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):30s} ann={ann} dd={fm.get('dd')} pos={rec.get('positive_segments')}/5 2025={s25}", flush=True)
    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["frontier_improvement"] = (ann > 49.93 and v["positive_segments"] >= 4 and v["agg_2024_2026"] > 0)
    results.sort(key=lambda v: ((v.get("positive_segments") or 0), (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results}, open(args.out, "w"), indent=2, default=str)
    nfi = len([v for v in results if v.get("frontier_improvement")])
    print(f"\n[r5G] wrote {args.out}: {len(results)} results, {nfi} frontier_improvement in {time.time()-t0:.0f}s")
    print("\n=== TOP 15 ===")
    for v in results[:15]:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        s25 = (v.get("segment_returns") or {}).get("2025","n/a")
        print(f"  {v['label']:30s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} pos={v['positive_segments']}/5 2025={s25}")

if __name__ == "__main__":
    main()
