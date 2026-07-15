#!/usr/bin/env python3
"""P7 production-parity gate evidence.

Combines the P1 wiring tests (r15_p1_production_wiring.rs, 5 tests) and the
P7 parity extensions (r15_p7_production_parity.rs, 3 tests). Together they
cover the plan §9 production-parity contract through the REAL MartingaleRuntime
executor + backtest comparison.
"""

import json
import os
import subprocess

ART = "docs/superpowers/artifacts/glm-martingale-core-round15"


def run(cmd, timeout=900):
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
    return r.returncode, r.stdout, r.stderr


def write_gate(name, payload):
    out_dir = os.path.join(ART, "p7", "gates")
    os.makedirs(out_dir, exist_ok=True)
    path = os.path.join(out_dir, name + ".json")
    with open(path, "w") as fh:
        json.dump(payload, fh, indent=2, sort_keys=True)
    print(f"wrote {path} passed={payload.get('passed')}")


def cargo(pkg, test_name):
    rc, out, err = run(["cargo", "test", "-p", pkg, "--test", test_name], timeout=900)
    passed = rc == 0 and "test result: ok." in out and "0 failed" in out
    summary = next((ln for ln in out.splitlines() if ln.startswith("test result:")), "")
    return {"passed": passed, "summary_line": summary}


def main():
    p1 = cargo("trading-engine", "r15_p1_production_wiring")
    p7 = cargo("trading-engine", "r15_p7_production_parity")
    both = p1["passed"] and p7["passed"]
    write_gate(
        "backtest_live_restart_order_trace_match",
        {
            "passed": both,
            "p1_wiring_test": p1,
            "p7_parity_test": p7,
            "covers": [
                "backtest_live_order_trace_matches (P1: live first-leg notional/margin == backtest open_leg)",
                "reconcile_does_not_duplicate_cycle_or_so (P1)",
                "restart_restores_selector_half_life_deadline_cluster_reserve (P1)",
                "same_timestamp_strategy_order_invariant (P7)",
                "exchange_rounding_min_notional_parity (P7)",
                "inactive_cycle_continues_so_tp_sl (P7)",
            ],
        },
    )
    write_gate(
        "production_wiring_tests_pass",
        {
            "passed": both,
            "p1_wiring_test": p1,
            "p7_parity_test": p7,
        },
    )


if __name__ == "__main__":
    main()
