#!/usr/bin/env python3
"""Round 21 corrected authority (additive — does NOT overwrite R20).

Plan §15.7: '不覆盖历史 artifact，修正只用 additive authority'. This file
sits alongside round20-corrected-authority.json and records the Round 21
machine state per plan §13 + §16.
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


def git_sha() -> str:
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT,
            stderr=subprocess.DEVNULL).decode().strip()
    except Exception:
        return "unknown"


# Count registry rows
reg_rows = 0
terminal_statuses: dict[str, int] = {}
complete_rows: list[dict] = []
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
            complete_rows.append(r)

# Best C1E result (the only family that produced complete replays — actually
# all C1E were rejected_concentration, so the best DIAGNOSTIC row).
best_diagnostic = None
with open(ART / "exploration-registry.jsonl") as fh:
    best_ann = -1e9
    for line in fh:
        line = line.strip()
        if not line:
            continue
        r = json.loads(line)
        if r.get("status") in ("complete", "rejected_concentration", "rejected_gate"):
            m = r.get("metrics", {}) or {}
            ann = m.get("annualized_return_pct")
            if ann is not None and ann > best_ann:
                best_ann = ann
                best_diagnostic = r

authority = {
    "schema_version": 1,
    "round": 21,
    "authority": "glm_round21_strict_execution_per_chatgpt_plan",
    "generated_date": "2026-07-20",
    "supersedes": [],
    "additive_to": "docs/superpowers/artifacts/glm-martingale-core-round20/"
                   "round20-corrected-authority.json",
    "plan": "docs/superpowers/plans/2026-07-20-glm-martingale-core-round21-"
            "real-execution-crossfit-plan.md",
    "commit_sha": git_sha(),
    "corrected_machine_state": "BLOCKED_ENGINE_DATA_OR_EXECUTION",
    "machine_state_reason": (
        "R4 produced real frozen fits for C1E (5 fits, 2/12 blocks) and B1S "
        "(72 fits, 12/12 blocks). P1S produced 0 fits (MR_share ≈ 0.004-0.011 "
        "for all symbols vs BTC factor at 1h — partial cointegration is "
        "empirically absent). V1B produced 0 fits (rank-1 covariance eigenvec-"
        "tor is a directional market factor that cannot satisfy long_gross>=0.35 "
        "AND short_gross>=0.35). Plan §16: any blocked mandatory family => "
        "round BLOCKED_ENGINE_DATA_OR_EXECUTION."),
    "phase_reached": STATE["phase"],
    "phase_status": STATE["phase_status"],
    "blocked_reasons": STATE["blocked_reasons"],
    "target_hit": False,
    "frontier_progress": False,
    "production_ready_candidates": 0,
    "strict_valid_search_rows": 0,
    "three_tier_hits": {
        "conservative_ann_50_dd_10": False,
        "balanced_ann_90_dd_20": False,
        "aggressive_ann_110_dd_30": False,
    },
    "five_of_five_positive_valid_candidates": 0,
    "registry_rows": reg_rows,
    "registry_terminal_breakdown": terminal_statuses,
    "families_status": FAMILIES["families"],
    "g1_partial_replays": G1["total_replays"],
    "g1_partial_families": G1["families_run"],
    "g1_blocked_families": G1["blocked_families"],
    "best_diagnostic_row": ({
        "experiment_id": best_diagnostic.get("experiment_id"),
        "family": best_diagnostic.get("family"),
        "status": best_diagnostic.get("status"),
        "first_failed_gate": best_diagnostic.get("first_failed_gate"),
        "ann_pct": (best_diagnostic.get("metrics", {}) or {}).get(
            "annualized_return_pct"),
        "max_dd_pct": (best_diagnostic.get("metrics", {}) or {}).get(
            "max_drawdown_pct"),
        "actual_symbols": (best_diagnostic.get("metrics", {}) or {}).get(
            "actual_symbols"),
        "groups_with_so": (best_diagnostic.get("metrics", {}) or {}).get(
            "groups_with_so"),
        "max_symbol_concentration_pct": (best_diagnostic.get("metrics", {}) or {}).get(
            "max_symbol_concentration_pct"),
        "max_group_concentration_pct": (best_diagnostic.get("metrics", {}) or {}).get(
            "max_group_concentration_pct"),
        "min_liquidation_buffer_pct": (best_diagnostic.get("metrics", {}) or {}).get(
            "min_liquidation_buffer_pct"),
        "budget": best_diagnostic.get("budget"),
        "fingerprint_sha256": best_diagnostic.get("fingerprint_sha256"),
    } if best_diagnostic else None),
    "r20_blocking_findings_addressed_by_r21": [
        "central registry bootstrapped + enforced (10 canaries in validator)",
        "exchange_model module now exists with 12 R2 canary tests (deep wiring is R2.1)",
        "MarketLegId type added (spot/perp distinct, R20 B1 dedup bug fixed at type level)",
        "SynchronizedFit.weights field added (V1B signed weights expressible)",
        "min_liquidation_buffer_pct is now non-null (R2 §4.12)",
        "family enum extended to accept C1E/B1S/P1S/V1B",
        "recursive fingerprint index now includes round 20 (18666 excluded keys)",
        "data contract freezes (venue, market_type, symbol, timeframe) loader key",
        "borrow availability checked => B1S reverse-basis correctly blocked",
        "rolling causal cross-fit manifest pre-registered + committed before load",
        "launcher is the single spawn site (CI scan enforces monopoly)",
        "budget_quote override bug fixed (launcher rewrites config to argv budget)",
    ],
    "real_findings_this_round": [
        "P1S partial cointegration against BTC factor has MR_share ≈ 0.01 at "
        "1h frequency — the residual is ~random-walk. No 1h-frequency MR-"
        "dominant relationship exists in the 12-symbol universe vs BTC.",
        "V1B rank-1 covariance eigenvector is a directional market factor "
        "(long_gross=1.0 on 8/12 blocks). The budget constraint requires "
        "both-side gross, which rank-1 cannot provide.",
        "C1E pairs produce real SO cycles but symbol concentration >50% by "
        "construction (only 2 symbols per pair). A multi-pair C1E scheduler "
        "(R4.1) is needed to dilute concentration.",
        "B1S spot/perp fit artifacts are correct but the engine's symbol-"
        "keyed data loader dedupes [BTCUSDT, BTCUSDT] to 1 series. The "
        "engine needs a per-leg market_type loader (R4.3).",
    ],
    "next_plan_recommendations": [
        "R2.1: deep-wire filter_order_conservative into the 4 order-emit "
        "sites of run_synchronized_cycle_replay.",
        "R4.1: implement the C1E deficit-round-robin scheduler runtime so "
        "multiple disjoint pairs share one account and dilute concentration.",
        "R4.3: extend the engine's data loader to load spot+perp as distinct "
        "series keyed by MarketLegId, then re-run B1S.",
        "R4.P1S: try daily/4h frequency OR PC1 (first principal component) "
        "as the cointegrating factor instead of BTC. The 1h MR signal is "
        "too weak.",
        "R4.V1B: implement VECM rank>1 or target a non-factor residual (e.g. "
        "second principal component) so both-side gross is achievable.",
        "After R4 unblocks: resume the G0/G1/G2/R8 pipeline per plan.",
    ],
    "validated_at": datetime.now(timezone.utc).isoformat(),
}

with open(ART / "round21-authority.json", "w") as fh:
    json.dump(authority, fh, indent=2, sort_keys=True)
print(f"wrote {ART / 'round21-authority.json'}")
print(f"state: {authority['corrected_machine_state']}")
print(f"phase_reached: {authority['phase_reached']}")
print(f"registry_rows: {authority['registry_rows']}")
print(f"terminal_breakdown: {authority['registry_terminal_breakdown']}")
if authority["best_diagnostic_row"]:
    b = authority["best_diagnostic_row"]
    print(f"best_diagnostic: {b['family']} ann={b['ann_pct']} dd={b['max_dd_pct']} "
          f"status={b['status']} ffg={b['first_failed_gate']}")
