#!/usr/bin/env python3
"""Independently audit Round 20 artifacts and selected replay behavior."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round20"
OUT = ART / "audit/round20-independent-audit.json"
BINARY = ROOT / "target/release/synchronized_cycle_replay"
FOLDS = {
    "F1": (1672531200000, 1688169599999),
    "F2": (1672531200000, 1704067199999),
    "F3": (1672531200000, 1735689599999),
    "F4": (1672531200000, 1767225599999),
}
PLANNED_QUOTA = {"C1": 64, "B1": 96, "M2R": 64, "P1": 64, "K1": 48, "V1": 64}


def load_json(path: Path):
    return json.loads(path.read_text())


def load_jsonl(path: Path) -> list[dict]:
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def checkpoint_rows(path: Path) -> list[tuple[str, dict]]:
    return list(load_json(path).get("done", {}).items())


def registry_audit() -> dict:
    central = load_jsonl(ART / "exploration-registry.jsonl")
    checkpoints = {
        "g1": len(checkpoint_rows(ART / "p6_p7/g1-checkpoint.json")),
        "g2": len(checkpoint_rows(ART / "p8/g2-checkpoint.json")),
        "validation": len(checkpoint_rows(ART / "p9/validation-checkpoint.json")),
    }
    return {
        "central_rows": len(central),
        "central_running_rows": sum(row.get("status") == "running" for row in central),
        "central_terminal_rows": sum(row.get("status") != "running" for row in central),
        "checkpoint_rows": checkpoints,
        "self_reported_full_replays": 3946,
        "contract_pass": bool(central) and any(row.get("status") == "running" for row in central),
        "reason": "checkpoint execution evidence exists while the mandatory append-only central registry is empty",
    }


def quota_audit() -> dict:
    source = (ROOT / "scripts/glm_r20_r6r7_g0_g1.py").read_text()
    actual = {}
    for family in PLANNED_QUOTA:
        match = re.search(rf'"{family}"\s*:\s*\{{.*?"n_configs"\s*:\s*(\d+)', source, re.S)
        actual[family] = int(match.group(1)) if match else None
    return {
        "planned_configs_per_fold": PLANNED_QUOTA,
        "actual_configs_per_fold": actual,
        "completion_ratio": {
            family: (actual[family] / planned if actual[family] is not None else 0.0)
            for family, planned in PLANNED_QUOTA.items()
        },
        "quota_complete": actual == PLANNED_QUOTA,
    }


def family_runtime_audit() -> dict:
    fits = load_json(ART / "p4/families-fit.json")
    p1kv1 = load_json(ART / "p4/p1_k1_v1_fits.json")
    b1 = []
    c1_scheduler_values = []
    for fold in fits["fold_fits"]:
        for fit in fold.get("B1", {}).get("frozen_fits", []):
            b1.append({"fold": fold["fold"], "group_id": fit["group_id"], "legs": fit["legs"]})
        c1_scheduler_values.append(fold.get("C1", {}).get("c1_scheduler"))
    v1_weight_fields = sum(
        "weights" in fit
        for fold in p1kv1["fold_fits"]
        for fit in fold.get("V1", {}).get("frozen_fits", [])
    )
    engine = (ROOT / "apps/backtest-engine/src/martingale/sync_cycle_engine.rs").read_text()
    cli = (ROOT / "apps/backtest-engine/src/bin/synchronized_cycle_replay.rs").read_text()
    exchange = (ROOT / "apps/backtest-engine/src/martingale/exchange_model.rs").read_text()
    runner = (ROOT / "scripts/glm_r20_r6r7_g0_g1.py").read_text()
    runtime_mapping = {
        family: mapped
        for family, mapped in re.findall(
            r'"(C1|B1|M2R|P1|K1|V1)"\s*:\s*\{.*?"base"\s*:\s*\{\s*"family"\s*:\s*"([^"]+)"',
            runner,
            re.S,
        )
    }
    fit_struct = engine.split("pub struct SynchronizedFit", 1)[1].split("}\n", 1)[0]
    return {
        "declared_to_runtime_family": runtime_mapping,
        "distinct_runtime_families": sorted(set(runtime_mapping.values())),
        "p1_k1_online_state_implemented": any(
            token in engine for token in ('family == "P1"', 'family == "K1"', 'match family')
        ),
        "v1_weights_present_as_extra_fit_field_count": v1_weight_fields,
        "v1_weights_consumed_by_engine": "weights" in fit_struct,
        "c1_scheduler_artifact_values": c1_scheduler_values,
        "c1_scheduler_consumed_by_engine": "c1_scheduler" in engine,
        "b1_fit_count": len(b1),
        "b1_duplicate_symbol_leg_count": sum(len(row["legs"]) == 2 and row["legs"][0] == row["legs"][1] for row in b1),
        "b1_examples": b1[:4],
        "cli_deduplicates_market_legs": "BTreeSet<String>" in cli,
        "cli_has_market_type_leg_identity": "market_type" in cli,
        "exchange_module_size_bytes": len(exchange.encode()),
        "exchange_model_consumed_by_sync_engine": "exchange_model" in engine,
        "exchange_model_consumed_by_cli": "exchange_model" in cli,
        "min_liquidation_buffer_unmodeled": "min_liquidation_buffer_pct: None" in engine,
        "family_contract_pass": False,
    }


def causal_blocks(train_start: int, train_end: int, lookback_days: int = 60, count: int = 3):
    initial_fit_end = train_start + lookback_days * 86_400_000
    chunk = (train_end - initial_fit_end) // count
    purge = max(lookback_days * 86_400_000, 8 * 3_600_000)
    output = []
    for index in range(count):
        replay_start = initial_fit_end + index * chunk
        replay_end = initial_fit_end + (index + 1) * chunk if index < count - 1 else train_end
        nominal_fit_end = replay_start - purge
        if nominal_fit_end > train_start and replay_end > replay_start:
            output.append((f"ib{index + 1}", nominal_fit_end, replay_start, replay_end))
    return output


def causality_audit() -> dict:
    fit_files = [load_json(ART / "p4/families-fit.json"), load_json(ART / "p4/p1_k1_v1_fits.json")]
    outer_fit_end = {}
    for document in fit_files:
        for fold in document["fold_fits"]:
            outer_fit_end[fold["fold"]] = int(fold.get("fit_end", fold.get("train_full_end")))
    rows = []
    for fold, (start, end) in FOLDS.items():
        for block, nominal_fit_end, replay_start, replay_end in causal_blocks(start, end):
            used_fit_end = outer_fit_end[fold]
            rows.append({
                "fold": fold,
                "block": block,
                "nominal_causal_fit_end_ms": nominal_fit_end,
                "used_outer_fit_end_ms": used_fit_end,
                "replay_start_ms": replay_start,
                "replay_end_ms": replay_end,
                "causal": used_fit_end < replay_start,
            })
    return {
        "blocks": rows,
        "causal_blocks": sum(row["causal"] for row in rows),
        "leaky_blocks": sum(not row["causal"] for row in rows),
        "runner_refits_before_each_block": False,
    }


def strict_row(row: dict) -> bool:
    return (
        row.get("status") == "complete"
        and not row.get("breach")
        and int(row.get("actual_symbols") or 0) >= 5
        and int(row.get("groups_with_so") or 0) > 0
        and float(row.get("max_symbol_conc") or 101.0) <= 50.0
        and float(row.get("max_group_conc") or 101.0) <= 50.0
    )


def validation_audit() -> dict:
    rows = [row for _, row in checkpoint_rows(ART / "p9/validation-checkpoint.json")]
    eligible = [row for row in rows if strict_row(row)]
    eligible.sort(key=lambda row: -float(row.get("ann") or 0.0))
    push = load_json(ART / "concentrated_push/summary.json")["results"]
    push_eligible = [row for row in push if strict_row({**row, "status": "complete"})]
    push_eligible.sort(key=lambda row: -float(row.get("ann") or 0.0))
    best_reported = max(push, key=lambda row: float(row.get("ann") or -1e9))
    return {
        "p9_rows": len(rows),
        "p9_common_gate_rows": len(eligible),
        "p9_best_common_gate": eligible[0] if eligible else None,
        "continued_validation_rows": len(push),
        "continued_common_gate_rows": len(push_eligible),
        "continued_best_common_gate": push_eligible[0] if push_eligible else None,
        "reported_best": best_reported,
        "reported_best_group_concentration_pass": float(best_reported["max_group_conc"]) <= 50.0,
        "selection_manifest_exists": (ART / "p7/selected-configs.json").exists(),
        "p8_p9_same_commit": True,
        "p8_p9_commit": "5adf50d",
        "post_validation_neighborhood_and_direct_f4_push": True,
        "oos_contract_pass": False,
        "reason": "F4 validation was read repeatedly for neighborhood and direct parameter search after P9",
    }


def run_replay(config: Path, budget: int, start: int, end: int) -> tuple[dict, str]:
    command = [
        str(BINARY), "--config", str(config), "--budget", str(budget),
        "--start-ms", str(start), "--end-ms", str(end),
        "--market-data", str(ROOT / "data/market_data_full.db"),
        "--funding-data", str(ROOT / "data/funding_rates_round12.db"),
    ]
    completed = subprocess.run(command, cwd=ROOT, check=True, capture_output=True, text=True, timeout=180)
    return json.loads(completed.stdout), sha256_bytes(completed.stdout.encode())


def replay_audit() -> dict:
    push_cfg = ART / "concentrated_push/configs/push_cap30_mult150_fo80_1000.json"
    best, best_stdout_sha = run_replay(push_cfg, 1000, 1767820800000, 1780271999999)
    summary = best["sync_summary"]
    metrics = best["metrics"]
    g1_cfg = ART / "p6_p7/configs/g1_C1_F4_008_ib2_1000.json"
    ib2 = next(row for row in causal_blocks(*FOLDS["F4"]) if row[0] == "ib2")
    budget_1000, sha_1000 = run_replay(g1_cfg, 1000, ib2[2], ib2[3])
    budget_4999, sha_4999 = run_replay(g1_cfg, 4999, ib2[2], ib2[3])
    return {
        "best_continued_replay": {
            "config": str(push_cfg.relative_to(ROOT)),
            "config_sha256": sha256_file(push_cfg),
            "stdout_sha256": best_stdout_sha,
            "annualized_return_pct": metrics["annualized_return_pct"],
            "max_drawdown_pct": metrics["max_drawdown_pct"],
            "actual_symbols": summary["actual_symbol_count"],
            "groups_with_so": summary["groups_with_so"],
            "max_symbol_concentration_pct": summary["max_symbol_abs_net_pnl_share_pct"],
            "max_group_concentration_pct": summary["max_group_abs_net_pnl_share_pct"],
            "strict_common_gate_pass": summary["max_group_abs_net_pnl_share_pct"] <= 50.0,
        },
        "g1_budget_override_reproduction": {
            "config": str(g1_cfg.relative_to(ROOT)),
            "embedded_budget_quote": load_json(g1_cfg).get("budget_quote"),
            "cli_1000_output_budget_quote": budget_1000["budget_quote"],
            "cli_4999_output_budget_quote": budget_4999["budget_quote"],
            "cli_1000_stdout_sha256": sha_1000,
            "cli_4999_stdout_sha256": sha_4999,
            "outputs_identical": sha_1000 == sha_4999,
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--skip-replays", action="store_true")
    args = parser.parse_args()
    evidence = {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "audit_scope": "Round 20 implementation, registries, checkpoints, selection history, and selected independent replays",
        "original_self_reported_state": "VALID_SEARCH_NO_FRONTIER_PROGRESS",
        "corrected_machine_state": "materially_incomplete_invalid_results",
        "target_hit": False,
        "production_ready_candidates": 0,
        "strict_valid_search_rows": 0,
        "registry": registry_audit(),
        "quota": quota_audit(),
        "family_runtime": family_runtime_audit(),
        "fit_causality": causality_audit(),
        "validation": validation_audit(),
        "phase_corrections": {
            "P0": "invalid_empty_central_registry_and_stub_negative_tests",
            "P1": "artifact_only_not_registry_proven",
            "P2": "isolated_exchange_module_not_connected_to_replay",
            "P3": "schedule_artifact_exists_but_runner_reuses_outer_train_end_fit",
            "P4": "labels_created_but_six_machine_definitions_not_implemented",
            "P5": "incorrectly_skipped_despite_c1_and_p1_g1_survivors",
            "P6": "not_registry_proven_and_runtime_parameters_not_family_specific",
            "P7": "research_diagnostic_only_wrong_budget_leaky_fit_and_quota_shortfall",
            "P8": "research_diagnostic_only_not_from_committed_g1_manifest",
            "P9": "invalid_no_selection_freeze_and_repeated_oos_reuse",
            "P10": "blocked_predecessor",
            "P11": "blocked_predecessor",
            "P12": "superseded_by_chatgpt_corrected_authority",
        },
        "never_repeat": [
            "Do not infer execution from checkpoint counts when the central registry is empty.",
            "Do not map named mechanisms to M1_pair/M2F without online family state in the engine.",
            "Do not represent spot and perpetual legs by duplicate strings that the loader deduplicates.",
            "Do not place exchange realism in an uncalled module and claim production-conservative replay.",
            "Do not embed 4999U in configs labeled 1000U.",
            "Do not reuse outer-train-end fits for earlier inner blocks.",
            "Do not call partial quotas exhaustive.",
            "Do not read validation, tune parameters, and read the same validation again.",
            "Do not skip Soft-SEL when its declared parent gate has survivors.",
        ],
    }
    if not args.skip_replays:
        evidence["independent_replays"] = replay_audit()
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({"output": str(OUT.relative_to(ROOT)), "corrected_machine_state": evidence["corrected_machine_state"]}))


if __name__ == "__main__":
    main()
