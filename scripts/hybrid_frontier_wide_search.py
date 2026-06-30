#!/usr/bin/env python3
"""Research-only wide search for hybrid martingale frontier candidates."""
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


def trend_key(rule: str, symbol: str) -> str:
    return f"{rule}:{symbol}"


def build_rule_stream(market_data: str | Path, symbol: str, rule: str) -> dict:
    if rule == "ema20_50_lf":
        return probe.build_trend_stream(market_data, symbol, 1.0, fast=20, slow=50)
    if rule == "ema50_200_lf":
        return probe.build_trend_stream(market_data, symbol, 1.0, fast=50, slow=200)
    if rule == "mom20_lf":
        return probe.build_momentum_stream(market_data, symbol, 1.0, lookback=20, mode="long_flat")
    if rule == "mom20_ls":
        return probe.build_momentum_stream(market_data, symbol, 1.0, lookback=20, mode="long_short")
    if rule == "mom60_ls":
        return probe.build_momentum_stream(market_data, symbol, 1.0, lookback=60, mode="long_short")
    if rule == "donchian20_lf":
        return probe.build_donchian_stream(market_data, symbol, 1.0, lookback=20, mode="long_flat")
    if rule == "donchian20_ls":
        return probe.build_donchian_stream(market_data, symbol, 1.0, lookback=20, mode="long_short")
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


def summarize_frontier(rows: list[dict]) -> dict:
    summary = {}
    for profile in probe.PROFILE_TARGETS:
        subset = [row for row in rows if row["profile"] == profile]
        seg_cap = [row for row in subset if row.get("seg_pass") and row.get("cap", 999999) < 5000]
        passes = [row for row in subset if row.get("pass")]
        near = [row for row in seg_cap if not row.get("pass")]
        summary[profile] = {
            "rows": len(subset),
            "passes": len(passes),
            "best_ann_seg_cap": max(seg_cap, key=lambda row: row["ann"], default=None),
            "best_dd_seg_cap": min(seg_cap, key=lambda row: row["dd"], default=None),
            "top_near_misses": sorted(near, key=lambda row: row["ann"], reverse=True)[:5],
        }
    return summary


def candidate_to_row(report: dict, meta: dict) -> dict:
    full = report["full_metrics"]
    segment = report["segment_gate"]
    return {
        **meta,
        "ann": full["annualized_return_pct"],
        "dd": full["max_drawdown_pct"],
        "cap": full["max_capital_used_quote"],
        "pass": report["passes_offline"],
        "full_pass": report["full_gate"]["passes"],
        "seg_pass": segment["passes"],
        "pos": segment["positive_segments"],
        "c2426": segment["combined_2024_2026_return_pct"],
        "violations": report["full_gate"]["violations"] + segment["violations"],
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profiles", default="conservative,balanced,aggressive")
    parser.add_argument("--replay-dir", default="docs/superpowers/reports")
    parser.add_argument("--market-data", default="data/market_data_full.db")
    parser.add_argument("--funding-data", default="data/funding_rates.db")
    parser.add_argument("--trend-symbols", default="BTCUSDT,ETHUSDT,BNBUSDT,SOLUSDT,INJUSDT,AAVEUSDT,LINKUSDT,DOGEUSDT,ADAUSDT,XRPUSDT")
    parser.add_argument("--trend-rules", default="ema20_50_lf,mom20_lf,mom20_ls,mom60_ls,donchian20_lf,donchian20_ls")
    parser.add_argument("--funding-symbols", default="BTCUSDT,ETHUSDT,DYDXUSDT,INJUSDT,AAVEUSDT")
    parser.add_argument("--martingale-allocations", default="500,1000,1500,2000")
    parser.add_argument("--trend-allocations", default="0,250,500,750")
    parser.add_argument("--funding-allocations", default="0,100,250,500")
    parser.add_argument("--trend-group-size", type=int, default=3)
    parser.add_argument("--funding-group-size", type=int, default=2)
    parser.add_argument("--budget", type=float, default=5000.0)
    parser.add_argument("--limit", type=int, default=2000)
    parser.add_argument("--out-json", required=True)
    parser.add_argument("--out-md", required=True)
    return parser.parse_args()


def choose_groups(symbols: list[str], size: int) -> list[tuple[str, ...]]:
    if size <= 0 or not symbols:
        return [()]
    groups = list(itertools.combinations(symbols, min(size, len(symbols))))
    return groups[:200]


def run_search(args: argparse.Namespace) -> dict:
    profiles = parse_csv(args.profiles)
    trend_symbols = parse_csv(args.trend_symbols)
    trend_rules = parse_csv(args.trend_rules)
    funding_symbols = parse_csv(args.funding_symbols)
    m_allocs = [float(x) for x in parse_csv(args.martingale_allocations)]
    t_allocs = [float(x) for x in parse_csv(args.trend_allocations)]
    f_allocs = [float(x) for x in parse_csv(args.funding_allocations)]
    trend_groups = [()] + choose_groups(trend_symbols, args.trend_group_size)
    funding_groups = [()] + choose_groups(funding_symbols, args.funding_group_size)

    trend_base = {
        trend_key(rule, symbol): build_rule_stream(args.market_data, symbol, rule)
        for rule in trend_rules
        for symbol in trend_symbols
    }
    funding_base = {
        symbol: probe.build_funding_stream(args.funding_data, symbol, 1.0, probe.SEGMENTS["full"][0], probe.SEGMENTS["full"][1])
        for symbol in funding_symbols
    }

    rows = []
    replay_dir = Path(args.replay_dir)
    for profile in profiles:
        replays = sorted(replay_dir.glob(f"replay_{profile}_*.json"))
        for replay in replays:
            for m_alloc in m_allocs:
                m_stream = probe.load_martingale_stream(replay, m_alloc)
                for t_alloc in t_allocs:
                    valid_trend_groups = [()] if t_alloc == 0 else [group for group in trend_groups if group]
                    valid_trend_rules = ["none"] if t_alloc == 0 else trend_rules
                    for f_alloc in f_allocs:
                        valid_funding_groups = [()] if f_alloc == 0 else [group for group in funding_groups if group]
                        for rule in valid_trend_rules:
                            for trend_group in valid_trend_groups:
                                for funding_group in valid_funding_groups:
                                    if len(rows) >= args.limit:
                                        break
                                    streams = [m_stream]
                                    if t_alloc > 0:
                                        streams += [scale_stream(trend_base[trend_key(rule, symbol)], t_alloc) for symbol in trend_group]
                                    streams += [scale_stream(funding_base[symbol], f_alloc) for symbol in funding_group]
                                    combined = probe.combine_streams(streams, args.budget)
                                    report = probe.build_candidate_report(profile, combined, args.budget)
                                    rows.append(candidate_to_row(report, {
                                        "profile": profile,
                                        "replay": replay.name,
                                        "m_alloc": m_alloc,
                                        "t_alloc": t_alloc,
                                        "f_alloc": f_alloc,
                                        "trend_rule": rule,
                                        "trend_symbols": ",".join(trend_group),
                                        "funding_symbols": ",".join(funding_group),
                                    }))
                                if len(rows) >= args.limit:
                                    break
                            if len(rows) >= args.limit:
                                break
                        if len(rows) >= args.limit:
                            break
                    if len(rows) >= args.limit:
                        break
                if len(rows) >= args.limit:
                    break
    summary = summarize_frontier(rows)
    return {"live_parity_status": probe.LIVE_PARITY_STATUS, "rows": rows, "summary": summary}


def write_outputs(result: dict, out_json: str, out_md: str) -> None:
    Path(out_json).write_text(json.dumps(result, indent=2, sort_keys=True))
    lines = ["# Hybrid Frontier Wide Search", "", f"- live_parity_status: {result['live_parity_status']}", f"- rows: {len(result['rows'])}", ""]
    for profile, summary in result["summary"].items():
        lines.append(f"## {profile}")
        lines.append(f"- passes: {summary['passes']}")
        lines.append(f"- best_ann_seg_cap: {summary['best_ann_seg_cap']}")
        lines.append(f"- best_dd_seg_cap: {summary['best_dd_seg_cap']}")
        lines.append("")
    lines.append("This is Phase 1 research-only evidence and is not live-ready.")
    Path(out_md).write_text("\n".join(lines))


def main() -> int:
    args = parse_args()
    result = run_search(args)
    write_outputs(result, args.out_json, args.out_md)
    print(json.dumps({"rows": len(result["rows"]), "passes": sum(v["passes"] for v in result["summary"].values())}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
