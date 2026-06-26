#!/usr/bin/env python3
"""定时下载 Binance K线数据到 market_data.db（spot + futures_usdt_perp）
用法: python3 download_klines.py [db_path]
默认每天由 crontab 调用，从各 symbol 最新 open_time 续传到 now。
"""
import sqlite3
import time
import sys
import os
from datetime import datetime, timezone

import requests

DB_PATH = sys.argv[1] if len(sys.argv) > 1 else "/home/bumblebee/Project/grid_binance/data/market_data_full.db"
SPOT_BASE = "https://api.binance.com"
FUTURES_BASE = "https://fapi.binance.com"
INTERVAL = "1m"
LIMIT = 1000
SLEEP = 0.15  # rate limit 友好
MAX_RETRIES = 3


def fetch_klines(base_url, symbol, start_ms, end_ms=None):
    """从 Binance REST API 下载 klines，返回原始 JSON 行列表"""
    path = "/fapi/v1/klines" if "fapi" in base_url else "/api/v3/klines"
    params = {"symbol": symbol, "interval": INTERVAL, "startTime": int(start_ms), "limit": LIMIT}
    if end_ms:
        params["endTime"] = int(end_ms)
    for attempt in range(MAX_RETRIES):
        try:
            r = requests.get(f"{base_url}{path}", params=params, timeout=15)
            if r.status_code in (418, 429):
                wait = min(60, 3 * (2 ** attempt))
                print(f"  {symbol}: rate limited {r.status_code}, wait {wait}s")
                time.sleep(wait)
                continue
            r.raise_for_status()
            return r.json()
        except requests.RequestException as e:
            if attempt < MAX_RETRIES - 1:
                time.sleep(2 ** attempt)
            else:
                print(f"  {symbol}: fetch failed after {MAX_RETRIES} retries: {e}")
                return []
    return []


def main():
    if not os.path.exists(DB_PATH):
        print(f"ERROR: DB not found: {DB_PATH}")
        sys.exit(1)

    conn = sqlite3.connect(DB_PATH, timeout=30)
    c = conn.cursor()

    # 获取所有 (symbol, market_type) + 最新 open_time
    c.execute("""
        SELECT symbol, market_type, MAX(open_time) as latest
        FROM klines GROUP BY symbol, market_type
    """)
    pairs = c.fetchall()
    now_str = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    print(f"[{now_str}] {len(pairs)} symbol-market pairs to sync")

    total = 0
    for symbol, market_type, latest_ms in pairs:
        base = FUTURES_BASE if "futures" in (market_type or "") else SPOT_BASE
        start_ms = (latest_ms or 0) + 60_000
        now_ms = int(time.time() * 1000)
        if start_ms >= now_ms:
            continue

        inserted = 0
        cursor = start_ms
        while cursor < now_ms:
            rows = fetch_klines(base, symbol, cursor, now_ms)
            if not rows:
                break
            normalized = []
            for r in rows:
                normalized.append((
                    symbol, market_type, INTERVAL,
                    int(r[0]), float(r[1]), float(r[2]), float(r[3]),
                    float(r[4]), float(r[5]), int(r[6]),
                ))
            c.executemany(
                "INSERT OR IGNORE INTO klines VALUES (?,?,?,?,?,?,?,?,?,?)",
                normalized,
            )
            conn.commit()
            inserted += len(normalized)
            next_cursor = int(rows[-1][0]) + 60_000
            if next_cursor <= cursor or len(rows) < LIMIT:
                break
            cursor = next_cursor
            time.sleep(SLEEP)

        if inserted > 0:
            total += inserted
            print(f"  {symbol} {market_type}: +{inserted} bars")

    conn.close()
    print(f"[{datetime.now(timezone.utc).strftime('%Y-%m-%d %H:%M UTC')}] DONE: {total} bars inserted")


if __name__ == "__main__":
    main()
