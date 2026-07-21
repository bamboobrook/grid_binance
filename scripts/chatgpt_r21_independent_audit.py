#!/usr/bin/env python3
"""Independently audit Round 21 evidence and replay two decisive blocks."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import subprocess
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round21"
OUT = ART / "audit/round21-independent-audit.json"
REGISTRY = ART / "exploration-registry.jsonl"
BINARY = ROOT / "target/release/synchronized_cycle_replay"
MANIFEST = ART / "r3/gates/causal_crossfit_manifest.json"
PLANNED = {"C1E": 64, "B1S": 96, "P1S": 64, "V1B": 64, "P1S_SoftSEL": 32}


def load_json(path: Path):
    return json.loads(path.read_text())


def load_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def registry_audit(rows: list[dict]) -> dict:
    running = [row for row in rows if row.get("status") == "running"]
    terminal = [row for row in rows if row.get("status") != "running"]
    by_id = defaultdict(list)
    for row in rows:
        by_id[row.get("experiment_id")].append(row)
    malformed_pairs = sum(
        sum(row.get("status") == "running" for row in exp_rows) != 1
        or sum(row.get("status") != "running" for row in exp_rows) != 1
        for exp_rows in by_id.values()
    )
    hash_fields = [
        "engine_sha256", "market_data_sha256", "funding_data_sha256",
        "exchange_filter_snapshot_sha256", "maintenance_tiers_sha256",
        "cost_model_sha256", "effective_config_sha256",
    ]
    invalid_hash_rows = {
        field: sum(
            not isinstance(row.get(field), str)
            or re.fullmatch(r"[0-9a-f]{64}", row.get(field, "")) is None
            for row in terminal
        )
        for field in hash_fields
    }
    trace_fields = [
        "trace_event_sha256", "trace_trade_sha256", "trace_order_sha256",
        "trace_equity_sha256", "trace_funding_sha256", "trace_rejection_sha256",
    ]
    invalid_trace_rows = {
        field: sum(
            not isinstance(row.get(field), str)
            or re.fullmatch(r"[0-9a-f]{64}", row.get(field, "")) is None
            for row in terminal
        )
        for field in trace_fields
    }
    return {
        "rows": len(rows),
        "running_rows": len(running),
        "terminal_rows": len(terminal),
        "unique_experiment_ids": len(by_id),
        "malformed_running_terminal_pairs": malformed_pairs,
        "running_rows_git_dirty_true": sum(row.get("git_dirty") is True for row in running),
        "terminal_statuses": dict(Counter(row.get("status") for row in terminal)),
        "invalid_or_non_sha256_contract_rows": invalid_hash_rows,
        "invalid_trace_rows": invalid_trace_rows,
        "failure_ledger_rows": len(load_jsonl(ART / "failure-ledger.jsonl")),
        "contract_pass": False,
    }


def engine_audit() -> dict:
    sync = (ROOT / "apps/backtest-engine/src/martingale/sync_cycle_engine.rs").read_text()
    scheduler = (ROOT / "scripts/glm_r21_c1e_scheduler.py").read_text()
    selected_cfg = load_json(ART / "_run_configs/g1bs_tb05_m3p0_fo100_ez1p0.json")
    parity = load_json(ART / "r10/gates/stream_parity.json")
    return {
        "filter_rounding_called": "filter_order_conservative" in sync,
        "partial_fill_called_by_main_replay": "apply_partial_fill(" in sync,
        "liquidation_model_called_by_main_replay": "liquidation_triggered(" in sync,
        "partial_fill_count_hardcoded_zero": '"partial_fill_count": 0_i64' in sync,
        "liquidation_count_hardcoded_zero": '"liquidation_count": 0_i64' in sync,
        "legging_loss_hardcoded_zero": '"legging_loss_quote": 0.0_f64' in sync,
        "liquidation_buffer_uses_approximation": "maintenance_margin is approximated" in sync,
        "selected_candidate_scheduler": selected_cfg["synchronized_cycle"].get("c1_scheduler"),
        "scheduler_is_static_fit_subset": "active_fits = fits[:max_live]" in scheduler,
        "scheduler_complete_rows": load_json(ART / "g1/gates/g1_scheduler.json").get("total_complete"),
        "parity_is_same_binary_twice": "SAME binary" in parity.get("honest_caveat", ""),
        "parity_has_real_live_fake_exchange_stream_comparison": False,
        "production_conservative_contract_pass": False,
    }


def family_audit(rows: list[dict]) -> dict:
    terminal = [row for row in rows if row.get("status") != "running"]
    output = {}
    for family in ("C1E", "B1S", "P1S", "V1B", "P1S_SoftSEL"):
        fam = [row for row in terminal if row.get("family") == family]
        output[family] = {
            "terminal_rows": len(fam),
            "complete_rows": sum(row.get("status") == "complete" for row in fam),
            "statuses": dict(Counter(row.get("status") for row in fam)),
            "planned_configs_per_fold": PLANNED[family],
        }
    return output


def blockspecific_rows(rows: list[dict]) -> list[dict]:
    pattern = re.compile(r"^g1bs_(tb\d+)_(m[^_]+_fo[^_]+_ez[^_]+)$")
    output = []
    for row in rows:
        if row.get("status") == "running":
            continue
        match = pattern.match(row.get("experiment_id", ""))
        if not match:
            continue
        metrics = row.get("metrics") or {}
        output.append({
            "block": match.group(1), "params": match.group(2),
            "status": row.get("status"), "first_failed_gate": row.get("first_failed_gate"),
            "annualized_return_pct": metrics.get("annualized_return_pct"),
            "total_return_pct": metrics.get("total_return_pct"),
            "max_drawdown_pct": metrics.get("max_drawdown_pct"),
        })
    return output


def compound_returns(returns: list[float]) -> float | None:
    wealth = 1.0
    for value in returns:
        wealth *= 1.0 + value / 100.0
        if wealth <= 0:
            return None
    return (wealth - 1.0) * 100.0


def selector_audit(rows: list[dict]) -> dict:
    block_rows = blockspecific_rows(rows)
    grouped = defaultdict(list)
    for row in block_rows:
        grouped[row["params"]].append(row)
    policies = []
    for params, samples in grouped.items():
        samples.sort(key=lambda row: row["block"])
        returns = [float(row.get("total_return_pct") or 0.0) for row in samples]
        dds = [float(row.get("max_drawdown_pct") or 0.0) for row in samples]
        policies.append({
            "params": params,
            "blocks": len(samples),
            "complete_blocks": sum(row["status"] == "complete" for row in samples),
            "positive_complete_blocks": sum(
                row["status"] == "complete" and float(row.get("total_return_pct") or 0.0) > 0
                for row in samples),
            "failed_blocks": sum(row["status"] != "complete" for row in samples),
            "worst_dd_pct": max(dds),
            "compounded_return_pct": compound_returns(returns),
        })
    policies.sort(key=lambda item: (-item["positive_complete_blocks"], item["worst_dd_pct"]))
    selected = load_json(ART / "r8/selected-configs.json")
    strict_cold = load_json(ART / "g1/gates/five_cold_starts_blocks.json")
    block_summary = load_json(ART / "g1/gates/g1_blockspec.json")
    selected_blocks = sorted({
        row.get("fold")
        for tier in selected.get("best_per_tier", {}).values()
        for row in tier
    })
    return {
        "test_block_days": 90,
        "short_block_ann_used_for_tier_claim": True,
        "manifest_requires_stitched_days_gte_365_for_ann": True,
        "stitched_metrics_artifact_exists": any(ART.glob("**/*stitched*.json")),
        "reported_blocks_positive": block_summary.get("blocks_positive"),
        "reported_blocks_denominator": block_summary.get("blocks_total"),
        "manifest_test_blocks": len(load_json(MANIFEST).get("test_blocks", [])),
        "missing_block_dropped_from_denominator": (
            block_summary.get("blocks_total") != len(load_json(MANIFEST).get("test_blocks", []))
        ),
        "selected_tier_blocks": selected_blocks,
        "selected_rows_all_from_tb05": selected_blocks == ["tb05"],
        "reported_cold_start_rule": selected.get("cold_start_5_of_5"),
        "strict_pre_registered_five_block_results": strict_cold.get("positivity"),
        "best_fixed_parameter_policies": policies[:8],
        "fixed_policy_reaches_four_of_five": any(
            policy["positive_complete_blocks"] >= 4
            and policy["blocks"] == 5
            for policy in policies
        ),
        "selection_contract_pass": False,
    }


def authority_audit() -> dict:
    authority = load_json(ART / "round21-authority.json")
    strict = load_json(ART / "g1/gates/five_cold_starts_blocks.json")
    return {
        "self_reported_state": authority.get("corrected_machine_state"),
        "self_reported_target_hit": authority.get("target_hit"),
        "self_reported_production_ready_candidates": authority.get("production_ready_candidates"),
        "self_reported_registry_rows": authority.get("registry_rows"),
        "actual_registry_rows": len(load_jsonl(REGISTRY)),
        "authority_claims_five_of_five": authority.get("five_of_five_positive_valid_candidates"),
        "authority_points_to_strict_evidence": authority.get("strict_cold_start_evidence"),
        "strict_evidence_best_positive": max(
            detail.get("positive", 0) for detail in strict.get("positivity", {}).values()
        ),
        "internally_consistent": False,
    }


def run_replay(block_id: str, params: str) -> dict:
    block = next(
        row for row in load_json(MANIFEST)["test_blocks"] if row["block_id"] == block_id
    )
    config = ART / f"_run_configs/g1bs_{block_id}_{params}.json"
    command = [
        str(BINARY), "--config", str(config), "--budget", "500",
        "--start-ms", str(block["test_start_ms"]), "--end-ms", str(block["test_end_ms"]),
        "--market-data", str(ROOT / "data/market_data_full.db"),
        "--funding-data", str(ROOT / "data/funding_rates_round12.db"),
    ]
    completed = subprocess.run(command, cwd=ROOT, check=True, capture_output=True, text=True, timeout=180)
    data = json.loads(completed.stdout)
    sync = data["sync_summary"]
    return {
        "block": block_id,
        "config": str(config.relative_to(ROOT)),
        "config_sha256": sha256_file(config),
        "metrics": data["metrics"],
        "actual_symbols": sync["actual_symbol_count"],
        "groups_with_so": sync["groups_with_so"],
        "max_group_concentration_pct": sync["max_group_abs_net_pnl_share_pct"],
        "liquidation_count": sync["liquidation_count"],
        "partial_fill_count": sync["partial_fill_count"],
        "legging_loss_quote": sync["legging_loss_quote"],
        "order_trace_present": "order_stream_sha256" in sync.get("trace_digests", {}),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--skip-replays", action="store_true")
    args = parser.parse_args()
    rows = load_jsonl(REGISTRY)
    evidence = {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "original_self_reported_state": "CROSS_VALIDATED_RESEARCH_FINALIST",
        "corrected_machine_state": "materially_incomplete_invalid_results",
        "target_hit": False,
        "production_ready_candidates": 0,
        "strict_valid_search_rows": 0,
        "registry": registry_audit(rows),
        "engine": engine_audit(),
        "families": family_audit(rows),
        "selector_and_crossfit": selector_audit(rows),
        "authority": authority_audit(),
        "phase_corrections": {
            "R0": "invalid_registry_hash_trace_and_clean_commit_contract",
            "R1": "partial_data_contract_only",
            "R2": "failed_runtime_wiring_partial_fill_liquidation_legging_order_trace",
            "R3": "manifest_valid_but_reporting_rule_violated",
            "R4": "partial_b1s_only_p1s_v1b_no_complete_candidate_c1e_scheduler_not_runtime",
            "G0": "invalid_deferred_and_artifact_only_binding",
            "G1": "research_diagnostic_only_quota_and_stitched_policy_failed",
            "G2": "blocked_predecessor_and_not_from_final_block_specific_manifest",
            "R8": "invalid_short_block_ann_and_post_test_top_block_selection",
            "R9": "not_executed",
            "R10": "invalid_same_binary_determinism_is_not_live_fake_exchange_parity",
            "HANDOFF": "superseded_by_chatgpt_corrected_authority",
        },
        "never_repeat": [
            "Do not compare annualized metrics from a 90-day block with annual target gates.",
            "Do not select the five best test blocks and call them five cold starts.",
            "Do not count blocks where any parameter is positive; evaluate one frozen parameter policy across all blocks.",
            "Do not drop a no-complete block from the cross-fit denominator.",
            "Do not claim a scheduler when the implementation statically truncates fits before replay.",
            "Do not mark isolated unit helpers as wired when the main replay hard-codes zero partial fills, liquidations, and legging loss.",
            "Do not call two runs of one backtest binary backtest-vs-live parity.",
            "Do not declare a family complete when it has zero complete G1 rows.",
            "Do not leave the failure ledger empty when thousands of rows fail.",
            "Do not use labels or truncated strings where the registry contract requires full SHA256 hashes.",
        ],
    }
    if not args.skip_replays:
        evidence["independent_replays"] = [
            run_replay("tb05", "m3p0_fo100_ez1p0"),
            run_replay("tb02", "m3p0_fo100_ez1p0"),
        ]
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({"output": str(OUT.relative_to(ROOT)), "state": evidence["corrected_machine_state"]}))


if __name__ == "__main__":
    main()
