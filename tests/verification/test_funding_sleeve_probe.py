import importlib.util
import sys
import unittest
from pathlib import Path


SCRIPT = Path("scripts/funding_sleeve_probe.py")
SPEC = importlib.util.spec_from_file_location("funding_probe", SCRIPT)
probe = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = probe
SPEC.loader.exec_module(probe)


class FundingSleeveProbeTest(unittest.TestCase):
    def test_short_perp_receives_positive_funding(self):
        events = [
            probe.FundingEvent("BTCUSDT", 0, 0.001),
            probe.FundingEvent("BTCUSDT", 8 * 60 * 60 * 1000, -0.0002),
        ]

        curve = probe.build_symbol_curve(events, side="short")

        self.assertAlmostEqual(curve[-1].equity, 1.0008)

    def test_segment_summary_uses_only_points_in_range(self):
        day = 24 * 60 * 60 * 1000
        curve = [
            probe.EquityPoint(0, 1.0),
            probe.EquityPoint(day, 1.1),
            probe.EquityPoint(2 * day, 1.05),
            probe.EquityPoint(3 * day, 1.2),
        ]

        summary = probe.summarize_curve(curve, 0, 3 * day)

        self.assertAlmostEqual(summary["total_return_pct"], 20.0)
        self.assertAlmostEqual(summary["max_drawdown_pct"], 4.545454545454541)
        self.assertGreater(summary["annualized_return_pct"], 0.0)

    def test_gate_rejects_low_funding_yield(self):
        result = {
            "annualized_return_pct": 12.0,
            "max_drawdown_pct": 0.5,
            "positive_segments": 5,
            "combined_2024_2026_return_pct": 20.0,
        }

        self.assertFalse(probe.evaluate_profile_gate(result, "conservative"))


if __name__ == "__main__":
    unittest.main()
