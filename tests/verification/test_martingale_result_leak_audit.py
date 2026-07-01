import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


SCRIPT = Path("scripts/martingale_result_leak_audit.py")
SPEC = importlib.util.spec_from_file_location("leak_audit", SCRIPT)
audit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(audit)


class MartingaleResultLeakAuditTest(unittest.TestCase):
    def test_full_pass_without_segments_is_not_final_pass(self):
        row = audit.normalize_record(
            {
                "profile": "aggressive",
                "annualized_return_pct": 120.0,
                "max_drawdown_pct": 28.0,
                "max_capital_used_quote": 4000.0,
                "budget_blocked_legs": 0,
                "live_parity": True,
                "positive_segments": 1,
                "combined_2024_2026_return_pct": -60.0,
                "source": "sample.json",
            }
        )

        self.assertTrue(row["full_gate"])
        self.assertFalse(row["final_gate"])
        self.assertIn("only 1/5 positive segments", row["violations"])
        self.assertIn("2024-2026 return -60.00% <= 0", row["violations"])

    def test_complete_record_can_pass_final_gate(self):
        row = audit.normalize_record(
            {
                "profile": "balanced",
                "full_metrics": {
                    "annualized_return_pct": 91.0,
                    "max_drawdown_pct": 19.5,
                },
                "max_input_quote": 4999.0,
                "passes_offline": True,
                "live_parity_status": "research_only",
                "gate": {"violations": []},
                "segment_metrics": {
                    "h1_2023": {"total_return_pct": 1.0},
                    "h2_2023": {"total_return_pct": 2.0},
                    "2024": {"total_return_pct": 3.0},
                    "2025": {"total_return_pct": 4.0},
                    "2026_ytd": {"total_return_pct": 5.0},
                },
                "source": "dgt.json",
            }
        )

        self.assertTrue(row["full_gate"])
        self.assertTrue(row["segment_gate"])
        self.assertTrue(row["final_gate"])
        self.assertEqual(row["violations"], [])

    def test_scan_json_file_extracts_nested_rows(self):
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "result.json"
            path.write_text(
                json.dumps(
                    {
                        "rows": [
                            {
                                "profile": "conservative",
                                "full_metrics": {
                                    "annualized_return_pct": 51.0,
                                    "max_drawdown_pct": 9.5,
                                },
                                "max_input_quote": 1000.0,
                                "positive_segments": 4,
                                "combined_2024_2026_return_pct": 1.0,
                                "live_parity_status": "research_only",
                            }
                        ]
                    }
                )
            )

            rows = audit.scan_json_file(path)

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["profile"], "conservative")
        self.assertTrue(rows[0]["final_gate"])

    def test_scan_json_file_skips_non_utf8_json_artifacts(self):
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "._result.json"
            path.write_bytes(b"\xa3not valid utf-8 json")

            rows = audit.scan_json_file(path)

        self.assertEqual(rows, [])

    def test_default_roots_do_not_scan_tmp(self):
        with patch(
            "sys.argv",
            [
                "martingale_result_leak_audit.py",
                "--out-json",
                "/tmp/out.json",
                "--out-md",
                "/tmp/out.md",
            ],
        ):
            args = audit.parse_args()

        self.assertEqual(args.roots, ["docs"])


if __name__ == "__main__":
    unittest.main()
