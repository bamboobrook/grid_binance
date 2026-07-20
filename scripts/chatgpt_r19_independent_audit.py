#!/usr/bin/env python3
"""Independently audit Round 19 from raw checkpoints and engine behavior.

This intentionally does not read any validation window. The optional replays
only cover the already-selected F3 train window.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import statistics
import subprocess
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round19"
OUT = ART / "audit/round19-independent-audit.json"
CENTRAL_REGISTRY = ART / "exploration-registry.jsonl"
G1_REGISTRY = ART / "r6/g1-registry.jsonl"
G2_REGISTRY = ART / "r7/g2-registry.jsonl"
G1_CHECKPOINT = ART / "r6/g1-checkpoint.json"
G2_CHECKPOINT = ART / "r7/g2-checkpoint.json"
FAMILY_GATE = ART / "r4/gates/five_families.json"
FITS = ART / "r4/families-fit.json"
G0 = ART / "r5/g0_binding.json"
ENGINE = ROOT / "apps/backtest-engine/src/martingale/sync_cycle_engine.rs"
BINARY = ROOT / "target/release/synchronized_cycle_replay"

RUNNING_FIELDS = {
    "experiment_id",
    "status",
    "fingerprint_sha256",
    "raw_command",
    "pid",
    "started_at",
    "engine_sha256",
    "data_sha256",
    "funding_sha256",
    "trigger_contract_sha256",
    "fit_contract_sha256",
    "universe_group_weights_sha256",
    "resolved_config_sha256",
    "effective_config_sha256",
    "cost_model_sha256",
}
TERMINAL_FIELDS = {
    "experiment_id",
    "status",
    "fingerprint_sha256",
    "raw_command",
    "exit_code",
    "wall_s",
    "event_stream_sha256",
    "trade_stream_sha256",
    "equity_stream_sha256",
    "funding_stream_sha256",
    "rejection_stream_sha256",
}


def load_json(path: Path):
    return json.loads(path.read_text())


def load_jsonl(path: Path) -> list[dict]:
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def registry_audit() -> dict:
    central = load_jsonl(CENTRAL_REGISTRY)
    local = load_jsonl(G1_REGISTRY) + load_jsonl(G2_REGISTRY)
    all_required = RUNNING_FIELDS | TERMINAL_FIELDS
    missing = {field: sum(field not in row for row in local) for field in sorted(all_required)}
    running_rows = sum(row.get("status") == "running" for row in local)
    terminal_rows = sum(row.get("status") != "running" for row in local)
    fingerprints = {row.get("fingerprint_sha256") for row in local if row.get("fingerprint_sha256")}
    return {
        "central_rows": len(central),
        "local_rows": len(local),
        "g1_rows": len(load_jsonl(G1_REGISTRY)),
        "g2_rows": len(load_jsonl(G2_REGISTRY)),
        "running_rows": running_rows,
        "terminal_rows": terminal_rows,
        "unique_terminal_fingerprints": len(fingerprints),
        "missing_required_field_rows": missing,
        "contract_pass": bool(central) and running_rows > 0 and not any(missing.values()),
    }


def strict_g1() -> dict:
    rows = load_json(G1_CHECKPOINT)["done"].values()
    grouped: dict[tuple, dict[str, dict]] = defaultdict(dict)
    for row in rows:
        key = (row["family"], row["fold"], row["config_id"], int(row["budget"]))
        grouped[key][row["block"]] = row

    survivors: dict[tuple[str, str], list[dict]] = defaultdict(list)
    for (family, fold, config_id, budget), blocks in grouped.items():
        samples = [blocks.get(name) for name in ("tb1", "tb2", "tb3")]
        if any(sample is None or sample.get("status") != "complete" for sample in samples):
            continue
        reasons = []
        anns = [float(sample.get("ann") or 0.0) for sample in samples]
        if any(sample.get("breach") for sample in samples):
            reasons.append("breach")
        if any(float(sample.get("dd") or 0.0) > 45.0 for sample in samples):
            reasons.append("dd_gt_45")
        if sum(ann < 0.0 for ann in anns) >= 2:
            reasons.append("two_blocks_negative")
        if any(int(sample.get("actual_symbols") or 0) < 5 for sample in samples):
            reasons.append("any_block_symbols_lt_5")
        if any(float(sample.get("max_symbol_conc") or 0.0) > 50.0 for sample in samples):
            reasons.append("symbol_concentration_gt_50")
        if any(float(sample.get("max_group_conc") or 0.0) > 50.0 for sample in samples):
            reasons.append("group_concentration_gt_50")
        if any(int(sample.get("groups_with_so") or 0) == 0 for sample in samples):
            reasons.append("any_block_no_real_so")
        if reasons:
            continue
        median_ann = statistics.median(anns)
        worst_dd = max(float(sample.get("dd") or 0.0) for sample in samples)
        survivors[(family, fold)].append(
            {
                "config_id": config_id,
                "budget": budget,
                "median_ann_pct": median_ann,
                "worst_dd_pct": worst_dd,
                "positive_blocks": sum(ann > 0.0 for ann in anns),
                "score": median_ann - worst_dd * 0.5 + sum(ann > 0.0 for ann in anns) * 5.0,
            }
        )

    output = {}
    for family in ("M1R", "M2F"):
        output[family] = {}
        for fold in ("F1", "F2", "F3", "F4"):
            ranked = sorted(survivors[(family, fold)], key=lambda row: -row["score"])
            output[family][fold] = {"strict_survivor_count": len(ranked), "top16": ranked[:16]}
    return output


def g2_rerouted_inputs() -> dict:
    """Reproduce R7's looser G1 re-selection, which omitted G1 hard gates."""
    rows = load_json(G1_CHECKPOINT)["done"].values()
    grouped: dict[tuple, dict[str, dict]] = defaultdict(dict)
    for row in rows:
        key = (row["family"], row["fold"], row["config_id"], int(row["budget"]))
        grouped[key][row["block"]] = row
    selected: dict[tuple[str, str], list[dict]] = defaultdict(list)
    for (family, fold, config_id, budget), blocks in grouped.items():
        samples = [blocks.get(name) for name in ("tb1", "tb2", "tb3")]
        if any(sample is None or sample.get("status") != "complete" for sample in samples):
            continue
        anns = [float(sample.get("ann") or 0.0) for sample in samples]
        if any(sample.get("breach") for sample in samples):
            continue
        if any(float(sample.get("dd") or 0.0) > 45.0 for sample in samples):
            continue
        if sum(ann < 0.0 for ann in anns) >= 2:
            continue
        if sum(int(sample.get("groups_with_so") or 0) for sample in samples) == 0:
            continue
        median_ann = statistics.median(anns)
        worst_dd = max(float(sample.get("dd") or 0.0) for sample in samples)
        selected[(family, fold)].append(
            {
                "config_id": config_id,
                "budget": budget,
                "score": median_ann - worst_dd * 0.5 + sum(ann > 0.0 for ann in anns) * 5.0,
            }
        )

    output = {}
    for family in ("M1R", "M2F"):
        output[family] = {}
        for fold in ("F1", "F2", "F3", "F4"):
            ranked = sorted(selected[(family, fold)], key=lambda row: -row["score"])
            seen = set()
            top16 = []
            for row in ranked:
                if row["config_id"] in seen:
                    continue
                seen.add(row["config_id"])
                top16.append(row)
                if len(top16) == 16:
                    break
            output[family][fold] = {"rerouted_config_count": len(top16), "config_ids": [r["config_id"] for r in top16]}
    return output


def strict_g2(g1: dict) -> dict:
    rows = load_json(G2_CHECKPOINT)["done"].values()
    grouped: dict[tuple, dict[str, dict]] = defaultdict(dict)
    for row in rows:
        key = (row["family"], row["fold"], row["config_id"], int(row["budget"]))
        grouped[key][row["window"]] = row

    allowed = {}
    for family, folds in g1.items():
        for fold, detail in folds.items():
            allowed[(family, fold)] = {row["config_id"] for row in detail["top16"]}

    finalists: dict[tuple[str, str], list[dict]] = defaultdict(list)
    for (family, fold, config_id, budget), windows in grouped.items():
        if config_id not in allowed.get((family, fold), set()):
            continue
        subblocks = [windows.get(f"sb{i}") for i in range(1, 6)]
        full = windows.get("full_train")
        if full is None or any(row is None or row.get("status") != "complete" for row in subblocks):
            continue
        anns = [float(row.get("ann") or 0.0) for row in subblocks]
        reasons = []
        if statistics.median(anns) < 30.0:
            reasons.append("median_ann_lt_30")
        if max(float(row.get("dd") or 0.0) for row in subblocks) > 35.0:
            reasons.append("worst_dd_gt_35")
        if sum(ann > 0.0 for ann in anns) < 4:
            reasons.append("positive_blocks_lt_4")
        all_rows = subblocks + [full]
        if any(row.get("breach") for row in all_rows):
            reasons.append("breach")
        if any(int(row.get("actual_symbols") or 0) < 5 for row in all_rows):
            reasons.append("any_window_symbols_lt_5")
        if any(float(row.get("max_symbol_conc") or 0.0) > 50.0 for row in all_rows):
            reasons.append("symbol_concentration_gt_50")
        if any(float(row.get("max_group_conc") or 0.0) > 50.0 for row in all_rows):
            reasons.append("group_concentration_gt_50")
        if sum(int(row.get("groups_with_so") or 0) > 0 for row in subblocks) < 4:
            reasons.append("real_so_blocks_lt_4")
        if reasons:
            continue
        finalists[(family, fold)].append(
            {
                "config_id": config_id,
                "budget": budget,
                "median_ann_pct": statistics.median(anns),
                "worst_dd_pct": max(float(row.get("dd") or 0.0) for row in subblocks),
                "positive_blocks": sum(ann > 0.0 for ann in anns),
            }
        )

    output = {}
    for family in ("M1R", "M2F"):
        output[family] = {}
        for fold in ("F1", "F2", "F3", "F4"):
            rows = finalists[(family, fold)]
            output[family][fold] = {"strict_finalist_count": len(rows), "finalists": rows}
    return output


def nearest_train_diagnostics() -> list[dict]:
    rows = load_json(G2_CHECKPOINT)["done"].values()
    grouped: dict[tuple, dict[str, dict]] = defaultdict(dict)
    for row in rows:
        key = (row["family"], row["fold"], row["config_id"], int(row["budget"]))
        grouped[key][row["window"]] = row

    diagnostics = []
    for (family, fold, config_id, budget), windows in grouped.items():
        subblocks = [windows.get(f"sb{i}") for i in range(1, 6)]
        full = windows.get("full_train")
        if full is None or any(row is None or row.get("status") != "complete" for row in subblocks):
            continue
        if any(row.get("breach") for row in subblocks + [full]):
            continue
        if any(int(row.get("actual_symbols") or 0) < 5 for row in subblocks):
            continue
        if any(int(row.get("groups_with_so") or 0) == 0 for row in subblocks):
            continue
        anns = [float(row.get("ann") or 0.0) for row in subblocks]
        if sum(ann > 0.0 for ann in anns) < 5:
            continue
        concentration_passes = sum(
            float(row.get("max_symbol_conc") or 0.0) <= 50.0
            and float(row.get("max_group_conc") or 0.0) <= 50.0
            for row in subblocks
        )
        diagnostics.append(
            {
                "config_id": config_id,
                "family": family,
                "fold": fold,
                "budget": budget,
                "median_subblock_ann_pct": statistics.median(anns),
                "worst_subblock_dd_pct": max(float(row.get("dd") or 0.0) for row in subblocks),
                "positive_subblocks": 5,
                "real_so_subblocks": 5,
                "concentration_pass_subblocks": concentration_passes,
                "full_train_ann_pct": float(full.get("ann") or 0.0),
                "full_train_dd_pct": float(full.get("dd") or 0.0),
                "full_train_max_symbol_concentration_pct": float(full.get("max_symbol_conc") or 0.0),
                "full_train_max_group_concentration_pct": float(full.get("max_group_conc") or 0.0),
                "params": full.get("params", {}),
                "classification": "train_only_selected_diagnostic_with_inner_fit_lookahead_not_candidate",
            }
        )
    diagnostics.sort(key=lambda row: (-row["median_subblock_ann_pct"], row["worst_subblock_dd_pct"]))
    return diagnostics[:20]


def m2_exposure_audit() -> list[dict]:
    output = []
    for fold in load_json(FITS)["fold_fits"]:
        fit = fold["M2F"]["frozen_fits"][0]
        signs = fit["leg_direction_signs"]
        betas = fit["betas"]
        equal_weight_exposure = sum(sign * beta for sign, beta in zip(signs, betas)) / len(signs)
        output.append(
            {
                "fold": fold["fold"],
                "legs": len(signs),
                "equal_weight_signed_beta_exposure": equal_weight_exposure,
                "required_abs_max": 0.10,
                "pass": abs(equal_weight_exposure) <= 0.10,
            }
        )
    return output


def fit_causality_audit() -> list[dict]:
    """R7 reused each fold's train-end fit in all five earlier subblocks."""
    output = []
    for fold in load_json(FITS)["fold_fits"]:
        train_start = int(fold["train_full_start"])
        train_end = int(fold["train_full_end"])
        fit_end = int(fold["fit_end"])
        chunk = (train_end - train_start) // 5
        subblocks = []
        for index in range(5):
            start = train_start + index * chunk
            end = train_start + (index + 1) * chunk if index < 4 else train_end
            subblocks.append(
                {
                    "subblock": f"sb{index + 1}",
                    "start_ms": start,
                    "end_ms": end,
                    "fit_end_ms": fit_end,
                    "causal": fit_end < start,
                }
            )
        output.append(
            {
                "fold": fold["fold"],
                "train_end_fit_reused_by_r7": True,
                "causal_subblocks": sum(row["causal"] for row in subblocks),
                "leaky_subblocks": sum(not row["causal"] for row in subblocks),
                "subblocks": subblocks,
            }
        )
    return output


def family_and_g0_audit() -> dict:
    families = load_json(FAMILY_GATE)["families"]
    g0 = load_json(G0)
    return {
        "planned_families": ["M1R", "M2F", "P1", "K1", "V1"],
        "statuses": {name: detail["status"] for name, detail in families.items()},
        "implemented_count": sum(str(detail["status"]).startswith("implemented") for detail in families.values()),
        "missing_implementations": [name for name, detail in families.items() if not str(detail["status"]).startswith("implemented")],
        "g0_families": [
            {
                "family": row["family"],
                "fold": row["fold"],
                "all_bound": row["all_bound"],
                "inert_params": row["inert_params"],
            }
            for row in g0["families"]
        ],
        "strict_g0_pass": all(row["all_bound"] is True for row in g0["families"]),
        "self_reported_conditional_binding": g0.get("all_implemented_families_conditionally_bound"),
    }


def engine_contract_audit() -> dict:
    source = ENGINE.read_text()
    return {
        "invocation_local_cycle_sequence_after_audit_fix": (
            "let mut next_cycle_seq = 1_u64" in source and "static CYCLE_SEQ" not in source
        ),
        "min_liquidation_buffer_is_none": "min_liquidation_buffer_pct: None" in source,
        "pc1_returns_none": 'Some("PC1") => None' in source,
        "no_exchange_filter_contract": not any(token in source for token in ("step_size", "tick_size", "min_qty")),
        "no_partial_fill_state": not any(token in source for token in ("partial_fill_qty", "filled_qty", "legging_state")),
        "no_rejection_cooldown_state": not any(token in source for token in ("cooldown_until", "permanent_freeze", "rejection_state_hash")),
        "tautological_no_so_test": "assert!(g_so == 0 || g_so >= 1" in source,
        "m2_static_train_signs": "Ok(fit.leg_direction_signs.clone())" in source,
        "m2_equal_notional_comment": "M2 remains equal" in source,
        "production_hard_gate_pass": False,
    }


def run_train_replay(config_id: str) -> dict:
    config = ART / f"r7/configs/g2_{config_id}_full_train_1000.json"
    command = [
        str(BINARY),
        "--config",
        str(config),
        "--budget",
        "1000",
        "--start-ms",
        "1672531200000",
        "--end-ms",
        "1735689599999",
        "--market-data",
        str(ROOT / "data/market_data_full.db"),
        "--funding-data",
        str(ROOT / "data/funding_rates_round12.db"),
    ]
    completed = subprocess.run(command, cwd=ROOT, check=True, capture_output=True, text=True, timeout=120)
    result = json.loads(completed.stdout)
    checkpoint = next(
        row
        for row in load_json(G2_CHECKPOINT)["done"].values()
        if row.get("config_id") == config_id and row.get("window") == "full_train" and int(row.get("budget")) == 1000
    )
    summary = result["sync_summary"]
    metric_checks = {
        "ann": abs(result["metrics"]["annualized_return_pct"] - checkpoint["ann"]) <= 1e-12,
        "dd": abs(result["metrics"]["max_drawdown_pct"] - checkpoint["dd"]) <= 1e-12,
        "trades": result["metrics"]["trade_count"] == checkpoint["trade_count"],
        "groups_with_so": summary["groups_with_so"] == checkpoint["groups_with_so"],
        "actual_symbols": summary["actual_symbol_count"] == checkpoint["actual_symbols"],
    }
    return {
        "config_id": config_id,
        "scope": "F3 train only; validation not read",
        "config_sha256": sha256_file(config),
        "binary_sha256": sha256_file(BINARY),
        "metrics": {
            "annualized_return_pct": result["metrics"]["annualized_return_pct"],
            "max_drawdown_pct": result["metrics"]["max_drawdown_pct"],
            "trade_count": result["metrics"]["trade_count"],
            "groups_with_so": summary["groups_with_so"],
            "actual_symbol_count": summary["actual_symbol_count"],
            "max_symbol_concentration_pct": summary["max_symbol_abs_net_pnl_share_pct"],
            "max_group_concentration_pct": summary["max_group_abs_net_pnl_share_pct"],
            "group_atomic_reject": summary["group_atomic_reject"],
            "min_liquidation_buffer_pct": result["metrics"].get("min_liquidation_buffer_pct"),
        },
        "trace_digests": summary["trace_digests"],
        "checkpoint_exact_match": all(metric_checks.values()),
        "metric_checks": metric_checks,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--skip-replays", action="store_true")
    args = parser.parse_args()

    g1 = strict_g1()
    evidence = {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "audit_scope": "Round 19 train artifacts and implementation; no validation read",
        "original_self_reported_state": "VALID_SEARCH_NO_FRONTIER_PROGRESS",
        "corrected_machine_state": "materially_incomplete_invalid_results",
        "target_hit": False,
        "production_ready_candidates": 0,
        "registry": registry_audit(),
        "families_and_g0": family_and_g0_audit(),
        "engine_contract": engine_contract_audit(),
        "m2_equal_weight_factor_exposure": m2_exposure_audit(),
        "fit_causality": fit_causality_audit(),
        "strict_g1": g1,
        "r7_reselected_inputs_omitting_g1_hard_gates": g2_rerouted_inputs(),
        "strict_g2": strict_g2(g1),
        "nearest_train_only_diagnostics": nearest_train_diagnostics(),
        "strict_valid_search_rows": 0,
        "strict_valid_search_rows_reason": (
            "R2 production hard gates are absent, G0 has inert parameters, three planned families were not implemented, "
            "M2F does not implement the specified dynamic constrained basket, and the append-only central registry is empty"
        ),
        "phase_corrections": {
            "R0": "invalid_empty_central_registry",
            "R1": "partial_index_only_not_connected_to_runners",
            "R2": "materially_incomplete_failclose",
            "R3": "partial_research_cycle_only",
            "R4": "materially_incomplete_three_families_missing_m2_contract_invalid",
            "R5": "failed_inert_parameters",
            "R6": "research_diagnostic_only_blocked_predecessor",
            "R7": "research_diagnostic_only_inconsistent_g1_input",
            "R8": "blocked_predecessor",
            "R9": "blocked_predecessor",
            "R9.5": "blocked_predecessor",
            "R10": "blocked_predecessor",
            "R11": "blocked_predecessor",
            "R12": "superseded_by_chatgpt_audit",
        },
    }
    if not args.skip_replays:
        evidence["independent_train_replays"] = [
            run_train_replay("M1R_F3_044"),
            run_train_replay("M1R_F3_028"),
        ]
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({"output": str(OUT.relative_to(ROOT)), "corrected_machine_state": evidence["corrected_machine_state"]}))


if __name__ == "__main__":
    main()
