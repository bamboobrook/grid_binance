#!/usr/bin/env python3
"""GLM Round 7 Task A: Merge Round 1-6 non-repeat registry."""
import argparse, json, os, glob, re

def parse_registry_jsonl(path):
    entries = []
    if not os.path.exists(path): return entries
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line: continue
            try: entries.append(json.loads(line))
            except: pass
    return entries

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    # Read all round registries
    registries = {}
    for rnd in range(2, 8):
        path = f"docs/superpowers/artifacts/glm-martingale-core-round{rnd}/exploration-registry.jsonl"
        entries = parse_registry_jsonl(path)
        registries[f"R{rnd}"] = entries

    # Build do_not_repeat list
    do_not_repeat = []
    seen_keys = set()
    for rnd, entries in registries.items():
        for e in entries:
            family = e.get("family", "unknown")
            decision = e.get("decision", "")
            result = e.get("result", "")
            non_repeat = e.get("non_repeat_key", "")
            key = non_repeat or f"{rnd}-{family}"
            if key in seen_keys: continue
            seen_keys.add(key)
            if decision in ("rejected", "no_improvement", "no_effect", "no_dd_improvement",
                           "no_target_hit", "blocked", "no_effect_config_only"):
                do_not_repeat.append({
                    "key": key,
                    "round": rnd,
                    "family": family,
                    "decision": decision,
                    "result": result[:200],
                    "allowed_reopen_condition": "Only with a new mechanism key not covered by this entry"
                })

    # Add R1 known failures from the search ledger
    r1_failures = [
        {"key": "regime-gate-grid", "round": "R1", "family": "regime_gate",
         "decision": "rejected", "result": "0/3276 passed segment gates",
         "allowed_reopen_condition": "Only if gate is cycle-attribution-derived"},
        {"key": "tight-sl-only", "round": "R1", "family": "tight_sl",
         "decision": "rejected", "result": "DD<=10% makes ann negative",
         "allowed_reopen_condition": "Only paired with dynamic deal-size"},
        {"key": "portfolio-stop-alone", "round": "R1", "family": "portfolio_stop",
         "decision": "rejected", "result": "Reduces DD but kills return",
         "allowed_reopen_condition": "Only as live circuit breaker"},
        {"key": "custom-ladder-rescan", "round": "R2-R5", "family": "custom_ladder",
         "decision": "rejected", "result": "fixed150 always best; custom near 0% under last-exec SO",
         "allowed_reopen_condition": "Only with rebound/wick SO or range-regime sleeve"},
        {"key": "same-symbol-hedge-grid", "round": "R5", "family": "hedged_grid",
         "decision": "rejected", "result": "400 candidates ann 0-4.5%",
         "allowed_reopen_condition": "Only if hedge leg is capped martingale rescue"},
        {"key": "long-gate-relaxation", "round": "R4", "family": "gate_relax",
         "decision": "rejected", "result": "Relaxing long gate worsens 2025/2026",
         "allowed_reopen_condition": "Never without cycle-attribution proof"},
        {"key": "btc-breadth-proxy", "round": "R3", "family": "breadth_regime",
         "decision": "rejected", "result": "BTC breadth worsened 2025",
         "allowed_reopen_condition": "Only with cycle-derived health, not BTC-only"},
    ]
    for f in r1_failures:
        if f["key"] not in seen_keys:
            do_not_repeat.append(f)
            seen_keys.add(f["key"])

    # Partial/missing
    partial_or_missing = [
        {"key": "r6-task-a-detail", "issue": "R6 attribution had only segment summary, no symbol/leg state"},
        {"key": "r6-task-e-safety-freeze-engine", "issue": "Config-only, engine logic not fully implemented"},
        {"key": "r6-task-d-trailing-lock-engine", "issue": "Engine logic implemented but DD increased"},
        {"key": "r6-task-f-dynamic-blend", "issue": "Only static blend tested; dynamic allocator deferred"},
        {"key": "r6-task-g-r5-r6-live-parity", "issue": "R5/R6 features remain backtest-only"},
        {"key": "r5-ankr-lowdd-result", "issue": "Script committed but result never completed (superseded by R6)"},
    ]

    # Promising
    promising = [
        {"key": "r5-last-executed-so", "result": "ann 44.6-59.5% depending on symbol/mult"},
        {"key": "r5-vol-target", "result": "marginal +0.5pp ann at same DD"},
        {"key": "r6-quarantine-q1w24p24", "result": "ann 55.8%/DD 25.0% on XRP (ann boost)"},
        {"key": "r6-blend-ankr20-r460", "result": "DD 19.4% (balanced DD gate) but ann 34.9%"},
        {"key": "r6-blend-xrpq20-r460", "result": "DD 18.0% (lowest DD with 4/5 pos) but ann 34.0%"},
    ]

    output = {
        "generated_at_utc": "2026-07-03T00:00:00Z",
        "total_do_not_repeat": len(do_not_repeat),
        "total_partial_or_missing": len(partial_or_missing),
        "total_promising": len(promising),
        "do_not_repeat": do_not_repeat,
        "partial_or_missing": partial_or_missing,
        "promising": promising,
    }

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w") as f:
        json.dump(output, f, indent=2)
    print(f"Wrote {args.out}: {len(do_not_repeat)} do_not_repeat, {len(partial_or_missing)} partial, {len(promising)} promising")

if __name__ == "__main__":
    main()
