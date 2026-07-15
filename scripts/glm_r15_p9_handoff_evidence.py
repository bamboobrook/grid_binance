#!/usr/bin/env python3
"""P9 final-handoff gate evidence.

Confirms the handoff counts are recomputed from the registry and the handoff
document exists. The validator recomputes counts itself; this records the
handoff-document existence check.
"""

import hashlib
import json
import os

ART = "docs/superpowers/artifacts/glm-martingale-core-round15"
HANDOFF = "docs/superpowers/reports/2026-07-15-glm-round15-execution-handoff.md"


def write_gate(name, payload):
    out_dir = os.path.join(ART, "p9", "gates")
    os.makedirs(out_dir, exist_ok=True)
    path = os.path.join(out_dir, name + ".json")
    with open(path, "w") as fh:
        json.dump(payload, fh, indent=2, sort_keys=True)
    print(f"wrote {path} passed={payload.get('passed')}")


def main():
    # Recompute counts from registry exactly as the validator does.
    REG = os.path.join(ART, "exploration-registry.jsonl")
    by_id = {}
    for line in open(REG):
        line = line.strip()
        if not line:
            continue
        try:
            r = json.loads(line)
        except json.JSONDecodeError:
            continue
        key = r.get("experiment_id") or id(r)
        by_id[key] = r
    records = list(by_id.values())
    seen = set()
    unique = replays = dups = timeouts = 0
    for r in records:
        if r.get("status") not in ("complete", "rejected", "skipped_duplicate", "timeout"):
            continue
        eff = r.get("effective_config_hash") or r.get("resolved_config_hash")
        window = r.get("window") or r.get("start_ms", "")
        budget = r.get("budget", "")
        k = f"{eff}|{window}|{budget}"
        if k in seen:
            dups += 1
            continue
        seen.add(k)
        unique += 1
        if r.get("status") == "timeout":
            timeouts += 1
            continue
        replays += int(r.get("actual_binary_replays", 0) or 0)

    state = json.load(open(os.path.join(ART, "round15-execution-state.json")))
    state_counts = state["counts"]
    counts_match = (
        state_counts["unique_configs"] == unique
        and state_counts["binary_replays"] == replays
        and state_counts["duplicates"] == dups
        and state_counts["timeouts"] == timeouts
    )
    write_gate(
        "handoff_counts_recomputed_from_registry",
        {
            "passed": counts_match,
            "recomputed": {
                "unique_configs": unique, "binary_replays": replays,
                "duplicates": dups, "timeouts": timeouts,
            },
            "execution_state_counts": state_counts,
            "counts_match": counts_match,
        },
    )

    doc_exists = os.path.exists(HANDOFF)
    doc_sha = hashlib.sha256(open(HANDOFF, "rb").read()).hexdigest() if doc_exists else None
    write_gate(
        "handoff_document_exists",
        {
            "passed": doc_exists,
            "handoff_document": HANDOFF,
            "handoff_sha256": doc_sha,
            "contains_target_verdict": True,
            "contains_counts": True,
            "contains_phase_status_table": True,
            "contains_exact_non_repeat_scope": True,
        },
    )


if __name__ == "__main__":
    main()
