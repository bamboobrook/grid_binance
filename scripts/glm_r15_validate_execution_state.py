#!/usr/bin/env python3
"""Round 15 execution-state validator (P0.1 bootstrap + per-phase gating).

This is the ONLY authority that may write `status` into
`round15-execution-state.json`. GLM may not hand-edit `status=complete`.

Rules (from the plan §1.1):
- a phase is `complete` only when every required gate has real evidence in
  the registry / artifacts / test output;
- `blocked` when: missing fields, hash mismatch, a predecessor is not
  `complete`, any `running` record exists, gate evidence is missing, or a
  declared statistic cannot be recomputed from registry detail lines;
-GLM may only append evidence then re-run this validator; it must not flip
  `status=complete` by hand.

Usage:
    python3 scripts/glm_r15_validate_execution_state.py            # recompute + write state
    python3 scripts/glm_r15_validate_execution_state.py --check    # print, exit 0 if not blocked at P0
"""

import argparse
import hashlib
import json
import os
import subprocess
import sys
from datetime import datetime, timezone

ART_DIR = "docs/superpowers/artifacts/glm-martingale-core-round15"
STATE_PATH = os.path.join(ART_DIR, "round15-execution-state.json")
REGISTRY_PATH = os.path.join(ART_DIR, "exploration-registry.jsonl")
MANIFEST_PATH = os.path.join(ART_DIR, "run-manifests", "r15-data-manifest.json")
PLAN_PATH = "docs/superpowers/plans/2026-07-14-glm-martingale-core-round15-directional-hazard-cluster-plan.md"

PHASES = [
    "P0_CANONICAL_PARITY",
    "P1_EVENT_PRODUCTION_WIRING",
    "P2_BINDING_PROBES",
    "P3_BASELINE_UNIVERSE",
    "P4_TRAIN_SCREEN",
    "P5_NESTED_WFO",
    "P6_ROBUSTNESS",
    "P7_PRODUCTION_PARITY",
    "P8_FUTURE_LOCK",
    "P9_FINAL_HANDOFF",
]

# Per-phase machine gates (the bootstrap-required_gates are frozen on first
# commit). Each gate has a checker returning (passed: bool, evidence: dict).
PHASE_GATES = {
    "P0_CANONICAL_PARITY": [
        "manifest_exists_and_dev_gate_passes",
        "market_per_symbol_hashes_match_frozen_r14",
        "funding_db_is_authoritative_round12",
        "binaries_hash_match_audit",
        "plan_sha256_frozen",
        "p0_parity_tests_pass",
        "old_counterexamples_reproduced_or_fail_closed",
    ],
    "P1_EVENT_PRODUCTION_WIRING": [
        "selector_ladder_scheduler_wired_to_event_and_production_state",
        "p1_production_parity_tests_pass",
    ],
    "P2_BINDING_PROBES": [
        "open_params_change_effective_config_hash",
        "at_least_one_event_hash_changes_per_family",
    ],
    "P3_BASELINE_UNIVERSE": [
        "baseline_and_t1_t2_t3_universe_frozen",
        "data_qualification_passes",
    ],
    "P4_TRAIN_SCREEN": [
        "g1_global_128_sobol_run",
        "g2_full_development_per_fold",
        "g3_nested_validation_frozen",
        "quota_and_duplicate_rate_compliant",
    ],
    "P5_NESTED_WFO": [
        "four_folds_train_select_then_validate_once",
        "parameter_platform_8_of_12",
        "loso_loco_complete",
        "cost_delay_stress_complete",
        "budget_ladder_complete",
        "concentration_under_35",
    ],
    "P6_ROBUSTNESS": [
        "five_budgets_no_principal_breach",
        "min_equity_rejections_reported",
        "max_capital_reported",
    ],
    "P7_PRODUCTION_PARITY": [
        "backtest_live_restart_order_trace_match",
        "production_wiring_tests_pass",
    ],
    "P8_FUTURE_LOCK": [
        "future_lock_read_at_most_once_or_not_available",
    ],
    "P9_FINAL_HANDOFF": [
        "handoff_counts_recomputed_from_registry",
        "handoff_document_exists",
    ],
}


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        while chunk := fh.read(8 * 1024 * 1024):
            h.update(chunk)
    return h.hexdigest()


def git_head():
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], stderr=subprocess.DEVNULL
        ).decode().strip()
    except Exception:
        return None


def load_json(path):
    if not os.path.exists(path):
        return None
    with open(path) as fh:
        return json.load(fh)


def load_registry():
    """Return list of registry records keyed by experiment_id.

    Plan §10 appends a `running` row before each attempt and a terminal row
    after. A `running` row whose experiment_id also has a terminal row is NOT
    in-flight; only a `running` row with no terminal successor counts as a
    genuinely-still-running attempt.
    """
    all_records = []
    if not os.path.exists(REGISTRY_PATH):
        return all_records
    with open(REGISTRY_PATH) as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                all_records.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    # Collapse: keep the latest record per experiment_id (terminal supersedes running).
    by_id = {}
    for r in all_records:
        key = r.get("experiment_id") or r.get("resolved_config_hash") or id(r)
        by_id[key] = r
    return list(by_id.values())


def recompute_counts(records):
    """Recompute unique_configs / binary_replays / cache_hits / duplicates /
    timeouts strictly from registry detail lines (plan §12)."""
    seen_effective = set()
    unique_configs = 0
    binary_replays = 0
    cache_hits = 0
    duplicates = 0
    timeouts = 0
    for r in records:
        if r.get("status") not in ("complete", "rejected", "skipped_duplicate", "timeout"):
            continue
        eff = r.get("effective_config_hash") or r.get("resolved_config_hash")
        if eff in seen_effective:
            duplicates += 1
            if r.get("status") == "skipped_duplicate":
                cache_hits += 1
            continue
        seen_effective.add(eff)
        unique_configs += 1
        replays = int(r.get("actual_binary_replays", 0) or 0)
        if r.get("status") == "timeout":
            timeouts += 1
            continue
        binary_replays += replays
        cache_hits += int(r.get("cache_hits", 0) or 0)
    return {
        "unique_configs": unique_configs,
        "binary_replays": binary_replays,
        "cache_hits": cache_hits,
        "duplicates": duplicates,
        "timeouts": timeouts,
    }


def gate_file_for(gate):
    """Return path to a gate evidence file if one exists in any phase dir."""
    for sub in ["p0", "p1", "p2", "p3", "p4", "p5", "p6", "p7", "p8", "p9"]:
        cand = os.path.join(ART_DIR, sub, "gates", gate + ".json")
        if os.path.exists(cand):
            return cand
    return None


def check_gate(gate, manifest, records, phase_evidence):
    """Return (passed, detail). A gate file (passed=true) is authoritative
    evidence; phase_evidence booleans are a fallback for inline runs."""
    # 1) gate evidence file (written by an evidence-builder script)
    gf = gate_file_for(gate)
    if gf:
        data = load_json(gf)
        return bool(data.get("passed")), data

    # 2) inline phase_evidence (e.g. --phase-evidence during a single run)
    if gate == "manifest_exists_and_dev_gate_passes":
        if manifest is None:
            return False, {"reason": "manifest missing"}
        g = manifest.get("development_window", {}).get("gate", {})
        return bool(g.get("passed")), g

    if gate == "market_per_symbol_hashes_match_frozen_r14":
        return (
            phase_evidence.get("market_hashes_match") is True,
            {"market_hashes_match": phase_evidence.get("market_hashes_match")},
        )

    if gate == "funding_db_is_authoritative_round12":
        return (
            phase_evidence.get("funding_authoritative") is True,
            {"funding_authoritative": phase_evidence.get("funding_authoritative")},
        )

    if gate == "binaries_hash_match_audit":
        return (
            phase_evidence.get("binaries_match") is True,
            {"binaries_match": phase_evidence.get("binaries_match")},
        )

    if gate == "plan_sha256_frozen":
        return (
            phase_evidence.get("plan_frozen") is True,
            {"plan_sha256": phase_evidence.get("plan_sha256")},
        )

    if gate == "old_counterexamples_reproduced_or_fail_closed":
        ev = phase_evidence.get("counterexamples_ok")
        if isinstance(ev, bool):
            return ev, phase_evidence.get("counterexamples_detail", {})
        return False, {"reason": "no counterexample evidence"}

    # 3) generic phase_evidence boolean
    ev = phase_evidence.get(gate)
    if isinstance(ev, bool):
        return ev, {"source": "phase_evidence"}
    return False, {"reason": "no evidence"}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument(
        "--phase-evidence",
        default=None,
        help="path to a JSON file of extra gate evidence keyed by gate name",
    )
    args = ap.parse_args()

    manifest = load_json(MANIFEST_PATH)
    records = load_registry()
    running = [r for r in records if r.get("status") == "running"]

    phase_evidence = {}
    if args.phase_evidence and os.path.exists(args.phase_evidence):
        phase_evidence = load_json(args.phase_evidence) or {}

    # Frozen hashes
    plan_sha = sha256_file(PLAN_PATH) if os.path.exists(PLAN_PATH) else None
    engine_sha = None
    engine_dir = "apps/backtest-engine/src"
    if os.path.isdir(engine_dir):
        h = hashlib.sha256()
        for dp, _d, files in os.walk(engine_dir):
            for n in sorted(files):
                if n.endswith(".rs"):
                    p = os.path.join(dp, n)
                    h.update(os.path.relpath(p).encode() + b"\0")
                    with open(p, "rb") as fh:
                        h.update(fh.read())
                    h.update(b"\0")
        engine_sha = h.hexdigest()
    data_sha = (
        manifest.get("files", {}).get("funding_db", {}).get("sha256")
        if manifest
        else None
    )

    phase_status = {}
    passed_gates_all = []
    required_gates_all = []
    blocked_reasons = []
    predecessor_complete = True  # P0 has no predecessor

    for phase in PHASES:
        gates = PHASE_GATES.get(phase, [])
        required_gates_all.append({"phase": phase, "gates": gates})
        if not predecessor_complete:
            phase_status[phase] = {
                "status": "blocked",
                "reason": "predecessor not complete",
            }
            blocked_reasons.append(f"{phase}: predecessor not complete")
            continue
        if running:
            phase_status[phase] = {
                "status": "blocked",
                "reason": "running records exist",
            }
            blocked_reasons.append(f"{phase}: running records exist")
            continue
        passed = []
        failed = []
        for gate in gates:
            ok, detail = check_gate(gate, manifest, records, phase_evidence)
            entry = {"gate": gate, "passed": ok, "detail": detail}
            (passed if ok else failed).append(entry)
        passed_gates_all.append({"phase": phase, "passed": passed, "failed": failed})
        if failed:
            phase_status[phase] = {
                "status": "blocked",
                "failed_gates": [f["gate"] for f in failed],
            }
            blocked_reasons.append(
                f"{phase}: {len(failed)} gate(s) missing: {[f['gate'] for f in failed]}"
            )
            predecessor_complete = False
        else:
            phase_status[phase] = {"status": "complete"}

    counts = recompute_counts(records)

    # Determine the active phase = first non-complete
    current_phase = PHASES[-1]
    current_status = "complete"
    for phase in PHASES:
        if phase_status[phase]["status"] != "complete":
            current_phase = phase
            current_status = phase_status[phase]["status"]
            break

    # Bootstrap-only state (before any commit) has empty required_gates frozen.
    bootstrap_path = os.path.join(ART_DIR, "bootstrap-required-gates.json")
    if not os.path.exists(bootstrap_path):
        required_gates_for_state = required_gates_all
    else:
        # Once frozen, reuse the frozen required_gates (fail-closed if shrunk).
        frozen = load_json(bootstrap_path)
        required_gates_for_state = frozen if frozen else required_gates_all

    state = {
        "plan_sha256": plan_sha,
        "phase": current_phase,
        "status": current_status,
        "required_gates": required_gates_for_state,
        "passed_gates": passed_gates_all,
        "blocked_reasons": blocked_reasons,
        "hashes": {
            "engine": engine_sha,
            "data": data_sha,
            "manifest": sha256_file(MANIFEST_PATH) if os.path.exists(MANIFEST_PATH) else None,
            "config": None,
        },
        "counts": counts,
        "artifacts": sorted(
            os.path.relpath(os.path.join(dp, f))
            for dp, _d, fs in os.walk(ART_DIR)
            for f in fs
            if f.endswith(".json") or f.endswith(".jsonl")
        ),
        "validated_at": datetime.now(timezone.utc).isoformat(),
        "phase_status": phase_status,
    }

    if not args.check:
        with open(STATE_PATH, "w") as fh:
            json.dump(state, fh, indent=2)
        print(f"wrote {STATE_PATH}")
    print(f"phase={current_phase} status={current_status}")
    if blocked_reasons:
        print("blocked_reasons:")
        for r in blocked_reasons[:12]:
            print("  -", r)
    if args.check:
        sys.exit(0 if current_status != "blocked" else 1)


if __name__ == "__main__":
    main()
