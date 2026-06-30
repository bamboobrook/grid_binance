#!/usr/bin/env python3
"""Research-only wide search for hybrid martingale frontier candidates."""
from __future__ import annotations

import argparse
import importlib.util
import itertools
import json
from pathlib import Path

PROBE_PATH = Path(__file__).with_name("hybrid_martingale_frontier_probe.py")
SPEC = importlib.util.spec_from_file_location("hybrid_probe", PROBE_PATH)
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)


def parse_csv(value: str) -> list[str]:
    return [item.strip() for item in value.split(",") if item.strip()]


def scale_stream(stream: dict, allocation: float) -> dict:
    scaled = dict(stream)
    scaled["points"] = [
        {"timestamp_ms": point["timestamp_ms"], "equity_quote": point["equity_quote"] * allocation}
        for point in stream["points"]
    ]
    scaled["max_capital_used_quote"] = float(stream.get("max_capital_used_quote", 1.0)) * allocation
    scaled["budget_blocked_events"] = int(stream.get("budget_blocked_events", 0))
    return scaled


def summarize_frontier(rows: list[dict]) -> dict:
    summary = {}
    for profile in probe.PROFILE_TARGETS:
        subset = [row for row in rows if row["profile"] == profile]
        seg_cap = [row for row in subset if row.get("seg_pass") and row.get("cap", 999999) < 5000]
        passes = [row for row in subset if row.get("pass")]
        near = [row for row in seg_cap if not row.get("pass")]
        summary[profile] = {
            "rows": len(subset),
            "passes": len(passes),
            "best_ann_seg_cap": max(seg_cap, key=lambda row: row["ann"], default=None),
            "best_dd_seg_cap": min(seg_cap, key=lambda row: row["dd"], default=None),
            "top_near_misses": sorted(near, key=lambda row: row["ann"], reverse=True)[:5],
        }
    return summary
