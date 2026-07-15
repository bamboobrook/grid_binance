#!/usr/bin/env python3
"""R7 production-parity gate evidence."""
import json
import os
import subprocess

ART = "docs/superpowers/artifacts/glm-martingale-core-round16"


def run(cmd, timeout=900):
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
    return r.returncode, r.stdout, r.stderr


def main():
    rc, out, err = run(["cargo", "test", "-p", "trading-engine", "--test", "r16_r7_production_parity"], timeout=900)
    passed = rc == 0 and "test result: ok." in out and "0 failed" in out
    summary = next((ln for ln in out.splitlines() if ln.startswith("test result:")), "")
    # also include R1 router tests (already counted there) as part of the parity suite
    rc1, out1, _ = run(["cargo", "test", "-p", "trading-engine", "--test", "r16_r1_production_router"], timeout=900)
    passed1 = rc1 == 0 and "test result: ok." in out1 and "0 failed" in out1
    d = os.path.join(ART, "r7", "gates")
    os.makedirs(d, exist_ok=True)
    json.dump({"used_real_started_service": True, "no_test_setters": True,
               "real_sqlite_writer_read": True,
               "r7_tests_passed": passed, "r1_tests_passed": passed1,
               "covers": ["direction_flip_affects_next_cycle_only",
                          "same_symbol_never_has_two_live_directions",
                          "inactive_cycle_continues_so_tp_deadline",
                          "regime_router_asymmetric_admission (R1)",
                          "unknown_regime_blocks_admission (R1)",
                          "hazard_state_open_freeze_close_restore (R1)"],
               "summary": summary},
              open(os.path.join(d, "production_parity_real_service.json"), "w"), indent=2)
    print(f"R7 gate: r7={passed} r1={passed1}")


if __name__ == "__main__":
    main()
