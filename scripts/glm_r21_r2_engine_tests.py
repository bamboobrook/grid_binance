#!/usr/bin/env python3
"""Round 21 R2: run the 12 engine-conservative canary tests and write the
evidence file the validator consumes (engine_conservative_tests.json).

Plan §4 mandates 12 tests (§4.1-§4.12). The validator's R2 gate
`engine_conservative_tests_pass` requires all_passed=True. This script runs
`cargo test --release --lib -p backtest-engine r21_conservative` and parses
the per-test pass/fail into the evidence JSON.

Plan §4 last paragraph: "R2 任一测试失败，整轮状态为 BLOCKED_ENGINE_DATA_OR_EXECUTION，
禁止开始 family fit 或搜索."
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round21"
OUT = ART / "r2" / "gates" / "engine_conservative_tests.json"

# Plan §4.1-§4.12 canary list (matches the test names in
# r21_conservative_engine.rs).
CANARIES = [
    ("r21_canary_1", "§4.1 spot/perp same symbol are two distinct MarketLegIds"),
    ("r21_canary_2", "§4.2 PRICE_FILTER/LOT_SIZE/MARKET_LOT_SIZE/MIN_NOTIONAL applied"),
    ("r21_canary_3", "§4.3 post-rounding paired hedge re-check"),
    ("r21_canary_4", "§4.4 maintenance tier + conservative liquidation"),
    ("r21_canary_5", "§4.5 25/50/75% partial fill + legging loss"),
    ("r21_canary_6", "§4.6 FO/SO/TP/abort/end-close uniform fee; funding as cycle cashflow"),
    ("r21_canary_7", "§4.7 next-SO + close fee + maintenance reserve gate"),
    ("r21_canary_8", "§4.8 reject cooldown permanent_config vs temporary_margin"),
    ("r21_canary_9", "§4.9 kill/restart/reconcile idempotent"),
    ("r21_canary_10", "§4.10 backtest adapter vs fake exchange parity"),
    ("r21_canary_11", "§4.11 64-concurrency same-input same-hash"),
    ("r21_canary_12", "§4.12 min_liquidation_buffer_pct non-null, event-ledger recomputed"),
]


def run_tests() -> dict:
    cmd = ["cargo", "test", "--release", "--lib", "-p", "backtest-engine",
           "r21_conservative", "--", "--nocapture"]
    proc = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True,
                          timeout=600)
    out = proc.stdout + "\n" + proc.stderr
    passed: dict[str, bool] = {}
    # Parse "test ... ... ok" / "test ... ... FAILED" lines
    for line in out.splitlines():
        m = re.match(r"\s*test\s+(.*?(r21_canary_\d+).*?)\s+\.\.\.\s+(ok|FAILED|ignored)\b",
                     line)
        if m:
            full_name, short, status = m.group(1), m.group(2), m.group(3)
            passed[short] = (status == "ok")
    summary = re.search(r"test result: ok\. (\d+) passed; (\d+) failed", out)
    total_passed = int(summary.group(1)) if summary else 0
    total_failed = int(summary.group(2)) if summary else 0
    return {
        "per_canary": {c: {"plan_ref": ref, "passed": passed.get(c, False)}
                       for c, ref in CANARIES},
        "cargo_exit_code": proc.returncode,
        "total_passed": total_passed,
        "total_failed": total_failed,
        "stdout_tail": out[-2000:],
    }


def main() -> int:
    OUT.parent.mkdir(parents=True, exist_ok=True)
    result = run_tests()
    per = result["per_canary"]
    all_passed = all(v["passed"] for v in per.values()) and result["total_failed"] == 0
    evidence = {
        "phase": "R21 R2 engine conservative tests (plan §4.1-§4.12)",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "canaries": per,
        "all_passed": all_passed,
        "total_passed": result["total_passed"],
        "total_failed": result["total_failed"],
        "cargo_exit_code": result["cargo_exit_code"],
        "plan_§4_blocking_rule": ("R2 任一测试失败，整轮状态为 "
                                  "BLOCKED_ENGINE_DATA_OR_EXECUTION，禁止开始 "
                                  "family fit 或搜索."),
        "stdout_tail": result["stdout_tail"],
    }
    with open(OUT, "w") as fh:
        json.dump(evidence, fh, indent=2, sort_keys=True)
    print(f"wrote {OUT}")
    print(f"all_passed={all_passed} "
          f"({result['total_passed']} passed, {result['total_failed']} failed)")
    return 0 if all_passed else 1


if __name__ == "__main__":
    sys.exit(main())
