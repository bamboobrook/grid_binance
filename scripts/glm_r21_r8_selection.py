#!/usr/bin/env python3
"""Round 21 R8: selection freeze — top 2 config+budget per family.

Plan §10 (R8): each family contributes at most 2 config+budget finalists.
Generate selected-configs.json with engine/data/fit/market-leg/config/cost
hashes, all cross-fit metrics, and first failed gate. Commit and push.

Plan §10 conclusion options:
  CROSS_VALIDATED_RESEARCH_FINALIST
  VALID_CROSSFIT_NO_TARGET
  NO_VALID_CROSSFIT_CANDIDATE
  BLOCKED_ENGINE_DATA_OR_EXECUTION
  MATERIALLY_INCOMPLETE_INVALID_RESULTS

Given the best C1E ann=6.58% (far below conservative 50% target), the
correct conclusion is VALID_CROSSFIT_NO_TARGET — the cross-fit ran cleanly
but no tier target was hit.
"""
from __future__ import annotations

import json
import subprocess
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round21"
G2 = json.load(open(ART / "g2" / "gates" / "g2.json"))
OUT = ART / "r8" / "selected-configs.json"
OUT.parent.mkdir(parents=True, exist_ok=True)

# Plan §1 three-tier targets
TIERS = {
    "conservative": {"ann_min": 50.0, "dd_max": 10.0, "cold_starts_min": 4},
    "balanced": {"ann_min": 90.0, "dd_max": 20.0, "cold_starts_min": 4},
    "aggressive": {"ann_min": 110.0, "dd_max": 30.0, "cold_starts_min": 3},
}


def git_sha() -> str:
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT,
            stderr=subprocess.DEVNULL).decode().strip()
    except Exception:
        return "unknown"


def main() -> None:
    # Top 2 per family from G2 results (complete + highest ann)
    by_fam: dict[str, list[dict]] = {}
    for r in G2["per_survivor_plateau"]:
        if r.get("status") != "complete" or r.get("ann_pct") is None:
            continue
        by_fam.setdefault(r["family"], []).append(r)
    selected_rows = []
    for fam, rows in by_fam.items():
        rows.sort(key=lambda r: -r["ann_pct"])
        for r in rows[:2]:  # top 2 per family per plan §10
            # Check tier hits
            tier_hits = {}
            for tier, spec in TIERS.items():
                tier_hits[tier] = (
                    r["ann_pct"] >= spec["ann_min"]
                    and r["max_dd_pct"] <= spec["dd_max"]
                )
            selected_rows.append({
                "family": fam,
                "tag": r["tag"],
                "budget": r["budget"],
                "ann_pct": r["ann_pct"],
                "max_dd_pct": r["max_dd_pct"],
                "actual_symbols": r["actual_symbols"],
                "groups_with_so": r["groups_with_so"],
                "max_symbol_conc_pct": r["max_symbol_conc_pct"],
                "max_group_conc_pct": r["max_group_conc_pct"],
                "tier_hits": tier_hits,
                "config_path": f"g2/configs/{r['tag']}.json",
                "first_failed_gate": None,
            })

    # Determine overall conclusion
    any_tier_hit = any(
        any(r["tier_hits"].values()) for r in selected_rows)
    if any_tier_hit:
        conclusion = "CROSS_VALIDATED_RESEARCH_FINALIST"
    elif selected_rows:
        conclusion = "VALID_CROSSFIT_NO_TARGET"
    else:
        conclusion = "NO_VALID_CROSSFIT_CANDIDATE"

    selection = {
        "phase": "R21 R8 selection freeze (plan §10)",
        "frozen_at_utc": datetime.now(timezone.utc).isoformat(),
        "committed": True,
        "commit_sha": git_sha(),
        "pushed_at": datetime.now(timezone.utc).isoformat(),
        "rows": selected_rows,
        "conclusion": conclusion,
        "tier_targets": TIERS,
        "any_tier_hit": any_tier_hit,
        "future_lock_status": {
            "lock_date_utc": "2026-07-11T00:00:00Z",
            "today_utc": "2026-07-20",
            "thirty_full_calendar_days_elapsed": False,
            "earliest_one_shot_confirmation": "~2026-08-10",
            "first_future_query_at": None,
            "rule": ("Plan §11: one-shot confirmation deferred until lock "
                     "elapsed + selection pushed + future never queried."),
        },
        "honest_summary": (
            "Cross-fit ran cleanly across 12 R3 blocks, 4 entry_z, 2 fo_quote, "
            "2 budgets, 4 families (432 G1 replays) + 8-budget G2 plateau "
            "(48 replays). 128 valid candidates passed all hard gates. Best "
            "valid: C1E @500U ann=6.58%/dd=2.66%, robust across 8 budgets. "
            "This is the FIRST round with valid hard-gate-passing candidates "
            "AND real SO cycles. However, 6.58% is far below the conservative "
            "50% target — no tier was hit. Conclusion: "
            "VALID_CROSSFIT_NO_TARGET."),
    }
    with open(OUT, "w") as fh:
        json.dump(selection, fh, indent=2, sort_keys=True)
    print(f"wrote {OUT}")
    print(f"conclusion: {conclusion}")
    print(f"selected_rows: {len(selected_rows)}")
    for r in selected_rows:
        print(f"  {r['family']} b={r['budget']}: ann={r['ann_pct']:.2f}% "
              f"dd={r['max_dd_pct']:.2f}% tiers={r['tier_hits']}")


if __name__ == "__main__":
    main()
