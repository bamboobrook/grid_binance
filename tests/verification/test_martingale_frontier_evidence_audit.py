import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path("scripts/martingale_frontier_evidence_audit.py")
SPEC = importlib.util.spec_from_file_location("frontier_audit", SCRIPT)
audit = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = audit
SPEC.loader.exec_module(audit)


class MartingaleFrontierEvidenceAuditTest(unittest.TestCase):
    def test_parse_probe_report_counts_rows_and_passes(self):
        text = """# Report
- rows: `1200`
## conservative
- passes: `0`
## balanced
- passes: `1`
## aggressive
- passes: `0`
"""

        parsed = audit.parse_probe_report("trend", text)

        self.assertEqual(parsed["rows"], 1200)
        self.assertEqual(parsed["passes"], {"conservative": 0, "balanced": 1, "aggressive": 0})
        self.assertEqual(parsed["total_passes"], 1)

    def test_audit_reports_zero_total_passes(self):
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "probe.md"
            path.write_text("# Report\n- rows: `30`\n- Conservative passes: `0`\n- Balanced passes: `0`\n- Aggressive passes: `0`\n")

            result = audit.audit_reports([("funding", path)])

        self.assertEqual(result["total_passes"], 0)
        self.assertEqual(result["reports"][0]["rows"], 30)

    def test_json_like_records_takes_precedence_over_pass_like_rows(self):
        text = """# Report
- JSON-like records scanned: `18485`
- Full-gate pass-like rows: `19`
- Final-gate pass rows: `0`
"""

        parsed = audit.parse_probe_report("leak", text)

        self.assertEqual(parsed["rows"], 18485)


if __name__ == "__main__":
    unittest.main()
