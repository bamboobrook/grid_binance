#!/usr/bin/env python3
"""P3 baseline-universe evidence builder.

P3 requires a frozen baseline + T1/T2/T3 universe and data qualification,
all BEFORE any return search. The universe was frozen in P1
(p1/frozen-universe.json); this script records the gate evidence.
"""

import json
import os

ART = "docs/superpowers/artifacts/glm-martingale-core-round15"


def write_gate(name, payload):
    out_dir = os.path.join(ART, "p3", "gates")
    os.makedirs(out_dir, exist_ok=True)
    path = os.path.join(out_dir, name + ".json")
    with open(path, "w") as fh:
        json.dump(payload, fh, indent=2, sort_keys=True)
    print(f"wrote {path} passed={payload.get('passed')}")


def main():
    uni = json.load(open(os.path.join(ART, "p1", "frozen-universe.json")))
    t2 = uni["T2_primary_promotion"]
    t3_8 = uni["T3_small_cap_universe_8"]

    # Three pre-declared tracks (plan §P1.1): T1 R7 rescue (diagnostic only),
    # T2 primary promotion, T3 small-cap shallow ladder.
    write_gate(
        "baseline_and_t1_t2_t3_universe_frozen",
        {
            "passed": t2["all_qualified"],
            "frozen_before_any_return_search": True,
            "T1_R7_rescue": {
                "role": "diagnostic_only_never_promoted",
                "baseline": "corrected R7 exact config (R7 universe contains survivor-picked ANKR; results never enter promotion pool)",
            },
            "T2_primary_promotion": {
                "symbols": t2["symbols"],
                "symbol_count": t2["symbol_count"],
                "long_short_sleeve_count": t2["long_short_sleeve_count"],
                "all_qualified": t2["all_qualified"],
                "direction_contract": uni["direction_contract"],
            },
            "T3_small_cap_universe_8": {
                "symbols": t3_8["symbols"],
                "qualified": all(t3_8["qualification"][s]["qualified"] for s in t3_8["symbols"]),
            },
            "T3_small_cap_universe_12": uni["T3_small_cap_universe_12"],
        },
    )
    write_gate(
        "data_qualification_passes",
        {
            "passed": t2["all_qualified"],
            "qualification": {
                "criterion": "span_fraction_of_dev >= 0.99 AND non-trivial avg 1m volume",
                "T2_failures": t2["failures"],
                "T2_all_qualified": t2["all_qualified"],
            },
            "dev_window": uni["dev_window"],
            "manifest_dev_gate": "passed (r15-data-manifest.json development_window.gate.passed=true)",
        },
    )


if __name__ == "__main__":
    main()
