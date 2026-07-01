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

    def test_default_reports_include_external_claim_gate_matrix(self):
        names = [name for name, _path in audit.DEFAULT_REPORTS]

        self.assertIn("external_claim_gate_matrix", names)

    def test_default_reports_include_target_gap_audit(self):
        names = [name for name, _path in audit.DEFAULT_REPORTS]

        self.assertIn("target_gap_audit", names)

    def test_default_reports_include_goal_completion_audit(self):
        names = [name for name, _path in audit.DEFAULT_REPORTS]

        self.assertIn("goal_completion_audit", names)

    def test_default_reports_include_pair_neutral_grid_probe(self):
        names = [name for name, _path in audit.DEFAULT_REPORTS]

        self.assertIn("pair_neutral_grid", names)

    def test_default_reports_include_pair_neutral_risk_control_probe(self):
        names = [name for name, _path in audit.DEFAULT_REPORTS]

        self.assertIn("pair_neutral_risk_control", names)

    def test_default_reports_include_pair_neutral_portfolio_probe(self):
        names = [name for name, _path in audit.DEFAULT_REPORTS]

        self.assertIn("pair_neutral_portfolio", names)


if __name__ == "__main__":
    unittest.main()
