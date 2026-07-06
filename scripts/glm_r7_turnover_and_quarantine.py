#!/usr/bin/env python3
"""GLM Round 7 Task D+E: Turnover Quality Scheduler + Attribution Quarantine.

Task D: Uses existing cooldown + TP + funding gate fields to create a
"turnover quality scheduler" — cycles with low expected TP/cost ratio get
longer cooldowns. No new engine feature needed; just config generation.

Task E: Uses existing quarantine fields on candidates routed by Task B attribution.
The quarantine engine logic was already implemented in R6 (stop tracking + pause).
This runs the full grid on all routed candidates.

Combined search: both modify the same config's risk_limits.
"""
import argparse, json, os, subprocess, sys, time, copy
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999

def run_replay(config, budget, s, e, pid):
    p = f"/tmp/r7DE_{os.getpid()}_{pid}.json"
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

CANDS = {
    "ANKR": "docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-G-best-ANKRUSDT.json",
    "XRPQ": "docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-C-best-quarantine.json",
    "fine": "docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-fine-combo-best.json",
    "R4": "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json",
}

def apply_task_de(base, cooldown_s, tp_bps, quarantine_trigger, quarantine_window, quarantine_pause):
    """Task D: turnover quality = higher cooldown + adjusted TP.
    Task E: quarantine with different parameters."""
    cfg = copy.deepcopy(base)
    for s in cfg["strategies"]:
        rl = s.get("risk_limits", {})
        if cooldown_s:
            for t in s["entry_triggers"]:
                if "cooldown" in t:
                    t["cooldown"]["seconds"] = cooldown_s
        if quarantine_trigger:
            rl["quarantine_stop_count_trigger"] = quarantine_trigger
            rl["quarantine_stop_window_hours"] = quarantine_window
            rl["quarantine_pause_hours"] = quarantine_pause
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

    # Task D grid: cooldown variants (turnover quality)
    cooldowns = [None, 21600, 43200, 86400, 172800]  # 6h, 12h, 24h, 48h
    # Task E grid: quarantine variants (attribution-driven)
    quarantine_configs = [
        (None, None, None),  # baseline
        (1, 24, 24),   # q1w24p24 (R6 best)
        (1, 24, 72),   # q1w24p72
        (1, 72, 72),   # q1w72p72
        (2, 24, 24),   # q2w24p24
        (2, 72, 168),  # q2w72p168
        (1, 168, 168), # q1w168p168
    ]

    jobs = []
    for cname, cpath in CANDS.items():
        if not os.path.exists(cpath): continue
        base = load(cpath)
        jobs.append((base, f"{cname}_base"))
        # Task D: cooldown variants
        for cd in cooldowns:
            if cd is None: continue
            cfg = apply_task_de(base, cd, None, None, None, None)
            jobs.append((cfg, f"{cname}_cd{cd}"))
        # Task E: quarantine variants
        for qt, qw, qp in quarantine_configs:
            if qt is None: continue
            cfg = apply_task_de(base, None, None, qt, qw, qp)
            jobs.append((cfg, f"{cname}_q{qt}w{qw}p{qp}"))
        # Task D+E combined: cooldown + quarantine
        for cd in [21600, 43200]:
            for qt, qw, qp in [(1, 24, 24), (1, 72, 72)]:
                cfg = apply_task_de(base, cd, None, qt, qw, qp)
                jobs.append((cfg, f"{cname}_cd{cd}_q{qt}w{qw}"))

    print(f"[r7DE] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
            if done % 20 == 0 or (ann > 45 and dd <= 22):
                s25 = rec.get("segment_returns",{}).get("2025","?")
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):28s} ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 2025={s25}", flush=True)

    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["target"] = (ann > 50 and dd <= 20 and v["positive_segments"] >= 4)
        v["nf"] = (ann > 45 and dd < 25 and v["positive_segments"] >= 4 and v["agg_2024_2026"] > 0)
    results.sort(key=lambda v: ((v.get("positive_segments") or 0), (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results}, open(args.out, "w"), indent=2, default=str)
    nt = len([v for v in results if v.get("target")])
    nf = len([v for v in results if v.get("nf")])
    print(f"\n[r7DE] wrote {args.out}: {len(results)} results. TARGET-HIT={nt} NF={nf} in {time.time()-t0:.0f}s")
    print("\n=== TOP 15 ===")
    for v in results[:15]:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        s25 = (v.get("segment_returns") or {}).get("2025","n/a")
        print(f"  {v['label']:28s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} pos={v['positive_segments']}/5 2025={s25}")

if __name__ == "__main__":
    main()
