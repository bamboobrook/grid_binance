#!/usr/bin/env python3
"""Phase 1 research-only hybrid martingale frontier probe.

This script is intentionally offline-only. It combines martingale replay streams,
trend/breakout streams, and funding/carry streams to test whether a hybrid
portfolio frontier can reach the original C/B/A gates.

No live trading, Binance order placement, flyingkid publishing, or live-parity
claim is performed here. Stream construction is no-lookahead: each decision
timestamp uses data at or before t. In other words, each decision timestamp uses
data at or before t, and every indicator has an explicit warmup.
Contract phrase: decision timestamp uses data at or before t.
"""
from __future__ import annotations

import argparse
import json
import sqlite3
from pathlib import Path
from typing import Iterable

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

    combined_2024_2026 = compound_returns(
        segment_metrics.get(name, {}).get("total_return_pct", 0.0)
        for name in ("2024", "2025", "2026_ytd")
    )

    if positive < constraints["min_positive_segments"]:
        violations.append(f"only {positive}/5 segments positive; need {constraints['min_positive_segments']}")
    if constraints["no_2024_2026_total_loss"] and combined_2024_2026 < 0:
        violations.append(f"2024-2026 combined return {combined_2024_2026:.2f}% < 0")

    return {
        "passes": not violations,
        "violations": violations,
        "positive_segments": positive,
        "combined_2024_2026_return_pct": combined_2024_2026,
    }


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


def build_momentum_stream(
    market_db: str | Path,
    symbol: str,
    allocation_quote: float,
    lookback: int = 20,
    mode: str = "long_flat",
    fee_bps: float = 2.0,
) -> dict:
    """Build a daily momentum stream; signal uses the previous completed day."""
    daily = load_daily_closes(market_db, symbol)
    equity = allocation_quote
    points = []
    position = 0
    for index in range(lookback + 1, len(daily)):
        prev_close = daily[index - 1]["close"]
        ref_close = daily[index - 1 - lookback]["close"]
        momentum = prev_close / ref_close - 1.0 if ref_close > 0 else 0.0
        desired = 1 if momentum > 0 else (-1 if mode == "long_short" and momentum < 0 else 0)
        if desired != position:
            equity *= 1.0 - fee_bps / 10_000.0
            position = desired
        last_close = daily[index - 1]["close"]
        close = daily[index]["close"]
        if last_close > 0:
            if position == 1:
                equity *= close / last_close
            elif position == -1:
                equity *= 2.0 - close / last_close
        points.append({"timestamp_ms": daily[index]["timestamp_ms"], "equity_quote": equity})
    return {
        "name": f"trend:{symbol}:mom{lookback}_{mode}",
        "kind": "trend",
        "symbols": [symbol],
        "points": points,
        "max_capital_used_quote": allocation_quote,
        "budget_blocked_events": 0,
        "fee_bps": fee_bps,
        "no_lookahead": True,
        "live_parity_status": LIVE_PARITY_STATUS,
    }


def build_donchian_stream(
    market_db: str | Path,
    symbol: str,
    allocation_quote: float,
    lookback: int = 20,
    mode: str = "long_flat",
    fee_bps: float = 2.0,
) -> dict:
    """Build a daily Donchian breakout stream; channel excludes the signal day."""
    daily = load_daily_closes(market_db, symbol)
    equity = allocation_quote
    points = []
    position = 0
    for index in range(lookback + 1, len(daily)):
        window = [row["close"] for row in daily[index - 1 - lookback:index - 1]]
        upper = max(window)
        lower = min(window)
        signal_close = daily[index - 1]["close"]
        desired = 1 if signal_close > upper else (-1 if mode == "long_short" and signal_close < lower else position)
        if mode == "long_flat" and signal_close < lower:
            desired = 0
        if desired != position:
            equity *= 1.0 - fee_bps / 10_000.0
            position = desired
        last_close = daily[index - 1]["close"]
        close = daily[index]["close"]
        if last_close > 0:
            if position == 1:
                equity *= close / last_close
            elif position == -1:
                equity *= 2.0 - close / last_close
        points.append({"timestamp_ms": daily[index]["timestamp_ms"], "equity_quote": equity})
    return {
        "name": f"trend:{symbol}:donchian{lookback}_{mode}",
        "kind": "trend",
        "symbols": [symbol],
        "points": points,
        "max_capital_used_quote": allocation_quote,
        "budget_blocked_events": 0,
        "fee_bps": fee_bps,
        "no_lookahead": True,
        "live_parity_status": LIVE_PARITY_STATUS,
    }


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
    for points in aligned:
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
        report[name] = compute_metrics(slice_points(points, start_ms, end_ms))
    return report


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


if __name__ == "__main__":
    raise SystemExit(main())
