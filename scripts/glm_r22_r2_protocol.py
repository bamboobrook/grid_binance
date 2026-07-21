#!/usr/bin/env python3
"""Round 22 R2: freeze the continuous prequential protocol.

Plan §4: 12 × 90-day test blocks, ONE continuous replay (no equity reset at
block boundaries). Selector re-selects pairs using only data <= block_t_start
- purge. After a block closes, its data may enter the next fit window.

Key difference from R21: R21 ran each block independently with fresh equity.
R22 runs blocks sequentially on ONE continuous account.
"""
from __future__ import annotations

import hashlib
import json
import subprocess
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round22"
OUT_DIR = ART / "r2"
OUT_DIR.mkdir(parents=True, exist_ok=True)
OUT = OUT_DIR / "prequential-protocol.json"

DEV_START = 1672531200000  # 2023-01-01
DEV_END = 1780271999999    # 2026-05-31
DAY_MS = 86_400_000
BLOCK_SIZE_DAYS = 90
MIN_FIT_DAYS = 180
PURGE_DAYS = 1
COLD_START_OFFSETS_DAYS = [0, 30, 60, 90, 120]


def git_commit_sha():
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT,
            stderr=subprocess.DEVNULL).decode().strip()
    except Exception:
        return None


def build_blocks():
    blocks = []
    i = 0
    while True:
        test_start = DEV_START + (MIN_FIT_DAYS + PURGE_DAYS) * DAY_MS + i * BLOCK_SIZE_DAYS * DAY_MS
        test_end = test_start + BLOCK_SIZE_DAYS * DAY_MS - 1
        if test_end > DEV_END:
            partial_days = (DEV_END - test_start) // DAY_MS + 1
            if partial_days < 60:
                break
            test_end = DEV_END
        fit_start = DEV_START
        fit_end = test_start - PURGE_DAYS * DAY_MS
        fit_days = (fit_end - fit_start) // DAY_MS
        blocks.append({
            "block_id": f"tb{i+1:02d}",
            "test_start_ms": test_start,
            "test_end_ms": test_end,
            "test_start_utc": datetime.fromtimestamp(
                test_start / 1000, tz=timezone.utc).isoformat(),
            "test_end_utc": datetime.fromtimestamp(
                test_end / 1000, tz=timezone.utc).isoformat(),
            "test_days": (test_end - test_start) // DAY_MS + 1,
            "fit_start_ms": fit_start,
            "fit_end_ms": fit_end,
            "fit_start_utc": datetime.fromtimestamp(
                fit_start / 1000, tz=timezone.utc).isoformat(),
            "fit_end_utc": datetime.fromtimestamp(
                fit_end / 1000, tz=timezone.utc).isoformat(),
            "fit_days": fit_days,
            "purge_days": PURGE_DAYS,
            "causal_invariant": "fit_end < test_start - purge",
            "reporting_rule": ("if fit_days<180 OR test_days<90: raw only; "
                               "stitched test days >=365 => portfolio ann"),
        })
        i += 1
        if len(blocks) > 20:
            break
    return blocks


def main():
    blocks = build_blocks()
    cold_starts = []
    for offset_days in COLD_START_OFFSETS_DAYS:
        start_ms = DEV_START + offset_days * DAY_MS
        cold_starts.append({
            "offset_days": offset_days,
            "cold_start_ms": start_ms,
            "cold_start_utc": datetime.fromtimestamp(
                start_ms / 1000, tz=timezone.utc).isoformat(),
        })
    total_test_days = sum(b["test_days"] for b in blocks)
    protocol = {
        "phase": "R22 R2 continuous prequential protocol (plan §4)",
        "frozen_at_utc": datetime.now(timezone.utc).isoformat(),
        "frozen": True,
        "committed": True,
        "commit_sha_at_freeze": git_commit_sha(),
        "dev_window": {
            "start_ms": DEV_START, "end_ms": DEV_END,
            "start_utc": "2023-01-01T00:00:00Z",
            "end_utc": "2026-05-31T23:59:59.999Z",
        },
        "continuous_protocol": {
            "rule": "ONE continuous shared cash/margin/equity account across all 12 blocks; no equity reset at block boundaries",
            "selector_rule": "pairs re-selected using only data <= block_t_start - purge; after block closes its data enters next fit window",
            "transition_rule": "selector swap: conservatively close old cycles with full cost, then open new cycles; active cycle legs frozen mid-block",
        },
        "test_blocks": blocks,
        "cold_start_offsets": cold_starts,
        "total_test_blocks": len(blocks),
        "total_stitched_test_days": total_test_days,
        "stitched_ann_eligible": total_test_days >= 365,
        "frozen_parameters": {
            "block_size_days": BLOCK_SIZE_DAYS,
            "min_fit_days": MIN_FIT_DAYS,
            "purge_days": PURGE_DAYS,
            "purge_rule": ("purge >= max(signal lookback, funding latency "
                           "8h, 1 completed decision bar 1m)"),
            "fit_window_mode": "expanding (fit_start = dev_start always)",
            "stitch_rule": ("stitched equity contains ONLY test-block days; "
                            "fit-window days are never in the stitch; "
                            "NO equity reset at block boundaries"),
            "refit_rule": ("each test block triggers a separate refit on the "
                           "expanding window; fit_end < test_start - purge"),
            "annualization_rule": ("single block fit_days<180 OR test_days<90 "
                                   "=> raw only; stitched test days >=365 "
                                   "=> portfolio ann"),
        },
        "fail_close_invariants": [
            "Any fit with fit_end_ms >= test_start_ms - purge_ms => invalid_data_leakage",
            "Any runner loading an outer-train-end fit to replay an earlier inner block => blocked",
            "Stitched equity containing any fit-window day => invalid",
            "Model hyperparameter selection by strategy validation return => invalid_overfit",
            "Equity reset at block boundary => invalid (must be continuous)",
            "Selector reading block_t data for block_t pair selection => leakage",
            "90-day block ann entering target judgment => rejected (need stitched >=365d)",
        ],
    }
    with open(OUT, "w") as fh:
        json.dump(protocol, fh, indent=2, sort_keys=True)
    print(f"wrote {OUT}")
    print(f"blocks: {len(blocks)}, total_test_days: {total_test_days}, "
          f"ann_eligible: {total_test_days >= 365}")
    print(f"commit: {protocol['commit_sha_at_freeze']}")


if __name__ == "__main__":
    main()
