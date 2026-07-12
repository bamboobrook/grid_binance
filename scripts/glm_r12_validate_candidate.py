#!/usr/bin/env python3
"""Round 12 candidate replay harness with fail-closed promotion semantics.

This script can run shared-account static portfolio replays, cold starts, and a
budget ladder. It deliberately does not label fixed-parameter segment replays
as anchored walk-forward, and it does not synthesize cost stress when the
binary has no fee/slippage override. Missing mandatory evidence keeps
`fully_live_ready` false.
"""

import argparse
import hashlib
import json
import os
import subprocess


REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates_round12.db"
DATA_MANIFEST = (
    "docs/superpowers/artifacts/glm-martingale-core-round12/"
    "run-manifests/r12-data-manifest.json"
)

DEV_START = 1672531200000
DEV_END = 1780271999999
COLD_START_SEGMENTS = [
    ("h1_2023", 1672531200000, 1688169599999),
    ("h2_2023", 1688169600000, 1704067199999),
    ("2024", 1704067200000, 1735689599999),
    ("2025", 1735689600000, 1767225599999),
    ("2026_ytd", 1767225600000, 1780271999999),
]
BUDGET_LADDER = [1000, 2000, 3000, 4000, 4999]
TARGETS = {
    "conservative": {"ann_min": 50.0, "dd_max": 10.0, "positive_segments_min": 4},
    "balanced": {"ann_min": 90.0, "dd_max": 20.0, "positive_segments_min": 4},
    "aggressive": {"ann_min": 110.0, "dd_max": 30.0, "positive_segments_min": 3},
}


def effective_config_hash(config):
    canonical = json.dumps(config, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode()).hexdigest()


def replay(config_path, budget, start_ms, end_ms, portfolio_id):
    command = [
        REPLAY,
        "--config", config_path,
        "--budget", str(budget),
        "--start-ms", str(start_ms),
        "--end-ms", str(end_ms),
        "--market-data", MARKET_DB,
        "--funding-data", FUNDING_DB,
        "--profile", "aggressive",
        "--portfolio-id", portfolio_id,
        "--exchange-min-notional", "5",
    ]
    try:
        process = subprocess.run(command, capture_output=True, text=True, timeout=900)
    except subprocess.TimeoutExpired:
        return {"status": "error", "error": "timeout"}
    if process.returncode != 0:
        return {
            "status": "error",
            "error": process.stderr.strip()[-2000:] or f"exit {process.returncode}",
        }
    try:
        result = json.loads(process.stdout)
    except json.JSONDecodeError as error:
        return {"status": "error", "error": f"invalid JSON: {error}"}
    on_budget = result.get("on_budget", {})
    return {
        "status": "complete",
        "ann": on_budget.get("annualized_return_pct"),
        "dd": on_budget.get("max_drawdown_pct"),
        "ret": on_budget.get("total_return_pct"),
        "min_equity": on_budget.get("min_equity_quote"),
        "principal_breached": on_budget.get("principal_breached"),
        "trades": result.get("trade_count"),
        "max_cap": result.get("on_max_capital_used", {}).get("max_capital_used_quote"),
        "blocked": result.get("budget_blocked_legs"),
    }


def manifest_gate(path, required_symbols):
    if not os.path.exists(path):
        return {"passed": False, "reason": f"missing manifest: {path}"}
    with open(path, encoding="utf-8") as handle:
        manifest = json.load(handle)
    development_gate = manifest.get("development_window", {}).get("gate", {})
    manifest_symbols = set(manifest.get("development_window", {}).get("symbols", {}))
    missing_symbols = sorted(set(required_symbols) - manifest_symbols)
    complete_hashes = all(
        metadata is not None and metadata.get("sha256_complete")
        for metadata in manifest.get("files", {}).values()
    )
    return {
        "passed": bool(
            development_gate.get("passed") and complete_hashes and not missing_symbols
        ),
        "development_coverage_passed": bool(development_gate.get("passed")),
        "complete_file_hashes": complete_hashes,
        "missing_required_symbols": missing_symbols,
        "manifest_path": path,
    }


def research_threshold_hits(full, positive_segments):
    if full.get("status") != "complete":
        return []
    hits = []
    for name, target in TARGETS.items():
        if (
            full["ann"] >= target["ann_min"]
            and full["dd"] <= target["dd_max"]
            and positive_segments >= target["positive_segments_min"]
        ):
            hits.append(name)
    return hits


def validate_candidate(config_path, budget=4999, data_manifest=DATA_MANIFEST):
    with open(config_path, encoding="utf-8") as handle:
        config = json.load(handle)
    strategies = config.get("portfolio_config", config).get("strategies", [])
    traded_symbols = sorted({strategy.get("symbol") for strategy in strategies if strategy.get("symbol")})
    result = {
        "schema_version": 2,
        "config_path": config_path,
        "effective_config_sha256": effective_config_hash(config),
        "budget": budget,
        "traded_symbols": traded_symbols,
        "symbol_count": len(traded_symbols),
        "replay_semantics": {
            "shared_account": True,
            "static_portfolio": True,
            "dynamic_sleeve_allocator": False,
        },
        "data_gate": manifest_gate(data_manifest, traded_symbols),
        "full_metrics": None,
        "cold_start_segments": {},
        "budget_ladder": {},
        "research_threshold_hits": [],
        "promotion_target_hits": [],
        "mandatory_evidence": {
            "anchored_walk_forward_with_train_selection": "not_implemented",
            "neighbor_stability": "not_implemented",
            "leave_one_symbol_out_and_pnl_share": "not_implemented",
            "fee_slippage_funding_latency_stress": "not_implemented",
            "untouched_holdout": "not_run_by_this_harness",
            "backtest_live_order_trace": "not_run_by_this_harness",
            "production_db_reconcile": "not_run_by_this_harness",
        },
        "pass_gates": {},
        "fully_live_ready": False,
    }

    print(f"Full development replay (budget={budget})", flush=True)
    full = replay(config_path, budget, DEV_START, DEV_END, "r12val_full")
    result["full_metrics"] = full
    result["pass_gates"]["full_replay"] = full.get("status") == "complete"
    if full.get("status") != "complete":
        result["pass_gates"]["overall"] = False
        return result

    print("Cold-start segments", flush=True)
    positive_segments = 0
    for segment_name, start_ms, end_ms in COLD_START_SEGMENTS:
        metrics = replay(
            config_path,
            budget,
            start_ms,
            end_ms,
            f"r12val_cs_{segment_name}",
        )
        result["cold_start_segments"][segment_name] = metrics
        if metrics.get("status") == "complete" and metrics["ret"] > 0:
            positive_segments += 1
    result["pass_gates"]["positive_segments"] = positive_segments

    print("Budget ladder", flush=True)
    for ladder_budget in BUDGET_LADDER:
        result["budget_ladder"][str(ladder_budget)] = replay(
            config_path,
            ladder_budget,
            DEV_START,
            DEV_END,
            f"r12val_budget_{ladder_budget}",
        )
    result["pass_gates"]["budget_ladder_runnable"] = all(
        metrics.get("status") == "complete"
        for metrics in result["budget_ladder"].values()
    )
    result["pass_gates"]["budget_ladder_no_principal_breach"] = all(
        metrics.get("status") == "complete" and not metrics.get("principal_breached")
        for metrics in result["budget_ladder"].values()
    )

    result["research_threshold_hits"] = research_threshold_hits(full, positive_segments)
    result["pass_gates"]["multi_symbol_minimum"] = len(traded_symbols) >= 5
    result["pass_gates"]["mandatory_evidence_complete"] = False
    result["pass_gates"]["overall"] = False
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", required=True)
    parser.add_argument("--budget", type=int, default=4999)
    parser.add_argument("--data-manifest", default=DATA_MANIFEST)
    parser.add_argument("--out")
    args = parser.parse_args()

    result = validate_candidate(args.config, args.budget, args.data_manifest)
    output = args.out or f"/tmp/r12_validation_{os.path.basename(args.config)}"
    with open(output, "w", encoding="utf-8") as handle:
        json.dump(result, handle, indent=2)
        handle.write("\n")
    print(f"Wrote {output}")
    print(f"Research threshold hits: {result['research_threshold_hits']}")
    print("Promotion target hits: []")
    print("fully_live_ready: false")


if __name__ == "__main__":
    main()
