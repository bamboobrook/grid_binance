import importlib.util
import sys
import unittest
from pathlib import Path
from unittest.mock import patch


SCRIPT = Path("scripts/pair_neutral_risk_control_probe.py")
SPEC = importlib.util.spec_from_file_location("pair_neutral_risk", SCRIPT)
risk = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = risk
SPEC.loader.exec_module(risk)


class PairNeutralRiskControlProbeTest(unittest.TestCase):
    def test_row_from_report_preserves_pair_and_risk_fields(self):
        report = {
            "streams": ["pair_grid:AAAUSDT:BBBUSDT"],
            "symbols": ["AAAUSDT", "BBBUSDT"],
            "full_metrics": {
                "annualized_return_pct": 51.0,
                "max_drawdown_pct": 9.0,
                "max_capital_used_quote": 1000.0,
            },
            "segment_gate": {
                "passes": True,
                "positive_segments": 5,
                "combined_2024_2026_return_pct": 10.0,
                "violations": [],
            },
            "full_gate": {"passes": True, "violations": []},
            "passes_offline": True,
            "risk_control": {"risk_events": 2},
        }

        row = risk.row_from_report("conservative", ("AAAUSDT", "BBBUSDT"), 1000.0, 80, 1.0, 10.0, 30, report)

        self.assertEqual(row["pair"], "AAAUSDT,BBBUSDT")
        self.assertEqual(row["risk"], "dd10_cd30")
        self.assertEqual(row["risk_events"], 2)
        self.assertTrue(row["pass"])

    def test_default_limit_covers_default_search_space(self):
        with patch(
            "sys.argv",
            [
                "pair_neutral_risk_control_probe.py",
                "--out-json",
                "/tmp/out.json",
                "--out-md",
                "/tmp/out.md",
            ],
        ):
            args = risk.parse_args()

        profiles = len(risk.parse_csv(args.profiles))
        allocations = len(risk.parse_csv(args.allocations))
        lookbacks = len(risk.parse_csv(args.lookbacks))
        entry_zs = len(risk.parse_csv(args.entry_zs))
        dd_stops = len(risk.parse_csv(args.dd_stops))
        cooldowns = len(risk.parse_csv(args.cooldowns))
        self.assertGreaterEqual(
            args.limit,
            profiles * args.max_pairs * allocations * lookbacks * entry_zs * dd_stops * cooldowns,
        )


if __name__ == "__main__":
    unittest.main()
