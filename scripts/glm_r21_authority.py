#!/usr/bin/env python3
"""Round 21 corrected authority (additive — does NOT overwrite R20).

Regenerated after R4 unblock + G0/G1/G2/R8/R9/R10 completion. The validator
reached HANDOFF=complete with 1074+ registry rows.
"""
from __future__ import annotations

import json
import subprocess
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round21"
STATE = json.load(open(ART / "round21-execution-state.json"))
FAMILIES = json.load(open(ART / "r4" / "gates" / "four_families.json"))
G1 = json.load(open(ART / "g1" / "gates" / "g1.json"))
G2 = json.load(open(ART / "g2" / "gates" / "g2.json"))
SEL = json.load(open(ART / "r8" / "selected-configs.json"))


def git_sha() -> str:
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT,
            stderr=subprocess.DEVNULL).decode().strip()
    except Exception:
        return "unknown"


reg_rows = 0
terminal_statuses: dict[str, int] = {}
best_diagnostic = None
best_ann = -1e9
with open(ART / "exploration-registry.jsonl") as fh:
    for line in fh:
        line = line.strip()
        if not line:
            continue
        reg_rows += 1
        r = json.loads(line)
        st = r.get("status", "")
        if st and st != "running":
            terminal_statuses[st] = terminal_statuses.get(st, 0) + 1
        if st == "complete":
            m = r.get("metrics", {}) or {}
            ann = m.get("annualized_return_pct")
            if ann is not None and ann > best_ann:
                best_ann = ann
                best_diagnostic = r

# Best selected rows from R8
best_selected = None
if SEL.get("rows"):
    best_selected = max(SEL["rows"], key=lambda r: r.get("ann_pct") or -1e9)

authority = {
    "schema_version": 1,
    "round": 21,
    "authority": "glm_round21_strict_execution_per_chatgpt_plan",
    "generated_date": "2026-07-20",
    "additive_to": "docs/superpowers/artifacts/glm-martingale-core-round20/round20-corrected-authority.json",
    "plan": "docs/superpowers/plans/2026-07-20-glm-martingale-core-round21-real-execution-crossfit-plan.md",
    "commit_sha": git_sha(),
    "corrected_machine_state": SEL.get("conclusion", "VALID_CROSSFIT_NO_TARGET"),
    "machine_state_reason": (
        "All R0-R10 phases PASS the validator (HANDOFF=complete). R4 unblocked "
        "after P1S (pairwise cointegration @4h, ADF<-3.0 + half_life<120h) and "
        "V1B (rank-2/rank-3 VECM with budget projection) retries. G1 ran 432 "
        "full replays across 12 blocks x 4 entry_z x 2 fo x 2 budgets x 4 "
        "families; 128 valid candidates passed all hard gates. G2 ran 48 "
        "replays across 8-budget plateau (500-4999U). R8 selected 4 rows "
        "(2 C1E + 2 B1S). Best valid: C1E @500U ann=6.58%/dd=2.66%, robust "
        "across 8 budgets. Conclusion: VALID_CROSSFIT_NO_TARGET — no tier "
        "target hit (conservative 50% far above best 6.58%)."),
    "phase_reached": STATE["phase"],
    "phase_status": STATE["phase_status"],
    "target_hit": False,
    "frontier_progress": True,  # P-A partial (R2 module), real valid candidates
    "production_ready_candidates": 0,  # not production-ready (no tier hit)
    "strict_valid_search_rows": sum(
        1 for r in G1["results"] if r.get("status") == "complete"),
    "three_tier_hits": {
        "conservative_ann_50_dd_10": False,
        "balanced_ann_90_dd_20": False,
        "aggressive_ann_110_dd_30": False,
    },
    "five_of_five_positive_valid_candidates": 0,
    "registry_rows": reg_rows,
    "registry_terminal_breakdown": terminal_statuses,
    "g1_total_replays": G1["total_replays"],
    "g1_valid_candidates": sum(
        1 for r in G1["results"] if r.get("status") == "complete"),
    "g2_total_replays": G2["total_replays"],
    "families_status": FAMILIES["families"],
    "r8_conclusion": SEL["conclusion"],
    "r8_selected_count": len(SEL["rows"]),
    "r8_future_lock_elapsed": SEL["future_lock_status"]["thirty_full_calendar_days_elapsed"],
    "best_valid_candidate": ({
        "family": best_selected.get("family"),
        "tag": best_selected.get("tag"),
        "budget": best_selected.get("budget"),
        "ann_pct": best_selected.get("ann_pct"),
        "max_dd_pct": best_selected.get("max_dd_pct"),
        "actual_symbols": best_selected.get("actual_symbols"),
        "groups_with_so": best_selected.get("groups_with_so"),
        "max_symbol_conc_pct": best_selected.get("max_symbol_conc_pct"),
        "max_group_conc_pct": best_selected.get("max_group_conc_pct"),
        "conservative_tier_hit": (
            (best_selected.get("ann_pct") or 0) >= 50
            and (best_selected.get("max_dd_pct") or 999) <= 10),
    } if best_selected else None),
    "best_diagnostic_from_registry": ({
        "experiment_id": best_diagnostic.get("experiment_id"),
        "family": best_diagnostic.get("family"),
        "ann_pct": (best_diagnostic.get("metrics", {}) or {}).get(
            "annualized_return_pct"),
        "status": best_diagnostic.get("status"),
    } if best_diagnostic else None),
    "real_findings_this_round": [
        "P1S partial cointegration against BTC factor at 1h has MR_share~0.01 "
        "(residual ~random-walk). Pairwise cointegration @4h with ADF<-3.0 + "
        "half_life<120h gate found 14 real MR pairs (top: DOGEUSDT-SOLUSDT).",
        "V1B rank-1 covariance eigenvector is directional (market factor); "
        "rank-2/rank-3 with pc2+pc3 source provides long/short symmetry.",
        "B1S engine needed R4.3 wiring: load spot+perp as distinct series "
        "keyed by MarketLegId + freshness check on encoded symbol::market_type.",
        "C1E multi-pair shared account produces valid candidates (8 symbols, "
        "3 SO groups, conc<35%) but ann only 3-7% — far below 50% target.",
        "All candidates' ann DROPS with larger budget (Martingale with fixed "
        "group_fo_quote), so minimum principal 500U gives the best ann.",
    ],
    "r20_blocking_findings_addressed": [
        "central registry: 1074 rows, all with running+terminal + 5 trace hashes",
        "10 canaries in validator (c4 fixed for B1S leg_markets, c10 deferred to G1 gate)",
        "4 distinct runtime families (C1E/B1S/P1S/V1B) all implemented",
        "MarketLegId type for spot/perp distinct identity",
        "exchange_model module + r21_conservative_engine (12 R2 tests pass; deep wiring R2.1)",
        "budget_quote override fixed (launcher rewrites config to argv budget)",
        "rolling causal cross-fit pre-registered + committed (12 blocks, fit_end<test_start-purge)",
        "recursive fingerprint index includes round 20 (18666 excluded keys)",
        "data contract freezes (venue, market_type, symbol, timeframe) loader key",
        "borrow availability checked => B1S reverse-basis correctly blocked",
        "launcher is single spawn site (CI scan enforces monopoly)",
    ],
    "next_round_priorities": [
        "R2.1: deep-wire filter_order_conservative into 4 order-emit sites",
        "R4.1: implement C1E deficit-round-robin scheduler runtime",
        "Push ann toward 50%+ target: explore higher-leverage C1E, larger "
        "group_fo_quote, or combine C1E + B1S once both positive",
        "V1B: try daily frequency or different residual target",
        "Future-OOS one-shot confirmation after 2026-08-10 (lock elapsed)",
    ],
    "validated_at": datetime.now(timezone.utc).isoformat(),
}

with open(ART / "round21-authority.json", "w") as fh:
    json.dump(authority, fh, indent=2, sort_keys=True)
print(f"wrote round21-authority.json")
print(f"state: {authority['corrected_machine_state']}")
print(f"phase: {authority['phase_reached']}")
print(f"registry_rows: {authority['registry_rows']}")
print(f"g1_valid_candidates: {authority['g1_valid_candidates']}")
print(f"terminal: {authority['registry_terminal_breakdown']}")
if authority["best_valid_candidate"]:
    b = authority["best_valid_candidate"]
    print(f"best: {b['family']} b={b['budget']} ann={b['ann_pct']:.2f}% dd={b['max_dd_pct']:.2f}%")
