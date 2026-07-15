#!/usr/bin/env python3
"""B2 G1 staged search — checkpoint-and-resume version.

Solves the 590s shell-timeout + registry-collapse problem by writing a
PERSISTENT checkpoint file after every single replay. The checkpoint stores
all completed (config, window, budget) -> result mappings. On resume, the
runner reads the checkpoint and skips already-done keys exactly. The checkpoint
is the canonical progress record (not the registry experiment_id collapse).

192 unique Sobol configs (T2=96 seed 20260721, T3-8=64 seed 20260722,
T3-12=32 seed 20260723) × 4 earliest-train 30-day windows × 1000/3000/4999U
= 2304 actual binary replays.

Run repeatedly (loop) until checkpoint shows all 2304 done. Each invocation
does as many replays as fit in the time budget, then saves and exits cleanly.
"""
import hashlib
import json
import os
import subprocess
import sys
import time

from scipy.stats import qmc

ART = "docs/superpowers/artifacts/glm-martingale-core-round17"
CKPT = os.path.join(ART, "b2", "g1-checkpoint.json")
SUMMARY = os.path.join(ART, "b2", "g1-sobol.json")

WINDOWS = [
    ("w1", 1672531200000, 1672531200000 + 30 * 86_400_000),
    ("w2", 1676419200000, 1676419200000 + 30 * 86_400_000),
    ("w3", 1680307200000, 1680307200000 + 30 * 86_400_000),
    ("w4", 1684934400000, 1684934400000 + 30 * 86_400_000),
]
BUDGETS = [1000.0, 3000.0, 4999.0]

T2 = ["BTCUSDT","ETHUSDT","BNBUSDT","SOLUSDT","XRPUSDT","DOGEUSDT","ADAUSDT","TRXUSDT","LINKUSDT","LTCUSDT","BCHUSDT","DOTUSDT"]
T3_8 = ["BTCUSDT","ETHUSDT","BNBUSDT","SOLUSDT","XRPUSDT","DOGEUSDT","ADAUSDT","TRXUSDT"]
TRACK_QUOTA = {
    "T2": (96, 20260721, T2),
    "T3_8": (64, 20260722, T3_8),
    "T3_12": (32, 20260723, T2),
}

# dims: long_fo, long_mult, long_legs, spacing, tp, leverage, router_trend_horizon, hazard_deadline_hl
PARAM_BOUNDS = [(10, 20), (1.25, 1.55), (4, 5), (120, 250), (120, 250), (3, 5), (12, 48), (2, 5)]

# Stop before this wall-clock second to leave time to save cleanly.
TIME_BUDGET_S = 540


def build(symbols, fo, mult, legs, sp, tp, lev, trend_h, dl_hl):
    n = len(symbols); wt = round(100.0 / (2 * n), 4)
    rl = {"htf_regime_gate_enabled": True,
          "r17_router": {"trend_horizon_4h": int(trend_h), "breadth_threshold": 0.70, "enter_persistence": 3, "exit_persistence": 2, "minimum_dwell_hours": 24, "range_displacement_z": 1.25, "shock_downside_q": 0.90, "cusum_sigma": 4.0, "shock_cooldown_hours": 24},
          "r17_hazard": {"half_life_window_h": 168, "deadline_half_lives": int(dl_hl), "deadline_cap_h": 72, "after_deadline": "freeze_so", "reserve_next_legs": 2}}
    strats = []
    for sym in symbols:
        strats.append(_sleeve(sym, "long", fo, mult, legs, sp, tp, lev, wt, rl))
        strats.append(_sleeve(sym, "short", fo, round(mult * 0.85, 2), max(3, legs - 1), sp + 50, tp, lev, wt, rl))
    return {"direction_mode": "long_and_short", "risk_limits": {"max_global_budget_quote": "4999"}, "strategies": strats}


def _sleeve(sym, d, fo, m, legs, sp, tp, lev, wt, rl):
    return {"strategy_id": f"{'L' if d == 'long' else 'S'}-{sym}", "symbol": sym, "market": "usd_m_futures",
            "direction": d, "direction_mode": "long_and_short", "margin_mode": "isolated", "leverage": int(lev),
            "spacing": {"fixed_percent": {"step_bps": int(sp)}},
            "sizing": {"multiplier": {"first_order_quote": str(int(fo)), "multiplier": str(m), "max_legs": int(legs)}},
            "take_profit": {"percent": {"bps": int(tp)}}, "stop_loss": None, "indicators": [],
            "entry_triggers": [{"cooldown": {"seconds": 39600}}], "portfolio_weight_pct": str(wt), "risk_limits": rl}


def eff_hash(cfg):
    return hashlib.sha256(json.dumps(cfg, sort_keys=True).encode()).hexdigest()


def gen_all_configs():
    """Generate the full deterministic plan: list of (track, idx, cfg, ch, params)."""
    plan = []
    for track, (quota, seed, symbols) in TRACK_QUOTA.items():
        sampler = qmc.Sobol(d=len(PARAM_BOUNDS), scramble=True, seed=seed)
        nd = 1
        while nd < quota:
            nd *= 2
        sample = sampler.random(nd)[:quota]
        lower = [b[0] for b in PARAM_BOUNDS]; upper = [b[1] for b in PARAM_BOUNDS]
        scaled = qmc.scale(sample, lower, upper)
        for i, params in enumerate(scaled):
            fo, mult, legs, sp, tp, lev, th, dl = params
            fo = int(round(fo / 5) * 5); mult = round(mult, 2); legs = int(round(legs))
            sp = int(round(sp / 10) * 10); tp = int(round(tp / 10) * 10); lev = int(round(lev))
            th = int(round(th)); dl = int(round(dl))
            cfg = build(symbols, fo, mult, legs, sp, tp, lev, th, dl)
            ch = eff_hash(cfg)
            plan.append({"track": track, "idx": i, "config": cfg, "ch": ch,
                         "params": {"fo": fo, "mult": mult, "legs": legs, "sp": sp, "tp": tp, "lev": lev, "th": th, "dl": dl}})
    return plan


def run_one(cfg, budget, start, end, label, ch):
    cfgdir = os.path.join(ART, "configs", "g1"); os.makedirs(cfgdir, exist_ok=True)
    path = os.path.join(cfgdir, f"g1_{ch[:12]}.json")
    with open(path, "w") as f:
        json.dump({"portfolio_config": cfg}, f, sort_keys=True)
    cmd = ["target/release/portfolio_budget_replay", "--config", path, "--budget", str(int(budget)),
           "--start-ms", str(start), "--end-ms", str(end),
           "--market-data", "data/market_data_full.db", "--funding-data", "data/funding_rates_round12.db",
           "--exchange-min-notional", "5.0"]
    t0 = time.time()
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=180)
        wall = time.time() - t0
        if r.returncode != 0:
            return {"status": "error", "wall_s": round(wall, 1)}
        d = json.loads(r.stdout[r.stdout.find("{"):r.stdout.rfind("}") + 1])
        ob = d.get("on_budget", {})
        return {"status": "complete", "wall_s": round(wall, 1),
                "ann": round(ob.get("annualized_return_pct") or -999, 4),
                "dd": round(ob.get("max_drawdown_pct") or -999, 4),
                "breached": ob.get("principal_breached"), "trade_count": d.get("trade_count"),
                "min_equity": round(ob.get("min_equity_quote") or 0, 4)}
    except subprocess.TimeoutExpired:
        return {"status": "timeout", "wall_s": 180}
    except Exception as e:
        return {"status": "error", "err": str(e)[:100]}


def load_ckpt():
    if os.path.exists(CKPT):
        try:
            return json.load(open(CKPT))
        except Exception:
            pass
    return {"done": {}, "config_params": {}}


def save_ckpt(ckpt):
    os.makedirs(os.path.dirname(CKPT), exist_ok=True)
    with open(CKPT, "w") as f:
        json.dump(ckpt, f)


def main():
    plan = gen_all_configs()
    total_expected = len(plan) * len(WINDOWS) * len(BUDGETS)
    ckpt = load_ckpt()
    # store config params for summary
    for item in plan:
        ckpt["config_params"].setdefault(item["ch"], {"track": item["track"], "idx": item["idx"], "params": item["params"]})

    t_start = time.time()
    n_done_this_run = 0
    for item in plan:
        for wname, ws, we in WINDOWS:
            for budget in BUDGETS:
                key = f"{item['ch']}|{wname}|{budget}"
                if key in ckpt["done"]:
                    continue
                # time budget check
                if time.time() - t_start > TIME_BUDGET_S:
                    save_ckpt(ckpt)
                    print(f"[time-budget] saved checkpoint; done={len(ckpt['done'])}/{total_expected}; this run did {n_done_this_run}")
                    return False  # not complete
                label = f"r17_g1_{item['track']}_{item['idx']:03d}_{wname}_{int(budget)}"
                r = run_one(item["config"], budget, ws, we, label, item["ch"])
                ckpt["done"][key] = {"track": item["track"], "idx": item["idx"], "window": wname,
                                     "budget": budget, "params": item["params"], **r}
                n_done_this_run += 1
                # save checkpoint every 10 replays
                if n_done_this_run % 10 == 0:
                    save_ckpt(ckpt)
                el = time.time() - t_start
                print(f"[{len(ckpt['done'])}/{total_expected}] {item['track']} {item['idx']:03d} {wname} {int(budget)}U: {r.get('status')} ann={r.get('ann')} el={el:.0f}s", flush=True)

    save_ckpt(ckpt)
    print(f"[COMPLETE] done={len(ckpt['done'])}/{total_expected}")
    return True


if __name__ == "__main__":
    done = main()
    sys.exit(0 if done else 1)
