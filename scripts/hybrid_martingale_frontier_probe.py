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
