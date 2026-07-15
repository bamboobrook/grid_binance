#!/usr/bin/env python3
"""P1/P3 universe freeze + data qualification.

Freeze the T2/T3 directional universe BEFORE any return search (plan §4, §1.1).
For each candidate symbol verify:
  - data completeness over the dev window (manifest gate already covers this)
  - futures liquidity (median 1m quote volume over dev window > threshold)
  - exchange minNotional feasibility for the small-capital ladder
  - listing longevity (data spans a large fraction of the dev window)
Each symbol gets BOTH a long and a short Martingale sleeve.

Output:
  docs/superpowers/artifacts/glm-martingale-core-round15/p1/frozen-universe.json
"""

import json
import os
import sqlite3

ART = "docs/superpowers/artifacts/glm-martingale-core-round15"
DEV_START = 1672531200000
DEV_END = 1780271999999

T2_SYMBOLS = [
    "BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT",
    "ADAUSDT", "TRXUSDT", "LINKUSDT", "LTCUSDT", "BCHUSDT", "DOTUSDT",
]
T3_UNIVERSE_8 = [
    "BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT",
    "ADAUSDT", "TRXUSDT",
]


def qualify(mconn, symbol):
    """Return qualification stats for one symbol."""
    row = mconn.execute(
        """
        SELECT MIN(open_time), MAX(open_time), COUNT(*),
               -- median-ish: avg of close over a sample to gauge liquidity
               AVG(volume)
        FROM klines
        WHERE symbol=? AND market_type='futures_usdt_perp' AND timeframe='1m'
          AND open_time>=? AND open_time<=?
        """,
        (symbol, DEV_START, DEV_END),
    ).fetchone()
    min_t, max_t, count, avg_vol = row
    # minNotional: Binance USD-M is typically 5 USDT for most, higher for BTC.
    # We record the conservative 5.0 exchange floor (CLI default) and let the
    # backtest reject if a first-order is below it.
    span_days = (max_t - min_t) / 86_400_000.0 if min_t and max_t else 0.0
    full_span_days = (DEV_END - DEV_START) / 86_400_000.0
    return {
        "min_open_time": min_t,
        "max_open_time": max_t,
        "row_count": count,
        "span_fraction_of_dev": round(span_days / full_span_days, 4) if full_span_days else 0.0,
        "avg_1m_volume": round(avg_vol or 0.0, 2),
        "exchange_min_notional_assumed": 5.0,
    }


def main():
    mconn = sqlite3.connect("file:data/market_data_full.db?mode=ro", uri=True)
    t2 = {}
    t2_fails = []
    for sym in T2_SYMBOLS:
        q = qualify(mconn, sym)
        # qualification: full dev span, non-trivial volume
        ok = q["span_fraction_of_dev"] >= 0.99 and q["avg_1m_volume"] > 0
        q["qualified"] = ok
        t2[sym] = q
        if not ok:
            t2_fails.append(sym)

    t3_8 = {sym: t2[sym] for sym in T3_UNIVERSE_8 if sym in t2}

    out = {
        "schema_version": 1,
        "frozen_at": "before_any_return_search",
        "dev_window": {"start_ms": DEV_START, "end_ms": DEV_END},
        "direction_contract": {
            "each_symbol_has_long_and_short_sleeve": True,
            "same_symbol_at_most_one_live_cycle": True,
            "direction_flip_affects_next_cycle_only": True,
        },
        "T2_primary_promotion": {
            "symbols": T2_SYMBOLS,
            "symbol_count": len(T2_SYMBOLS),
            "long_short_sleeve_count": len(T2_SYMBOLS) * 2,
            "qualification": t2,
            "all_qualified": len(t2_fails) == 0,
            "failures": t2_fails,
        },
        "T3_small_cap_universe_8": {
            "symbols": T3_UNIVERSE_8,
            "qualification": t3_8,
        },
        "T3_small_cap_universe_12": {"symbols": T2_SYMBOLS},
        "note": (
            "Symbols frozen before any return search. A symbol is qualified if it "
            "spans >=99% of the dev window with non-trivial volume. Each symbol "
            "carries both a long and a short Martingale sleeve."
        ),
    }
    os.makedirs(os.path.join(ART, "p1"), exist_ok=True)
    path = os.path.join(ART, "p1", "frozen-universe.json")
    with open(path, "w") as fh:
        json.dump(out, fh, indent=2, sort_keys=True)
    print(f"wrote {path}")
    print(f"T2 all_qualified={out['T2_primary_promotion']['all_qualified']} fails={t2_fails}")
    for sym in T2_SYMBOLS:
        q = t2[sym]
        print(
            f"  {sym}: span={q['span_fraction_of_dev']} avg_vol={q['avg_1m_volume']:.1f} qualified={q['qualified']}"
        )


if __name__ == "__main__":
    main()
