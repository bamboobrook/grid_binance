#!/usr/bin/env python3
"""B2 G1 staged search (plan §11). 192 unique scrambled Sobol effective configs:
T2=96 seed 20260721, T3-8=64 seed 20260722, T3-12=32 seed 20260723.
4 fixed earliest-train 30-day windows × 1000/3000/4999U = 2304 replays.
G1 only eliminates. Conservative ladder center; R17 router/hazard/funding/cluster
params are the main Sobol dimensions (A4-bound)."""
import hashlib, json, os, subprocess, time
from scipy.stats import qmc

ART = "docs/superpowers/artifacts/glm-martingale-core-round17"
WINDOWS = [
    ("w1", 1672531200000, 1672531200000 + 30*86_400_000),
    ("w2", 1676419200000, 1676419200000 + 30*86_400_000),
    ("w3", 1680307200000, 1680307200000 + 30*86_400_000),
    ("w4", 1684934400000, 1684934400000 + 30*86_400_000),
]
BUDGETS = [1000.0, 3000.0, 4999.0]
TRACK_QUOTA = {"T2": (96, 20260721, ["BTCUSDT","ETHUSDT","BNBUSDT","SOLUSDT","XRPUSDT","DOGEUSDT","ADAUSDT","TRXUSDT","LINKUSDT","LTCUSDT","BCHUSDT","DOTUSDT"]),
               "T3_8": (64, 20260722, ["BTCUSDT","ETHUSDT","BNBUSDT","SOLUSDT","XRPUSDT","DOGEUSDT","ADAUSDT","TRXUSDT"]),
               "T3_12": (32, 20260723, ["BTCUSDT","ETHUSDT","BNBUSDT","SOLUSDT","XRPUSDT","DOGEUSDT","ADAUSDT","TRXUSDT","LINKUSDT","LTCUSDT","BCHUSDT","DOTUSDT"])}

# dims: long_fo, long_mult, long_legs, spacing, tp, leverage, router_trend_horizon, hazard_deadline_hl
PARAM_BOUNDS = [(10, 20), (1.25, 1.55), (4, 5), (120, 250), (120, 250), (3, 5), (12, 48), (2, 5)]


def build(symbols, fo, mult, legs, sp, tp, lev, trend_h, dl_hl):
    n = len(symbols); wt = round(100.0/(2*n), 4)
    strats = []
    for sym in symbols:
        rl = {"htf_regime_gate_enabled": True,
              "r17_router": {"trend_horizon_4h": int(trend_h), "breadth_threshold": 0.70, "enter_persistence": 3, "exit_persistence": 2, "minimum_dwell_hours": 24, "range_displacement_z": 1.25, "shock_downside_q": 0.90, "cusum_sigma": 4.0, "shock_cooldown_hours": 24},
              "r17_hazard": {"half_life_window_h": 168, "deadline_half_lives": int(dl_hl), "deadline_cap_h": 72, "after_deadline": "freeze_so", "reserve_next_legs": 2}}
        strats.append(_sleeve(sym, "long", fo, mult, legs, sp, tp, lev, wt, rl))
        strats.append(_sleeve(sym, "short", fo, round(mult*0.85,2), max(3,legs-1), sp+50, tp, lev, wt, rl))
    return {"direction_mode": "long_and_short", "risk_limits": {"max_global_budget_quote": "4999"}, "strategies": strats}


def _sleeve(sym, d, fo, m, legs, sp, tp, lev, wt, rl):
    return {"strategy_id": f"{'L' if d=='long' else 'S'}-{sym}", "symbol": sym, "market": "usd_m_futures",
            "direction": d, "direction_mode": "long_and_short", "margin_mode": "isolated", "leverage": int(lev),
            "spacing": {"fixed_percent": {"step_bps": int(sp)}},
            "sizing": {"multiplier": {"first_order_quote": str(int(fo)), "multiplier": str(m), "max_legs": int(legs)}},
            "take_profit": {"percent": {"bps": int(tp)}}, "stop_loss": None, "indicators": [],
            "entry_triggers": [{"cooldown": {"seconds": 39600}}], "portfolio_weight_pct": str(wt), "risk_limits": rl}


def eff_hash(cfg): return hashlib.sha256(json.dumps(cfg, sort_keys=True).encode()).hexdigest()


def run_cli(cfg, budget, start, end, label):
    cfgdir = os.path.join(ART, "configs", "g1"); os.makedirs(cfgdir, exist_ok=True)
    ch = eff_hash(cfg)[:12]; path = os.path.join(cfgdir, f"g1_{ch}.json")
    with open(path, "w") as f: json.dump({"portfolio_config": cfg}, f, sort_keys=True)
    cmd = ["target/release/portfolio_budget_replay", "--config", path, "--budget", str(int(budget)),
           "--start-ms", str(start), "--end-ms", str(end),
           "--market-data", "data/market_data_full.db", "--funding-data", "data/funding_rates_round12.db",
           "--exchange-min-notional", "5.0"]
    t0 = time.time()
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=180)
        wall = time.time() - t0
        if r.returncode != 0:
            return {"status": "error", "wall_s": round(wall,1), "config_hash": ch}
        d = json.loads(r.stdout[r.stdout.find("{"):r.stdout.rfind("}")+1])
        ob = d.get("on_budget", {})
        return {"status": "complete", "wall_s": round(wall,1),
                "ann": round(ob.get("annualized_return_pct") or -999, 4), "dd": round(ob.get("max_drawdown_pct") or -999, 4),
                "breached": ob.get("principal_breached"), "trade_count": d.get("trade_count"),
                "raw_command": "portfolio_budget_replay", "exit_code": r.returncode, "config_hash": ch}
    except subprocess.TimeoutExpired:
        return {"status": "timeout", "config_hash": ch}
    except Exception as e:
        return {"status": "error", "err": str(e)[:100], "config_hash": ch}


def append(rec):
    with open(os.path.join(ART, "exploration-registry.jsonl"), "a") as f:
        f.write(json.dumps(rec, sort_keys=True) + "\n")


def main():
    # Pre-populate seen keys from the registry so a resumed run skips
    # already-completed configs (resume-safety).
    seen = set()
    reg_path = os.path.join(ART, "exploration-registry.jsonl")
    if os.path.exists(reg_path):
        for line in open(reg_path):
            line = line.strip()
            if not line:
                continue
            try:
                r = json.loads(line)
            except json.JSONDecodeError:
                continue
            if r.get("status") in ("complete", "skipped_duplicate"):
                eff = r.get("effective_config_hash")
                w = r.get("window") or r.get("start_ms", "")
                b = r.get("budget", "")
                seen.add(f"{eff}|{w}|{b}")
    actual = dups = timeouts = 0; uniq_cfgs = set()
    t_start = time.time()
    for track, (quota, seed, symbols) in TRACK_QUOTA.items():
        sampler = qmc.Sobol(d=len(PARAM_BOUNDS), scramble=True, seed=seed)
        nd = 1
        while nd < quota: nd *= 2
        sample = sampler.random(nd)[:quota]
        lower = [b[0] for b in PARAM_BOUNDS]; upper = [b[1] for b in PARAM_BOUNDS]
        scaled = qmc.scale(sample, lower, upper)
        for i, params in enumerate(scaled):
            fo, mult, legs, sp, tp, lev, th, dl = params
            fo = int(round(fo/5)*5); mult = round(mult,2); legs = int(round(legs))
            sp = int(round(sp/10)*10); tp = int(round(tp/10)*10); lev = int(round(lev)); th = int(round(th)); dl = int(round(dl))
            cfg = build(symbols, fo, mult, legs, sp, tp, lev, th, dl)
            ch = eff_hash(cfg); uniq_cfgs.add(ch)
            for wname, ws, we in WINDOWS:
                for budget in BUDGETS:
                    key = f"{ch}|{wname}|{budget}"
                    label = f"r17_g1_{track}_{i:03d}_{wname}_{int(budget)}"
                    if key in seen:
                        dups += 1
                        append({"experiment_id": label, "family": f"G1_{track}", "track": track,
                                "window": wname, "budget": budget, "effective_config_hash": ch,
                                "status": "skipped_duplicate", "actual_binary_replays": 0, "cache_hits": 1,
                                "raw_command": "cache", "exit_code": 0})
                        continue
                    seen.add(key)
                    append({"experiment_id": label, "family": f"G1_{track}", "track": track,
                            "window": wname, "budget": budget, "effective_config_hash": ch,
                            "resolved_config_hash": ch, "status": "running", "started_at": time.time(),
                            "raw_command": "portfolio_budget_replay"})
                    r = run_cli(cfg, budget, ws, we, label)
                    rec = {"experiment_id": label, "family": f"G1_{track}", "track": track,
                           "window": wname, "budget": budget, "effective_config_hash": ch,
                           "resolved_config_hash": ch, "finished_at": time.time(),
                           "raw_command": "portfolio_budget_replay", "exit_code": r.get("exit_code", -1)}
                    rec.update(r); rec["status"] = r.get("status","error")
                    rec["actual_binary_replays"] = 1 if r.get("status")=="complete" else 0
                    rec["cache_hits"] = 0
                    if r.get("status")=="complete": actual += 1
                    if r.get("status")=="timeout": timeouts += 1
                    append(rec)
            el = time.time() - t_start
            print(f"[{track} {i+1}/{quota}] fo{fo} m{mult} l{legs} s{sp} t{tp} lv{lev} th{th} dl{dl}: actual={actual} el={el:.0f}s", flush=True)
    out = {"schema_version": 1, "phase": "B2_G1", "sampler": "scipy_sobol",
           "unique_configs": len(uniq_cfgs), "actual_replays": actual, "duplicates": dups, "timeouts": timeouts,
           "windows": 4, "budgets": 3, "wall_s": round(time.time()-t_start,1)}
    os.makedirs(os.path.join(ART, "b2"), exist_ok=True)
    with open(os.path.join(ART, "b2", "g1-sobol.json"), "w") as f:
        json.dump(out, f, indent=2, sort_keys=True)
    d = os.path.join(ART, "b2", "gates"); os.makedirs(d, exist_ok=True)
    json.dump({"unique_configs": len(uniq_cfgs), "actual_replays": actual, "windows": 4, "budgets": 3,
               "duplicates": dups}, open(os.path.join(d, "g1_192_sobol_2304_replays.json"), "w"), indent=2)
    print(f"\nG1 done: unique={len(uniq_cfgs)} actual={actual} dups={dups} timeouts={timeouts}")


if __name__ == "__main__":
    main()
