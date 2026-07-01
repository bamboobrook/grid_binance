#!/usr/bin/env python3
"""GLM Martingale Core — Portfolio optimization around the breakthrough structure.

The breakthrough (glm_portfolio_segment_search.py) found that
long-bull(BNB/BCH) + crash-short(AAVE) with loose gates makes BOTH 2024 and
2025 positive for the first time ever. Remaining blockers:
  - 2026_ytd bear: BCH long loses -29% (BTC>ema50 holds 51% of 2026 even in bear)
  - h2_2023: late-bull reversal burns the short.

This script optimizes around that structure with:
  1. Tighter long gate: require per-symbol close>ema50 (not just ema100) so the
     long leg stops firing when the symbol is actually in downtrend.
  2. Tighter stop-loss on longs to cap 2026 bear losses.
  3. More/stronger crash-short legs to capture 2026 crash.
  4. Optional portfolio DD pause (research env) to test its DD-cap effect.

Usage:
  python3 scripts/glm_portfolio_optimize.py \
      --out docs/superpowers/artifacts/glm-martingale-core/portfolio-optimized.json
"""
import argparse
import itertools
import json
import os
import subprocess
import sys
import time
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"

FULL_SEGMENTS = [
    ("h1_2023", 1672531200000, 1688169599999),
    ("h2_2023", 1688169600000, 1704067199999),
    ("2024", 1704067200000, 1735689599999),
    ("2025", 1735689600000, 1767225599999),
    ("2026_ytd", 1767225600000, 1780271999999),
]
FULL_START, FULL_END = 1672531200000, 1780271999999


def run_replay(config, budget, start_ms, end_ms, profile, portfolio_id, env=None):
    cfg_path = f"/tmp/glm_op_{os.getpid()}_{portfolio_id}.json"
    with open(cfg_path, "w") as f:
        json.dump({"portfolio_config": config}, f)
    cmd = [
        REPLAY, "--config", cfg_path,
        "--budget", str(budget),
        "--start-ms", str(start_ms), "--end-ms", str(end_ms),
        "--market-data", MARKET_DB, "--funding-data", FUNDING_DB,
        "--profile", profile, "--portfolio-id", portfolio_id,
        "--exchange-min-notional", "5",
    ]
    e = dict(os.environ)
    if env:
        e.update(env)
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=900, env=e)
    except subprocess.TimeoutExpired:
        return {"error": "timeout"}
    finally:
        try:
            os.remove(cfg_path)
        except OSError:
            pass
    if proc.returncode != 0:
        return {"error": proc.stderr.strip()[:500]}
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError:
        return {"error": proc.stdout[:500]}


def metrics(result):
    if "error" in result or "on_budget" not in result:
        return None
    ob = result["on_budget"]
    return {
        "ann": ob.get("annualized_return_pct"),
        "dd": ob.get("max_drawdown_pct"),
        "ret": ob.get("total_return_pct"),
        "min_eq": ob.get("min_equity_quote"),
        "breached": ob.get("principal_breached"),
        "trades": result.get("trade_count"),
        "max_cap": result.get("on_max_capital_used", {}).get("max_capital_used_quote"),
    }


def mk_strategy(sid, symbol, direction, gate_exprs, params, weight):
    triggers = [{"cooldown": {"seconds": params["cooldown"]}}]
    for expr in gate_exprs:
        triggers.append({"indicator_expression": {"expression": expr}})
    return {
        "strategy_id": sid, "symbol": symbol, "market": "usd_m_futures",
        "direction": direction, "direction_mode": "long_and_short",
        "margin_mode": "isolated", "leverage": params["leverage"],
        "spacing": {"fixed_percent": {"step_bps": params["step_bps"]}},
        "sizing": {"multiplier": {
            "first_order_quote": str(params["first_q"]),
            "multiplier": str(params["multiplier"]),
            "max_legs": params["max_legs"]}},
        "take_profit": {"percent": {"bps": params["tp_bps"]}},
        "stop_loss": {"strategy_drawdown_pct": {"pct_bps": params["sl_bps"]}},
        "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}],
        "entry_triggers": triggers,
        "risk_limits": {
            "max_active_cycles": None, "max_global_budget_quote": None,
            "max_symbol_budget_quote": None, "max_direction_budget_quote": None,
            "max_strategy_budget_quote": None, "max_global_drawdown_quote": None},
        "portfolio_weight_pct": str(weight),
    }


def exprs(t, s):
    return [x.format(S=s) for x in t]


# Long gate variants — KEY OPTIMIZATION: tighter per-symbol uptrend requirement
LONG_GATES = {
    # breakthrough gate (loose): close>ema100 + BTC>ema50
    "loose": ["{S}.close > {S}.ema(100)", "BTCUSDT.close > BTCUSDT.ema(50)"],
    # tighter: symbol itself above its ema50 (kills downtrend entries)
    "sym50": ["{S}.close > {S}.ema(50)", "BTCUSDT.close > BTCUSDT.ema(50)"],
    # tight bull: symbol ema stack + BTC
    "bull": ["{S}.close > {S}.ema(50)", "{S}.ema(50) > {S}.ema(200)",
             "BTCUSDT.close > BTCUSDT.ema(50)"],
    # sym50 + adx strength (trend confirmed)
    "sym50adx": ["{S}.close > {S}.ema(50)", "adx(14) > 20",
                 "BTCUSDT.close > BTCUSDT.ema(50)"],
    # range-dip: buy oversold dips only in uptrend
    "dip": ["adx(14) < 28", "rsi(14) < 45", "{S}.close > {S}.ema(100)"],
}
# Short gate variants
SHORT_GATES = {
    "loose": ["{S}.close < {S}.ema(100)", "BTCUSDT.close < BTCUSDT.ema(50)"],
    "sym50": ["{S}.close < {S}.ema(50)", "BTCUSDT.close < BTCUSDT.ema(50)"],
    "crash": ["{S}.close < {S}.ema(50)", "{S}.ema(50) < {S}.ema(200)",
              "BTCUSDT.close < BTCUSDT.ema(50)"],
    "pop": ["adx(14) < 28", "rsi(14) > 55", "{S}.close < {S}.ema(100)"],
    "sym50adx": ["{S}.close < {S}.ema(50)", "adx(14) > 20",
                 "BTCUSDT.close < BTCUSDT.ema(50)"],
}

LONG_BULL = ["BNBUSDT", "TRXUSDT", "BCHUSDT"]
SHORT_CRASH = ["AAVEUSDT", "DOTUSDT", "NEARUSDT", "ARBUSDT", "OPUSDT", "ADAUSDT",
               "FILUSDT", "SOLUSDT", "AVAXUSDT", "INJUSDT", "LINKUSDT", "ATOMUSDT"]

PARAMS = {
    # long params: vary stop-loss tightness (the 2026 fix)
    "long_con": [dict(leverage=5, multiplier=1.4, max_legs=5, step_bps=120,
                      tp_bps=220, cooldown=21600, first_q=55.0, sl_bps=sl)
                 for sl in [1200, 1500, 1800]],
    "long_bal": [dict(leverage=10, multiplier=1.8, max_legs=6, step_bps=120,
                      tp_bps=320, cooldown=14400, first_q=40.0, sl_bps=sl)
                 for sl in [1800, 2500, 3000]],
    "long_agg": [dict(leverage=10, multiplier=2.5, max_legs=8, step_bps=180,
                      tp_bps=420, cooldown=10800, first_q=30.0, sl_bps=sl)
                 for sl in [2500, 3500, 4500]],
    # short params: tighter (crash shorts need to survive 2024 bull)
    "short_con": [dict(leverage=5, multiplier=1.3, max_legs=4, step_bps=150,
                       tp_bps=300, cooldown=21600, first_q=50.0, sl_bps=sl)
                  for sl in [1000, 1500]],
    "short_bal": [dict(leverage=10, multiplier=1.6, max_legs=5, step_bps=150,
                       tp_bps=350, cooldown=14400, first_q=35.0, sl_bps=sl)
                  for sl in [1500, 2200]],
    "short_agg": [dict(leverage=10, multiplier=2.0, max_legs=6, step_bps=180,
                       tp_bps=400, cooldown=10800, first_q=28.0, sl_bps=sl)
                  for sl in [2000, 3000]],
}

# Portfolio DD pause env options (research-only, Task 4 probe)
DD_PAUSE_ENVS = {
    "none": {},
    "ddpause8": {"MARTINGALE_BT_PORTFOLIO_EQUITY_STOP_PCT": "8",
                 "MARTINGALE_BT_PORTFOLIO_STOP_COOLDOWN_HOURS": "24"},
    "ddpause12": {"MARTINGALE_BT_PORTFOLIO_EQUITY_STOP_PCT": "12",
                  "MARTINGALE_BT_PORTFOLIO_STOP_COOLDOWN_HOURS": "12"},
}


def build(longs, shorts, lg_name, sg_name, profile, lw, sw, sl_env):
    lg = LONG_GATES[lg_name]
    sg = SHORT_GATES[sg_name]
    lp = PARAMS["long_" + profile[:3]]
    sp = PARAMS["short_" + profile[:3]]
    strategies = []
    for i, s in enumerate(longs):
        strategies.append(mk_strategy(f"op-L{i}-{s}", s, "long", exprs(lg, s),
                                      lp[i % len(lp)], lw))
    for i, s in enumerate(shorts):
        strategies.append(mk_strategy(f"op-S{i}-{s}", s, "short", exprs(sg, s),
                                      sp[i % len(sp)], sw))
    return {"direction_mode": "long_and_short", "strategies": strategies,
            "risk_limits": {"max_global_budget_quote": "5000"}}


def gen_candidates():
    """Generate focused candidate set around the breakthrough structure."""
    cands = []
    long_sets = [
        ["BNBUSDT"],
        ["BNBUSDT", "BCHUSDT"],
        ["BNBUSDT", "TRXUSDT"],
        ["TRXUSDT"],
    ]
    short_sets = [
        ["AAVEUSDT"],
        ["AAVEUSDT", "DOTUSDT"],
        ["SOLUSDT", "AVAXUSDT"],
        ["ARBUSDT", "OPUSDT"],
        ["AAVEUSDT", "SOLUSDT", "DOTUSDT"],
    ]
    profiles = ["conservative", "balanced", "aggressive"]
    weight_sets = [
        (40, 15), (35, 18), (30, 20), (50, 10), (45, 12),
    ]
    for profile in profiles:
        for ls in long_sets:
            for ss in short_sets:
                for lg in ["loose", "sym50", "bull", "dip"]:
                    for sg in ["loose", "sym50", "crash"]:
                        for lw, sw in weight_sets:
                            # only test dd-pause on the loose gate (most trades)
                            sl_envs = ["none", "ddpause8"] if lg == "loose" else ["none"]
                            for sl_env in sl_envs:
                                cfg = build(ls, ss, lg, sg, profile, lw, sw, sl_env)
                                label = (f"{profile[:3]}-L{lg[:4]}S{sg[:4]}-"
                                         f"{len(ls)}{len(ss)}-{lw}w{sw}-{sl_env}")
                                cands.append((cfg, profile, label, sl_env))
    return cands


def eval_two_seg(args):
    cfg, profile, label, sl_env = args
    env = DD_PAUSE_ENVS[sl_env]
    m24 = metrics(run_replay(cfg, 5000, FULL_SEGMENTS[2][1], FULL_SEGMENTS[2][2],
                             profile, label[:12] + "24", env))
    m25 = metrics(run_replay(cfg, 5000, FULL_SEGMENTS[3][1], FULL_SEGMENTS[3][2],
                             profile, label[:12] + "25", env))
    survives = bool(m24 and m25 and m24["ret"] and m25["ret"]
                    and m24["ret"] > 0 and m25["ret"] > 0
                    and not m24.get("breached") and not m25.get("breached"))
    # also relax: allow if combined 24+25 > 0 AND neither < -3 (near-miss)
    combined = ((m24["ret"] or 0) + (m25["ret"] or 0)) if (m24 and m25) else -999
    near = bool(m24 and m25 and combined > 0
                and (m24["ret"] or 0) > -3 and (m25["ret"] or 0) > -3)
    return {"label": label, "profile": profile, "config": cfg, "sl_env": sl_env,
            "m2024": m24, "m2025": m25, "survives": survives, "near": near}


def full_validate(rec):
    cfg, profile, label, sl_env = (rec["config"], rec["profile"],
                                   rec["label"][:12], rec["sl_env"])
    env = DD_PAUSE_ENVS[sl_env]
    seg_m = {}
    for name, s, e in FULL_SEGMENTS:
        seg_m[name] = metrics(run_replay(cfg, 5000, s, e, profile, label + name, env))
    full_m = metrics(run_replay(cfg, 5000, FULL_START, FULL_END, profile, label + "fl", env))
    rec["segment_metrics"] = seg_m
    rec["full_metrics"] = full_m
    pos = sum(1 for v in seg_m.values() if v and v["ret"] and v["ret"] > 0)
    rets = {n: (v["ret"] if v and v["ret"] else 0.0) for n, v in seg_m.items()}
    agg = sum(rets.get(k, 0) for k in ("2024", "2025", "2026_ytd"))
    tot = sum(rets.values())
    h1c = rets.get("h1_2023", 0) / tot * 100 if tot > 0 else 0
    rec["positive_segments"] = pos
    rec["agg_2024_2026"] = agg
    rec["h1_2023_contribution"] = h1c
    rec["segment_returns"] = rets
    return rec


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=22)
    ap.add_argument("--max-full-validate", type=int, default=80)
    args = ap.parse_args()

    cands = gen_candidates()
    print(f"[glm-op] {len(cands)} candidates x 2 segs, {args.workers} workers",
          flush=True)

    results = []
    t0 = time.time()
    with ProcessPoolExecutor(max_workers=args.works if False else args.workers) as ex:
        futs = {ex.submit(eval_two_seg, c): c for c in cands}
        done = 0
        for fut in as_completed(futs):
            try:
                rec = fut.result()
            except Exception as e:
                rec = {"label": "?", "error": str(e), "survives": False, "near": False,
                       "m2024": None, "m2025": None}
            results.append(rec)
            done += 1
            m24, m25 = rec.get("m2024") or {}, rec.get("m2025") or {}
            tag = "SURVIVE" if rec.get("survives") else ("NEAR" if rec.get("near") else "")
            if done % 80 == 0 or rec.get("survives") or rec.get("near"):
                print(f"  [{done}/{len(cands)}] {rec.get('label','?'):30s} "
                      f"24={m24.get('ret')} 25={m25.get('ret')} {tag}", flush=True)

    survivors = [r for r in results if r.get("survives")]
    nears = [r for r in results if r.get("near") and not r.get("survives")]
    print(f"\n[glm-op] survivors {len(survivors)}, near {len(nears)} "
          f"in {time.time()-t0:.0f}s", flush=True)

    # full-validate survivors first, then top nears
    pool = survivors + nears
    pool.sort(key=lambda r: ((r["m2024"]["ret"] or 0) + (r["m2025"]["ret"] or 0)),
              reverse=True)
    to_val = pool[:args.max_full_validate]
    print(f"[glm-op] full-validating {len(to_val)}", flush=True)

    validated = []
    for i, rec in enumerate(to_val):
        try:
            v = full_validate(rec)
            validated.append(v)
            fm = v.get("full_metrics") or {}
            print(f"  [FV {i+1}/{len(to_val)}] {v['label']:30s} "
                  f"ann={fm.get('ann')} dd={fm.get('dd')} pos={v.get('positive_segments')}/5 "
                  f"agg2426={v.get('agg_2024_2026'):.1f}", flush=True)
        except Exception as e:
            print(f"  [FV ERR] {rec.get('label')} {e}", flush=True)

    GATES = {"conservative": (50.0, 10.0, 4),
             "balanced": (90.0, 20.0, 4),
             "aggressive": (110.0, 30.0, 3)}
    for v in validated:
        g = GATES[v["profile"]]
        fm = v.get("full_metrics") or {}
        ann, dd = fm.get("ann") or 0, fm.get("dd") or 999
        v["gate_pass"] = (ann > g[0] and dd <= g[1] and v["positive_segments"] >= g[2]
                          and v["agg_2024_2026"] > 0 and v["h1_2023_contribution"] < 60
                          and not fm.get("breached"))
    # rank by (pos_segs, then segment-score from the plan)
    def seg_score(v):
        rets = v.get("segment_returns", {})
        anns = [m["ann"] if (m := v["segment_metrics"][n]) else 0 for n,_ ,_ in FULL_SEGMENTS]
        mean = sum(anns)/len(anns) if anns else 0
        std = (sum((a-mean)**2 for a in anns)/len(anns))**0.5 if anns else 0
        fm = v.get("full_metrics") or {}
        return (v["positive_segments"], (fm.get("ann") or 0) - 0.5*std
                + 0.4*v["agg_2024_2026"] - 2.0*max(0,(fm.get("dd") or 0)-GATES[v["profile"]][1]))
    validated.sort(key=seg_score, reverse=True)

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w") as f:
        json.dump({
            "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "n_candidates": len(cands), "n_survivors": len(survivors),
            "n_near": len(nears), "n_validated": len(validated),
            "validated": validated,
            "passes": [v for v in validated if v.get("gate_pass")],
        }, f, indent=2, default=str)
    print(f"\n[glm-op] wrote {args.out}: {len(validated)} validated, "
          f"{len([v for v in validated if v.get('gate_pass')])} passes", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
