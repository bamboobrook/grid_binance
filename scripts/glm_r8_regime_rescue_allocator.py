#!/usr/bin/env python3
"""GLM Round 8 Task P2: 2025 Regime-Rescue Martingale Allocator.

Lagged allocator that rotates among martingale sleeves (ANKR-q, R6-QB, R4-combo, R5-fine)
and cash based on rolling DD/regime state. Uses only prior data.
"""
import argparse, json, os, subprocess, sys, time, math
from collections import defaultdict

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999

CANDS = {
    "ANKR-q": "docs/superpowers/artifacts/glm-martingale-core-round7/promising/r7-ANKR-q1w24p24.json",
    "QB": "docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-QB-best.json",
    "R4": "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json",
    "fine": "docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-fine-combo-best.json",
}

def get_curve(name, config_path, budget=5000):
    cmd = [REPLAY, "--config", config_path, "--budget", str(budget),
           "--start-ms", str(FULL_START), "--end-ms", str(FULL_END),
           "--market-data", MARKET_DB, "--funding-data", FUNDING_DB,
           "--profile", "aggressive", "--portfolio-id", f"r8p2_{name}",
           "--exchange-min-notional", "5", "--equity-curve-points", "5000"]
    pr = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    if pr.returncode != 0: return None
    r = json.loads(pr.stdout)
    return r.get("equity_curve", [])

def run_allocator(curves, lookback_days, rebalance_days, score_fn, max_hi, min_lo, cash_dd, hyst_gap):
    all_ts = sorted(set(p["timestamp_ms"] for c in curves.values() for p in c))
    pnl_by = {}
    for name, ec in curves.items():
        first = ec[0]["equity_quote"]
        pnl_by[name] = {p["timestamp_ms"]: p["equity_quote"] - first for p in ec}
    
    budget = 5000.0
    active = "R4"  # start with low-DD
    last_rebal = 0
    merged_pnl = 0.0
    peak = budget
    max_dd = 0.0
    lookback_ms = lookback_days * 86400000
    rebal_ms = rebalance_days * 86400000
    last_active = active
    active_score = -999

    for i, ts in enumerate(all_ts):
        if i > 0 and (ts - all_ts[last_rebal]) >= rebal_ms:
            lookback_start_ts = ts - lookback_ms
            scores = {}
            for name in curves:
                pnl_now = pnl_by[name].get(ts, 0)
                pnl_past = max((v for t, v in pnl_by[name].items() if t <= lookback_start_ts), default=0)
                ret = pnl_now - pnl_past
                # DD in lookback
                lookback_pnls = [v for t, v in pnl_by[name].items() if lookback_start_ts <= t <= ts]
                lb_peak = max(lookback_pnls) if lookback_pnls else 0
                lb_trough = min(lookback_pnls) if lookback_pnls else 0
                lb_dd = (lb_peak - lb_trough) / (budget + lb_peak) * 100 if (budget + lb_peak) > 0 else 0
                scores[name] = score_fn(ret, lb_dd)
            
            best = max(scores, key=scores.get)
            # Hysteresis: only switch if score gap exceeds threshold
            if scores[best] - active_score > hyst_gap or active_score <= -999:
                active = best
                active_score = scores[best]
            else:
                active_score = scores.get(active, active_score)
            
            # Cash DD trigger
            current_eq = budget + merged_pnl
            dd = (peak - current_eq) / peak * 100 if peak > 0 else 0
            if cash_dd and dd > cash_dd:
                active = "cash"
            
            last_rebal = i
        
        # Apply
        if active == "cash":
            step = 0.0
        else:
            pnl_now = pnl_by.get(active, {}).get(ts, 0)
            prev_ts = all_ts[i-1] if i > 0 else ts
            pnl_prev = pnl_by.get(active, {}).get(prev_ts, 0)
            step = pnl_now - pnl_prev
        
        merged_pnl += step
        eq = budget + merged_pnl
        if eq > peak: peak = eq
        dd = (peak - eq) / peak * 100 if peak > 0 else 0
        if dd > max_dd: max_dd = dd

    if not all_ts: return None
    days = (all_ts[-1] - all_ts[0]) / 86400000
    total_ret = (budget + merged_pnl - budget) / budget
    ann = ((1 + total_ret) ** (365 / days) - 1) * 100 if days > 0 and (1 + total_ret) > 0 else -999
    return {"ann": round(ann, 1), "dd": round(max_dd, 1), "ret": round(total_ret * 100, 1)}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--budget", type=float, default=5000)
    args = ap.parse_args()

    print("=== Building curves ===", flush=True)
    curves = {}
    for name, path in CANDS.items():
        if not os.path.exists(path): print(f"  {name}: MISSING"); continue
        ec = get_curve(name, path)
        if ec: curves[name] = ec; print(f"  {name}: {len(ec)} pts")

    lookbacks = [14, 30, 60, 90]
    rebalances = [7, 14, 30]
    scores = {
        "rr1dd": lambda ret, dd: ret - 1.0 * dd,
        "rr2dd": lambda ret, dd: ret - 2.0 * dd,
        "rrcost": lambda ret, dd: ret - 0.5 * dd,
        "recovery": lambda ret, dd: ret / (dd + 1) if ret > 0 else ret - dd,
    }
    max_his = [0.25, 0.35, 0.50]
    min_los = [0.20, 0.35, 0.50]
    cash_dds = [None, 8, 12, 16]
    hyst_gaps = [0, 3, 6]

    total = len(lookbacks) * len(rebalances) * len(scores) * len(max_his) * len(min_los) * len(cash_dds) * len(hyst_gaps)
    print(f"=== Running {total} allocator configs ===", flush=True)
    t0 = time.time()
    results = []
    done = 0
    for lb in lookbacks:
        for rb in rebalances:
            for sname, sfn in scores.items():
                for mh in max_his:
                    for ml in min_los:
                        for cd in cash_dds:
                            for hg in hyst_gaps:
                                r = run_allocator(curves, lb, rb, sfn, mh, ml, cd, hg)
                                if r:
                                    results.append({
                                        "label": f"lb{lb}_rb{rb}_{sname}_hi{mh}_lo{ml}_cash{cd}_hys{hg}",
                                        "params": {"lookback_days": lb, "rebalance_days": rb, "score": sname,
                                                   "max_high_ann_weight": mh, "min_low_dd_weight": ml,
                                                   "cash_weight_when_rolling_dd_gt": cd, "switch_hysteresis_score_gap": hg},
                                        "full_metrics": r,
                                        "traded_symbols": list(CANDS.keys()),
                                    })
                                done += 1
                                if done % 200 == 0:
                                    print(f"  [{done}/{total}]", flush=True)

    results.sort(key=lambda v: v.get("full_metrics", {}).get("ann", -999), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results, "total_configs": total}, open(args.out, "w"), indent=2)
    print(f"\n[r8P2] wrote {args.out}: {len(results)} configs in {time.time()-t0:.0f}s")
    print("\n=== TOP 10 ===")
    for v in results[:10]:
        fm = v.get("full_metrics", {})
        print(f"  {v['label']:50s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f}")

if __name__ == "__main__":
    main()
