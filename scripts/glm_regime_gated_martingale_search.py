#!/usr/bin/env python3
"""GLM Martingale Core — Regime-Gated Martingale single-strategy segment-first search.

Plan Task 2. The CORE innovation vs all prior work: PER-SYMBOL regime gating
(prior searches only used BTC regime). Per external research consensus:
  - ADX < 25 = ranging  -> allow martingale averaging (mean reversion works)
  - ADX > 25 = trending -> disable counter-trend new cycles
  - per-asset EMA trend gate (price vs ema, ema stack)
  - RSI/BB momentum confirms entry timing
  - BTC macro veto overlay (cross-symbol refs)

All of these are expressible in the live-parity expression language, so any
candidate produced here is live-reproducible (no research-only mechanisms).

This script generates single-symbol martingale configs and runs them through a
SEGMENT-FIRST pipeline:
  1. For each (symbol, direction, regime-family, params), run the 2024+2025
     segments (the two hardest anti-overfit segments) first.
  2. Only candidates positive on BOTH 2024 and 2025 advance to full 5-segment
     validation (the slow part). This is the segment-first filter that prior
     work lacked.

Segment-first is the whole point: prior work sorted by full-period ann then
validated segments late, repeatedly selecting 2023H1-dependent rows. Here we
gate on the two segments that 0/590 prior configs could make both-positive.

Usage:
  python3 scripts/glm_regime_gated_martingale_search.py \
      --symbols SOLUSDT,BTCUSDT,ETHUSDT \
      --directions long,short \
      --out docs/superpowers/artifacts/glm-martingale-core/regime-gated-single-strategy.json
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

# Segment windows (start, end_exclusive_minus_1)
SEG_2024 = (1704067200000, 1735689599999)
SEG_2025 = (1735689600000, 1767225599999)
FULL_SEGMENTS = [
    ("h1_2023", 1672531200000, 1688169599999),
    ("h2_2023", 1688169600000, 1704067199999),
    ("2024", 1704067200000, 1735689599999),
    ("2025", 1735689600000, 1767225599999),
    ("2026_ytd", 1767225600000, 1780271999999),
]
FULL_START, FULL_END = 1672531200000, 1780271999999


def run_replay(config, budget, start_ms, end_ms, profile, portfolio_id):
    """Run one replay with an inline config dict."""
    cfg_path = f"/tmp/glm_rg_cfg_{os.getpid()}_{portfolio_id}.json"
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


# ---------------------------------------------------------------------------
# Regime families. Each is a set of entry_triggers to ADD to a base martingale
# config. The base martingale behavior (DCA ladder, TP, SL) is unchanged — the
# regime expressions only GATE when new cycles open. This keeps it martingale-
# core with indicators as the plan mandates.
# ---------------------------------------------------------------------------

def regime_family(name, symbol):
    """Return list of entry_trigger dicts for a regime family.

    Uses per-symbol indicators (the innovation) + optional BTC macro veto.
    All expressions use the live-parity indicator language.
    """
    S = symbol  # per-symbol prefix optional (default = strategy symbol)
    btc_down = "BTCUSDT.close < BTCUSDT.ema(50)"
    btc_up = "BTCUSDT.close > BTCUSDT.ema(50)"
    if name == "none":
        return []
    if name == "ema_trend_long":
        # long only when per-symbol uptrend (price>ema50>ema200-ish via two checks)
        return [
            {"indicator_expression": {"expression": f"{S}.close > {S}.ema(50)"}},
            {"indicator_expression": {"expression": f"{S}.ema(50) > {S}.ema(200)"}},
            {"indicator_expression": {"expression": btc_up}},
        ]
    if name == "ema_trend_short":
        return [
            {"indicator_expression": {"expression": f"{S}.close < {S}.ema(50)"}},
            {"indicator_expression": {"expression": f"{S}.ema(50) < {S}.ema(200)"}},
            {"indicator_expression": {"expression": btc_down}},
        ]
    if name == "adx_range_long":
        # ranging market + per-symbol uptrend bias + BTC not crashing
        return [
            {"indicator_expression": {"expression": f"adx(14) < 25"}},
            {"indicator_expression": {"expression": f"{S}.close > {S}.ema(100)"}},
            {"indicator_expression": {"expression": btc_up}},
        ]
    if name == "adx_range_short":
        return [
            {"indicator_expression": {"expression": f"adx(14) < 25"}},
            {"indicator_expression": {"expression": f"{S}.close < {S}.ema(100)"}},
            {"indicator_expression": {"expression": btc_down}},
        ]
    if name == "adx_rsi_long":
        # ranging + oversold dip-buy in uptrend
        return [
            {"indicator_expression": {"expression": f"adx(14) < 28"}},
            {"indicator_expression": {"expression": f"rsi(14) < 45"}},
            {"indicator_expression": {"expression": f"{S}.close > {S}.ema(100)"}},
        ]
    if name == "adx_rsi_short":
        return [
            {"indicator_expression": {"expression": f"adx(14) < 28"}},
            {"indicator_expression": {"expression": f"rsi(14) > 55"}},
            {"indicator_expression": {"expression": f"{S}.close < {S}.ema(100)"}},
        ]
    if name == "bb_extreme_long":
        return [
            {"indicator_expression": {"expression": f"bb_lower(20, 2) > {S}.close"}},
            {"indicator_expression": {"expression": f"{S}.close > {S}.ema(200)"}},
        ]
    if name == "bb_extreme_short":
        return [
            {"indicator_expression": {"expression": f"bb_upper(20, 2) < {S}.close"}},
            {"indicator_expression": {"expression": f"{S}.close < {S}.ema(200)"}},
        ]
    if name == "atr_vol_long":
        # only enter when volatility is moderate (not panic) + uptrend
        return [
            {"indicator_expression": {"expression": f"atr_percent(14) < 5"}},
            {"indicator_expression": {"expression": f"{S}.close > {S}.ema(50)"}},
        ]
    if name == "atr_vol_short":
        return [
            {"indicator_expression": {"expression": f"atr_percent(14) < 5"}},
            {"indicator_expression": {"expression": f"{S}.close < {S}.ema(50)"}},
        ]
    if name == "donchian_ema_long":
        # breakout-style: above ema + adx confirms trend strength (ride the trend w/ martingale)
        return [
            {"indicator_expression": {"expression": f"{S}.close > {S}.ema(50)"}},
            {"indicator_expression": {"expression": f"adx(14) > 20"}},
            {"indicator_expression": {"expression": btc_up}},
        ]
    if name == "donchian_ema_short":
        return [
            {"indicator_expression": {"expression": f"{S}.close < {S}.ema(50)"}},
            {"indicator_expression": {"expression": f"adx(14) > 20"}},
            {"indicator_expression": {"expression": btc_down}},
        ]
    return []


# Conservative / balanced / aggressive martingale param grids.
# These are deliberately SMALL and use consensus values (not overfit magic
# numbers). The regime gate is the variable under test.
PARAM_GRIDS = {
    "conservative": [
        dict(leverage=5, multiplier=1.3, max_legs=4, step_bps=80, tp_bps=180,
             sl_bps=1500, cooldown=21600, first_q=60.0),
        dict(leverage=5, multiplier=1.5, max_legs=5, step_bps=100, tp_bps=200,
             sl_bps=1800, cooldown=21600, first_q=60.0),
        dict(leverage=7, multiplier=1.4, max_legs=5, step_bps=120, tp_bps=250,
             sl_bps=2000, cooldown=21600, first_q=50.0),
    ],
    "balanced": [
        dict(leverage=10, multiplier=1.6, max_legs=6, step_bps=100, tp_bps=300,
             sl_bps=2500, cooldown=21600, first_q=40.0),
        dict(leverage=10, multiplier=2.0, max_legs=7, step_bps=150, tp_bps=350,
             sl_bps=3000, cooldown=21600, first_q=35.0),
        dict(leverage=10, multiplier=1.8, max_legs=6, step_bps=120, tp_bps=320,
             sl_bps=2800, cooldown=14400, first_q=45.0),
    ],
    "aggressive": [
        dict(leverage=10, multiplier=2.2, max_legs=8, step_bps=150, tp_bps=400,
             sl_bps=4000, cooldown=10800, first_q=30.0),
        dict(leverage=10, multiplier=2.5, max_legs=8, step_bps=200, tp_bps=450,
             sl_bps=5000, cooldown=10800, first_q=28.0),
        dict(leverage=10, multiplier=3.0, max_legs=9, step_bps=180, tp_bps=420,
             sl_bps=4500, cooldown=14400, first_q=25.0),
    ],
}


def build_config(symbol, direction, regime, params, profile):
    """Build a single-strategy portfolio config (martingale core + regime gate)."""
    triggers = [{"cooldown": {"seconds": params["cooldown"]}}]
    triggers += regime_family(regime, symbol)
    strat = {
        "strategy_id": f"rg-{symbol}-{direction}-{regime}-{profile}",
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
        "portfolio_weight_pct": "100",
    }
    return {
        "direction_mode": "long_and_short",
        "strategies": [strat],
        "risk_limits": {"max_global_budget_quote": "5000"},
    }


REGIME_FAMILIES = [
    "none",
    "ema_trend_long", "ema_trend_short",
    "adx_range_long", "adx_range_short",
    "adx_rsi_long", "adx_rsi_short",
    "bb_extreme_long", "bb_extreme_short",
    "atr_vol_long", "atr_vol_short",
    "donchian_ema_long", "donchian_ema_short",
]

DIRECTION_BY_REGIME = {
    "none": ["long", "short"],
    "ema_trend_long": ["long"],
    "ema_trend_short": ["short"],
    "adx_range_long": ["long"],
    "adx_range_short": ["short"],
    "adx_rsi_long": ["long"],
    "adx_rsi_short": ["short"],
    "bb_extreme_long": ["long"],
    "bb_extreme_short": ["short"],
    "atr_vol_long": ["long"],
    "atr_vol_short": ["short"],
    "donchian_ema_long": ["long"],
    "donchian_ema_short": ["short"],
}


def eval_candidate(args):
    """Worker: run 2024 + 2025 segments for one candidate. Returns dict."""
    symbol, direction, regime, profile, params = args
    config = build_config(symbol, direction, regime, params, profile)
    budget = 5000
    pid = f"{symbol[:4]}{direction[0]}{regime[:4]}{profile[0]}".replace(".", "")
    m2024 = metrics(run_replay(config, budget, SEG_2024[0], SEG_2024[1],
                               profile, f"{pid}_24"))
    m2025 = metrics(run_replay(config, budget, SEG_2025[0], SEG_2025[1],
                               profile, f"{pid}_25"))
    rec = {
        "symbol": symbol, "direction": direction, "regime": regime,
        "profile": profile, "params": params,
        "m2024": m2024, "m2025": m2025,
        "config": config,
    }
    # segment-first gate: both 2024 and 2025 positive -> survives
    rec["survives_seg_gate"] = bool(
        m2024 and m2025 and m2024["ret"] and m2025["ret"]
        and m2024["ret"] > 0 and m2025["ret"] > 0
        and not m2024.get("breached") and not m2025.get("breached")
    )
    return rec


def full_validate(candidate, profile):
    """Run the remaining 3 segments + full period for a survivor."""
    config = candidate["config"]
    budget = 5000
    pid = f"fv{candidate['symbol'][:4]}{candidate['direction'][0]}{candidate['regime'][:4]}"
    seg_m = {}
    for name, s, e in FULL_SEGMENTS:
        seg_m[name] = metrics(run_replay(config, budget, s, e, profile, f"{pid}_{name}"))
    full_m = metrics(run_replay(config, budget, FULL_START, FULL_END, profile, f"{pid}_full"))
    candidate["segment_metrics"] = seg_m
    candidate["full_metrics"] = full_m
    pos_segs = sum(1 for v in seg_m.values()
                   if v and v["ret"] and v["ret"] > 0)
    seg_rets = {n: (v["ret"] if v and v["ret"] else 0.0) for n, v in seg_m.items()}
    agg2426 = sum(seg_rets.get(k, 0) for k in ("2024", "2025", "2026_ytd"))
    h1c = (seg_rets.get("h1_2023", 0) / (sum(seg_rets.values()) or 1) * 100.0
           if sum(seg_rets.values()) > 0 else 0.0)
    candidate["positive_segments"] = pos_segs
    candidate["agg_2024_2026"] = agg2426
    candidate["h1_2023_contribution"] = h1c
    return candidate


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--symbols", required=True)
    ap.add_argument("--directions", default="long,short")
    ap.add_argument("--profiles", default="conservative,balanced,aggressive")
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=12)
    ap.add_argument("--max-full-validate", type=int, default=120,
                    help="cap on how many survivors get full 5-seg validation")
    args = ap.parse_args()

    symbols = [s.strip() for s in args.symbols.split(",") if s.strip()]
    profiles = [p.strip() for p in args.profiles.split(",") if p.strip()]

    # Build candidate list: symbol x regime(auto-direction) x profile x paramgrid
    jobs = []
    for symbol in symbols:
        for regime in REGIME_FAMILIES:
            dirs = DIRECTION_BY_REGIME[regime]
            for profile in profiles:
                for params in PARAM_GRIDS[profile]:
                    for d in dirs:
                        jobs.append((symbol, d, regime, profile, params))
    print(f"[glm] {len(jobs)} candidates x 2 segments (2024,2025) "
          f"with {args.workers} workers", flush=True)

    results = []
    t0 = time.time()
    with ProcessPoolExecutor(max_workers=args.workers) as ex:
        futs = {ex.submit(eval_candidate, j): j for j in jobs}
        done = 0
        for fut in as_completed(futs):
            try:
                rec = fut.result()
            except Exception as e:
                rec = {"error": str(e), "job": futs[fut]}
            results.append(rec)
            done += 1
            if done % 20 == 0 or (rec.get("survives_seg_gate")):
                m24 = rec.get("m2024") or {}
                m25 = rec.get("m2025") or {}
                tag = "SURVIVE" if rec.get("survives_seg_gate") else ""
                print(f"  [{done}/{len(jobs)}] {rec.get('symbol','?'):12s} "
                      f"{rec.get('direction','?'):5s} {rec.get('regime','?'):18s} "
                      f"{rec.get('profile','?'):12s} 24={m24.get('ret')} "
                      f"25={m25.get('ret')} {tag}", flush=True)

    survivors = [r for r in results if r.get("survives_seg_gate")]
    print(f"\n[glm] segment-gate survivors (2024>0 AND 2025>0): {len(survivors)} "
          f"/ {len(results)} in {time.time()-t0:.0f}s", flush=True)

    # Sort survivors by combined 2024+2025 return, then full-validate the top N
    survivors.sort(
        key=lambda r: (r["m2024"]["ret"] or 0) + (r["m2025"]["ret"] or 0),
        reverse=True,
    )
    to_validate = survivors[:args.max_full_validate]
    print(f"[glm] full-validating top {len(to_validate)} survivors (5 segments + full)",
          flush=True)

    validated = []
    for i, cand in enumerate(to_validate):
        try:
            v = full_validate(cand, cand["profile"])
            validated.append(v)
            fm = v.get("full_metrics") or {}
            print(f"  [FV {i+1}/{len(to_validate)}] {v['symbol']:12s} "
                  f"{v['direction']:5s} {v['regime']:18s} {v['profile']:12s} "
                  f"full_ann={fm.get('ann')} dd={fm.get('dd')} "
                  f"pos_segs={v.get('positive_segments')}/5 "
                  f"agg2426={v.get('agg_2024_2026'):.1f}", flush=True)
        except Exception as e:
            print(f"  [FV ERR] {cand.get('symbol')} {e}", flush=True)

    # Score and rank
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

    validated.sort(
        key=lambda v: ((v.get("positive_segments") or 0),
                       (v.get("full_metrics") or {}).get("ann") or 0),
        reverse=True,
    )

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w") as f:
        json.dump({
            "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "n_candidates": len(jobs),
            "n_survivors_seg_gate": len(survivors),
            "n_full_validated": len(validated),
            "validated": validated,
            "passes": [v for v in validated if v.get("gate_pass")],
        }, f, indent=2, default=str)
    print(f"\n[glm] wrote {args.out}: {len(validated)} validated, "
          f"{len([v for v in validated if v.get('gate_pass')])} passes", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
