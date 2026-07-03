#!/usr/bin/env python3
"""GLM Round 6 Task F: Risk-Budget Blend of ANKR/XRP/DOGE/R4/fine/cash.

Combines high-return ANKR with lower-DD XRP/DOGE/R4 to find non-linear DD improvement.
Tests static blend frontiers. Full 5-segment validation.
"""
import argparse, json, os, subprocess, sys, time, copy
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999

def run_replay(config, budget, s, e, pid):
    p = f"/tmp/r6Ff_{os.getpid()}_{pid}.json"
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

def load_config(path):
    with open(path) as f: return json.load(f)["portfolio_config"]

def scale_strategy_weights(config, scale):
    """Scale all portfolio_weight_pct by a factor."""
    cfg = copy.deepcopy(config)
    for s in cfg["strategies"]:
        w = float(s["portfolio_weight_pct"])
        s["portfolio_weight_pct"] = str(round(w * scale, 4))
    return cfg

def build_blend(configs_with_weights):
    """Build a portfolio from multiple configs at given weight scales.
    configs_with_weights: [(config, scale), ...]
    """
    all_strats = []
    for cfg, scale in configs_with_weights:
        scaled = scale_strategy_weights(cfg, scale)
        for s in scaled["strategies"]:
            # Make strategy IDs unique
            s["strategy_id"] = f"blend_{s['strategy_id']}_{len(all_strats)}"
            all_strats.append(s)
    return {"direction_mode": "long_and_short", "strategies": all_strats,
            "risk_limits": {"max_global_budget_quote": "5000"}}

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
    ap.add_argument("--mode", default="static-frontier")
    ap.add_argument("--out", required=True)
    ap.add_argument("--report", default=None)
    ap.add_argument("--workers", type=int, default=20)
    args = ap.parse_args()

    ankr = load_config("docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-G-best-ANKRUSDT.json")
    r4 = load_config("docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json")
    fine = load_config("docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-fine-combo-best.json")

    # Also load XRP and DOGE from the universe expansion results
    try:
        univ = json.load(open("docs/superpowers/artifacts/glm-martingale-core-round5/r5-symbol-universe-expansion.json"))
        xrp = None; doge = None
        for v in univ["results"]:
            if v.get("label") == "replaceL2_XRPUSDT" and xrp is None: xrp = v["config"]
            if v.get("label") == "replaceL2_DOGEUSDT" and doge is None: doge = v["config"]
    except: xrp = None; doge = None

    # Static blend frontier: ANKR × XRP/R4/fine at various ratios
    jobs = []
    # Pure baselines
    for name, cfg in [("ankr100", ankr), ("r4100", r4), ("fine100", fine)]:
        if xrp: jobs.append((xrp, "xrp100"))
        if doge: jobs.append((doge, "doge100"))

    # Blends: ANKR at 20-80% × R4/fine at 20-80%
    for ankr_pct in [0.2, 0.4, 0.6, 0.8]:
        for r4_pct in [0.0, 0.2, 0.4, 0.6]:
            total = ankr_pct + r4_pct
            if total > 1.0 or total < 0.2: continue
            cfg = build_blend([(ankr, ankr_pct), (r4, r4_pct)])
            jobs.append((cfg, f"ankr{int(ankr_pct*100)}_r4{int(r4_pct*100)}"))
    # Blends: ANKR × fine
    for ankr_pct in [0.2, 0.4, 0.6, 0.8]:
        for fine_pct in [0.2, 0.4]:
            total = ankr_pct + fine_pct
            if total > 1.0: continue
            cfg = build_blend([(ankr, ankr_pct), (fine, fine_pct)])
            jobs.append((cfg, f"ankr{int(ankr_pct*100)}_fine{int(fine_pct*100)}"))
    # Blends: ANKR × XRP
    if xrp:
        for ankr_pct in [0.3, 0.5, 0.7]:
            for xrp_pct in [0.3, 0.5]:
                total = ankr_pct + xrp_pct
                if total > 1.0: continue
                cfg = build_blend([(ankr, ankr_pct), (xrp, xrp_pct)])
                jobs.append((cfg, f"ankr{int(ankr_pct*100)}_xrp{int(xrp_pct*100)}"))

    print(f"[r6F] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
            if done % 10 == 0 or (ann > 40 and dd <= 20):
                s25 = rec.get("segment_returns",{}).get("2025","?")
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):24s} ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 2025={s25}", flush=True)

    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["nf_cons_dd"] = (ann >= 50 and dd <= 18 and v["positive_segments"] >= 4)
        v["nf_cons_close"] = (ann >= 45 and dd <= 12 and v["positive_segments"] >= 4)
        v["nf_bal_dd"] = (ann >= 70 and dd <= 22 and v["positive_segments"] >= 4)
        v["frontier"] = (dd < 32.1 and ann > 40 and v["positive_segments"] >= 4 and v["agg_2024_2026"] > 0)
    results.sort(key=lambda v: ((v.get("positive_segments") or 0),
                                (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results}, open(args.out, "w"), indent=2, default=str)
    nc = len([v for v in results if v.get("nf_cons_dd")])
    ncc = len([v for v in results if v.get("nf_cons_close")])
    nb = len([v for v in results if v.get("nf_bal_dd")])
    nf = len([v for v in results if v.get("frontier")])
    print(f"\n[r6F] wrote {args.out}: {len(results)} results. NF-cons-dd={nc} NF-cons-close={ncc} NF-bal-dd={nb} frontier={nf} in {time.time()-t0:.0f}s")
    print("\n=== TOP 15 ===")
    for v in results[:15]:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        s25 = (v.get("segment_returns") or {}).get("2025","n/a")
        print(f"  {v['label']:24s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} pos={v['positive_segments']}/5 2025={s25}")

if __name__ == "__main__":
    main()
