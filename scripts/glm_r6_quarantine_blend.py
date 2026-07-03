#!/usr/bin/env python3
"""GLM Round 6: XRP+Quarantine × R4 Blend search.

Combines XRP+quarantine (ann55.8%/DD25.0%) with R4 combo (ann34.7%/DD17.7%)
at various blend ratios to find ann>50% AND DD≤20%.
"""
import argparse, json, os, subprocess, sys, time, copy
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [("h1_2023",1672531200000,1688169599999),("h2_2023",1688169600000,1704067199999),("2024",1704067200000,1735689599999),("2025",1735689600000,1767225599999),("2026_ytd",1767225600000,1780271999999)]
FULL_START, FULL_END = 1672531200000, 1780271999999

def run_replay(config, budget, s, e, pid):
    p = f"/tmp/r6QB_{os.getpid()}_{pid}.json"
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

def scale_weights(config, scale):
    cfg = copy.deepcopy(config)
    for s in cfg["strategies"]:
        w = float(s["portfolio_weight_pct"])
        s["portfolio_weight_pct"] = str(round(w * scale, 4))
    return cfg

def build_blend(components):
    """components: [(config, scale), ...]"""
    all_strats = []
    for cfg, scale in components:
        scaled = scale_weights(cfg, scale)
        for s in scaled["strategies"]:
            s["strategy_id"] = f"b_{s['strategy_id']}_{len(all_strats)}"
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
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=24)
    args = ap.parse_args()

    # Load XRP+quarantine config
    xrp_q = load_config("docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-C-best-quarantine.json")
    r4 = load_config("docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json")

    # Also load plain XRP (no quarantine) and ANKR for comparison
    univ = json.load(open("docs/superpowers/artifacts/glm-martingale-core-round5/r5-symbol-universe-expansion.json"))
    xrp_plain = None
    for v in univ["results"]:
        if v.get("label") == "replaceL2_XRPUSDT": xrp_plain = v["config"]; break

    jobs = []
    # Baselines
    jobs.append((xrp_q, "xrpq100"))
    if xrp_plain: jobs.append((xrp_plain, "xrp100"))
    jobs.append((r4, "r4100"))

    # Blends: XRP+quarantine × R4 at various ratios
    for xrp_pct in [0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]:
        for r4_pct in [0.2, 0.3, 0.4, 0.5, 0.6]:
            total = xrp_pct + r4_pct
            if total > 1.0 or total < 0.3: continue
            cfg = build_blend([(xrp_q, xrp_pct), (r4, r4_pct)])
            jobs.append((cfg, f"xrpq{int(xrp_pct*100)}_r4{int(r4_pct*100)}"))

    # Also XRP+quarantine plain (no R4) at reduced scale
    for xrp_pct in [0.5, 0.7, 0.9]:
        cfg = scale_weights(xrp_q, xrp_pct)
        jobs.append((cfg, f"xrpq{int(xrp_pct*100)}_cash"))

    print(f"[r6QB] {len(jobs)} candidates x 6 replays, {args.workers} workers", flush=True)
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
            if done % 5 == 0 or (ann > 45 and dd <= 22):
                s25 = rec.get("segment_returns",{}).get("2025","?")
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):24s} ann={ann} dd={dd} pos={rec.get('positive_segments')}/5 2025={s25}", flush=True)

    for v in results:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["target_cons"] = (ann > 50 and dd <= 10 and v["positive_segments"] >= 4)
        v["target_bal"] = (ann > 90 and dd <= 20 and v["positive_segments"] >= 4)
        v["nf_cons_dd"] = (ann >= 50 and dd <= 18 and v["positive_segments"] >= 4)
        v["nf_bal_dd"] = (ann >= 60 and dd <= 22 and v["positive_segments"] >= 4)
        v["frontier"] = (dd < 25.0 and ann > 40 and v["positive_segments"] >= 4 and v["agg_2024_2026"] > 0)
    results.sort(key=lambda v: ((v.get("positive_segments") or 0), (v.get("full_metrics") or {}).get("ann") or 0), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"results": results}, open(args.out, "w"), indent=2, default=str)
    nc = len([v for v in results if v.get("nf_cons_dd")])
    nb = len([v for v in results if v.get("nf_bal_dd")])
    nf = len([v for v in results if v.get("frontier")])
    print(f"\n[r6QB] wrote: {len(results)} results. NF-cons-dd={nc} NF-bal-dd={nb} frontier={nf} in {time.time()-t0:.0f}s")
    print("\n=== TOP 15 ===")
    for v in results[:15]:
        if "error" in v: continue
        fm = v.get("full_metrics") or {}
        s25 = (v.get("segment_returns") or {}).get("2025","n/a")
        print(f"  {v['label']:24s} ann={fm.get('ann'):7.1f} dd={fm.get('dd'):6.1f} pos={v['positive_segments']}/5 2025={s25}")
    # Save best that meets ann>50 and dd<=20
    hits = [v for v in results if v.get("frontier") and (v.get("full_metrics") or {}).get("ann",0) > 45]
    if hits:
        best = max(hits, key=lambda v: (v.get("full_metrics") or {}).get("ann",0))
        fm = best.get("full_metrics") or {}
        print(f"\nBEST ann>45/dd<25: {best['label']} ann={fm.get('ann'):.1f} dd={fm.get('dd'):.1f}")
        json.dump({"portfolio_config": best["config"]}, open("docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-QB-best.json", "w"), indent=2)

if __name__ == "__main__":
    main()
