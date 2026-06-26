#!/usr/bin/env python3
"""方向5: 下载 Binance USD-M funding rate 历史，存入 grid_binance/data/funding_rates.db
每 8 小时一条，用于后续 funding-aware 回测。
用法: python3 download_funding.py
"""
import sqlite3
import time
import sys
import os
from datetime import datetime, timezone

import requests

DB_PATH = "/home/bumblebee/Project/grid_binance/data/funding_rates.db"
BASE_URL = "https://fapi.binance.com"
SYMBOLS = [
    "BTCUSDT","ETHUSDT","BNBUSDT","SOLUSDT","XRPUSDT","DOGEUSDT","ADAUSDT","TRXUSDT",
    "AVAXUSDT","LINKUSDT","DOTUSDT","BCHUSDT","NEARUSDT","APTUSDT","ATOMUSDT","ETCUSDT",
    "HBARUSDT","ICPUSDT","UNIUSDT","FILUSDT","AAVEUSDT","INJUSDT","ZECUSDT","DASHUSDT",
    "ALGOUSDT","CRVUSDT","EGLDUSDT","DYDXUSDT","COMPUSDT","GALAUSDT"
]
START_MS = 1672531200000  # 2023-01-01
LIMIT = 1000
SLEEP = 0.15

def init_db(path):
    conn = sqlite3.connect(path)
    c = conn.cursor()
    c.execute("""
        CREATE TABLE IF NOT EXISTS funding_rates (
            symbol TEXT NOT NULL,
            funding_time INTEGER NOT NULL,
            funding_rate REAL NOT NULL,
            mark_price REAL,
            PRIMARY KEY (symbol, funding_time)
        )
    """)
    conn.commit()
    return conn

def fetch_funding(symbol, start_ms, end_ms):
    """从 Binance API 下载 funding rate 历史"""
    url = f"{BASE_URL}/fapi/v1/fundingRate"
    all_rows = []
    cursor = start_ms
    while cursor < end_ms:
        params = {"symbol": symbol, "startTime": cursor, "limit": LIMIT}
        for attempt in range(3):
            try:
                r = requests.get(url, params=params, timeout=15)
                if r.status_code in (418, 429):
                    wait = min(60, 3 * (2 ** attempt))
                    print(f"  {symbol}: rate limited, wait {wait}s")
                    time.sleep(wait)
                    continue
                r.raise_for_status()
                data = r.json()
                break
            except requests.RequestException as e:
                if attempt < 2:
                    time.sleep(2 ** attempt)
                else:
                    print(f"  {symbol}: failed after 3 retries: {e}")
                    return all_rows
        if not data:
            break
        for row in data:
            all_rows.append((
                symbol,
                int(row["fundingTime"]),
                float(row["fundingRate"]),
                float(row.get("markPrice") or 0),
            ))
        if len(data) < LIMIT:
            break
        cursor = int(data[-1]["fundingTime"]) + 1
        time.sleep(SLEEP)
    return all_rows

def main():
    now_ms = int(time.time() * 1000)
    conn = init_db(DB_PATH)
    c = conn.cursor()

    now_str = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    print(f"[{now_str}] Downloading funding rates for {len(SYMBOLS)} symbols since 2023-01-01")

    total = 0
    for symbol in SYMBOLS:
        # Check what we already have
        c.execute("SELECT MAX(funding_time) FROM funding_rates WHERE symbol=?", (symbol,))
        row = c.fetchone()
        start = row[0] + 1 if row[0] else START_MS

        if start >= now_ms:
            print(f"  {symbol}: up to date")
            continue

        rows = fetch_funding(symbol, start, now_ms)
        if rows:
            c.executemany("INSERT OR REPLACE INTO funding_rates VALUES (?,?,?,?)", rows)
            conn.commit()
            total += len(rows)
            print(f"  {symbol}: +{len(rows)} rates (total {total})")

    conn.close()
    print(f"[{datetime.now(timezone.utc).strftime('%Y-%m-%d %H:%M UTC')}] DONE: {total} funding rates")

    # Summary
    conn = sqlite3.connect(DB_PATH)
    c = conn.cursor()
    c.execute("SELECT COUNT(*), COUNT(DISTINCT symbol) FROM funding_rates")
    cnt, syms = c.fetchone()
    c.execute("SELECT MIN(funding_time), MAX(funding_time) FROM funding_rates")
    first, last = c.fetchone()
    print(f"DB: {cnt} rates, {syms} symbols, {first} to {last}")
    conn.close()

if __name__ == "__main__":
    main()
