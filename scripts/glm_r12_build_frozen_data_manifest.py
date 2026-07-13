#!/usr/bin/env python3
"""Build the corrected Round 12 market/funding data manifest.

The backtest loader prefers `futures_usdt_perp` / `1m`. The original Round 12
manifest omitted those filters and therefore counted spot and higher-timeframe
rows as duplicates. This implementation uses the same market selection as the
loader and records both development and opened-holdout coverage.
"""

import argparse
import hashlib
import json
import os
import sqlite3
import subprocess
import sys
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


def file_metadata(path, include_hash=True):
    stat = os.stat(path)
    return {
        "path": path,
        "size_bytes": stat.st_size,
        "mtime_ns": stat.st_mtime_ns,
        "sha256": sha256_file(path) if include_hash else None,
        "sha256_complete": include_hash,
    }


def expected_minute_count(start, end):
    first = ((start + MINUTE_MS - 1) // MINUTE_MS) * MINUTE_MS
    last = (end // MINUTE_MS) * MINUTE_MS
    return 0 if first > last else ((last - first) // MINUTE_MS) + 1


def canonical_value(value):
    if value is None:
        return "NULL"
    if isinstance(value, float):
        return value.hex()
    return str(value)


def market_stats(conn, symbol, start, end):
    query = """
        SELECT open_time, open, high, low, close, volume, close_time
        FROM klines
        WHERE symbol=? AND market_type=? AND timeframe=?
          AND open_time>=? AND open_time<=?
        ORDER BY open_time, rowid
    """
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
        min_time = timestamp if min_time is None else min_time
        max_time = timestamp
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
        digest.update(("|".join(canonical_value(value) for value in row) + "\n").encode())

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
    query = """
        SELECT funding_time, funding_rate, mark_price, typeof(mark_price)
        FROM funding_rates
        WHERE symbol=? AND funding_time>=? AND funding_time<=?
        ORDER BY funding_time, rowid
    """
    digest = hashlib.sha256()
    row_count = 0
    duplicate_rows = 0
    text_mark_price_rows = 0
    min_time = None
    max_time = None
    previous_time = None
    gap_count_over_9h = 0
    max_gap_hours = 0.0

    for funding_time, funding_rate, mark_price, mark_type in conn.execute(
        query, (symbol, start, end)
    ):
        funding_time = int(funding_time)
        row_count += 1
        min_time = funding_time if min_time is None else min_time
        max_time = funding_time
        if funding_time == previous_time:
            duplicate_rows += 1
        elif previous_time is not None:
            gap_hours = (funding_time - previous_time) / 3_600_000.0
            max_gap_hours = max(max_gap_hours, gap_hours)
            if gap_hours > 9.0:
                gap_count_over_9h += 1
        previous_time = funding_time
        if mark_type == "text":
            text_mark_price_rows += 1
        digest.update(
            (
                f"{funding_time}|{canonical_value(funding_rate)}|"
                f"{canonical_value(mark_price)}\n"
            ).encode()
        )

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
    digest = hashlib.sha256()
    rows = list(
        conn.execute(
            """
            SELECT open_time, premium FROM premium_index
            WHERE symbol=? AND open_time>=? AND open_time<=?
            ORDER BY open_time
            """,
            (symbol, start, end),
        )
    )
    for row in rows:
        digest.update(("|".join(canonical_value(value) for value in row) + "\n").encode())
    return {
        "available": bool(rows),
        "row_count": len(rows),
        "min_open_time": rows[0][0] if rows else None,
        "max_open_time": rows[-1][0] if rows else None,
        "canonical_sha256": digest.hexdigest(),
    }


def git_head():
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], text=True, stderr=subprocess.DEVNULL
        ).strip()
    except (OSError, subprocess.CalledProcessError):
        return None


def window_stats(market_conn, funding_conn, premium_conn, start, end):
    symbols = {}
    for symbol in CANDIDATE_SYMBOLS:
        market = market_stats(market_conn, symbol, start, end)
        funding = funding_stats(funding_conn, symbol, start, end)
        premium = premium_stats(premium_conn, symbol, start, end)
        symbols[symbol] = {"market": market, "funding": funding, "premium_index": premium}
        print(
            f"  {symbol}: market={market['unique_open_times']}/{market['expected_minutes']} "
            f"funding={funding['row_count']} covered={funding['range_covered']}",
            flush=True,
        )
    return symbols


def coverage_gate(symbols):
    market_failures = [
        symbol for symbol, stats in symbols.items() if not stats["market"]["range_complete"]
    ]
    funding_failures = [
        symbol for symbol, stats in symbols.items() if not stats["funding"]["range_covered"]
    ]
    return {
        "passed": not market_failures and not funding_failures,
        "market_failures": market_failures,
        "funding_failures": funding_failures,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--market-db", default="data/market_data_full.db")
    parser.add_argument("--funding-db", default="data/funding_rates_round12.db")
    parser.add_argument("--premium-db", default="data/premium_index.db")
    parser.add_argument(
        "--out",
        default=(
            "docs/superpowers/artifacts/glm-martingale-core-round12/"
            "run-manifests/r12-data-manifest.json"
        ),
    )
    parser.add_argument("--provenance-json")
    parser.add_argument("--holdout-opened", action="store_true")
    parser.add_argument("--skip-file-hash", action="store_true")
    args = parser.parse_args()

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    premium_conn = (
        sqlite3.connect(f"file:{args.premium_db}?mode=ro", uri=True)
        if args.premium_db and os.path.exists(args.premium_db)
        else None
    )
    market_conn = sqlite3.connect(f"file:{args.market_db}?mode=ro", uri=True)
    funding_conn = sqlite3.connect(f"file:{args.funding_db}?mode=ro", uri=True)

    print("Development window:", flush=True)
    development = window_stats(
        market_conn, funding_conn, premium_conn, DEV_START, DEV_END
    )
    holdout = None
    if args.holdout_opened:
        print("Holdout window:", flush=True)
        holdout = window_stats(
            market_conn, funding_conn, premium_conn, HOLDOUT_START, HOLDOUT_END
        )

    market_conn.close()
    funding_conn.close()
    if premium_conn is not None:
        premium_conn.close()

    provenance = None
    if args.provenance_json:
        with open(args.provenance_json, encoding="utf-8") as handle:
            provenance = json.load(handle)

    manifest = {
        "schema_version": 2,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "code_commit": git_head(),
        "selection": {"market_type": MARKET_TYPE, "timeframe": TIMEFRAME},
        "files": {
            "market": file_metadata(args.market_db, not args.skip_file_hash),
            "funding": file_metadata(args.funding_db, not args.skip_file_hash),
            "premium_index": (
                file_metadata(args.premium_db, not args.skip_file_hash)
                if args.premium_db and os.path.exists(args.premium_db)
                else None
            ),
        },
        "engine": {
            "path": "target/release/portfolio_budget_replay",
            "sha256": (
                sha256_file("target/release/portfolio_budget_replay")
                if os.path.exists("target/release/portfolio_budget_replay")
                else None
            ),
        },
        "development_window": {
            "start_ms": DEV_START,
            "end_ms": DEV_END,
            "symbols": development,
            "gate": coverage_gate(development),
        },
        "holdout_window": {
            "start_ms": HOLDOUT_START,
            "end_ms": HOLDOUT_END,
            "opened": args.holdout_opened,
            "symbols": holdout,
            "gate": coverage_gate(holdout) if holdout is not None else None,
        },
        "fetch_provenance": provenance,
        "known_limitations": [
            "premium-index coverage is recorded but is not a hard gate for candidates that do not consume it",
            "original Round12 development-data HTTP response hashes were not preserved",
        ],
    }

    with open(args.out, "w", encoding="utf-8") as handle:
        json.dump(manifest, handle, indent=2, sort_keys=True)
        handle.write("\n")

    print(f"Wrote {args.out}")
    print(f"Development gate: {manifest['development_window']['gate']['passed']}")
    if not manifest["development_window"]["gate"]["passed"]:
        print("FAIL: development data coverage is incomplete", file=sys.stderr)
        return 1
    if args.holdout_opened:
        print(f"Holdout gate: {manifest['holdout_window']['gate']['passed']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
