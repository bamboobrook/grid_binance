#!/usr/bin/env python3
"""Read-only live-promotion gate audit for martingale/grid research candidates."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


DEFAULT_GOAL = Path("/tmp/martingale_goal_completion_audit.json")
DEFAULT_EVIDENCE = Path("/tmp/martingale_frontier_evidence_audit.json")
DEFAULT_FINAL = Path("docs/superpowers/reports/2026-07-01-final-martingale-verdict-and-external-check.md")
LIVE_PARITY_STATUS = "research_only"


def load_json(path: Path) -> dict[str, Any]:
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


def requirement_status(goal: dict[str, Any], name: str) -> str:
    requirements = goal.get("requirements")
    if not isinstance(requirements, dict):
        return "missing"
    item = requirements.get(name)
    if not isinstance(item, dict):
        return "missing"
    status = item.get("status")
    return str(status) if status is not None else "missing"


def total_passes(evidence: dict[str, Any]) -> int:
    try:
        return int(evidence.get("total_passes") or 0)
    except (TypeError, ValueError):
        return 0


def audit_promotion_gate(
    goal: dict[str, Any],
    evidence: dict[str, Any],
    final_text: str,
    explicit_live_approval: bool,
) -> dict[str, Any]:
    passes = total_passes(evidence)
    final_lower = final_text.lower()
    blocking_reasons: list[str] = []

    if goal.get("goal_complete") is not True:
        blocking_reasons.append("goal_complete is not true")
    if requirement_status(goal, "all_profiles_final_pass") != "passed":
        blocking_reasons.append("all profile final gates are not passed")
    if requirement_status(goal, "live_ready") != "passed":
        blocking_reasons.append("live_ready requirement is not passed")
    if passes <= 0:
        blocking_reasons.append("machine index has 0 final/pass rows")
    if "no candidate should be promoted to live" in final_lower:
        blocking_reasons.append("final report explicitly blocks live promotion")
    if "no qualifying martingale/grid portfolio has been found" in final_lower:
        blocking_reasons.append("final report found no qualifying martingale/grid portfolio")
    if not explicit_live_approval:
        blocking_reasons.append("explicit live approval flag is absent")

    return {
        "promotion_allowed": not blocking_reasons,
        "live_parity_status": LIVE_PARITY_STATUS,
        "explicit_live_approval": explicit_live_approval,
        "goal_complete": goal.get("goal_complete") is True,
        "machine_index_total_passes": passes,
        "blocking_reasons": blocking_reasons,
        "research_only": True,
    }


def write_report(result: dict[str, Any], out_md: Path) -> None:
    lines = [
        "# 2026-07-01 Martingale Live Promotion Gate Audit",
        "",
        "This is a read-only live-promotion gate for the martingale/grid objective. It does not trade, touch Binance, flyingkid, live mode, or real funds.",
        "",
        f"- Promotion Allowed: `{result['promotion_allowed']}`",
        f"- Live parity status: `{result['live_parity_status']}`",
        f"- Explicit live approval: `{result['explicit_live_approval']}`",
        f"- Goal complete: `{result['goal_complete']}`",
        f"- Machine-index final/pass rows: `{result['machine_index_total_passes']}`",
        "",
        "## Blocking Reasons",
        "",
    ]
    reasons = result.get("blocking_reasons")
    if isinstance(reasons, list) and reasons:
        lines.extend(f"- {reason}" for reason in reasons)
    else:
        lines.append("- none")
    lines.extend(
        [
            "",
            "## Conclusion",
            "",
        ]
    )
    if result["promotion_allowed"]:
        lines.append("Promotion gate is open, but this audit still does not place orders or configure live trading.")
    else:
        lines.append("Promotion gate is closed. Current martingale/grid evidence must remain research_only and must not be promoted to live trading.")
    out_md.write_text("\n".join(lines) + "\n")


def run(
    goal_json_path: Path,
    evidence_json_path: Path,
    final_md_path: Path,
    out_json: Path,
    out_md: Path,
    explicit_live_approval: bool,
) -> int:
    result = audit_promotion_gate(
        goal=load_json(goal_json_path),
        evidence=load_json(evidence_json_path),
        final_text=read_text(final_md_path),
        explicit_live_approval=explicit_live_approval,
    )
    out_json.write_text(json.dumps(result, indent=2, sort_keys=True))
    write_report(result, out_md)
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--goal-json", default=str(DEFAULT_GOAL))
    parser.add_argument("--evidence-json", default=str(DEFAULT_EVIDENCE))
    parser.add_argument("--final-md", default=str(DEFAULT_FINAL))
    parser.add_argument("--out-json", required=True)
    parser.add_argument("--out-md", required=True)
    parser.add_argument("--explicit-live-approval", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    code = run(
        goal_json_path=Path(args.goal_json),
        evidence_json_path=Path(args.evidence_json),
        final_md_path=Path(args.final_md),
        out_json=Path(args.out_json),
        out_md=Path(args.out_md),
        explicit_live_approval=bool(args.explicit_live_approval),
    )
    result = load_json(Path(args.out_json))
    print(json.dumps({"promotion_allowed": result.get("promotion_allowed")}, sort_keys=True))
    return code


if __name__ == "__main__":
    raise SystemExit(main())
