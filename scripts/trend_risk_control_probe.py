#!/usr/bin/env python3
"""Research-only portfolio DD stop/cooldown probe for trend sleeves."""
from __future__ import annotations

import argparse
import importlib.util
import itertools
import json
from pathlib import Path


TREND_PATH = Path(__file__).with_name("trend_sleeve_frontier_probe.py")
TREND_SPEC = importlib.util.spec_from_file_location("trend_frontier", TREND_PATH)
trend = importlib.util.module_from_spec(TREND_SPEC)
TREND_SPEC.loader.exec_module(trend)
probe = trend.probe

MS_PER_DAY = 86_400_000


def parse_csv(value: str) -> list[str]:
    return [item.strip() for item in value.split(",") if item.strip()]


def risk_key(dd_stop_pct: float, cooldown_days: int) -> str:
    return f"dd{dd_stop_pct:g}_cd{cooldown_days}"


def apply_dd_stop_cooldown(points: list[dict], dd_stop_pct: float, cooldown_days: int) -> list[dict]:
    ordered = sorted(points, key=lambda point: int(point["timestamp_ms"]))
    if not ordered:
        return []
    peak = float(ordered[0]["equity_quote"])
    frozen_until = -1
    frozen_equity = None
    risk_events = 0
    output = []
    for point in ordered:
        ts = int(point["timestamp_ms"])
        raw_equity = float(point["equity_quote"])
        if frozen_equity is not None and ts < frozen_until:
            equity = frozen_equity
        else:
            frozen_equity = None
            equity = raw_equity
            peak = max(peak, equity)
            drawdown = (peak - equity) / peak * 100.0 if peak > 0 else 0.0
            if drawdown >= dd_stop_pct:
                frozen_equity = equity
                frozen_until = ts + cooldown_days * MS_PER_DAY
                risk_events += 1
        output.append({"timestamp_ms": ts, "equity_quote": equity, "risk_events": risk_events})
    return output


def controlled_report(profile: str, combined: dict, budget: float, dd_stop_pct: float, cooldown_days: int) -> dict:
    controlled_points = apply_dd_stop_cooldown(combined["points"], dd_stop_pct, cooldown_days)
    controlled = dict(combined)
    controlled["points"] = controlled_points
    controlled["metrics"] = probe.compute_metrics(controlled_points)
    controlled["metrics"].update(
        {
            "max_capital_used_quote": combined["metrics"]["max_capital_used_quote"],
            "budget_blocked_events": combined["metrics"]["budget_blocked_events"],
            "symbol_count": combined["metrics"]["symbol_count"],
            "risk_events": controlled_points[-1]["risk_events"] if controlled_points else 0,
        }
    )
    report = probe.build_candidate_report(profile, controlled, budget)
    report["risk_control"] = {
        "dd_stop_pct": dd_stop_pct,
        "cooldown_days": cooldown_days,
        "risk_events": controlled["metrics"]["risk_events"],
    }
    return report


def row_from_report(profile: str, rule: str, symbols: tuple[str, ...], allocation: float, dd_stop_pct: float, cooldown_days: int, report: dict) -> dict:
    base = trend.candidate_to_row(profile, rule, symbols, allocation, report)
    base["risk"] = risk_key(dd_stop_pct, cooldown_days)
    base["risk_events"] = report["risk_control"]["risk_events"]
    return base


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
        }
    return summary


def run_search(args: argparse.Namespace) -> dict:
    profiles = parse_csv(args.profiles)
    symbols = parse_csv(args.symbols)
    rules = parse_csv(args.rules)
    allocations = [float(value) for value in parse_csv(args.allocations)]
    dd_stops = [float(value) for value in parse_csv(args.dd_stops)]
    cooldowns = [int(value) for value in parse_csv(args.cooldowns)]
    groups = [
        group
        for size in range(args.min_group_size, args.max_group_size + 1)
        for group in itertools.combinations(symbols, size)
        if trend.group_is_allowed(group)
    ][: args.max_groups]
    base_streams = {
        (rule, symbol): trend.build_rule_stream(args.market_data, symbol, rule, 1.0)
        for rule in rules
        for symbol in symbols
    }

    rows = []
    for profile in profiles:
        for rule in rules:
            for group in groups:
                for allocation in allocations:
                    allocations_by_symbol = trend.even_weight_allocations(group, allocation)
                    streams = [
                        trend.scale_stream(base_streams[(rule, symbol)], symbol_allocation)
                        for symbol, symbol_allocation in allocations_by_symbol.items()
                    ]
                    combined = probe.combine_streams(streams, args.budget)
                    for dd_stop in dd_stops:
                        for cooldown in cooldowns:
                            report = controlled_report(profile, combined, args.budget, dd_stop, cooldown)
                            rows.append(row_from_report(profile, rule, group, allocation, dd_stop, cooldown, report))
                            if len(rows) >= args.limit:
                                return {"live_parity_status": probe.LIVE_PARITY_STATUS, "rows": rows, "summary": summarize(rows)}
    return {"live_parity_status": probe.LIVE_PARITY_STATUS, "rows": rows, "summary": summarize(rows)}


def write_outputs(result: dict, out_json: str | Path, out_md: str | Path) -> None:
    Path(out_json).write_text(json.dumps(result, indent=2, sort_keys=True))

    def fmt(row: dict | None) -> str:
        if not row:
            return "`None`"
        return (
            f"`{row['risk']}` `{row['rule']}` `{row['symbols']}` ann `{row['ann']:.2f}` "
            f"DD `{row['dd']:.2f}` cap `{row['cap']:.2f}` pos `{row['pos']}/5` "
            f"2024-2026 `{row['c2426']:.2f}` events `{row['risk_events']}` pass `{row['pass']}`"
        )

    lines = [
        "# 2026-07-01 Trend Risk Control Probe",
        "",
        "This is a research-only portfolio DD stop/cooldown check. It does not trade, touch Binance, flyingkid, live mode, or real funds.",
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
        lines.append(f"Potential research-only risk-controlled trend passes found: `{total_passes}`. These require manual replay and live-parity design before any trading conclusion.")
    else:
        lines.append("Potential research-only risk-controlled trend passes found: `0` under this scan. Portfolio DD stop/cooldown lowers drawdown in some cases but does not preserve enough annualized return to meet the original gates.")
    Path(out_md).write_text("\n".join(lines) + "\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profiles", default="conservative,balanced,aggressive")
    parser.add_argument("--market-data", default="data/market_data_full.db")
    parser.add_argument("--symbols", default="BTCUSDT,ETHUSDT,BNBUSDT,SOLUSDT,INJUSDT")
    parser.add_argument("--rules", default="mom20_ls,mom60_ls,donchian20_ls,donchian60_ls")
    parser.add_argument("--allocations", default="1000,2000,3000,4000")
    parser.add_argument("--dd-stops", default="10,20,30,40")
    parser.add_argument("--cooldowns", default="15,30,60")
    parser.add_argument("--min-group-size", type=int, default=2)
    parser.add_argument("--max-group-size", type=int, default=4)
    parser.add_argument("--max-groups", type=int, default=25)
    parser.add_argument("--budget", type=float, default=5000.0)
    parser.add_argument("--limit", type=int, default=20000)
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
