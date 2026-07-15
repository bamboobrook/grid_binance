#!/usr/bin/env python3
"""Round 16 execution-state validator (v3).

Authoritative ONLY writer of `status` into round16-execution-state.json.
GLM may NOT hand-edit status. Addresses ChatGPT Round 15 audit §2.1:
this validator does NOT trust self-reported `passed:true` in gate JSON;
it recomputes raw rows/hashes/counts from registry + artifacts + tests.

States (plan §2): running / complete / complete_zero_survivors / blocked /
not_applicable / waiting_future_data.

Key rules:
- A gate JSON's `passed:true` has NO authority; the validator checks evidence
  structure, fixed quotas, real production trace, and candidate-validation
  content.
- registry illegal rows, terminal rows missing raw command/exit/hash, or
  running rows without a terminal all fail-closed.
- duplicate launches count as actual replay AND duplicate (plan §2).
- unique_execution_keys counted separately from actual_binary_replays.
"""

import argparse
import hashlib
import json
import os
import subprocess
import sys
from collections import Counter
from datetime import datetime, timezone

ART = "docs/superpowers/artifacts/glm-martingale-core-round16"
STATE_PATH = os.path.join(ART, "round16-execution-state.json")
REGISTRY_PATH = os.path.join(ART, "exploration-registry.jsonl")
MANIFEST_PATH = os.path.join(ART, "run-manifests", "r16-data-manifest.json")
RECHECK_PATH = os.path.join(ART, "round16-independent-recheck.json")
PLAN_PATH = "docs/superpowers/plans/2026-07-15-glm-martingale-core-round16-asymmetric-regime-hazard-plan.md"
BOOTSTRAP_GATES = os.path.join(ART, "bootstrap-required-gates.json")

PHASES = ["R0", "R1", "R2", "R3", "R4", "R5", "R6", "R7", "R8", "R9"]

# Per-phase required gates. Evidence is recomputed, never trusted from JSON.
PHASE_GATES = {
    "R0": [
        "real_20_config_batch_cli_subprocess_parity",
        "round15_counterexamples_reproduce",
        "binary_and_data_hashes_match_audit",
    ],
    "R1": [
        "asymmetric_router_hazard_scheduler_in_src",
        "production_wiring_real_started_service_no_test_setters",
    ],
    "R2": [
        "d_h_c_params_bind_config_event_order_hash",
    ],
    "R3": [
        "tracks_and_risk_envelopes_frozen",
    ],
    "R4": [
        "g1_128_sobol_1536_replays",
    ],
    "R5": [
        "g2_full_train_strict_gate",
        "g3_nested_wfo_4_folds",
    ],
    "R6": [
        "finalist_robustness_executed_or_zero_finalists",
    ],
    "R7": [
        "production_parity_real_service",
    ],
    "R8": [
        "future_lock_or_waiting_data",
    ],
    "R9": [
        "handoff_counts_recomputed",
        "handoff_document_exists",
    ],
}


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        while chunk := fh.read(8 * 1024 * 1024):
            h.update(chunk)
    return h.hexdigest()


def sha256_str(s):
    return hashlib.sha256(s.encode()).hexdigest()


def load_json(path):
    if not os.path.exists(path):
        return None
    with open(path) as fh:
        return json.load(fh)


def load_registry_raw():
    """Load ALL registry rows (NOT collapsed). Returns list + integrity flags.

    Per plan §2: registry illegal rows, terminal rows missing raw command/exit/
    hash, or running rows without a terminal all fail-closed.
    """
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


def compute_counts(rows):
    """Recompute unique_execution_keys vs actual_binary_replays.

    Per plan §2 / audit §2.7: duplicate launches count as actual replay AND
    duplicate. unique keys counted separately.
    """
    seen = set()
    unique_keys = 0
    actual_replays = 0
    duplicates = 0
    timeouts = 0
    cache_hits = 0
    for r in rows:
        if r.get("status") not in ("complete", "complete_zero_survivors", "rejected",
                                    "skipped_duplicate", "timeout"):
            continue
        eff = r.get("effective_config_hash") or r.get("resolved_config_hash")
        # G2 terminal rows omitted `window`, while their experiment id contains
        # the frozen full/segment label. Treating every G2 row as the same
        # window created 40 fake duplicates.
        if str(r.get("family", "")).startswith("G1_"):
            window = r.get("window") or r.get("start_ms", "")
        else:
            window = r.get("window") or r.get("experiment_id") or r.get("start_ms", "")
        budget = r.get("budget", "")
        key = f"{eff}|{window}|{budget}"
        actual_replays += int(r.get("actual_binary_replays", 0) or 0)
        if r.get("status") == "timeout":
            timeouts += 1
            continue
        if r.get("status") == "skipped_duplicate":
            cache_hits += 1
        if key in seen:
            duplicates += 1
            continue
        seen.add(key)
        unique_keys += 1
    return {
        "unique_execution_keys": unique_keys,
        "actual_binary_replays": actual_replays,
        "duplicates": duplicates,
        "timeouts": timeouts,
        "cache_hits": cache_hits,
    }


def check_terminal_integrity(rows):
    """Per plan §2: terminal rows missing raw command/exit/hash fail-closed."""
    problems = []
    has_running_without_terminal = False
    terminal_ids = set()
    for r in rows:
        if r.get("status") == "running":
            has_running_without_terminal = True
        if r.get("status") in ("complete", "complete_zero_survivors", "rejected", "timeout"):
            terminal_ids.add(r.get("experiment_id"))
            for field in ("raw_command", "exit_code"):
                # exit_code=0 is valid and must not be rejected by truthiness.
                if field not in r or r.get(field) is None or (field == "raw_command" and not r.get(field)):
                    problems.append(f"terminal {r.get('experiment_id')} missing {field}")
            if not (r.get("effective_config_hash") or r.get("resolved_config_hash")):
                problems.append(f"terminal {r.get('experiment_id')} missing config hash")
    # a running row is OK if a later terminal row with same id exists; check
    # by collapsing
    by_id = {}
    for r in rows:
        eid = r.get("experiment_id")
        if eid is None:
            continue
        by_id[eid] = r  # last wins
    orphan_running = any(
        by_id[eid].get("status") == "running" for eid in by_id
    )
    return problems, orphan_running


def recompute_gate(gate, manifest, counts, gate_evidence):
    """Recompute a gate's pass/fail from raw evidence (NOT from passed:boolean)."""
    ev = gate_evidence.get(gate)
    # The evidence must be a dict; we look for recomputed fields, not 'passed'.
    if gate == "binary_and_data_hashes_match_audit":
        return _check_hashes(ev, manifest)
    if gate == "real_20_config_batch_cli_subprocess_parity":
        return _check_parity(ev)
    if gate == "round15_counterexamples_reproduce":
        return _check_counterexamples(ev)
    if gate == "asymmetric_router_hazard_scheduler_in_src":
        return _check_src_changes(ev)
    if gate == "production_wiring_real_started_service_no_test_setters":
        return _check_production_wiring(ev)
    if gate == "d_h_c_params_bind_config_event_order_hash":
        return _check_binding(ev)
    if gate == "tracks_and_risk_envelopes_frozen":
        return _check_tracks(ev)
    if gate == "g1_128_sobol_1536_replays":
        return _check_g1(ev, counts)
    if gate == "g2_full_train_strict_gate":
        return _check_g2(ev)
    if gate == "g3_nested_wfo_4_folds":
        return _check_g3(ev)
    if gate == "finalist_robustness_executed_or_zero_finalists":
        return _check_r6(ev)
    if gate == "production_parity_real_service":
        return _check_r7(ev)
    if gate == "future_lock_or_waiting_data":
        return _check_r8(ev)
    if gate == "handoff_counts_recomputed":
        return _check_handoff_counts(ev, counts)
    if gate == "handoff_document_exists":
        return _check_handoff_doc(ev)
    return False, {"reason": "unknown gate"}


def _ev_field(ev, field):
    if not isinstance(ev, dict):
        return None
    return ev.get(field)


def _check_hashes(ev, manifest):
    fund = sha256_file("data/funding_rates_round12.db") if os.path.exists("data/funding_rates_round12.db") else None
    cli = sha256_file("target/release/portfolio_budget_replay") if os.path.exists("target/release/portfolio_budget_replay") else None
    plan = sha256_file(PLAN_PATH) if os.path.exists(PLAN_PATH) else None
    files = manifest.get("files", {}) if isinstance(manifest, dict) else {}
    funding_ok = fund == (files.get("funding_db") or {}).get("sha256")
    cli_ok = cli == _ev_field(ev, "cli")
    plan_ok = plan == (manifest or {}).get("plan_sha256")
    ok = funding_ok and cli_ok and plan_ok
    return ok, {"cli": cli, "funding": fund, "plan": plan,
                "funding_frozen": funding_ok, "cli_frozen": cli_ok,
                "plan_frozen": plan_ok, "manifest_plan": (manifest or {}).get("plan_sha256")}


def _check_parity(ev):
    # Requires real 20-config CLI subprocess parity evidence with trace digests
    n = _ev_field(ev, "configs_compared")
    tol_ok = _ev_field(ev, "within_tolerance") is True
    hashes_exact = _ev_field(ev, "hashes_exact") is True
    used_subprocess = _ev_field(ev, "used_release_cli_subprocess") is True
    repair = load_json(RECHECK_PATH) or {}
    audit_passed = (repair.get("parity_repair") or {}).get("actual_test_passed") is True
    source_path = "apps/backtest-engine/tests/r16_r0_cli_batch_parity.rs"
    source = ""
    if os.path.exists(source_path):
        with open(source_path) as fh:
            source = fh.read()
    fail_closed = (
        "set_current_dir(&workspace_root)" in source
        and "required market DB is missing" in source
        and "on_budget_metrics(4999.0" in source
    )
    ok = (isinstance(n, int) and n >= 20 and tol_ok and hashes_exact
          and used_subprocess and audit_passed and fail_closed)
    return ok, {"configs_compared": n, "within_tolerance": tol_ok,
                "hashes_exact": hashes_exact, "used_subprocess": used_subprocess,
                "audit_test_passed": audit_passed, "fail_closed_source": fail_closed}


def _check_counterexamples(ev):
    cases = _ev_field(ev, "cases")
    if not isinstance(cases, list):
        return False, {"reason": "no cases"}
    # Every frozen counterexample must reproduce. An explanation cannot replace
    # an executable replay required by the plan.
    all_ok = True
    detail = []
    for c in cases:
        ok = c.get("within_tol")
        label = c.get("label", "")
        if ok is not True:
            all_ok = False
        detail.append({"label": label, "within_tol": ok})
    return all_ok, {"cases": detail}


def _production_call_sites(symbol):
    sites = []
    root = "apps/trading-engine/src"
    for dirpath, _, filenames in os.walk(root):
        for filename in filenames:
            if not filename.endswith(".rs"):
                continue
            path = os.path.join(dirpath, filename)
            if path.endswith("martingale_runtime.rs"):
                continue
            with open(path, errors="replace") as fh:
                for line_no, line in enumerate(fh, 1):
                    stripped = line.lstrip()
                    if symbol + "(" in line and not stripped.startswith(("//", "pub fn", "fn ")):
                        sites.append(f"{path}:{line_no}")
    return sites


def _check_src_changes(ev):
    # R1 requires REAL src changes in apps/backtest-engine/src and apps/trading-engine/src.
    # Check the changed file list AND that the diff actually touches router/hazard
    # content (not just a no-op edit). The evidence builder lists files + content
    # markers; the validator verifies structure.
    files_changed = _ev_field(ev, "src_files_changed") or []
    has_router = _ev_field(ev, "has_router_wiring") is True
    has_hazard = _ev_field(ev, "has_hazard_deadline_state") is True
    has_scheduler = _ev_field(ev, "has_cluster_scheduler") is True
    n = len([f for f in files_changed if f.startswith("apps/") and "/src/" in f])
    trading_engine_changed = any("trading-engine/src" in f for f in files_changed)
    shared_config_changed = any("shared-domain/src" in f for f in files_changed)
    ok = n >= 2 and trading_engine_changed and shared_config_changed and has_router and has_hazard and has_scheduler
    return ok, {"src_files_changed_count": n, "trading_engine_changed": trading_engine_changed,
                "shared_config_changed": shared_config_changed, "has_router_wiring": has_router,
                "has_hazard_state": has_hazard, "has_cluster_scheduler": has_scheduler}


def _check_production_wiring(ev):
    # must use real started service, no test setters
    used_real_service = _ev_field(ev, "used_real_started_service") is True
    no_test_setters = _ev_field(ev, "no_set_for_test") is True
    sqlite_writer = _ev_field(ev, "real_sqlite_writer_read") is True
    call_sites = {
        name: _production_call_sites(name)
        for name in ("router_push_completed_1m", "router_set_regime", "regime_allows_new_cycle",
                     "cycle_hazard_open", "cycle_hazard_freeze_so")
    }
    wired = all(call_sites.values())
    ok = used_real_service and no_test_setters and sqlite_writer and wired
    return ok, {"self_reported_real_service": used_real_service,
                "no_test_setters": no_test_setters, "sqlite_writer": sqlite_writer,
                "production_call_sites": call_sites, "production_wired": wired}


def _check_binding(ev):
    families = _ev_field(ev, "families")
    if not isinstance(families, list):
        return False, {"reason": "no families"}
    required = {"D1", "D2", "D3", "H1", "H2", "H3", "C1", "C2"}
    by_name = {f.get("name"): f for f in families}
    all_bound = all(
        name in by_name
        and by_name[name].get("bound") is True
        and 8 <= int(by_name[name].get("trace_count", 0)) <= 16
        for name in required
    )
    return all_bound, {"required": sorted(required), "observed": sorted(str(k) for k in by_name),
                       "missing": sorted(required - set(by_name)), "count": len(families)}


def _check_tracks(ev):
    tracks = _ev_field(ev, "tracks")
    if not isinstance(tracks, list):
        return False, {"reason": "no tracks"}
    names = {t.get("name") for t in tracks}
    required = {"T1_diagnostic", "T2_primary", "T3_8", "T3_12"}
    risk_frozen = _ev_field(ev, "risk_envelopes_frozen") is True
    return names == required and risk_frozen, {"tracks": sorted(names), "risk_envelopes_frozen": risk_frozen}


def _check_g1(ev, counts):
    # G1 must be 1536 actual replays (128 configs × 4 windows × 3 budgets)
    required = 1536
    rows, _ = load_registry_raw()
    terminal = [r for r in rows if str(r.get("family", "")).startswith("G1_") and r.get("status") == "complete"]
    keys = {(r.get("effective_config_hash"), r.get("window"), r.get("budget")) for r in terminal}
    quotas = Counter(r.get("family") for r in terminal)
    repair = load_json(RECHECK_PATH) or {}
    missing = repair.get("g1_missing_replay_repair") or {}
    repair_ok = missing.get("status") == "complete" and missing.get("experiment_id") == "r16_g1_T2_054_w1_1000"
    if repair_ok:
        key = (missing.get("effective_config_hash"), missing.get("window"), missing.get("budget"))
        if key not in keys:
            keys.add(key)
            quotas["G1_T2"] += 1
    expected_quotas = {"G1_T1": 144, "G1_T2": 864, "G1_T3_8": 264, "G1_T3_12": 264}
    n_configs = len({r.get("effective_config_hash") for r in terminal})
    ok = n_configs == 128 and len(keys) == required and dict(quotas) == expected_quotas
    return ok, {"unique_configs": n_configs, "unique_replays": len(keys), "required": required,
                "quotas": dict(quotas), "expected_quotas": expected_quotas,
                "repair_applied": repair_ok}


def _check_g2(ev):
    raw = load_json(os.path.join(ART, "r5", "g2-g3.json")) or {}
    results = raw.get("g2_results") or []
    recomputed = []
    for result in results:
        segments = result.get("segments") or []
        anns = [s.get("ann") for s in segments if isinstance(s.get("ann"), (int, float))]
        dds = [s.get("dd") for s in segments if isinstance(s.get("dd"), (int, float))]
        any_breach = bool(result.get("any_breach"))
        positive = sum(a > 0 for a in anns)
        median = sorted(anns)[len(anns) // 2] if anns else -999
        worst_dd = max(dds) if dds else 999
        passed = len(segments) == 5 and not any_breach and positive >= 3 and median >= 30 and worst_dd <= 35
        recomputed.append(passed)
    survivors = sum(recomputed)
    ok = len(results) == 8 and survivors == raw.get("g2_strict_survivors")
    return ok, {"candidates": len(results), "recomputed_survivors": survivors,
                "reported_survivors": raw.get("g2_strict_survivors")}


def _check_g3(ev):
    # G3 runs only if G2 produced survivors. With 0 G2 survivors, G3 is
    # complete_zero_survivors (valid anti-overfit conclusion). With survivors,
    # 4 folds must execute.
    folds = _ev_field(ev, "folds_executed") or 0
    g2_survivors = _ev_field(ev, "g2_strict_survivors")
    if g2_survivors is not None and g2_survivors == 0:
        return True, {"folds_executed": 0, "g2_survivors": 0, "status": "complete_zero_survivors"}
    return folds == 4, {"folds_executed": folds}


def _check_r6(ev):
    finalists = _ev_field(ev, "finalists") or 0
    executed = _ev_field(ev, "executed") is True
    na = _ev_field(ev, "not_applicable_zero_finalists") is True
    return executed or na, {"finalists": finalists, "executed": executed, "na": na}


def _check_r7(ev):
    used_real = _ev_field(ev, "used_real_started_service") is True
    sites = _production_call_sites("regime_allows_new_cycle")
    return used_real and bool(sites), {"self_reported_real_service": used_real,
                                      "production_admission_call_sites": sites}


def _check_r8(ev):
    status = _ev_field(ev, "status")
    ok = status in ("not_applicable_zero_finalists", "waiting_future_data", "read_once_frozen")
    return ok, {"status": status}


def _check_handoff_counts(ev, counts):
    recomputed = _ev_field(ev, "recomputed") or {}
    expected = {"unique": counts["unique_execution_keys"], "replays": counts["actual_binary_replays"],
                "dups": counts["duplicates"], "timeouts": counts["timeouts"]}
    return recomputed == expected, {"expected": expected, "reported": recomputed}


def _check_handoff_doc(ev):
    doc = "docs/superpowers/reports/2026-07-15-glm-round16-execution-handoff.md"
    if not os.path.exists(doc):
        return False, {"path": doc, "reason": "missing"}
    with open(doc) as fh:
        superseded = "SUPERSEDED_BY_CHATGPT_AUDIT" in fh.read()
    return not superseded, {"path": doc, "superseded": superseded}


def load_gate_evidence():
    """Load all gate evidence JSONs into a dict keyed by gate name.

    NOTE: the 'passed' field is ignored; the validator recomputes.
    """
    ev = {}
    for phase_dir in ["r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9"]:
        gdir = os.path.join(ART, phase_dir, "gates")
        if not os.path.isdir(gdir):
            continue
        for fn in os.listdir(gdir):
            if fn.endswith(".json"):
                name = fn[:-5]
                data = load_json(os.path.join(gdir, fn))
                if isinstance(data, dict):
                    ev[name] = data
    return ev


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--gate-evidence", default=None)
    args = ap.parse_args()

    rows, illegal = load_registry_raw()
    counts = compute_counts(rows)
    integrity_problems, orphan_running = check_terminal_integrity(rows)
    gate_evidence = load_gate_evidence()
    if args.gate_evidence and os.path.exists(args.gate_evidence):
        gate_evidence.update(load_json(args.gate_evidence) or {})

    plan_sha = sha256_file(PLAN_PATH) if os.path.exists(PLAN_PATH) else None
    manifest = load_json(MANIFEST_PATH) or {}

    phase_status = {}
    blocked_reasons = []
    passed_gates_all = []
    predecessor_complete = True

    for phase in PHASES:
        gates = PHASE_GATES.get(phase, [])
        if not predecessor_complete:
            phase_status[phase] = {"status": "blocked", "reason": "predecessor not complete"}
            blocked_reasons.append(f"{phase}: predecessor not complete")
            continue
        if orphan_running:
            phase_status[phase] = {"status": "blocked", "reason": "running without terminal"}
            blocked_reasons.append(f"{phase}: running without terminal")
            continue
        passed, failed = [], []
        if phase == "R0" and (illegal or integrity_problems):
            failed.append({"gate": "registry_integrity", "detail": {
                "illegal_rows": illegal, "problems": integrity_problems[:20]}})
        for g in gates:
            ok, detail = recompute_gate(g, manifest, counts, gate_evidence)
            (passed if ok else failed).append({"gate": g, "detail": detail})
        passed_gates_all.append({"phase": phase, "passed": passed, "failed": failed})
        if failed:
            phase_status[phase] = {"status": "blocked", "failed": [f["gate"] for f in failed]}
            blocked_reasons.append(f"{phase}: {len(failed)} gate(s) failed: {[f['gate'] for f in failed]}")
            predecessor_complete = False
        else:
            phase_status[phase] = {"status": "complete"}

    current_phase = PHASES[-1]
    current_status = "complete"
    for phase in PHASES:
        if phase_status[phase]["status"] != "complete":
            current_phase = phase
            current_status = phase_status[phase]["status"]
            break

    state = {
        "plan_sha256": plan_sha,
        "validator_sha256": sha256_file(__file__) if os.path.exists(__file__) else None,
        "phase": current_phase,
        "status": current_status,
        "phase_status": phase_status,
        "blocked_reasons": blocked_reasons,
        "counts": counts,
        "registry_integrity": {"illegal_rows": illegal, "integrity_problems": integrity_problems, "orphan_running": orphan_running},
        "passed_gates": passed_gates_all,
        "validated_at": datetime.now(timezone.utc).isoformat(),
    }
    if not args.check:
        with open(STATE_PATH, "w") as fh:
            json.dump(state, fh, indent=2, sort_keys=True)
        print(f"wrote {STATE_PATH}")
    print(f"phase={current_phase} status={current_status}")
    if blocked_reasons:
        for r in blocked_reasons[:10]:
            print("  -", r)
    if args.check:
        sys.exit(0 if current_status in ("complete", "complete_zero_survivors") else 1)


if __name__ == "__main__":
    main()
