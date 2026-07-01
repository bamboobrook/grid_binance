import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path("scripts/martingale_goal_completion_audit.py")
SPEC = importlib.util.spec_from_file_location("goal_completion_audit", SCRIPT)
audit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(audit)


class MartingaleGoalCompletionAuditTest(unittest.TestCase):
    def test_zero_passes_means_original_goal_is_not_complete(self):
        target_gap = {
            "total_rows": 3,
            "final_passes": 0,
            "summary": {
                "conservative": {"passes": 0, "nearest": []},
                "balanced": {"passes": 0, "nearest": []},
                "aggressive": {"passes": 0, "nearest": []},
            },
        }
        evidence = {"total_passes": 0, "reports": []}

        result = audit.audit_completion(target_gap, evidence, external_text="", final_text="")

        self.assertFalse(result["goal_complete"])
        self.assertEqual(result["requirements"]["all_profiles_final_pass"]["status"], "failed")
        self.assertIn("0 final target passes", result["requirements"]["all_profiles_final_pass"]["evidence"])

    def test_live_ready_requirement_needs_explicit_final_candidate_evidence(self):
        target_gap = {
            "total_rows": 1,
            "final_passes": 1,
            "summary": {
                "conservative": {"passes": 1, "nearest": []},
                "balanced": {"passes": 1, "nearest": []},
                "aggressive": {"passes": 1, "nearest": []},
            },
        }
        evidence = {"total_passes": 1, "reports": []}

        result = audit.audit_completion(
            target_gap,
            evidence,
            external_text="No public external claim found.",
            final_text="No candidate should be promoted to live.",
        )

        self.assertFalse(result["goal_complete"])
        self.assertEqual(result["requirements"]["live_ready"]["status"], "failed")

    def test_main_writes_json_and_markdown_reports(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            target_gap_path = root / "target_gap.json"
            evidence_path = root / "evidence.json"
            external_path = root / "external.md"
            final_path = root / "final.md"
            out_json = root / "out.json"
            out_md = root / "out.md"
            target_gap_path.write_text(
                json.dumps(
                    {
                        "total_rows": 10,
                        "final_passes": 0,
                        "summary": {
                            "conservative": {"passes": 0, "nearest": []},
                            "balanced": {"passes": 0, "nearest": []},
                            "aggressive": {"passes": 0, "nearest": []},
                        },
                    }
                )
            )
            evidence_path.write_text(json.dumps({"total_passes": 0, "reports": []}))
            external_path.write_text("No public external claim found.")
            final_path.write_text("No qualifying martingale/grid portfolio has been found.")

            code = audit.run(
                target_gap_path=target_gap_path,
                evidence_path=evidence_path,
                external_path=external_path,
                final_path=final_path,
                out_json=out_json,
                out_md=out_md,
            )

            self.assertEqual(code, 0)
            self.assertIn("goal_complete", out_json.read_text())
            self.assertIn("Goal Complete: `False`", out_md.read_text())


if __name__ == "__main__":
    unittest.main()
