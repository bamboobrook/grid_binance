#!/usr/bin/env python3
"""R4 G1 cheap-stress Sobol screen (plan §11).

Generates 128 unique Sobol effective configs across T1/T2/T3-8/T3-12 with the
plan's fixed quotas and scramble seeds, then runs REAL binary replays on the
four earliest-train 30-day windows × 1000/3000/4999U.

Plan target: 128 × 4 × 3 = 1536 actual binary replays. Each is a real full
30-day BatchReplay (not a fast screen). Duplicate keys cache-hit and do NOT
count as executed.

Uses the asymmetric regime router (htf_regime_gate_enabled) so long+short
sleeves are admitted by regime, NOT symmetric.
"""
import hashlib
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from scipy.stats import qmc  # noqa: E402

ART = "docs/superpowers/artifacts/glm-martingale-core-round16"

# Four earliest-train 30-day windows (plan §11, fixed, no hindsight)
WINDOWS = [
    ("w1", 1672531200000, 1672531200000 + 30 * 86_400_000),       # 2023-01-01..01-30
    ("w2", 1676419200000, 1676419200000 + 30 * 86_400_000),       # 2023-02-15..03-16
    ("w3", 1680307200000, 1680307200000 + 30 * 86_400_000),       # 2023-04-01..04-30
    ("w4", 1684934400000, 1684934400000 + 30 * 86_400_000),       # 2023-05-25..06-23
]
BUDGETS = [1000.0, 3000.0, 4999.0]

# Track quotas + sampler seeds (plan §11)
TRACK_QUOTA = {
    "T1": (12, 20260715),
    "T2": (72, 20260716),
    "T3_8": (22, 20260717),
    "T3_12": (22, 20260718),
}

T1_DIAG = ["BNBUSDT", "TRXUSDT", "ANKRUSDT", "AAVEUSDT", "SOLUSDT", "DOTUSDT"]
T2 = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT", "TRXUSDT", "LINKUSDT", "LTCUSDT", "BCHUSDT", "DOTUSDT"]
T3_8 = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT", "TRXUSDT"]
TRACK_SYMBOLS = {"T1": T1_DIAG, "T2": T2, "T3_8": T3_8, "T3_12": T2}

# Param bounds (balanced envelope center, with asymmetric short ladder)
PARAM_BOUNDS = [
    (10, 45),    # first_order_quote (long)
    (1.25, 2.2),  # multiplier (long)
    (3, 7),       # max_legs (long)
    (80, 250),    # spacing_bps
    (80, 250),    # tp_bps
    (3, 8),       # leverage
]


def build_config(symbols, fo, mult, legs, sp, tp, lev):
    n = len(symbols)
    wt = round(100.0 / (2 * n), 4)  # long + short per symbol
    strats = []
    for sym in symbols:
        # LONG sleeve (router-admitted in BULL/RANGE)
        strats.append(_sleeve(sym, "long", fo, mult, legs, sp, tp, lev, wt))
        # SHORT sleeve (shallower: mult*0.85, legs-1, spacing+50; router-admitted in BEAR/RANGE)
        strats.append(_sleeve(sym, "short", fo, round(mult * 0.85, 2), max(3, legs - 1), sp + 50, tp, lev, wt))
    return {"direction_mode": "long_and_short",
            "risk_limits": {"max_global_budget_quote": "4999"},
            "strategies": strats}


def _sleeve(sym, direction, fo, mult, legs, sp, tp, lev, wt):
    return {
        "strategy_id": f"{'L' if direction == 'long' else 'S'}-{sym}", "symbol": sym,
        "market": "usd_m_futures", "direction": direction, "direction_mode": "long_and_short",
        "margin_mode": "isolated", "leverage": int(lev),
        "spacing": {"fixed_percent": {"step_bps": int(sp)}},
        "sizing": {"multiplier": {"first_order_quote": str(int(fo)), "multiplier": str(mult), "max_legs": int(legs)}},
        "take_profit": {"percent": {"bps": int(tp)}}, "stop_loss": None, "indicators": [],
        "entry_triggers": [{"cooldown": {"seconds": 39600}}],
        "portfolio_weight_pct": str(wt),
        "risk_limits": {"htf_regime_gate_enabled": True},  # asymmetric router admission
    }


def eff_hash(cfg):
    return hashlib.sha256(json.dumps(cfg, sort_keys=True).encode()).hexdigest()


def run_cli(config, budget, start, end):
    cfgdir = os.path.join(ART, "configs", "g1")
    os.makedirs(cfgdir, exist_ok=True)
    h = eff_hash(config)[:12]
    path = os.path.join(cfgdir, f"g1_{h}.json")
    with open(path, "w") as f:
        json.dump({"portfolio_config": config}, f, sort_keys=True)
    cmd = ["target/release/portfolio_budget_replay", "--config", path,
           "--budget", str(int(budget)), "--start-ms", str(start), "--end-ms", str(end),
           "--market-data", "data/market_data_full.db",
           "--funding-data", "data/funding_rates_round12.db",
           "--exchange-min-notional", "5.0"]
    t0 = time.time()
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=180)
        wall = time.time() - t0
        if r.returncode != 0:
            return {"status": "error", "wall_s": round(wall, 1), "err": r.stderr[:150]}
        d = json.loads(r.stdout[r.stdout.find("{"):r.stdout.rfind("}") + 1])
        ob = d.get("on_budget", {})
        return {"status": "complete", "wall_s": round(wall, 1),
                "ann": round(ob.get("annualized_return_pct") or -999, 4),
                "dd": round(ob.get("max_drawdown_pct") or -999, 4),
                "breached": ob.get("principal_breached"), "raw_command": " ".join(cmd[:3]) + " ...",
                "exit_code": r.returncode, "config_hash": h}
    except subprocess.TimeoutExpired:
        return {"status": "timeout", "wall_s": 180, "config_hash": h}
    except Exception as e:
        return {"status": "error", "err": str(e)[:150]}


def append_registry(rec):
    with open(os.path.join(ART, "exploration-registry.jsonl"), "a") as f:
        f.write(json.dumps(rec, sort_keys=True) + "\n")


def main():
    all_results = []
    seen_keys = set()
    actual_replays = 0
    duplicates = 0
    timeouts = 0
    cache_hits = 0
    unique_configs = set()
    t_start = time.time()

    for track, (quota, seed) in TRACK_QUOTA.items():
        symbols = TRACK_SYMBOLS[track]
        sampler = qmc.Sobol(d=len(PARAM_BOUNDS), scramble=True, seed=seed)
        n_draw = 1
        while n_draw < quota:
            n_draw *= 2
        sample = sampler.random(n_draw)[:quota]
        lower = [b[0] for b in PARAM_BOUNDS]
        upper = [b[1] for b in PARAM_BOUNDS]
        scaled = qmc.scale(sample, lower, upper)

        for i, params in enumerate(scaled):
            fo, mult, legs, sp, tp, lev = params
            fo = int(round(fo / 5) * 5)
            mult = round(mult, 2)
            legs = int(round(legs))
            sp = int(round(sp / 10) * 10)
            tp = int(round(tp / 10) * 10)
            lev = int(round(lev))
            cfg = build_config(symbols, fo, mult, legs, sp, tp, lev)
            ch = eff_hash(cfg)
            unique_configs.add(ch)
            cfg_short = {"fo": fo, "m": mult, "legs": legs, "sp": sp, "tp": tp, "lev": lev}

            for wname, ws, we in WINDOWS:
                for budget in BUDGETS:
                    key = f"{ch}|{wname}|{budget}"
                    label = f"r16_g1_{track}_{i:03d}_{wname}_{int(budget)}"
                    if key in seen_keys:
                        duplicates += 1
                        cache_hits += 1
                        append_registry({"experiment_id": label, "family": f"G1_{track}",
                                         "track": track, "window": wname, "budget": budget,
                                         "effective_config_hash": ch, "status": "skipped_duplicate",
                                         "actual_binary_replays": 0, "cache_hits": 1,
                                         "raw_command": "cache", "exit_code": 0})
                        continue
                    seen_keys.add(key)
                    running = {"experiment_id": label, "family": f"G1_{track}", "track": track,
                               "hypothesis": "G1 Sobol cheap-stress", "window": wname, "budget": budget,
                               "effective_config_hash": ch, "resolved_config_hash": ch,
                               "params": cfg_short, "seed": seed, "sampler": "scipy_sobol",
                               "status": "running", "started_at": time.time(),
                               "raw_command": "portfolio_budget_replay"}
                    append_registry(running)
                    result = run_cli(cfg, budget, ws, we)
                    rec = dict(running)
                    rec.update(result)
                    rec["status"] = result.get("status", "error")
                    rec["actual_binary_replays"] = 1 if result.get("status") == "complete" else 0
                    rec["cache_hits"] = 0
                    rec["finished_at"] = time.time()
                    if result.get("status") == "complete":
                        actual_replays += 1
                    if result.get("status") == "timeout":
                        timeouts += 1
                    append_registry(rec)
                    all_results.append({"track": track, "idx": i, "window": wname, "budget": budget,
                                        **{k: result.get(k) for k in ("ann", "dd", "breached", "status")}})
            elapsed = time.time() - t_start
            print(f"[{track} {i+1}/{quota}] fo{fo} m{mult} l{legs} s{sp} t{tp} lev{lev}: replays={actual_replays} elapsed={elapsed:.0f}s", flush=True)

    out = {"schema_version": 1, "phase": "R4_G1", "sampler": "scipy_sobol",
           "unique_configs": len(unique_configs), "actual_replays": actual_replays,
           "duplicates": duplicates, "timeouts": timeouts, "cache_hits": cache_hits,
           "windows": len(WINDOWS), "budgets": len(BUDGETS),
           "wall_s": round(time.time() - t_start, 1),
           "results_sample": all_results[:50]}
    os.makedirs(os.path.join(ART, "r4"), exist_ok=True)
    with open(os.path.join(ART, "r4", "g1-sobol.json"), "w") as f:
        json.dump(out, f, indent=2, sort_keys=True)
    print(f"\nG1 done: unique_configs={len(unique_configs)} actual_replays={actual_replays} dups={duplicates} timeouts={timeouts}")


if __name__ == "__main__":
    main()
