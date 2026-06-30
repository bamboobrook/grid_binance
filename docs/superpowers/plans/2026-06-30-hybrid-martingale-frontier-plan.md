# Hybrid Martingale Frontier Probe Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a research-only offline probe that combines martingale replay streams, trend/breakout streams, and funding/carry streams, then reports whether any hybrid portfolio reaches the original C/B/A gates.

**Architecture:** Keep Phase 1 isolated in Python under `scripts/` and `tests/verification/`; do not modify Rust backtest or trading-engine code. The probe normalizes each sleeve into timestamped equity/capital streams, combines them with explicit weights under a budget cap, and emits JSON/markdown robustness reports with `research_only` live-parity status.

**Tech Stack:** Python 3 standard library (`argparse`, `sqlite3`, `json`, `math`, `statistics`, `dataclasses`, `pathlib`), Node `node:test` contract tests for source-level guardrails, local SQLite market/funding databases, existing martingale replay JSON.

**Spec:** `docs/superpowers/specs/2026-06-30-hybrid-martingale-frontier-design.md`

---

## File Structure

- Create `scripts/hybrid_martingale_frontier_probe.py`
  - Owns all Phase 1 research-only logic.
  - Exposes pure helper functions for tests: `compute_metrics`, `evaluate_profile_gate`, `evaluate_segment_gate`, `resample_equity_curve`, `load_martingale_stream`, `build_trend_stream`, `build_funding_stream`, `combine_streams`, `build_candidate_report`, and `write_reports`.
  - CLI reads local data, builds a fixed research portfolio, and writes `--out-json` plus optional `--out-md`.
- Create `tests/verification/hybrid_frontier_probe_contract.test.mjs`
  - Node source contract tests that make sure the probe preserves the original gates, segment names, no-live wording, and no-lookahead guardrails.
- Create `tests/verification/hybrid_frontier_probe_sample.test.py`
  - Python unit tests using temporary SQLite databases and small JSON fixtures.
  - Tests deterministic math without requiring the large market database.
- Create `docs/superpowers/reports/2026-06-30-hybrid-frontier-smoke.md`
  - Written by the probe after the first smoke run.
- No production Rust files are modified in this plan.

## Shared Constants Required In The Probe

Use these exact constants in `scripts/hybrid_martingale_frontier_probe.py`:

```python
PROFILE_TARGETS = {
    "conservative": {"ann_min": 50.0, "dd_max": 10.0},
    "balanced": {"ann_min": 90.0, "dd_max": 20.0},
    "aggressive": {"ann_min": 110.0, "dd_max": 30.0},
}

SEGMENTS = {
    "h1_2023": (1672531200000, 1688169599999),
    "h2_2023": (1688169600000, 1704067199999),
    "2024": (1704067200000, 1735689599999),
    "2025": (1735689600000, 1767225599999),
    "2026_ytd": (1767225600000, 1780271999999),
    "full": (1672531200000, 1780271999999),
}

SEGMENT_CONSTRAINTS = {
    "conservative": {"min_positive_segments": 4, "max_segment_dd": 12.0, "no_2024_2026_total_loss": False},
    "balanced": {"min_positive_segments": 3, "max_segment_dd": 24.0, "no_2024_2026_total_loss": True},
    "aggressive": {"min_positive_segments": 3, "max_segment_dd": 36.0, "no_2024_2026_total_loss": True},
}

LIVE_PARITY_STATUS = "research_only"
```

---

### Task 1: Contract Test For Original Gates And Research-Only Boundary

**Files:**
- Create: `tests/verification/hybrid_frontier_probe_contract.test.mjs`
- Create: `scripts/hybrid_martingale_frontier_probe.py`

- [ ] **Step 1: Write the failing Node contract test**

Create `tests/verification/hybrid_frontier_probe_contract.test.mjs` with:

```javascript
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const sourcePath = "scripts/hybrid_martingale_frontier_probe.py";

test("hybrid frontier probe preserves original C/B/A gates", () => {
  const source = readFileSync(sourcePath, "utf8");
  assert.match(source, /"conservative":\s*\{"ann_min":\s*50\.0,\s*"dd_max":\s*10\.0\}/);
  assert.match(source, /"balanced":\s*\{"ann_min":\s*90\.0,\s*"dd_max":\s*20\.0\}/);
  assert.match(source, /"aggressive":\s*\{"ann_min":\s*110\.0,\s*"dd_max":\s*30\.0\}/);
  assert.match(source, /"h1_2023":\s*\(1672531200000,\s*1688169599999\)/);
  assert.match(source, /"h2_2023":\s*\(1688169600000,\s*1704067199999\)/);
  assert.match(source, /"2024":\s*\(1704067200000,\s*1735689599999\)/);
  assert.match(source, /"2025":\s*\(1735689600000,\s*1767225599999\)/);
  assert.match(source, /"2026_ytd":\s*\(1767225600000,\s*1780271999999\)/);
});

test("hybrid frontier probe cannot claim live parity in Phase 1", () => {
  const source = readFileSync(sourcePath, "utf8");
  assert.match(source, /LIVE_PARITY_STATUS\s*=\s*"research_only"/);
  assert.doesNotMatch(source, /live_parity_passed/);
  assert.match(source, /Phase 1 research-only/);
});

test("hybrid frontier probe documents no-lookahead stream construction", () => {
  const source = readFileSync(sourcePath, "utf8");
  assert.match(source, /no-lookahead/);
  assert.match(source, /warmup/);
  assert.match(source, /decision timestamp uses data at or before t/);
});
```

- [ ] **Step 2: Run the contract test to verify it fails**

Run:

```bash
node --test tests/verification/hybrid_frontier_probe_contract.test.mjs
```

Expected: FAIL with `ENOENT: no such file or directory, open 'scripts/hybrid_martingale_frontier_probe.py'`.

- [ ] **Step 3: Add a minimal probe skeleton**

Create `scripts/hybrid_martingale_frontier_probe.py` with:

```python
#!/usr/bin/env python3
"""Phase 1 research-only hybrid martingale frontier probe.

This script is intentionally offline-only. It combines martingale replay streams,
trend/breakout streams, and funding/carry streams to test whether a hybrid
portfolio frontier can reach the original C/B/A gates.

No live trading, Binance order placement, flyingkid publishing, or live-parity
claim is performed here. Stream construction is no-lookahead: each decision
timestamp uses data at or before t, and every indicator has an explicit warmup.
"""
from __future__ import annotations

PROFILE_TARGETS = {
    "conservative": {"ann_min": 50.0, "dd_max": 10.0},
    "balanced": {"ann_min": 90.0, "dd_max": 20.0},
    "aggressive": {"ann_min": 110.0, "dd_max": 30.0},
}

SEGMENTS = {
    "h1_2023": (1672531200000, 1688169599999),
    "h2_2023": (1688169600000, 1704067199999),
    "2024": (1704067200000, 1735689599999),
    "2025": (1735689600000, 1767225599999),
    "2026_ytd": (1767225600000, 1780271999999),
    "full": (1672531200000, 1780271999999),
}

SEGMENT_CONSTRAINTS = {
    "conservative": {"min_positive_segments": 4, "max_segment_dd": 12.0, "no_2024_2026_total_loss": False},
    "balanced": {"min_positive_segments": 3, "max_segment_dd": 24.0, "no_2024_2026_total_loss": True},
    "aggressive": {"min_positive_segments": 3, "max_segment_dd": 36.0, "no_2024_2026_total_loss": True},
}

LIVE_PARITY_STATUS = "research_only"


def main() -> int:
    print("hybrid frontier probe skeleton: Phase 1 research-only")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 4: Run the contract test to verify it passes**

Run:

```bash
node --test tests/verification/hybrid_frontier_probe_contract.test.mjs
```

Expected: PASS, `3 pass`.

- [ ] **Step 5: Commit**

```bash
git add scripts/hybrid_martingale_frontier_probe.py tests/verification/hybrid_frontier_probe_contract.test.mjs
git commit -m "test: 修复思路 固化混合马丁探针边界"
```

---

### Task 2: Metrics, Segment, And Gate Helpers

**Files:**
- Modify: `scripts/hybrid_martingale_frontier_probe.py`
- Create: `tests/verification/hybrid_frontier_probe_sample.test.py`

- [ ] **Step 1: Write failing Python tests for metrics and gates**

Create `tests/verification/hybrid_frontier_probe_sample.test.py` with:

```python
import importlib.util
from pathlib import Path


SCRIPT = Path("scripts/hybrid_martingale_frontier_probe.py")
SPEC = importlib.util.spec_from_file_location("hybrid_probe", SCRIPT)
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)


def test_compute_metrics_positive_curve():
    points = [
        {"timestamp_ms": 1672531200000, "equity_quote": 1000.0},
        {"timestamp_ms": 1672617600000, "equity_quote": 1100.0},
        {"timestamp_ms": 1672704000000, "equity_quote": 1050.0},
        {"timestamp_ms": 1672790400000, "equity_quote": 1200.0},
    ]
    metrics = probe.compute_metrics(points)
    assert round(metrics["total_return_pct"], 4) == 20.0
    assert metrics["annualized_return_pct"] > 1000.0
    assert round(metrics["max_drawdown_pct"], 4) == round((1100.0 - 1050.0) / 1100.0 * 100.0, 4)


def test_evaluate_profile_gate_enforces_budget_and_original_thresholds():
    passing = {
        "annualized_return_pct": 55.0,
        "max_drawdown_pct": 9.0,
        "max_capital_used_quote": 4999.0,
        "budget_blocked_events": 0,
        "symbol_count": 3,
    }
    assert probe.evaluate_profile_gate("conservative", passing, 5000.0)["passes"] is True

    over_budget = dict(passing, max_capital_used_quote=5000.0)
    result = probe.evaluate_profile_gate("conservative", over_budget, 5000.0)
    assert result["passes"] is False
    assert "capital 5000.00 is not below budget 5000.00" in result["violations"]

    single_symbol = dict(passing, symbol_count=1)
    result = probe.evaluate_profile_gate("conservative", single_symbol, 5000.0)
    assert result["passes"] is False
    assert "single-symbol portfolio is not allowed" in result["violations"]


def test_segment_gate_rejects_h1_only_overfit():
    segment_metrics = {
        "h1_2023": {"total_return_pct": 200.0, "max_drawdown_pct": 8.0},
        "h2_2023": {"total_return_pct": -10.0, "max_drawdown_pct": 8.0},
        "2024": {"total_return_pct": -20.0, "max_drawdown_pct": 8.0},
        "2025": {"total_return_pct": -30.0, "max_drawdown_pct": 8.0},
        "2026_ytd": {"total_return_pct": -5.0, "max_drawdown_pct": 8.0},
    }
    result = probe.evaluate_segment_gate("balanced", segment_metrics)
    assert result["passes"] is False
    assert result["positive_segments"] == 1
    assert any("segments positive" in item for item in result["violations"])
    assert any("2024-2026 combined return" in item for item in result["violations"])
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
python3 -m unittest tests/verification/hybrid_frontier_probe_sample.test.py
```

Expected: FAIL with `AttributeError` for `compute_metrics`.

- [ ] **Step 3: Implement metrics and gate helpers**

Append these imports and functions to `scripts/hybrid_martingale_frontier_probe.py` after the constants:

```python
import math
from typing import Iterable


MS_PER_DAY = 86_400_000


def compute_metrics(points: list[dict]) -> dict:
    """Compute return and drawdown metrics from timestamped equity points."""
    clean = [
        {"timestamp_ms": int(p["timestamp_ms"]), "equity_quote": float(p["equity_quote"])}
        for p in points
        if p.get("equity_quote") is not None
    ]
    clean.sort(key=lambda p: p["timestamp_ms"])
    if len(clean) < 2:
        return {
            "total_return_pct": 0.0,
            "annualized_return_pct": 0.0,
            "max_drawdown_pct": 0.0,
            "start_equity_quote": clean[0]["equity_quote"] if clean else 0.0,
            "end_equity_quote": clean[-1]["equity_quote"] if clean else 0.0,
            "days": 0.0,
        }

    start = clean[0]["equity_quote"]
    end = clean[-1]["equity_quote"]
    if start <= 0:
        raise ValueError("start equity must be positive")
    total_return_pct = (end / start - 1.0) * 100.0
    days = max((clean[-1]["timestamp_ms"] - clean[0]["timestamp_ms"]) / MS_PER_DAY, 1.0 / 24.0)
    annualized_return_pct = ((end / start) ** (365.0 / days) - 1.0) * 100.0

    peak = clean[0]["equity_quote"]
    max_dd = 0.0
    for point in clean:
        peak = max(peak, point["equity_quote"])
        if peak > 0:
            max_dd = max(max_dd, (peak - point["equity_quote"]) / peak * 100.0)

    return {
        "total_return_pct": total_return_pct,
        "annualized_return_pct": annualized_return_pct,
        "max_drawdown_pct": max_dd,
        "start_equity_quote": start,
        "end_equity_quote": end,
        "days": days,
    }


def compound_returns(returns_pct: Iterable[float]) -> float:
    growth = 1.0
    for value in returns_pct:
        growth *= 1.0 + float(value) / 100.0
    return (growth - 1.0) * 100.0


def evaluate_profile_gate(profile: str, metrics: dict, budget: float) -> dict:
    target = PROFILE_TARGETS[profile]
    violations = []
    ann = metrics.get("annualized_return_pct")
    dd = metrics.get("max_drawdown_pct")
    capital = metrics.get("max_capital_used_quote", 0.0)
    blocked = metrics.get("budget_blocked_events", 0)
    symbol_count = metrics.get("symbol_count", 0)

    if ann is None or ann <= target["ann_min"]:
        violations.append(f"annualized {ann} <= required {target['ann_min']}")
    if dd is None or dd > target["dd_max"]:
        violations.append(f"drawdown {dd} > allowed {target['dd_max']}")
    if capital >= budget:
        violations.append(f"capital {capital:.2f} is not below budget {budget:.2f}")
    if blocked:
        violations.append(f"budget blocked events {blocked} > 0")
    if symbol_count < 2:
        violations.append("single-symbol portfolio is not allowed")

    return {"passes": not violations, "violations": violations}


def evaluate_segment_gate(profile: str, segment_metrics: dict) -> dict:
    constraints = SEGMENT_CONSTRAINTS[profile]
    violations = []
    required = [name for name in SEGMENTS if name != "full"]
    missing = [name for name in required if name not in segment_metrics]
    if missing:
        violations.append("missing segment metrics: " + ",".join(missing))

    positive = 0
    for name in required:
        metrics = segment_metrics.get(name, {})
        total = metrics.get("total_return_pct")
        dd = metrics.get("max_drawdown_pct")
        if total is not None and total >= 0:
            positive += 1
        if dd is not None and dd > constraints["max_segment_dd"]:
            violations.append(f"{name}: DD {dd:.2f}% > {constraints['max_segment_dd']:.2f}%")

    if positive < constraints["min_positive_segments"]:
        violations.append(f"only {positive}/5 segments positive; need {constraints['min_positive_segments']}")

    combined_2024_2026 = compound_returns(
        segment_metrics.get(name, {}).get("total_return_pct", 0.0)
        for name in ("2024", "2025", "2026_ytd")
    )
    if constraints["no_2024_2026_total_loss"] and combined_2024_2026 < 0:
        violations.append(f"2024-2026 combined return {combined_2024_2026:.2f}% < 0")

    return {
        "passes": not violations,
        "violations": violations,
        "positive_segments": positive,
        "combined_2024_2026_return_pct": combined_2024_2026,
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
python3 -m unittest tests/verification/hybrid_frontier_probe_sample.test.py
node --test tests/verification/hybrid_frontier_probe_contract.test.mjs
```

Expected: Python `OK`; Node `3 pass`.

- [ ] **Step 5: Commit**

```bash
git add scripts/hybrid_martingale_frontier_probe.py tests/verification/hybrid_frontier_probe_sample.test.py
git commit -m "feat: 修复思路 添加混合前沿 gate 评估"
```

---

### Task 3: Martingale Replay Sleeve Loader And Resampler

**Files:**
- Modify: `scripts/hybrid_martingale_frontier_probe.py`
- Modify: `tests/verification/hybrid_frontier_probe_sample.test.py`

- [ ] **Step 1: Add failing tests for martingale stream loading**

Append to `tests/verification/hybrid_frontier_probe_sample.test.py`:

```python
import json


def test_load_martingale_stream_from_replay_json(tmp_path):
    replay = {
        "portfolio_id": "demo_m",
        "symbols": ["BTCUSDT", "ETHUSDT"],
        "budget_quote": 4000.0,
        "max_capital_used_quote": 1234.0,
        "budget_blocked_legs": 0,
        "equity_curve": [
            {"timestamp_ms": 1000, "equity_quote": 1000.0},
            {"timestamp_ms": 2000, "equity_quote": 1010.0},
            {"timestamp_ms": 3000, "equity_quote": 990.0},
        ],
    }
    path = tmp_path / "replay.json"
    path.write_text(json.dumps(replay))
    stream = probe.load_martingale_stream(path, allocation_quote=2000.0)
    assert stream["name"] == "martingale:demo_m"
    assert stream["symbols"] == ["BTCUSDT", "ETHUSDT"]
    assert stream["max_capital_used_quote"] == 1234.0
    assert stream["budget_blocked_events"] == 0
    assert stream["points"][0]["equity_quote"] == 2000.0
    assert round(stream["points"][1]["equity_quote"], 4) == 2020.0
    assert round(stream["points"][2]["equity_quote"], 4) == 1980.0


def test_resample_equity_curve_forward_fills_without_lookahead():
    points = [
        {"timestamp_ms": 1000, "equity_quote": 100.0},
        {"timestamp_ms": 3000, "equity_quote": 120.0},
    ]
    sampled = probe.resample_equity_curve(points, [500, 1000, 2000, 3000, 4000])
    assert sampled == [
        {"timestamp_ms": 1000, "equity_quote": 100.0},
        {"timestamp_ms": 2000, "equity_quote": 100.0},
        {"timestamp_ms": 3000, "equity_quote": 120.0},
        {"timestamp_ms": 4000, "equity_quote": 120.0},
    ]
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
python3 -m unittest tests/verification/hybrid_frontier_probe_sample.test.py
```

Expected: FAIL with `AttributeError` for `load_martingale_stream`.

- [ ] **Step 3: Implement replay loader and no-lookahead resampler**

Add imports near the top:

```python
import json
from pathlib import Path
```

Append functions:

```python
def resample_equity_curve(points: list[dict], timestamps: list[int]) -> list[dict]:
    """Forward-fill equity using only points at or before each timestamp."""
    ordered = sorted(
        [{"timestamp_ms": int(p["timestamp_ms"]), "equity_quote": float(p["equity_quote"])} for p in points],
        key=lambda p: p["timestamp_ms"],
    )
    result = []
    index = 0
    last = None
    for ts in sorted(int(t) for t in timestamps):
        while index < len(ordered) and ordered[index]["timestamp_ms"] <= ts:
            last = ordered[index]
            index += 1
        if last is not None:
            result.append({"timestamp_ms": ts, "equity_quote": last["equity_quote"]})
    return result


def load_martingale_stream(path: str | Path, allocation_quote: float) -> dict:
    """Load an existing martingale replay JSON and scale its equity curve to an allocation."""
    data = json.loads(Path(path).read_text())
    curve = data.get("equity_curve") or []
    if len(curve) < 2:
        raise ValueError(f"martingale replay has no usable equity_curve: {path}")
    start_equity = float(curve[0]["equity_quote"])
    if start_equity <= 0:
        raise ValueError(f"martingale replay start equity must be positive: {path}")
    scaled = [
        {
            "timestamp_ms": int(point["timestamp_ms"]),
            "equity_quote": allocation_quote * float(point["equity_quote"]) / start_equity,
        }
        for point in curve
    ]
    return {
        "name": f"martingale:{data.get('portfolio_id', Path(path).stem)}",
        "kind": "martingale",
        "symbols": list(data.get("symbols", [])),
        "points": scaled,
        "max_capital_used_quote": float(data.get("max_capital_used_quote") or allocation_quote),
        "budget_blocked_events": int(data.get("budget_blocked_legs") or 0),
        "source": str(path),
        "live_parity_status": LIVE_PARITY_STATUS,
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
python3 -m unittest tests/verification/hybrid_frontier_probe_sample.test.py
node --test tests/verification/hybrid_frontier_probe_contract.test.mjs
```

Expected: Python `OK`; Node `3 pass`.

- [ ] **Step 5: Commit**

```bash
git add scripts/hybrid_martingale_frontier_probe.py tests/verification/hybrid_frontier_probe_sample.test.py
git commit -m "feat: 修复思路 接入马丁 replay sleeve"
```

---

### Task 4: Trend Stream Builder From Local Klines

**Files:**
- Modify: `scripts/hybrid_martingale_frontier_probe.py`
- Modify: `tests/verification/hybrid_frontier_probe_sample.test.py`

- [ ] **Step 1: Add failing tests for trend stream construction**

Append to `tests/verification/hybrid_frontier_probe_sample.test.py`:

```python
import sqlite3


def make_market_db(path, rows):
    con = sqlite3.connect(path)
    con.execute(
        "CREATE TABLE klines (symbol TEXT, market_type TEXT, timeframe TEXT, open_time INTEGER, open REAL, high REAL, low REAL, close REAL, volume REAL, close_time INTEGER)"
    )
    con.executemany("INSERT INTO klines VALUES (?,?,?,?,?,?,?,?,?,?)", rows)
    con.commit()
    con.close()


def test_build_trend_stream_uses_previous_close_for_signal(tmp_path):
    db = tmp_path / "market.db"
    day = 86_400_000
    rows = []
    closes = [100, 101, 102, 103, 104, 90, 89, 88, 110, 111, 112, 113]
    for i, close in enumerate(closes):
        rows.append(("BTCUSDT", "futures_usdt_perp", "1m", i * day, close, close, close, close, 1.0, i * day + 60_000 - 1))
    make_market_db(db, rows)
    stream = probe.build_trend_stream(
        market_db=db,
        symbol="BTCUSDT",
        allocation_quote=1000.0,
        fast=2,
        slow=4,
        fee_bps=0.0,
    )
    assert stream["name"] == "trend:BTCUSDT:ema2_4"
    assert stream["symbols"] == ["BTCUSDT"]
    assert len(stream["points"]) >= 8
    assert stream["points"][0]["timestamp_ms"] >= 4 * day
    assert stream["no_lookahead"] is True
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
python3 -m unittest tests/verification/hybrid_frontier_probe_sample.test.py
```

Expected: FAIL with `AttributeError` for `build_trend_stream`.

- [ ] **Step 3: Implement kline loading, EMA, and trend stream**

Add imports:

```python
import sqlite3
```

Append functions:

```python
def load_daily_closes(market_db: str | Path, symbol: str, market_type: str = "futures_usdt_perp") -> list[dict]:
    """Load one close per UTC day from local 1m klines."""
    con = sqlite3.connect(str(market_db))
    try:
        rows = con.execute(
            """
            SELECT open_time, close
            FROM klines
            WHERE symbol = ? AND market_type = ? AND timeframe = '1m'
            ORDER BY open_time
            """,
            (symbol, market_type),
        ).fetchall()
    finally:
        con.close()
    by_day = {}
    for ts, close in rows:
        day_key = int(ts) // MS_PER_DAY
        by_day[day_key] = {"timestamp_ms": int(ts), "close": float(close)}
    return [by_day[key] for key in sorted(by_day)]


def ema_values(values: list[float], period: int) -> list[float | None]:
    if period <= 0:
        raise ValueError("EMA period must be positive")
    alpha = 2.0 / (period + 1.0)
    out: list[float | None] = []
    ema = None
    for index, value in enumerate(values):
        if ema is None:
            ema = float(value)
        else:
            ema = alpha * float(value) + (1.0 - alpha) * ema
        out.append(ema if index + 1 >= period else None)
    return out


def build_trend_stream(
    market_db: str | Path,
    symbol: str,
    allocation_quote: float,
    fast: int = 20,
    slow: int = 50,
    fee_bps: float = 2.0,
) -> dict:
    """Build a daily long/flat EMA trend stream; signal uses previous day data."""
    daily = load_daily_closes(market_db, symbol)
    closes = [row["close"] for row in daily]
    fast_ema = ema_values(closes, fast)
    slow_ema = ema_values(closes, slow)
    equity = allocation_quote
    points = []
    position = 0
    max_period = max(fast, slow)
    for index in range(1, len(daily)):
        prev_fast = fast_ema[index - 1]
        prev_slow = slow_ema[index - 1]
        if prev_fast is None or prev_slow is None or index < max_period:
            continue
        desired = 1 if prev_fast > prev_slow else 0
        if desired != position:
            equity *= 1.0 - fee_bps / 10_000.0
            position = desired
        prev_close = daily[index - 1]["close"]
        close = daily[index]["close"]
        if position and prev_close > 0:
            equity *= close / prev_close
        points.append({"timestamp_ms": daily[index]["timestamp_ms"], "equity_quote": equity})
    if not points:
        points = [{"timestamp_ms": row["timestamp_ms"], "equity_quote": allocation_quote} for row in daily[max_period:]]
    return {
        "name": f"trend:{symbol}:ema{fast}_{slow}",
        "kind": "trend",
        "symbols": [symbol],
        "points": points,
        "max_capital_used_quote": allocation_quote,
        "budget_blocked_events": 0,
        "fee_bps": fee_bps,
        "no_lookahead": True,
        "live_parity_status": LIVE_PARITY_STATUS,
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
python3 -m unittest tests/verification/hybrid_frontier_probe_sample.test.py
node --test tests/verification/hybrid_frontier_probe_contract.test.mjs
```

Expected: Python `OK`; Node `3 pass`.

- [ ] **Step 5: Commit**

```bash
git add scripts/hybrid_martingale_frontier_probe.py tests/verification/hybrid_frontier_probe_sample.test.py
git commit -m "feat: 修复思路 增加离线趋势 sleeve"
```

---

### Task 5: Funding Carry Stream Builder

**Files:**
- Modify: `scripts/hybrid_martingale_frontier_probe.py`
- Modify: `tests/verification/hybrid_frontier_probe_sample.test.py`

- [ ] **Step 1: Add failing tests for funding stream construction**

Append to `tests/verification/hybrid_frontier_probe_sample.test.py`:

```python
def make_funding_db(path, rows):
    con = sqlite3.connect(path)
    con.execute("CREATE TABLE funding_rates (symbol TEXT, funding_time INTEGER, funding_rate REAL, mark_price REAL)")
    con.executemany("INSERT INTO funding_rates VALUES (?,?,?,?)", rows)
    con.commit()
    con.close()


def test_build_funding_stream_short_perp_receives_positive_funding(tmp_path):
    db = tmp_path / "funding.db"
    rows = [
        ("BTCUSDT", 1000, 0.001, 100.0),
        ("BTCUSDT", 2000, 0.001, 100.0),
        ("BTCUSDT", 3000, -0.0005, 100.0),
    ]
    make_funding_db(db, rows)
    stream = probe.build_funding_stream(db, "BTCUSDT", allocation_quote=1000.0, start_ms=0, end_ms=4000)
    assert stream["name"] == "funding:BTCUSDT:short_perp"
    assert stream["symbols"] == ["BTCUSDT"]
    assert round(stream["points"][-1]["equity_quote"], 4) == 1001.5
    assert stream["funding_events"] == 3
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
python3 -m unittest tests/verification/hybrid_frontier_probe_sample.test.py
```

Expected: FAIL with `AttributeError` for `build_funding_stream`.

- [ ] **Step 3: Implement funding stream**

Append:

```python
def build_funding_stream(
    funding_db: str | Path,
    symbol: str,
    allocation_quote: float,
    start_ms: int,
    end_ms: int,
) -> dict:
    """Build a short-perp funding stream. Positive funding_rate benefits shorts."""
    con = sqlite3.connect(str(funding_db))
    try:
        rows = con.execute(
            """
            SELECT funding_time, funding_rate
            FROM funding_rates
            WHERE symbol = ? AND funding_time >= ? AND funding_time <= ?
            ORDER BY funding_time
            """,
            (symbol, int(start_ms), int(end_ms)),
        ).fetchall()
    finally:
        con.close()
    equity = allocation_quote
    points = []
    for ts, rate in rows:
        equity += allocation_quote * float(rate)
        points.append({"timestamp_ms": int(ts), "equity_quote": equity})
    if not points:
        points = [{"timestamp_ms": int(start_ms), "equity_quote": allocation_quote}]
    return {
        "name": f"funding:{symbol}:short_perp",
        "kind": "funding",
        "symbols": [symbol],
        "points": points,
        "max_capital_used_quote": allocation_quote,
        "budget_blocked_events": 0,
        "funding_events": len(rows),
        "no_lookahead": True,
        "live_parity_status": LIVE_PARITY_STATUS,
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
python3 -m unittest tests/verification/hybrid_frontier_probe_sample.test.py
node --test tests/verification/hybrid_frontier_probe_contract.test.mjs
```

Expected: Python `OK`; Node `3 pass`.

- [ ] **Step 5: Commit**

```bash
git add scripts/hybrid_martingale_frontier_probe.py tests/verification/hybrid_frontier_probe_sample.test.py
git commit -m "feat: 修复思路 增加 funding carry sleeve"
```

---

### Task 6: Portfolio Combiner And Segment Reporter

**Files:**
- Modify: `scripts/hybrid_martingale_frontier_probe.py`
- Modify: `tests/verification/hybrid_frontier_probe_sample.test.py`

- [ ] **Step 1: Add failing tests for combining streams and reporting segments**

Append to `tests/verification/hybrid_frontier_probe_sample.test.py`:

```python
def test_combine_streams_aligns_points_and_sums_capital():
    streams = [
        {
            "name": "a",
            "symbols": ["BTCUSDT"],
            "points": [{"timestamp_ms": 1000, "equity_quote": 100.0}, {"timestamp_ms": 2000, "equity_quote": 110.0}],
            "max_capital_used_quote": 100.0,
            "budget_blocked_events": 0,
        },
        {
            "name": "b",
            "symbols": ["ETHUSDT"],
            "points": [{"timestamp_ms": 1000, "equity_quote": 200.0}, {"timestamp_ms": 2000, "equity_quote": 190.0}],
            "max_capital_used_quote": 200.0,
            "budget_blocked_events": 0,
        },
    ]
    combined = probe.combine_streams(streams, budget=500.0)
    assert combined["symbols"] == ["BTCUSDT", "ETHUSDT"]
    assert combined["points"][-1]["equity_quote"] == 300.0
    assert combined["metrics"]["max_capital_used_quote"] == 300.0
    assert combined["metrics"]["symbol_count"] == 2


def test_slice_points_for_segment_includes_boundary_points():
    points = [
        {"timestamp_ms": 1000, "equity_quote": 100.0},
        {"timestamp_ms": 2000, "equity_quote": 110.0},
        {"timestamp_ms": 3000, "equity_quote": 120.0},
    ]
    sliced = probe.slice_points(points, 1500, 2500)
    assert sliced[0]["timestamp_ms"] == 1500
    assert sliced[0]["equity_quote"] == 100.0
    assert sliced[-1]["timestamp_ms"] == 2500
    assert sliced[-1]["equity_quote"] == 110.0
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
python3 -m unittest tests/verification/hybrid_frontier_probe_sample.test.py
```

Expected: FAIL with `AttributeError` for `combine_streams`.

- [ ] **Step 3: Implement combiner and segment slicing**

Append:

```python
def slice_points(points: list[dict], start_ms: int, end_ms: int) -> list[dict]:
    timestamps = [int(start_ms)]
    timestamps.extend(int(p["timestamp_ms"]) for p in points if start_ms <= int(p["timestamp_ms"]) <= end_ms)
    timestamps.append(int(end_ms))
    return resample_equity_curve(points, sorted(set(timestamps)))


def combine_streams(streams: list[dict], budget: float) -> dict:
    """Combine sleeve equity streams by summing aligned equity values."""
    if not streams:
        raise ValueError("at least one stream is required")
    all_timestamps = sorted({
        int(point["timestamp_ms"])
        for stream in streams
        for point in stream["points"]
    })
    aligned = [resample_equity_curve(stream["points"], all_timestamps) for stream in streams]
    by_stream = []
    for stream, points in zip(streams, aligned):
        by_stream.append({point["timestamp_ms"]: point["equity_quote"] for point in points})

    combined_points = []
    for ts in all_timestamps:
        values = [series.get(ts) for series in by_stream]
        if all(value is not None for value in values):
            combined_points.append({"timestamp_ms": ts, "equity_quote": sum(values)})

    symbols = sorted({symbol for stream in streams for symbol in stream.get("symbols", [])})
    max_capital = sum(float(stream.get("max_capital_used_quote", 0.0)) for stream in streams)
    blocked = sum(int(stream.get("budget_blocked_events", 0)) for stream in streams)
    metrics = compute_metrics(combined_points)
    metrics.update({
        "max_capital_used_quote": max_capital,
        "budget_blocked_events": blocked + (1 if max_capital >= budget else 0),
        "symbol_count": len(symbols),
    })
    return {
        "streams": [stream["name"] for stream in streams],
        "symbols": symbols,
        "points": combined_points,
        "metrics": metrics,
        "live_parity_status": LIVE_PARITY_STATUS,
    }


def segment_report(points: list[dict]) -> dict:
    report = {}
    for name, (start_ms, end_ms) in SEGMENTS.items():
        if name == "full":
            report[name] = compute_metrics(slice_points(points, start_ms, end_ms))
        else:
            report[name] = compute_metrics(slice_points(points, start_ms, end_ms))
    return report
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
python3 -m unittest tests/verification/hybrid_frontier_probe_sample.test.py
node --test tests/verification/hybrid_frontier_probe_contract.test.mjs
```

Expected: Python `OK`; Node `3 pass`.

- [ ] **Step 5: Commit**

```bash
git add scripts/hybrid_martingale_frontier_probe.py tests/verification/hybrid_frontier_probe_sample.test.py
git commit -m "feat: 修复思路 组合混合 sleeve 曲线"
```

---

### Task 7: CLI Search And Report Writer

**Files:**
- Modify: `scripts/hybrid_martingale_frontier_probe.py`
- Modify: `tests/verification/hybrid_frontier_probe_sample.test.py`

- [ ] **Step 1: Add failing tests for report structure**

Append to `tests/verification/hybrid_frontier_probe_sample.test.py`:

```python
def test_build_candidate_report_marks_research_only():
    combined = {
        "streams": ["martingale:a", "trend:BTCUSDT:ema2_4"],
        "symbols": ["BTCUSDT", "ETHUSDT"],
        "points": [
            {"timestamp_ms": 1672531200000, "equity_quote": 1000.0},
            {"timestamp_ms": 1672617600000, "equity_quote": 1010.0},
        ],
        "metrics": {
            "annualized_return_pct": 60.0,
            "max_drawdown_pct": 5.0,
            "total_return_pct": 1.0,
            "max_capital_used_quote": 3000.0,
            "budget_blocked_events": 0,
            "symbol_count": 2,
        },
        "live_parity_status": "research_only",
    }
    report = probe.build_candidate_report("conservative", combined, budget=5000.0)
    assert report["profile"] == "conservative"
    assert report["live_parity_status"] == "research_only"
    assert report["full_gate"]["passes"] is True
    assert "segment_gate" in report
    assert "sleeve_attribution" in report
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
python3 -m unittest tests/verification/hybrid_frontier_probe_sample.test.py
```

Expected: FAIL with `AttributeError` for `build_candidate_report`.

- [ ] **Step 3: Implement candidate report, CLI, and markdown writer**

Replace the existing `main()` skeleton and append helpers:

```python
import argparse


def build_candidate_report(profile: str, combined: dict, budget: float) -> dict:
    segments = segment_report(combined["points"])
    segment_metrics = {name: value for name, value in segments.items() if name != "full"}
    full_metrics = dict(combined["metrics"])
    full_gate = evaluate_profile_gate(profile, full_metrics, budget)
    segment_gate = evaluate_segment_gate(profile, segment_metrics)
    return {
        "profile": profile,
        "budget": budget,
        "live_parity_status": LIVE_PARITY_STATUS,
        "streams": combined["streams"],
        "symbols": combined["symbols"],
        "full_metrics": full_metrics,
        "segments": segments,
        "full_gate": full_gate,
        "segment_gate": segment_gate,
        "passes_offline": full_gate["passes"] and segment_gate["passes"],
        "sleeve_attribution": [{"name": name} for name in combined["streams"]],
    }


def write_reports(report: dict, out_json: str | Path, out_md: str | Path | None = None) -> None:
    Path(out_json).write_text(json.dumps(report, indent=2, sort_keys=True))
    if out_md:
        lines = [
            "# Hybrid Frontier Probe Smoke Report",
            "",
            f"- profile: {report['profile']}",
            f"- budget: {report['budget']}",
            f"- live_parity_status: {report['live_parity_status']}",
            f"- passes_offline: {report['passes_offline']}",
            f"- streams: {', '.join(report['streams'])}",
            f"- symbols: {', '.join(report['symbols'])}",
            f"- full annualized: {report['full_metrics'].get('annualized_return_pct'):.4f}",
            f"- full max DD: {report['full_metrics'].get('max_drawdown_pct'):.4f}",
            "",
            "This is Phase 1 research-only evidence and is not live-ready.",
            "",
        ]
        Path(out_md).write_text("\n".join(lines))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", choices=sorted(PROFILE_TARGETS), required=True)
    parser.add_argument("--budget", type=float, default=5000.0)
    parser.add_argument("--martingale-replay", required=True)
    parser.add_argument("--martingale-allocation", type=float, default=1500.0)
    parser.add_argument("--market-data", default="data/market_data_full.db")
    parser.add_argument("--funding-data", default="data/funding_rates.db")
    parser.add_argument("--trend-symbols", default="BTCUSDT,ETHUSDT,BNBUSDT")
    parser.add_argument("--trend-allocation", type=float, default=750.0)
    parser.add_argument("--funding-symbols", default="BTCUSDT,ETHUSDT")
    parser.add_argument("--funding-allocation", type=float, default=250.0)
    parser.add_argument("--out-json", required=True)
    parser.add_argument("--out-md", default=None)
    return parser.parse_args()


def run_from_args(args: argparse.Namespace) -> dict:
    streams = [load_martingale_stream(args.martingale_replay, args.martingale_allocation)]
    for symbol in [s.strip() for s in args.trend_symbols.split(",") if s.strip()]:
        streams.append(build_trend_stream(args.market_data, symbol, args.trend_allocation))
    for symbol in [s.strip() for s in args.funding_symbols.split(",") if s.strip()]:
        streams.append(build_funding_stream(args.funding_data, symbol, args.funding_allocation, SEGMENTS["full"][0], SEGMENTS["full"][1]))
    combined = combine_streams(streams, args.budget)
    report = build_candidate_report(args.profile, combined, args.budget)
    write_reports(report, args.out_json, args.out_md)
    return report


def main() -> int:
    args = parse_args()
    report = run_from_args(args)
    print(json.dumps({
        "profile": report["profile"],
        "passes_offline": report["passes_offline"],
        "live_parity_status": report["live_parity_status"],
        "annualized_return_pct": report["full_metrics"].get("annualized_return_pct"),
        "max_drawdown_pct": report["full_metrics"].get("max_drawdown_pct"),
    }, sort_keys=True))
    return 0
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
python3 -m unittest tests/verification/hybrid_frontier_probe_sample.test.py
node --test tests/verification/hybrid_frontier_probe_contract.test.mjs
python3 -m py_compile scripts/hybrid_martingale_frontier_probe.py
```

Expected: Python `OK`; Node `3 pass`; py_compile exit 0.

- [ ] **Step 5: Commit**

```bash
git add scripts/hybrid_martingale_frontier_probe.py tests/verification/hybrid_frontier_probe_sample.test.py
git commit -m "feat: 修复思路 输出混合前沿探针报告"
```

---

### Task 8: Smoke Run Against Local Data

**Files:**
- Create: `docs/superpowers/reports/2026-06-30-hybrid-frontier-smoke.md`
- Generated: `/tmp/hybrid_frontier_smoke.json`

- [ ] **Step 1: Run full verification tests**

Run:

```bash
python3 -m unittest tests/verification/hybrid_frontier_probe_sample.test.py
node --test tests/verification/hybrid_frontier_probe_contract.test.mjs
python3 -m py_compile scripts/hybrid_martingale_frontier_probe.py
```

Expected: all pass.

- [ ] **Step 2: Run a local smoke probe**

Run:

```bash
python3 scripts/hybrid_martingale_frontier_probe.py \
  --profile aggressive \
  --budget 5000 \
  --martingale-replay docs/superpowers/reports/replay_aggressive_4000.json \
  --martingale-allocation 1500 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --trend-symbols BTCUSDT,ETHUSDT,BNBUSDT \
  --trend-allocation 750 \
  --funding-symbols BTCUSDT,ETHUSDT \
  --funding-allocation 250 \
  --out-json /tmp/hybrid_frontier_smoke.json \
  --out-md docs/superpowers/reports/2026-06-30-hybrid-frontier-smoke.md
```

Expected: command exits 0 and prints one JSON summary with `"live_parity_status": "research_only"`.

- [ ] **Step 3: Inspect smoke JSON**

Run:

```bash
python3 - <<'PY'
import json
obj=json.load(open('/tmp/hybrid_frontier_smoke.json'))
print(obj['profile'], obj['passes_offline'], obj['live_parity_status'])
print(round(obj['full_metrics']['annualized_return_pct'], 4), round(obj['full_metrics']['max_drawdown_pct'], 4))
print(obj['full_gate'])
print(obj['segment_gate'])
PY
```

Expected: prints profile, pass/fail status, annualized return, DD, and gate violations. A fail result is acceptable; this task validates the probe, not the strategy.

- [ ] **Step 4: Commit smoke report**

```bash
git add docs/superpowers/reports/2026-06-30-hybrid-frontier-smoke.md
git commit -m "docs: 修复思路 记录混合前沿探针 smoke"
```

---

### Task 9: Completion Review For Phase 1 Probe

**Files:**
- Modify only if needed: `docs/superpowers/reports/2026-06-30-hybrid-frontier-smoke.md`

- [ ] **Step 1: Check git status**

Run:

```bash
git status --short
```

Expected: no unexpected modified tracked files. If `/tmp/hybrid_frontier_smoke.json` exists, it is outside git and does not matter.

- [ ] **Step 2: Run verification suite one final time**

Run:

```bash
python3 -m unittest tests/verification/hybrid_frontier_probe_sample.test.py
node --test tests/verification/hybrid_frontier_probe_contract.test.mjs
python3 -m py_compile scripts/hybrid_martingale_frontier_probe.py
```

Expected: all pass.

- [ ] **Step 3: Summarize current frontier evidence**

Open `docs/superpowers/reports/2026-06-30-hybrid-frontier-smoke.md` and ensure it contains:

```markdown
This is Phase 1 research-only evidence and is not live-ready.
```

If the smoke report does not include that sentence, add it and commit:

```bash
git add docs/superpowers/reports/2026-06-30-hybrid-frontier-smoke.md
git commit -m "docs: 修复思路 标注混合探针非实盘结论"
```

- [ ] **Step 4: Decide next work**

If `passes_offline` is true, write a new promotion spec for the exact mechanisms used by the passing candidate. If `passes_offline` is false, broaden Phase 1 search in a new plan by adding deterministic grid search over allocations and symbols, while preserving the same gates and `research_only` status.
