# Dynamic Breakout/Trend Probe Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a research-only dynamic breakout/trend portfolio probe that tests whether a non-martingale return source deserves later live-parity work.

**Architecture:** Add one isolated Python script that loads local 1m futures bars, compresses them to UTC daily OHLC, builds no-lookahead breakout/trend return streams, dynamically ranks streams, applies top-N selection, symbol weight caps, volatility targeting, and portfolio DD cooldown. Reuse the existing hybrid frontier metrics and C/B/A gate helpers for return, drawdown, segment, and budget checks. Keep every output marked `research_only`; live trading, Binance, flyingkid, Rust engines, and live-parity promotion are out of scope.

**Tech Stack:** Python 3 standard library (`argparse`, `collections`, `dataclasses`, `importlib.util`, `itertools`, `json`, `math`, `sqlite3`, `statistics`, `pathlib`), `unittest`, local SQLite `data/market_data_full.db`, markdown/JSON reports.

---

## File Map

- Create `scripts/dynamic_breakout_trend_probe.py`
  - Owns market loading, daily OHLC compression, signal stream construction, rolling ranking, top-N selection, symbol caps, volatility targeting, DD stop/cooldown, gate evaluation, CLI search, and JSON/Markdown output.
- Create `tests/verification/test_dynamic_breakout_trend_probe.py`
  - Owns deterministic unit tests for no-lookahead signals, ranking, weight controls, volatility controls, DD cooldown, gate behavior, and `research_only` output.
- Create `docs/superpowers/reports/2026-07-01-dynamic-breakout-trend-probe.md`
  - Final bounded research report produced by the new script.

No production trading file, Binance credential path, flyingkid output path, Rust engine, or live configuration is modified in this plan.

---

## Task 1: Daily Bars And No-Lookahead Signal Streams

**Files:**
- Create: `scripts/dynamic_breakout_trend_probe.py`
- Create: `tests/verification/test_dynamic_breakout_trend_probe.py`

- [ ] **Step 1: Write failing tests for daily compression and next-day execution**

Create `tests/verification/test_dynamic_breakout_trend_probe.py`:

```python
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path("scripts/dynamic_breakout_trend_probe.py")
SPEC = importlib.util.spec_from_file_location("dynamic_trend", SCRIPT)
dynamic = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = dynamic
SPEC.loader.exec_module(dynamic)


DAY = 86_400_000


def bar(day_index, open_, high, low, close):
    return {
        "timestamp_ms": day_index * DAY,
        "open": float(open_),
        "high": float(high),
        "low": float(low),
        "close": float(close),
        "volume": 1.0,
    }


def stream(name, symbol, returns):
    return {
        "name": name,
        "symbol": symbol,
        "rule": "test",
        "points": [
            {
                "timestamp_ms": index * DAY,
                "return": float(value),
                "position": 1,
                "strength": abs(float(value)),
            }
            for index, value in enumerate(returns)
        ],
        "live_parity_status": "research_only",
    }


class DynamicBreakoutTrendProbeTest(unittest.TestCase):
    def test_compress_daily_ohlc_is_deterministic(self):
        rows = [
            {"timestamp_ms": 60_000, "open": 100, "high": 102, "low": 99, "close": 101, "volume": 3},
            {"timestamp_ms": 120_000, "open": 101, "high": 105, "low": 100, "close": 104, "volume": 4},
            {"timestamp_ms": DAY + 60_000, "open": 110, "high": 112, "low": 108, "close": 111, "volume": 5},
            {"timestamp_ms": DAY + 120_000, "open": 111, "high": 113, "low": 107, "close": 109, "volume": 6},
        ]

        daily = dynamic.compress_daily_ohlc(rows)

        self.assertEqual(
            daily,
            [
                {"timestamp_ms": 0, "open": 100.0, "high": 105.0, "low": 99.0, "close": 104.0, "volume": 7.0},
                {"timestamp_ms": DAY, "open": 110.0, "high": 113.0, "low": 107.0, "close": 109.0, "volume": 11.0},
            ],
        )

    def test_momentum_stream_uses_previous_completed_day_for_next_day_position(self):
        daily = [
            bar(0, 100, 100, 100, 100),
            bar(1, 90, 90, 90, 90),
            bar(2, 110, 110, 110, 110),
            bar(3, 120, 120, 120, 120),
        ]

        result = dynamic.build_signal_stream(
            "BTCUSDT",
            daily,
            "mom1_lf",
            fee_bps=0.0,
            slippage_bps=0.0,
        )

        first = result["points"][0]
        self.assertEqual(first["timestamp_ms"], 2 * DAY)
        self.assertEqual(first["position"], 0)
        self.assertEqual(first["return"], 0.0)
        self.assertEqual(result["live_parity_status"], "research_only")

    def test_donchian_stream_channel_excludes_signal_day(self):
        daily = [
            bar(0, 100, 100, 100, 100),
            bar(1, 101, 101, 101, 101),
            bar(2, 103, 103, 103, 103),
            bar(3, 99, 99, 99, 99),
        ]

        result = dynamic.build_signal_stream(
            "ETHUSDT",
            daily,
            "donchian2_lf",
            fee_bps=0.0,
            slippage_bps=0.0,
        )

        self.assertEqual(result["points"][0]["timestamp_ms"], 3 * DAY)
        self.assertEqual(result["points"][0]["position"], 1)
        self.assertLess(result["points"][0]["return"], 0.0)
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
python3 -m unittest tests/verification/test_dynamic_breakout_trend_probe.py
```

Expected: fail because `scripts/dynamic_breakout_trend_probe.py` does not exist.

- [ ] **Step 3: Implement daily compression and signal stream builders**

Create `scripts/dynamic_breakout_trend_probe.py` with this foundation:

```python
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


def desired_position(spec: RuleSpec, daily: list[dict], index: int, fast_ema: list[float | None], slow_ema: list[float | None]) -> tuple[int, float]:
    signal_index = index - 1
    if spec.family == "momentum":
        lookback = int(spec.lookback or 0)
        ref_index = signal_index - lookback
        if ref_index < 0:
            return 0, 0.0
        ref_close = float(daily[ref_index]["close"])
        signal_close = float(daily[signal_index]["close"])
        momentum = signal_close / ref_close - 1.0 if ref_close > 0 else 0.0
        position = 1 if momentum > 0 else (-1 if spec.mode == "long_short" and momentum < 0 else 0)
        return position, abs(momentum)
    if spec.family == "donchian":
        lookback = int(spec.lookback or 0)
        window_start = signal_index - lookback
        if window_start < 0:
            return 0, 0.0
        window = [float(row["close"]) for row in daily[window_start:signal_index]]
        if not window:
            return 0, 0.0
        upper = max(window)
        lower = min(window)
        signal_close = float(daily[signal_index]["close"])
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
    for index in range(1, len(daily)):
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
```

- [ ] **Step 4: Run tests and compile**

Run:

```bash
python3 -m unittest tests/verification/test_dynamic_breakout_trend_probe.py
python3 -m py_compile scripts/dynamic_breakout_trend_probe.py
```

Expected: all tests pass and compile succeeds.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add scripts/dynamic_breakout_trend_probe.py tests/verification/test_dynamic_breakout_trend_probe.py
git commit -m "feat: 修复思路 增加动态趋势信号流"
```

---

## Task 2: Rolling Ranking, Top-N Selection, Weight Caps, And Volatility Target

**Files:**
- Modify: `scripts/dynamic_breakout_trend_probe.py`
- Modify: `tests/verification/test_dynamic_breakout_trend_probe.py`

- [ ] **Step 1: Add failing tests for ranking and portfolio controls**

Append these tests inside `DynamicBreakoutTrendProbeTest`:

```python
    def test_rank_streams_excludes_current_and_future_returns(self):
        weak_future = stream("trend:BTCUSDT:test", "BTCUSDT", [-0.02, -0.02, 0.80, 0.80])
        steady_past = stream("trend:ETHUSDT:test", "ETHUSDT", [0.03, 0.02, -0.10, -0.10])

        ranked = dynamic.rank_streams([weak_future, steady_past], as_of_ts=2 * DAY, lookback_days=10)

        self.assertEqual(ranked[0]["name"], "trend:ETHUSDT:test")
        self.assertGreater(ranked[0]["score"], ranked[1]["score"])

    def test_select_top_streams_enforces_two_symbols_when_possible(self):
        ranked = [
            {"name": "trend:BTCUSDT:a", "symbol": "BTCUSDT", "score": 10.0},
            {"name": "trend:BTCUSDT:b", "symbol": "BTCUSDT", "score": 9.0},
            {"name": "trend:ETHUSDT:a", "symbol": "ETHUSDT", "score": 8.0},
        ]

        selected = dynamic.select_top_streams(ranked, top_n=2, max_symbol_weight=0.5, min_symbols=2)

        self.assertEqual([item["name"] for item in selected], ["trend:BTCUSDT:a", "trend:ETHUSDT:a"])
        self.assertEqual({item["symbol"] for item in selected}, {"BTCUSDT", "ETHUSDT"})

    def test_capped_equal_weights_rejects_single_symbol_concentration(self):
        selected = [
            {"name": "trend:BTCUSDT:a", "symbol": "BTCUSDT", "score": 10.0},
            {"name": "trend:ETHUSDT:a", "symbol": "ETHUSDT", "score": 9.0},
            {"name": "trend:SOLUSDT:a", "symbol": "SOLUSDT", "score": 8.0},
        ]

        weights = dynamic.capped_equal_weights(selected, max_symbol_weight=0.5)

        self.assertAlmostEqual(sum(weights.values()), 1.0)
        self.assertLessEqual(weights["trend:BTCUSDT:a"], 0.5)
        self.assertLessEqual(weights["trend:ETHUSDT:a"], 0.5)
        self.assertLessEqual(weights["trend:SOLUSDT:a"], 0.5)

    def test_volatility_target_scales_down_high_realized_volatility(self):
        returns = [0.10, -0.10, 0.08, -0.08, 0.09, -0.09]

        scale = dynamic.volatility_scale(returns, target_vol_pct=20.0)

        self.assertGreater(scale, 0.0)
        self.assertLess(scale, 1.0)
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
python3 -m unittest tests/verification/test_dynamic_breakout_trend_probe.py
```

Expected: fail because ranking, selection, weights, and volatility helpers are missing.

- [ ] **Step 3: Implement ranking and risk sizing helpers**

Append these functions to `scripts/dynamic_breakout_trend_probe.py`:

```python
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
    for symbol, names in by_symbol.items():
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
```

- [ ] **Step 4: Run tests and compile**

Run:

```bash
python3 -m unittest tests/verification/test_dynamic_breakout_trend_probe.py
python3 -m py_compile scripts/dynamic_breakout_trend_probe.py
```

Expected: all tests pass and compile succeeds.

- [ ] **Step 5: Commit Task 2**

Run:

```bash
git add scripts/dynamic_breakout_trend_probe.py tests/verification/test_dynamic_breakout_trend_probe.py
git commit -m "feat: 修复思路 增加动态趋势组合选择"
```

---

## Task 3: Dynamic Portfolio Simulation, DD Cooldown, And Gates

**Files:**
- Modify: `scripts/dynamic_breakout_trend_probe.py`
- Modify: `tests/verification/test_dynamic_breakout_trend_probe.py`

- [ ] **Step 1: Add failing tests for DD cooldown and gate rejection**

Append these tests inside `DynamicBreakoutTrendProbeTest`:

```python
    def test_dynamic_portfolio_dd_stop_freezes_exposure_and_records_event(self):
        streams = [
            stream("trend:BTCUSDT:a", "BTCUSDT", [0.05, 0.05, -0.30, 0.40, 0.10]),
            stream("trend:ETHUSDT:a", "ETHUSDT", [0.04, 0.04, -0.20, 0.30, 0.10]),
        ]

        portfolio = dynamic.build_dynamic_portfolio(
            streams,
            allocation_quote=3000.0,
            rebalance_days=1,
            score_lookback_days=2,
            top_n=2,
            max_symbol_weight=0.5,
            target_vol_pct=100.0,
            vol_lookback_days=2,
            dd_stop_pct=10.0,
            cooldown_days=2,
        )

        self.assertEqual(portfolio["risk_events"], 1)
        frozen_points = [point for point in portfolio["points"] if point["in_cooldown"]]
        self.assertGreaterEqual(len(frozen_points), 1)
        self.assertEqual(frozen_points[0]["gross_weight"], 0.0)
        self.assertEqual(portfolio["live_parity_status"], "research_only")

    def test_gate_evaluation_rejects_high_drawdown_even_with_high_return(self):
        points = [
            {"timestamp_ms": dynamic.hybrid.SEGMENTS["full"][0], "equity_quote": 3000.0},
            {"timestamp_ms": dynamic.hybrid.SEGMENTS["full"][0] + DAY, "equity_quote": 6000.0},
            {"timestamp_ms": dynamic.hybrid.SEGMENTS["full"][0] + 2 * DAY, "equity_quote": 3600.0},
        ]
        portfolio = {
            "streams": ["trend:BTCUSDT:a", "trend:ETHUSDT:a"],
            "symbols": ["BTCUSDT", "ETHUSDT"],
            "points": points,
            "metrics": {
                **dynamic.hybrid.compute_metrics(points),
                "max_capital_used_quote": 3000.0,
                "budget_blocked_events": 0,
                "symbol_count": 2,
                "max_symbol_weight_observed": 0.5,
                "risk_events": 0,
            },
            "live_parity_status": "research_only",
        }

        report = dynamic.build_candidate_report("conservative", portfolio, budget=5000.0, max_symbol_weight=0.5)

        self.assertFalse(report["passes_offline"])
        self.assertIn("drawdown", " ".join(report["full_gate"]["violations"]))

    def test_row_from_report_preserves_research_only_and_segment_fields(self):
        report = {
            "profile": "balanced",
            "live_parity_status": "research_only",
            "streams": ["trend:BTCUSDT:a", "trend:ETHUSDT:a"],
            "symbols": ["BTCUSDT", "ETHUSDT"],
            "full_metrics": {
                "annualized_return_pct": 95.0,
                "max_drawdown_pct": 15.0,
                "max_capital_used_quote": 3000.0,
                "max_symbol_weight_observed": 0.5,
                "risk_events": 1,
            },
            "segment_gate": {
                "passes": True,
                "positive_segments": 4,
                "combined_2024_2026_return_pct": 12.0,
                "violations": [],
            },
            "full_gate": {"passes": True, "violations": []},
            "passes_offline": True,
            "config": {"top_n": 2, "rebalance_days": 7},
        }

        row = dynamic.row_from_report(report)

        self.assertEqual(row["live_parity_status"], "research_only")
        self.assertEqual(row["symbol_count"], 2)
        self.assertEqual(row["pos"], 4)
        self.assertTrue(row["pass"])
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
python3 -m unittest tests/verification/test_dynamic_breakout_trend_probe.py
```

Expected: fail because portfolio simulation and report helpers are missing.

- [ ] **Step 3: Implement dynamic portfolio and candidate report helpers**

Append these functions to `scripts/dynamic_breakout_trend_probe.py`:

```python
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


def build_candidate_report(profile: str, portfolio: dict, budget: float, max_symbol_weight: float, config: dict | None = None) -> dict:
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
```

- [ ] **Step 4: Run tests and compile**

Run:

```bash
python3 -m unittest tests/verification/test_dynamic_breakout_trend_probe.py
python3 -m py_compile scripts/dynamic_breakout_trend_probe.py
```

Expected: all tests pass and compile succeeds.

- [ ] **Step 5: Commit Task 3**

Run:

```bash
git add scripts/dynamic_breakout_trend_probe.py tests/verification/test_dynamic_breakout_trend_probe.py
git commit -m "feat: 修复思路 增加动态趋势风险门禁"
```

---

## Task 4: CLI Search And Report Writer

**Files:**
- Modify: `scripts/dynamic_breakout_trend_probe.py`
- Modify: `tests/verification/test_dynamic_breakout_trend_probe.py`

- [ ] **Step 1: Add failing tests for summary and output status**

Append these tests inside `DynamicBreakoutTrendProbeTest`:

```python
    def test_summarize_counts_profile_passes(self):
        rows = [
            {"profile": "conservative", "pass": False, "ann": 40.0, "dd": 8.0},
            {"profile": "conservative", "pass": True, "ann": 55.0, "dd": 9.0},
            {"profile": "balanced", "pass": False, "ann": 80.0, "dd": 18.0},
        ]

        summary = dynamic.summarize(rows)

        self.assertEqual(summary["conservative"]["rows"], 2)
        self.assertEqual(summary["conservative"]["passes"], 1)
        self.assertEqual(summary["balanced"]["passes"], 0)

    def test_write_outputs_marks_research_only(self):
        result = {
            "live_parity_status": "research_only",
            "rows": [
                {
                    "profile": "conservative",
                    "pass": False,
                    "ann": 40.0,
                    "dd": 8.0,
                    "cap": 3000.0,
                    "pos": 4,
                    "c2426": 2.0,
                    "symbols": "BTCUSDT,ETHUSDT",
                    "top_n": 2,
                    "rebalance_days": 7,
                    "target_vol_pct": 20.0,
                    "dd_stop_pct": 10.0,
                    "cooldown_days": 15,
                    "risk_events": 0,
                }
            ],
            "summary": {
                "conservative": {"rows": 1, "passes": 0, "best_ann": None, "best_dd": None, "passes_rows": []},
                "balanced": {"rows": 0, "passes": 0, "best_ann": None, "best_dd": None, "passes_rows": []},
                "aggressive": {"rows": 0, "passes": 0, "best_ann": None, "best_dd": None, "passes_rows": []},
            },
        }
        with tempfile.TemporaryDirectory() as tmp:
            out_json = Path(tmp) / "out.json"
            out_md = Path(tmp) / "out.md"

            dynamic.write_outputs(result, out_json, out_md)

            payload = json.loads(out_json.read_text())
            text = out_md.read_text()
            self.assertEqual(payload["live_parity_status"], "research_only")
            self.assertIn("research-only", text)
            self.assertIn("live_parity_status: `research_only`", text)
            self.assertNotIn("live_parity_passed", text)
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
python3 -m unittest tests/verification/test_dynamic_breakout_trend_probe.py
```

Expected: fail because summary, CLI search, and output helpers are missing.

- [ ] **Step 3: Implement search, summary, report writing, and CLI**

Append these functions to `scripts/dynamic_breakout_trend_probe.py`:

```python
def build_stream_universe(market_data: str | Path, symbols: list[str], rules: list[str], fee_bps: float, slippage_bps: float, min_daily_bars: int) -> tuple[list[dict], list[str]]:
    streams = []
    rejections = []
    for symbol in symbols:
        daily = load_daily_bars(market_data, symbol)
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
    rows = []
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
        portfolio = build_dynamic_portfolio(
            streams,
            allocation_quote=allocation,
            rebalance_days=rebalance_days,
            score_lookback_days=score_lookback,
            top_n=top_n,
            max_symbol_weight=args.max_symbol_weight,
            target_vol_pct=target_vol,
            vol_lookback_days=vol_lookback,
            dd_stop_pct=dd_stop,
            cooldown_days=cooldown,
        )
        report = build_candidate_report(profile, portfolio, budget=args.budget, max_symbol_weight=args.max_symbol_weight, config=config)
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
        lines.append(f"Potential research-only dynamic trend passes found: `{total_passes}`. These require manual replay and a separate live-parity promotion design before any trading conclusion.")
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
    parser.add_argument("--limit", type=int, default=8000)
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
```

- [ ] **Step 4: Run tests and compile**

Run:

```bash
python3 -m unittest tests/verification/test_dynamic_breakout_trend_probe.py
python3 -m py_compile scripts/dynamic_breakout_trend_probe.py
```

Expected: all tests pass and compile succeeds.

- [ ] **Step 5: Commit Task 4**

Run:

```bash
git add scripts/dynamic_breakout_trend_probe.py tests/verification/test_dynamic_breakout_trend_probe.py
git commit -m "feat: 修复思路 增加动态趋势探针报告"
```

---

## Task 5: Bounded Search, Report, And Final Verification

**Files:**
- Create: `docs/superpowers/reports/2026-07-01-dynamic-breakout-trend-probe.md`
- Modify: `scripts/dynamic_breakout_trend_probe.py`
- Modify: `tests/verification/test_dynamic_breakout_trend_probe.py`

- [ ] **Step 1: Run the bounded offline scan**

Run:

```bash
python3 scripts/dynamic_breakout_trend_probe.py \
  --profiles conservative,balanced,aggressive \
  --market-data data/market_data_full.db \
  --symbols BTCUSDT,ETHUSDT,BNBUSDT,SOLUSDT,INJUSDT,AAVEUSDT,LINKUSDT,DOGEUSDT,ADAUSDT,XRPUSDT \
  --rules donchian20_lf,donchian20_ls,donchian60_lf,donchian60_ls,mom20_lf,mom20_ls,mom60_lf,mom60_ls,ema20_50_lf,ema50_200_lf \
  --allocation-quotes 1000,2000,3000,4000 \
  --rebalance-days 7,30 \
  --score-lookbacks 63,126,252 \
  --top-ns 2,3,4,6 \
  --target-vols 15,20,30,40 \
  --vol-lookbacks 20,60 \
  --dd-stops 10,20,30 \
  --cooldowns 0,15,30 \
  --max-symbol-weight 0.5 \
  --fee-bps 2 \
  --slippage-bps 2 \
  --min-daily-bars 600 \
  --budget 5000 \
  --limit 8000 \
  --out-json /tmp/dynamic_breakout_trend_probe.json \
  --out-md docs/superpowers/reports/2026-07-01-dynamic-breakout-trend-probe.md
```

Expected: exits 0 and prints JSON containing `rows` and `passes`. The markdown report contains `live_parity_status: research_only` and explicitly says it does not trade or touch Binance/flyingkid/live funds.

- [ ] **Step 2: Verify unit tests, compile, report wording, and whitespace**

Run:

```bash
python3 -m unittest tests/verification/test_dynamic_breakout_trend_probe.py tests/verification/test_trend_sleeve_frontier_probe.py tests/verification/test_trend_risk_control_probe.py
python3 -m py_compile scripts/dynamic_breakout_trend_probe.py scripts/trend_sleeve_frontier_probe.py scripts/trend_risk_control_probe.py
rg -n "live_parity_passed|touch Binance|real funds" scripts/dynamic_breakout_trend_probe.py tests/verification/test_dynamic_breakout_trend_probe.py docs/superpowers/reports/2026-07-01-dynamic-breakout-trend-probe.md
git diff --check
```

Expected:
- unittest reports `OK`;
- compile exits 0;
- `rg` shows the intended safety wording and no `live_parity_passed`;
- `git diff --check` exits 0.

- [ ] **Step 3: Read the generated result and state the decision**

Run:

```bash
python3 - <<'PY'
import json
from pathlib import Path
data = json.loads(Path("/tmp/dynamic_breakout_trend_probe.json").read_text())
print(json.dumps({
    "rows": len(data["rows"]),
    "passes": {k: v["passes"] for k, v in data["summary"].items()},
    "live_parity_status": data["live_parity_status"],
}, sort_keys=True))
PY
```

Expected: prints the exact number of rows and pass counts. If any pass count is nonzero, write no live conclusion; the next artifact is a separate live-parity promotion design for the exact rule family. If all pass counts are zero, the report conclusion remains that this dynamic trend probe did not change the martingale/grid verdict.

- [ ] **Step 4: Commit final probe results**

Run:

```bash
git add scripts/dynamic_breakout_trend_probe.py tests/verification/test_dynamic_breakout_trend_probe.py docs/superpowers/reports/2026-07-01-dynamic-breakout-trend-probe.md
git commit -m "docs: 修复思路 验证动态突破趋势组合探针"
```

---

## Self-Review

- Spec coverage: Task 1 covers local SQLite daily compression and no-lookahead Donchian, momentum, and EMA stream generation; Task 2 covers rolling past-only ranking, top-N selection, symbol caps, and volatility targeting; Task 3 covers DD stop/cooldown, C/B/A gates, segment gates, and `research_only` status; Task 4 covers CLI and JSON/Markdown outputs; Task 5 covers the bounded offline run and verification.
- Red-flag scan: plan text provides exact files, function names, test cases, commands, expected failures, expected passes, and commit messages.
- Type consistency: stream fields use `name`, `symbol`, `symbols`, `rule`, `points`, `return`, `position`, `strength`, and `live_parity_status` throughout; row fields use `profile`, `ann`, `dd`, `cap`, `symbol_count`, `symbols`, `streams`, `pos`, `c2426`, `risk_events`, `max_symbol_weight`, `top_n`, `rebalance_days`, `target_vol_pct`, `dd_stop_pct`, `cooldown_days`, `pass`, `full_pass`, `seg_pass`, `violations`, and `live_parity_status` throughout.
