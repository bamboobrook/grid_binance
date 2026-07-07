#!/usr/bin/env python3
"""GLM Round 8 Task P3: Dynamic Grid Reset Inside DCA.

Tests different spacing configurations (as proxy for dynamic reset) on the
R7-ANKR-q base. Uses existing engine (no new feature needed).
"""
import argparse, json, os, subprocess, sys, time, copy
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999

def run_replay(config, budget, s, e, pid):
    p = f"/tmp/r8P3_{os.getpid()}_{pid}.json"
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

BASES = {
    "ANKR-q": "docs/superpowers/artifacts/glm-martingale-core-round7/promising/r7-ANKR-q1w24p24.json",
    "fine": "docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-fine-combo-best.json",
    "QB": "docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-QB-best.json",
    "R4": "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json",
}

def apply_spacing(base, long_step, short_step, so_condition):
    cfg = copy.deepcopy(base)
    for s in cfg["strategies"]:
        # Override spacing
        if "fixed_percent" in s.get("spacing", {}):
            is_long = s.get("direction") == "long"
            s["spacing"]["fixed_percent"]["step_bps"] = long_step if is_long else short_step
        # Override safety condition
        rl = s.get("risk_limits", {})
        if so_condition == "existing_and_adx_below_30":
            rl["safety_skip_adx_threshold"] = 30
        elif so_condition == "existing_and_rsi_below_40" and s.get("direction") == "long":
            rl["safety_order_condition"] = "rsi(14) < 40"
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

    long_steps = [120, 150, 180, 210]
    short_steps = [150, 180, 250]
    so_conditions = ["existing", "existing_and_adx_below_30", "existing_and_rsi_below_40"]

    jobs = []
    for bname, bpath in BASES.items():
        if not os.path.exists(bpath): continue
        base = load(bpath)
        jobs.append((base, f"{bname}_base"))
        for ls in long_steps:
            for ss in short_steps:
                for soc in so_conditions:
                    cfg = apply_spacing(base, ls, ss, soc)
                    jobs.append((cfg, f"{bname}_ls{ls}ss{ss}_{soc[:6]}"))

    print(f"[r8P3] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):32s} ann={ann} dd={dd} pos={rec.get('positive_segments')}/5", flush=True)

    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["target"] = (ann > 50 and dd <= 20 and v["positive_segments"] >= 4)
    results.sort(key=lambda v: ((v.get("positive_segments") or 0), (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results}, open(args.out, "w"), indent=2, default=str)
    nt = len([v for v in results if v.get("target")])
    print(f"\n[r8P3] wrote {args.out}: {len(results)} results, {nt} target hits in {time.time()-t0:.0f}s")
    print("\n=== TOP 10 ===")
    for v in results[:10]:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        print(f"  {v['label']:32s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} pos={v['positive_segments']}/5")

if __name__ == "__main__":
    main()
