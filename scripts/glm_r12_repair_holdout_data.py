#!/usr/bin/env python3
"""Repair Round 12 holdout market/funding gaps from Binance USD-M endpoints."""

import argparse
import hashlib
import json
import sqlite3
import time
from datetime import datetime, timezone

import requests


HOLDOUT_START = 1780272000000
HOLDOUT_END = 1783727999999
MINUTE_MS = 60_000
BASE_URL = "https://fapi.binance.com"
SYMBOLS = [
    "BNBUSDT", "TRXUSDT", "BCHUSDT", "AAVEUSDT", "SOLUSDT", "DOTUSDT",
    "ANKRUSDT", "XRPUSDT", "ETHUSDT", "BTCUSDT", "ADAUSDT", "LINKUSDT",
    "AVAXUSDT", "LTCUSDT", "DOGEUSDT",
]


def request_json(session, path, params, response_digest, retries=4):
    url = f"{BASE_URL}{path}"
    for attempt in range(retries):
        response = session.get(url, params=params, timeout=30)
        response_digest.update(response.content)
        if response.status_code in (418, 429):
            time.sleep(min(30, 2 ** (attempt + 1)))
            continue
        response.raise_for_status()
        return response.json()
    raise RuntimeError(f"request failed after {retries} attempts: {url} {params}")


def expected_minutes(start_ms, end_ms):
    last = end_ms // MINUTE_MS * MINUTE_MS
    return set(range(start_ms, last + 1, MINUTE_MS))


def repair_market_symbol(conn, session, symbol, start_ms, end_ms):
    present = {
        row[0]
        for row in conn.execute(
            """
            SELECT open_time FROM klines
            WHERE symbol=? AND market_type='futures_usdt_perp' AND timeframe='1m'
              AND open_time BETWEEN ? AND ?
            """,
            (symbol, start_ms, end_ms),
        )
    }
    missing = expected_minutes(start_ms, end_ms) - present
    digest = hashlib.sha256()
    request_count = 0
    inserted = 0
    if missing:
        cursor = min(missing)
        fetch_end = max(missing)
        while cursor <= fetch_end:
            rows = request_json(
                session,
                "/fapi/v1/klines",
                {
                    "symbol": symbol,
                    "interval": "1m",
                    "startTime": cursor,
                    "endTime": fetch_end,
                    "limit": 1000,
                },
                digest,
            )
            request_count += 1
            if not rows:
                break
            selected = [row for row in rows if int(row[0]) in missing]
            conn.executemany(
                """
                INSERT INTO klines (
                    symbol, market_type, timeframe, open_time, open, high, low,
                    close, volume, close_time
                ) VALUES (?, 'futures_usdt_perp', '1m', ?, ?, ?, ?, ?, ?, ?)
                """,
                [
                    (
                        symbol,
                        int(row[0]),
                        float(row[1]),
                        float(row[2]),
                        float(row[3]),
                        float(row[4]),
                        float(row[5]),
                        int(row[6]),
                    )
                    for row in selected
                ],
            )
            inserted += len(selected)
            next_cursor = int(rows[-1][0]) + MINUTE_MS
            if next_cursor <= cursor:
                raise RuntimeError(f"non-advancing kline cursor for {symbol}")
            cursor = next_cursor
            if len(rows) < 1000:
                break
            time.sleep(0.1)
        conn.commit()

    remaining = expected_minutes(start_ms, end_ms) - {
        row[0]
        for row in conn.execute(
            """
            SELECT open_time FROM klines
            WHERE symbol=? AND market_type='futures_usdt_perp' AND timeframe='1m'
              AND open_time BETWEEN ? AND ?
            """,
            (symbol, start_ms, end_ms),
        )
    }
    return {
        "missing_before": len(missing),
        "inserted": inserted,
        "missing_after": len(remaining),
        "request_count": request_count,
        "http_response_stream_sha256": digest.hexdigest() if request_count else None,
    }


def repair_funding_symbol(conn, session, symbol, start_ms, end_ms):
    digest = hashlib.sha256()
    rows = request_json(
        session,
        "/fapi/v1/fundingRate",
        {
            "symbol": symbol,
            "startTime": start_ms,
            "endTime": end_ms,
            "limit": 1000,
        },
        digest,
    )
    before = conn.total_changes
    conn.executemany(
        "INSERT OR REPLACE INTO funding_rates VALUES (?, ?, ?, ?)",
        [
            (
                symbol,
                int(row["fundingTime"]),
                float(row["fundingRate"]),
                float(row["markPrice"]) if row.get("markPrice") else None,
            )
            for row in rows
        ],
    )
    conn.commit()
    changed = conn.total_changes - before
    count, min_time, max_time = conn.execute(
        """
        SELECT COUNT(*), MIN(funding_time), MAX(funding_time)
        FROM funding_rates
        WHERE symbol=? AND funding_time BETWEEN ? AND ?
        """,
        (symbol, start_ms, end_ms),
    ).fetchone()
    return {
        "api_rows": len(rows),
        "rows_changed": changed,
        "db_rows_in_range": count,
        "min_funding_time": min_time,
        "max_funding_time": max_time,
        "request_count": 1,
        "http_response_sha256": digest.hexdigest(),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--market-db", default="data/market_data_full.db")
    parser.add_argument("--funding-db", default="data/funding_rates_round12.db")
    parser.add_argument(
        "--out",
        default=(
            "docs/superpowers/artifacts/glm-martingale-core-round12/"
            "run-manifests/r12-holdout-fetch-provenance.json"
        ),
    )
    parser.add_argument("--start-ms", type=int, default=HOLDOUT_START)
    parser.add_argument("--end-ms", type=int, default=HOLDOUT_END)
    parser.add_argument("--symbols", nargs="+", default=SYMBOLS)
    args = parser.parse_args()

    market_conn = sqlite3.connect(args.market_db, timeout=60)
    funding_conn = sqlite3.connect(args.funding_db, timeout=60)
    session = requests.Session()
    session.headers["User-Agent"] = "grid-binance-round12-audit/1.0"

    result = {
        "schema_version": 1,
        "source": "Binance USD-M Futures REST API",
        "base_url": BASE_URL,
        "fetched_at": datetime.now(timezone.utc).isoformat(),
        "window": {"start_ms": args.start_ms, "end_ms": args.end_ms},
        "symbols": {},
    }
    for symbol in args.symbols:
        symbol = symbol.strip().upper()
        print(f"Repairing {symbol}...", flush=True)
        market = repair_market_symbol(
            market_conn, session, symbol, args.start_ms, args.end_ms
        )
        funding = repair_funding_symbol(
            funding_conn, session, symbol, args.start_ms, args.end_ms
        )
        result["symbols"][symbol] = {"market": market, "funding": funding}
        print(
            f"  market {market['missing_before']} -> {market['missing_after']}; "
            f"funding rows={funding['db_rows_in_range']}",
            flush=True,
        )

    market_conn.close()
    funding_conn.close()
    with open(args.out, "w", encoding="utf-8") as handle:
        json.dump(result, handle, indent=2, sort_keys=True)
        handle.write("\n")
    print(f"Wrote {args.out}")

    remaining = {
        symbol: stats["market"]["missing_after"]
        for symbol, stats in result["symbols"].items()
        if stats["market"]["missing_after"]
    }
    if remaining:
        raise SystemExit(f"market gaps remain: {remaining}")


if __name__ == "__main__":
    main()
