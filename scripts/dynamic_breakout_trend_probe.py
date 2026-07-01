#!/usr/bin/env python3
"""Research-only dynamic breakout/trend portfolio probe."""
from __future__ import annotations

import argparse
import bisect
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


def load_daily_bars(
    market_db: str | Path,
    symbol: str,
    market_type: str = "futures_usdt_perp",
    start_ms: int | None = None,
    end_ms: int | None = None,
) -> list[dict]:
    con = sqlite3.connect(str(market_db))
    try:
        has_symbol_time_index = bool(
            con.execute(
                "SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = 'idx_klines_symbol_time'"
            ).fetchone()
        )
        index_hint = " INDEXED BY idx_klines_symbol_time" if has_symbol_time_index else ""
        cursor = con.execute(
            f"""
            SELECT open_time, open, high, low, close, volume
            FROM klines{index_hint}
            WHERE symbol = ?
              AND (? IS NULL OR open_time >= ?)
              AND (? IS NULL OR open_time <= ?)
              AND market_type = ?
              AND timeframe = '1m'
            ORDER BY open_time
            """,
            (
                symbol,
                start_ms,
                int(start_ms) if start_ms is not None else None,
                end_ms,
                int(end_ms) if end_ms is not None else None,
                market_type,
            ),
        )
        daily = []
        current = None
        for ts, open_, high, low, close, volume in cursor:
            day_start = int(ts) // MS_PER_DAY * MS_PER_DAY
            if current is None or current["timestamp_ms"] != day_start:
                if current is not None:
                    daily.append(current)
                current = {
                    "timestamp_ms": day_start,
                    "open": float(open_),
                    "high": float(high),
                    "low": float(low),
                    "close": float(close),
                    "volume": float(volume or 0.0),
                }
            else:
                current["high"] = max(current["high"], float(high))
                current["low"] = min(current["low"], float(low))
                current["close"] = float(close)
                current["volume"] += float(volume or 0.0)
        if current is not None:
            daily.append(current)
        return daily
    finally:
        con.close()


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


def prepare_stream_cache(streams: list[dict]) -> None:
    for stream in streams:
        points = sorted(stream.get("points", []), key=lambda point: int(point["timestamp_ms"]))
        timestamps = [int(point["timestamp_ms"]) for point in points]
        returns = [float(point["return"]) for point in points]
        strengths = [float(point.get("strength", 0.0)) for point in points]
        prefix_returns = [0.0]
        prefix_log_growth = [0.0]
        prefix_strength = [0.0]
        prefix_downside_count = [0]
        prefix_downside_sum = [0.0]
        prefix_downside_sq = [0.0]
        for value, strength in zip(returns, strengths):
            prefix_returns.append(prefix_returns[-1] + value)
            prefix_log_growth.append(prefix_log_growth[-1] + math.log(max(1e-12, 1.0 + value)))
            prefix_strength.append(prefix_strength[-1] + strength)
            is_downside = value < 0.0
            prefix_downside_count.append(prefix_downside_count[-1] + (1 if is_downside else 0))
            prefix_downside_sum.append(prefix_downside_sum[-1] + (value if is_downside else 0.0))
            prefix_downside_sq.append(prefix_downside_sq[-1] + (value * value if is_downside else 0.0))
        stream["_cache"] = {
            "timestamps": timestamps,
            "returns": returns,
            "prefix_returns": prefix_returns,
            "prefix_log_growth": prefix_log_growth,
            "prefix_strength": prefix_strength,
            "prefix_downside_count": prefix_downside_count,
            "prefix_downside_sum": prefix_downside_sum,
            "prefix_downside_sq": prefix_downside_sq,
        }


def trailing_score(stream: dict, as_of_ts: int, lookback_days: int) -> float:
    cache = stream.get("_cache")
    if cache is None:
        returns = stream_returns_before(stream, as_of_ts, lookback_days)
        if len(returns) < 2:
            return -1_000_000.0
        mean_return = statistics.fmean(returns)
        total_return = math.prod(1.0 + value for value in returns) - 1.0
        strength_points = [
            float(point.get("strength", 0.0))
            for point in stream.get("points", [])
            if int(as_of_ts) - int(lookback_days) * MS_PER_DAY <= int(point["timestamp_ms"]) < int(as_of_ts)
        ]
        strength = statistics.fmean(strength_points) if strength_points else 0.0
        downside = [value for value in returns if value < 0.0]
        downside_vol = statistics.pstdev(downside) if len(downside) >= 2 else 0.0
        return total_return + mean_return * 10.0 + strength - downside_vol * 2.0

    start_ts = int(as_of_ts) - int(lookback_days) * MS_PER_DAY
    timestamps = cache["timestamps"]
    start_index = bisect.bisect_left(timestamps, start_ts)
    end_index = bisect.bisect_left(timestamps, int(as_of_ts))
    count = end_index - start_index
    if count < 2:
        return -1_000_000.0
    sum_return = cache["prefix_returns"][end_index] - cache["prefix_returns"][start_index]
    log_growth = cache["prefix_log_growth"][end_index] - cache["prefix_log_growth"][start_index]
    strength_sum = cache["prefix_strength"][end_index] - cache["prefix_strength"][start_index]
    downside_count = cache["prefix_downside_count"][end_index] - cache["prefix_downside_count"][start_index]
    downside_sum = cache["prefix_downside_sum"][end_index] - cache["prefix_downside_sum"][start_index]
    downside_sq = cache["prefix_downside_sq"][end_index] - cache["prefix_downside_sq"][start_index]
    mean_return = sum_return / count
    total_return = math.exp(log_growth) - 1.0
    strength = strength_sum / count
    downside_vol = 0.0
    if downside_count >= 2:
        downside_mean = downside_sum / downside_count
        downside_var = max(0.0, downside_sq / downside_count - downside_mean * downside_mean)
        downside_vol = math.sqrt(downside_var)
    return total_return + mean_return * 10.0 + strength - downside_vol * 2.0


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


def build_base_portfolio_path(
    streams: list[dict],
    rebalance_days: int,
    score_lookback_days: int,
    top_n: int,
    max_symbol_weight: float,
    ranking_cache: dict[tuple[int, int], list[dict]] | None = None,
) -> dict:
    timestamps = common_timestamps(streams)
    maps = {stream["name"]: point_map(stream) for stream in streams}
    by_name = {stream["name"]: stream for stream in streams}
    weights: dict[str, float] = {}
    points = []
    rebalance_counter = 0

    for ts in timestamps:
        if rebalance_counter % max(1, rebalance_days) == 0:
            if ranking_cache is None:
                ranked = rank_streams(streams, ts, score_lookback_days)
            else:
                ranked = ranking_cache.setdefault((score_lookback_days, ts), rank_streams(streams, ts, score_lookback_days))
            selected = select_top_streams(ranked, top_n=top_n, max_symbol_weight=max_symbol_weight, min_symbols=2)
            weights = capped_equal_weights(selected, max_symbol_weight=max_symbol_weight)
        rebalance_counter += 1

        day_return = 0.0
        symbol_weights: collections.Counter[str] = collections.Counter()
        for name, weight in weights.items():
            point = maps[name][ts]
            symbol = by_name[name]["symbol"]
            symbol_weights[symbol] += weight
            day_return += weight * float(point["return"])
        points.append(
            {
                "timestamp_ms": ts,
                "base_return": day_return,
                "gross_weight": sum(abs(value) for value in weights.values()),
                "max_symbol_weight": max(symbol_weights.values(), default=0.0),
            }
        )
    return {"points": points, "streams": sorted(by_name), "symbols": sorted({stream["symbol"] for stream in streams})}


def portfolio_from_base_path(
    base_path: dict,
    streams: list[dict],
    allocation_quote: float,
    target_vol_pct: float,
    vol_lookback_days: int,
    dd_stop_pct: float,
    cooldown_days: int,
) -> dict:
    if allocation_quote <= 0:
        raise ValueError("allocation_quote must be positive")
    if allocation_quote >= 5000.0:
        raise ValueError("allocation_quote must stay below 5000")
    equity = float(allocation_quote)
    peak = equity
    cooldown_until = -1
    risk_events = 0
    portfolio_returns: list[float] = []
    points = []
    max_symbol_weight_observed = 0.0

    for base_point in base_path["points"]:
        ts = int(base_point["timestamp_ms"])
        in_cooldown = ts < cooldown_until
        scale = volatility_scale(portfolio_returns[-vol_lookback_days:], target_vol_pct=target_vol_pct)
        day_return = 0.0 if in_cooldown else float(base_point["base_return"]) * scale
        gross_weight = 0.0 if in_cooldown else float(base_point["gross_weight"]) * scale
        symbol_weight = 0.0 if in_cooldown else float(base_point["max_symbol_weight"]) * scale
        max_symbol_weight_observed = max(max_symbol_weight_observed, symbol_weight)
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
                "gross_weight": gross_weight,
                "max_symbol_weight": symbol_weight,
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
        "streams": list(base_path.get("streams", sorted(stream["name"] for stream in streams))),
        "symbols": symbols,
        "points": points,
        "metrics": metrics,
        "risk_events": risk_events,
        "live_parity_status": LIVE_PARITY_STATUS,
    }


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
    base_path = build_base_portfolio_path(
        streams,
        rebalance_days=rebalance_days,
        score_lookback_days=score_lookback_days,
        top_n=top_n,
        max_symbol_weight=max_symbol_weight,
    )
    return portfolio_from_base_path(
        base_path,
        streams,
        allocation_quote=allocation_quote,
        target_vol_pct=target_vol_pct,
        vol_lookback_days=vol_lookback_days,
        dd_stop_pct=dd_stop_pct,
        cooldown_days=cooldown_days,
    )


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


def build_stream_universe(
    market_data: str | Path,
    symbols: list[str],
    rules: list[str],
    fee_bps: float,
    slippage_bps: float,
    min_daily_bars: int,
) -> tuple[list[dict], list[str]]:
    streams = []
    rejections = []
    for symbol in symbols:
        daily = load_daily_bars(market_data, symbol, start_ms=hybrid.SEGMENTS["full"][0], end_ms=hybrid.SEGMENTS["full"][1])
        if len(daily) < min_daily_bars:
            rejections.append(f"{symbol}: only {len(daily)} daily bars < {min_daily_bars}")
            continue
        for rule in rules:
            try:
                stream = build_signal_stream(symbol, daily, rule, fee_bps=fee_bps, slippage_bps=slippage_bps)
            except ValueError as exc:
                rejections.append(f"{symbol}:{rule}: {exc}")
                continue
            if len(stream["points"]) < min_daily_bars // 2:
                rejections.append(f"{symbol}:{rule}: only {len(stream['points'])} return points")
                continue
            streams.append(stream)
    return streams, rejections


def summarize(rows: list[dict]) -> dict:
    summary = {}
    for profile in hybrid.PROFILE_TARGETS:
        subset = [row for row in rows if row["profile"] == profile]
        passes = [row for row in subset if row["pass"]]
        summary[profile] = {
            "rows": len(subset),
            "passes": len(passes),
            "best_ann": max(subset, key=lambda row: row["ann"], default=None),
            "best_dd": min(subset, key=lambda row: row["dd"], default=None),
            "passes_rows": passes[:20],
        }
    return summary


def run_search(args: argparse.Namespace) -> dict:
    profiles = parse_csv(args.profiles)
    symbols = parse_csv(args.symbols)
    rules = parse_csv(args.rules)
    allocation_quotes = parse_floats(args.allocation_quotes)
    rebalance_days_values = parse_ints(args.rebalance_days)
    score_lookbacks = parse_ints(args.score_lookbacks)
    top_ns = parse_ints(args.top_ns)
    target_vols = parse_floats(args.target_vols)
    vol_lookbacks = parse_ints(args.vol_lookbacks)
    dd_stops = parse_floats(args.dd_stops)
    cooldowns = parse_ints(args.cooldowns)
    streams, rejections = build_stream_universe(
        args.market_data,
        symbols,
        rules,
        fee_bps=args.fee_bps,
        slippage_bps=args.slippage_bps,
        min_daily_bars=args.min_daily_bars,
    )
    prepare_stream_cache(streams)
    rows = []
    base_paths: dict[tuple[int, int, int], dict] = {}
    ranking_cache: dict[tuple[int, int], list[dict]] = {}
    configs = itertools.product(
        profiles,
        allocation_quotes,
        rebalance_days_values,
        score_lookbacks,
        top_ns,
        target_vols,
        vol_lookbacks,
        dd_stops,
        cooldowns,
    )
    for profile, allocation, rebalance_days, score_lookback, top_n, target_vol, vol_lookback, dd_stop, cooldown in configs:
        if allocation >= args.budget:
            continue
        config = {
            "top_n": top_n,
            "rebalance_days": rebalance_days,
            "score_lookback_days": score_lookback,
            "target_vol_pct": target_vol,
            "vol_lookback_days": vol_lookback,
            "dd_stop_pct": dd_stop,
            "cooldown_days": cooldown,
            "allocation_quote": allocation,
            "fee_bps": args.fee_bps,
            "slippage_bps": args.slippage_bps,
        }
        base_key = (rebalance_days, score_lookback, top_n)
        if base_key not in base_paths:
            base_paths[base_key] = build_base_portfolio_path(
                streams,
                rebalance_days=rebalance_days,
                score_lookback_days=score_lookback,
                top_n=top_n,
                max_symbol_weight=args.max_symbol_weight,
                ranking_cache=ranking_cache,
            )
        portfolio = portfolio_from_base_path(
            base_paths[base_key],
            streams,
            allocation_quote=allocation,
            target_vol_pct=target_vol,
            vol_lookback_days=vol_lookback,
            dd_stop_pct=dd_stop,
            cooldown_days=cooldown,
        )
        report = build_candidate_report(
            profile,
            portfolio,
            budget=args.budget,
            max_symbol_weight=args.max_symbol_weight,
            config=config,
        )
        rows.append(row_from_report(report))
        if len(rows) >= args.limit:
            break
    return {
        "live_parity_status": LIVE_PARITY_STATUS,
        "stream_count": len(streams),
        "rejections": rejections,
        "rows": rows,
        "summary": summarize(rows),
    }


def format_row(row: dict | None) -> str:
    if not row:
        return "`None`"
    return (
        f"symbols `{row['symbols']}` ann `{row['ann']:.2f}` DD `{row['dd']:.2f}` "
        f"cap `{row['cap']:.2f}` pos `{row['pos']}/5` 2024-2026 `{row['c2426']:.2f}` "
        f"top_n `{row['top_n']}` rebalance `{row['rebalance_days']}` vol `{row['target_vol_pct']}` "
        f"dd_stop `{row['dd_stop_pct']}` cooldown `{row['cooldown_days']}` "
        f"events `{row['risk_events']}` pass `{row['pass']}`"
    )


def write_outputs(result: dict, out_json: str | Path, out_md: str | Path) -> None:
    Path(out_json).write_text(json.dumps(result, indent=2, sort_keys=True))
    lines = [
        "# 2026-07-01 Dynamic Breakout/Trend Probe",
        "",
        "This is a research-only dynamic breakout/trend portfolio check. It does not trade, touch Binance, flyingkid, live mode, or real funds.",
        "",
        f"- live_parity_status: `{result['live_parity_status']}`",
        f"- streams: `{result.get('stream_count', 0)}`",
        f"- rows: `{len(result['rows'])}`",
        f"- rejected streams: `{len(result.get('rejections', []))}`",
        "",
    ]
    for profile, item in result["summary"].items():
        lines.append(f"## {profile}")
        lines.append("")
        lines.append(f"- passes: `{item['passes']}`")
        lines.append(f"- best_ann: {format_row(item['best_ann'])}")
        lines.append(f"- best_dd: {format_row(item['best_dd'])}")
        lines.append("")
    lines.append("## Conclusion")
    lines.append("")
    total_passes = sum(item["passes"] for item in result["summary"].values())
    if total_passes:
        lines.append(
            f"Potential research-only dynamic trend passes found: `{total_passes}`. "
            "These require manual replay and a separate live-parity promotion design before any trading conclusion."
        )
    else:
        lines.append("Potential research-only dynamic trend passes found: `0` under this scan. This does not change the martingale/grid verdict.")
    Path(out_md).write_text("\n".join(lines) + "\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profiles", default="conservative,balanced,aggressive")
    parser.add_argument("--market-data", default="data/market_data_full.db")
    parser.add_argument("--symbols", default="BTCUSDT,ETHUSDT,BNBUSDT,SOLUSDT,INJUSDT,AAVEUSDT,LINKUSDT,DOGEUSDT,ADAUSDT,XRPUSDT")
    parser.add_argument("--rules", default="donchian20_lf,donchian20_ls,donchian60_lf,donchian60_ls,mom20_lf,mom20_ls,mom60_lf,mom60_ls,ema20_50_lf,ema50_200_lf")
    parser.add_argument("--allocation-quotes", default="1000,2000,3000,4000")
    parser.add_argument("--rebalance-days", default="7,30")
    parser.add_argument("--score-lookbacks", default="63,126,252")
    parser.add_argument("--top-ns", default="2,3,4,6")
    parser.add_argument("--target-vols", default="15,20,30,40")
    parser.add_argument("--vol-lookbacks", default="20,60")
    parser.add_argument("--dd-stops", default="10,20,30")
    parser.add_argument("--cooldowns", default="0,15,30")
    parser.add_argument("--max-symbol-weight", type=float, default=0.5)
    parser.add_argument("--fee-bps", type=float, default=2.0)
    parser.add_argument("--slippage-bps", type=float, default=2.0)
    parser.add_argument("--min-daily-bars", type=int, default=600)
    parser.add_argument("--budget", type=float, default=5000.0)
    parser.add_argument("--limit", type=int, default=20736)
    parser.add_argument("--out-json", required=True)
    parser.add_argument("--out-md", required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    result = run_search(args)
    write_outputs(result, args.out_json, args.out_md)
    print(json.dumps({"rows": len(result["rows"]), "passes": sum(item["passes"] for item in result["summary"].values())}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
