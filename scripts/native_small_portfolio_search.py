#!/usr/bin/env python3
"""Native small-capital martingale portfolio search.

This script is intentionally research-only: it generates live-parity portfolio
JSON configs and runs `portfolio_budget_replay` against historical data. It does
not touch databases, live trading, Binance, or app state.
"""
from __future__ import annotations

import argparse
import concurrent.futures as futures
import itertools
import json
import math
import os
import random
import subprocess
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


FULL = (1672531200000, 1780271999999)
SEGMENTS = {
    "h1_2023": (1672531200000, 1688169599999),
    "h2_2023": (1688169600000, 1704067199999),
    "2024": (1704067200000, 1735689599999),
    "2025": (1735689600000, 1767225599999),
    "2026_ytd": (1767225600000, 1780271999999),
}

CORE_SYMBOLS = [
    "BTCUSDT",
    "ETHUSDT",
    "SOLUSDT",
    "BNBUSDT",
    "XRPUSDT",
    "TRXUSDT",
    "ADAUSDT",
    "LTCUSDT",
    "LINKUSDT",
    "AVAXUSDT",
    "DOTUSDT",
    "BCHUSDT",
    "ETCUSDT",
    "APTUSDT",
    "NEARUSDT",
    "COMPUSDT",
    "ICPUSDT",
    "GALAUSDT",
]

SEGMENT_YEAR_FRACTIONS = {
    "h1_2023": 181.0 / 365.0,
    "h2_2023": 184.0 / 365.0,
    "2024": 366.0 / 365.0,
    "2025": 365.0 / 365.0,
    "2026_ytd": 151.0 / 365.0,
}


@dataclass(frozen=True)
class ProfileTarget:
    name: str
    ann: float
    dd: float
    min_budget: float
    max_budget: float


TARGETS = {
    "conservative": ProfileTarget("conservative", 50.0, 10.0, 1000.0, 5000.0),
    "balanced": ProfileTarget("balanced", 90.0, 20.0, 1000.0, 5000.0),
    "aggressive": ProfileTarget("aggressive", 110.0, 30.0, 1000.0, 5000.0),
}


def fmt_decimal(value: Any) -> str:
    if isinstance(value, str):
        return value
    text = f"{float(value):.10f}".rstrip("0").rstrip(".")
    return text if text else "0"


def planned_margin(first_order: float, multiplier: float, legs: int, leverage: int) -> float:
    return sum(first_order * (multiplier**i) / leverage for i in range(legs))


def margin_per_first_order(multiplier: float, legs: int, leverage: int) -> float:
    return sum((multiplier**i) / leverage for i in range(legs))


def filter_expressions(entry_filter: str, direction: str) -> list[str]:
    expressions: list[str] = []
    if entry_filter in ("trend", "trend_rsi"):
        expressions.append("close > ema(200)" if direction == "long" else "close < ema(200)")
        if entry_filter == "trend_rsi":
            expressions.append("rsi(14) < 65" if direction == "long" else "rsi(14) > 35")
    elif entry_filter == "rsi_extreme":
        expressions.append("rsi(14) < 30" if direction == "long" else "rsi(14) > 70")
    elif entry_filter == "rsi_moderate":
        expressions.append("rsi(14) < 35" if direction == "long" else "rsi(14) > 65")
    elif entry_filter == "bb_extreme":
        expressions.append("close < bb_lower(20,2.5)" if direction == "long" else "close > bb_upper(20,2.5)")
    elif entry_filter == "bb_moderate":
        expressions.append("close < bb_lower(20,2)" if direction == "long" else "close > bb_upper(20,2)")
    elif entry_filter == "rsi_bb_extreme":
        expressions.append("rsi(14) < 35" if direction == "long" else "rsi(14) > 65")
        expressions.append("close < bb_lower(20,2)" if direction == "long" else "close > bb_upper(20,2)")
    elif entry_filter == "rsi_bb_moderate":
        expressions.append("rsi(14) < 40" if direction == "long" else "rsi(14) > 60")
        expressions.append("close < bb_lower(20,1.5)" if direction == "long" else "close > bb_upper(20,1.5)")
    elif entry_filter == "btc_trend":
        expressions.append("BTCUSDT.close > BTCUSDT.ema(200)" if direction == "long" else "BTCUSDT.close < BTCUSDT.ema(200)")
    elif entry_filter == "btc_trend_rsi":
        expressions.append("BTCUSDT.close > BTCUSDT.ema(200)" if direction == "long" else "BTCUSDT.close < BTCUSDT.ema(200)")
        expressions.append("rsi(14) < 65" if direction == "long" else "rsi(14) > 35")
    return expressions


def stop_loss(stop_kind: str, stop_bps: int, rb_ema: int | None) -> dict[str, Any]:
    if stop_kind == "regime_break":
        return {
            "regime_break_stop": {
                "ema_period": int(rb_ema or 100),
                "drawdown_pct_bps": int(stop_bps),
            }
        }
    return {"strategy_drawdown_pct": {"pct_bps": int(stop_bps)}}


def strategy(
    *,
    symbol: str,
    direction: str,
    first_order: float,
    leverage: int,
    multiplier: float,
    max_legs: int,
    step_bps: int,
    tp_bps: int,
    stop_bps: int,
    cooldown: int,
    entry_filter: str,
    weight: float,
    stop_kind: str,
    rb_ema: int | None,
    max_cycle_age: float | None,
    adx_min: int | None,
    tag: str,
) -> dict[str, Any]:
    triggers: list[dict[str, Any]] = [{"cooldown": {"seconds": int(cooldown)}}]
    if adx_min is not None:
        triggers.append({"indicator_expression": {"expression": f"adx(14) > {int(adx_min)}"}})
    for expression in filter_expressions(entry_filter, direction):
        triggers.append({"indicator_expression": {"expression": expression}})
    risk_limits: dict[str, Any] = {}
    if max_cycle_age is not None:
        risk_limits["max_cycle_age_hours"] = float(max_cycle_age)
    return {
        "strategy_id": (
            f"native-{tag}-{symbol}-{direction}-foq{fmt_decimal(first_order)}"
            f"-lev{leverage}-m{fmt_decimal(multiplier)}-l{max_legs}-s{step_bps}"
            f"-tp{tp_bps}-sl{stop_bps}-{entry_filter}-rb{rb_ema}-age{max_cycle_age}"
        ),
        "symbol": symbol,
        "market": "usd_m_futures",
        "direction": direction,
        "direction_mode": "long_and_short" if direction in ("long", "short") else direction,
        "margin_mode": "isolated",
        "leverage": int(leverage),
        "spacing": {"fixed_percent": {"step_bps": int(step_bps)}},
        "sizing": {
            "multiplier": {
                "first_order_quote": fmt_decimal(first_order),
                "multiplier": fmt_decimal(multiplier),
                "max_legs": int(max_legs),
            }
        },
        "take_profit": {"percent": {"bps": int(tp_bps)}},
        "stop_loss": stop_loss(stop_kind, stop_bps, rb_ema),
        "indicators": [{"atr": {"period": 21}}, {"adx": {"period": 14}}],
        "entry_triggers": triggers,
        "risk_limits": risk_limits,
        "portfolio_weight_pct": fmt_decimal(weight),
    }


def base_template_pool(profile: str) -> list[dict[str, Any]]:
    if profile == "conservative":
        first_orders = [5.0, 7.5, 10.0, 12.5, 15.0]
        leverages = [3, 4, 5]
        multipliers = [1.20, 1.30, 1.40, 1.55]
        legs = [3, 4, 5]
        steps = [45, 65, 90, 120, 160]
        tps = [24, 35, 50, 70]
        stops = [250, 350, 500, 700]
        ages = [24.0, 48.0, 96.0, 168.0, None]
        filters = ["rsi_moderate", "bb_moderate", "trend_rsi", "btc_trend_rsi", "none"]
        stop_kinds = ["strategy_drawdown", "regime_break"]
    elif profile == "balanced":
        first_orders = [5.0, 7.5, 10.0, 15.0, 20.0, 25.0]
        leverages = [4, 5, 6, 8]
        multipliers = [1.30, 1.45, 1.60, 1.80]
        legs = [4, 5, 6]
        steps = [35, 50, 70, 100, 140]
        tps = [35, 50, 70, 95]
        stops = [400, 600, 850, 1200]
        ages = [24.0, 48.0, 96.0, 168.0, None]
        filters = ["rsi_moderate", "bb_moderate", "rsi_bb_moderate", "trend_rsi", "btc_trend_rsi", "none"]
        stop_kinds = ["strategy_drawdown", "regime_break"]
    else:
        first_orders = [5.0, 10.0, 15.0, 20.0, 30.0, 40.0]
        leverages = [5, 6, 8, 10]
        multipliers = [1.45, 1.70, 2.00, 2.30]
        legs = [4, 5, 6]
        steps = [25, 40, 60, 85, 120]
        tps = [50, 70, 100, 140]
        stops = [600, 900, 1300, 1800]
        ages = [24.0, 48.0, 96.0, None]
        filters = ["rsi_moderate", "bb_moderate", "rsi_bb_moderate", "trend_rsi", "btc_trend_rsi", "none"]
        stop_kinds = ["strategy_drawdown", "regime_break"]

    templates: list[dict[str, Any]] = []
    for first_order, leverage, multiplier, max_legs, step_bps, tp_bps, stop_bps, age, entry_filter, stop_kind in itertools.product(
        first_orders, leverages, multipliers, legs, steps, tps, stops, ages, filters, stop_kinds
    ):
        margin = planned_margin(first_order, multiplier, max_legs, leverage)
        if margin > 800.0:
            continue
        if first_order < 5.0:
            continue
        if stop_kind == "regime_break" and entry_filter in ("bb_moderate", "rsi_moderate"):
            # Keep regime-break mainly for trend/regime-aware entries. Pure mean
            # reversion plus EMA break tended to flatten into near-zero turnover.
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


def symbol_direction_pool(profile: str) -> list[tuple[str, str]]:
    if profile == "conservative":
        longs = ["BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT", "TRXUSDT", "XRPUSDT", "ADAUSDT", "LTCUSDT"]
        shorts = ["DOTUSDT", "APTUSDT", "COMPUSDT", "NEARUSDT", "GALAUSDT", "ETCUSDT"]
    elif profile == "balanced":
        longs = ["BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT", "XRPUSDT", "ADAUSDT", "LTCUSDT", "LINKUSDT", "AVAXUSDT"]
        shorts = ["DOTUSDT", "APTUSDT", "COMPUSDT", "NEARUSDT", "GALAUSDT", "ETCUSDT", "BCHUSDT", "ICPUSDT"]
    else:
        longs = ["BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT", "XRPUSDT", "ADAUSDT", "LINKUSDT", "AVAXUSDT", "BCHUSDT"]
        shorts = ["DOTUSDT", "APTUSDT", "COMPUSDT", "NEARUSDT", "GALAUSDT", "ETCUSDT", "ICPUSDT", "BCHUSDT"]
    return [(s, "long") for s in longs] + [(s, "short") for s in shorts]


def make_config(
    profile: str,
    budget: float,
    rng: random.Random,
    idx: int,
    templates: list[dict[str, Any]],
) -> dict[str, Any]:
    target_count = rng.choice([4, 5, 5, 6, 7] if profile == "conservative" else [5, 6, 7, 8, 9])
    if profile == "aggressive":
        target_count = rng.choice([5, 6, 7, 8])
    pairs = symbol_direction_pool(profile)
    rng.shuffle(pairs)
    selected = pairs[:target_count]
    if profile == "conservative":
        utilization_choices = [0.28, 0.38, 0.50, 0.65, 0.82]
    elif profile == "balanced":
        utilization_choices = [0.45, 0.60, 0.75, 0.90, 0.98]
    else:
        utilization_choices = [0.60, 0.75, 0.90, 0.98]
    weights_raw = [rng.uniform(0.6, 1.6) for _ in selected]
    weights_sum = sum(weights_raw)
    weights = [w / weights_sum * 100.0 for w in weights_raw]
    strategies = []
    for seq, ((symbol, direction), weight) in enumerate(zip(selected, weights, strict=True)):
        tmpl = dict(rng.choice(templates))
        cap = budget * (weight / 100.0)
        # Native small-cap search must size from the budget downward, not start
        # from a tiny first order upward. This mirrors the Rust single-strategy
        # search: complete ladder margin = first_order * sum(mult^i/leverage).
        util = rng.choice(utilization_choices)
        denom = margin_per_first_order(tmpl["multiplier"], tmpl["max_legs"], tmpl["leverage"])
        first_order = max(5.0, cap * util / max(denom, 1e-9))
        margin_after = planned_margin(first_order, tmpl["multiplier"], tmpl["max_legs"], tmpl["leverage"])
        if margin_after > cap * 0.98:
            first_order = max(5.0, first_order * (cap * 0.98 / margin_after))
            margin_after = planned_margin(first_order, tmpl["multiplier"], tmpl["max_legs"], tmpl["leverage"])
        if margin_after > cap * 1.01:
            # If a very small weight cannot fit the exchange minimum, drop legs
            # until the strategy can still place a legal 5U first order.
            for legs in sorted([2, 3, 4, 5, 6]):
                denom = margin_per_first_order(tmpl["multiplier"], legs, tmpl["leverage"])
                trial_first = max(5.0, cap * min(util, 0.95) / max(denom, 1e-9))
                trial_margin = planned_margin(trial_first, tmpl["multiplier"], legs, tmpl["leverage"])
                if trial_margin <= cap * 0.98:
                    tmpl["max_legs"] = legs
                    first_order = trial_first
                    break
        strategies.append(
            strategy(
                symbol=symbol,
                direction=direction,
                first_order=round(first_order, 4),
                leverage=int(tmpl["leverage"]),
                multiplier=float(tmpl["multiplier"]),
                max_legs=int(tmpl["max_legs"]),
                step_bps=int(tmpl["step_bps"]),
                tp_bps=int(tmpl["tp_bps"]),
                stop_bps=int(tmpl["stop_bps"]),
                cooldown=rng.choice([900, 1800, 3600, 7200]),
                entry_filter=str(tmpl["entry_filter"]),
                weight=weight,
                stop_kind=str(tmpl["stop_kind"]),
                rb_ema=tmpl["rb_ema"],
                max_cycle_age=tmpl["max_cycle_age"],
                adx_min=tmpl["adx_min"],
                tag=f"{profile}-{idx:05d}-{seq}",
            )
        )
    return {
        "portfolio_config": {
            "direction_mode": "long_and_short",
            "strategies": strategies,
            "risk_limits": {
                "max_global_budget_quote": fmt_decimal(budget),
            },
        }
    }


def run_replay(args: argparse.Namespace, cfg: dict[str, Any], start: int, end: int, timeout: int) -> dict[str, Any]:
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as fh:
        json.dump(cfg, fh)
        cfg_path = fh.name
    try:
        cmd = [
            str(args.replay_bin),
            "--config",
            cfg_path,
            "--budget",
            fmt_decimal(args.budget),
            "--profile",
            args.profile,
            "--start-ms",
            str(start),
            "--end-ms",
            str(end),
            "--market-data",
            str(args.market_data),
            "--funding-data",
            str(args.funding_data),
            "--exchange-min-notional",
            "5",
            "--equity-curve-points",
            "16",
        ]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        if proc.returncode != 0:
            return {"ok": False, "error": proc.stderr[-1600:], "returncode": proc.returncode}
        data = json.loads(proc.stdout)
        on = data.get("on_budget", {}) or {}
        return {
            "ok": True,
            "ann": on.get("annualized_return_pct"),
            "dd": on.get("max_drawdown_pct"),
            "ret": on.get("total_return_pct"),
            "principal_breached": on.get("principal_breached"),
            "min_equity": on.get("min_equity_quote"),
            "cap": data.get("max_capital_used_quote"),
            "blocked": data.get("budget_blocked_legs"),
            "trades": data.get("trade_count"),
            "stops": data.get("stop_count"),
            "gate": (data.get("gate") or {}).get("passed"),
            "strategy_count": data.get("strategy_count"),
            "symbols": data.get("symbols"),
            "equity_curve_sample": data.get("equity_curve_sample"),
        }
    except subprocess.TimeoutExpired:
        return {"ok": False, "error": "timeout"}
    finally:
        try:
            os.unlink(cfg_path)
        except OSError:
            pass


def segment_score(full: dict[str, Any], segments: dict[str, dict[str, Any]]) -> dict[str, Any]:
    returns = []
    dds = []
    positive = 0
    for name, result in segments.items():
        if not result.get("ok"):
            continue
        ret = result.get("ret")
        dd = result.get("dd")
        if isinstance(ret, (int, float)):
            returns.append(float(ret))
            if ret > 0:
                positive += 1
        if isinstance(dd, (int, float)):
            dds.append(float(dd))
    ann = float(full.get("ann") or -999.0)
    dd = float(full.get("dd") or 999.0)
    h1 = float(segments.get("h1_2023", {}).get("ret") or 0.0)
    full_ret = float(full.get("ret") or 0.0)
    h1_ratio = h1 / full_ret if abs(full_ret) > 1e-9 else 0.0
    mean_ret = sum(returns) / len(returns) if returns else -999.0
    variance = sum((r - mean_ret) ** 2 for r in returns) / len(returns) if returns else 999.0
    return {
        "positive_segments": positive,
        "segment_returns": returns,
        "max_segment_dd": max(dds) if dds else None,
        "h1_contribution_ratio": h1_ratio,
        "score": ann - 2.5 * max(0.0, dd - 15.0) + mean_ret - math.sqrt(max(0.0, variance)),
    }


def candidate_gate(profile: str, full: dict[str, Any], segments: dict[str, dict[str, Any]] | None = None) -> bool:
    target = TARGETS[profile]
    if not full.get("ok"):
        return False
    if full.get("principal_breached"):
        return False
    if float(full.get("ann") or -999.0) < target.ann:
        return False
    if float(full.get("dd") or 999.0) > target.dd:
        return False
    if float(full.get("cap") or 0.0) > target.max_budget:
        return False
    if int(full.get("blocked") or 0) > 0:
        return False
    if segments is None:
        return True
    sc = segment_score(full, segments)
    if sc["positive_segments"] < 4:
        return False
    if sc["max_segment_dd"] is not None and sc["max_segment_dd"] > target.dd * 1.8:
        return False
    if sc["h1_contribution_ratio"] > 0.45:
        return False
    return True


def load_existing_configs(paths: list[str]) -> list[dict[str, Any]]:
    out = []
    for pattern in paths:
        for path in sorted(Path().glob(pattern) if not pattern.startswith("/") else Path("/").glob(pattern[1:])):
            try:
                data = json.loads(path.read_text())
                if "portfolio_config" in data:
                    out.append(data)
            except Exception:
                continue
    return out


def save_report(path: Path, report: dict[str, Any]) -> None:
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(report, ensure_ascii=False, indent=2))
    tmp.replace(path)


def run_one(job: tuple[int, dict[str, Any], argparse.Namespace]) -> dict[str, Any]:
    idx, cfg, args = job
    full = run_replay(args, cfg, FULL[0], FULL[1], args.timeout)
    return {"idx": idx, "config": cfg, "full": full}


def validate_segments(args: argparse.Namespace, item: dict[str, Any]) -> dict[str, Any]:
    cfg = item["config"]
    segments = {}
    for name, (start, end) in SEGMENTS.items():
        segments[name] = run_replay(args, cfg, start, end, args.segment_timeout)
    return {**item, "segments": segments, "segment_score": segment_score(item["full"], segments)}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", choices=sorted(TARGETS), required=True)
    parser.add_argument("--budget", type=float, default=5000.0)
    parser.add_argument("--count", type=int, default=240)
    parser.add_argument("--seed", type=int, default=20260629)
    parser.add_argument("--jobs", type=int, default=20)
    parser.add_argument("--top-segment", type=int, default=20)
    parser.add_argument("--timeout", type=int, default=900)
    parser.add_argument("--segment-timeout", type=int, default=600)
    parser.add_argument("--out-dir", required=True)
    parser.add_argument("--replay-bin", required=True)
    parser.add_argument("--market-data", required=True)
    parser.add_argument("--funding-data", required=True)
    parser.add_argument("--existing-config", action="append", default=[])
    args = parser.parse_args()
    args.replay_bin = Path(args.replay_bin)
    args.market_data = Path(args.market_data)
    args.funding_data = Path(args.funding_data)

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    rng = random.Random(args.seed)

    templates = base_template_pool(args.profile)
    configs = [make_config(args.profile, args.budget, rng, i, templates) for i in range(args.count)]
    configs.extend(load_existing_configs(args.existing_config))

    report: dict[str, Any] = {
        "profile": args.profile,
        "budget": args.budget,
        "seed": args.seed,
        "count": len(configs),
        "jobs": args.jobs,
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "full_results": [],
        "segment_validations": [],
    }
    report_path = out_dir / f"native_{args.profile}_b{int(args.budget)}_seed{args.seed}.json"
    run_log = out_dir / f"native_{args.profile}_b{int(args.budget)}_seed{args.seed}.log"

    jobs = [(i, cfg, args) for i, cfg in enumerate(configs)]
    done = 0
    with futures.ProcessPoolExecutor(max_workers=args.jobs) as pool:
        future_map = {pool.submit(run_one, job): job[0] for job in jobs}
        with run_log.open("a") as log:
            for fut in futures.as_completed(future_map):
                done += 1
                try:
                    item = fut.result()
                except Exception as exc:
                    item = {"idx": future_map[fut], "config": None, "full": {"ok": False, "error": repr(exc)}}
                report["full_results"].append(item)
                full = item["full"]
                if full.get("ok"):
                    line = (
                        f"DONE {done}/{len(jobs)} idx={item['idx']} "
                        f"ann={float(full.get('ann') or -999):.2f} dd={float(full.get('dd') or 999):.2f} "
                        f"ret={float(full.get('ret') or 0):.2f} cap={float(full.get('cap') or 0):.1f} "
                        f"blocked={full.get('blocked')} trades={full.get('trades')}\n"
                    )
                else:
                    line = f"DONE {done}/{len(jobs)} idx={item['idx']} ERROR {full.get('error')}\n"
                log.write(line)
                log.flush()
                if done % max(1, min(10, args.jobs)) == 0:
                    save_report(report_path, report)
    full_ok = [r for r in report["full_results"] if r["full"].get("ok")]
    full_ok.sort(
        key=lambda r: (
            float(r["full"].get("ann") or -999.0)
            - 3.0 * max(0.0, float(r["full"].get("dd") or 999.0) - TARGETS[args.profile].dd),
            -float(r["full"].get("dd") or 999.0),
        ),
        reverse=True,
    )
    segment_candidates = full_ok[: args.top_segment]
    for item in segment_candidates:
        validated = validate_segments(args, item)
        report["segment_validations"].append(validated)
        save_report(report_path, report)
    report["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    report["passes"] = [
        r
        for r in report["segment_validations"]
        if candidate_gate(args.profile, r["full"], r.get("segments"))
    ]
    save_report(report_path, report)
    print(report_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
