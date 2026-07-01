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


def compact_equity_points(points: list[dict]) -> list[dict]:
    if len(points) <= 3:
        return points
    compacted = []
    day_points = []
    current_day = int(points[0]["timestamp_ms"]) // MS_PER_DAY

    def flush_day() -> None:
        if not day_points:
            return
        keep = [
            day_points[0],
            max(day_points, key=lambda point: float(point["equity_quote"])),
            min(day_points, key=lambda point: float(point["equity_quote"])),
            day_points[-1],
        ]
        seen = set()
        for point in sorted(keep, key=lambda item: int(item["timestamp_ms"])):
            timestamp = int(point["timestamp_ms"])
            if timestamp not in seen:
                compacted.append(point)
                seen.add(timestamp)

    for point in points:
        day = int(point["timestamp_ms"]) // MS_PER_DAY
        if day != current_day:
            flush_day()
            day_points.clear()
            current_day = day
        day_points.append(point)
    flush_day()
    return compacted


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

    points = compact_equity_points(points)

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


def scale_dgt_stream(stream: dict, principal_quote: float) -> dict:
    if principal_quote <= 0:
        raise ValueError("principal_quote must be positive")
    base_principal = float(stream["principal_quote"])
    if base_principal <= 0:
        raise ValueError("stream principal_quote must be positive")
    scale = principal_quote / base_principal
    symbol = stream["symbols"][0]
    grid_spacing = float(stream["grid_spacing"])
    half_grid_count = int(stream["half_grid_count"])
    return {
        **stream,
        "name": f"dgt:{symbol}:gs{grid_spacing}:h{half_grid_count}:p{principal_quote}",
        "points": [
            {"timestamp_ms": point["timestamp_ms"], "equity_quote": float(point["equity_quote"]) * scale}
            for point in stream["points"]
        ],
        "max_input_quote": float(stream["max_input_quote"]) * scale,
        "max_capital_used_quote": float(stream.get("max_capital_used_quote", stream["max_input_quote"])) * scale,
        "total_input_quote": float(stream["total_input_quote"]) * scale,
        "total_fee_quote": float(stream["total_fee_quote"]) * scale,
        "principal_quote": principal_quote,
    }


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


def streams_share_timestamps(streams: list[dict]) -> bool:
    if not streams:
        return False
    first_points = streams[0]["points"]
    for stream in streams[1:]:
        points = stream["points"]
        if len(points) != len(first_points):
            return False
        if any(left["timestamp_ms"] != right["timestamp_ms"] for left, right in zip(first_points, points)):
            return False
    return True


def combine_metadata(streams: list[dict], points: list[dict]) -> dict:
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


def combine_streams(streams: list[dict]) -> dict:
    if not streams:
        raise ValueError("at least one stream is required")
    if streams_share_timestamps(streams):
        points = [
            {
                "timestamp_ms": timestamp_point["timestamp_ms"],
                "equity_quote": sum(float(stream["points"][index]["equity_quote"]) for stream in streams),
            }
            for index, timestamp_point in enumerate(streams[0]["points"])
        ]
        return combine_metadata(streams, points)

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
    return combine_metadata(streams, points)


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
    bars_cache = {}
    unit_stream_cache = {}
    for group in groups:
        for spacing in spacings:
            for half_count in half_counts:
                for principal in principals:
                    streams = []
                    invalid = None
                    for symbol in group:
                        if symbol not in bars_cache:
                            bars = load_1m_bars(
                                args.market_data,
                                symbol,
                                args.market_type,
                                SEGMENTS["full"][0],
                                SEGMENTS["full"][1],
                            )
                            if len(bars) < args.min_bars:
                                invalid = f"insufficient bars for {symbol}: {len(bars)}"
                                break
                            bars_cache[symbol] = bars
                        key = (symbol, spacing, half_count)
                        if key not in unit_stream_cache:
                            unit_stream_cache[key] = simulate_dgt_symbol(
                                symbol,
                                bars_cache[symbol],
                                1.0,
                                spacing,
                                half_count,
                                args.fee_bps,
                            )
                        streams.append(scale_dgt_stream(unit_stream_cache[key], principal))
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
    Path(out_md).write_text("\n".join(lines))


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
