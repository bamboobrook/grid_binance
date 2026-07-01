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
