#!/usr/bin/env python3
"""Reconstruct G1 summary + Pareto survivors from the registry (the run was
interrupted by the shell timeout but all replays are persisted in the registry).

G1 only determines elimination (plan §11). The 30-day-window ann is NOT a valid
performance figure. We select global <=24 Pareto configs (high ann, low DD,
no breach) for G2 full-window development.
"""
import json
import os

ART = "docs/superpowers/artifacts/glm-martingale-core-round16"


def main():
    recs = [json.loads(l) for l in open(os.path.join(ART, "exploration-registry.jsonl")) if l.strip()]
    g1 = [r for r in recs if "G1" in r.get("family", "") and r.get("status") == "complete"]
    uniq = set(r.get("effective_config_hash") for r in g1)

    # Pareto on (ann desc, dd asc) per config, aggregated across window/budget.
    # For each config, take its best no-breach (ann, dd) across all windows/budgets.
    by_cfg = {}
    for r in g1:
        if r.get("breached"):
            continue
        ch = r.get("effective_config_hash")
        ann = r.get("ann", -999); dd = r.get("dd", 999)
        if ch not in by_cfg or ann > by_cfg[ch]["ann"]:
            by_cfg[ch] = {"ann": ann, "dd": dd, "track": r.get("track"),
                          "params": r.get("params"), "window": r.get("window"),
                          "budget": r.get("budget")}

    cfgs = list(by_cfg.values())
    # Pareto front: not dominated (another with >=ann AND <=dd)
    pareto = []
    for c in sorted(cfgs, key=lambda x: -x["ann"]):
        dominated = any(o["ann"] >= c["ann"] and o["dd"] <= c["dd"] and o is not c for o in cfgs)
        if not dominated:
            pareto.append(c)
    pareto = pareto[:24]

    # count actual replays (unique key) vs duplicates
    seen = set()
    actual = dups = 0
    for r in g1:
        key = f"{r.get('effective_config_hash')}|{r.get('window')}|{r.get('budget')}"
        actual += 1
        if key in seen:
            dups += 1
        seen.add(key)

    out = {
        "schema_version": 1, "phase": "R4_G1", "sampler": "scipy_sobol",
        "unique_configs": len(uniq), "actual_replays": len(g1),
        "duplicates": dups, "windows": 4, "budgets": 3,
        "tracks": {"T1": sum(1 for r in g1 if r.get("track") == "T1"),
                   "T2": sum(1 for r in g1 if r.get("track") == "T2"),
                   "T3_8": sum(1 for r in g1 if r.get("track") == "T3_8"),
                   "T3_12": sum(1 for r in g1 if r.get("track") == "T3_12")},
        "no_breach": sum(1 for r in g1 if not r.get("breached")),
        "breached": sum(1 for r in g1 if r.get("breached")),
        "pareto_survivors": pareto,
        "note": "G1 only determines elimination; 30-day-window ann is NOT valid performance. Pareto survivors proceed to G2 full-window development.",
    }
    os.makedirs(os.path.join(ART, "r4"), exist_ok=True)
    with open(os.path.join(ART, "r4", "g1-sobol.json"), "w") as f:
        json.dump(out, f, indent=2, sort_keys=True)
    # gate evidence
    d = os.path.join(ART, "r4", "gates")
    os.makedirs(d, exist_ok=True)
    json.dump({"unique_configs": len(uniq), "actual_replays": len(g1),
               "windows": 4, "budgets": 3, "duplicates": dups,
               "duplicate_rate_under_25pct": (dups / max(1, len(g1))) <= 0.25},
              open(os.path.join(d, "g1_128_sobol_1536_replays.json"), "w"), indent=2)
    print(f"G1 reconstructed: {len(uniq)} unique configs, {len(g1)} actual replays, {len(pareto)} pareto survivors")


if __name__ == "__main__":
    main()
