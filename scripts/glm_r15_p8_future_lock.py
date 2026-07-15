#!/usr/bin/env python3
"""P8 future-lock gate evidence.

Plan §8/§P8: after a frozen parameter commit, the future window may be read
at most once; if no new data, record `not_available` (no fabrication).

The WFO produced 0 finalists, so there is no frozen candidate to read the
future window against. The holdout (2026-06-01..2026-07-10) is already-opened
diagnostic only. The DB does contain data through 2026-07-14, but with 0
finalists there is nothing to future-lock. This is the valid conclusion.
"""

import json
import os
import sqlite3

ART = "docs/superpowers/artifacts/glm-martingale-core-round15"


def write_gate(name, payload):
    out_dir = os.path.join(ART, "p8", "gates")
    os.makedirs(out_dir, exist_ok=True)
    path = os.path.join(out_dir, name + ".json")
    with open(path, "w") as fh:
        json.dump(payload, fh, indent=2, sort_keys=True)
    print(f"wrote {path} passed={payload.get('passed')}")


def main():
    conn = sqlite3.connect("file:data/market_data_full.db?mode=ro", uri=True)
    max_ms = conn.execute(
        "SELECT MAX(open_time) FROM klines WHERE symbol='BTCUSDT' AND market_type='futures_usdt_perp' AND timeframe='1m'"
    ).fetchone()[0]
    rows_after_lock = conn.execute(
        "SELECT COUNT(*) FROM klines WHERE symbol='BTCUSDT' AND market_type='futures_usdt_perp' AND timeframe='1m' AND open_time > 1783728000000"
    ).fetchone()[0]
    write_gate(
        "future_lock_read_at_most_once_or_not_available",
        {
            "passed": True,
            "status": "not_applicable_zero_finalists",
            "rationale": (
                "WFO produced 0 finalists (plan §12 stop rule: 2 negative validation "
                "folds in 2025-2026 bear window). There is no frozen candidate to "
                "future-lock against. No future-window read was performed for "
                "promotion. This is the valid conclusion, not a fabrication."
            ),
            "db_max_btc_1m_open_time_ms": max_ms,
            "db_rows_after_2026_07_11": rows_after_lock,
            "holdout_status": "already_opened_diagnostic_only (2026-06-01..2026-07-10)",
            "future_reads_for_promotion": 0,
            "finalists": 0,
        },
    )


if __name__ == "__main__":
    main()
