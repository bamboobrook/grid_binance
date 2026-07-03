#!/usr/bin/env python3
"""GLM Round 6 Task D+E: Partial-TP Trailing Lock + Safety Freeze search.

Task D: trailing profit lock after partial TP stage fires.
Task E: freeze safety orders after partial TP stage or at portfolio DD.
Combined search since both modify the same exit/SO path.
"""
import argparse, json, os, subprocess, sys, time, copy
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999

def run_replay(config, budget, s, e, pid):
    p = f"/tmp/r6DE_{os.getpid()}_{pid}.json"
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

def add_task_de(config, trailing_after_stage, activation_bps, callback_bps, floor_bps, freeze_after_stage):
    """Add trailing lock and safety freeze fields to all strategy risk_limits."""
    cfg = copy.deepcopy(config)
    for s in cfg["strategies"]:
        rl = s.get("risk_limits", {})
        if trailing_after_stage is not None:
            rl["trailing_lock_after_stage"] = trailing_after_stage
            rl["trailing_lock_activation_bps"] = activation_bps
            rl["trailing_lock_callback_bps"] = callback_bps
            rl["trailing_lock_floor_bps"] = floor_bps
        if freeze_after_stage is not None:
            rl["freeze_safety_after_partial_tp_stage"] = freeze_after_stage
        s["risk_limits"] = rl
    return cfg

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

    # Grid per plan Task D + E
    trailing_configs = [
        (None, None, None, None, None),  # baseline (no trailing)
        (1, 800, 250, 0, None),   # arm after stage 1, activation 800, callback 250, floor 0
        (1, 1000, 250, 50, None),
        (1, 600, 400, 0, None),
        (0, 800, 250, 0, None),   # arm after stage 0 (earliest)
        (1, 1400, 600, 100, None),
    ]
    freeze_configs = [
        None,  # no freeze
        0,     # freeze after stage 0
        1,     # freeze after stage 1
    ]

    jobs = []
    for ti, (ta, aa, ca, fa, _) in enumerate(trailing_configs):
        for fi, freeze in enumerate(freeze_configs):
            cfg = add_task_de(base, ta, aa, ca, fa, freeze)
            lbl = f"t{ti}f{fi}"
            jobs.append((cfg, lbl))

    print(f"[r6DE] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
            print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):8s} ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 2025={s25}", flush=True)

    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["frontier"] = (dd < 32.1 and ann > 40 and v["positive_segments"] >= 4 and v["agg_2024_2026"] > 0)
    results.sort(key=lambda v: ((v.get("positive_segments") or 0), (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results}, open(args.out, "w"), indent=2, default=str)
    nf = len([v for v in results if v.get("frontier")])
    print(f"\n[r6DE] wrote {args.out}: {len(results)} results, {nf} frontier in {time.time()-t0:.0f}s")
    print("\n=== ALL ===")
    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        s25 = (v.get("segment_returns") or {}).get("2025","n/a")
        print(f"  {v['label']:8s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} pos={v['positive_segments']}/5 2025={s25}")

if __name__ == "__main__":
    main()
