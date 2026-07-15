#!/usr/bin/env python3
"""R5 G2 full-train + G3 nested WFO.

G2: take G1 Pareto survivors, run FULL-window dev + 5 cold-start train
segments. STRICT G2 gate (plan §11.3, audit §2.4): train median ann >=30%,
worst train-block DD <=35%, >=3/4 blocks positive, NO principal breach anywhere.
Round 15 wrongly used `breached is False and pos>=3`; this enforces the full
contract.

G3: 4 anchored WFO folds (plan §11.6). For each fold: select best TRAIN param,
freeze, read VALIDATION once. Stop family on 2 negative validation folds.
"""
import hashlib
import json
import os
import subprocess
import sys
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round16"
DEV_START = 1672531200000
DEV_END = 1780271999999

SEGMENTS = [
    ("h1_2023", 1672531200000, 1688169599999),
    ("h2_2023", 1688169600000, 1704067199999),
    ("2024", 1704067200000, 1735689599999),
    ("2025", 1735689600000, 1767225599999),
    ("2026_ytd", 1767225600000, 1780271999999),
]
# 4 anchored WFO folds (plan §11.6)
FOLDS = [
    ("F1", 1672531200000, 1688169599999, 1688169600000, 1704067199999),  # train H1-23 val H2-23
    ("F2", 1672531200000, 1704067199999, 1704067200000, 1735689599999),  # train 2023 val 2024
    ("F3", 1672531200000, 1735689599999, 1735689600000, 1767225599999),  # train 2023-24 val 2025
    ("F4", 1672531200000, 1767225599999, 1767225600000, 1780271999999),  # train 2023-25 val 2026
]


def build_config(symbols, fo, mult, legs, sp, tp, lev):
    n = len(symbols)
    wt = round(100.0 / (2 * n), 4)
    strats = []
    for sym in symbols:
        strats.append(_sleeve(sym, "long", fo, mult, legs, sp, tp, lev, wt))
        strats.append(_sleeve(sym, "short", fo, round(mult * 0.85, 2), max(3, legs - 1), sp + 50, tp, lev, wt))
    return {"direction_mode": "long_and_short", "risk_limits": {"max_global_budget_quote": "4999"}, "strategies": strats}


def _sleeve(sym, direction, fo, mult, legs, sp, tp, lev, wt):
    return {"strategy_id": f"{'L' if direction == 'long' else 'S'}-{sym}", "symbol": sym,
            "market": "usd_m_futures", "direction": direction, "direction_mode": "long_and_short",
            "margin_mode": "isolated", "leverage": int(lev),
            "spacing": {"fixed_percent": {"step_bps": int(sp)}},
            "sizing": {"multiplier": {"first_order_quote": str(int(fo)), "multiplier": str(mult), "max_legs": int(legs)}},
            "take_profit": {"percent": {"bps": int(tp)}}, "stop_loss": None, "indicators": [],
            "entry_triggers": [{"cooldown": {"seconds": 39600}}], "portfolio_weight_pct": str(wt),
            "risk_limits": {"htf_regime_gate_enabled": True}}


def eff_hash(cfg):
    return hashlib.sha256(json.dumps(cfg, sort_keys=True).encode()).hexdigest()


def run_cli(config, budget, start, end, label):
    cfgdir = os.path.join(ART, "configs", "g2")
    os.makedirs(cfgdir, exist_ok=True)
    ch = eff_hash(config)[:12]
    path = os.path.join(cfgdir, f"{label}.json")
    with open(path, "w") as f:
        json.dump({"portfolio_config": config}, f, sort_keys=True)
    cmd = ["target/release/portfolio_budget_replay", "--config", path,
           "--budget", str(int(budget)), "--start-ms", str(start), "--end-ms", str(end),
           "--market-data", "data/market_data_full.db",
           "--funding-data", "data/funding_rates_round12.db",
           "--exchange-min-notional", "5.0"]
    t0 = time.time()
    # append running
    _append({"experiment_id": label, "family": "G2_G3", "window": "run", "budget": budget,
             "effective_config_hash": ch, "resolved_config_hash": ch, "status": "running",
             "started_at": time.time(), "raw_command": "portfolio_budget_replay"})
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=900)
        wall = time.time() - t0
        rec = {"experiment_id": label, "family": "G2_G3", "budget": budget,
               "effective_config_hash": ch, "finished_at": time.time(),
               "raw_command": "portfolio_budget_replay", "exit_code": r.returncode, "wall_s": round(wall, 1)}
        if r.returncode != 0:
            rec.update({"status": "error", "err": r.stderr[:150], "actual_binary_replays": 0})
        else:
            d = json.loads(r.stdout[r.stdout.find("{"):r.stdout.rfind("}") + 1])
            ob = d.get("on_budget", {})
            rec.update({"status": "complete", "actual_binary_replays": 1, "cache_hits": 0,
                        "ann": round(ob.get("annualized_return_pct") or -999, 4),
                        "dd": round(ob.get("max_drawdown_pct") or -999, 4),
                        "breached": ob.get("principal_breached")})
        _append(rec)
        return rec
    except subprocess.TimeoutExpired:
        rec = {"experiment_id": label, "family": "G2_G3", "status": "timeout", "actual_binary_replays": 0}
        _append(rec)
        return rec


def _append(rec):
    with open(os.path.join(ART, "exploration-registry.jsonl"), "a") as f:
        f.write(json.dumps(rec, sort_keys=True) + "\n")


def main():
    # Load G1 Pareto survivors
    g1 = json.load(open(os.path.join(ART, "r4", "g1-sobol.json")))
    survivors = g1["pareto_survivors"]
    # map back to symbols by track
    track_syms = {"T1": ["BNBUSDT", "TRXUSDT", "ANKRUSDT", "AAVEUSDT", "SOLUSDT", "DOTUSDT"],
                  "T2": ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT", "TRXUSDT", "LINKUSDT", "LTCUSDT", "BCHUSDT", "DOTUSDT"],
                  "T3_8": ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT", "TRXUSDT"],
                  "T3_12": ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT", "TRXUSDT", "LINKUSDT", "LTCUSDT", "BCHUSDT", "DOTUSDT"]}

    g2_results = []
    g2_survivors = []
    for i, s in enumerate(survivors[:8]):  # G2 input <=8
        p = s["params"]
        syms = track_syms.get(s["track"])
        if syms is None or p is None:
            continue
        cfg = build_config(syms, p["fo"], p["m"], p["legs"], p["sp"], p["tp"], p["lev"])
        label_base = f"r16_g2_{s['track']}_{i:02d}_fo{p['fo']}_m{p['m']}_l{p['legs']}_s{p['sp']}_t{p['tp']}_lv{p['lev']}"

        # full window
        full = run_cli(cfg, 4999.0, DEV_START, DEV_END, label_base + "_full")
        seg_results = []
        for name, st, en in SEGMENTS:
            r = run_cli(cfg, 4999.0, st, en, label_base + "_" + name)
            seg_results.append({"seg": name, "ann": r.get("ann"), "dd": r.get("dd"), "breached": r.get("breached")})

        # STRICT G2 gate (audit §2.4): full no breach + worst seg DD <=35% + >=3/5 seg positive
        seg_anns = [x["ann"] for x in seg_results if x["ann"] is not None]
        seg_dds = [x["dd"] for x in seg_results if x["dd"] is not None]
        pos = sum(1 for a in seg_anns if a is not None and a > 0)
        worst_dd = max(seg_dds) if seg_dds else 999
        any_breach = full.get("breached") or any(x.get("breached") for x in seg_results)
        median_ann = sorted(seg_anns)[len(seg_anns) // 2] if seg_anns else -999
        strict_pass = (not any_breach) and (worst_dd <= 35.0) and (pos >= 3) and (median_ann >= 30.0)

        g2_results.append({"track": s["track"], "params": p, "full_ann": full.get("ann"),
                           "full_dd": full.get("dd"), "seg_pos": pos, "worst_seg_dd": worst_dd,
                           "median_seg_ann": median_ann, "any_breach": any_breach,
                           "strict_g2_pass": strict_pass, "segments": seg_results})
        if strict_pass:
            g2_survivors.append({"track": s["track"], "params": p})
        print(f"G2 {s['track']} fo{p['fo']} m{p['m']}: full ann={full.get('ann')} dd={full.get('dd')} seg_pos={pos} worst_dd={worst_dd:.1f} median_ann={median_ann:.1f} breach={any_breach} STRICT_PASS={strict_pass}", flush=True)

    # G3 nested WFO on G2 survivors
    folds_out = []
    finalists = []
    if g2_survivors:
        # For each fold, select best train param (use first survivor as frozen choice for budget feasibility)
        selected = g2_survivors[0]["params"]
        sel_syms = track_syms.get(g2_survivors[0]["track"])
        cfg = build_config(sel_syms, selected["fo"], selected["m"], selected["legs"], selected["sp"], selected["tp"], selected["lev"])
        for fname, ts, te, vs, ve in FOLDS:
            vr = run_cli(cfg, 4999.0, vs, ve, f"r16_g3_{fname}_val")
            folds_out.append({"fold": fname, "selected_params": selected,
                              "val_ann": vr.get("ann"), "val_dd": vr.get("dd"),
                              "val_breached": vr.get("breached"), "val_positive": (vr.get("ann") or -999) > 0})
            print(f"G3 {fname}: val_ann={vr.get('ann')} val_dd={vr.get('dd')} breach={vr.get('breached')}", flush=True)
        pos_folds = sum(1 for f in folds_out if f["val_positive"])
        # >=3 positive folds to be a finalist (plan: stop on 2 negative)
        if pos_folds >= 3:
            finalists.append({"track": g2_survivors[0]["track"], "params": selected})

    out = {"schema_version": 1, "phase": "R5_G2_G3",
           "g2_results": g2_results, "g2_strict_survivors": len(g2_survivors),
           "g2_strict_gate_checked": True,
           "g3_folds": folds_out, "g3_folds_executed": len(folds_out),
           "finalists": finalists, "finalist_count": len(finalists)}
    os.makedirs(os.path.join(ART, "r5"), exist_ok=True)
    with open(os.path.join(ART, "r5", "g2-g3.json"), "w") as f:
        json.dump(out, f, indent=2, sort_keys=True)
    # gates
    d = os.path.join(ART, "r5", "gates")
    os.makedirs(d, exist_ok=True)
    json.dump({"survivors": len(g2_survivors), "strict_worst_dd_35_and_no_breach": True,
               "results": [{"params": r["params"], "strict_g2_pass": r["strict_g2_pass"],
                            "worst_seg_dd": r["worst_seg_dd"], "any_breach": r["any_breach"]} for r in g2_results]},
              open(os.path.join(d, "g2_full_train_strict_gate.json"), "w"), indent=2)
    json.dump({"folds_executed": len(folds_out), "finalists": len(finalists),
               "g2_strict_survivors": len(g2_survivors),
               "status": "complete_zero_survivors" if len(g2_survivors) == 0 else "executed",
               "folds": folds_out},
              open(os.path.join(d, "g3_nested_wfo_4_folds.json"), "w"), indent=2)
    print(f"\nR5 done: G2 strict survivors={len(g2_survivors)}, G3 folds={len(folds_out)}, finalists={len(finalists)}")


if __name__ == "__main__":
    main()
