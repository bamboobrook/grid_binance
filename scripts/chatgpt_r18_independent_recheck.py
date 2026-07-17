#!/usr/bin/env python3
"""Independent replay of every Round 18 G3 row with the corrected M1 ledger."""

import glob
import hashlib
import json
import os
import subprocess
import time
from datetime import datetime, timezone


ROOT = "docs/superpowers/artifacts/glm-martingale-core-round18"
CONFIG_DIR = os.path.join(ROOT, "r8", "configs")
OLD_CHECKPOINT = os.path.join(ROOT, "r8", "g3-checkpoint.json")
OUTPUT = os.path.join(ROOT, "audit", "round18-independent-recheck.json")
BINARY = "target/release/synchronized_cycle_replay"

FOLD_WINDOWS = {
    "F2": (1704662400000, 1735689599999),
    "F3": (1736294400000, 1767225599999),
    "F4": (1767820800000, 1780271999999),
}


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_name(path):
    stem = os.path.basename(path).removesuffix(".json")
    prefix, family, sequence, budget, fold = stem.split("_")
    if prefix != "g3":
        raise ValueError(f"unexpected config filename: {stem}")
    config_id = f"{family}_{sequence}"
    return config_id, int(budget), fold


def run_one(path, budget, fold):
    start_ms, end_ms = FOLD_WINDOWS[fold]
    command = [
        BINARY,
        "--config",
        path,
        "--budget",
        str(budget),
        "--start-ms",
        str(start_ms),
        "--end-ms",
        str(end_ms),
        "--market-data",
        "data/market_data_full.db",
        "--funding-data",
        "data/funding_rates_round12.db",
    ]
    started = time.time()
    process = subprocess.run(command, capture_output=True, text=True, timeout=900)
    wall_s = time.time() - started
    if process.returncode != 0:
        return {
            "status": "error",
            "exit_code": process.returncode,
            "wall_s": round(wall_s, 3),
            "stderr": process.stderr[-1000:],
            "command": command,
        }
    payload = json.loads(process.stdout)
    metrics = payload["metrics"]
    sync = payload["sync_summary"]
    return {
        "status": "complete",
        "exit_code": 0,
        "wall_s": round(wall_s, 3),
        "command": command,
        "resolved_config_sha256": payload["resolved_config_sha256"],
        "ann": metrics["annualized_return_pct"],
        "dd": metrics["max_drawdown_pct"],
        "min_equity": metrics["min_equity_quote"],
        "breach": metrics["breach"],
        "trade_count": metrics["trade_count"],
        "fee_quote": metrics["total_fee_quote"],
        "slippage_quote": metrics["total_slippage_quote"],
        "funding_quote": metrics["total_funding_quote"],
        "actual_symbols": sync.get("actual_symbols", []),
        "actual_symbol_count": sync.get("actual_symbol_count", 0),
        "max_symbol_gross_share_pct": sync.get("max_symbol_gross_share_pct"),
        "max_symbol_positive_pnl_share_pct": sync.get("max_symbol_positive_pnl_share_pct"),
        "max_symbol_abs_net_pnl_share_pct": sync.get("max_symbol_abs_net_pnl_share_pct"),
        "max_group_abs_net_pnl_share_pct": sync.get("max_group_abs_net_pnl_share_pct"),
        "group_fo": sync.get("group_fo", []),
        "group_so": sync.get("group_so", []),
        "group_tp": sync.get("group_tp", []),
        "group_reduce": sync.get("group_reduce", []),
        "group_atomic_reject": sync.get("group_atomic_reject", []),
        "groups_with_so": sync.get("groups_with_so", 0),
        "cycles_with_so": sync.get("cycles_with_so", 0),
        "trace_digests": sync.get("trace_digests", {}),
    }


def main():
    with open(OLD_CHECKPOINT) as handle:
        old = json.load(handle)["done"]

    rows = []
    for path in sorted(glob.glob(os.path.join(CONFIG_DIR, "g3_*.json"))):
        config_id, budget, fold = parse_name(path)
        key = f"{config_id}|{budget}|{fold}"
        old_row = old.get(key, {})
        print(f"recheck {key}", flush=True)
        corrected = run_one(path, budget, fold)
        rows.append(
            {
                "key": key,
                "config_id": config_id,
                "budget": budget,
                "fold": fold,
                "config_path": path,
                "old_claim": {
                    "ann": old_row.get("ann"),
                    "dd": old_row.get("dd"),
                    "breach": old_row.get("breach"),
                    "trade_count": old_row.get("trade_count"),
                },
                "corrected": corrected,
            }
        )

    completed = [row for row in rows if row["corrected"]["status"] == "complete"]
    positive = [row for row in completed if row["corrected"]["ann"] > 0]
    valid_common = [
        row
        for row in completed
        if not row["corrected"]["breach"]
        and row["corrected"]["groups_with_so"] > 0
        and row["corrected"]["actual_symbol_count"] >= 5
        and (row["corrected"]["max_symbol_abs_net_pnl_share_pct"] or 100.0) <= 50.0
        and (row["corrected"]["max_group_abs_net_pnl_share_pct"] or 100.0) <= 50.0
    ]
    summary = {
        "rows_expected": 24,
        "rows_completed": len(completed),
        "positive_rows": len(positive),
        "martingale_common_gate_rows": len(valid_common),
        "best_ann": max(
            (
                {
                    "key": row["key"],
                    "ann": row["corrected"]["ann"],
                    "dd": row["corrected"]["dd"],
                }
                for row in completed
            ),
            key=lambda row: row["ann"],
            default=None,
        ),
        "best_martingale_common_gate": max(
            (
                {
                    "key": row["key"],
                    "ann": row["corrected"]["ann"],
                    "dd": row["corrected"]["dd"],
                }
                for row in valid_common
            ),
            key=lambda row: row["ann"],
            default=None,
        ),
        "target_hit": False,
        "target_note": "No row can hit a final tier without valid 5-cold-start evidence; all old G3 claims are superseded by corrected engine rows.",
    }
    output = {
        "schema_version": 1,
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "corrected_machine_state": "materially_incomplete_invalid_results",
        "purpose": "Supersede invalid Round 18 G3 results produced by static long/long directions and incorrect Martingale accounting.",
        "base_head_before_correction": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "corrected_engine_source_sha256": sha256_file(
            "apps/backtest-engine/src/martingale/sync_cycle_engine.rs"
        ),
        "release_binary_sha256": sha256_file(BINARY),
        "market_data_sha256": sha256_file("data/market_data_full.db"),
        "funding_data_sha256": sha256_file("data/funding_rates_round12.db"),
        "summary": summary,
        "rows": rows,
    }
    os.makedirs(os.path.dirname(OUTPUT), exist_ok=True)
    with open(OUTPUT, "w") as handle:
        json.dump(output, handle, indent=2, sort_keys=True)
    print(json.dumps(summary, indent=2, sort_keys=True))
    print(f"wrote {OUTPUT}")


if __name__ == "__main__":
    main()
