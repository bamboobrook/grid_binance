#!/usr/bin/env python3
"""Research-only dynamic breakout/trend portfolio probe."""
from __future__ import annotations

import argparse
import collections
import importlib.util
import itertools
import json
import math
import sqlite3
import statistics
from dataclasses import dataclass
from pathlib import Path


HYBRID_PATH = Path(__file__).with_name("hybrid_martingale_frontier_probe.py")
HYBRID_SPEC = importlib.util.spec_from_file_location("hybrid_probe", HYBRID_PATH)
hybrid = importlib.util.module_from_spec(HYBRID_SPEC)
HYBRID_SPEC.loader.exec_module(hybrid)

MS_PER_DAY = 86_400_000
LIVE_PARITY_STATUS = "research_only"


@dataclass(frozen=True)
class RuleSpec:
    family: str
    fast: int | None = None
    slow: int | None = None
    lookback: int | None = None
    mode: str = "long_flat"


def parse_csv(value: str) -> list[str]:
    return [item.strip() for item in value.split(",") if item.strip()]


def parse_floats(value: str) -> list[float]:
    return [float(item) for item in parse_csv(value)]


def parse_ints(value: str) -> list[int]:
    return [int(item) for item in parse_csv(value)]


def compress_daily_ohlc(rows: list[dict]) -> list[dict]:
    grouped: collections.OrderedDict[int, dict] = collections.OrderedDict()
    for row in sorted(rows, key=lambda item: int(item["timestamp_ms"])):
        day_start = int(row["timestamp_ms"]) // MS_PER_DAY * MS_PER_DAY
        if day_start not in grouped:
            grouped[day_start] = {
                "timestamp_ms": day_start,
                "open": float(row["open"]),
                "high": float(row["high"]),
                "low": float(row["low"]),
                "close": float(row["close"]),
                "volume": float(row.get("volume", 0.0)),
            }
        else:
            current = grouped[day_start]
            current["high"] = max(current["high"], float(row["high"]))
            current["low"] = min(current["low"], float(row["low"]))
            current["close"] = float(row["close"])
            current["volume"] += float(row.get("volume", 0.0))
    return list(grouped.values())


def load_minute_bars(market_db: str | Path, symbol: str, market_type: str = "futures_usdt_perp") -> list[dict]:
    con = sqlite3.connect(str(market_db))
    try:
        rows = con.execute(
            """
            SELECT open_time, open, high, low, close, volume
            FROM klines
            WHERE symbol = ? AND market_type = ? AND timeframe = '1m'
            ORDER BY open_time
            """,
            (symbol, market_type),
        ).fetchall()
    finally:
        con.close()
    return [
        {
            "timestamp_ms": int(open_time),
            "open": float(open_),
            "high": float(high),
            "low": float(low),
            "close": float(close),
            "volume": float(volume or 0.0),
        }
        for open_time, open_, high, low, close, volume in rows
    ]


def load_daily_bars(market_db: str | Path, symbol: str, market_type: str = "futures_usdt_perp") -> list[dict]:
    return compress_daily_ohlc(load_minute_bars(market_db, symbol, market_type))


def parse_rule(rule: str) -> RuleSpec:
    if rule.startswith("mom") and rule.endswith("_lf"):
        return RuleSpec(family="momentum", lookback=int(rule[3:-3]), mode="long_flat")
    if rule.startswith("mom") and rule.endswith("_ls"):
        return RuleSpec(family="momentum", lookback=int(rule[3:-3]), mode="long_short")
    if rule.startswith("donchian") and rule.endswith("_lf"):
        return RuleSpec(family="donchian", lookback=int(rule[8:-3]), mode="long_flat")
    if rule.startswith("donchian") and rule.endswith("_ls"):
        return RuleSpec(family="donchian", lookback=int(rule[8:-3]), mode="long_short")
    if rule.startswith("ema") and rule.endswith("_lf"):
        fast, slow = rule[3:-3].split("_", 1)
        return RuleSpec(family="ema", fast=int(fast), slow=int(slow), mode="long_flat")
    raise ValueError(f"unknown signal rule: {rule}")


def ema_values(values: list[float], period: int) -> list[float | None]:
    if period <= 0:
        raise ValueError("EMA period must be positive")
    alpha = 2.0 / (period + 1.0)
    out: list[float | None] = []
    ema = None
    for index, value in enumerate(values):
        ema = float(value) if ema is None else alpha * float(value) + (1.0 - alpha) * ema
        out.append(ema if index + 1 >= period else None)
    return out


def first_executable_index(spec: RuleSpec) -> int:
    if spec.family in {"momentum", "donchian"}:
        return int(spec.lookback or 0) + 1
    if spec.family == "ema":
        return max(int(spec.fast or 0), int(spec.slow or 0))
    raise ValueError(f"unknown signal family: {spec.family}")


def desired_position(
    spec: RuleSpec,
    daily: list[dict],
    index: int,
    fast_ema: list[float | None],
    slow_ema: list[float | None],
) -> tuple[int, float]:
    signal_index = index - 1
    if spec.family == "momentum":
        lookback = int(spec.lookback or 0)
        ref_index = signal_index - lookback
        ref_close = float(daily[ref_index]["close"])
        signal_close = float(daily[signal_index]["close"])
        momentum = signal_close / ref_close - 1.0 if ref_close > 0 else 0.0
        position = 1 if momentum > 0 else (-1 if spec.mode == "long_short" and momentum < 0 else 0)
        return position, abs(momentum)
    if spec.family == "donchian":
        lookback = int(spec.lookback or 0)
        signal_close = float(daily[signal_index]["close"])
        window = [float(row["close"]) for row in daily[signal_index - lookback:signal_index]]
        upper = max(window)
        lower = min(window)
        if signal_close > upper:
            return 1, signal_close / upper - 1.0 if upper > 0 else 0.0
        if signal_close < lower:
            position = -1 if spec.mode == "long_short" else 0
            return position, lower / signal_close - 1.0 if signal_close > 0 else 0.0
        return 0, 0.0
    if spec.family == "ema":
        prev_fast = fast_ema[signal_index]
        prev_slow = slow_ema[signal_index]
        if prev_fast is None or prev_slow is None:
            return 0, 0.0
        distance = prev_fast / prev_slow - 1.0 if prev_slow > 0 else 0.0
        return (1 if distance > 0 else 0), abs(distance)
    raise ValueError(f"unknown signal family: {spec.family}")


def build_signal_stream(symbol: str, daily: list[dict], rule: str, fee_bps: float, slippage_bps: float) -> dict:
    spec = parse_rule(rule)
    closes = [float(row["close"]) for row in daily]
    fast_ema = ema_values(closes, int(spec.fast or 1)) if spec.family == "ema" else [None] * len(daily)
    slow_ema = ema_values(closes, int(spec.slow or 1)) if spec.family == "ema" else [None] * len(daily)
    cost_rate = (float(fee_bps) + float(slippage_bps)) / 10_000.0
    previous_position = 0
    points = []
    start_index = first_executable_index(spec)
    for index in range(start_index, len(daily)):
        position, strength = desired_position(spec, daily, index, fast_ema, slow_ema)
        prev_close = float(daily[index - 1]["close"])
        close = float(daily[index]["close"])
        day_return = 0.0
        if prev_close > 0:
            price_return = close / prev_close - 1.0
            day_return = position * price_return
        if position != previous_position:
            day_return -= cost_rate
        points.append(
            {
                "timestamp_ms": int(daily[index]["timestamp_ms"]),
                "return": day_return,
                "position": position,
                "strength": strength,
            }
        )
        previous_position = position
    return {
        "name": f"trend:{symbol}:{rule}",
        "kind": "dynamic_trend_stream",
        "symbol": symbol,
        "symbols": [symbol],
        "rule": rule,
        "points": points,
        "fee_bps": float(fee_bps),
        "slippage_bps": float(slippage_bps),
        "no_lookahead": True,
        "live_parity_status": LIVE_PARITY_STATUS,
    }


def stream_returns_before(stream: dict, as_of_ts: int, lookback_days: int) -> list[float]:
    start_ts = int(as_of_ts) - int(lookback_days) * MS_PER_DAY
    return [
        float(point["return"])
        for point in stream.get("points", [])
        if start_ts <= int(point["timestamp_ms"]) < int(as_of_ts)
    ]


def trailing_score(stream: dict, as_of_ts: int, lookback_days: int) -> float:
    returns = stream_returns_before(stream, as_of_ts, lookback_days)
    if len(returns) < 2:
        return -1_000_000.0
    mean_return = statistics.fmean(returns)
    downside = [value for value in returns if value < 0.0]
    downside_vol = statistics.pstdev(downside) if len(downside) >= 2 else 0.0
    total_return = math.prod(1.0 + value for value in returns) - 1.0
    strength_points = [
        float(point.get("strength", 0.0))
        for point in stream.get("points", [])
        if int(as_of_ts) - int(lookback_days) * MS_PER_DAY <= int(point["timestamp_ms"]) < int(as_of_ts)
    ]
    strength = statistics.fmean(strength_points) if strength_points else 0.0
    risk_penalty = downside_vol * 2.0
    return total_return + mean_return * 10.0 + strength - risk_penalty


def rank_streams(streams: list[dict], as_of_ts: int, lookback_days: int) -> list[dict]:
    ranked = []
    for stream in streams:
        ranked.append(
            {
                "name": stream["name"],
                "symbol": stream["symbol"],
                "rule": stream["rule"],
                "score": trailing_score(stream, as_of_ts, lookback_days),
            }
        )
    ranked.sort(key=lambda item: (item["score"], item["name"]), reverse=True)
    return ranked


def select_top_streams(ranked: list[dict], top_n: int, max_symbol_weight: float, min_symbols: int = 2) -> list[dict]:
    if top_n < min_symbols:
        raise ValueError("top_n must be at least min_symbols")
    if not 0.0 < max_symbol_weight <= 1.0:
        raise ValueError("max_symbol_weight must be in (0, 1]")
    max_streams_per_symbol = max(1, int(math.floor(max_symbol_weight * top_n + 1e-12)))
    selected = []
    counts: collections.Counter[str] = collections.Counter()
    for item in ranked:
        symbol = item["symbol"]
        if counts[symbol] >= max_streams_per_symbol:
            continue
        selected.append(item)
        counts[symbol] += 1
        if len(selected) == top_n:
            break
    if len({item["symbol"] for item in selected}) < min_symbols:
        selected = []
        seen_symbols = set()
        for item in ranked:
            if item["symbol"] in seen_symbols:
                continue
            selected.append(item)
            seen_symbols.add(item["symbol"])
            if len(selected) == min_symbols:
                break
    return selected


def capped_equal_weights(selected: list[dict], max_symbol_weight: float) -> dict[str, float]:
    if not selected:
        return {}
    raw_weight = 1.0 / len(selected)
    by_symbol: collections.defaultdict[str, list[str]] = collections.defaultdict(list)
    for item in selected:
        by_symbol[item["symbol"]].append(item["name"])
    weights = {}
    for _symbol, names in by_symbol.items():
        symbol_weight = min(max_symbol_weight, raw_weight * len(names))
        per_stream = symbol_weight / len(names)
        for name in names:
            weights[name] = per_stream
    total = sum(weights.values())
    if total <= 0.0:
        return {}
    return {name: value / total for name, value in weights.items()}


def realized_vol_pct(returns: list[float]) -> float:
    if len(returns) < 2:
        return 0.0
    return statistics.pstdev(returns) * math.sqrt(365.0) * 100.0


def volatility_scale(returns: list[float], target_vol_pct: float, max_scale: float = 1.0) -> float:
    vol = realized_vol_pct(returns)
    if vol <= 0.0:
        return min(1.0, max_scale)
    return max(0.0, min(max_scale, float(target_vol_pct) / vol))


def point_map(stream: dict) -> dict[int, dict]:
    return {int(point["timestamp_ms"]): point for point in stream.get("points", [])}


def common_timestamps(streams: list[dict]) -> list[int]:
    if not streams:
        return []
    sets = [set(point_map(stream).keys()) for stream in streams]
    return sorted(set.intersection(*sets))


def build_dynamic_portfolio(
    streams: list[dict],
    allocation_quote: float,
    rebalance_days: int,
    score_lookback_days: int,
    top_n: int,
    max_symbol_weight: float,
    target_vol_pct: float,
    vol_lookback_days: int,
    dd_stop_pct: float,
    cooldown_days: int,
) -> dict:
    if allocation_quote <= 0:
        raise ValueError("allocation_quote must be positive")
    if allocation_quote >= 5000.0:
        raise ValueError("allocation_quote must stay below 5000")
    timestamps = common_timestamps(streams)
    maps = {stream["name"]: point_map(stream) for stream in streams}
    by_name = {stream["name"]: stream for stream in streams}
    equity = float(allocation_quote)
    peak = equity
    cooldown_until = -1
    risk_events = 0
    weights: dict[str, float] = {}
    portfolio_returns: list[float] = []
    points = []
    rebalance_counter = 0
    max_symbol_weight_observed = 0.0

    for ts in timestamps:
        in_cooldown = ts < cooldown_until
        if not in_cooldown and rebalance_counter % max(1, rebalance_days) == 0:
            ranked = rank_streams(streams, ts, score_lookback_days)
            selected = select_top_streams(ranked, top_n=top_n, max_symbol_weight=max_symbol_weight, min_symbols=2)
            weights = capped_equal_weights(selected, max_symbol_weight=max_symbol_weight)
        rebalance_counter += 1

        scale = volatility_scale(portfolio_returns[-vol_lookback_days:], target_vol_pct=target_vol_pct)
        effective_weights = {} if in_cooldown else {name: weight * scale for name, weight in weights.items()}
        day_return = 0.0
        symbol_weights: collections.Counter[str] = collections.Counter()
        for name, weight in effective_weights.items():
            point = maps[name][ts]
            symbol = by_name[name]["symbol"]
            symbol_weights[symbol] += weight
            day_return += weight * float(point["return"])
        max_symbol_weight_observed = max(max_symbol_weight_observed, max(symbol_weights.values(), default=0.0))
        equity *= max(0.0, 1.0 + day_return)
        peak = max(peak, equity)
        drawdown = (peak - equity) / peak * 100.0 if peak > 0 else 0.0
        if not in_cooldown and dd_stop_pct > 0.0 and drawdown >= dd_stop_pct:
            risk_events += 1
            cooldown_until = ts + int(cooldown_days) * MS_PER_DAY
            peak = equity
        portfolio_returns.append(day_return)
        points.append(
            {
                "timestamp_ms": ts,
                "equity_quote": equity,
                "daily_return": day_return,
                "gross_weight": sum(abs(value) for value in effective_weights.values()),
                "max_symbol_weight": max(symbol_weights.values(), default=0.0),
                "in_cooldown": in_cooldown,
                "risk_events": risk_events,
            }
        )

    symbols = sorted({stream["symbol"] for stream in streams})
    metrics = hybrid.compute_metrics(points)
    metrics.update(
        {
            "max_capital_used_quote": float(allocation_quote),
            "budget_blocked_events": 0,
            "symbol_count": len(symbols),
            "max_symbol_weight_observed": max_symbol_weight_observed,
            "risk_events": risk_events,
        }
    )
    return {
        "streams": sorted(by_name),
        "symbols": symbols,
        "points": points,
        "metrics": metrics,
        "risk_events": risk_events,
        "live_parity_status": LIVE_PARITY_STATUS,
    }


def build_candidate_report(
    profile: str,
    portfolio: dict,
    budget: float,
    max_symbol_weight: float,
    config: dict | None = None,
) -> dict:
    base = hybrid.build_candidate_report(profile, portfolio, budget)
    full_gate = dict(base["full_gate"])
    segment_gate = dict(base["segment_gate"])
    full_violations = list(full_gate["violations"])
    segment_violations = list(segment_gate["violations"])
    observed = float(portfolio["metrics"].get("max_symbol_weight_observed", 0.0))
    if observed > max_symbol_weight + 1e-12:
        full_violations.append(f"max symbol weight {observed:.4f} > cap {max_symbol_weight:.4f}")
    combined = float(segment_gate.get("combined_2024_2026_return_pct", 0.0))
    if combined <= 0.0:
        segment_violations.append(f"2024-2026 combined return {combined:.2f}% <= 0")
    full_gate["violations"] = full_violations
    full_gate["passes"] = not full_violations
    segment_gate["violations"] = segment_violations
    segment_gate["passes"] = not segment_violations
    base["full_gate"] = full_gate
    base["segment_gate"] = segment_gate
    base["passes_offline"] = full_gate["passes"] and segment_gate["passes"]
    base["config"] = dict(config or {})
    base["live_parity_status"] = LIVE_PARITY_STATUS
    return base


def row_from_report(report: dict) -> dict:
    metrics = report["full_metrics"]
    segment = report["segment_gate"]
    config = report.get("config", {})
    return {
        "profile": report["profile"],
        "ann": metrics.get("annualized_return_pct"),
        "dd": metrics.get("max_drawdown_pct"),
        "cap": metrics.get("max_capital_used_quote"),
        "symbol_count": len(set(report.get("symbols", []))),
        "symbols": ",".join(report.get("symbols", [])),
        "streams": ",".join(report.get("streams", [])),
        "pos": segment.get("positive_segments"),
        "c2426": segment.get("combined_2024_2026_return_pct"),
        "risk_events": metrics.get("risk_events", 0),
        "max_symbol_weight": metrics.get("max_symbol_weight_observed", 0.0),
        "top_n": config.get("top_n"),
        "rebalance_days": config.get("rebalance_days"),
        "target_vol_pct": config.get("target_vol_pct"),
        "dd_stop_pct": config.get("dd_stop_pct"),
        "cooldown_days": config.get("cooldown_days"),
        "pass": report.get("passes_offline", False),
        "full_pass": report["full_gate"]["passes"],
        "seg_pass": segment["passes"],
        "violations": report["full_gate"]["violations"] + segment["violations"],
        "live_parity_status": LIVE_PARITY_STATUS,
    }
