#!/usr/bin/env python3
"""Quantify how far saved research candidates are from the martingale targets."""
from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Any


PROFILE_TARGETS = {
    "conservative": {"ann": 50.0, "dd": 10.0},
    "balanced": {"ann": 90.0, "dd": 20.0},
    "aggressive": {"ann": 110.0, "dd": 30.0},
}

DEFAULT_SOURCES = [
    ("trend_sleeve", "/tmp/trend_sleeve_frontier_probe.json"),
    ("trend_risk_control", "/tmp/trend_risk_control_probe.json"),
    ("pair_neutral_grid", "/tmp/pair_neutral_grid_probe.json"),
    ("funding_sleeve", "/tmp/funding_sleeve_probe.json"),
    ("saved_result_leak_audit", "/tmp/martingale_result_leak_audit_wide.json"),
]


def as_float(value: Any) -> float | None:
    if value is None or isinstance(value, bool):
        return None
    try:
        result = float(value)
    except (TypeError, ValueError):
        return None
    if math.isnan(result) or math.isinf(result):
        return None
    return result


def as_int(value: Any) -> int | None:
    number = as_float(value)
    return None if number is None else int(number)


def first_value(data: dict, names: list[str]) -> Any:
    for name in names:
        if name in data and data[name] is not None:
            return data[name]
    return None


def label_for(source: str, data: dict) -> str:
    if source == "funding_sleeve":
        return f"funding {data.get('symbol', 'unknown')} {data.get('side', 'unknown')}"
    parts = []
    for key in ("rule", "risk", "symbols", "pair", "symbol", "side"):
        value = data.get(key)
        if value:
            parts.append(str(value))
    if parts:
        return " ".join(parts)
    if data.get("source"):
        return Path(str(data["source"])).name
    return source


def normalize_candidate(source: str, data: dict, profile: str | None = None) -> dict | None:
    candidate_profile = str(profile or data.get("profile") or "").lower()
    if candidate_profile not in PROFILE_TARGETS:
        return None
    capital = as_float(first_value(data, ["capital_quote", "cap", "max_capital_used_quote", "max_input_quote"]))
    if capital is None and source == "funding_sleeve":
        capital = 1000.0
    return {
        "source": source,
        "profile": candidate_profile,
        "annualized_return_pct": as_float(first_value(data, ["annualized_return_pct", "ann"])),
        "max_drawdown_pct": as_float(first_value(data, ["max_drawdown_pct", "dd"])),
        "capital_quote": capital,
        "positive_segments": as_int(first_value(data, ["positive_segments", "pos"])),
        "combined_2024_2026_return_pct": as_float(
            first_value(data, ["combined_2024_2026_return_pct", "c2426"])
        ),
        "label": label_for(source, data),
        "raw_source": str(data.get("source") or ""),
    }


def score_candidate(row: dict) -> dict:
    target = PROFILE_TARGETS[row["profile"]]
    ann = row["annualized_return_pct"]
    dd = row["max_drawdown_pct"]
    capital = row["capital_quote"]
    positive_segments = row["positive_segments"]
    combined_2024_2026 = row["combined_2024_2026_return_pct"]
    ann_gap = target["ann"] if ann is None else max(0.0, target["ann"] - ann)
    dd_excess = target["dd"] if dd is None else max(0.0, dd - target["dd"])
    capital_excess = 5000.0 if capital is None else max(0.0, capital - 5000.0)
    segment_gap = 4 if positive_segments is None else max(0, 4 - positive_segments)
    c2426_gap = 1.0 if combined_2024_2026 is None else max(0.0, -combined_2024_2026)
    full_gate = (
        ann is not None
        and dd is not None
        and ann > target["ann"]
        and dd <= target["dd"]
        and (capital is None or capital < 5000.0)
    )
    segment_gate = (
        positive_segments is not None
        and positive_segments >= 4
        and combined_2024_2026 is not None
        and combined_2024_2026 > 0
    )
    gap_score = (
        ann_gap / max(target["ann"], 1.0)
        + dd_excess / max(target["dd"], 1.0)
        + capital_excess / 5000.0
        + segment_gap / 4.0
        + min(c2426_gap, 100.0) / 100.0
    )
    return {
        **row,
        "ann_gap": ann_gap,
        "dd_excess": dd_excess,
        "capital_excess": capital_excess,
        "segment_gap": segment_gap,
        "c2426_gap": c2426_gap,
        "gap_score": gap_score,
        "full_gate": full_gate,
        "segment_gate": segment_gate,
        "target_pass": full_gate and segment_gate,
    }


def rows_from_source(source: str, path: Path) -> list[dict]:
    try:
        data = json.loads(path.read_text())
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        return []
    raw_rows = data.get("rows") if isinstance(data, dict) else None
    if not isinstance(raw_rows, list):
        return []
    rows = []
    if source == "funding_sleeve":
        for raw in raw_rows:
            if not isinstance(raw, dict):
                continue
            for profile in PROFILE_TARGETS:
                normalized = normalize_candidate(source, raw, profile=profile)
                if normalized:
                    rows.append(score_candidate(normalized))
        return rows
    for raw in raw_rows:
        if not isinstance(raw, dict):
            continue
        normalized = normalize_candidate(source, raw)
        if normalized:
            rows.append(score_candidate(normalized))
    return rows


def summarize(rows: list[dict]) -> dict:
    summary = {}
    for profile in PROFILE_TARGETS:
        subset = [row for row in rows if row["profile"] == profile]
        nearest = sorted(subset, key=lambda row: row["gap_score"])[:10]
        best_ann = sorted(
            subset,
            key=lambda row: row["annualized_return_pct"] if row["annualized_return_pct"] is not None else -1e9,
            reverse=True,
        )[:5]
        lowest_dd = sorted(
            subset,
            key=lambda row: row["max_drawdown_pct"] if row["max_drawdown_pct"] is not None else 1e9,
        )[:5]
        summary[profile] = {
            "rows": len(subset),
            "passes": sum(1 for row in subset if row["target_pass"]),
            "nearest": nearest,
            "best_ann": best_ann,
            "lowest_dd": lowest_dd,
        }
    return summary


def frontier_bounds(rows: list[dict]) -> dict:
    bounds = {}
    for profile, target in PROFILE_TARGETS.items():
        subset = [
            row
            for row in rows
            if row["profile"] == profile
            and row["capital_excess"] == 0
            and row["segment_gate"]
            and row["annualized_return_pct"] is not None
            and row["max_drawdown_pct"] is not None
        ]
        within_dd = [row for row in subset if row["max_drawdown_pct"] <= target["dd"]]
        at_target_ann = [row for row in subset if row["annualized_return_pct"] > target["ann"]]
        bounds[profile] = {
            "max_ann_within_dd": max(within_dd, key=lambda row: row["annualized_return_pct"], default=None),
            "min_dd_at_target_ann": min(at_target_ann, key=lambda row: row["max_drawdown_pct"], default=None),
        }
    return bounds


def audit_sources(source_specs: list[tuple[str, Path]]) -> dict:
    rows = []
    source_counts = {}
    for source, path in source_specs:
        source_rows = rows_from_source(source, path)
        source_counts[source] = len(source_rows)
        rows.extend(source_rows)
    summary = summarize(rows)
    return {
        "research_only": True,
        "sources": {name: str(path) for name, path in source_specs},
        "source_counts": source_counts,
        "total_rows": len(rows),
        "final_passes": sum(item["passes"] for item in summary.values()),
        "summary": summary,
        "frontier_bounds": frontier_bounds(rows),
    }


def fmt(value: Any) -> str:
    if value is None:
        return "missing"
    if isinstance(value, float):
        return f"{value:.2f}"
    return str(value)


def row_line(row: dict) -> str:
    return (
        f"  - `{row['source']}` `{row['label']}` score `{row['gap_score']:.3f}` "
        f"ann `{fmt(row['annualized_return_pct'])}` gap `{fmt(row['ann_gap'])}` "
        f"DD `{fmt(row['max_drawdown_pct'])}` excess `{fmt(row['dd_excess'])}` "
        f"cap `{fmt(row['capital_quote'])}` cap_excess `{fmt(row['capital_excess'])}` "
        f"pos `{fmt(row['positive_segments'])}` seg_gap `{fmt(row['segment_gap'])}` "
        f"c2426 `{fmt(row['combined_2024_2026_return_pct'])}` c2426_gap `{fmt(row['c2426_gap'])}`"
    )


def compact_row(row: dict | None) -> str:
    if not row:
        return "`None`"
    return (
        f"`{row['source']}` `{row['label']}` ann `{fmt(row['annualized_return_pct'])}` "
        f"DD `{fmt(row['max_drawdown_pct'])}` cap `{fmt(row['capital_quote'])}` "
        f"pos `{fmt(row['positive_segments'])}` c2426 `{fmt(row['combined_2024_2026_return_pct'])}`"
    )


def write_report(result: dict, out_md: Path) -> None:
    lines = [
        "# 2026-07-01 Martingale Target Gap Audit",
        "",
        "This is a read-only gap audit of saved research artifacts. It does not run live, touch Binance, flyingkid, or real funds.",
        "",
        "Targets use the original gates: conservative ann >50% DD <=10%, balanced ann >90% DD <=20%, aggressive ann >110% DD <=30%, capital below 5000U, at least 4/5 positive segments, and positive combined 2024-2026 return.",
        "",
        f"- normalized candidate rows: `{result['total_rows']}`",
        f"- final target passes: `{result['final_passes']}`",
        "",
        "## Sources",
        "",
    ]
    for source, count in result["source_counts"].items():
        lines.append(f"- `{source}` rows `{count}` path `{result['sources'][source]}`")
    lines.append("")
    for profile, item in result["summary"].items():
        target = PROFILE_TARGETS[profile]
        lines.append(f"## {profile}")
        lines.append("")
        lines.append(f"- target ann: `>{target['ann']}`")
        lines.append(f"- target DD: `<={target['dd']}`")
        lines.append(f"- rows: `{item['rows']}`")
        lines.append(f"- passes: `{item['passes']}`")
        bounds = result["frontier_bounds"][profile]
        lines.append(f"- max_ann_within_target_dd: {compact_row(bounds['max_ann_within_dd'])}")
        lines.append(f"- min_dd_at_target_ann: {compact_row(bounds['min_dd_at_target_ann'])}")
        lines.append("- nearest by transparent gap score:")
        for row in item["nearest"][:5]:
            lines.append(row_line(row))
        lines.append("- highest annualized rows:")
        for row in item["best_ann"][:3]:
            lines.append(row_line(row))
        lines.append("- lowest drawdown rows:")
        for row in item["lowest_dd"][:3]:
            lines.append(row_line(row))
        lines.append("")
    lines.extend(
        [
            "## Conclusion",
            "",
        ]
    )
    if result["final_passes"]:
        lines.append("At least one saved row appears to meet the target gates. It requires manual replay before any live-readiness claim.")
    else:
        lines.append("No saved row from the audited DGT, trend, funding, and leak-audit artifacts meets the original target gates. The nearest rows still fail on annualized return, drawdown, segment balance, or missing segment evidence.")
    out_md.write_text("\n".join(lines) + "\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out-json", required=True)
    parser.add_argument("--out-md", required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    result = audit_sources([(name, Path(path)) for name, path in DEFAULT_SOURCES])
    Path(args.out_json).write_text(json.dumps(result, indent=2, sort_keys=True))
    write_report(result, Path(args.out_md))
    print(json.dumps({"rows": result["total_rows"], "final_passes": result["final_passes"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
