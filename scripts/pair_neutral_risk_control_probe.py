#!/usr/bin/env python3
"""Research-only DD stop/cooldown probe for pair-neutral grid streams."""
from __future__ import annotations

import argparse
import importlib.util
import itertools
import json
from pathlib import Path


PAIR_PATH = Path(__file__).with_name("pair_neutral_grid_probe.py")
PAIR_SPEC = importlib.util.spec_from_file_location("pair_neutral_grid", PAIR_PATH)
pair_grid = importlib.util.module_from_spec(PAIR_SPEC)
PAIR_SPEC.loader.exec_module(pair_grid)

TREND_RISK_PATH = Path(__file__).with_name("trend_risk_control_probe.py")
TREND_RISK_SPEC = importlib.util.spec_from_file_location("trend_risk", TREND_RISK_PATH)
trend_risk = importlib.util.module_from_spec(TREND_RISK_SPEC)
TREND_RISK_SPEC.loader.exec_module(trend_risk)

hybrid = pair_grid.hybrid
LIVE_PARITY_STATUS = "research_only"


def parse_csv(value: str) -> list[str]:
    return [item.strip() for item in value.split(",") if item.strip()]


def risk_key(dd_stop_pct: float, cooldown_days: int) -> str:
    return trend_risk.risk_key(dd_stop_pct, cooldown_days)


def controlled_report(profile: str, combined: dict, budget: float, dd_stop_pct: float, cooldown_days: int) -> dict:
    controlled_points = trend_risk.apply_dd_stop_cooldown(combined["points"], dd_stop_pct, cooldown_days)
    controlled = dict(combined)
    controlled["points"] = controlled_points
    controlled["metrics"] = hybrid.compute_metrics(controlled_points)
    controlled["metrics"].update(
        {
            "max_capital_used_quote": combined["metrics"]["max_capital_used_quote"],
            "budget_blocked_events": combined["metrics"]["budget_blocked_events"],
            "symbol_count": combined["metrics"]["symbol_count"],
            "risk_events": controlled_points[-1]["risk_events"] if controlled_points else 0,
        }
    )
    report = hybrid.build_candidate_report(profile, controlled, budget)
    report["risk_control"] = {
        "dd_stop_pct": dd_stop_pct,
        "cooldown_days": cooldown_days,
        "risk_events": controlled["metrics"]["risk_events"],
    }
    return report


def row_from_report(
    profile: str,
    pair: tuple[str, str],
    allocation: float,
    lookback: int,
    entry_z: float,
    dd_stop_pct: float,
    cooldown_days: int,
    report: dict,
) -> dict:
    base = pair_grid.candidate_to_row(profile, pair, allocation, lookback, entry_z, report)
    base["risk"] = risk_key(dd_stop_pct, cooldown_days)
    base["risk_events"] = report["risk_control"]["risk_events"]
    return base


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
    dd_stops = [float(value) for value in parse_csv(args.dd_stops)]
    cooldowns = [int(value) for value in parse_csv(args.cooldowns)]
    pairs = list(itertools.combinations(symbols, 2))[: args.max_pairs]
    pair_rows = {pair: pair_grid.load_daily_pair_rows(args.market_data, pair[0], pair[1]) for pair in pairs}

    rows = []
    for profile in profiles:
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
                        combined = hybrid.combine_streams([stream], args.budget)
                        for dd_stop in dd_stops:
                            for cooldown in cooldowns:
                                report = controlled_report(profile, combined, args.budget, dd_stop, cooldown)
                                rows.append(
                                    row_from_report(
                                        profile, pair, allocation, lookback, entry_z, dd_stop, cooldown, report
                                    )
                                )
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
            f"`{row['risk']}` `{row['pair']}` lb `{row['lookback']}` z `{row['entry_z']}` "
            f"ann `{row['ann']:.2f}` DD `{row['dd']:.2f}` cap `{row['cap']:.2f}` "
            f"pos `{row['pos']}/5` 2024-2026 `{row['c2426']:.2f}` "
            f"events `{row['risk_events']}` pass `{row['pass']}`"
        )

    lines = [
        "# 2026-07-01 Pair-Neutral Risk Control Probe",
        "",
        "This is a research-only DD stop/cooldown check for pair-neutral grid streams. It does not trade, touch Binance, flyingkid, live mode, or real funds.",
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
        lines.append(f"Potential research-only pair-neutral risk-control passes found: `{total_passes}`. These require full replay and live-parity design before any trading conclusion.")
    else:
        lines.append("Potential research-only pair-neutral risk-control passes found: `0` under this scan. DD stop/cooldown does not preserve enough return while satisfying the original drawdown and segment gates.")
    Path(out_md).write_text("\n".join(lines) + "\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profiles", default="conservative,balanced,aggressive")
    parser.add_argument("--market-data", default="data/market_data_full.db")
    parser.add_argument("--symbols", default="BTCUSDT,ETHUSDT,BNBUSDT,SOLUSDT,XRPUSDT,ADAUSDT,DOGEUSDT,LINKUSDT")
    parser.add_argument("--allocations", default="1000,2000,3000,4000")
    parser.add_argument("--lookbacks", default="20,40,80")
    parser.add_argument("--entry-zs", default="1.0,1.5,2.0")
    parser.add_argument("--dd-stops", default="10,20,30")
    parser.add_argument("--cooldowns", default="15,30,60")
    parser.add_argument("--exit-z", type=float, default=0.25)
    parser.add_argument("--fee-bps", type=float, default=4.0)
    parser.add_argument("--max-pairs", type=int, default=28)
    parser.add_argument("--budget", type=float, default=5000.0)
    parser.add_argument("--limit", type=int, default=30000)
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
