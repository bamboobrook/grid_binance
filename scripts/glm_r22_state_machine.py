#!/usr/bin/env python3
"""Round 22 R0: state machine validator with 10 canaries.

Plan §2 canaries:
1. dirty commit → invalid
2. short hash → invalid
3. null order hash → invalid
4. reused experiment_id → invalid
5. missing failure row for non-complete terminal → invalid
6. config/argv budget mismatch → invalid
7. G2 without committed G1 parent → blocked
8. 90-day ann entering target → rejected (plan: only stitched >=365d ann)
9. manifest block deleted → blocked
10. cold-start selection after seeing test results → invalid
"""
from __future__ import annotations

import glob
import hashlib
import json
import os
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round22"
STATE_PATH = ART / "round22-execution-state.json"
REGISTRY = ART / "exploration-registry.jsonl"
PLAN = (ROOT / "docs/superpowers/plans/"
        "2026-07-21-glm-martingale-core-round22-prequential-dynamic-multipair-plan.md")

PHASES = ["R0", "R1", "R2", "R3", "R4", "R5", "G0", "G1", "G2", "R8", "HANDOFF"]
PHASE_GATES = {
    "R0": ["r21_corrected_authority_verified", "central_state_created",
           "ten_canaries_pass"],
    "R1": ["engine_conservative_tests_pass"],
    "R2": ["prequential_protocol_frozen"],
    "R3": ["continuous_replay_built"],
    "R4": ["selectors_implemented"],
    "R5": ["execution_enhancement_implemented"],
    "G0": ["g0_binding_per_mechanism", "quota_manifest_frozen"],
    "G1": ["g1_prequential_replay"],
    "G2": ["g2_budget_stress"],
    "R8": ["selection_freeze"],
    "HANDOFF": ["handoff_from_validator"],
}


def sha256_file(path):
    p = Path(path)
    if not p.exists():
        return None
    h = hashlib.sha256()
    with open(p, "rb") as fh:
        while chunk := fh.read(8 << 20):
            h.update(chunk)
    return h.hexdigest()


def load_registry():
    rows = []
    if not REGISTRY.exists():
        return rows
    with open(REGISTRY) as fh:
        for ln in fh:
            ln = ln.strip()
            if not ln:
                continue
            try:
                rows.append(json.loads(ln))
            except json.JSONDecodeError:
                pass
    return rows


def _checkpoint_rows():
    total = 0
    for path in glob.glob(str(ART / "**" / "*checkpoint.json"), recursive=True):
        try:
            data = json.load(open(path))
        except (OSError, json.JSONDecodeError):
            continue
        done = data.get("done", {}) if isinstance(data, dict) else {}
        total += len(done) if isinstance(done, (dict, list)) else 0
    return total


def _local_registry_rows():
    total = 0
    for path in glob.glob(str(ART / "**" / "*.jsonl"), recursive=True):
        if os.path.abspath(path) == os.path.abspath(REGISTRY):
            continue
        try:
            total += sum(1 for line in open(path) if line.strip())
        except OSError:
            continue
    return total


def canary1_dirty_commit(rows):
    for r in rows:
        if r.get("status") not in (None, "running"):
            if r.get("git_dirty") is True:
                return False, {"reason": f"{r.get('experiment_id')}: committed dirty"}
    return True, {"checked": len(rows)}


def canary2_short_hash(rows):
    for r in rows:
        gc = r.get("git_commit")
        if gc and len(gc) < 40:
            return False, {"reason": f"{r.get('experiment_id')}: short hash {gc}"}
    return True, {}


def canary3_null_order_hash(rows):
    for r in rows:
        if r.get("status") == "complete":
            if r.get("trace_order_sha256") is None:
                return False, {"reason": f"{r.get('experiment_id')}: null order hash"}
    return True, {}


def canary4_reused_experiment_id(rows):
    by_exp = {}
    for r in rows:
        eid = r.get("experiment_id")
        if eid:
            by_exp.setdefault(eid, []).append(r)
    for eid, exp_rows in by_exp.items():
        terminals = [r for r in exp_rows if r.get("status")
                     and r.get("status") != "running"]
        if len(terminals) > 1:
            return False, {"reason": f"{eid}: {len(terminals)} terminals"}
    return True, {"experiments": len(by_exp)}


def canary5_missing_failure_row(rows):
    fl_path = ART / "failure-ledger.jsonl"
    fl_experiments = set()
    if fl_path.exists():
        with open(fl_path) as fh:
            for ln in fh:
                ln = ln.strip()
                if not ln:
                    continue
                try:
                    fl_experiments.add(json.loads(ln).get("experiment_id"))
                except json.JSONDecodeError:
                    pass
    for r in rows:
        st = r.get("status")
        if st and st not in ("running", "complete", "skipped_duplicate"):
            if r.get("experiment_id") not in fl_experiments:
                return False, {"reason": f"{r.get('experiment_id')}: missing failure row for {st}"}
    return True, {"failure_ledger_count": len(fl_experiments)}


def canary6_budget_mismatch(rows):
    for r in rows:
        if r.get("status") != "complete":
            continue
        m = r.get("metrics", {}) or {}
        if m.get("budget_mismatch"):
            return False, {"reason": f"{r.get('experiment_id')}: budget mismatch"}
    return True, {}


def canary7_g2_from_g1():
    manifest_path = ART / "g1" / "selected-configs.json"
    if not manifest_path.exists():
        return True, {"reason": "no G2 runs yet"}
    try:
        in_git = subprocess.run(
            ["git", "cat-file", "-e", f"HEAD:{manifest_path.relative_to(ROOT)}"],
            cwd=ROOT, capture_output=True, timeout=10).returncode == 0
    except Exception:
        in_git = False
    if not in_git:
        return False, {"reason": "G1 manifest not committed before G2"}
    return True, {"manifest_committed": in_git}


def canary8_90day_ann_target(rows):
    for r in rows:
        if r.get("status") != "complete":
            continue
        m = r.get("metrics", {}) or {}
        ann = m.get("annualized_return_pct")
        days = r.get("replay_end_ms", 0) - r.get("replay_start_ms", 0)
        days = days // 86_400_000 if days else 0
        if ann and days > 0 and days < 365 and ann >= 50:
            return False, {"reason": f"{r.get('experiment_id')}: {days}d block ann={ann}% in target"}
    return True, {}


def canary9_manifest_block_deleted():
    """Canary passes during bootstrap (before R2 creates the manifest).
    After R2 creates it, deletion is caught."""
    manifest_path = ART / "r2" / "prequential-protocol.json"
    if not manifest_path.exists():
        # Check if R2 gate evidence exists (meaning manifest WAS created)
        r2_evidence = (ART / "r2" / "gates" / "prequential_protocol.json").exists()
        if r2_evidence:
            return False, {"reason": "R2 protocol manifest was deleted after creation"}
        return True, {"reason": "R2 not yet started; manifest not yet created"}
    return True, {"manifest_exists": True}


def canary10_coldstart_selection_after_test():
    sel_path = ART / "r8" / "selected-configs.json"
    if not sel_path.exists():
        return True, {"reason": "no selection yet"}
    sel = json.load(open(sel_path))
    if sel.get("coldstart_selected_after_test"):
        return False, {"reason": "cold starts selected after seeing test results"}
    return True, {}


def run_ten_canaries(rows):
    results = {}
    results["c1_dirty_commit"] = canary1_dirty_commit(rows)
    results["c2_short_hash"] = canary2_short_hash(rows)
    results["c3_null_order_hash"] = canary3_null_order_hash(rows)
    results["c4_reused_experiment_id"] = canary4_reused_experiment_id(rows)
    results["c5_missing_failure_row"] = canary5_missing_failure_row(rows)
    results["c6_budget_mismatch"] = canary6_budget_mismatch(rows)
    results["c7_g2_from_g1"] = canary7_g2_from_g1()
    results["c8_90day_ann_target"] = canary8_90day_ann_target(rows)
    results["c9_manifest_block_deleted"] = canary9_manifest_block_deleted()
    results["c10_coldstart_after_test"] = canary10_coldstart_selection_after_test()
    results["all_passed"] = all(p for p, _ in results.values()
                                if isinstance(p, bool))
    return results


def recompute_gate(gate, rows):
    if gate == "r21_corrected_authority_verified":
        p = (ROOT / "docs/superpowers/artifacts/glm-martingale-core-round21/"
             "round21-corrected-authority.json")
        if not p.exists():
            return False, {"reason": "missing R21 corrected authority"}
        a = json.load(open(p))
        ok = a.get("target_hit") is False
        return ok, {"state": a.get("corrected_machine_state"),
                    "target_hit": a.get("target_hit")}
    if gate == "central_state_created":
        ART.mkdir(parents=True, exist_ok=True)
        if not REGISTRY.exists():
            REGISTRY.touch()
        if not (ART / "failure-ledger.jsonl").exists():
            (ART / "failure-ledger.jsonl").touch()
        return True, {"registry_exists": True}
    if gate == "ten_canaries_pass":
        return run_ten_canaries(rows)["all_passed"], run_ten_canaries(rows)
    if gate == "engine_conservative_tests_pass":
        p = ART / "r1" / "gates" / "engine_conservative_tests.json"
        if not p.exists():
            return False, {"reason": "R1 evidence missing"}
        d = json.load(open(p))
        return d.get("all_passed") is True, d
    if gate == "prequential_protocol_frozen":
        p = ART / "r2" / "prequential-protocol.json"
        if not p.exists():
            return False, {"reason": "R2 protocol missing"}
        d = json.load(open(p))
        in_git = subprocess.run(
            ["git", "cat-file", "-e", f"HEAD:{p.relative_to(ROOT)}"],
            cwd=ROOT, capture_output=True, timeout=10).returncode == 0
        ok = d.get("frozen") is True and in_git
        return ok, {"frozen": d.get("frozen"), "committed": in_git}
    if gate == "continuous_replay_built":
        p = ART / "r3" / "gates" / "continuous_replay.json"
        if not p.exists():
            return False, {"reason": "R3 evidence missing"}
        d = json.load(open(p))
        return d.get("built") is True, d
    if gate == "selectors_implemented":
        p = ART / "r4" / "gates" / "selectors.json"
        if not p.exists():
            return False, {"reason": "R4 evidence missing"}
        d = json.load(open(p))
        return all(d.get("selectors", {}).values()), d
    if gate == "execution_enhancement_implemented":
        p = ART / "r5" / "gates" / "execution_enhancement.json"
        if not p.exists():
            return False, {"reason": "R5 evidence missing"}
        d = json.load(open(p))
        return d.get("e0_implemented") is True, d
    if gate == "g0_binding_per_mechanism":
        p = ART / "g0" / "gates" / "g0_binding.json"
        if not p.exists():
            return False, {"reason": "G0 evidence missing"}
        d = json.load(open(p))
        return all(v.get("all_bound") for v in d.get("mechanisms", {}).values()), d
    if gate == "quota_manifest_frozen":
        p = ART / "g0" / "gates" / "quota_manifest.json"
        if not p.exists():
            return False, {"reason": "quota manifest missing"}
        d = json.load(open(p))
        in_git = subprocess.run(
            ["git", "cat-file", "-e", f"HEAD:{p.relative_to(ROOT)}"],
            cwd=ROOT, capture_output=True, timeout=10).returncode == 0
        ok = d.get("committed") is True and in_git
        return ok, d
    if gate == "g1_prequential_replay":
        p = ART / "g1" / "gates" / "g1.json"
        if not p.exists():
            return False, {"reason": "G1 evidence missing"}
        d = json.load(open(p))
        return d.get("total_replays", 0) > 0, d
    if gate == "g2_budget_stress":
        p = ART / "g2" / "gates" / "g2.json"
        if not p.exists():
            return False, {"reason": "G2 evidence missing"}
        d = json.load(open(p))
        return d.get("total_runs", 0) > 0 or d.get("status") == "not_applicable", d
    if gate == "selection_freeze":
        p = ART / "r8" / "selected-configs.json"
        if not p.exists():
            return False, {"reason": "R8 selection missing"}
        d = json.load(open(p))
        return d.get("committed") is True, d
    if gate == "handoff_from_validator":
        candidates = glob.glob(str(
            ROOT / "docs/superpowers/reports/*round22*handoff*.md"))
        return len(candidates) > 0, {"path": candidates[0] if candidates else None}
    return False, {"reason": f"unknown gate {gate}"}


def main():
    ART.mkdir(parents=True, exist_ok=True)
    if not REGISTRY.exists():
        REGISTRY.touch()
    if not (ART / "failure-ledger.jsonl").exists():
        (ART / "failure-ledger.jsonl").touch()
    rows = load_registry()
    plan_sha = sha256_file(PLAN)
    phase_status = {}
    blocked = []
    pred_ok = True
    for phase in PHASES:
        if not pred_ok:
            phase_status[phase] = {"status": "blocked_predecessor"}
            blocked.append(f"{phase}: blocked_predecessor")
            continue
        passed, failed = [], []
        for g in PHASE_GATES[phase]:
            ok, det = recompute_gate(g, rows)
            (passed if ok else failed).append({"gate": g, "detail": det})
        if failed:
            phase_status[phase] = {"status": "blocked",
                                   "failed": [f["gate"] for f in failed]}
            blocked.append(f"{phase}: {len(failed)} failed: "
                           f"{[f['gate'] for f in failed]}")
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
        "phase": cur, "status": cur_st,
        "phase_status": phase_status,
        "blocked_reasons": blocked,
        "registry_rows": len(rows),
        "validated_at": datetime.now(timezone.utc).isoformat(),
    }
    with open(STATE_PATH, "w") as fh:
        json.dump(state, fh, indent=2, sort_keys=True)
    print(f"wrote {STATE_PATH}")
    print(f"phase={cur} status={cur_st} registry_rows={len(rows)}")
    for r in blocked[:10]:
        print("  -", r)


if __name__ == "__main__":
    main()
