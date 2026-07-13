#!/usr/bin/env python3
"""Independently replay Round 13 candidates with fail-closed target gates."""

import argparse
import concurrent.futures
import datetime as dt
import hashlib
import json
import os
import subprocess
import sys


REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates_round12.db"
DATA_MANIFEST = (
    "docs/superpowers/artifacts/glm-martingale-core-round13/"
    "run-manifests/r13-data-manifest.json"
)
DEV_START = 1_672_531_200_000
DEV_END = 1_780_271_999_999
HOLDOUT_START = 1_780_272_000_000
HOLDOUT_END = 1_783_727_999_999
SEGMENTS = [
    ("h1_2023", 1_672_531_200_000, 1_688_169_599_999),
    ("h2_2023", 1_688_169_600_000, 1_704_067_199_999),
    ("2024", 1_704_067_200_000, 1_735_689_599_999),
    ("2025", 1_735_689_600_000, 1_767_225_599_999),
    ("2026_ytd", 1_767_225_600_000, 1_780_271_999_999),
]
BUDGETS = [1_000, 2_000, 3_000, 4_000, 4_999]
TARGETS = {
    "conservative": {"ann_min": 50.0, "dd_max": 10.0, "positive_min": 4},
    "balanced": {"ann_min": 90.0, "dd_max": 20.0, "positive_min": 4},
    "aggressive": {"ann_min": 110.0, "dd_max": 30.0, "positive_min": 3},
}
CANDIDATES = {
    "r7_corrected_event_level_frontier": (
        "docs/superpowers/artifacts/glm-martingale-core-round7/"
        "promising/r7-ANKR-q1w24p24.json"
    ),
    "r4_corrected_baseline": (
        "docs/superpowers/artifacts/glm-martingale-core-round4/"
        "promising/r4-combo-best.json"
    ),
    "robust_pool_icp_trx_long_pair": (
        "docs/superpowers/artifacts/glm-martingale-core-round13/"
        "promising/r13-icp-trx-lp-pair.json"
    ),
    "r4like_bnb_trx_long_pair": (
        "docs/superpowers/artifacts/glm-martingale-core-round13/"
        "promising/r13-best-bnb-trx-s100-m08.json"
    ),
    "robust_pool_five_symbol_long_portfolio": (
        "docs/superpowers/artifacts/glm-martingale-core-round13/"
        "promising/r13-5sym-icp-trx-bnb-btc-xrp.json"
    ),
}


def sha256_bytes(payload):
    return hashlib.sha256(payload).hexdigest()


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        while chunk := handle.read(8 * 1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_config_hash(config):
    payload = json.dumps(config, sort_keys=True, separators=(",", ":")).encode()
    return sha256_bytes(payload)


def source_tree_hash():
    scope = ["Cargo.lock", "Cargo.toml", "apps", "crates", "scripts"]
    tracked_diff = subprocess.run(
        ["git", "diff", "--binary", "HEAD", "--", *scope],
        capture_output=True,
        check=True,
    ).stdout
    untracked = subprocess.run(
        ["git", "ls-files", "--others", "--exclude-standard", "--", *scope],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.splitlines()
    digest = hashlib.sha256(tracked_diff)
    for path in sorted(untracked):
        digest.update(path.encode())
        digest.update(b"\0")
        digest.update(bytes.fromhex(sha256_file(path)))
    return digest.hexdigest()


def run_replay(config_path, budget, start_ms, end_ms, run_id):
    command = [
        REPLAY,
        "--config",
        config_path,
        "--budget",
        str(budget),
        "--start-ms",
        str(start_ms),
        "--end-ms",
        str(end_ms),
        "--market-data",
        MARKET_DB,
        "--funding-data",
        FUNDING_DB,
        "--profile",
        "aggressive",
        "--portfolio-id",
        run_id,
        "--exchange-min-notional",
        "5",
    ]
    process = subprocess.run(command, capture_output=True, text=True, timeout=900)
    if process.returncode != 0:
        return {
            "status": "error",
            "command": command,
            "error": process.stderr.strip()[-4_000:] or f"exit={process.returncode}",
        }
    try:
        raw = json.loads(process.stdout)
    except json.JSONDecodeError as error:
        return {"status": "error", "command": command, "error": str(error)}
    concentration = sorted(
        raw.get("realized_pnl_by_symbol", []),
        key=lambda item: item.get("positive_pnl_share_pct", 0.0),
        reverse=True,
    )
    return {
        "status": "complete",
        "command": command,
        "raw_stdout_sha256": sha256_bytes(process.stdout.encode()),
        "on_budget": raw.get("on_budget"),
        "trade_count": raw.get("trade_count"),
        "stop_count": raw.get("stop_count"),
        "max_capital_used_quote": raw.get("max_capital_used_quote"),
        "budget_blocked_legs": raw.get("budget_blocked_legs"),
        "total_fee_quote": raw.get("total_fee_quote"),
        "total_slippage_quote": raw.get("total_slippage_quote"),
        "total_funding_quote": raw.get("total_funding_quote"),
        "per_strategy": raw.get("per_strategy", []),
        "realized_pnl_by_symbol": concentration,
    }


def metrics(replay):
    return replay.get("on_budget") or {}


def audit_candidate(item):
    label, config_path = item
    print(f"[r13-audit] {label}", file=sys.stderr, flush=True)
    with open(config_path, encoding="utf-8") as handle:
        config = json.load(handle)
    portfolio = config.get("portfolio_config", config)
    strategies = portfolio.get("strategies", [])
    symbols = sorted({strategy["symbol"] for strategy in strategies})

    full = run_replay(config_path, 4_999, DEV_START, DEV_END, f"r13fix_{label}_full")
    cold_segments = {
        name: run_replay(config_path, 4_999, start, end, f"r13fix_{label}_{name}")
        for name, start, end in SEGMENTS
    }
    budget_ladder = {
        str(budget): run_replay(
            config_path, budget, DEV_START, DEV_END, f"r13fix_{label}_b{budget}"
        )
        for budget in BUDGETS
    }
    holdout = run_replay(
        config_path, 4_999, HOLDOUT_START, HOLDOUT_END, f"r13fix_{label}_holdout"
    )

    positive_segments = sum(
        1
        for replay in cold_segments.values()
        if replay.get("status") == "complete"
        and metrics(replay).get("total_return_pct", float("-inf")) > 0.0
    )
    max_positive_pnl_share = max(
        (
            row.get("positive_pnl_share_pct", 0.0)
            for row in full.get("realized_pnl_by_symbol", [])
        ),
        default=100.0,
    )
    max_configured_cap_pct = max(
        (row.get("weight_pct", 100.0) for row in full.get("per_strategy", [])),
        default=100.0,
    )
    full_metrics = metrics(full)
    metric_tier_hits = [
        tier
        for tier, target in TARGETS.items()
        if full.get("status") == "complete"
        and full_metrics.get("annualized_return_pct", float("-inf")) >= target["ann_min"]
        and full_metrics.get("max_drawdown_pct", float("inf")) <= target["dd_max"]
        and positive_segments >= target["positive_min"]
    ]
    small_cap_no_breach = all(
        budget_ladder[str(budget)].get("status") == "complete"
        and not metrics(budget_ladder[str(budget)]).get("principal_breached", True)
        for budget in (1_000, 2_000)
    )
    structure_passed = (
        len(symbols) >= 5
        and len(symbols) >= 3
        and max_configured_cap_pct <= 35.0
        and max_positive_pnl_share <= 35.0
    )
    holdout_metrics = metrics(holdout)
    holdout_result_passed = (
        holdout.get("status") == "complete"
        and holdout_metrics.get("total_return_pct", float("-inf")) > 0.0
        and not holdout_metrics.get("principal_breached", True)
    )

    reject_reasons = []
    if len(symbols) < 5:
        reject_reasons.append("fewer than 5 traded symbols")
    if max_configured_cap_pct > 35.0:
        reject_reasons.append("configured strategy/symbol cap exceeds 35%")
    if max_positive_pnl_share > 35.0:
        reject_reasons.append("realized positive PnL concentration exceeds 35%")
    if not metric_tier_hits:
        reject_reasons.append("no annualized/DD/positive-segment tier hit")
    if not small_cap_no_breach:
        reject_reasons.append("1000/2000U principal-breach gate failed")
    if not holdout_result_passed:
        reject_reasons.append("holdout result gate failed")
    reject_reasons.extend(
        [
            "holdout was already opened before this candidate audit",
            "anchored nested WFO with train-only selection missing",
            "production DB/executor parity missing",
        ]
    )

    return {
        "label": label,
        "config_path": config_path,
        "config_file_sha256": sha256_file(config_path),
        "effective_config_sha256": canonical_config_hash(config),
        "traded_symbols": symbols,
        "symbol_count": len(symbols),
        "strategy_count": len(strategies),
        "full": full,
        "cold_start_segments": cold_segments,
        "positive_segments": positive_segments,
        "budget_ladder": budget_ladder,
        "holdout": holdout,
        "gates": {
            "metric_tier_hits": metric_tier_hits,
            "accepted_target_hits": [],
            "small_cap_1000_2000_no_principal_breach": small_cap_no_breach,
            "minimum_five_symbols": len(symbols) >= 5,
            "minimum_three_assets": len(symbols) >= 3,
            "max_configured_cap_pct": max_configured_cap_pct,
            "max_realized_positive_pnl_share_pct": max_positive_pnl_share,
            "structure_passed": structure_passed,
            "holdout_result_passed": holdout_result_passed,
            "holdout_candidate_unseen": False,
            "anchored_nested_wfo": False,
            "production_db_executor_parity": False,
            "fully_live_ready": False,
        },
        "reject_reasons": reject_reasons,
        "actual_binary_replays": 12,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True)
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--market-db-sha256")
    args = parser.parse_args()

    with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as executor:
        candidates = list(executor.map(audit_candidate, CANDIDATES.items()))

    git_head = subprocess.run(
        ["git", "rev-parse", "HEAD"], capture_output=True, text=True, check=True
    ).stdout.strip()
    result = {
        "schema_version": 1,
        "round": 13,
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "source": {
            "base_commit": git_head,
            "source_scope": ["Cargo.lock", "Cargo.toml", "apps", "crates", "scripts"],
            "dirty_source_tree_sha256": source_tree_hash(),
            "engine_binary_sha256": sha256_file(REPLAY),
        },
        "data": {
            "market_db_path": MARKET_DB,
            "market_db_size": os.path.getsize(MARKET_DB),
            "market_db_sha256": args.market_db_sha256 or sha256_file(MARKET_DB),
            "funding_db_path": FUNDING_DB,
            "funding_db_size": os.path.getsize(FUNDING_DB),
            "funding_db_sha256": sha256_file(FUNDING_DB),
            "manifest_path": DATA_MANIFEST,
            "manifest_sha256": sha256_file(DATA_MANIFEST),
        },
        "windows": {
            "development": [DEV_START, DEV_END],
            "holdout": [HOLDOUT_START, HOLDOUT_END],
        },
        "actual_binary_replays": sum(row["actual_binary_replays"] for row in candidates),
        "unique_config_count": len(candidates),
        "candidates": candidates,
        "target_verdict": {
            "conservative": "NOT MET",
            "balanced": "NOT MET",
            "aggressive": "NOT MET",
        },
        "fully_live_ready_candidates": [],
    }
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    temporary = f"{args.out}.tmp"
    with open(temporary, "w", encoding="utf-8") as handle:
        json.dump(result, handle, indent=2)
        handle.write("\n")
    os.replace(temporary, args.out)
    print(f"wrote {args.out}: {result['actual_binary_replays']} replays")


if __name__ == "__main__":
    main()
