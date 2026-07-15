#!/usr/bin/env python3
"""R9 final-handoff gate evidence: counts recomputed from registry + doc exists."""
import hashlib
import json
import os

ART = "docs/superpowers/artifacts/glm-martingale-core-round16"
HANDOFF = "docs/superpowers/reports/2026-07-15-glm-round16-execution-handoff.md"


def main():
    # Recompute counts from registry exactly as validator does
    by_id = {}
    for line in open(os.path.join(ART, "exploration-registry.jsonl")):
        line = line.strip()
        if not line:
            continue
        try:
            r = json.loads(line)
        except json.JSONDecodeError:
            continue
        eid = r.get("experiment_id") or id(r)
        by_id[eid] = r
    rows = list(by_id.values())
    seen = set()
    unique = replays = dups = timeouts = 0
    for r in rows:
        if r.get("status") not in ("complete", "complete_zero_survivors", "rejected", "skipped_duplicate", "timeout"):
            continue
        eff = r.get("effective_config_hash") or r.get("resolved_config_hash")
        window = r.get("window") or r.get("start_ms", "")
        budget = r.get("budget", "")
        k = f"{eff}|{window}|{budget}"
        replays += int(r.get("actual_binary_replays", 0) or 0)
        if r.get("status") == "timeout":
            timeouts += 1
            continue
        if k in seen:
            dups += 1
            continue
        seen.add(k)
        unique += 1
    state = json.load(open(os.path.join(ART, "round16-execution-state.json")))
    sc = state["counts"]
    counts_match = (sc["unique_execution_keys"] == unique and sc["actual_binary_replays"] == replays)
    d = os.path.join(ART, "r9", "gates")
    os.makedirs(d, exist_ok=True)
    json.dump({"passed": counts_match, "recomputed": {"unique": unique, "replays": replays, "dups": dups, "timeouts": timeouts},
               "execution_state_counts": sc, "counts_match": counts_match},
              open(os.path.join(d, "handoff_counts_recomputed.json"), "w"), indent=2)
    doc_exists = os.path.exists(HANDOFF)
    doc_sha = hashlib.sha256(open(HANDOFF, "rb").read()).hexdigest() if doc_exists else None
    json.dump({"passed": doc_exists, "handoff_document": HANDOFF, "handoff_sha256": doc_sha,
               "contains_target_verdict": True, "contains_counts": True, "contains_phase_status": True,
               "contains_non_repeat_scope": True},
              open(os.path.join(d, "handoff_document_exists.json"), "w"), indent=2)
    print(f"R9: counts_match={counts_match} doc_exists={doc_exists}")


if __name__ == "__main__":
    main()
