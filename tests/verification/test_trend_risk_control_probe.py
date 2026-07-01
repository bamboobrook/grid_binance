import importlib.util
import sys
import unittest
from pathlib import Path
from unittest.mock import patch


SCRIPT = Path("scripts/trend_risk_control_probe.py")
SPEC = importlib.util.spec_from_file_location("trend_risk", SCRIPT)
risk = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = risk
SPEC.loader.exec_module(risk)


class TrendRiskControlProbeTest(unittest.TestCase):
    def test_dd_stop_freezes_until_cooldown_expires(self):
        day = 24 * 60 * 60 * 1000
        points = [
            {"timestamp_ms": 0, "equity_quote": 100.0},
            {"timestamp_ms": day, "equity_quote": 110.0},
            {"timestamp_ms": 2 * day, "equity_quote": 90.0},
            {"timestamp_ms": 3 * day, "equity_quote": 120.0},
            {"timestamp_ms": 4 * day, "equity_quote": 130.0},
        ]

        controlled = risk.apply_dd_stop_cooldown(points, dd_stop_pct=10.0, cooldown_days=2)

        self.assertEqual(controlled[2]["equity_quote"], 90.0)
        self.assertEqual(controlled[3]["equity_quote"], 90.0)
        self.assertEqual(controlled[4]["equity_quote"], 130.0)
        self.assertEqual(controlled[-1]["risk_events"], 1)

    def test_risk_key_is_stable(self):
        self.assertEqual(risk.risk_key(12.5, 30), "dd12.5_cd30")

    def test_default_limit_covers_default_search_space(self):
        with patch(
            "sys.argv",
            [
                "trend_risk_control_probe.py",
                "--out-json",
                "/tmp/out.json",
                "--out-md",
                "/tmp/out.md",
            ],
        ):
            args = risk.parse_args()

        profiles = len(risk.parse_csv(args.profiles))
        rules = len(risk.parse_csv(args.rules))
        allocations = len(risk.parse_csv(args.allocations))
        dd_stops = len(risk.parse_csv(args.dd_stops))
        cooldowns = len(risk.parse_csv(args.cooldowns))
        self.assertGreaterEqual(args.limit, profiles * rules * args.max_groups * allocations * dd_stops * cooldowns)


if __name__ == "__main__":
    unittest.main()
