#!/usr/bin/env python3
"""Summarize research-only martingale/frontier evidence reports."""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


PROFILES = ("conservative", "balanced", "aggressive")

DEFAULT_REPORTS = [
    ("saved_json_leak_audit", "docs/superpowers/reports/2026-07-01-martingale-result-leak-audit.md"),
    ("target_gap_audit", "docs/superpowers/reports/2026-07-01-martingale-target-gap-audit.md"),
    ("funding_sleeve", "docs/superpowers/reports/2026-07-01-funding-sleeve-probe.md"),
    ("trend_sleeve", "docs/superpowers/reports/2026-07-01-trend-sleeve-frontier-probe.md"),
    ("trend_risk_control", "docs/superpowers/reports/2026-07-01-trend-risk-control-probe.md"),
    ("dgt_dynamic_grid", "docs/superpowers/reports/2026-07-01-dgt-dynamic-grid-probe.md"),
    ("pair_neutral_grid", "docs/superpowers/reports/2026-07-01-pair-neutral-grid-probe.md"),
    (
        "pair_neutral_risk_control",
        "docs/superpowers/reports/2026-07-01-pair-neutral-risk-control-probe.md",
    ),
    (
        "pair_neutral_portfolio",
        "docs/superpowers/reports/2026-07-01-pair-neutral-portfolio-probe.md",
    ),
    (
        "external_claim_gate_matrix",
        "docs/superpowers/reports/2026-07-01-external-martingale-grid-claim-gate-matrix.md",
    ),
    (
        "search_freeze_reopen_criteria",
        "docs/superpowers/reports/2026-07-01-martingale-grid-search-freeze-and-reopen-criteria.md",
    ),
    ("goal_completion_audit", "docs/superpowers/reports/2026-07-01-martingale-goal-completion-audit.md"),
    ("final_external_check", "docs/superpowers/reports/2026-07-01-final-martingale-verdict-and-external-check.md"),
]


def int_after(pattern: str, text: str) -> int | None:
    match = re.search(pattern, text, flags=re.IGNORECASE | re.MULTILINE)
    return int(match.group(1)) if match else None


def parse_probe_report(name: str, text: str) -> dict:
    rows = (
        int_after(r"JSON-like records scanned:\s*`?(\d+)`?", text)
        or int_after(r"normalized candidate rows:\s*`?(\d+)`?", text)
        or int_after(r"Symbols scanned:\s*`?(\d+)`?", text)
        or int_after(r"^- rows:\s*`?(\d+)`?", text)
        or 0
    )
    passes = {}
    for profile in PROFILES:
        profile_title = profile.capitalize()
        passes[profile] = (
            int_after(rf"{profile_title} passes:\s*`?(\d+)`?", text)
            if re.search(rf"{profile_title} passes:", text, flags=re.IGNORECASE)
            else None
        )
        if passes[profile] is None:
            section = re.search(rf"## {profile}\s+(.*?)(?=\n## |\Z)", text, flags=re.IGNORECASE | re.DOTALL)
            passes[profile] = int_after(r"passes:\s*`?(\d+)`?", section.group(1)) if section else 0
    if "Final-gate pass rows" in text:
        final_passes = int_after(r"Final-gate pass rows:\s*`?(\d+)`?", text) or 0
        passes = {profile: 0 for profile in PROFILES}
        passes["final_gate_rows"] = final_passes
    if "Goal Complete:" in text:
        passes = {profile: 0 for profile in PROFILES}
        passes["goal_complete"] = 1 if re.search(r"Goal Complete:\s*`?True`?", text) else 0
    total_passes = sum(value for key, value in passes.items() if key in PROFILES)
    total_passes += passes.get("final_gate_rows", 0)
    total_passes += passes.get("goal_complete", 0)
    return {
        "name": name,
        "rows": rows,
        "passes": passes,
        "total_passes": total_passes,
    }


def audit_reports(report_specs: list[tuple[str, Path]]) -> dict:
    reports = []
    for name, path in report_specs:
        text = Path(path).read_text()
        report = parse_probe_report(name, text)
        report["path"] = str(path)
        reports.append(report)
    return {
        "reports": reports,
        "total_rows": sum(report["rows"] for report in reports),
        "total_passes": sum(report["total_passes"] for report in reports),
    }


def write_report(result: dict, out_md: Path) -> None:
    lines = [
        "# 2026-07-01 Martingale Frontier Evidence Audit",
        "",
        "This is a read-only index of current research evidence. It does not trade, touch Binance, flyingkid, live mode, or real funds.",
        "",
        f"- reports indexed: `{len(result['reports'])}`",
        f"- rows/symbols indexed where available: `{result['total_rows']}`",
        f"- machine-reported final/pass rows: `{result['total_passes']}`",
        "",
        "## Indexed Reports",
        "",
    ]
    for report in result["reports"]:
        lines.append(
            f"- `{report['name']}` rows `{report['rows']}` passes `{report['total_passes']}` source `{report['path']}`"
        )
    lines.extend(
        [
            "",
            "## Conclusion",
            "",
        ]
    )
    if result["total_passes"]:
        lines.append("At least one indexed report claims a pass-like row. It requires manual replay before any conclusion.")
    else:
        lines.append("No indexed research report currently contains a machine-reported final/pass row satisfying the original gates.")
    out_md.write_text("\n".join(lines) + "\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out-json", required=True)
    parser.add_argument("--out-md", required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    result = audit_reports([(name, Path(path)) for name, path in DEFAULT_REPORTS])
    Path(args.out_json).write_text(json.dumps(result, indent=2, sort_keys=True))
    write_report(result, Path(args.out_md))
    print(json.dumps({"reports": len(result["reports"]), "total_passes": result["total_passes"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
