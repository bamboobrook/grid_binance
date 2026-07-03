#!/usr/bin/env python3
"""GLM Round 6 Task B: Portfolio Drawdown State Machine search.

Tests staged DD rules that throttle entries (scale first order, freeze safety,
extend cooldown) instead of binary pause. Full 5-segment validation.
"""
import argparse, json, os, subprocess, sys, time, random
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999

def run_replay(config, budget, s, e, pid):
    p = f"/tmp/r6Bf_{os.getpid()}_{pid}.json"
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

def load_base(path):
    with open(path) as f: return json.load(f)["portfolio_config"]

def add_dd_rules(config, rules, recovery_pct, new_cycle_dd_pause):
    """Add drawdown state rules to portfolio risk_limits."""
    rl = config.get("risk_limits", {})
    if rules:
        rl["drawdown_state_rules"] = rules
    if recovery_pct is not None:
        rl["drawdown_state_recovery_pct"] = recovery_pct
    # Override new_cycle_drawdown_pause_pct to a high value so the DD state machine
    # can intercept instead of the binary pause
    if new_cycle_dd_pause is not None:
        rl["new_cycle_drawdown_pause_pct"] = new_cycle_dd_pause
    config["risk_limits"] = rl
    return config

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
    ap.add_argument("--base-config", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=24)
    args = ap.parse_args()
    base = load_base(args.base_config)

    # Grid per plan section 8
    state_thresholds = [
        [(6, {"first_order_scale": 0.8}), (10, {"first_order_scale": 0.6}), (14, {"first_order_scale": 0.4, "freeze_safety_orders": True})],
        [(8, {"first_order_scale": 0.7}), (12, {"first_order_scale": 0.5}), (16, {"first_order_scale": 0.3, "freeze_safety_orders": True})],
        [(10, {"first_order_scale": 1.0}), (15, {"first_order_scale": 0.5}), (20, {"first_order_scale": 0.25, "freeze_safety_orders": True})],
        [(12, {"first_order_scale": 0.8}), (18, {"first_order_scale": 0.4, "freeze_safety_orders": True}), (24, {"first_order_scale": 0.2, "freeze_safety_orders": True})],
    ]
    recoveries = [0.25, 0.50, 0.75]
    new_cycle_pauses = [50.0]  # high value so DD state machine intercepts

    jobs = []
    # Baseline (no DD rules)
    import copy
    b = copy.deepcopy(base)
    b["risk_limits"]["new_cycle_drawdown_pause_pct"] = 6.0  # engine default
    jobs.append((b, "baseline_default"))
    # High pause without rules (effectively no pause)
    b2 = copy.deepcopy(base)
    b2["risk_limits"]["new_cycle_drawdown_pause_pct"] = 50.0
    jobs.append((b2, "baseline_nopause"))

    for si, states in enumerate(state_thresholds):
        for ri, recovery in enumerate(recoveries):
            rules = [{"trigger_drawdown_pct": t, **actions} for t, actions in states]
            cfg = copy.deepcopy(base)
            cfg = add_dd_rules(cfg, rules, recovery, 50.0)
            lbl = f"s{si}r{ri}"
            jobs.append((cfg, lbl))

    print(f"[r6B] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
            s25 = rec.get("segment_returns",{}).get("2025","?")
            print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):20s} ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 2025={s25}", flush=True)

    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["nf_cons_dd"] = (ann >= 50 and dd <= 18 and v["positive_segments"] >= 4)
        v["nf_cons_close"] = (ann >= 45 and dd <= 12 and v["positive_segments"] >= 4)
        v["frontier"] = (ann > 59.5 and v["positive_segments"] >= 4 and v["agg_2024_2026"] > 0)
    results.sort(key=lambda v: ((v.get("positive_segments") or 0), (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results}, open(args.out, "w"), indent=2, default=str)
    nc = len([v for v in results if v.get("nf_cons_dd")])
    ncc = len([v for v in results if v.get("nf_cons_close")])
    nf = len([v for v in results if v.get("frontier")])
    print(f"\n[r6B] wrote {args.out}: {len(results)} results. NF-cons-dd={nc} NF-cons-close={ncc} frontier={nf} in {time.time()-t0:.0f}s")
    print("\n=== ALL ===")
    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        s25 = (v.get("segment_returns") or {}).get("2025","n/a")
        print(f"  {v['label']:20s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} pos={v['positive_segments']}/5 2025={s25}")

if __name__ == "__main__":
    main()
