#!/usr/bin/env python3
"""Independently recheck the Round 12 R4 baseline and repaired holdout."""

import argparse
import hashlib
import json
import os

from glm_r12_validate_candidate import (
    BUDGET_LADDER,
    COLD_START_SEGMENTS,
    DATA_MANIFEST,
    DEV_END,
    DEV_START,
    replay,
)


HOLDOUT_START = 1780272000000
HOLDOUT_END = 1783727999999
DEFAULT_CONFIG = (
    "docs/superpowers/artifacts/glm-martingale-core-round4/"
    "promising/r4-combo-best.json"
)
DEFAULT_OUT = (
    "docs/superpowers/artifacts/glm-martingale-core-round12/"
    "r12-r4-independent-recheck.json"
)
DEFAULT_HOLDOUT_OUT = (
    "docs/superpowers/artifacts/glm-martingale-core-round12/"
    "r12-holdout-validation.json"
)


def file_sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def require_data_gates(manifest_path):
    with open(manifest_path, encoding="utf-8") as handle:
        manifest = json.load(handle)
    development = manifest.get("development_window", {}).get("gate", {})
    holdout = manifest.get("holdout_window", {}).get("gate", {})
    if not development.get("passed") or not holdout.get("passed"):
        raise SystemExit(
            f"data gates must pass before recheck: development={development}, holdout={holdout}"
        )
    return manifest


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default=DEFAULT_CONFIG)
    parser.add_argument("--manifest", default=DATA_MANIFEST)
    parser.add_argument("--out", default=DEFAULT_OUT)
    parser.add_argument("--holdout-out", default=DEFAULT_HOLDOUT_OUT)
    args = parser.parse_args()

    manifest = require_data_gates(args.manifest)
    result = {
        "schema_version": 1,
        "candidate": "R4-combo",
        "config": args.config,
        "config_sha256": file_sha256(args.config),
        "engine_sha256": manifest.get("engine", {}).get("sha256"),
        "data_manifest": args.manifest,
        "data_manifest_sha256": file_sha256(args.manifest),
        "development": {"full_4999": None, "cold_start_segments": {}, "budget_ladder": {}},
        "holdout": {"start_ms": HOLDOUT_START, "end_ms": HOLDOUT_END, "budget_ladder": {}},
        "promotion_target_hits": [],
        "fully_live_ready": False,
    }

    print("Development full 4999", flush=True)
    full = replay(args.config, 4999, DEV_START, DEV_END, "r12audit_r4_full")
    result["development"]["full_4999"] = full
    for budget in BUDGET_LADDER:
        if budget == 4999:
            metrics = full
        else:
            print(f"Development budget {budget}", flush=True)
            metrics = replay(
                args.config, budget, DEV_START, DEV_END, f"r12audit_r4_dev_{budget}"
            )
        result["development"]["budget_ladder"][str(budget)] = metrics

    for name, start_ms, end_ms in COLD_START_SEGMENTS:
        print(f"Cold start {name}", flush=True)
        result["development"]["cold_start_segments"][name] = replay(
            args.config, 4999, start_ms, end_ms, f"r12audit_r4_{name}"
        )

    for budget in BUDGET_LADDER:
        print(f"Holdout budget {budget}", flush=True)
        result["holdout"]["budget_ladder"][str(budget)] = replay(
            args.config,
            budget,
            HOLDOUT_START,
            HOLDOUT_END,
            f"r12audit_r4_holdout_{budget}",
        )

    positive_segments = sum(
        metrics.get("status") == "complete" and metrics.get("ret", 0) > 0
        for metrics in result["development"]["cold_start_segments"].values()
    )
    result["development"]["positive_segments"] = positive_segments
    holdout_4999 = result["holdout"]["budget_ladder"]["4999"]
    holdout_pass = bool(
        holdout_4999.get("status") == "complete"
        and holdout_4999.get("ret", 0) > 0
        and not holdout_4999.get("principal_breached")
    )
    result["holdout"]["gate_pass"] = holdout_pass
    result["holdout"]["verdict"] = (
        "PASS: positive return and no principal breach"
        if holdout_pass
        else "FAIL: return is non-positive, replay failed, or principal was breached"
    )

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as handle:
        json.dump(result, handle, indent=2, sort_keys=True)
        handle.write("\n")

    holdout_artifact = {
        "schema_version": 2,
        "candidate": "R4-combo",
        "data_manifest": args.manifest,
        "data_gate_passed": True,
        "holdout_window": {"start_ms": HOLDOUT_START, "end_ms": HOLDOUT_END},
        "holdout_opened": True,
        "budget_ladder": result["holdout"]["budget_ladder"],
        "gate_pass": holdout_pass,
        "verdict": result["holdout"]["verdict"],
    }
    with open(args.holdout_out, "w", encoding="utf-8") as handle:
        json.dump(holdout_artifact, handle, indent=2, sort_keys=True)
        handle.write("\n")
    print(f"Wrote {args.out}")
    print(f"Wrote {args.holdout_out}")


if __name__ == "__main__":
    main()
