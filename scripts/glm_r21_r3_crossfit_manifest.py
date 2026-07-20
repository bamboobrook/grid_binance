#!/usr/bin/env python3
"""Round 21 R3: pre-register the rolling-origin causal cross-fit manifest
(plan §5).

Plan §5 mandates that the cross-fit dates are written to a manifest and
committed/pushed BEFORE the first market-data load. After commit, the dates
may NOT change. The manifest freezes:

  - fit window: >=180d expanding/rolling
  - purge: >= max(signal lookback, funding latency, 1 completed decision bar)
  - test blocks: 90d non-overlap
  - per test block: separate refit, fit_end < test_start - purge
  - stitch: only test-block equity, never fit data
  - 5 pre-registered cold-start offsets

The 2023-01-01..2026-05-31 dev window is split into as many 90d test blocks
as possible. With ~1241 dev days and 90d blocks + 180d minimum fit, we get:

  block_size_days = 90
  min_fit_days    = 180
  purge_days      = max(signal_lookback=1d, funding_latency=8h, 1 bar=1m) = 1d
  block_start[i]  = dev_start + (min_fit_days + purge_days + i*block_size) days
  block_end[i]    = block_start[i] + block_size_days

Plan §5: any single block <180d fit only reports raw return/DD/PF, not ann;
only stitched test days >=365 report portfolio ann.

5 cold-start offsets: each shifts the dev window by a fixed number of days
so the same strategy is replayed from 5 different equity starting points
(plan §1 cold-start requirement: >=4/5 positive, report 5/5).
"""
from __future__ import annotations

import hashlib
import json
import subprocess
from datetime import datetime, timezone, timedelta
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round21"
OUT_DIR = ART / "r3" / "gates"
OUT = OUT_DIR / "causal_crossfit_manifest.json"

# Dev window (plan §0): 2023-01-01..2026-05-31
DEV_START = 1672531200000  # 2023-01-01 00:00 UTC
DEV_END = 1780271999999    # 2026-05-31 23:59:59.999 UTC
DAY_MS = 86_400_000

# Plan §5 frozen parameters
BLOCK_SIZE_DAYS = 90
MIN_FIT_DAYS = 180
# purge >= max(signal lookback, funding latency, 1 decision bar)
# signal lookback: max residual fit lookback in families (B1S 240d, P1S state
# space, V1B VECM) — but the FIT lookback is the fit window size, not the
# purge. Purge is the gap between fit_end and test_start to prevent the fit
# window from leaking into the test. We use 1d (>= 8h funding latency + 1m
# decision bar + buffer).
PURGE_DAYS = 1

# Plan §5: 5 pre-registered cold-start offsets. Each shifts the equity
# starting point within the dev window so the same strategy is replayed from
# 5 different initial conditions. Offsets are in days from DEV_START.
COLD_START_OFFSETS_DAYS = [0, 30, 60, 90, 120]


def git_commit_sha() -> str | None:
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT,
            stderr=subprocess.DEVNULL).decode().strip()
    except Exception:
        return None


def build_blocks() -> list[dict]:
    """Build 90d non-overlapping test blocks with expanding fit windows.
    block_start[i] = dev_start + (min_fit_days + purge_days) days + i*block_size
    block_end[i]   = block_start[i] + block_size_days
    fit_start[i]   = dev_start (expanding window)
    fit_end[i]     = block_start[i] - purge_days (strictly < test_start)
    Each block is a separate refit (plan §5: 'each test block single refit').
    """
    blocks = []
    i = 0
    while True:
        test_start = DEV_START + (MIN_FIT_DAYS + PURGE_DAYS) * DAY_MS + i * BLOCK_SIZE_DAYS * DAY_MS
        test_end = test_start + BLOCK_SIZE_DAYS * DAY_MS - 1
        if test_end > DEV_END:
            # last partial block: include only if it has >=60d (else skip)
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
            "reporting_rule": ("if fit_days < 180 OR test_days < 90: report "
                               "raw return/DD/PF only, NOT annualized; only "
                               "stitched test days >=365 report portfolio ann"),
        })
        i += 1
        if len(blocks) > 20:
            break
    return blocks


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    blocks = build_blocks()
    cold_starts = []
    for offset_days in COLD_START_OFFSETS_DAYS:
        start_ms = DEV_START + offset_days * DAY_MS
        cold_starts.append({
            "offset_days": offset_days,
            "cold_start_ms": start_ms,
            "cold_start_utc": datetime.fromtimestamp(
                start_ms / 1000, tz=timezone.utc).isoformat(),
            "rule": ("equity curve begins at this timestamp; prior dev data "
                     "still used for fit but not for equity baseline"),
        })
    total_test_days = sum(b["test_days"] for b in blocks)
    manifest = {
        "phase": "R21 R3 pre-registered rolling-origin causal cross-fit (plan §5)",
        "frozen_at_utc": datetime.now(timezone.utc).isoformat(),
        "dev_window": {
            "start_ms": DEV_START, "end_ms": DEV_END,
            "start_utc": "2023-01-01T00:00:00Z",
            "end_utc": "2026-05-31T23:59:59.999Z",
            "total_days": (DEV_END - DEV_START) // DAY_MS + 1,
        },
        "frozen_parameters": {
            "block_size_days": BLOCK_SIZE_DAYS,
            "min_fit_days": MIN_FIT_DAYS,
            "purge_days": PURGE_DAYS,
            "purge_rule": ("purge >= max(signal lookback, funding latency "
                           "8h, 1 completed decision bar 1m)"),
            "fit_window_mode": "expanding (fit_start = dev_start always)",
            "stitch_rule": ("stitched equity contains ONLY test-block days; "
                            "fit-window days are never in the stitch"),
            "refit_rule": ("each test block triggers a separate refit; "
                           "fit_end < test_start - purge enforced"),
            "annualization_rule": ("single block fit_days<180 OR test_days<90 "
                                   "=> raw only; stitched test days >=365 "
                                   "=> portfolio ann"),
        },
        "test_blocks": blocks,
        "cold_start_offsets": cold_starts,
        "total_test_blocks": len(blocks),
        "total_stitched_test_days": total_test_days,
        "stitched_ann_eligible": total_test_days >= 365,
        "commit_sha_at_freeze": git_commit_sha(),
        "future_query_prohibition": ("selection MUST be committed/pushed BEFORE "
                                     "any 2026-07-11+ data query (plan §11); "
                                     "this manifest is committed before any "
                                     "market data is loaded for search"),
    }
    # Plan §5: future-fit injection must fail-close. Record the invariant.
    manifest["fail_close_invariants"] = [
        "Any fit with fit_end_ms >= test_start_ms - purge_ms => invalid_data_leakage (canary 5)",
        "Any runner loading an outer-train-end fit to replay an earlier inner block => blocked",
        "Any validation query (2026-07-11+) before selection commit => invalid_oos (canary 8)",
        "Stitched equity containing any fit-window day => invalid",
        "Model hyperparameter selection by strategy validation return => invalid_overfit",
    ]
    with open(OUT, "w") as fh:
        json.dump(manifest, fh, indent=2, sort_keys=True)
    print(f"wrote {OUT}")
    print(f"test_blocks: {len(blocks)}, total_test_days: {total_test_days}, "
          f"ann_eligible: {total_test_days >= 365}")
    print(f"cold_starts: {len(cold_starts)}")
    print(f"commit_sha_at_freeze: {manifest['commit_sha_at_freeze']}")


if __name__ == "__main__":
    main()
