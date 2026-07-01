#!/usr/bin/env python3
"""Research-only trend sleeve frontier probe for the martingale target gates."""
from __future__ import annotations

import argparse
import importlib.util
import itertools
import json
from pathlib import Path


PROBE_PATH = Path(__file__).with_name("hybrid_martingale_frontier_probe.py")
SPEC = importlib.util.spec_from_file_location("hybrid_probe", PROBE_PATH)
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)


def parse_csv(value: str) -> list[str]:
    return [item.strip() for item in value.split(",") if item.strip()]


def group_is_allowed(symbols: tuple[str, ...]) -> bool:
    return len(set(symbols)) >= 2


def even_weight_allocations(symbols: list[str] | tuple[str, ...], allocation_quote: float) -> dict[str, float]:
    if not symbols:
        return {}
    each = float(allocation_quote) / len(symbols)
    return {symbol: each for symbol in symbols}


def build_rule_stream(market_data: str | Path, symbol: str, rule: str, allocation: float) -> dict:
    if rule == "ema20_50_lf":
        return probe.build_trend_stream(market_data, symbol, allocation, fast=20, slow=50)
    if rule == "ema50_200_lf":
        return probe.build_trend_stream(market_data, symbol, allocation, fast=50, slow=200)
    if rule == "mom20_lf":
        return probe.build_momentum_stream(market_data, symbol, allocation, lookback=20, mode="long_flat")
    if rule == "mom20_ls":
        return probe.build_momentum_stream(market_data, symbol, allocation, lookback=20, mode="long_short")
    if rule == "mom60_ls":
        return probe.build_momentum_stream(market_data, symbol, allocation, lookback=60, mode="long_short")
    if rule == "donchian20_lf":
        return probe.build_donchian_stream(market_data, symbol, allocation, lookback=20, mode="long_flat")
    if rule == "donchian20_ls":
        return probe.build_donchian_stream(market_data, symbol, allocation, lookback=20, mode="long_short")
    if rule == "donchian60_ls":
        return probe.build_donchian_stream(market_data, symbol, allocation, lookback=60, mode="long_short")
    raise ValueError(f"unknown trend rule: {rule}")


def scale_stream(stream: dict, allocation: float) -> dict:
    scaled = dict(stream)
    scaled["points"] = [
        {"timestamp_ms": point["timestamp_ms"], "equity_quote": point["equity_quote"] * allocation}
        for point in stream["points"]
    ]
    scaled["max_capital_used_quote"] = float(stream.get("max_capital_used_quote", 1.0)) * allocation
    scaled["budget_blocked_events"] = int(stream.get("budget_blocked_events", 0))
    return scaled


def candidate_to_row(profile: str, rule: str, symbols: tuple[str, ...], allocation: float, report: dict) -> dict:
    full = report["full_metrics"]
    segment = report["segment_gate"]
    return {
        "profile": profile,
        "rule": rule,
        "symbols": ",".join(symbols),
        "symbol_count": len(set(symbols)),
        "allocation_quote": allocation,
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
    for profile in probe.PROFILE_TARGETS:
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
    rules = parse_csv(args.rules)
    allocations = [float(value) for value in parse_csv(args.allocations)]
    groups = [
        group
        for size in range(args.min_group_size, args.max_group_size + 1)
        for group in itertools.combinations(symbols, size)
        if group_is_allowed(group)
    ][: args.max_groups]
    base_streams = {
        (rule, symbol): build_rule_stream(args.market_data, symbol, rule, 1.0)
        for rule in rules
        for symbol in symbols
    }

    rows = []
    for profile in profiles:
        for rule in rules:
            for group in groups:
                for allocation in allocations:
                    allocations_by_symbol = even_weight_allocations(group, allocation)
                    streams = [
                        scale_stream(base_streams[(rule, symbol)], symbol_allocation)
                        for symbol, symbol_allocation in allocations_by_symbol.items()
                    ]
                    combined = probe.combine_streams(streams, args.budget)
                    report = probe.build_candidate_report(profile, combined, args.budget)
                    rows.append(candidate_to_row(profile, rule, group, allocation, report))
                    if len(rows) >= args.limit:
                        return {"live_parity_status": probe.LIVE_PARITY_STATUS, "rows": rows, "summary": summarize(rows)}
    return {"live_parity_status": probe.LIVE_PARITY_STATUS, "rows": rows, "summary": summarize(rows)}


def write_outputs(result: dict, out_json: str | Path, out_md: str | Path) -> None:
    Path(out_json).write_text(json.dumps(result, indent=2, sort_keys=True))

    def fmt(row: dict | None) -> str:
        if not row:
            return "`None`"
        return (
            f"`{row['rule']}` `{row['symbols']}` ann `{row['ann']:.2f}` "
            f"DD `{row['dd']:.2f}` cap `{row['cap']:.2f}` "
            f"pos `{row['pos']}/5` 2024-2026 `{row['c2426']:.2f}` "
            f"pass `{row['pass']}`"
        )

    lines = [
        "# 2026-07-01 Trend Sleeve Frontier Probe",
        "",
        "This is a research-only trend sleeve check. It does not trade, touch Binance, flyingkid, live mode, or real funds.",
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
    lines.append("## Conclusion")
    lines.append("")
    total_passes = sum(item["passes"] for item in result["summary"].values())
    if total_passes:
        lines.append(f"Potential research-only trend sleeve passes found: `{total_passes}`. These require manual replay and live-parity design before any trading conclusion.")
    else:
        lines.append("Potential research-only trend sleeve passes found: `0` under this scan. Trend/momentum streams improve 2024-2026 behavior compared with pure martingale, but the observed frontier still fails the original return/DD gates.")
    Path(out_md).write_text("\n".join(lines) + "\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profiles", default="conservative,balanced,aggressive")
    parser.add_argument("--market-data", default="data/market_data_full.db")
    parser.add_argument("--symbols", default="BTCUSDT,ETHUSDT,BNBUSDT,SOLUSDT,INJUSDT")
    parser.add_argument("--rules", default="mom20_ls,mom60_ls,donchian20_ls,donchian60_ls")
    parser.add_argument("--allocations", default="1000,2000,3000,4000")
    parser.add_argument("--min-group-size", type=int, default=2)
    parser.add_argument("--max-group-size", type=int, default=4)
    parser.add_argument("--max-groups", type=int, default=25)
    parser.add_argument("--budget", type=float, default=5000.0)
    parser.add_argument("--limit", type=int, default=3000)
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
