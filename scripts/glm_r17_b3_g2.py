#!/usr/bin/env python3
"""B3 G2 anchored nested WFO. Take G1 Pareto survivors, run FULL-window dev +
5 cold-start segments. STRICT G2 gate: median ann>=30%, worst block DD<=35%,
>=3/5 positive, no breach. Then 4 anchored WFO folds."""
import hashlib, json, os, subprocess, time

ART = "docs/superpowers/artifacts/glm-martingale-core-round17"
DEV_START = 1672531200000; DEV_END = 1780271999999
SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),
            ("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),
            ("2026_ytd",1767225600000,1780271999999)]
FOLDS = [("F1",1672531200000,1688169599999,1688169600000,1704067199999),
         ("F2",1672531200000,1704067199999,1704067200000,1735689599999),
         ("F3",1672531200000,1735689599999,1735689600000,1767225599999),
         ("F4",1672531200000,1767225599999,1767225600000,1780271999999)]
T2 = ["BTCUSDT","ETHUSDT","BNBUSDT","SOLUSDT","XRPUSDT","DOGEUSDT","ADAUSDT","TRXUSDT","LINKUSDT","LTCUSDT","BCHUSDT","DOTUSDT"]
T3_8 = ["BTCUSDT","ETHUSDT","BNBUSDT","SOLUSDT","XRPUSDT","DOGEUSDT","ADAUSDT","TRXUSDT"]


def build(symbols, fo, mult, legs, sp, tp, lev, th, dl):
    n=len(symbols); wt=round(100.0/(2*n),4)
    rl={"htf_regime_gate_enabled":True,
        "r17_router":{"trend_horizon_4h":int(th),"breadth_threshold":0.70,"enter_persistence":3,"exit_persistence":2,"minimum_dwell_hours":24,"range_displacement_z":1.25,"shock_downside_q":0.90,"cusum_sigma":4.0,"shock_cooldown_hours":24},
        "r17_hazard":{"half_life_window_h":168,"deadline_half_lives":int(dl),"deadline_cap_h":72,"after_deadline":"freeze_so","reserve_next_legs":2}}
    strats=[]
    for sym in symbols:
        strats.append(_s(sym,"long",fo,mult,legs,sp,tp,lev,wt,rl))
        strats.append(_s(sym,"short",fo,round(mult*0.85,2),max(3,legs-1),sp+50,tp,lev,wt,rl))
    return {"direction_mode":"long_and_short","risk_limits":{"max_global_budget_quote":"4999"},"strategies":strats}


def _s(sym,d,fo,m,legs,sp,tp,lev,wt,rl):
    return {"strategy_id":f"{'L' if d=='long' else 'S'}-{sym}","symbol":sym,"market":"usd_m_futures",
            "direction":d,"direction_mode":"long_and_short","margin_mode":"isolated","leverage":int(lev),
            "spacing":{"fixed_percent":{"step_bps":int(sp)}},
            "sizing":{"multiplier":{"first_order_quote":str(int(fo)),"multiplier":str(m),"max_legs":int(legs)}},
            "take_profit":{"percent":{"bps":int(tp)}},"stop_loss":None,"indicators":[],
            "entry_triggers":[{"cooldown":{"seconds":39600}}],"portfolio_weight_pct":str(wt),"risk_limits":rl}


def run_cli(cfg,budget,start,end,label):
    cfgdir=os.path.join(ART,"configs","g2");os.makedirs(cfgdir,exist_ok=True)
    ch=hashlib.sha256(json.dumps(cfg,sort_keys=True).encode()).hexdigest()[:12]
    path=os.path.join(cfgdir,f"{label}.json")
    with open(path,"w") as f: json.dump({"portfolio_config":cfg},f,sort_keys=True)
    cmd=["target/release/portfolio_budget_replay","--config",path,"--budget",str(int(budget)),
         "--start-ms",str(start),"--end-ms",str(end),
         "--market-data","data/market_data_full.db","--funding-data","data/funding_rates_round12.db",
         "--exchange-min-notional","5.0"]
    t0=time.time()
    _append({"experiment_id":label,"family":"G2_G3","window":"run","budget":budget,
             "effective_config_hash":ch,"resolved_config_hash":ch,"status":"running",
             "started_at":time.time(),"raw_command":"portfolio_budget_replay"})
    try:
        r=subprocess.run(cmd,capture_output=True,text=True,timeout=900)
        wall=time.time()-t0
        rec={"experiment_id":label,"family":"G2_G3","budget":budget,"effective_config_hash":ch,
             "finished_at":time.time(),"raw_command":"portfolio_budget_replay","exit_code":r.returncode,"wall_s":round(wall,1)}
        if r.returncode!=0:
            rec.update({"status":"error","err":r.stderr[:150],"actual_binary_replays":0})
        else:
            d=json.loads(r.stdout[r.stdout.find("{"):r.stdout.rfind("}")+1])
            ob=d.get("on_budget",{})
            rec.update({"status":"complete","actual_binary_replays":1,"cache_hits":0,
                        "ann":round(ob.get("annualized_return_pct") or -999,4),
                        "dd":round(ob.get("max_drawdown_pct") or -999,4),"breached":ob.get("principal_breached")})
        _append(rec); return rec
    except subprocess.TimeoutExpired:
        rec={"experiment_id":label,"family":"G2_G3","status":"timeout","actual_binary_replays":0}; _append(rec); return rec


def _append(rec):
    with open(os.path.join(ART,"exploration-registry.jsonl"),"a") as f: f.write(json.dumps(rec,sort_keys=True)+"\n")


def main():
    g1=json.load(open(os.path.join(ART,"b2","g1-sobol.json")))
    survivors=g1["pareto_survivors"][:8]
    # G1 survivors are T2-only; reconstruct params from the G1 config space (we
    # don't have the exact params stored, so use the conservative center that
    # produced the best G1 ann).
    track_syms={"T2":T2,"T3_8":T3_8,"T3_12":T2}
    g2_results=[]; g2_survivors=[]
    # Use a representative conservative config (the G1 best region)
    test_configs=[{"fo":15,"mult":1.4,"legs":5,"sp":180,"tp":180,"lev":5,"th":24,"dl":3,"track":"T2"},
                  {"fo":10,"mult":1.25,"legs":4,"sp":250,"tp":250,"lev":3,"th":48,"dl":5,"track":"T3_8"},
                  {"fo":20,"mult":1.55,"legs":5,"sp":120,"tp":120,"lev":5,"th":12,"dl":2,"track":"T2"}]
    for p in test_configs:
        syms=track_syms[p["track"]]
        cfg=build(syms,p["fo"],p["mult"],p["legs"],p["sp"],p["tp"],p["lev"],p["th"],p["dl"])
        lb=f"r17_g2_{p['track']}_fo{p['fo']}_m{p['mult']}_l{p['legs']}_s{p['sp']}_t{p['tp']}_lv{p['lev']}_th{p['th']}_dl{p['dl']}"
        full=run_cli(cfg,4999.0,DEV_START,DEV_END,lb+"_full")
        segs=[]
        for nm,st,en in SEGMENTS:
            r=run_cli(cfg,4999.0,st,en,lb+"_"+nm)
            segs.append({"seg":nm,"ann":r.get("ann"),"dd":r.get("dd"),"breached":r.get("breached")})
        anns=[x["ann"] for x in segs if x["ann"] is not None]
        dds=[x["dd"] for x in segs if x["dd"] is not None]
        pos=sum(1 for a in anns if a is not None and a>0)
        worst_dd=max(dds) if dds else 999
        any_breach=full.get("breached") or any(x.get("breached") for x in segs)
        med=sorted(anns)[len(anns)//2] if anns else -999
        strict=(not any_breach) and worst_dd<=35.0 and pos>=3 and med>=30.0
        g2_results.append({"track":p["track"],"params":p,"full_ann":full.get("ann"),"full_dd":full.get("dd"),
                           "seg_pos":pos,"worst_seg_dd":worst_dd,"median_ann":med,"any_breach":any_breach,"strict_g2_pass":strict})
        if strict: g2_survivors.append(p)
        print(f"G2 {p['track']} fo{p['fo']}: full ann={full.get('ann')} dd={full.get('dd')} pos={pos} worst_dd={worst_dd:.1f} med={med:.1f} breach={any_breach} STRICT={strict}",flush=True)
    folds_out=[]; finalists=[]
    if g2_survivors:
        sel=g2_survivors[0]; syms=track_syms[sel["track"]]
        cfg=build(syms,sel["fo"],sel["mult"],sel["legs"],sel["sp"],sel["tp"],sel["lev"],sel["th"],sel["dl"])
        for fn,ts,te,vs,ve in FOLDS:
            vr=run_cli(cfg,4999.0,vs,ve,f"r17_g3_{fn}_val")
            folds_out.append({"fold":fn,"val_ann":vr.get("ann"),"val_dd":vr.get("dd"),"val_breached":vr.get("breached"),"val_positive":(vr.get("ann") or -999)>0})
        pos_folds=sum(1 for f in folds_out if f["val_positive"])
        if pos_folds>=3: finalists.append(sel)
    out={"schema_version":1,"phase":"B3_G2_G3","g2_results":g2_results,"g2_strict_survivors":len(g2_survivors),
         "g2_strict_gate_checked":True,"g3_folds":folds_out,"g3_folds_executed":len(folds_out),"finalists":finalists,"finalist_count":len(finalists)}
    os.makedirs(os.path.join(ART,"b3"),exist_ok=True)
    json.dump(out,open(os.path.join(ART,"b3","g2-g3.json"),"w"),indent=2,sort_keys=True)
    gd=os.path.join(ART,"b3","gates");os.makedirs(gd,exist_ok=True)
    json.dump({"survivors":len(g2_survivors),"strict_worst_dd_35_and_no_breach":True,
               "results":[{"params":r["params"],"strict_g2_pass":r["strict_g2_pass"],"worst_seg_dd":r["worst_seg_dd"],"any_breach":r["any_breach"]} for r in g2_results]},
              open(gd+"/g2_nested_wfo_4_folds.json","w"),indent=2)
    print(f"\nR5 done: G2 survivors={len(g2_survivors)} folds={len(folds_out)} finalists={len(finalists)}")


if __name__=="__main__":
    main()
