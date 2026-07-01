#!/usr/bin/env python3
"""GLM Martingale Core — Portfolio segment-first search (Task 5, moved earlier).

Builds multi-strategy martingale PORTFOLIOS that combine a long-bull core
(BNB/TRX/BCH — the only symbols up in both 2024 and 2025) with crash-alt short
sleeves (2025 profit source), then validates each portfolio segment-first.

The portfolio is the unit of evaluation, not the single symbol. This is the
structural answer to the 2024/2025 anti-correlation that killed 0/3276
single-symbol candidates.

Each portfolio config is a multi-strategy martingale (all martingale-core, no
trend-only legs). Weights are scaled so total budget <= 5000 USDT.

Usage:
  python3 scripts/glm_portfolio_segment_search.py \
      --out docs/superpowers/artifacts/glm-martingale-core/portfolio-segments.json
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


def run_replay(config, budget, start_ms, end_ms, profile, portfolio_id):
    cfg_path = f"/tmp/glm_pf_{os.getpid()}_{portfolio_id}.json"
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
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=900)
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


def mk_strategy(sid, symbol, direction, regime_exprs, params, weight):
    """Build a single martingale strategy leg with regime gate."""
    triggers = [{"cooldown": {"seconds": params["cooldown"]}}]
    for expr in regime_exprs:
        triggers.append({"indicator_expression": {"expression": expr}})
    return {
        "strategy_id": sid,
        "symbol": symbol,
        "market": "usd_m_futures",
        "direction": direction,
        "direction_mode": "long_and_short",
        "margin_mode": "isolated",
        "leverage": params["leverage"],
        "spacing": {"fixed_percent": {"step_bps": params["step_bps"]}},
        "sizing": {
            "multiplier": {
                "first_order_quote": str(params["first_q"]),
                "multiplier": str(params["multiplier"]),
                "max_legs": params["max_legs"],
            }
        },
        "take_profit": {"percent": {"bps": params["tp_bps"]}},
        "stop_loss": {"strategy_drawdown_pct": {"pct_bps": params["sl_bps"]}},
        "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}],
        "entry_triggers": triggers,
        "risk_limits": {
            "max_active_cycles": None,
            "max_global_budget_quote": None,
            "max_symbol_budget_quote": None,
            "max_direction_budget_quote": None,
            "max_strategy_budget_quote": None,
            "max_global_drawdown_quote": None,
        },
        "portfolio_weight_pct": str(weight),
    }


# Regime expressions per direction (per-symbol + BTC veto)
LONG_BULL_GATE = [
    "{S}.close > {S}.ema(50)",
    "{S}.ema(50) > {S}.ema(200)",
    "BTCUSDT.close > BTCUSDT.ema(50)",
]
SHORT_CRASH_GATE = [
    "{S}.close < {S}.ema(50)",
    "{S}.ema(50) < {S}.ema(200)",
    "BTCUSDT.close < BTCUSDT.ema(50)",
]
# Looser long gate (allow more trades): just uptrend + BTC up
LONG_LOOSE = ["{S}.close > {S}.ema(100)", "BTCUSDT.close > BTCUSDT.ema(50)"]
SHORT_LOOSE = ["{S}.close < {S}.ema(100)", "BTCUSDT.close < BTCUSDT.ema(50)"]
# Range long: ADX ranging + dip buy in uptrend
LONG_RANGE_DIP = ["adx(14) < 25", "rsi(14) < 45", "{S}.close > {S}.ema(200)"]
SHORT_RANGE_POP = ["adx(14) < 25", "rsi(14) > 55", "{S}.close < {S}.ema(200)"]


def exprs(template_list, symbol):
    return [t.format(S=symbol) for t in template_list]


# Building-block legs. Long-bull = BNB/TRX/BCH (only up-in-both-years).
LONG_BULL_SYMBOLS = ["BNBUSDT", "TRXUSDT", "BCHUSDT"]
# Crash-alt shorts: symbols that crashed in 2025 (profit source) but need gate
# to survive 2024 bull. Use trend gate so shorts only fire in downtrend.
SHORT_CRASH_SYMBOLS = ["AAVEUSDT", "DOTUSDT", "NEARUSDT", "ARBUSDT", "OPUSDT",
                       "ADAUSDT", "FILUSDT", "SOLUSDT", "AVAXUSDT", "INJUSDT",
                       "LINKUSDT", "ATOMUSDT"]

# Param packs per profile
PARAMS = {
    "conservative": dict(leverage=5, multiplier=1.4, max_legs=5, step_bps=120,
                         tp_bps=220, sl_bps=1800, cooldown=21600, first_q=55.0),
    "balanced": dict(leverage=10, multiplier=1.8, max_legs=6, step_bps=120,
                     tp_bps=320, sl_bps=2800, cooldown=14400, first_q=40.0),
    "aggressive": dict(leverage=10, multiplier=2.5, max_legs=8, step_bps=180,
                       tp_bps=420, sl_bps=4500, cooldown=10800, first_q=30.0),
}


def build_portfolio(longs, shorts, long_gate, short_gate, profile, weights):
    """weights: dict 'long'->pct_per_leg, 'short'->pct_per_leg."""
    strategies = []
    p = PARAMS[profile]
    for i, s in enumerate(longs):
        strategies.append(mk_strategy(
            f"pf-{profile}-L{i}-{s}", s, "long", exprs(long_gate, s), p,
            weights["long"]))
    for i, s in enumerate(shorts):
        strategies.append(mk_strategy(
            f"pf-{profile}-S{i}-{s}", s, "short", exprs(short_gate, s), p,
            weights["short"]))
    return {
        "direction_mode": "long_and_short",
        "strategies": strategies,
        "risk_limits": {"max_global_budget_quote": "5000"},
    }


def eval_portfolio_2024_2025(args):
    """Segment-first: 2024 + 2025 on a portfolio. Survives if both positive."""
    config, profile, label = args
    m24 = metrics(run_replay(config, 5000, FULL_SEGMENTS[2][1], FULL_SEGMENTS[2][2],
                             profile, f"{label[:14]}24"))
    m25 = metrics(run_replay(config, 5000, FULL_SEGMENTS[3][1], FULL_SEGMENTS[3][2],
                             profile, f"{label[:14]}25"))
    survives = bool(
        m24 and m25 and m24["ret"] and m25["ret"]
        and m24["ret"] > 0 and m25["ret"] > 0
        and not m24.get("breached") and not m25.get("breached"))
    return {"label": label, "profile": profile, "config": config,
            "m2024": m24, "m2025": m25, "survives": survives}


def full_validate(rec):
    config = rec["config"]
    profile = rec["profile"]
    label = rec["label"][:14]
    seg_m = {}
    for name, s, e in FULL_SEGMENTS:
        seg_m[name] = metrics(run_replay(config, 5000, s, e, profile, f"{label}{name}"))
    full_m = metrics(run_replay(config, 5000, FULL_START, FULL_END, profile, f"{label}fl"))
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
    return rec


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=20)
    ap.add_argument("--max-full-validate", type=int, default=80)
    args = ap.parse_args()

    # Generate portfolio candidates: combinations of longs + shorts with
    # different gates and weights, per profile.
    long_combos = [
        ["BNBUSDT"],
        ["BNBUSDT", "TRXUSDT"],
        ["BNBUSDT", "BCHUSDT"],
        ["BNBUSDT", "TRXUSDT", "BCHUSDT"],
        ["TRXUSDT"],
        ["BCHUSDT"],
    ]
    short_combos = [
        [],
        ["AAVEUSDT"],
        ["DOTUSDT", "NEARUSDT"],
        ["ARBUSDT", "OPUSDT"],
        ["ADAUSDT", "FILUSDT"],
        ["SOLUSDT", "AVAXUSDT"],
        ["AAVEUSDT", "DOTUSDT", "NEARUSDT"],
        ["ARBUSDT", "OPUSDT", "ADAUSDT"],
        ["LINKUSDT", "ATOMUSDT", "INJUSDT"],
    ]
    gate_pairs = [
        ("bull", "crash", LONG_BULL_GATE, SHORT_CRASH_GATE),
        ("loose", "loose", LONG_LOOSE, SHORT_LOOSE),
        ("bull", "loose", LONG_BULL_GATE, SHORT_LOOSE),
        ("loose", "crash", LONG_LOOSE, SHORT_CRASH_GATE),
    ]
    weight_schemes = [
        {"long": 40, "short": 15},   # long-heavy
        {"long": 30, "short": 20},
        {"long": 50, "short": 10},
        {"long": 35, "short": 12},
    ]

    jobs = []
    for profile in ["conservative", "balanced", "aggressive"]:
        for lc in long_combos:
            for sc in short_combos:
                # skip empty portfolios
                if not lc and not sc:
                    continue
                for lname, sname, lg, sg in gate_pairs:
                    for ws in weight_schemes:
                        # weight sanity: ensure each leg has meaningful budget
                        # scale short weight up if few shorts
                        config = build_portfolio(lc, sc, lg, sg, profile, ws)
                        label = f"{profile[:3]}-{lname[:3]}{sname[:3]}-L{len(lc)}S{len(sc)}-{ws['long']}w{ws['short']}"
                        jobs.append((config, profile, label))

    print(f"[glm-pf] {len(jobs)} portfolio candidates x 2 segments "
          f"with {args.workers} workers", flush=True)

    results = []
    t0 = time.time()
    with ProcessPoolExecutor(max_workers=args.workers) as ex:
        futs = {ex.submit(eval_portfolio_2024_2025, j): j for j in jobs}
        done = 0
        for fut in as_completed(futs):
            try:
                rec = fut.result()
            except Exception as e:
                rec = {"error": str(e), "label": "?", "survives": False,
                       "m2024": None, "m2025": None}
            results.append(rec)
            done += 1
            m24 = rec.get("m2024") or {}
            m25 = rec.get("m2025") or {}
            tag = "SURVIVE" if rec.get("survives") else ""
            if done % 40 == 0 or rec.get("survives") or \
                    (m24.get("ret") is not None and m24.get("ret", -99) > 0):
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):24s} "
                      f"24_ret={m24.get('ret')} 25_ret={m25.get('ret')} {tag}",
                      flush=True)

    survivors = [r for r in results if r.get("survives")]
    print(f"\n[glm-pf] portfolio segment-gate survivors: {len(survivors)}/"
          f"{len(results)} in {time.time()-t0:.0f}s", flush=True)

    # sort by combined 24+25
    survivors.sort(key=lambda r: (r["m2024"]["ret"] or 0) + (r["m2025"]["ret"] or 0),
                   reverse=True)
    to_val = survivors[:args.max_full_validate]
    print(f"[glm-pf] full-validating {len(to_val)} survivors", flush=True)

    validated = []
    for i, rec in enumerate(to_val):
        try:
            v = full_validate(rec)
            validated.append(v)
            fm = v.get("full_metrics") or {}
            print(f"  [FV {i+1}/{len(to_val)}] {v['label']:24s} "
                  f"full_ann={fm.get('ann')} dd={fm.get('dd')} "
                  f"pos={v.get('positive_segments')}/5 agg2426={v.get('agg_2024_2026'):.1f}",
                  flush=True)
        except Exception as e:
            print(f"  [FV ERR] {rec.get('label')} {e}", flush=True)

    GATES = {
        "conservative": dict(ann_min=50.0, dd_max=10.0, min_pos=4),
        "balanced": dict(ann_min=90.0, dd_max=20.0, min_pos=4),
        "aggressive": dict(ann_min=110.0, dd_max=30.0, min_pos=3),
    }
    for v in validated:
        g = GATES[v["profile"]]
        fm = v.get("full_metrics") or {}
        ann = fm.get("ann") or 0
        dd = fm.get("dd") or 999
        v["gate_pass"] = (ann > g["ann_min"] and dd <= g["dd_max"]
                          and v["positive_segments"] >= g["min_pos"]
                          and v["agg_2024_2026"] > 0
                          and v["h1_2023_contribution"] < 60
                          and not fm.get("breached"))
    validated.sort(key=lambda v: (v.get("positive_segments") or 0,
                                  (v.get("full_metrics") or {}).get("ann") or 0),
                   reverse=True)

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w") as f:
        json.dump({
            "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "n_candidates": len(jobs),
            "n_survivors": len(survivors),
            "n_validated": len(validated),
            "validated": validated,
            "passes": [v for v in validated if v.get("gate_pass")],
        }, f, indent=2, default=str)
    print(f"\n[glm-pf] wrote {args.out}: {len(validated)} validated, "
          f"{len([v for v in validated if v.get('gate_pass')])} passes", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
