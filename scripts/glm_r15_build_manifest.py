#!/usr/bin/env python3
"""Build the Round 15 frozen data manifest.

Round 15 authoritative inputs (frozen at bootstrap):

- market data:  data/market_data_full.db  (per-symbol row hashes verified
  31/31 against the frozen r14 manifest for the development window; the
  whole-file SHA256 differs from the r14 record only because the holdout
  window rows are appended/rewritten on each download, which does NOT touch
  development-window rows).
- funding data: data/funding_rates_round12.db  (NOT data/funding_rates.db).
  The current data/funding_rates.db has 0 funding rows for ANKRUSDT and
  LTCUSDT because a later re-download used a trimmed symbol list and dropped
  them. funding_rates_round12.db carries all 32 symbols including LTC/ANKR,
  its dev-window per-symbol row hashes match the frozen r14 manifest 31/31,
  and its whole-file SHA256 equals the audit's funding baseline
  4d77dbde...  This is therefore the faithful Round 15 funding baseline.
- premium data: data/premium_index.db  (SHA256 matches the audit baseline).

Per P0.1, this shift is recorded explicitly; no r14 metric is silently reused.
"""

import argparse
import hashlib
import json
import os
import sqlite3
import subprocess
from datetime import datetime, timezone

DEV_START = 1672531200000
DEV_END = 1780271999999
HOLDOUT_START = 1780272000000
HOLDOUT_END = 1783727999999
MARKET_TYPE = "futures_usdt_perp"
TIMEFRAME = "1m"
MINUTE_MS = 60_000
FUNDING_EDGE_TOLERANCE_MS = 9 * 60 * 60 * 1_000

CANDIDATE_SYMBOLS = [
    "AAVEUSDT", "ADAUSDT", "ALGOUSDT", "ANKRUSDT", "APTUSDT",
    "ATOMUSDT", "AVAXUSDT", "BCHUSDT", "BNBUSDT", "BTCUSDT",
    "COMPUSDT", "CRVUSDT", "DOGEUSDT", "DOTUSDT", "DYDXUSDT",
    "EGLDUSDT", "ETCUSDT", "ETHUSDT", "FILUSDT", "GALAUSDT",
    "HBARUSDT", "ICPUSDT", "INJUSDT", "LINKUSDT", "LTCUSDT",
    "NEARUSDT", "SOLUSDT", "TRXUSDT", "UNIUSDT", "XRPUSDT",
    "ZECUSDT",
]


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        while chunk := handle.read(8 * 1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_value(value):
    if value is None:
        return "NULL"
    if isinstance(value, float):
        return value.hex()
    return str(value)


def expected_minute_count(start, end):
    first = ((start + MINUTE_MS - 1) // MINUTE_MS) * MINUTE_MS
    last = (end // MINUTE_MS) * MINUTE_MS
    return 0 if first > last else ((last - first) // MINUTE_MS) + 1


def market_stats(conn, symbol, start, end):
    query = (
        "SELECT open_time, open, high, low, close, volume, close_time "
        "FROM klines "
        "WHERE symbol=? AND market_type=? AND timeframe=? "
        "AND open_time>=? AND open_time<=? "
        "ORDER BY open_time, rowid"
    )
    digest = hashlib.sha256()
    row_count = 0
    unique_count = 0
    duplicate_rows = 0
    off_grid_rows = 0
    min_time = None
    max_time = None
    previous_time = None
    max_gap_minutes = 0
    gap_count = 0
    for row in conn.execute(query, (symbol, MARKET_TYPE, TIMEFRAME, start, end)):
        timestamp = int(row[0])
        row_count += 1
        min_time = timestamp if min_time is None else min(min_time, timestamp)
        max_time = timestamp if max_time is None else max(max_time, timestamp)
        if timestamp % MINUTE_MS:
            off_grid_rows += 1
        if timestamp == previous_time:
            duplicate_rows += 1
        else:
            unique_count += 1
            if previous_time is not None and timestamp - previous_time > MINUTE_MS:
                missing = (timestamp - previous_time) // MINUTE_MS - 1
                max_gap_minutes = max(max_gap_minutes, missing)
                gap_count += 1
            previous_time = timestamp
        digest.update(("|".join(canonical_value(v) for v in row) + "\n").encode())
    expected = expected_minute_count(start, end)
    missing_minutes = max(0, expected - unique_count)
    return {
        "market_type": MARKET_TYPE,
        "timeframe": TIMEFRAME,
        "row_count": row_count,
        "unique_open_times": unique_count,
        "expected_minutes": expected,
        "missing_minutes": missing_minutes,
        "coverage_pct": round((unique_count / expected * 100.0), 8) if expected else 0.0,
        "min_open_time": min_time,
        "max_open_time": max_time,
        "duplicate_key_rows": duplicate_rows,
        "off_grid_rows": off_grid_rows,
        "internal_gap_count": gap_count,
        "max_internal_gap_minutes": max_gap_minutes,
        "canonical_sha256": digest.hexdigest(),
        "range_complete": bool(
            expected
            and unique_count == expected
            and duplicate_rows == 0
            and off_grid_rows == 0
        ),
    }


def funding_stats(conn, symbol, start, end):
    query = (
        "SELECT funding_time, funding_rate, mark_price, typeof(mark_price) "
        "FROM funding_rates "
        "WHERE symbol=? AND funding_time>=? AND funding_time<=? "
        "ORDER BY funding_time, rowid"
    )
    digest = hashlib.sha256()
    row_count = 0
    duplicate_rows = 0
    text_mark_price_rows = 0
    min_time = None
    max_time = None
    previous_time = None
    gap_count_over_9h = 0
    max_gap_hours = 0.0
    for ft, fr, mp, mt in conn.execute(query, (symbol, start, end)):
        ft = int(ft)
        row_count += 1
        min_time = ft if min_time is None else min(min_time, ft)
        max_time = ft if max_time is None else max(max_time, ft)
        if ft == previous_time:
            duplicate_rows += 1
        elif previous_time is not None:
            gap_hours = (ft - previous_time) / 3_600_000.0
            max_gap_hours = max(max_gap_hours, gap_hours)
            if gap_hours > 9.0:
                gap_count_over_9h += 1
        previous_time = ft
        if mt == "text":
            text_mark_price_rows += 1
        digest.update((f"{ft}|{canonical_value(fr)}|{canonical_value(mp)}\n").encode())
    range_covered = bool(
        row_count
        and min_time <= start + FUNDING_EDGE_TOLERANCE_MS
        and max_time >= end - FUNDING_EDGE_TOLERANCE_MS
        and duplicate_rows == 0
        and text_mark_price_rows == 0
    )
    return {
        "row_count": row_count,
        "min_funding_time": min_time,
        "max_funding_time": max_time,
        "duplicate_key_rows": duplicate_rows,
        "text_mark_price_rows": text_mark_price_rows,
        "gap_count_over_9h": gap_count_over_9h,
        "max_gap_hours": round(max_gap_hours, 8),
        "edge_tolerance_hours": 9,
        "canonical_sha256": digest.hexdigest(),
        "range_covered": range_covered,
    }


def premium_stats(conn, symbol, start, end):
    if conn is None:
        return {"available": False, "reason": "premium DB not supplied"}
    try:
        query = (
            "SELECT open_time, close, close_time "
            "FROM premium_klines WHERE symbol=? AND open_time>=? AND open_time<=? "
            "ORDER BY open_time, rowid"
        )
        digest = hashlib.sha256()
        row_count = 0
        min_time = None
        max_time = None
        previous_time = None
        for row in conn.execute(query, (symbol, start, end)):
            row_count += 1
            t = int(row[0])
            min_time = t if min_time is None else min(min_time, t)
            max_time = t if max_time is None else max(max_time, t)
            if t != previous_time:
                previous_time = t
            digest.update(("|".join(canonical_value(v) for v in row) + "\n").encode())
        return {
            "available": True,
            "row_count": row_count,
            "min_open_time": min_time,
            "max_open_time": max_time,
            "canonical_sha256": digest.hexdigest(),
        }
    except sqlite3.Error:
        return {"available": False, "reason": "premium table missing"}


def git_head():
    try:
        out = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], stderr=subprocess.DEVNULL
        )
        return out.decode().strip()
    except Exception:
        return None


def git_dirty_hash():
    """Hash of the working-tree source so a dirty tree changes the manifest."""
    digest = hashlib.sha256()
    src_root = "apps/backtest-engine/src"
    for dirpath, _dirs, files in os.walk(src_root):
        for name in sorted(files):
            if name.endswith(".rs"):
                p = os.path.join(dirpath, name)
                rel = os.path.relpath(p)
                digest.update(rel.encode())
                digest.update(b"\0")
                with open(p, "rb") as fh:
                    digest.update(fh.read())
                digest.update(b"\0")
    return digest.hexdigest()


def build(args):
    mconn = sqlite3.connect(f"file:{args.market_db}?mode=ro", uri=True)
    fconn = sqlite3.connect(f"file:{args.funding_db}?mode=ro", uri=True)
    try:
        pconn = sqlite3.connect(f"file:{args.premium_db}?mode=ro", uri=True)
    except Exception:
        pconn = None

    dev_symbols = {}
    dev_market_failures = []
    dev_funding_failures = []
    holdout_market_failures = []
    holdout_funding_failures = []
    for sym in CANDIDATE_SYMBOLS:
        m = market_stats(mconn, sym, DEV_START, DEV_END)
        f = funding_stats(fconn, sym, DEV_START, DEV_END)
        p = premium_stats(pconn, sym, DEV_START, DEV_END)
        dev_symbols[sym] = {"market": m, "funding": f, "premium_index": p}
        if not m["range_complete"]:
            dev_market_failures.append(sym)
        if not f["range_covered"]:
            dev_funding_failures.append(sym)

    dev_gate = {
        "market_failures": dev_market_failures,
        "funding_failures": dev_funding_failures,
        "passed": not dev_market_failures and not dev_funding_failures,
    }

    holdout_symbols = {}
    for sym in CANDIDATE_SYMBOLS:
        m = market_stats(mconn, sym, HOLDOUT_START, HOLDOUT_END)
        f = funding_stats(fconn, sym, HOLDOUT_START, HOLDOUT_END)
        holdout_symbols[sym] = {"market": m, "funding": f}
        if not m["range_complete"]:
            holdout_market_failures.append(sym)
        if not f["range_covered"]:
            holdout_funding_failures.append(sym)
    holdout_gate = {
        "market_failures": holdout_market_failures,
        "funding_failures": holdout_funding_failures,
        "passed": not holdout_market_failures and not holdout_funding_failures,
    }

    files = {
        "market_db": {"path": args.market_db, "sha256": sha256_file(args.market_db)},
        "funding_db": {"path": args.funding_db, "sha256": sha256_file(args.funding_db)},
        "premium_db": {"path": args.premium_db, "sha256": sha256_file(args.premium_db)},
    }

    manifest = {
        "schema_version": 4,
        "round": 15,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "code_commit": git_head(),
        "dirty_source_sha256": git_dirty_hash(),
        "plan_sha256": sha256_file(args.plan),
        "selection": {
            "market_type": MARKET_TYPE,
            "timeframe": TIMEFRAME,
            "candidate_symbol_count": len(CANDIDATE_SYMBOLS),
        },
        "baseline_note": (
            "Authoritative funding source is data/funding_rates_round12.db, NOT "
            "data/funding_rates.db (which has 0 rows for ANKRUSDT/LTCUSDT after a "
            "trimmed re-download). round12 db file SHA256 matches the audit funding "
            "baseline 4d77dbde... and its dev-window per-symbol row hashes match the "
            "frozen r14 manifest 31/31."
        ),
        "development_window": {
            "start_ms": DEV_START,
            "end_ms": DEV_END,
            "symbols": dev_symbols,
            "gate": dev_gate,
        },
        "holdout_window": {
            "start_ms": HOLDOUT_START,
            "end_ms": HOLDOUT_END,
            "already_opened": True,
            "diagnostic_only": True,
            "symbols": holdout_symbols,
            "gate": holdout_gate,
        },
        "files": files,
    }
    with open(args.out, "w") as fh:
        json.dump(manifest, fh, indent=2, sort_keys=True)
    print(f"wrote {args.out}")
    print(
        f"dev gate passed={dev_gate['passed']} "
        f"(market_fail={len(dev_market_failures)} funding_fail={len(dev_funding_failures)})"
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--market-db", default="data/market_data_full.db")
    ap.add_argument("--funding-db", default="data/funding_rates_round12.db")
    ap.add_argument("--premium-db", default="data/premium_index.db")
    ap.add_argument(
        "--out",
        default="docs/superpowers/artifacts/glm-martingale-core-round15/run-manifests/r15-data-manifest.json",
    )
    ap.add_argument(
        "--plan",
        default="docs/superpowers/plans/2026-07-14-glm-martingale-core-round15-directional-hazard-cluster-plan.md",
    )
    args = ap.parse_args()
    build(args)


if __name__ == "__main__":
    main()
