#!/usr/bin/env python3
"""Audit whether the original martingale/grid objective is actually complete."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


PROFILES = ("conservative", "balanced", "aggressive")

DEFAULT_TARGET_GAP = Path("/tmp/martingale_target_gap_audit.json")
DEFAULT_EVIDENCE = Path("/tmp/martingale_frontier_evidence_audit.json")
DEFAULT_EXTERNAL = Path("docs/superpowers/reports/2026-07-01-external-martingale-grid-claim-gate-matrix.md")
DEFAULT_FINAL = Path("docs/superpowers/reports/2026-07-01-final-martingale-verdict-and-external-check.md")


def load_json(path: Path) -> dict:
    try:
        data = json.loads(path.read_text())
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        return {}
    return data if isinstance(data, dict) else {}


def read_text(path: Path) -> str:
    try:
        return path.read_text()
    except (OSError, UnicodeDecodeError):
        return ""


def requirement(status: str, evidence: str) -> dict:
    return {"status": status, "evidence": evidence}


def profile_passes(target_gap: dict, profile: str) -> int:
    summary = target_gap.get("summary")
    if not isinstance(summary, dict):
        return 0
    item = summary.get(profile)
    if not isinstance(item, dict):
        return 0
    value = item.get("passes", 0)
    try:
        return int(value)
    except (TypeError, ValueError):
        return 0


def nearest_line(target_gap: dict, profile: str) -> str:
    summary = target_gap.get("summary")
    if not isinstance(summary, dict):
        return "no nearest-row evidence"
    item = summary.get(profile)
    if not isinstance(item, dict):
        return "no nearest-row evidence"
    nearest = item.get("nearest")
    if not isinstance(nearest, list) or not nearest:
        return "no nearest-row evidence"
    row = nearest[0]
    if not isinstance(row, dict):
        return "nearest-row evidence malformed"
    ann = row.get("annualized_return_pct")
    dd = row.get("max_drawdown_pct")
    cap = row.get("capital_quote")
    pos = row.get("positive_segments")
    c2426 = row.get("combined_2024_2026_return_pct")
    label = row.get("label") or row.get("source") or "candidate"
    return f"nearest `{label}` ann `{ann}` DD `{dd}` cap `{cap}` pos `{pos}` c2426 `{c2426}`"


def audit_completion(target_gap: dict, evidence: dict, external_text: str, final_text: str) -> dict:
    final_passes = int(target_gap.get("final_passes") or 0)
    total_rows = int(target_gap.get("total_rows") or 0)
    indexed_passes = int(evidence.get("total_passes") or 0)
    reports = evidence.get("reports") if isinstance(evidence.get("reports"), list) else []

    profile_counts = {profile: profile_passes(target_gap, profile) for profile in PROFILES}
    all_profiles_pass = all(count > 0 for count in profile_counts.values())
    no_external_claim = "No public external claim found" in external_text
    final_says_no_candidate = "No qualifying martingale/grid portfolio has been found" in final_text
    no_live_promotion = "No candidate should be promoted to live" in final_text

    requirements = {
        "candidate_pool_indexed": requirement(
            "passed" if total_rows > 0 and reports else "failed",
            f"target_gap rows `{total_rows}`, evidence reports `{len(reports)}`",
        ),
        "all_profiles_final_pass": requirement(
            "passed" if final_passes > 0 and all_profiles_pass else "failed",
            f"{final_passes} final target passes; profile pass counts {profile_counts}",
        ),
        "conservative_gate": requirement(
            "passed" if profile_counts["conservative"] > 0 else "failed",
            f"conservative pass count `{profile_counts['conservative']}`; {nearest_line(target_gap, 'conservative')}",
        ),
        "balanced_gate": requirement(
            "passed" if profile_counts["balanced"] > 0 else "failed",
            f"balanced pass count `{profile_counts['balanced']}`; {nearest_line(target_gap, 'balanced')}",
        ),
        "aggressive_gate": requirement(
            "passed" if profile_counts["aggressive"] > 0 else "failed",
            f"aggressive pass count `{profile_counts['aggressive']}`; {nearest_line(target_gap, 'aggressive')}",
        ),
        "external_claim_check": requirement(
            "failed" if no_external_claim else "unknown",
            "external matrix found no public qualifying martingale/grid claim"
            if no_external_claim
            else "external matrix text did not contain a definitive no-claim marker",
        ),
        "live_ready": requirement(
            "failed" if no_live_promotion or final_says_no_candidate else "unknown",
            "final report says no candidate should be promoted to live"
            if no_live_promotion
            else "no final live-ready candidate evidence found",
        ),
        "machine_index_final_pass": requirement(
            "passed" if indexed_passes > 0 else "failed",
            f"evidence audit machine-reported final/pass rows `{indexed_passes}`",
        ),
    }
    goal_complete = all(item["status"] == "passed" for item in requirements.values())
    return {
        "goal_complete": goal_complete,
        "requirements": requirements,
    }


def write_report(result: dict, out_md: Path) -> None:
    lines = [
        "# 2026-07-01 Martingale Goal Completion Audit",
        "",
        "This is a read-only requirement-by-requirement audit of the original objective. It does not trade, touch Binance, flyingkid, live mode, or real funds.",
        "",
        f"- Goal Complete: `{result['goal_complete']}`",
        "",
        "## Requirements",
        "",
    ]
    for name, item in result["requirements"].items():
        lines.append(f"- `{name}` status `{item['status']}` evidence: {item['evidence']}")
    lines.extend(
        [
            "",
            "## Conclusion",
            "",
        ]
    )
    if result["goal_complete"]:
        lines.append("All audited requirements are passed by current evidence.")
    else:
        lines.append("The original objective is not complete under current evidence. At least one required gate has failed or lacks live-ready candidate proof.")
    out_md.write_text("\n".join(lines) + "\n")


def run(
    target_gap_path: Path,
    evidence_path: Path,
    external_path: Path,
    final_path: Path,
    out_json: Path,
    out_md: Path,
) -> int:
    result = audit_completion(
        target_gap=load_json(target_gap_path),
        evidence=load_json(evidence_path),
        external_text=read_text(external_path),
        final_text=read_text(final_path),
    )
    out_json.write_text(json.dumps(result, indent=2, sort_keys=True))
    write_report(result, out_md)
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--target-gap-json", default=str(DEFAULT_TARGET_GAP))
    parser.add_argument("--evidence-json", default=str(DEFAULT_EVIDENCE))
    parser.add_argument("--external-md", default=str(DEFAULT_EXTERNAL))
    parser.add_argument("--final-md", default=str(DEFAULT_FINAL))
    parser.add_argument("--out-json", required=True)
    parser.add_argument("--out-md", required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    code = run(
        target_gap_path=Path(args.target_gap_json),
        evidence_path=Path(args.evidence_json),
        external_path=Path(args.external_md),
        final_path=Path(args.final_md),
        out_json=Path(args.out_json),
        out_md=Path(args.out_md),
    )
    result = load_json(Path(args.out_json))
    print(json.dumps({"goal_complete": result.get("goal_complete")}, sort_keys=True))
    return code


if __name__ == "__main__":
    raise SystemExit(main())
