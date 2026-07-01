#!/usr/bin/env python3
"""Research-only multi-pair pair-neutral portfolio probe."""
from __future__ import annotations

import argparse
import importlib.util
import itertools
import json
from collections import Counter
from pathlib import Path


PAIR_PATH = Path(__file__).with_name("pair_neutral_grid_probe.py")
PAIR_SPEC = importlib.util.spec_from_file_location("pair_neutral_grid", PAIR_PATH)
pair_grid = importlib.util.module_from_spec(PAIR_SPEC)
PAIR_SPEC.loader.exec_module(pair_grid)

hybrid = pair_grid.hybrid
LIVE_PARITY_STATUS = "research_only"


def parse_csv(value: str) -> list[str]:
    return [item.strip() for item in value.split(",") if item.strip()]


def parse_ints(value: str) -> list[int]:
    return [int(item) for item in parse_csv(value)]


def parse_floats(value: str) -> list[float]:
    return [float(item) for item in parse_csv(value)]


def symbols_for_stream(stream: dict) -> list[str]:
    symbols = []
    for symbol in stream.get("symbols", []):
        if symbol not in symbols:
            symbols.append(symbol)
    return symbols


def symbol_overlap_ok(streams: list[dict], max_symbol_uses: int) -> bool:
    counts = Counter(symbol for stream in streams for symbol in symbols_for_stream(stream))
    return all(count <= max_symbol_uses for count in counts.values())


def portfolio_key(streams: list[dict]) -> str:
    return "portfolio:" + "|".join(stream["name"] for stream in streams)


def build_portfolio(streams: list[dict], budget: float) -> dict:
    combined = hybrid.combine_streams(streams, budget)
    combined["name"] = portfolio_key(streams)
    combined["live_parity_status"] = LIVE_PARITY_STATUS
    return combined


def gap_score(row: dict) -> float:
    target = hybrid.PROFILE_TARGETS[row["profile"]]
    ann_gap = max(0.0, target["ann_min"] - float(row["ann"])) / max(target["ann_min"], 1.0)
    dd_gap = max(0.0, float(row["dd"]) - target["dd_max"]) / max(target["dd_max"], 1.0)
    cap_gap = max(0.0, float(row["cap"]) - 5000.0) / 5000.0
    seg_gap = max(0.0, 4.0 - float(row.get("pos") or 0.0)) / 4.0
    c2426_gap = 0.0 if row.get("c2426") is None else max(0.0, -float(row["c2426"])) / 100.0
    return ann_gap + dd_gap + cap_gap + seg_gap + c2426_gap


def row_from_report(profile: str, streams: list[dict], report: dict, meta: dict | None = None) -> dict:
    full = report["full_metrics"]
    segment = report["segment_gate"]
    symbols = []
    pairs = []
    for stream in streams:
        stream_symbols = symbols_for_stream(stream)
        pairs.append(",".join(stream_symbols))
        for symbol in stream_symbols:
            if symbol not in symbols:
                symbols.append(symbol)
    row = {
        "profile": profile,
        "pairs": pairs,
        "symbols": symbols,
        "portfolio_size": len(streams),
        "ann": full["annualized_return_pct"],
        "dd": full["max_drawdown_pct"],
        "cap": full.get("max_capital_used_quote", 0.0),
        "pass": report["passes_offline"],
        "full_pass": report["full_gate"]["passes"],
        "seg_pass": segment["passes"],
        "pos": segment["positive_segments"],
        "c2426": segment["combined_2024_2026_return_pct"],
        "violations": report["full_gate"]["violations"] + segment["violations"],
        "streams": [stream["name"] for stream in streams],
        "live_parity_status": LIVE_PARITY_STATUS,
        "meta": meta or {},
    }
    row["gap_score"] = gap_score(row)
    return row


def summarize(rows: list[dict]) -> dict:
    summary = {}
    for profile in hybrid.PROFILE_TARGETS:
        subset = [row for row in rows if row["profile"] == profile]
        passes = [row for row in subset if row["pass"]]
        summary[profile] = {
            "rows": len(subset),
            "passes": len(passes),
            "best_ann": max(subset, key=lambda row: row["ann"], default=None),
            "best_dd": min(subset, key=lambda row: row["dd"], default=None),
            "nearest": sorted(subset, key=lambda row: row["gap_score"])[:5],
            "passes_rows": passes[:20],
        }
    return summary


def build_pair_streams(args: argparse.Namespace) -> list[dict]:
    symbols = parse_csv(args.symbols)
    allocations = parse_floats(args.allocations)
    lookbacks = parse_ints(args.lookbacks)
    entry_zs = parse_floats(args.entry_zs)
    pairs = list(itertools.combinations(symbols, 2))[: args.max_pairs]
    pair_rows = {pair: pair_grid.load_daily_pair_rows(args.market_data, pair[0], pair[1]) for pair in pairs}
    streams = []
    for pair in pairs:
        for allocation in allocations:
            for lookback in lookbacks:
                for entry_z in entry_zs:
                    stream = pair_grid.build_pair_grid_stream_from_rows(
                        pair_rows[pair],
                        symbol_a=pair[0],
                        symbol_b=pair[1],
                        allocation_quote=allocation,
                        lookback=lookback,
                        entry_z=entry_z,
                        exit_z=args.exit_z,
                        fee_bps=args.fee_bps,
                    )
                    metrics = hybrid.compute_metrics(stream["points"])
                    stream["rank_ann"] = metrics["annualized_return_pct"]
                    streams.append(stream)
    streams.sort(key=lambda stream: stream["rank_ann"], reverse=True)
    return streams[: args.max_streams]


def run_search(args: argparse.Namespace) -> dict:
    profiles = parse_csv(args.profiles)
    portfolio_sizes = parse_ints(args.portfolio_sizes)
    streams = build_pair_streams(args)
    rows = []
    seen = set()
    for size in portfolio_sizes:
        for combo in itertools.combinations(streams, size):
            if not symbol_overlap_ok(list(combo), args.max_symbol_uses):
                continue
            key = portfolio_key(list(combo))
            if key in seen:
                continue
            seen.add(key)
            combined = build_portfolio(list(combo), args.budget)
            for profile in profiles:
                report = hybrid.build_candidate_report(profile, combined, args.budget)
                rows.append(row_from_report(profile, list(combo), report, {"portfolio_size": size}))
                if len(rows) >= args.max_portfolios:
                    return {"live_parity_status": LIVE_PARITY_STATUS, "rows": rows, "summary": summarize(rows)}
    return {"live_parity_status": LIVE_PARITY_STATUS, "rows": rows, "summary": summarize(rows)}


def write_outputs(result: dict, out_json: str | Path, out_md: str | Path) -> None:
    Path(out_json).write_text(json.dumps(result, indent=2, sort_keys=True))

    def fmt(row: dict | None) -> str:
        if not row:
            return "`None`"
        return (
            f"pairs `{';'.join(row['pairs'])}` size `{row['portfolio_size']}` "
            f"ann `{row['ann']:.2f}` DD `{row['dd']:.2f}` cap `{row['cap']:.2f}` "
            f"pos `{row['pos']}/5` 2024-2026 `{row['c2426']:.2f}` "
            f"gap `{row['gap_score']:.3f}` pass `{row['pass']}`"
        )

    total_passes = sum(item["passes"] for item in result["summary"].values())
    lines = [
        "# 2026-07-01 Pair-Neutral Portfolio Probe",
        "",
        "This is a research-only multi-pair pair-neutral grid portfolio check. It does not trade, touch Binance, flyingkid, live mode, or real funds.",
        "",
        f"- live_parity_status: `{result['live_parity_status']}`",
        f"- rows: `{len(result['rows'])}`",
        f"- passes: `{total_passes}`",
        "",
    ]
    for profile, summary in result["summary"].items():
        lines.append(f"## {profile}")
        lines.append("")
        lines.append(f"- rows: `{summary['rows']}`")
        lines.append(f"- passes: `{summary['passes']}`")
        lines.append(f"- best_ann: {fmt(summary['best_ann'])}")
        lines.append(f"- best_dd: {fmt(summary['best_dd'])}")
        lines.append("- nearest:")
        for row in summary["nearest"]:
            lines.append(f"  - {fmt(row)}")
        lines.append("")
    lines.extend(["## Conclusion", ""])
    if total_passes:
        lines.append(f"Potential research-only multi-pair passes found: `{total_passes}`. These require a separate live-parity promotion design before any trading conclusion.")
    else:
        lines.append("Potential research-only multi-pair passes found: `0`. Multi-pair diversification did not close the original return/DD/segment gates in this bounded scan.")
    Path(out_md).write_text("\n".join(lines) + "\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profiles", default="conservative,balanced,aggressive")
    parser.add_argument("--market-data", default="data/market_data_full.db")
    parser.add_argument("--symbols", default="BTCUSDT,ETHUSDT,BNBUSDT,SOLUSDT,XRPUSDT,ADAUSDT,DOGEUSDT,LINKUSDT")
    parser.add_argument("--allocations", default="500,1000,1500")
    parser.add_argument("--lookbacks", default="20,40,80")
    parser.add_argument("--entry-zs", default="1.0,1.5,2.0")
    parser.add_argument("--portfolio-sizes", default="2,3,4")
    parser.add_argument("--exit-z", type=float, default=0.25)
    parser.add_argument("--fee-bps", type=float, default=4.0)
    parser.add_argument("--max-pairs", type=int, default=28)
    parser.add_argument("--max-streams", type=int, default=24)
    parser.add_argument("--max-symbol-uses", type=int, default=1)
    parser.add_argument("--max-portfolios", type=int, default=5000)
    parser.add_argument("--budget", type=float, default=5000.0)
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
