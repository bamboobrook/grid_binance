#!/usr/bin/env python3
"""GLM Round 6: R4 base + higher mult + last-exec SO + DD state machine + quarantine×fine blend.

Two searches:
1. R4 combo with mult 2.5-3.0 + last-exec SO + DD state machine (scale at high DD)
2. quarantine XRP × fine-combo blend
"""
import argparse, json, os, subprocess, sys, time, copy
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999
LONG_STRICT = ["{S}.close > {S}.ema(50)", "{S}.ema(50) > {S}.ema(200)", "BTCUSDT.close > BTCUSDT.ema(50)"]

def run_replay(config, budget, s, e, pid):
    p = f"/tmp/r6R4_{os.getpid()}_{pid}.json"
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

def load_config(path):
    with open(path) as f: return json.load(f)["portfolio_config"]

def scale_weights(config, scale):
    cfg = copy.deepcopy(config)
    for s in cfg["strategies"]:
        w = float(s["portfolio_weight_pct"])
        s["portfolio_weight_pct"] = str(round(w * scale, 4))
    return cfg

def ptp(bps, be_after=1, be_buf=100):
    orig=[0.30,0.30,0.40]; rem=1.0; stages=[]
    for i,of in enumerate(orig):
        if i==2: stages.append([1,1,bps[i]])
        else: rf=min(max(of/rem if rem>0 else 1,0),1); stages.append([int(round(rf*1000)),1000,bps[i]]); rem*=1-rf
    return {"partial":{"stages":stages,"breakeven_after_stage":be_after,"breakeven_buffer_bps":be_buf}}

def mk_r4_boost(sid, sym, d, gates, w, mult, legs, step, tp_bps, vol_target, dd_rules, cd_s):
    tr = [{"cooldown": {"seconds": cd_s}}]
    for g in gates: tr.append({"indicator_expression": {"expression": g}})
    rl = {"max_active_cycles": None, "max_global_budget_quote": None, "max_symbol_budget_quote": None, "max_direction_budget_quote": None, "max_strategy_budget_quote": None, "max_global_drawdown_quote": None, "safety_skip_adx_threshold": 35, "safety_order_basis": "last_executed_order"}
    if d == "long": rl["safety_order_condition"] = "rsi(14) < 45"
    if vol_target:
        rl["vol_target_atr_pct"] = vol_target
        rl["vol_target_min_scale"] = 0.5
        rl["vol_target_max_scale"] = 1.5
    if dd_rules: rl["drawdown_state_rules"] = dd_rules
    fq = "35.0" if d == "long" else "25.0"
    sl = 5000 if d == "long" else 3000
    return {"strategy_id": sid, "symbol": sym, "market": "usd_m_futures", "direction": d, "direction_mode": "long_and_short", "margin_mode": "isolated", "leverage": 10, "spacing": {"fixed_percent": {"step_bps": step}}, "sizing": {"multiplier": {"first_order_quote": fq, "multiplier": str(mult), "max_legs": legs}}, "take_profit": ptp(tp_bps), "stop_loss": {"strategy_drawdown_pct": {"pct_bps": sl}}, "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}], "entry_triggers": tr, "risk_limits": rl, "portfolio_weight_pct": str(w)}

def build_r4_boost(long_mult, short_mult, long_legs, tp_bps, vol_target, dd_rules, cd_h):
    longs = ["BNBUSDT", "TRXUSDT", "BCHUSDT"]; shorts = ["AAVEUSDT", "SOLUSDT", "DOTUSDT"]
    cd_s = cd_h * 3600
    strat = []
    for i, s in enumerate(longs):
        gates = [x.replace("{S}", s) for x in LONG_STRICT]
        strat.append(mk_r4_boost(f"L{i}-{s}", s, "long", gates, 13.3, long_mult, long_legs, 150, tp_bps, vol_target, dd_rules, cd_s))
    for i, s in enumerate(shorts):
        gates = ["roc(720) > 18", "rsi(14) > 65"]
        strat.append(mk_r4_boost(f"S{i}-{s}", s, "short", gates, 8.0, short_mult, 5, 180, tp_bps, vol_target, dd_rules, cd_s))
    rl = {"max_global_budget_quote": "5000"}
    if dd_rules: rl["drawdown_state_rules"] = dd_rules; rl["drawdown_state_recovery_pct"] = 0.5
    rl["new_cycle_drawdown_pause_pct"] = 50.0
    return {"direction_mode": "long_and_short", "strategies": strat, "risk_limits": rl}

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

    # Search 1: R4 boost — higher mult + last-exec SO + DD state machine
    long_mults = [2.5, 2.8, 3.0]
    short_mults = [1.6, 1.8, 2.0]
    tp_ladders = [(600,1200,2200), (800,1600,2600)]
    vol_targets = [None, 1.0]
    dd_rules_opts = [
        None,
        [{"trigger_drawdown_pct": 10, "first_order_scale": 0.6}, {"trigger_drawdown_pct": 15, "first_order_scale": 0.4, "freeze_safety_orders": True}],
        [{"trigger_drawdown_pct": 8, "first_order_scale": 0.7}, {"trigger_drawdown_pct": 12, "first_order_scale": 0.5, "freeze_safety_orders": True}],
    ]
    cds = [10, 11, 12]

    jobs1 = []
    for lm in long_mults:
        for sm in short_mults:
            for tp in tp_ladders:
                for vt in vol_targets:
                    for dd in dd_rules_opts:
                        for cd in cds:
                            cfg = build_r4_boost(lm, sm, 8, list(tp), vt, dd, cd)
                            lbl = f"r4_lm{lm}sm{sm}tp{tp[0]}vt{vt}dd{len(dd) if dd else 0}cd{cd}"
                            jobs1.append((cfg, lbl))

    # Search 2: quarantine XRP × fine-combo blend
    xrp_q = load_config("docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-C-best-quarantine.json")
    fine = load_config("docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-fine-combo-best.json")

    jobs2 = []
    jobs2.append((xrp_q, "xrpq100"))
    jobs2.append((fine, "fine100"))
    for xq_pct in [0.2, 0.3, 0.4, 0.5]:
        for fine_pct in [0.3, 0.4, 0.5, 0.6]:
            total = xq_pct + fine_pct
            if total > 1.0 or total < 0.3: continue
            all_strats = []
            for cfg, sc in [(xrp_q, xq_pct), (fine, fine_pct)]:
                scaled = scale_weights(cfg, sc)
                for s in scaled["strategies"]:
                    s["strategy_id"] = f"b_{s['strategy_id']}_{len(all_strats)}"
                    all_strats.append(s)
            cfg = {"direction_mode": "long_and_short", "strategies": all_strats, "risk_limits": {"max_global_budget_quote": "5000"}}
            jobs2.append((cfg, f"xq{int(xq_pct*100)}_fi{int(fine_pct*100)}"))

    all_jobs = jobs1 + jobs2
    # Sample if too many
    import random; random.seed(42)
    if len(all_jobs) > 300:
        all_jobs = random.sample(all_jobs, 300)

    print(f"[r6R4] {len(all_jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
    t0 = time.time()
    results = []
    with ProcessPoolExecutor(max_workers=args.workers) as ex:
        futs = {ex.submit(full_eval, j): j for j in all_jobs}
        done = 0
        for fut in as_completed(futs):
            try: rec = fut.result()
            except Exception as e: rec = {"label": "?", "error": str(e), "positive_segments": 0}
            results.append(rec)
            done += 1
            fm = rec.get("full_metrics") or {}
            ann = fm.get("ann") or 0; dd = fm.get("dd") or 999
            if done % 30 == 0 or (ann > 45 and dd <= 20):
                s25 = rec.get("segment_returns",{}).get("2025","?")
                print(f"  [{done}/{len(all_jobs)}] {rec.get('label','?'):36s} ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 2025={s25}", flush=True)

    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["target_hit"] = (ann > 50 and dd <= 20 and v["positive_segments"] >= 4)
        v["nf_cons_dd"] = (ann >= 50 and dd <= 18 and v["positive_segments"] >= 4)
        v["frontier"] = (ann > 34.7 and dd < 20 and v["positive_segments"] >= 4 and v["agg_2024_2026"] > 0)
    results.sort(key=lambda v: ((v.get("positive_segments") or 0), (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results}, open(args.out, "w"), indent=2, default=str)
    nth = len([v for v in results if v.get("target_hit")])
    nc = len([v for v in results if v.get("nf_cons_dd")])
    nf = len([v for v in results if v.get("frontier")])
    print(f"\n[r6R4] wrote: {len(results)} results. TARGET-HIT={nth} NF-cons-dd={nc} frontier={nf} in {time.time()-t0:.0f}s")
    print("\n=== TOP 20 ===")
    for v in results[:20]:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        s25 = (v.get("segment_returns") or {}).get("2025","n/a")
        print(f"  {v['label']:36s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} pos={v['positive_segments']}/5 2025={s25}")
    # Save best
    best = max([v for v in results if "error" not in v and v.get("positive_segments",0)>=4], key=lambda v: (v.get("full_metrics") or {}).get("ann",0))
    fm = best.get("full_metrics") or {}
    print(f"\nBEST: {best['label']} ann={fm.get('ann'):.1f} dd={fm.get('dd'):.1f}")
    json.dump({"portfolio_config": best["config"]}, open("docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-R4-boost-best.json", "w"), indent=2)

if __name__ == "__main__":
    main()
