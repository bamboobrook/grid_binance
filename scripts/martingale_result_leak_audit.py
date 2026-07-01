#!/usr/bin/env python3
"""Audit saved martingale/grid result artifacts for missed final-gate candidates."""
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

SEGMENT_NAMES = ["h1_2023", "h2_2023", "2024", "2025", "2026_ytd"]


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


def as_int(value: Any, default: int = 0) -> int:
    number = as_float(value)
    return default if number is None else int(number)


def first_value(data: dict, paths: list[tuple[str, ...]]) -> Any:
    for path in paths:
        current: Any = data
        for part in path:
            if not isinstance(current, dict) or part not in current:
                current = None
                break
            current = current[part]
        if current is not None:
            return current
    return None


def infer_positive_segments(data: dict) -> int | None:
    direct = first_value(data, [("positive_segments",), ("segment_gate", "positive_segments")])
    if direct is not None:
        return as_int(direct)
    segments = data.get("segment_metrics")
    if not isinstance(segments, dict):
        return None
    positive = 0
    found = 0
    for name in SEGMENT_NAMES:
        metrics = segments.get(name)
        if not isinstance(metrics, dict):
            continue
        value = as_float(metrics.get("total_return_pct"))
        if value is None:
            continue
        found += 1
        if value > 0:
            positive += 1
    return positive if found else None


def infer_combined_2024_2026(data: dict) -> float | None:
    direct = first_value(
        data,
        [
            ("combined_2024_2026_return_pct",),
            ("segment_gate", "combined_2024_2026_return_pct"),
            ("c2426",),
        ],
    )
    if direct is not None:
        return as_float(direct)
    segments = data.get("segment_metrics")
    if not isinstance(segments, dict):
        return None
    total = 1.0
    found = False
    for name in ["2024", "2025", "2026_ytd"]:
        metrics = segments.get(name)
        if not isinstance(metrics, dict):
            continue
        value = as_float(metrics.get("total_return_pct"))
        if value is None:
            continue
        found = True
        total *= 1.0 + value / 100.0
    return (total - 1.0) * 100.0 if found else None


def infer_live_ok(data: dict) -> bool:
    value = first_value(
        data,
        [
            ("live_parity",),
            ("live_parity_ok",),
            ("live_parity_status",),
        ],
    )
    if value is None:
        return True
    if isinstance(value, bool):
        return value
    return str(value).lower() in {"pass", "passed", "true", "ok", "research_only"}


def normalize_record(data: dict) -> dict:
    profile = str(data.get("profile") or "").lower()
    ann = as_float(
        first_value(
            data,
            [
                ("annualized_return_pct",),
                ("ann",),
                ("full_metrics", "annualized_return_pct"),
                ("on_budget", "annualized_return_pct"),
                ("gate", "annualized_return_pct"),
            ],
        )
    )
    dd = as_float(
        first_value(
            data,
            [
                ("max_drawdown_pct",),
                ("dd",),
                ("full_metrics", "max_drawdown_pct"),
                ("on_budget", "max_drawdown_pct"),
                ("gate", "max_drawdown_pct"),
            ],
        )
    )
    capital = as_float(
        first_value(
            data,
            [
                ("max_input_quote",),
                ("max_capital_used_quote",),
                ("max_capital_used",),
                ("cap",),
                ("on_max_capital_used", "max_capital_used_quote"),
            ],
        )
    )
    budget_blocked = as_int(
        first_value(
            data,
            [
                ("budget_blocked_legs",),
                ("budget_blocked_events",),
                ("blocked",),
            ],
        )
    )
    positive_segments = infer_positive_segments(data)
    combined_2024_2026 = infer_combined_2024_2026(data)
    live_ok = infer_live_ok(data)
    target = PROFILE_TARGETS.get(profile)
    violations = []

    full_gate = False
    if target is None:
        violations.append("unknown profile")
    else:
        if ann is None or ann <= target["ann"]:
            violations.append(f"annualized {ann} <= required {target['ann']}")
        if dd is None or dd > target["dd"]:
            violations.append(f"drawdown {dd} > allowed {target['dd']}")
        if capital is not None and capital >= 5000.0:
            violations.append(f"capital {capital:.2f} is not below 5000.00")
        if budget_blocked > 0:
            violations.append(f"budget blocked events {budget_blocked} > 0")
        full_gate = (
            ann is not None
            and dd is not None
            and ann > target["ann"]
            and dd <= target["dd"]
            and (capital is None or capital < 5000.0)
            and budget_blocked == 0
        )

    if positive_segments is None:
        segment_gate = False
        violations.append("missing segment evidence")
    elif positive_segments < 4:
        segment_gate = False
        violations.append(f"only {positive_segments}/5 positive segments")
    else:
        segment_gate = True
    if combined_2024_2026 is None:
        segment_gate = False
        violations.append("missing 2024-2026 return evidence")
    elif combined_2024_2026 <= 0:
        segment_gate = False
        violations.append(f"2024-2026 return {combined_2024_2026:.2f}% <= 0")

    if not live_ok:
        violations.append("live parity failed")

    return {
        "source": str(data.get("source") or ""),
        "profile": profile,
        "annualized_return_pct": ann,
        "max_drawdown_pct": dd,
        "capital_quote": capital,
        "budget_blocked_events": budget_blocked,
        "positive_segments": positive_segments,
        "combined_2024_2026_return_pct": combined_2024_2026,
        "live_ok": live_ok,
        "full_gate": full_gate,
        "segment_gate": segment_gate,
        "final_gate": full_gate and segment_gate and live_ok,
        "violations": violations,
    }


def candidate_dicts(value: Any) -> list[dict]:
    result = []
    if isinstance(value, dict):
        if any(key in value for key in ("profile", "full_metrics", "annualized_return_pct", "ann", "on_budget")):
            result.append(value)
        for nested in value.values():
            result.extend(candidate_dicts(nested))
    elif isinstance(value, list):
        for item in value:
            result.extend(candidate_dicts(item))
    return result


def scan_json_file(path: Path) -> list[dict]:
    try:
        data = json.loads(path.read_text())
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        return []
    rows = []
    for item in candidate_dicts(data):
        item = {**item, "source": str(path)}
        normalized = normalize_record(item)
        if normalized["profile"] in PROFILE_TARGETS and normalized["annualized_return_pct"] is not None:
            rows.append(normalized)
    return rows


def scan_roots(roots: list[Path]) -> list[dict]:
    rows = []
    for root in roots:
        if root.is_file() and root.suffix == ".json":
            rows.extend(scan_json_file(root))
        elif root.is_dir():
            for path in root.rglob("*.json"):
                rows.extend(scan_json_file(path))
    return rows


def summarize(rows: list[dict]) -> dict:
    summary = {}
    for profile in PROFILE_TARGETS:
        subset = [row for row in rows if row["profile"] == profile]
        full_pass = [row for row in subset if row["full_gate"]]
        final_pass = [row for row in subset if row["final_gate"]]
        best_ann = sorted(
            subset,
            key=lambda row: row["annualized_return_pct"] if row["annualized_return_pct"] is not None else -1e9,
            reverse=True,
        )[:10]
        summary[profile] = {
            "rows": len(subset),
            "full_gate_pass_like": len(full_pass),
            "final_gate_pass": len(final_pass),
            "top_ann": best_ann,
            "full_gate_rows": full_pass[:20],
            "final_gate_rows": final_pass,
        }
    return summary


def write_report(rows: list[dict], out_path: Path) -> None:
    summary = summarize(rows)
    lines = [
        "# 2026-07-01 Martingale Result Leak Audit",
        "",
        "This is a read-only audit of saved JSON artifacts. It does not run live, touch Binance, flyingkid, or real funds.",
        "",
        f"- JSON-like records scanned: `{len(rows)}`",
        "",
    ]
    total_final = sum(item["final_gate_pass"] for item in summary.values())
    total_full = sum(item["full_gate_pass_like"] for item in summary.values())
    lines.append(f"- Full-gate pass-like rows: `{total_full}`")
    lines.append(f"- Final-gate pass rows: `{total_final}`")
    lines.append("")
    for profile, item in summary.items():
        lines.append(f"## {profile}")
        lines.append("")
        lines.append(f"- rows: `{item['rows']}`")
        lines.append(f"- full-gate pass-like: `{item['full_gate_pass_like']}`")
        lines.append(f"- final-gate pass: `{item['final_gate_pass']}`")
        if item["full_gate_rows"]:
            lines.append("- full-gate pass-like rows:")
            for row in item["full_gate_rows"][:10]:
                lines.append(
                    f"  - `{Path(row['source']).name}` ann `{row['annualized_return_pct']}` "
                    f"DD `{row['max_drawdown_pct']}` cap `{row['capital_quote']}` "
                    f"pos `{row['positive_segments']}` c2426 `{row['combined_2024_2026_return_pct']}` "
                    f"final `{row['final_gate']}` violations `{'; '.join(row['violations'])}`"
                )
        if item["top_ann"]:
            lines.append("- top annualized rows:")
            for row in item["top_ann"][:5]:
                lines.append(
                    f"  - `{Path(row['source']).name}` ann `{row['annualized_return_pct']}` "
                    f"DD `{row['max_drawdown_pct']}` cap `{row['capital_quote']}` "
                    f"full `{row['full_gate']}` final `{row['final_gate']}`"
                )
        lines.append("")
    lines.append("## Conclusion")
    lines.append("")
    if total_final:
        lines.append(f"Potential final-gate rows found: `{total_final}`. These require manual replay before any conclusion.")
    else:
        lines.append("Potential final-gate rows found: `0`. The saved JSON artifact pool contains no machine-detected row satisfying the original gates with segment evidence.")
    lines.append("")
    lines.append("This audit is supplementary evidence; replay validation remains authoritative for any future candidate.")
    out_path.write_text("\n".join(lines) + "\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--roots", nargs="+", default=["docs"])
    parser.add_argument("--out-json", required=True)
    parser.add_argument("--out-md", required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    rows = scan_roots([Path(root) for root in args.roots])
    result = {"rows": rows, "summary": summarize(rows)}
    Path(args.out_json).write_text(json.dumps(result, indent=2, sort_keys=True))
    write_report(rows, Path(args.out_md))
    final_passes = sum(item["final_gate_pass"] for item in result["summary"].values())
    print(json.dumps({"rows": len(rows), "final_passes": final_passes}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
