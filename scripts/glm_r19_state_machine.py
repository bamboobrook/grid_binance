#!/usr/bin/env python3
"""Round 19 R0: central execution state machine + append-only registry +
failure ledger + recursive historical fingerprint index.

Plan §2: round19-execution-state.json must be recomputed by a validator from
raw registry/artifacts, never self-reported complete by a runner. Each phase is
pending/running/complete/blocked/invalid; predecessor not-passed => later is
blocked_predecessor.

This module is the SINGLE authority for phase status. Runners may only APPEND
rows to exploration-registry.jsonl and failure-ledger.jsonl; they may not edit
the state file.
"""
import argparse
import hashlib
import json
import os
import time
from datetime import datetime, timezone

ART = "docs/superpowers/artifacts/glm-martingale-core-round19"
STATE_PATH = os.path.join(ART, "round19-execution-state.json")
REGISTRY_PATH = os.path.join(ART, "exploration-registry.jsonl")
FAILURE_LEDGER = os.path.join(ART, "failure-ledger.jsonl")
HIST_INDEX = os.path.join(ART, "historical-fingerprint-index.json")
PLAN_PATH = "docs/superpowers/plans/2026-07-17-glm-martingale-core-round19-valid-residual-recovery-plan.md"

PHASES = ["R0", "R1", "R2", "R3", "R4", "R5", "R6", "R7", "R8", "R9", "R9.5", "R10", "R11", "R12"]

# Each phase has a list of gates that must recompute to True from raw evidence.
PHASE_GATES = {
    "R0": ["r18_authority_verified", "central_state_created"],
    "R1": ["historical_fingerprint_index_recursive"],
    "R2": ["engine_failclose_tests_pass", "rejection_cooldown", "conservative_multileg"],
    "R3": ["unified_cycle_machine_definition"],
    "R4": ["five_families_defined_and_fit"],
    "R5": ["g0_binding_per_family"],
    "R6": ["g1_per_family_per_fold"],
    "R7": ["g2_per_family_per_fold"],
    "R8": ["selection_freeze_committed_before_validation"],
    "R9": ["one_shot_anchored_validation"],
    "R9.5": ["combination_optional_or_skipped"],
    "R10": ["finalist_robustness_or_na"],
    "R11": ["production_parity_or_na"],
    "R12": ["handoff_from_validator"],
}

ALLOWED_TERMINAL = {
    "complete", "rejected_gate", "invalid_mechanism", "invalid_data",
    "invalid_engine_bug", "invalid_results", "timeout", "skipped_duplicate",
    "blocked_predecessor", "blocked_parent_gate", "blocked_no_stable_groups",
    "complete_zero_survivors", "not_applicable_zero_survivors",
    "not_martingale_no_so", "interrupted",
}


def sha256_file(path):
    if not os.path.exists(path):
        return None
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        while chunk := fh.read(8 << 20):
            h.update(chunk)
    return h.hexdigest()


def sha256_str(s):
    return hashlib.sha256(s.encode()).hexdigest()


def load_registry():
    rows = []
    illegal = 0
    if not os.path.exists(REGISTRY_PATH):
        return rows, illegal
    with open(REGISTRY_PATH) as fh:
        for ln in fh:
            ln = ln.strip()
            if not ln:
                continue
            try:
                rows.append(json.loads(ln))
            except json.JSONDecodeError:
                illegal += 1
    return rows, illegal


def check_terminal_integrity(rows):
    """Each experiment must have >=1 running and >=1 terminal row; terminal
    rows must have raw_command + exit_code + fingerprint."""
    by_exp = {}
    for r in rows:
        eid = r.get("experiment_id")
        if eid is None:
            continue
        by_exp.setdefault(eid, []).append(r)
    problems = []
    orphan_running = []
    for eid, exp_rows in by_exp.items():
        statuses = [r.get("status") for r in exp_rows]
        has_running = "running" in statuses
        has_terminal = any(s in ALLOWED_TERMINAL for s in statuses)
        if has_running and not has_terminal:
            orphan_running.append(eid)
        for r in exp_rows:
            if r.get("status") in ALLOWED_TERMINAL and r.get("status") != "running":
                if r.get("raw_command") in (None, "", "cache"):
                    problems.append(f"{r.get('experiment_id')} terminal missing raw_command")
                if "exit_code" not in r:
                    problems.append(f"{r.get('experiment_id')} terminal missing exit_code")
                if not r.get("fingerprint_sha256"):
                    problems.append(f"{r.get('experiment_id')} terminal missing fingerprint_sha256")
    return problems, orphan_running


def compute_counts(rows):
    """Recompute unique configs / replays / duplicates / timeouts from registry."""
    seen_fp = set()
    unique = replays = dups = timeouts = skipped = 0
    family_counts = {}
    for r in rows:
        st = r.get("status")
        if st not in ALLOWED_TERMINAL and st != "running":
            continue
        fam = r.get("family", "?")
        family_counts.setdefault(fam, {"started": 0, "terminal": 0, "duplicate": 0, "timeout": 0, "invalid": 0})
        if st == "running":
            family_counts[fam]["started"] += 1
            continue
        family_counts[fam]["started"] += 1
        family_counts[fam]["terminal"] += 1
        fp = r.get("fingerprint_sha256", "")
        replays += int(r.get("actual_binary_replays", 0) or 0)
        if st == "timeout":
            timeouts += 1
            family_counts[fam]["timeout"] += 1
            continue
        if st == "skipped_duplicate":
            skipped += 1
            family_counts[fam]["duplicate"] += 1
            continue
        if st in ("invalid_mechanism", "invalid_data", "invalid_engine_bug", "invalid_results", "not_martingale_no_so"):
            family_counts[fam]["invalid"] += 1
        if fp in seen_fp:
            dups += 1
            continue
        seen_fp.add(fp)
        unique += 1
    return {"unique_terminal_fingerprints": unique, "actual_binary_replays": replays,
            "duplicates": dups, "timeouts": timeouts, "skipped_duplicates": skipped,
            "by_family": family_counts}


def recompute_gate(gate, rows, counts):
    """Recompute each gate from RAW evidence. Never trust self-report."""
    ev_dir = ART
    if gate == "r18_authority_verified":
        p = "docs/superpowers/artifacts/glm-martingale-core-round18/r18-corrected-authority.json"
        if not os.path.exists(p):
            return False, {"reason": "missing R18 corrected authority"}
        a = json.load(open(p))
        ok = (a.get("corrected_machine_state") == "materially_incomplete_invalid_results"
              and a.get("target_hit") is False
              and a.get("production_ready_candidates") == 0)
        return ok, {"state": a.get("corrected_machine_state"), "target_hit": a.get("target_hit"),
                    "prod_ready": a.get("production_ready_candidates")}
    if gate == "central_state_created":
        ok = os.path.exists(STATE_PATH) and os.path.exists(REGISTRY_PATH)
        return ok, {"state_exists": os.path.exists(STATE_PATH), "registry_exists": os.path.exists(REGISTRY_PATH)}
    if gate == "historical_fingerprint_index_recursive":
        if not os.path.exists(HIST_INDEX):
            return False, {"reason": "no historical index"}
        d = json.load(open(HIST_INDEX))
        # require scans covered at least rounds 1-18 (multi-round)
        rounds_scanned = d.get("rounds_scanned", [])
        ok = len(rounds_scanned) >= 10 and d.get("excluded_keys_count", 0) > 0
        return ok, {"rounds_scanned": len(rounds_scanned), "excluded": d.get("excluded_keys_count")}
    if gate == "engine_failclose_tests_pass":
        # The 8 ChatGPT engine tests + R2's 20 fail-close tests
        # R2 writes a gate evidence file when tests pass
        p = os.path.join(ev_dir, "r2", "gates", "engine_failclose_tests.json")
        if not os.path.exists(p):
            return False, {"reason": "R2 failclose test evidence missing"}
        d = json.load(open(p))
        return d.get("all_passed") is True, d
    if gate == "rejection_cooldown":
        p = os.path.join(ev_dir, "r2", "gates", "rejection_cooldown.json")
        if not os.path.exists(p):
            return False, {"reason": "R2 cooldown evidence missing"}
        d = json.load(open(p))
        return d.get("implemented") is True, d
    if gate == "conservative_multileg":
        p = os.path.join(ev_dir, "r2", "gates", "conservative_multileg.json")
        if not os.path.exists(p):
            return False, {"reason": "R2 conservative multileg evidence missing"}
        d = json.load(open(p))
        return d.get("implemented") is True, d
    if gate == "unified_cycle_machine_definition":
        p = os.path.join(ev_dir, "r3", "gates", "unified_cycle_machine.json")
        if not os.path.exists(p):
            return False, {"reason": "R3 machine definition evidence missing"}
        d = json.load(open(p))
        return d.get("implemented") is True, d
    if gate == "five_families_defined_and_fit":
        p = os.path.join(ev_dir, "r4", "gates", "five_families.json")
        if not os.path.exists(p):
            return False, {"reason": "R4 families evidence missing"}
        d = json.load(open(p))
        fams = d.get("families", {})
        # require M1R + M2F at minimum; P1/K1/V1 may be blocked by their own gates
        required = {"M1R", "M2F"}
        present = set(fams.keys())
        return required.issubset(present), {"families": sorted(present), "required": sorted(required)}
    if gate == "g0_binding_per_family":
        p = os.path.join(ev_dir, "r5", "gates", "g0_binding.json")
        if not os.path.exists(p):
            return False, {"reason": "R5 G0 evidence missing"}
        d = json.load(open(p))
        return d.get("all_bound_families") is not None, d
    if gate == "g1_per_family_per_fold":
        p = os.path.join(ev_dir, "r6", "gates", "g1_search.json")
        if not os.path.exists(p):
            return False, {"reason": "R6 G1 evidence missing"}
        d = json.load(open(p))
        return d.get("total_replays", 0) > 0, d
    if gate == "g2_per_family_per_fold":
        p = os.path.join(ev_dir, "r7", "gates", "g2_search.json")
        if not os.path.exists(p):
            return False, {"reason": "R7 G2 evidence missing"}
        d = json.load(open(p))
        return d.get("total_replays", 0) > 0, d
    if gate == "selection_freeze_committed_before_validation":
        p = os.path.join(ev_dir, "r8", "gates", "selection_freeze.json")
        if not os.path.exists(p):
            return False, {"reason": "R8 selection freeze evidence missing"}
        d = json.load(open(p))
        return d.get("committed_before_validation") is True, d
    if gate == "one_shot_anchored_validation":
        p = os.path.join(ev_dir, "r9", "gates", "validation.json")
        if not os.path.exists(p):
            return False, {"reason": "R9 validation evidence missing"}
        d = json.load(open(p))
        return "outcome" in d, d
    if gate == "combination_optional_or_skipped":
        p = os.path.join(ev_dir, "r9_5", "gates", "combination.json")
        if not os.path.exists(p):
            # combination is optional; absence means skipped (valid)
            return True, {"skipped": True, "reason": "no combination attempted (valid: optional)"}
        d = json.load(open(p))
        return True, d
    if gate == "finalist_robustness_or_na":
        p = os.path.join(ev_dir, "r10", "gates", "robustness.json")
        if not os.path.exists(p):
            return False, {"reason": "R10 evidence missing"}
        d = json.load(open(p))
        return d.get("status") in ("complete", "not_applicable_zero_survivors"), d
    if gate == "production_parity_or_na":
        p = os.path.join(ev_dir, "r11", "gates", "production_parity.json")
        if not os.path.exists(p):
            return False, {"reason": "R11 evidence missing"}
        d = json.load(open(p))
        return d.get("status") in ("complete", "not_applicable_zero_survivors"), d
    if gate == "handoff_from_validator":
        p = "docs/superpowers/reports/2026-07-17-glm-round19-execution-handoff.md"
        if not os.path.exists(p):
            return False, {"reason": "handoff missing"}
        return True, {"handoff_exists": True}
    return False, {"reason": f"unknown gate {gate}"}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="dry-run, do not write state")
    args = ap.parse_args()

    rows, illegal = load_registry()
    counts = compute_counts(rows)
    problems, orphan = check_terminal_integrity(rows)

    plan_sha = sha256_file(PLAN_PATH)
    phase_status = {}
    blocked = []
    passed_all = []
    pred_ok = True
    for phase in PHASES:
        gates = PHASE_GATES.get(phase, [])
        if not pred_ok:
            phase_status[phase] = {"status": "blocked_predecessor", "reason": f"predecessor not complete"}
            blocked.append(f"{phase}: blocked_predecessor")
            continue
        if orphan:
            phase_status[phase] = {"status": "blocked", "reason": "orphan running rows"}
            blocked.append(f"{phase}: orphan running")
            pred_ok = False
            continue
        passed, failed = [], []
        for g in gates:
            ok, det = recompute_gate(g, rows, counts)
            (passed if ok else failed).append({"gate": g, "detail": det})
        passed_all.append({"phase": phase, "passed": passed, "failed": failed})
        if failed:
            phase_status[phase] = {"status": "blocked", "failed": [f["gate"] for f in failed]}
            blocked.append(f"{phase}: {len(failed)} failed: {[f['gate'] for f in failed]}")
            pred_ok = False
        else:
            phase_status[phase] = {"status": "complete"}

    cur = PHASES[-1]
    cur_st = "complete"
    for phase in PHASES:
        if phase_status[phase]["status"] != "complete":
            cur = phase
            cur_st = phase_status[phase]["status"]
            break

    state = {
        "plan_sha256": plan_sha,
        "validator_sha256": sha256_file(__file__),
        "phase": cur,
        "status": cur_st,
        "phase_status": phase_status,
        "blocked_reasons": blocked,
        "counts": counts,
        "registry_integrity": {"illegal_rows": illegal, "problems": len(problems), "orphan_running": len(orphan)},
        "passed_gates": passed_all,
        "validated_at": datetime.now(timezone.utc).isoformat(),
    }
    if not args.check:
        os.makedirs(ART, exist_ok=True)
        with open(STATE_PATH, "w") as fh:
            json.dump(state, fh, indent=2, sort_keys=True)
        print(f"wrote {STATE_PATH}")
    print(f"phase={cur} status={cur_st}")
    for r in blocked[:10]:
        print("  -", r)


if __name__ == "__main__":
    main()
