#!/usr/bin/env python3
"""GLM Round 7 Task C: Funding/fee cost-aware martingale gate search.

Tests funding cost gate on all base candidates. The gate blocks new cycles
when expected funding cost (from lookback) exceeds threshold.
Also includes Task F taper safety orders.
"""
import argparse, json, os, subprocess, sys, time, copy
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999

def run_replay(config, budget, s, e, pid):
    p = f"/tmp/r7C_{os.getpid()}_{pid}.json"
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

def load(path):
    with open(path) as f: return json.load(f)["portfolio_config"]

CANDIDATES = {
    "R5-ANKR": "docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-G-best-ANKRUSDT.json",
    "R6-XRPQ": "docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-C-best-quarantine.json",
    "R5-fine": "docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-fine-combo-best.json",
    "R4-combo": "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json",
}

def apply_cost_gate(base, max_funding_bps, mode, taper_after, taper_scale):
    cfg = copy.deepcopy(base)
    for s in cfg["strategies"]:
        rl = s.get("risk_limits", {})
        if max_funding_bps is not None:
            rl["max_expected_funding_cost_bps"] = max_funding_bps
            rl["funding_side_bias_mode"] = mode
        if taper_after is not None:
            rl["taper_safety_after_leg"] = taper_after
            rl["taper_safety_scale"] = taper_scale
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
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=24)
    args = ap.parse_args()

    funding_thresholds = [None, 5, 10, 20, 35, 50]
    modes = ["entry_only", "entry_and_safety"]
    taper_configs = [(None, None), (3, 0.5), (4, 0.5), (3, 0.25)]

    jobs = []
    for cand_name, cand_path in CANDIDATES.items():
        if not os.path.exists(cand_path): continue
        base = load(cand_path)
        jobs.append((base, f"{cand_name}_base"))
        for ft in funding_thresholds:
            if ft is None: continue
            for mode in modes:
                for ta, ts in taper_configs:
                    cfg = apply_cost_gate(base, ft, mode, ta, ts)
                    tag = f"ft{ft}m{mode[:2]}t{ta}ts{ts}" if ta else f"ft{ft}m{mode[:2]}"
                    jobs.append((cfg, f"{cand_name}_{tag}"))
        # Also pure taper without funding gate
        for ta, ts in taper_configs:
            if ta is None: continue
            cfg = apply_cost_gate(base, None, None, ta, ts)
            jobs.append((cfg, f"{cand_name}_taper{ta}ts{ts}"))

    print(f"[r7C] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
            if done % 30 == 0 or (ann > 45 and dd <= 22):
                s25 = rec.get("segment_returns",{}).get("2025","?")
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):36s} ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 2025={s25}", flush=True)

    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["cost_frontier"] = (ann > 0 and dd < 30 and v["positive_segments"] >= 4 and v["agg_2024_2026"] > 0)
    results.sort(key=lambda v: ((v.get("positive_segments") or 0), (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results}, open(args.out, "w"), indent=2, default=str)
    nf = len([v for v in results if v.get("cost_frontier")])
    print(f"\n[r7C] wrote {args.out}: {len(results)} results, {nf} cost_frontier in {time.time()-t0:.0f}s")
    print("\n=== TOP 15 ===")
    for v in results[:15]:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        s25 = (v.get("segment_returns") or {}).get("2025","n/a")
        print(f"  {v['label']:36s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} pos={v['positive_segments']}/5 2025={s25}")

if __name__ == "__main__":
    main()
