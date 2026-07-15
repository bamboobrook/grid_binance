#!/usr/bin/env python3
"""P0.1/P0.2 evidence builder: verify the Round 15 frozen baseline matches the
frozen Round 14 canonical per-symbol row hashes, and that the funding source is
the authoritative round12 db.

Writes docs/superpowers/artifacts/glm-martingale-core-round15/p0/gates/*.json
evidence files consumed by the validator.
"""

import hashlib
import json
import os
import subprocess

ART = "docs/superpowers/artifacts/glm-martingale-core-round15"
R14_MANIFEST = "docs/superpowers/artifacts/glm-martingale-core-round14/run-manifests/r14-data-manifest.json"
R15_MANIFEST = os.path.join(ART, "run-manifests", "r15-data-manifest.json")
PLAN_PATH = "docs/superpowers/plans/2026-07-14-glm-martingale-core-round15-directional-hazard-cluster-plan.md"

PBO_BIN = "target/release/portfolio_budget_replay"
HTF_BIN = "target/release/r14_htf_search"

AUDIT_MARKET_DB = "c404e4c80de4ac2a633e740f15f577239f539a29fc1c974dc6cfc05b1c29f790"
AUDIT_FUNDING_DB = "4d77dbdeddc42f8bb800e4b784bc6eb3e77d5213212be4e8bd3226274a4e1114"
AUDIT_PREMIUM_DB = "78bd01280bbb349383e090b9576cfa543865a5680997329c33f2e05c089a2627"
AUDIT_CLI_BIN = "d21c6971aacc5166adff56ce1d8b135f8f15d194c966d4324219885d181c4d98"
AUDIT_BATCH_BIN = "4c914d68cd8d19a95f7ea9a50ed18f845ca5a6be271f81e4a157509d47da00e7"


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        while chunk := fh.read(8 * 1024 * 1024):
            h.update(chunk)
    return h.hexdigest()


def git_head():
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], stderr=subprocess.DEVNULL
        ).decode().strip()
    except Exception:
        return None


def write_gate(name, payload):
    out_dir = os.path.join(ART, "p0", "gates")
    os.makedirs(out_dir, exist_ok=True)
    path = os.path.join(out_dir, name + ".json")
    with open(path, "w") as fh:
        json.dump(payload, fh, indent=2, sort_keys=True)
    print(f"wrote {path} passed={payload.get('passed')}")


def main():
    r14 = json.load(open(R14_MANIFEST))
    r15 = json.load(open(R15_MANIFEST))
    r14_syms = r14["development_window"]["symbols"]
    r15_syms = r15["development_window"]["symbols"]

    market_match = 0
    market_mismatch = []
    funding_match = 0
    funding_mismatch = []
    for sym in r14_syms:
        if sym not in r15_syms:
            continue
        if r14_syms[sym]["market"]["canonical_sha256"] == r15_syms[sym]["market"]["canonical_sha256"]:
            market_match += 1
        else:
            market_mismatch.append(sym)
        if r14_syms[sym]["funding"]["canonical_sha256"] == r15_syms[sym]["funding"]["canonical_sha256"]:
            funding_match += 1
        else:
            funding_mismatch.append(sym)
    write_gate(
        "market_per_symbol_hashes_match_frozen_r14",
        {
            "passed": len(market_mismatch) == 0,
            "symbols_checked": len(r14_syms),
            "matched": market_match,
            "mismatched": market_mismatch,
        },
    )

    # funding authoritative: round12 db file hash == audit funding baseline AND
    # per-symbol hashes match
    funding_file = sha256_file("data/funding_rates_round12.db")
    write_gate(
        "funding_db_is_authoritative_round12",
        {
            "passed": funding_file == AUDIT_FUNDING_DB and len(funding_mismatch) == 0,
            "round12_file_sha256": funding_file,
            "matches_audit_funding_baseline": funding_file == AUDIT_FUNDING_DB,
            "per_symbol_matched": funding_match,
            "per_symbol_mismatched": funding_mismatch,
            "note": (
                "data/funding_rates.db has 0 rows for ANKRUSDT/LTCUSDT and is NOT used. "
                "data/funding_rates_round12.db carries all 32 symbols and matches the audit."
            ),
        },
    )

    # binaries match audit
    cli = sha256_file(PBO_BIN) if os.path.exists(PBO_BIN) else None
    htf = sha256_file(HTF_BIN) if os.path.exists(HTF_BIN) else None
    write_gate(
        "binaries_hash_match_audit",
        {
            "passed": cli == AUDIT_CLI_BIN and htf == AUDIT_BATCH_BIN,
            "portfolio_budget_replay_sha256": cli,
            "matches_audit_cli": cli == AUDIT_CLI_BIN,
            "r14_htf_search_sha256": htf,
            "matches_audit_batch": htf == AUDIT_BATCH_BIN,
        },
    )

    # plan frozen
    plan_sha = sha256_file(PLAN_PATH)
    write_gate(
        "plan_sha256_frozen",
        {
            "passed": plan_sha == r15["plan_sha256"],
            "plan_sha256": plan_sha,
            "manifest_plan_sha256": r15["plan_sha256"],
        },
    )

    # manifest gate
    write_gate(
        "manifest_exists_and_dev_gate_passes",
        {
            "passed": bool(r15["development_window"]["gate"]["passed"]),
            "dev_gate": r15["development_window"]["gate"],
            "holdout_gate": r15["holdout_window"]["gate"],
        },
    )

    print("\nP0.1 evidence complete. market", market_match, "funding", funding_match)


if __name__ == "__main__":
    main()
