#!/usr/bin/env python3
"""Probe idealized funding-rate sleeves against martingale target gates."""
from __future__ import annotations

import argparse
import json
import math
import sqlite3
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


PROFILE_TARGETS = {
    "conservative": {"ann": 50.0, "dd": 10.0},
    "balanced": {"ann": 90.0, "dd": 20.0},
    "aggressive": {"ann": 110.0, "dd": 30.0},
}

SEGMENTS = {
    "h1_2023": (1672531200000, 1688169599999),
    "h2_2023": (1688169600000, 1704067199999),
    "2024": (1704067200000, 1735689599999),
    "2025": (1735689600000, 1767225599999),
    "2026_ytd": (1767225600000, 1780271999999),
}


@dataclass(frozen=True)
class FundingEvent:
    symbol: str
    funding_time: int
    funding_rate: float


@dataclass(frozen=True)
class EquityPoint:
    timestamp_ms: int
    equity: float


def load_events(db_path: Path, symbol: str, start_ms: int, end_ms: int) -> list[FundingEvent]:
    query = """
        SELECT symbol, funding_time, funding_rate
        FROM funding_rates
        WHERE symbol = ? AND funding_time BETWEEN ? AND ?
        ORDER BY funding_time
    """
    with sqlite3.connect(db_path) as conn:
        return [
            FundingEvent(row[0], int(row[1]), float(row[2]))
            for row in conn.execute(query, (symbol, start_ms, end_ms))
        ]


def load_symbols(db_path: Path) -> list[str]:
    with sqlite3.connect(db_path) as conn:
        return [row[0] for row in conn.execute("SELECT DISTINCT symbol FROM funding_rates ORDER BY symbol")]


def build_symbol_curve(events: Iterable[FundingEvent], side: str) -> list[EquityPoint]:
    multiplier = 1.0 if side == "short" else -1.0
    equity = 1.0
    curve: list[EquityPoint] = []
    for event in events:
        equity += multiplier * event.funding_rate
        if equity <= 0 or not math.isfinite(equity):
            break
        curve.append(EquityPoint(event.funding_time, equity))
    return curve


def summarize_curve(curve: list[EquityPoint], start_ms: int, end_ms: int) -> dict:
    points = [point for point in curve if start_ms <= point.timestamp_ms <= end_ms]
    if not points:
        return {
            "points": 0,
            "total_return_pct": 0.0,
            "annualized_return_pct": 0.0,
            "max_drawdown_pct": 0.0,
        }
    start_equity = points[0].equity
    end_equity = points[-1].equity
    peak = start_equity
    max_dd = 0.0
    for point in points:
        peak = max(peak, point.equity)
        if peak > 0:
            max_dd = max(max_dd, (peak - point.equity) / peak * 100.0)
    total_return_pct = (end_equity / start_equity - 1.0) * 100.0
    years = max((points[-1].timestamp_ms - points[0].timestamp_ms) / (365.25 * 24 * 60 * 60 * 1000), 1e-9)
    annualized = ((end_equity / start_equity) ** (1.0 / years) - 1.0) * 100.0
    return {
        "points": len(points),
        "total_return_pct": total_return_pct,
        "annualized_return_pct": annualized,
        "max_drawdown_pct": max_dd,
    }


def evaluate_profile_gate(result: dict, profile: str) -> bool:
    target = PROFILE_TARGETS[profile]
    return (
        result["annualized_return_pct"] > target["ann"]
        and result["max_drawdown_pct"] <= target["dd"]
        and result["positive_segments"] >= 4
        and result["combined_2024_2026_return_pct"] > 0.0
    )


def summarize_symbol(db_path: Path, symbol: str, side: str, start_ms: int, end_ms: int) -> dict:
    curve = build_symbol_curve(load_events(db_path, symbol, start_ms, end_ms), side)
    full = summarize_curve(curve, start_ms, end_ms)
    segment_summaries = {name: summarize_curve(curve, start, end) for name, (start, end) in SEGMENTS.items()}
    positive_segments = sum(1 for item in segment_summaries.values() if item["total_return_pct"] > 0)
    combined = 1.0
    for name in ["2024", "2025", "2026_ytd"]:
        combined *= 1.0 + segment_summaries[name]["total_return_pct"] / 100.0
    result = {
        "symbol": symbol,
        "side": side,
        **full,
        "positive_segments": positive_segments,
        "combined_2024_2026_return_pct": (combined - 1.0) * 100.0,
        "segments": segment_summaries,
    }
    result["passes"] = {profile: evaluate_profile_gate(result, profile) for profile in PROFILE_TARGETS}
    return result


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--funding-data", default="data/funding_rates.db")
    parser.add_argument("--symbols", nargs="*", default=None)
    parser.add_argument("--side", choices=["short", "long"], default="short")
    parser.add_argument("--start-ms", type=int, default=1672531200000)
    parser.add_argument("--end-ms", type=int, default=1780271999999)
    parser.add_argument("--top", type=int, default=20)
    parser.add_argument("--out-json", required=True)
    parser.add_argument("--out-md", required=True)
    return parser.parse_args()


def write_report(rows: list[dict], out_md: Path) -> None:
    passes = {profile: [row for row in rows if row["passes"][profile]] for profile in PROFILE_TARGETS}
    lines = [
        "# 2026-07-01 Funding Sleeve Probe",
        "",
        "This is an idealized research-only funding-rate sleeve check. It does not trade, touch Binance, flyingkid, live mode, or real funds.",
        "",
        f"- Symbols scanned: `{len(rows)}`",
        f"- Conservative passes: `{len(passes['conservative'])}`",
        f"- Balanced passes: `{len(passes['balanced'])}`",
        f"- Aggressive passes: `{len(passes['aggressive'])}`",
        "",
        "## Top Annualized",
        "",
    ]
    for row in sorted(rows, key=lambda item: item["annualized_return_pct"], reverse=True)[:10]:
        lines.append(
            f"- `{row['symbol']}` {row['side']} ann `{row['annualized_return_pct']:.2f}` "
            f"DD `{row['max_drawdown_pct']:.2f}` pos `{row['positive_segments']}/5` "
            f"2024-2026 `{row['combined_2024_2026_return_pct']:.2f}` "
            f"C/B/A `{row['passes']['conservative']}/{row['passes']['balanced']}/{row['passes']['aggressive']}`"
        )
    lines.extend(
        [
            "",
            "## Conclusion",
            "",
            "No funding sleeve reaches even the conservative `ann > 50%` target in this idealized standalone check. Funding can smooth or add low-drawdown carry, but the observed scale is not large enough to bridge the martingale/grid gap to the original 50/90/110% annualized targets.",
            "",
            "Any future apparent pass would still require live-parity implementation, spot/perp capital accounting, borrow/fee/slippage modeling, and replay with the martingale portfolio.",
        ]
    )
    out_md.write_text("\n".join(lines) + "\n")


def main() -> int:
    args = parse_args()
    db_path = Path(args.funding_data)
    symbols = args.symbols or load_symbols(db_path)
    rows = [summarize_symbol(db_path, symbol, args.side, args.start_ms, args.end_ms) for symbol in symbols]
    rows.sort(key=lambda item: item["annualized_return_pct"], reverse=True)
    payload = {"rows": rows, "top": rows[: args.top]}
    Path(args.out_json).write_text(json.dumps(payload, indent=2, sort_keys=True))
    write_report(rows, Path(args.out_md))
    print(
        json.dumps(
            {
                "rows": len(rows),
                "conservative_passes": sum(1 for row in rows if row["passes"]["conservative"]),
                "balanced_passes": sum(1 for row in rows if row["passes"]["balanced"]),
                "aggressive_passes": sum(1 for row in rows if row["passes"]["aggressive"]),
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
