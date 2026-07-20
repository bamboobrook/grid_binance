#!/usr/bin/env python3
"""Round 21 R0: CI static scan — enforce single spawn site for
synchronized_cycle_replay (plan §2 lines 57-58).

"CI 静态扫描其他 glm_r21_* 出现 subprocess.run.*synchronized_cycle_replay
必须失败"

The ONLY allowed spawn site is `scripts/glm_r21_launcher.py`
(`Launcher.run_synchronized_cycle`). Any other `scripts/glm_r21_*` script
that contains both `subprocess` AND `synchronized_cycle_replay` in a likely
spawn context FAILS the scan.

Exit codes:
  0 — scan passed (no forbidden spawn sites)
  1 — scan failed (forbidden spawn site found; printed to stderr)
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
ALLOWED_SPAWN_SCRIPTS = {"glm_r21_launcher.py"}
# The state machine, audit, and CI scan itself may mention the binary name
# without spawning it.
NON_SPAWN_MENTIONS_OK = {
    "glm_r21_state_machine.py", "glm_r21_ci_scan_launcher_monopoly.py",
    "chatgpt_r21_independent_audit.py",
}

# A "spawn" is a line containing both `subprocess.(run|Popen|call|check_*)`
# AND the binary name. Bare string mentions (in comments/gate names) are fine.
SPAWN_RE = re.compile(
    r"subprocess\.(run|Popen|call|check_call|check_output)\s*\(", re.MULTILINE)
BIN_RE = re.compile(r"synchronized_cycle_replay", re.MULTILINE)


def scan() -> int:
    violations = []
    for path in sorted(SCRIPTS.glob("glm_r21_*.py")):
        if path.name in NON_SPAWN_MENTIONS_OK:
            continue
        text = path.read_text()
        # Find lines that have a subprocess.* call AND mention the binary.
        for i, line in enumerate(text.splitlines(), start=1):
            if SPAWN_RE.search(line) and BIN_RE.search(line):
                if path.name not in ALLOWED_SPAWN_SCRIPTS:
                    violations.append((path.name, i, line.strip()))
    if violations:
        print("FAIL: forbidden synchronized_cycle_replay spawn sites found:",
              file=sys.stderr)
        for name, lineno, line in violations:
            print(f"  scripts/{name}:{lineno}: {line}", file=sys.stderr)
        print(file=sys.stderr)
        print("Only scripts/glm_r21_launcher.py may spawn the binary "
              "(plan §2 line 57-58).", file=sys.stderr)
        return 1
    print("OK: no forbidden spawn sites; synchronized_cycle_replay is only "
          "spawned by glm_r21_launcher.py")
    return 0


if __name__ == "__main__":
    sys.exit(scan())
