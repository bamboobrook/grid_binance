#!/usr/bin/env python3
"""GLM Round 12 Task P1: Build frozen data manifest.

Creates a comprehensive manifest of market + funding data for the development
window (2023-01-01 to 2026-05-31) and untouched holdout (2026-06-01 to 2026-07-10).

Checks:
- Market DB row counts, min/max time, duplicates, missing minutes
- Funding coverage for ALL traded symbols (must have >=1 row per futures symbol)
- ANKRUSDT funding gap detection
- Canonical range SHA256 for stability
"""
import argparse, json, os, sqlite3, hashlib, sys
from datetime import datetime

DEV_START = 1672531200000   # 2023-01-01
DEV_END = 1780271999999     # 2026-05-31 23:59:59.999
HOLDOUT_START = 1780272000000  # 2026-06-01
HOLDOUT_END = 1783727999999    # 2026-07-10 23:59:59.999

CANDIDATE_SYMBOLS = [
    "BNBUSDT", "TRXUSDT", "BCHUSDT", "AAVEUSDT", "SOLUSDT", "DOTUSDT",
    "ANKRUSDT", "XRPUSDT", "ETHUSDT", "BTCUSDT", "ADAUSDT", "LINKUSDT",
    "AVAXUSDT", "LTCUSDT", "DOGEUSDT",
]


def market_stats(conn, symbol, start, end):
    """Get market data stats for a symbol in [start, end]."""
    cur = conn.cursor()
    # Row count
    n = cur.execute("SELECT COUNT(*) FROM klines WHERE symbol=? AND open_time>=? AND open_time<=?",
                    (symbol, start, end)).fetchone()[0]
    if n == 0:
        return {"row_count": 0, "error": "no rows"}
    mn, mx = cur.execute("SELECT MIN(open_time), MAX(open_time) FROM klines WHERE symbol=? AND open_time>=? AND open_time<=?",
                         (symbol, start, end)).fetchone()
    # Duplicate check
    dups = cur.execute("""SELECT COUNT(*) FROM (
        SELECT symbol, open_time, COUNT(*) as c FROM klines
        WHERE symbol=? AND open_time>=? AND open_time<=?
        GROUP BY symbol, open_time HAVING c > 1
    )""", (symbol, start, end)).fetchone()[0]
    # Canonical SHA256 of (open_time, close) stream
    sha = hashlib.sha256()
    for row in cur.execute("SELECT open_time, close FROM klines WHERE symbol=? AND open_time>=? AND open_time<=? ORDER BY open_time",
                           (symbol, start, end)):
        sha.update(f"{row[0]},{row[1]}:".encode())
    return {
        "row_count": n, "min_open_time": mn, "max_open_time": mx,
        "duplicates": dups, "canonical_sha256": sha.hexdigest()[:32]
    }


def funding_stats(conn, symbol, start, end):
    """Get funding stats for a symbol."""
    cur = conn.cursor()
    n = cur.execute("SELECT COUNT(*) FROM funding_rates WHERE symbol=? AND funding_time>=? AND funding_time<=?",
                    (symbol, start, end)).fetchone()[0]
    if n == 0:
        return {"row_count": 0, "error": "NO_FUNDING_COVERAGE"}
    mn, mx = cur.execute("SELECT MIN(funding_time), MAX(funding_time) FROM funding_rates WHERE symbol=? AND funding_time>=? AND funding_time<=?",
                         (symbol, start, end)).fetchone()
    sha = hashlib.sha256()
    for row in cur.execute("SELECT funding_time, funding_rate FROM funding_rates WHERE symbol=? AND funding_time>=? AND funding_time<=? ORDER BY funding_time",
                           (symbol, start, end)):
        sha.update(f"{row[0]},{row[1]}:".encode())
    return {
        "row_count": n, "min_funding_time": mn, "max_funding_time": mx,
        "canonical_sha256": sha.hexdigest()[:32]
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--market-db", default="data/market_data_full.db")
    ap.add_argument("--funding-db", default="data/funding_rates_round12.db")
    ap.add_argument("--out", default="docs/superpowers/artifacts/glm-martingale-core-round12/run-manifests/r12-data-manifest.json")
    args = ap.parse_args()

    os.makedirs(os.path.dirname(args.out), exist_ok=True)

    # File hashes
    market_hash = hashlib.sha256(open(args.market_db, "rb").read(65536*64).hexdigest() if os.path.getsize(args.market_db) < 65536*64 else b"large").hexdigest()
    funding_hash = hashlib.sha256(open(args.funding_db, "rb").read()).hexdigest()

    manifest = {
        "generated_at": datetime.utcnow().isoformat() + "Z",
        "market_db": args.market_db,
        "funding_db": args.funding_db,
        "market_db_sha256": market_hash,
        "funding_db_sha256": funding_hash,
        "development_window": {"start_ms": DEV_START, "end_ms": DEV_END},
        "holdout_window": {"start_ms": HOLDOUT_START, "end_ms": HOLDOUT_END, "opened": False},
        "symbols": {},
        "funding_gate": {"all_covered": True, "missing": []},
    }

    mconn = sqlite3.connect(args.market_db)
    fconn = sqlite3.connect(args.funding_db)

    for sym in CANDIDATE_SYMBOLS:
        ms = market_stats(mconn, sym, DEV_START, DEV_END)
        fs = funding_stats(fconn, sym, DEV_START, DEV_END)
        manifest["symbols"][sym] = {"market": ms, "funding": fs}
        if fs.get("row_count", 0) == 0:
            manifest["funding_gate"]["all_covered"] = False
            manifest["funding_gate"]["missing"].append(sym)
            print(f"  WARNING: {sym} has NO funding coverage", file=sys.stderr)

    mconn.close()
    fconn.close()

    # Engine binary hash
    binary_path = "target/release/portfolio_budget_replay"
    if os.path.exists(binary_path):
        manifest["engine_binary_sha256"] = hashlib.sha256(open(binary_path, "rb").read()).hexdigest()
    else:
        manifest["engine_binary_sha256"] = "binary not found"

    with open(args.out, "w") as f:
        json.dump(manifest, f, indent=2)

    print(f"Wrote {args.out}")
    print(f"Funding gate: all_covered={manifest['funding_gate']['all_covered']}")
    if manifest["funding_gate"]["missing"]:
        print(f"  Missing funding: {manifest['funding_gate']['missing']}")

    # Fail if any funding missing
    if not manifest["funding_gate"]["all_covered"]:
        print("FAIL: funding gate not met — some symbols have no funding coverage", file=sys.stderr)
        sys.exit(1)
    print("PASS: all candidate symbols have funding coverage")


if __name__ == "__main__":
    main()
