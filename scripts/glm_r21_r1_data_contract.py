#!/usr/bin/env python3
"""Round 21 R1: data contract freeze (plan §3 lines 100-109).

Plan §3 mandates:
  - data loader key MUST be (venue, market_type, symbol, timeframe), at least
    distinguishing:
        binance / spot / BTCUSDT / 1m
        binance / futures_usdt_perp / BTCUSDT / 1m
  - freeze per source: schema, row count, min/max, duplicate, missing-minute,
    OHLC, funding settle, premium, borrow availability, provenance hash.
  - 缺 borrow 数据时 reverse basis 禁止；不得把缺值当 0 或 forward-fill 下单.

PERFORMANCE: the 118 GB market_data_full.db has 1.04 B rows. Global aggregates
(COUNT(*), GROUP BY) on the full table are disk-bound and take many minutes.
The available index is idx_klines_symbol_time(symbol, open_time) — so per-symbol
queries are fast. This script freezes the contract using ONLY index-backed
queries (per B1S symbol + the schema), never a full-table scan.

This script is read-only against data/*.db. The borrow availability check is
the hard gate that determines whether B1S reverse-basis FO is allowed (plan
§6.2 line 187).
"""
from __future__ import annotations

import hashlib
import json
import sqlite3
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round21"
OUT = ART / "r1" / "data-contract.json"

# Plan §0: 2023-01-01..2026-05-31 is the dev/cross-fit window.
DEV_START = 1672531200000  # 2023-01-01 00:00 UTC
DEV_END = 1780271999999    # 2026-05-31 23:59:59.999 UTC
LOCK_FUTURE_START = 1783555200000  # 2026-07-11 00:00 UTC

MARKET_DB = ROOT / "data/market_data_full.db"
FUNDING_DB = ROOT / "data/funding_rates.db"
PREMIUM_DB = ROOT / "data/premium_index.db"

# Plan §6.2 line 183: at least BTC/ETH/BNB/SOL/XRP/DOGE six symbols.
B1S_UNIVERSE = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT"]


def stat_db(path: Path) -> dict:
    if not path.exists():
        return {"exists": False}
    st = path.stat()
    out: dict = {"exists": True, "size_bytes": st.st_size,
                 "size_gb": round(st.st_size / 1e9, 3),
                 "mtime_utc": datetime.fromtimestamp(
                     st.st_mtime, tz=timezone.utc).isoformat()}
    # Only SHA256 files < 1 GB. The 118 GB market_data_full.db is captured by
    # size+mtime+per-symbol row counts (index-backed, fast).
    if st.st_size < 1_000_000_000:
        h = hashlib.sha256()
        with open(path, "rb") as fh:
            while chunk := fh.read(8 << 20):
                h.update(chunk)
        out["sha256"] = h.hexdigest()
    else:
        out["sha256"] = None
        out["sha256_skipped_reason"] = (
            f"file is {st.st_size / 1e9:.1f} GB; per-symbol row counts serve "
            f"as the provenance fingerprint (full SHA deferred)")
    return out


def query_market_schema(db: Path) -> dict:
    """Schema + per-(market_type, symbol, tf) coverage using the index."""
    if not db.exists():
        return {"exists": False}
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    cur = con.cursor()
    out: dict = {"schema": {}, "loader_key_examples": [],
                 "b1s_universe_coverage": {}}
    cur.execute("SELECT sql FROM sqlite_master WHERE type='table'")
    out["schema"]["klines_ddl"] = [r[0] for r in cur.fetchall()]
    cur.execute("SELECT name, sql FROM sqlite_master WHERE type='index'")
    idx = cur.fetchall()
    out["schema"]["indexes"] = [{"name": n, "ddl": d} for n, d in idx]
    out["loader_key_examples"] = [
        {"venue": "binance", "market_type": "spot",
         "symbol": "BTCUSDT", "timeframe": "1m"},
        {"venue": "binance", "market_type": "futures_usdt_perp",
         "symbol": "BTCUSDT", "timeframe": "1m"},
    ]
    # For each B1S symbol, freeze spot+perp 1m coverage using the index
    # (per-symbol queries hit idx_klines_symbol_time — fast even on 1B rows).
    for sym in B1S_UNIVERSE:
        rec: dict = {"symbol": sym, "legs": {}}
        for mt in ("spot", "futures_usdt_perp"):
            # COUNT/MIN/MAX for this (symbol, market_type, timeframe) — index scan.
            cur.execute(
                "SELECT COUNT(*), MIN(open_time), MAX(open_time) "
                "FROM klines WHERE symbol=? AND market_type=? AND timeframe='1m'",
                (sym, mt))
            cnt, mn, mx = cur.fetchone()
            missing = None
            if cnt and mn and mx:
                expected = (mx - mn) // 60_000 + 1
                missing = max(0, expected - cnt)
            # Duplicates: same (symbol, market_type, open_time)
            cur.execute(
                "SELECT COUNT(*) FROM ("
                "  SELECT open_time, COUNT(*) c FROM klines "
                "  WHERE symbol=? AND market_type=? AND timeframe='1m' "
                "  GROUP BY open_time HAVING c > 1)",
                (sym, mt))
            dup_groups = cur.fetchone()[0]
            # Dev-window row count (B1S only trades in DEV window for cross-fit)
            cur.execute(
                "SELECT COUNT(*) FROM klines WHERE symbol=? AND market_type=? "
                "AND timeframe='1m' AND open_time BETWEEN ? AND ?",
                (sym, mt, DEV_START, DEV_END))
            dev_rows = cur.fetchone()[0]
            rec["legs"][mt] = {
                "rows_total": cnt,
                "rows_in_dev_window": dev_rows,
                "min_open_time": mn, "max_open_time": mx,
                "min_utc": (datetime.fromtimestamp(mn / 1000, tz=timezone.utc)
                            .isoformat() if mn else None),
                "max_utc": (datetime.fromtimestamp(mx / 1000, tz=timezone.utc)
                            .isoformat() if mx else None),
                "missing_minutes_estimate": missing,
                "duplicate_open_time_groups": dup_groups,
            }
        out["b1s_universe_coverage"][sym] = rec
    con.close()
    return out


def query_funding(db: Path) -> dict:
    if not db.exists():
        return {"exists": False}
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    cur = con.cursor()
    out: dict = {"schema": [], "total_rows": 0, "symbols": 0,
                 "min_time": None, "max_time": None}
    cur.execute("SELECT sql FROM sqlite_master WHERE type='table'")
    out["schema"] = [r[0] for r in cur.fetchall()]
    cur.execute("SELECT COUNT(*), COUNT(DISTINCT symbol), "
                "MIN(funding_time), MAX(funding_time) FROM funding_rates")
    cnt, nsym, mn, mx = cur.fetchone()
    out.update({"total_rows": cnt, "symbols": nsym,
                "min_time": mn, "max_time": mx,
                "min_utc": datetime.fromtimestamp(
                    mn / 1000, tz=timezone.utc).isoformat() if mn else None,
                "max_utc": datetime.fromtimestamp(
                    mx / 1000, tz=timezone.utc).isoformat() if mx else None})
    out["b1s_universe_funding"] = {}
    for sym in B1S_UNIVERSE:
        cur.execute("SELECT COUNT(*), MIN(funding_time), MAX(funding_time) "
                    "FROM funding_rates WHERE symbol=?", (sym,))
        cnt, mn, mx = cur.fetchone()
        out["b1s_universe_funding"][sym] = {
            "rows": cnt, "min_time": mn, "max_time": mx,
            "max_utc": datetime.fromtimestamp(
                mx / 1000, tz=timezone.utc).isoformat() if mx else None,
            "coverage_to_dev_end_2026_05_31": (
                mx is not None and mx >= DEV_END - 86_400_000),
        }
    con.close()
    return out


def query_premium(db: Path) -> dict:
    if not db.exists():
        return {"exists": False}
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    cur = con.cursor()
    out: dict = {"schema": [], "total_rows": 0, "symbols": 0}
    cur.execute("SELECT sql FROM sqlite_master WHERE type='table'")
    out["schema"] = [r[0] for r in cur.fetchall()]
    try:
        cur.execute("SELECT COUNT(*), COUNT(DISTINCT symbol), "
                    "MIN(open_time), MAX(open_time) FROM premium_index")
        cnt, nsym, mn, mx = cur.fetchone()
        out.update({"total_rows": cnt, "symbols": nsym,
                    "min_time": mn, "max_time": mx})
    except sqlite3.OperationalError:
        pass
    con.close()
    return out


def check_borrow_data() -> dict:
    """Plan §3 line 109: 缺 borrow 数据时 reverse basis 禁止."""
    sources_checked = []
    borrow_found = False
    for db in (ROOT / "data").glob("*.db"):
        try:
            con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
            cur = con.cursor()
            cur.execute("SELECT name FROM sqlite_master WHERE type='table'")
            tables = [r[0].lower() for r in cur.fetchall()]
            con.close()
            borrow_tables = [t for t in tables
                             if any(k in t for k in (
                                 "borrow", "shortable", "short_avail",
                                 "margin_asset", "leverage_bracket"))]
            sources_checked.append({
                "db": db.name, "tables": tables,
                "borrow_tables_found": borrow_tables})
            if borrow_tables:
                borrow_found = True
        except sqlite3.OperationalError:
            continue
    return {
        "borrow_data_exists": borrow_found,
        "sources_checked": sources_checked,
        "plan_§3_line_109_rule": ("缺 borrow 数据时 reverse basis 禁止；"
                                   "不得把缺值当 0 或 forward-fill 下单."),
        "consequence_for_B1S": ("reverse-basis FO (short spot + long perp, "
                                "plan §6.2 line 187) is BLOCKED until a "
                                "Binance borrow snapshot is captured. "
                                "positive-basis FO (long spot + short perp) "
                                "remains allowed: USD-M perp short does not "
                                "require a borrow snapshot."),
    }


def main() -> None:
    contract = {
        "phase": "R21 R1 data contract freeze (plan §3 lines 100-109)",
        "frozen_at_utc": datetime.now(timezone.utc).isoformat(),
        "loader_key": "(venue, market_type, symbol, timeframe)",
        "dev_window": {"start_ms": DEV_START, "end_ms": DEV_END,
                       "start_utc": "2023-01-01T00:00:00Z",
                       "end_utc": "2026-05-31T23:59:59.999Z"},
        "locked_future_window": {"start_ms": LOCK_FUTURE_START,
                                 "start_utc": "2026-07-11T00:00:00Z",
                                 "rule": ("2026-07-11+ is sealed until 30 full "
                                          "calendar days elapse AND selection is "
                                          "committed/pushed AND future data has "
                                          "never been queried (plan §11).")},
        "market_data": {**stat_db(MARKET_DB), **query_market_schema(MARKET_DB)},
        "funding_data": {**stat_db(FUNDING_DB), **query_funding(FUNDING_DB)},
        "premium_data": {**stat_db(PREMIUM_DB), **query_premium(PREMIUM_DB)},
        "borrow": check_borrow_data(),
        "frozen": True,
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(contract, fh, indent=2, sort_keys=True)
    print(f"wrote {OUT}")
    cov = contract["market_data"].get("b1s_universe_coverage", {})
    for sym in B1S_UNIVERSE:
        rec = cov.get(sym, {})
        legs = rec.get("legs", {})
        spot = legs.get("spot", {}).get("rows_in_dev_window", "?")
        perp = legs.get("futures_usdt_perp", {}).get("rows_in_dev_window", "?")
        print(f"  {sym}: spot={spot} perp={perp}")
    print(f"funding rows: {contract['funding_data'].get('total_rows')}")
    print(f"borrow exists: {contract['borrow']['borrow_data_exists']}")
    print(f"B1S reverse-basis: "
          f"{'ALLOWED' if contract['borrow']['borrow_data_exists'] else 'BLOCKED'}")


if __name__ == "__main__":
    main()
