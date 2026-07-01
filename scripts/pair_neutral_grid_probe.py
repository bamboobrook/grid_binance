#!/usr/bin/env python3
"""Research-only pair-neutral grid probe for the original martingale gates."""
from __future__ import annotations

import argparse
import importlib.util
import itertools
import json
import math
import sqlite3
from pathlib import Path


PROBE_PATH = Path(__file__).with_name("hybrid_martingale_frontier_probe.py")
SPEC = importlib.util.spec_from_file_location("hybrid_probe", PROBE_PATH)
hybrid = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(hybrid)

LIVE_PARITY_STATUS = "research_only"


def parse_csv(value: str) -> list[str]:
    return [item.strip() for item in value.split(",") if item.strip()]


def rolling_zscores(values: list[float], lookback: int) -> list[float | None]:
    scores: list[float | None] = []
    for index, value in enumerate(values):
        if index < lookback:
            scores.append(None)
            continue
        window = values[index - lookback : index]
        mean = sum(window) / len(window)
        variance = sum((item - mean) ** 2 for item in window) / len(window)
        stdev = math.sqrt(variance)
        scores.append(None if stdev <= 0 else (value - mean) / stdev)
    return scores


def load_daily_pair_rows(market_db: str | Path, symbol_a: str, symbol_b: str) -> list[dict]:
    con = sqlite3.connect(str(market_db))
    try:
        rows = con.execute(
            """
            SELECT symbol, open_time, close
            FROM klines
            WHERE symbol IN (?, ?)
              AND market_type = 'futures_usdt_perp'
              AND timeframe = '1m'
            ORDER BY open_time
            """,
            (symbol_a, symbol_b),
        ).fetchall()
    finally:
        con.close()
    by_day: dict[int, dict] = {}
    for symbol, ts, close in rows:
        day = int(ts) // hybrid.MS_PER_DAY
        record = by_day.setdefault(day, {"timestamp_ms": int(ts)})
        key = "a" if symbol == symbol_a else "b"
        record[key] = float(close)
        record["timestamp_ms"] = int(ts)
    return [row for _day, row in sorted(by_day.items()) if "a" in row and "b" in row]


def build_pair_grid_stream_from_rows(
    rows: list[dict],
    symbol_a: str,
    symbol_b: str,
    allocation_quote: float,
    lookback: int,
    entry_z: float,
    exit_z: float,
    fee_bps: float,
) -> dict:
    spreads = [math.log(row["a"]) - math.log(row["b"]) for row in rows]
    zscores = rolling_zscores(spreads, lookback)
    equity = float(allocation_quote)
    points = []
    position = 0
    trades = 0
    for index in range(1, len(rows)):
        z_prev = zscores[index - 1]
        desired = position
        if z_prev is not None:
            if position == 0:
                if z_prev >= entry_z:
                    desired = -1
                elif z_prev <= -entry_z:
                    desired = 1
            elif abs(z_prev) <= exit_z:
                desired = 0
        if desired != position:
            equity *= 1.0 - fee_bps / 10_000.0
            position = desired
            trades += 1
        prev = rows[index - 1]
        current = rows[index]
        ret_a = current["a"] / prev["a"] - 1.0 if prev["a"] > 0 else 0.0
        ret_b = current["b"] / prev["b"] - 1.0 if prev["b"] > 0 else 0.0
        if position == 1:
            equity *= 1.0 + 0.5 * ret_a - 0.5 * ret_b
        elif position == -1:
            equity *= 1.0 - 0.5 * ret_a + 0.5 * ret_b
        points.append({"timestamp_ms": int(current["timestamp_ms"]), "equity_quote": equity})
    return {
        "name": f"pair_grid:{symbol_a}:{symbol_b}:lb{lookback}:z{entry_z}",
        "kind": "pair_neutral_grid",
        "symbols": [symbol_a, symbol_b],
        "points": points,
        "max_capital_used_quote": float(allocation_quote),
        "budget_blocked_events": 0,
        "trades": trades,
        "live_parity_status": LIVE_PARITY_STATUS,
    }


def build_pair_grid_stream(
    market_db: str | Path,
    symbol_a: str,
    symbol_b: str,
    allocation_quote: float,
    lookback: int,
    entry_z: float,
    exit_z: float,
    fee_bps: float,
) -> dict:
    rows = load_daily_pair_rows(market_db, symbol_a, symbol_b)
    return build_pair_grid_stream_from_rows(
        rows,
        symbol_a=symbol_a,
        symbol_b=symbol_b,
        allocation_quote=allocation_quote,
        lookback=lookback,
        entry_z=entry_z,
        exit_z=exit_z,
        fee_bps=fee_bps,
    )


def candidate_to_row(
    profile: str,
    pair: tuple[str, str],
    allocation: float,
    lookback: int,
    entry_z: float,
    report: dict,
) -> dict:
    full = report["full_metrics"]
    segment = report["segment_gate"]
    return {
        "profile": profile,
        "pair": ",".join(pair),
        "symbol_count": len(set(pair)),
        "allocation_quote": allocation,
        "lookback": lookback,
        "entry_z": entry_z,
        "ann": full["annualized_return_pct"],
        "dd": full["max_drawdown_pct"],
        "cap": full.get("max_capital_used_quote"),
        "pass": report["passes_offline"],
        "full_pass": report["full_gate"]["passes"],
        "seg_pass": segment["passes"],
        "pos": segment["positive_segments"],
        "c2426": segment["combined_2024_2026_return_pct"],
        "violations": report["full_gate"]["violations"] + segment["violations"],
        "streams": report["streams"],
    }


def summarize(rows: list[dict]) -> dict:
    summary = {}
    for profile in hybrid.PROFILE_TARGETS:
        subset = [row for row in rows if row["profile"] == profile]
        passes = [row for row in subset if row["pass"]]
        seg_cap = [row for row in subset if row["seg_pass"] and row["cap"] < 5000.0]
        summary[profile] = {
            "rows": len(subset),
            "passes": len(passes),
            "best_ann": max(subset, key=lambda row: row["ann"], default=None),
            "best_ann_seg_cap": max(seg_cap, key=lambda row: row["ann"], default=None),
            "best_dd_seg_cap": min(seg_cap, key=lambda row: row["dd"], default=None),
            "passes_rows": passes[:20],
        }
    return summary


def run_search(args: argparse.Namespace) -> dict:
    profiles = parse_csv(args.profiles)
    symbols = parse_csv(args.symbols)
    allocations = [float(value) for value in parse_csv(args.allocations)]
    lookbacks = [int(value) for value in parse_csv(args.lookbacks)]
    entry_zs = [float(value) for value in parse_csv(args.entry_zs)]
    pairs = list(itertools.combinations(symbols, 2))[: args.max_pairs]
    pair_rows = {pair: load_daily_pair_rows(args.market_data, pair[0], pair[1]) for pair in pairs}
    rows = []
    for profile in profiles:
        for pair in pairs:
            for allocation in allocations:
                for lookback in lookbacks:
                    for entry_z in entry_zs:
                        stream = build_pair_grid_stream_from_rows(
                            pair_rows[pair],
                            symbol_a=pair[0],
                            symbol_b=pair[1],
                            allocation_quote=allocation,
                            lookback=lookback,
                            entry_z=entry_z,
                            exit_z=args.exit_z,
                            fee_bps=args.fee_bps,
                        )
                        combined = hybrid.combine_streams([stream], args.budget)
                        report = hybrid.build_candidate_report(profile, combined, args.budget)
                        rows.append(candidate_to_row(profile, pair, allocation, lookback, entry_z, report))
                        if len(rows) >= args.limit:
                            return {
                                "live_parity_status": LIVE_PARITY_STATUS,
                                "rows": rows,
                                "summary": summarize(rows),
                            }
    return {"live_parity_status": LIVE_PARITY_STATUS, "rows": rows, "summary": summarize(rows)}


def write_outputs(result: dict, out_json: str | Path, out_md: str | Path) -> None:
    Path(out_json).write_text(json.dumps(result, indent=2, sort_keys=True))

    def fmt(row: dict | None) -> str:
        if not row:
            return "`None`"
        return (
            f"`{row['pair']}` lb `{row['lookback']}` z `{row['entry_z']}` "
            f"ann `{row['ann']:.2f}` DD `{row['dd']:.2f}` cap `{row['cap']:.2f}` "
            f"pos `{row['pos']}/5` 2024-2026 `{row['c2426']:.2f}` pass `{row['pass']}`"
        )

    lines = [
        "# 2026-07-01 Pair-Neutral Grid Probe",
        "",
        "This is a research-only pair-neutral grid check. It does not trade, touch Binance, flyingkid, live mode, or real funds.",
        "",
        f"- live_parity_status: `{result['live_parity_status']}`",
        f"- rows: `{len(result['rows'])}`",
        "",
    ]
    for profile, item in result["summary"].items():
        lines.append(f"## {profile}")
        lines.append("")
        lines.append(f"- passes: `{item['passes']}`")
        lines.append(f"- best_ann: {fmt(item['best_ann'])}")
        lines.append(f"- best_ann_seg_cap: {fmt(item['best_ann_seg_cap'])}")
        lines.append(f"- best_dd_seg_cap: {fmt(item['best_dd_seg_cap'])}")
        lines.append("")
    lines.extend(["## Conclusion", ""])
    total_passes = sum(item["passes"] for item in result["summary"].values())
    if total_passes:
        lines.append(f"Potential research-only pair-neutral grid passes found: `{total_passes}`. These require full replay and live-parity design before any trading conclusion.")
    else:
        lines.append("Potential research-only pair-neutral grid passes found: `0` under this scan. Pair-neutral spread grids reduce directional beta, but the observed frontier still fails the original return/DD/segment gates.")
    Path(out_md).write_text("\n".join(lines) + "\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profiles", default="conservative,balanced,aggressive")
    parser.add_argument("--market-data", default="data/market_data_full.db")
    parser.add_argument("--symbols", default="BTCUSDT,ETHUSDT,BNBUSDT,SOLUSDT,XRPUSDT,ADAUSDT,DOGEUSDT,LINKUSDT")
    parser.add_argument("--allocations", default="1000,2000,3000,4000")
    parser.add_argument("--lookbacks", default="20,40,80")
    parser.add_argument("--entry-zs", default="1.0,1.5,2.0")
    parser.add_argument("--exit-z", type=float, default=0.25)
    parser.add_argument("--fee-bps", type=float, default=4.0)
    parser.add_argument("--max-pairs", type=int, default=28)
    parser.add_argument("--budget", type=float, default=5000.0)
    parser.add_argument("--limit", type=int, default=5000)
    parser.add_argument("--out-json", required=True)
    parser.add_argument("--out-md", required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    result = run_search(args)
    write_outputs(result, args.out_json, args.out_md)
    print(json.dumps({"rows": len(result["rows"]), "passes": sum(v["passes"] for v in result["summary"].values())}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
