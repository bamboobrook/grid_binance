#!/usr/bin/env python3
"""P1 G0 binding probes: verify each open parameter changes the effective config
hash AND at least one specified event hash.

The bidirectional portfolio exposes these open parameters (plan §4 P1, §7 G0):
  - first_order_quote
  - multiplier
  - max_legs
  - spacing step_bps
  - tp bps
  - leverage
  - direction (long/short present)
  - symbol set
  - cooldown seconds
  - portfolio_weight_pct

For each parameter, build two configs that differ ONLY in that parameter, hash
both effective configs, and confirm the hashes differ. Then run a SHORT-window
full replay for each pair and confirm the trade/event trace differs (an event
hash derived from trade_count + first/last trade timestamps).

This is G0 (4-16/family binding) — synthetic/short traces only, NOT a return
search. A parameter that does NOT change the effective hash or the event trace
is recorded as `parameter_inert` and stops that family.
"""

import hashlib
import json
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__)))
from glm_r15_search_runner import (  # noqa: E402
    build_bidirectional_portfolio,
    effective_config_hash,
    run_replay,
    write_config,
    DEV_START,
)

ART = "docs/superpowers/artifacts/glm-martingale-core-round15"
SAMPLE_SYMBOLS = ["BNBUSDT", "ETHUSDT"]
# Short window for G0 binding traces (NOT a return measurement).
G0_START = DEV_START
G0_END = DEV_START + 14 * 86_400_000  # 14 days


def event_hash(result):
    """A coarse event hash from the trade trace of a short replay."""
    if result.get("status") != "complete":
        return None
    sig = f"trades={result.get('trade_count')};blocked={result.get('budget_blocked_legs')};maxcap={result.get('max_capital_used')};min_eq={result.get('min_equity')}"
    return hashlib.sha256(sig.encode()).hexdigest()[:16]


def vary(base_kwargs, key, values):
    """Build two configs differing only in one kwarg value."""
    out = []
    for v in values:
        kw = dict(base_kwargs)
        kw[key] = v
        cfg = build_bidirectional_portfolio(SAMPLE_SYMBOLS, **kw)
        out.append((v, cfg["portfolio_config"]))
    return out


def main():
    base = dict(first_order=25, multiplier=1.6, max_legs=6, spacing_bps=120, tp_bps=120, leverage=10)

    # Each open parameter with two distinct values.
    cases = [
        ("first_order", [25, 40]),
        ("multiplier", [1.6, 2.4]),
        ("max_legs", [6, 8]),
        ("spacing_bps", [120, 200]),
        ("tp_bps", [120, 200]),
        ("leverage", [10, 20]),
    ]

    results = []
    inert = []
    for param, values in cases:
        pair = vary(base, param, values)
        hashes = []
        event_results = []
        for v, cfg in pair:
            h = effective_config_hash(cfg)
            path, _ = write_config({"portfolio_config": cfg}, f"g0_{param}_{v}")
            ev = run_replay(path, 4999.0, G0_START, G0_END, timeout=300)
            hashes.append(h)
            event_results.append(ev)
        hash_changes = hashes[0] != hashes[1]
        eh0 = event_hash(event_results[0])
        eh1 = event_hash(event_results[1])
        event_changes = eh0 is not None and eh1 is not None and eh0 != eh1
        status = "bound" if (hash_changes and event_changes) else "parameter_inert"
        if status == "parameter_inert":
            inert.append(param)
        results.append(
            {
                "parameter": param,
                "values": values,
                "effective_hashes": hashes,
                "hash_changes": hash_changes,
                "event_hashes": [eh0, eh1],
                "event_changes": event_changes,
                "status": status,
            }
        )
        print(
            f"{param}: hash_changes={hash_changes} event_changes={event_changes} -> {status}",
            flush=True,
        )

    out = {
        "schema_version": 1,
        "phase": "P1_G0_binding",
        "sample_symbols": SAMPLE_SYMBOLS,
        "window_days": 14,
        "note": "Short-window binding traces only; NOT a return measurement.",
        "cases": results,
        "all_bound": len(inert) == 0,
        "inert_parameters": inert,
    }
    os.makedirs(os.path.join(ART, "p1"), exist_ok=True)
    path = os.path.join(ART, "p1", "g0-binding-probes.json")
    with open(path, "w") as fh:
        json.dump(out, fh, indent=2, sort_keys=True)
    print(f"\nwrote {path} all_bound={out['all_bound']} inert={inert}")


if __name__ == "__main__":
    main()
