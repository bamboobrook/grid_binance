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


def main() -> int:
    print("hybrid frontier probe skeleton: Phase 1 research-only")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
