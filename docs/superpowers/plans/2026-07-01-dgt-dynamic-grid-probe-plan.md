# DGT Dynamic Grid Probe Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a research-only DGT dynamic grid probe that can test multi-symbol DGT portfolios against the original C/B/A gates without touching live trading.

**Architecture:** Add one isolated Python script for DGT simulation, portfolio combination, gate evaluation, CLI search, and report writing. Add focused Python unit tests for DGT accounting and gate behavior. Keep all outputs marked `research_only`; promotion to live parity is out of scope.

**Tech Stack:** Python 3 standard library (`argparse`, `itertools`, `json`, `math`, `sqlite3`, `pathlib`), `unittest`, local SQLite `data/market_data_full.db`, markdown/JSON reports.

---

## File Map

- Create `scripts/dgt_dynamic_grid_probe.py`
  - Owns DGT simulation, market loading, portfolio combination, metrics, gates, CLI, JSON/markdown output.
- Create `tests/verification/test_dgt_dynamic_grid_probe.py`
  - Owns deterministic unit tests for reset accounting, budget gates, segment splitting, and report status.
- Create `docs/superpowers/reports/2026-07-01-dgt-dynamic-grid-probe.md`
  - Search result report after implementation and bounded search.

Do not modify Rust engine or live trading code in this phase.

---

## Task 1: Core DGT Accounting

**Files:**
- Create: `scripts/dgt_dynamic_grid_probe.py`
- Create: `tests/verification/test_dgt_dynamic_grid_probe.py`

- [ ] **Step 1: Write failing tests for DGT reset accounting**

Create `tests/verification/test_dgt_dynamic_grid_probe.py`:

```python
import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path("scripts/dgt_dynamic_grid_probe.py")
SPEC = importlib.util.spec_from_file_location("dgt_probe", SCRIPT)
dgt = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(dgt)


class DgtDynamicGridProbeTest(unittest.TestCase):
    def test_simulate_symbol_tracks_downside_topup_and_research_only(self):
        bars = [
            {"timestamp_ms": 0, "open": 100.0, "high": 100.0, "low": 100.0, "close": 100.0},
            {"timestamp_ms": 86_400_000, "open": 100.0, "high": 101.0, "low": 86.0, "close": 90.0},
            {"timestamp_ms": 2 * 86_400_000, "open": 90.0, "high": 98.0, "low": 89.0, "close": 96.0},
        ]
        stream = dgt.simulate_dgt_symbol(
            "BTCUSDT",
            bars,
            principal_quote=100.0,
            grid_spacing=0.05,
            half_grid_count=1,
            fee_bps=0.0,
        )
        self.assertEqual(stream["name"], "dgt:BTCUSDT:gs0.05:h1:p100.0")
        self.assertEqual(stream["live_parity_status"], "research_only")
        self.assertGreater(stream["reset_count"], 0)
        self.assertGreater(stream["max_input_quote"], 100.0)
        self.assertEqual(len(stream["points"]), 3)

    def test_simulate_symbol_tracks_upside_reset_without_external_topup(self):
        bars = [
            {"timestamp_ms": 0, "open": 100.0, "high": 100.0, "low": 100.0, "close": 100.0},
            {"timestamp_ms": 86_400_000, "open": 100.0, "high": 112.0, "low": 99.0, "close": 110.0},
            {"timestamp_ms": 2 * 86_400_000, "open": 110.0, "high": 116.0, "low": 108.0, "close": 114.0},
        ]
        stream = dgt.simulate_dgt_symbol(
            "BTCUSDT",
            bars,
            principal_quote=100.0,
            grid_spacing=0.05,
            half_grid_count=1,
            fee_bps=0.0,
        )
        self.assertGreater(stream["reset_count"], 0)
        self.assertEqual(round(stream["max_input_quote"], 6), 100.0)
        self.assertGreater(stream["points"][-1]["equity_quote"], stream["points"][0]["equity_quote"])
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
python3 -m unittest tests/verification/test_dgt_dynamic_grid_probe.py
```

Expected: FAIL because `scripts/dgt_dynamic_grid_probe.py` does not exist.

- [ ] **Step 3: Implement minimal DGT accounting**

Create `scripts/dgt_dynamic_grid_probe.py`:

```python
#!/usr/bin/env python3
"""Research-only Dynamic Grid Trading probe for martingale frontier exploration."""
from __future__ import annotations

import argparse
import itertools
import json
import math
import sqlite3
from pathlib import Path

MS_PER_DAY = 86_400_000
LIVE_PARITY_STATUS = "research_only"

SEGMENTS = {
    "h1_2023": (1672531200000, 1688169599999),
    "h2_2023": (1688169600000, 1704067199999),
    "2024": (1704067200000, 1735689599999),
    "2025": (1735689600000, 1767225599999),
    "2026_ytd": (1767225600000, 1780271999999),
    "full": (1672531200000, 1780271999999),
}

PROFILE_TARGETS = {
    "conservative": {"ann": 50.0, "dd": 10.0},
    "balanced": {"ann": 90.0, "dd": 20.0},
    "aggressive": {"ann": 110.0, "dd": 30.0},
}


def grid_levels(center_price: float, spacing: float, half_grid_count: int) -> list[float]:
    if center_price <= 0:
        raise ValueError("center_price must be positive")
    if spacing <= 0:
        raise ValueError("spacing must be positive")
    if half_grid_count <= 0:
        raise ValueError("half_grid_count must be positive")
    return [
        center_price * (1.0 - spacing * step)
        for step in range(half_grid_count, 0, -1)
    ] + [center_price] + [
        center_price * (1.0 + spacing * step)
        for step in range(1, half_grid_count + 1)
    ]


def simulate_dgt_symbol(
    symbol: str,
    bars: list[dict],
    principal_quote: float,
    grid_spacing: float,
    half_grid_count: int,
    fee_bps: float = 8.0,
) -> dict:
    if not bars:
        raise ValueError(f"no bars for {symbol}")
    if principal_quote <= 0:
        raise ValueError("principal_quote must be positive")
    fee_rate = fee_bps / 10_000.0
    grid_count = half_grid_count * 2
    center_price = float(bars[0]["open"])
    levels = grid_levels(center_price, grid_spacing, half_grid_count)
    lower = levels[0]
    upper = levels[-1]
    cursor = half_grid_count
    traversals = 0
    reset_count = 0
    usdt_wallet = 0.0
    inventory_qty = 0.0
    active_principal = principal_quote
    total_input_quote = principal_quote
    max_input_quote = principal_quote
    total_fee_quote = 0.0
    points = []
    last_close = center_price

    def equity(price: float) -> float:
        return usdt_wallet + inventory_qty * price + active_principal

    def fund_next_grid() -> None:
        nonlocal usdt_wallet, total_input_quote, max_input_quote
        if usdt_wallet >= principal_quote:
            usdt_wallet -= principal_quote
        else:
            needed = principal_quote - usdt_wallet
            total_input_quote += needed
            max_input_quote = max(max_input_quote, total_input_quote)
            usdt_wallet = 0.0

    def reset_grid(price: float) -> None:
        nonlocal center_price, levels, lower, upper, cursor, traversals, reset_count
        center_price = price
        levels = grid_levels(center_price, grid_spacing, half_grid_count)
        lower = levels[0]
        upper = levels[-1]
        cursor = half_grid_count
        traversals = 0
        reset_count += 1

    def arbitrage_profit(extra_traversals: int) -> float:
        gross = max(0.0, extra_traversals) * (principal_quote / grid_count) * grid_spacing
        fees = max(0.0, extra_traversals) * (principal_quote / grid_count) * fee_rate * 2.0
        return gross - fees

    for index, bar in enumerate(bars):
        path = [float(bar["open"]), float(bar["low"]), float(bar["high"]), float(bar["close"])]
        if index > 0:
            path[0] = last_close
        for start_price, end_price in zip(path, path[1:]):
            if start_price < end_price:
                while cursor < grid_count and start_price <= levels[cursor + 1] < end_price:
                    cursor += 1
                    traversals += 1
            elif start_price > end_price:
                while cursor > 0 and end_price <= levels[cursor - 1] < start_price:
                    cursor -= 1
                    traversals += 1

            if end_price >= upper or cursor == grid_count:
                profitable_steps = half_grid_count + max(0, traversals - half_grid_count)
                profit = arbitrage_profit(profitable_steps)
                usdt_wallet += principal_quote + profit
                total_fee_quote += profitable_steps * (principal_quote / grid_count) * fee_rate * 2.0
                fund_next_grid()
                reset_grid(end_price)

            if end_price <= lower or cursor == 0:
                profit = arbitrage_profit(max(0, traversals - half_grid_count))
                usdt_wallet += profit
                total_fee_quote += max(0, traversals - half_grid_count) * (principal_quote / grid_count) * fee_rate * 2.0
                inventory_qty += (principal_quote / max(center_price, 1e-12)) * (1.0 - fee_rate)
                fund_next_grid()
                reset_grid(end_price)

        last_close = float(bar["close"])
        points.append({"timestamp_ms": int(bar["timestamp_ms"]), "equity_quote": equity(last_close)})

    return {
        "name": f"dgt:{symbol}:gs{grid_spacing}:h{half_grid_count}:p{principal_quote}",
        "kind": "dgt",
        "symbols": [symbol],
        "points": points,
        "max_input_quote": max_input_quote,
        "max_capital_used_quote": max_input_quote,
        "total_input_quote": total_input_quote,
        "reset_count": reset_count,
        "total_fee_quote": total_fee_quote,
        "principal_quote": principal_quote,
        "grid_spacing": grid_spacing,
        "half_grid_count": half_grid_count,
        "live_parity_status": LIVE_PARITY_STATUS,
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
python3 -m unittest tests/verification/test_dgt_dynamic_grid_probe.py
python3 -m py_compile scripts/dgt_dynamic_grid_probe.py
```

Expected: tests pass and py_compile exits 0.

- [ ] **Step 5: Commit**

```bash
git add scripts/dgt_dynamic_grid_probe.py tests/verification/test_dgt_dynamic_grid_probe.py
git commit -m "feat: 修复思路 增加 DGT 动态网格会计探针"
```

---

## Task 2: Metrics, Segments, and Gates

**Files:**
- Modify: `scripts/dgt_dynamic_grid_probe.py`
- Modify: `tests/verification/test_dgt_dynamic_grid_probe.py`

- [ ] **Step 1: Add failing tests for metrics and gates**

Append to `tests/verification/test_dgt_dynamic_grid_probe.py` inside `DgtDynamicGridProbeTest`:

```python
    def test_profile_gate_rejects_single_symbol_and_over_budget(self):
        metrics = {
            "annualized_return_pct": 120.0,
            "max_drawdown_pct": 20.0,
            "max_input_quote": 4000.0,
            "symbol_count": 1,
            "positive_segments": 5,
            "combined_2024_2026_return_pct": 10.0,
        }
        result = dgt.evaluate_profile_gate("aggressive", metrics, budget=5000.0)
        self.assertFalse(result["passes"])
        self.assertIn("single-symbol candidate is not allowed", result["violations"])

        metrics["symbol_count"] = 2
        metrics["max_input_quote"] = 5000.0
        result = dgt.evaluate_profile_gate("aggressive", metrics, budget=5000.0)
        self.assertFalse(result["passes"])
        self.assertIn("capital 5000.00 is not below budget 5000.00", result["violations"])

    def test_segment_metrics_use_required_periods(self):
        points = [
            {"timestamp_ms": 1672531200000, "equity_quote": 100.0},
            {"timestamp_ms": 1688169600000, "equity_quote": 110.0},
            {"timestamp_ms": 1704067200000, "equity_quote": 120.0},
            {"timestamp_ms": 1735689600000, "equity_quote": 130.0},
            {"timestamp_ms": 1767225600000, "equity_quote": 140.0},
            {"timestamp_ms": 1780271999999, "equity_quote": 150.0},
        ]
        segments = dgt.compute_segment_metrics(points)
        self.assertEqual(set(segments), {"h1_2023", "h2_2023", "2024", "2025", "2026_ytd"})
        self.assertGreaterEqual(dgt.positive_segment_count(segments), 4)
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
python3 -m unittest tests/verification/test_dgt_dynamic_grid_probe.py
```

Expected: FAIL with missing `evaluate_profile_gate` and `compute_segment_metrics`.

- [ ] **Step 3: Implement metrics and gate helpers**

Append to `scripts/dgt_dynamic_grid_probe.py`:

```python

def compute_metrics(points: list[dict]) -> dict:
    if len(points) < 2:
        return {"annualized_return_pct": 0.0, "total_return_pct": 0.0, "max_drawdown_pct": 0.0}
    start = float(points[0]["equity_quote"])
    end = float(points[-1]["equity_quote"])
    if start <= 0:
        return {"annualized_return_pct": -100.0, "total_return_pct": -100.0, "max_drawdown_pct": 100.0}
    peak = start
    max_dd = 0.0
    for point in points:
        equity = float(point["equity_quote"])
        peak = max(peak, equity)
        if peak > 0:
            max_dd = max(max_dd, (peak - equity) / peak * 100.0)
    years = (int(points[-1]["timestamp_ms"]) - int(points[0]["timestamp_ms"])) / (365.25 * 24 * 3600 * 1000)
    total_return = (end / start - 1.0) * 100.0
    annualized = ((end / start) ** (1.0 / years) - 1.0) * 100.0 if years > 0 and end > 0 else -100.0
    return {
        "annualized_return_pct": annualized,
        "total_return_pct": total_return,
        "max_drawdown_pct": max_dd,
    }


def points_in_range(points: list[dict], start_ms: int, end_ms: int) -> list[dict]:
    return [point for point in points if start_ms <= int(point["timestamp_ms"]) <= end_ms]


def compute_segment_metrics(points: list[dict]) -> dict:
    return {
        name: compute_metrics(points_in_range(points, start_ms, end_ms))
        for name, (start_ms, end_ms) in SEGMENTS.items()
        if name != "full"
    }


def positive_segment_count(segment_metrics: dict) -> int:
    return sum(1 for metrics in segment_metrics.values() if metrics["total_return_pct"] > 0)


def combined_2024_2026_return(segment_metrics: dict) -> float:
    total = 1.0
    for name in ["2024", "2025", "2026_ytd"]:
        total *= 1.0 + segment_metrics.get(name, {}).get("total_return_pct", 0.0) / 100.0
    return (total - 1.0) * 100.0


def evaluate_profile_gate(profile: str, metrics: dict, budget: float) -> dict:
    target = PROFILE_TARGETS[profile]
    violations = []
    if metrics["annualized_return_pct"] <= target["ann"]:
        violations.append(f"annualized {metrics['annualized_return_pct']} <= required {target['ann']}")
    if metrics["max_drawdown_pct"] > target["dd"]:
        violations.append(f"drawdown {metrics['max_drawdown_pct']} > allowed {target['dd']}")
    if metrics["max_input_quote"] >= budget:
        violations.append(f"capital {metrics['max_input_quote']:.2f} is not below budget {budget:.2f}")
    if metrics["symbol_count"] < 2:
        violations.append("single-symbol candidate is not allowed")
    if metrics["positive_segments"] < 4:
        violations.append(f"only {metrics['positive_segments']}/5 segments positive; need 4")
    if metrics["combined_2024_2026_return_pct"] <= 0:
        violations.append(f"2024-2026 combined return {metrics['combined_2024_2026_return_pct']:.2f}% <= 0")
    return {"passes": not violations, "violations": violations}
```

- [ ] **Step 4: Run tests**

Run:

```bash
python3 -m unittest tests/verification/test_dgt_dynamic_grid_probe.py
python3 -m py_compile scripts/dgt_dynamic_grid_probe.py
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add scripts/dgt_dynamic_grid_probe.py tests/verification/test_dgt_dynamic_grid_probe.py
git commit -m "feat: 修复思路 增加 DGT 指标与门槛评估"
```

---

## Task 3: Market Loading and Portfolio Combination

**Files:**
- Modify: `scripts/dgt_dynamic_grid_probe.py`
- Modify: `tests/verification/test_dgt_dynamic_grid_probe.py`

- [ ] **Step 1: Add failing tests for stream combination**

Append to `tests/verification/test_dgt_dynamic_grid_probe.py` inside `DgtDynamicGridProbeTest`:

```python
    def test_combine_streams_sums_equity_and_capital(self):
        a = {
            "name": "dgt:A",
            "symbols": ["A"],
            "points": [
                {"timestamp_ms": 1, "equity_quote": 100.0},
                {"timestamp_ms": 2, "equity_quote": 110.0},
            ],
            "max_input_quote": 100.0,
            "total_fee_quote": 1.0,
        }
        b = {
            "name": "dgt:B",
            "symbols": ["B"],
            "points": [
                {"timestamp_ms": 1, "equity_quote": 200.0},
                {"timestamp_ms": 2, "equity_quote": 190.0},
            ],
            "max_input_quote": 200.0,
            "total_fee_quote": 2.0,
        }
        combined = dgt.combine_streams([a, b])
        self.assertEqual(combined["symbols"], ["A", "B"])
        self.assertEqual(combined["max_input_quote"], 300.0)
        self.assertEqual(combined["total_fee_quote"], 3.0)
        self.assertEqual(combined["points"][-1]["equity_quote"], 300.0)
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
python3 -m unittest tests/verification/test_dgt_dynamic_grid_probe.py
```

Expected: FAIL with missing `combine_streams`.

- [ ] **Step 3: Implement market loading and stream combination**

Append to `scripts/dgt_dynamic_grid_probe.py`:

```python

def load_1m_bars(market_db: str | Path, symbol: str, market_type: str, start_ms: int, end_ms: int) -> list[dict]:
    con = sqlite3.connect(str(market_db))
    try:
        rows = con.execute(
            """
            SELECT open_time, open, high, low, close
            FROM klines
            WHERE symbol = ? AND market_type = ? AND timeframe = '1m'
              AND open_time >= ? AND open_time <= ?
            ORDER BY open_time
            """,
            (symbol, market_type, start_ms, end_ms),
        ).fetchall()
    finally:
        con.close()
    return [
        {"timestamp_ms": int(ts), "open": float(open_), "high": float(high), "low": float(low), "close": float(close)}
        for ts, open_, high, low, close in rows
    ]


def combine_streams(streams: list[dict]) -> dict:
    if not streams:
        raise ValueError("at least one stream is required")
    timestamps = sorted(set.intersection(*(set(point["timestamp_ms"] for point in stream["points"]) for stream in streams)))
    if not timestamps:
        raise ValueError("streams have no overlapping timestamps")
    by_stream = {
        stream["name"]: {point["timestamp_ms"]: point["equity_quote"] for point in stream["points"]}
        for stream in streams
    }
    points = [
        {"timestamp_ms": timestamp, "equity_quote": sum(values[timestamp] for values in by_stream.values())}
        for timestamp in timestamps
    ]
    symbols = []
    for stream in streams:
        for symbol in stream.get("symbols", []):
            if symbol not in symbols:
                symbols.append(symbol)
    return {
        "name": "portfolio:" + ",".join(stream["name"] for stream in streams),
        "kind": "portfolio",
        "symbols": symbols,
        "points": points,
        "max_input_quote": sum(float(stream.get("max_input_quote", 0.0)) for stream in streams),
        "max_capital_used_quote": sum(float(stream.get("max_capital_used_quote", stream.get("max_input_quote", 0.0))) for stream in streams),
        "total_fee_quote": sum(float(stream.get("total_fee_quote", 0.0)) for stream in streams),
        "live_parity_status": LIVE_PARITY_STATUS,
    }
```

- [ ] **Step 4: Run tests**

Run:

```bash
python3 -m unittest tests/verification/test_dgt_dynamic_grid_probe.py
python3 -m py_compile scripts/dgt_dynamic_grid_probe.py
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add scripts/dgt_dynamic_grid_probe.py tests/verification/test_dgt_dynamic_grid_probe.py
git commit -m "feat: 修复思路 增加 DGT 组合流评估"
```

---

## Task 4: Candidate Reports and CLI Search

**Files:**
- Modify: `scripts/dgt_dynamic_grid_probe.py`
- Modify: `tests/verification/test_dgt_dynamic_grid_probe.py`

- [ ] **Step 1: Add failing report test**

Append to `tests/verification/test_dgt_dynamic_grid_probe.py` inside `DgtDynamicGridProbeTest`:

```python
    def test_build_candidate_report_stays_research_only(self):
        combined = {
            "symbols": ["BTCUSDT", "ETHUSDT"],
            "points": [
                {"timestamp_ms": 1672531200000, "equity_quote": 100.0},
                {"timestamp_ms": 1688169600000, "equity_quote": 120.0},
                {"timestamp_ms": 1704067200000, "equity_quote": 140.0},
                {"timestamp_ms": 1735689600000, "equity_quote": 170.0},
                {"timestamp_ms": 1767225600000, "equity_quote": 190.0},
                {"timestamp_ms": 1780271999999, "equity_quote": 230.0},
            ],
            "max_input_quote": 300.0,
            "total_fee_quote": 3.0,
            "live_parity_status": "research_only",
        }
        report = dgt.build_candidate_report("aggressive", combined, budget=5000.0, meta={"tag": "x"})
        self.assertEqual(report["live_parity_status"], "research_only")
        self.assertEqual(report["meta"]["tag"], "x")
        self.assertIn("passes_offline", report)
        self.assertIn("segment_metrics", report)
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
python3 -m unittest tests/verification/test_dgt_dynamic_grid_probe.py
```

Expected: FAIL with missing `build_candidate_report`.

- [ ] **Step 3: Implement candidate reporting and CLI**

Append to `scripts/dgt_dynamic_grid_probe.py`:

```python

def build_candidate_report(profile: str, combined: dict, budget: float, meta: dict | None = None) -> dict:
    full_metrics = compute_metrics(combined["points"])
    segment_metrics = compute_segment_metrics(combined["points"])
    gate_metrics = {
        **full_metrics,
        "max_input_quote": float(combined["max_input_quote"]),
        "symbol_count": len(combined["symbols"]),
        "positive_segments": positive_segment_count(segment_metrics),
        "combined_2024_2026_return_pct": combined_2024_2026_return(segment_metrics),
    }
    gate = evaluate_profile_gate(profile, gate_metrics, budget)
    return {
        "profile": profile,
        "live_parity_status": LIVE_PARITY_STATUS,
        "passes_offline": gate["passes"],
        "gate": gate,
        "full_metrics": full_metrics,
        "segment_metrics": segment_metrics,
        "max_input_quote": combined["max_input_quote"],
        "total_fee_quote": combined.get("total_fee_quote", 0.0),
        "symbols": combined["symbols"],
        "meta": meta or {},
    }


def parse_csv(value: str) -> list[str]:
    return [item.strip() for item in value.split(",") if item.strip()]


def parse_floats(value: str) -> list[float]:
    return [float(item) for item in parse_csv(value)]


def parse_ints(value: str) -> list[int]:
    return [int(item) for item in parse_csv(value)]


def choose_groups(symbols: list[str], size: int, limit: int) -> list[tuple[str, ...]]:
    return list(itertools.islice(itertools.combinations(symbols, size), limit))


def run_search(args: argparse.Namespace) -> dict:
    symbols = parse_csv(args.symbols)
    spacings = parse_floats(args.grid_spacings)
    half_counts = parse_ints(args.half_grid_counts)
    principals = parse_floats(args.principals)
    groups = choose_groups(symbols, args.group_size, args.group_limit)
    rows = []
    stream_cache = {}
    for group in groups:
        for spacing in spacings:
            for half_count in half_counts:
                for principal in principals:
                    streams = []
                    invalid = None
                    for symbol in group:
                        key = (symbol, spacing, half_count, principal)
                        if key not in stream_cache:
                            bars = load_1m_bars(args.market_data, symbol, args.market_type, SEGMENTS["full"][0], SEGMENTS["full"][1])
                            if len(bars) < args.min_bars:
                                invalid = f"insufficient bars for {symbol}: {len(bars)}"
                                break
                            stream_cache[key] = simulate_dgt_symbol(symbol, bars, principal, spacing, half_count, args.fee_bps)
                        streams.append(stream_cache[key])
                    if invalid:
                        continue
                    combined = combine_streams(streams)
                    for profile in parse_csv(args.profiles):
                        report = build_candidate_report(profile, combined, args.budget, {
                            "grid_spacing": spacing,
                            "half_grid_count": half_count,
                            "principal_quote": principal,
                            "group": ",".join(group),
                        })
                        rows.append(report)
                        if len(rows) >= args.limit:
                            return summarize_results(rows)
    return summarize_results(rows)


def summarize_results(rows: list[dict]) -> dict:
    summary = {}
    for profile in PROFILE_TARGETS:
        subset = [row for row in rows if row["profile"] == profile]
        passes = [row for row in subset if row["passes_offline"]]
        near = sorted(subset, key=lambda row: row["full_metrics"]["annualized_return_pct"], reverse=True)[:5]
        summary[profile] = {"rows": len(subset), "passes": len(passes), "top": near}
    return {"live_parity_status": LIVE_PARITY_STATUS, "rows": rows, "summary": summary}


def write_outputs(result: dict, out_json: str, out_md: str) -> None:
    Path(out_json).write_text(json.dumps(result, indent=2, sort_keys=True))
    lines = ["# DGT Dynamic Grid Probe", "", f"- live_parity_status: {LIVE_PARITY_STATUS}", f"- rows: {len(result['rows'])}", ""]
    for profile, summary in result["summary"].items():
        lines.append(f"## {profile}")
        lines.append(f"- rows: {summary['rows']}")
        lines.append(f"- passes: {summary['passes']}")
        for index, row in enumerate(summary["top"], start=1):
            full = row["full_metrics"]
            lines.append(
                f"- top {index}: ann={full['annualized_return_pct']:.2f}% dd={full['max_drawdown_pct']:.2f}% "
                f"max_input={row['max_input_quote']:.2f} symbols={','.join(row['symbols'])} "
                f"passes={row['passes_offline']} violations={row['gate']['violations']}"
            )
        lines.append("")
    lines.append("This is research_only evidence and is not live-ready.")
    Path(out_md).write_text("\\n".join(lines))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--market-data", default="data/market_data_full.db")
    parser.add_argument("--market-type", default="spot")
    parser.add_argument("--symbols", default="BTCUSDT,ETHUSDT,BNBUSDT,SOLUSDT,XRPUSDT,DOGEUSDT,ADAUSDT,LINKUSDT,AAVEUSDT,INJUSDT")
    parser.add_argument("--profiles", default="conservative,balanced,aggressive")
    parser.add_argument("--grid-spacings", default="0.02,0.03,0.05,0.07,0.10")
    parser.add_argument("--half-grid-counts", default="2,3,5,7")
    parser.add_argument("--principals", default="50,100,150")
    parser.add_argument("--group-size", type=int, default=2)
    parser.add_argument("--group-limit", type=int, default=20)
    parser.add_argument("--budget", type=float, default=5000.0)
    parser.add_argument("--fee-bps", type=float, default=8.0)
    parser.add_argument("--min-bars", type=int, default=1000)
    parser.add_argument("--limit", type=int, default=2000)
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

- [ ] **Step 4: Run tests**

Run:

```bash
python3 -m unittest tests/verification/test_dgt_dynamic_grid_probe.py
python3 -m py_compile scripts/dgt_dynamic_grid_probe.py
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add scripts/dgt_dynamic_grid_probe.py tests/verification/test_dgt_dynamic_grid_probe.py
git commit -m "feat: 修复思路 增加 DGT 搜索报告 CLI"
```

---

## Task 5: Bounded Search and Report

**Files:**
- Create: `docs/superpowers/reports/2026-07-01-dgt-dynamic-grid-probe.md`

- [ ] **Step 1: Run smoke search**

Run:

```bash
python3 scripts/dgt_dynamic_grid_probe.py \
  --market-data data/market_data_full.db \
  --market-type spot \
  --symbols BTCUSDT,ETHUSDT,BNBUSDT,SOLUSDT,XRPUSDT,DOGEUSDT \
  --grid-spacings 0.02,0.05,0.07,0.10 \
  --half-grid-counts 2,3,7 \
  --principals 50,100 \
  --group-size 2 \
  --group-limit 15 \
  --limit 1500 \
  --out-json /tmp/dgt_dynamic_grid_probe_smoke.json \
  --out-md /tmp/dgt_dynamic_grid_probe_smoke.md
```

Expected: command exits 0 and prints JSON with row count.

- [ ] **Step 2: Run focused BTC-seed multi-symbol search**

Run:

```bash
python3 scripts/dgt_dynamic_grid_probe.py \
  --market-data data/market_data_full.db \
  --market-type spot \
  --symbols BTCUSDT,ETHUSDT,BNBUSDT,SOLUSDT,XRPUSDT,DOGEUSDT,ADAUSDT,LINKUSDT,AAVEUSDT,INJUSDT \
  --grid-spacings 0.02,0.03,0.05,0.07,0.10 \
  --half-grid-counts 2,3,5,7 \
  --principals 50,100,150 \
  --group-size 2 \
  --group-limit 45 \
  --limit 5000 \
  --out-json /tmp/dgt_dynamic_grid_probe_g2.json \
  --out-md /tmp/dgt_dynamic_grid_probe_g2.md
```

Expected: command exits 0 and writes `/tmp/dgt_dynamic_grid_probe_g2.json`.

- [ ] **Step 3: Run group-size 3 search**

Run:

```bash
python3 scripts/dgt_dynamic_grid_probe.py \
  --market-data data/market_data_full.db \
  --market-type spot \
  --symbols BTCUSDT,ETHUSDT,BNBUSDT,SOLUSDT,XRPUSDT,DOGEUSDT,ADAUSDT,LINKUSDT,AAVEUSDT,INJUSDT \
  --grid-spacings 0.02,0.03,0.05,0.07,0.10 \
  --half-grid-counts 2,3,5,7 \
  --principals 50,100 \
  --group-size 3 \
  --group-limit 60 \
  --limit 5000 \
  --out-json /tmp/dgt_dynamic_grid_probe_g3.json \
  --out-md /tmp/dgt_dynamic_grid_probe_g3.md
```

Expected: command exits 0 and writes `/tmp/dgt_dynamic_grid_probe_g3.json`.

- [ ] **Step 4: Generate final report from JSON outputs**

Run this command to generate `docs/superpowers/reports/2026-07-01-dgt-dynamic-grid-probe.md` from the two search JSON files:

```bash
python3 - <<'PY'
import json
from pathlib import Path

inputs = [
    ("group_size_2", "/tmp/dgt_dynamic_grid_probe_g2.json"),
    ("group_size_3", "/tmp/dgt_dynamic_grid_probe_g3.json"),
]

def fmt(row):
    full = row["full_metrics"]
    return (
        f"`{','.join(row['symbols'])}` "
        f"ann `{full['annualized_return_pct']:.2f}%`, "
        f"DD `{full['max_drawdown_pct']:.2f}%`, "
        f"max_input `{row['max_input_quote']:.2f}`, "
        f"params `{row['meta']}`, "
        f"pass `{row['passes_offline']}`, "
        f"violations `{'; '.join(row['gate']['violations'])}`"
    )

lines = [
    "# 2026-07-01 DGT Dynamic Grid Probe",
    "",
    "This report summarizes the research-only DGT search. It is not live-ready and does not touch Binance, flyingkid, live mode, or real funds.",
    "",
    "## Scope",
    "",
    "- Data: `data/market_data_full.db`",
    "- Market type: `spot`",
    "- Profiles: conservative, balanced, aggressive",
    "- Gate targets: C >50% / DD<=10%, B >90% / DD<=20%, A >110% / DD<=30%, budget <5000U",
    "",
    "## Results",
    "",
]

total_passes = 0
for label, path in inputs:
    data = json.loads(Path(path).read_text())
    lines.append(f"### {label}")
    lines.append("")
    lines.append(f"- rows: `{len(data['rows'])}`")
    for profile in ["conservative", "balanced", "aggressive"]:
        summary = data["summary"][profile]
        total_passes += summary["passes"]
        subset = [row for row in data["rows"] if row["profile"] == profile]
        best_ann = max(subset, key=lambda row: row["full_metrics"]["annualized_return_pct"], default=None)
        budget_ok = [row for row in subset if row["max_input_quote"] < 5000]
        best_dd = min(budget_ok, key=lambda row: row["full_metrics"]["max_drawdown_pct"], default=None)
        lines.append(f"- {profile} passes: `{summary['passes']}`")
        if best_ann:
            lines.append(f"  - best annualized: {fmt(best_ann)}")
        if best_dd:
            lines.append(f"  - best DD under budget: {fmt(best_dd)}")
    lines.append("")

lines.append("## Conclusion")
lines.append("")
if total_passes:
    lines.append(f"- Offline passes found: `{total_passes}`. These are still `research_only` and require a separate live-parity promotion design before any live use.")
else:
    lines.append("- Offline passes found: `0`. The DGT search did not satisfy all original C/B/A gates in this scope.")
lines.append("- This is research_only evidence and is not live-ready.")

Path("docs/superpowers/reports/2026-07-01-dgt-dynamic-grid-probe.md").write_text("\n".join(lines))
PY
```

Expected: report file exists and contains concrete row/pass counts.

- [ ] **Step 5: Verify report has no placeholder**

Run:

```bash
rg -n "Fill this section|placeholder|TODO|TBD" docs/superpowers/reports/2026-07-01-dgt-dynamic-grid-probe.md
```

Expected: no matches.

- [ ] **Step 6: Commit**

```bash
git add docs/superpowers/reports/2026-07-01-dgt-dynamic-grid-probe.md
git commit -m "docs: 修复思路 记录 DGT 动态网格探针结果"
```

---

## Task 6: Final Verification and Merge

**Files:**
- No new file changes unless verification uncovers an issue.

- [ ] **Step 1: Run full verification in feature worktree**

Run:

```bash
python3 -m unittest tests/verification/test_dgt_dynamic_grid_probe.py
python3 -m py_compile scripts/dgt_dynamic_grid_probe.py
rg -n "live_parity_passed" scripts/dgt_dynamic_grid_probe.py docs/superpowers/reports/2026-07-01-dgt-dynamic-grid-probe.md
```

Expected:

- unittest passes.
- py_compile exits 0.
- `rg live_parity_passed` returns no matches.

- [ ] **Step 2: Merge to main**

From `/home/bumblebee/Project/grid_binance`:

```bash
git merge --ff-only dgt-dynamic-grid-probe
```

Expected: fast-forward merge.

- [ ] **Step 3: Run verification on main**

Run:

```bash
python3 -m unittest tests/verification/test_dgt_dynamic_grid_probe.py
python3 -m py_compile scripts/dgt_dynamic_grid_probe.py
```

Expected: all pass.

- [ ] **Step 4: Clean up worktree and branch**

Run:

```bash
git worktree remove /home/bumblebee/Project/grid_binance/.worktrees/dgt-dynamic-grid-probe
git branch -d dgt-dynamic-grid-probe
```

Expected: worktree removed and branch deleted.
