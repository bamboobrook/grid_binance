import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path("scripts/martingale_live_promotion_gate_audit.py")
SPEC = importlib.util.spec_from_file_location("live_promotion_gate", SCRIPT)
audit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(audit)


class MartingaleLivePromotionGateAuditTest(unittest.TestCase):
    def test_zero_final_passes_blocks_live_promotion_even_with_approval_flag(self):
        goal = {
            "goal_complete": False,
            "requirements": {
                "all_profiles_final_pass": {"status": "failed", "evidence": "0 final target passes"},
                "live_ready": {"status": "failed", "evidence": "no candidate should be promoted"},
            },
        }
        evidence = {"total_passes": 0, "reports": []}

        result = audit.audit_promotion_gate(
            goal,
            evidence,
            final_text="No candidate should be promoted to live.",
            explicit_live_approval=True,
        )

        self.assertFalse(result["promotion_allowed"])
        self.assertEqual(result["live_parity_status"], "research_only")
        self.assertIn("goal_complete is not true", result["blocking_reasons"])
        self.assertIn("machine index has 0 final/pass rows", result["blocking_reasons"])

    def test_only_all_live_gates_open_allow_promotion(self):
        goal = {
            "goal_complete": True,
            "requirements": {
                "all_profiles_final_pass": {"status": "passed", "evidence": "1 final target pass"},
                "live_ready": {"status": "passed", "evidence": "live parity candidate verified"},
            },
        }
        evidence = {"total_passes": 1, "reports": [{"name": "candidate"}]}

        result = audit.audit_promotion_gate(
            goal,
            evidence,
            final_text="Live-ready candidate verified. Promotion may proceed after explicit approval.",
            explicit_live_approval=True,
        )

        self.assertTrue(result["promotion_allowed"])
        self.assertEqual(result["blocking_reasons"], [])

    def test_main_writes_json_and_markdown_reports(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            goal_path = root / "goal.json"
            evidence_path = root / "evidence.json"
            final_path = root / "final.md"
            out_json = root / "out.json"
            out_md = root / "out.md"
            goal_path.write_text(
                json.dumps(
                    {
                        "goal_complete": False,
                        "requirements": {
                            "all_profiles_final_pass": {
                                "status": "failed",
                                "evidence": "0 final target passes",
                            }
                        },
                    }
                )
            )
            evidence_path.write_text(json.dumps({"total_passes": 0, "reports": []}))
            final_path.write_text("No qualifying martingale/grid portfolio has been found.")

            code = audit.run(
                goal_json_path=goal_path,
                evidence_json_path=evidence_path,
                final_md_path=final_path,
                out_json=out_json,
                out_md=out_md,
                explicit_live_approval=False,
            )

            self.assertEqual(code, 0)
            result = json.loads(out_json.read_text())
            self.assertFalse(result["promotion_allowed"])
            self.assertIn("Promotion Allowed: `False`", out_md.read_text())
            self.assertIn("research_only", out_md.read_text())


if __name__ == "__main__":
    unittest.main()
