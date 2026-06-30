#!/usr/bin/env python3
"""Segment-first, large-cap, regime-sleeve martingale portfolio search.

Phase A of the 2026-06-30 GLM execution handoff. Imports helpers from
`native_small_portfolio_search.py` (config schema, run_replay, segment_score,
 make_config) and overrides three things — NO engine/Rust change:

  1. LARGE-CAP only coin universe (BTC/ETH/SOL/BNB/XRP/TRX/ADA/DOGE).
     Research finding (2026-06-30 deep-research): small-caps carry a higher
     *idiosyncratic* (coin-specific) volatility share that a grid cannot
     diversify, and react to BTC with a 1-3 min lag (worse fills). 2025's
     independent altcoin bear (-50..83% on INJ/AAVE/GALA/NEAR) killed prior
     pools; large-caps were range-bound (-6.4%/-10.9%) = grid-friendly.
  2. SEGMENT-FIRST flow: cheap 3-segment anti-overfit screen (2024+2025+
     2026_ytd combined >= --screen-min) -> full replay -> all-5-segment hard
     gate. Avoids ranking by full-period ann (which selected 2023H1 tickets).
  3. REGIME SLEEVE: every strategy gated by BTC regime via entry_triggers
     (longs only when BTC.close>BTC.ema(200), shorts only when below; a
     "range" sleeve additionally requires adx(14)<25 for mean-reversion).
     Pairs long+short per symbol so the portfolio adapts to market state.

cycle-exit stays ON (regime_break_stop + max_cycle_age_hours), live-parity.

Research-only: calls portfolio_budget_replay, touches no DB/live/Binance.
"""
from __future__ import annotations

import argparse
import concurrent.futures as futures
import itertools
import json
import random
import time
from pathlib import Path
from typing import Any

# Reuse the battle-tested base module (sibling). Importing does NOT run its
# main() (guarded by __name__). We monkeypatch two module-globals so the base
# helpers (strategy / make_config) pick up our regime filters + large-cap pool.
import native_small_portfolio_search as ns

LARGE_CAP = [
    "BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT",
    "XRPUSDT", "TRXUSDT", "ADAUSDT", "DOGEUSDT",
]

# Anti-overfit screen = the three POST-H1-2023 segments. A candidate that only
# wins in 2023H1 will fail here; this is the whole point of segment-first.
SCREEN_SEGMENTS = {
    "2024": ns.SEGMENTS["2024"],
    "2025": ns.SEGMENTS["2025"],
    "2026_ytd": ns.SEGMENTS["2026_ytd"],
}


def _regime_filter_expressions(entry_filter: str, direction: str) -> list[str]:
    """Regime-sleeve entry triggers (monkeypatched over ns.filter_expressions).

    Longs enter only in BTC uptrend, shorts only in BTC downtrend, range sleeve
    additionally requires low ADX (mean-reversion friendly). All cross-symbol
    expressions are live-parity (indicator_runtime.rs resolves BTCUSDT.* in both
    backtest + trading-engine)."""
    exprs: list[str] = []
    bull = "BTCUSDT.close > BTCUSDT.ema(200)"
    bear = "BTCUSDT.close < BTCUSDT.ema(200)"
    if entry_filter == "btc_trend":
        exprs.append(bull if direction == "long" else bear)
    elif entry_filter == "btc_trend_rsi":
        exprs.append(bull if direction == "long" else bear)
        exprs.append("rsi(14) < 65" if direction == "long" else "rsi(14) > 35")
    elif entry_filter == "btc_range":
        # mean-reversion sleeve: with-trend AND ranging (low ADX)
        exprs.append(bull if direction == "long" else bear)
        exprs.append("adx(14) < 25")
    return exprs


# Monkeypatch so ns.strategy() (called by ns.make_config) emits regime triggers.
ns.filter_expressions = _regime_filter_expressions


def _large_cap_pool(profile: str) -> list[tuple[str, str]]:
    """Both directions for every large-cap symbol so the regime filter can pick.
    Duplicates are fine: a (BTC,long)+(BTC,short) pair is the regime sleeve."""
    symbols = LARGE_CAP
    return [(s, "long") for s in symbols] + [(s, "short") for s in symbols]


ns.symbol_direction_pool = _large_cap_pool


def base_template_pool(profile: str) -> list[dict[str, Any]]:
    """Parameter grid per profile, regime filters only + cycle-exit ON."""
    if profile == "conservative":
        first_orders = [5.0, 7.5, 10.0, 12.5, 15.0]
        leverages = [3, 4, 5]
        multipliers = [1.20, 1.30, 1.40, 1.55]
        legs = [3, 4, 5]
        steps = [45, 65, 90, 120, 160]
        tps = [24, 35, 50, 70]
        stops = [250, 350, 500, 700]
        ages = [48.0, 96.0, 168.0, None]
        stop_kinds = ["strategy_drawdown", "regime_break"]
    elif profile == "balanced":
        first_orders = [5.0, 7.5, 10.0, 15.0, 20.0, 25.0]
        leverages = [4, 5, 6, 8]
        multipliers = [1.30, 1.45, 1.60, 1.80]
        legs = [4, 5, 6]
        steps = [35, 50, 70, 100, 140]
        tps = [35, 50, 70, 95]
        stops = [400, 600, 850, 1200]
        ages = [48.0, 96.0, 168.0, None]
        stop_kinds = ["strategy_drawdown", "regime_break"]
    else:  # aggressive
        first_orders = [5.0, 10.0, 15.0, 20.0, 30.0, 40.0]
        leverages = [5, 6, 8, 10]
        multipliers = [1.45, 1.70, 2.00, 2.30]
        legs = [4, 5, 6]
        steps = [25, 40, 60, 85, 120]
        tps = [50, 70, 100, 140]
        stops = [600, 900, 1300, 1800]
        ages = [48.0, 96.0, None]
        stop_kinds = ["strategy_drawdown", "regime_break"]

    filters = ["btc_trend", "btc_trend_rsi", "btc_range"]

    templates: list[dict[str, Any]] = []
    for first_order, leverage, multiplier, max_legs, step_bps, tp_bps, stop_bps, age, entry_filter, stop_kind in itertools.product(
        first_orders, leverages, multipliers, legs, steps, tps, stops, ages, filters, stop_kinds
    ):
        margin = ns.planned_margin(first_order, multiplier, max_legs, leverage)
        if margin > 800.0:
            continue
        if first_order < 5.0:
            continue
        templates.append(
            {
                "first_order": first_order,
                "leverage": leverage,
                "multiplier": multiplier,
                "max_legs": max_legs,
                "step_bps": step_bps,
                "tp_bps": tp_bps,
                "stop_bps": stop_bps,
                "max_cycle_age": age,
                "entry_filter": entry_filter,
                "stop_kind": stop_kind,
                "rb_ema": 100 if stop_kind == "regime_break" else None,
                "adx_min": None,
                "planned_margin": margin,
            }
        )
    return templates


def screen_one(job: tuple[int, dict[str, Any], argparse.Namespace]) -> dict[str, Any]:
    """Run the 3 post-H1-2023 segments for one candidate."""
    idx, cfg, args = job
    segs: dict[str, dict[str, Any]] = {}
    for name, (start, end) in SCREEN_SEGMENTS.items():
        segs[name] = ns.run_replay(args, cfg, start, end, args.segment_timeout)
    combined = 0.0
    ok_count = 0
    breached = False
    for res in segs.values():
        if not res.get("ok"):
            continue
        ok_count += 1
        ret = res.get("ret")
        if isinstance(ret, (int, float)):
            combined += float(ret)
        if res.get("principal_breached"):
            breached = True
    return {
        "idx": idx,
        "config": cfg,
        "screen_segments": segs,
        "combined_2024_2026_ret": combined if ok_count else None,
        "screen_ok_count": ok_count,
        "principal_breached": breached,
    }


def screen_pass(args: argparse.Namespace, screen: dict[str, Any]) -> bool:
    if screen["screen_ok_count"] < 3:
        return False
    if screen["principal_breached"]:
        return False
    combined = screen["combined_2024_2026_ret"]
    if combined is None:
        return False
    return combined >= args.screen_min


def full_one(job: tuple[int, dict[str, Any], argparse.Namespace]) -> dict[str, Any]:
    idx, cfg, args = job
    full = ns.run_replay(args, cfg, ns.FULL[0], ns.FULL[1], args.timeout)
    return {"idx": idx, "config": cfg, "full": full}


def validate_all_segments(args: argparse.Namespace, item: dict[str, Any]) -> dict[str, Any]:
    cfg = item["config"]
    segments = {}
    for name, (start, end) in ns.SEGMENTS.items():
        segments[name] = ns.run_replay(args, cfg, start, end, args.segment_timeout)
    return {**item, "segments": segments, "segment_score": ns.segment_score(item["full"], segments)}


def save_report(path: Path, report: dict[str, Any]) -> None:
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(report, ensure_ascii=False, indent=2))
    tmp.replace(path)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=sorted(ns.TARGETS), required=True)
    parser.add_argument("--budget", type=float, default=5000.0)
    parser.add_argument("--count", type=int, default=200)
    parser.add_argument("--seed", type=int, default=20260630)
    parser.add_argument("--jobs", type=int, default=16)
    parser.add_argument("--screen-min", type=float, default=-25.0,
                        help="min combined 2024+2025+2026_ytd return %% to survive screen")
    parser.add_argument("--top-full", type=int, default=40,
                        help="max survivors to full-replay + all-segment validate")
    parser.add_argument("--timeout", type=int, default=900)
    parser.add_argument("--segment-timeout", type=int, default=600)
    parser.add_argument("--out-dir", required=True)
    parser.add_argument("--replay-bin", required=True)
    parser.add_argument("--market-data", required=True)
    parser.add_argument("--funding-data", required=True)
    args = parser.parse_args()
    args.replay_bin = Path(args.replay_bin)
    args.market_data = Path(args.market_data)
    args.funding_data = Path(args.funding_data)

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    rng = random.Random(args.seed)

    templates = base_template_pool(args.profile)
    configs = [ns.make_config(args.profile, args.budget, rng, i, templates) for i in range(args.count)]

    report: dict[str, Any] = {
        "variant": "segment_first_largecap_regime",
        "profile": args.profile,
        "budget": args.budget,
        "seed": args.seed,
        "count": len(configs),
        "jobs": args.jobs,
        "screen_min": args.screen_min,
        "symbols": LARGE_CAP,
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "screen_results": [],
        "full_results": [],
        "segment_validations": [],
        "passes": [],
    }
    report_path = out_dir / f"segfirst_{args.profile}_b{int(args.budget)}_seed{args.seed}.json"
    run_log = out_dir / f"segfirst_{args.profile}_b{int(args.budget)}_seed{args.seed}.log"

    # ---- Phase 1: segment-first screen (3 post-H1-2023 segments) ----
    jobs = [(i, cfg, args) for i, cfg in enumerate(configs)]
    survivors: list[tuple[int, dict[str, Any], dict[str, Any]]] = []
    done = 0
    with run_log.open("a") as log, futures.ProcessPoolExecutor(max_workers=args.jobs) as pool:
        future_map = {pool.submit(screen_one, j): j[0] for j in jobs}
        for fut in futures.as_completed(future_map):
            done += 1
            try:
                screen = fut.result()
            except Exception as exc:  # noqa: BLE001
                screen = {"idx": future_map[fut], "config": None,
                          "screen_segments": {}, "combined_2024_2026_ret": None,
                          "screen_ok_count": 0, "principal_breached": False, "error": repr(exc)}
            report["screen_results"].append({
                "idx": screen["idx"],
                "combined_2024_2026_ret": screen["combined_2024_2026_ret"],
                "screen_ok_count": screen["screen_ok_count"],
                "principal_breached": screen["principal_breached"],
                "error": screen.get("error"),
            })
            if screen.get("config") is not None and screen_pass(args, screen):
                survivors.append((screen["idx"], screen["config"], screen))
            comb = screen.get("combined_2024_2026_ret")
            line = (f"SCREEN {done}/{len(jobs)} idx={screen['idx']} "
                    f"comb24_26={comb if comb is None else round(comb, 2)} "
                    f"ok={screen['screen_ok_count']} breached={screen['principal_breached']} "
                    f"survivors={len(survivors)}\n")
            log.write(line)
            log.flush()
            if done % max(1, min(10, args.jobs)) == 0:
                save_report(report_path, report)
    report["screen_survivors"] = len(survivors)
    save_report(report_path, report)

    # ---- Phase 2: full replay for survivors (rank by segment-robust full ann) ----
    survivors.sort(key=lambda t: (t[2].get("combined_2024_2026_ret") or -9999.0), reverse=True)
    survivors = survivors[: args.top_full]
    sjobs = [(i, cfg, args) for (i, cfg, _s) in survivors]
    with run_log.open("a") as log, futures.ProcessPoolExecutor(max_workers=args.jobs) as pool:
        future_map = {pool.submit(full_one, j): j[0] for j in sjobs}
        for fut in futures.as_completed(future_map):
            try:
                item = fut.result()
            except Exception as exc:  # noqa: BLE001
                item = {"idx": future_map[fut], "config": None, "full": {"ok": False, "error": repr(exc)}}
            report["full_results"].append(item)
            full = item["full"]
            f = full if full.get("ok") else {}
            line = (f"FULL idx={item['idx']} ann={f.get('ann')} dd={f.get('dd')} "
                    f"ret={f.get('ret')} cap={f.get('cap')} blocked={f.get('blocked')}\n")
            log.write(line)
            log.flush()
            save_report(report_path, report)

    # ---- Phase 3: all-5-segment hard gate for full-OK survivors ----
    full_ok = [r for r in report["full_results"] if r["full"].get("ok")]
    for item in full_ok:
        validated = validate_all_segments(args, item)
        report["segment_validations"].append(validated)
        save_report(report_path, report)
    report["passes"] = [
        r for r in report["segment_validations"]
        if ns.candidate_gate(args.profile, r["full"], r.get("segments"))
    ]
    report["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    save_report(report_path, report)
    print(report_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
