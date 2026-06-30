import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path("scripts/hybrid_martingale_frontier_probe.py")
SPEC = importlib.util.spec_from_file_location("hybrid_probe", SCRIPT)
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)


class HybridFrontierProbeSampleTest(unittest.TestCase):
    def test_compute_metrics_positive_curve(self):
        points = [
            {"timestamp_ms": 1672531200000, "equity_quote": 1000.0},
            {"timestamp_ms": 1672617600000, "equity_quote": 1100.0},
            {"timestamp_ms": 1672704000000, "equity_quote": 1050.0},
            {"timestamp_ms": 1672790400000, "equity_quote": 1200.0},
        ]
        metrics = probe.compute_metrics(points)
        self.assertEqual(round(metrics["total_return_pct"], 4), 20.0)
        self.assertGreater(metrics["annualized_return_pct"], 1000.0)
        self.assertEqual(
            round(metrics["max_drawdown_pct"], 4),
            round((1100.0 - 1050.0) / 1100.0 * 100.0, 4),
        )

    def test_evaluate_profile_gate_enforces_budget_and_original_thresholds(self):
        passing = {
            "annualized_return_pct": 55.0,
            "max_drawdown_pct": 9.0,
            "max_capital_used_quote": 4999.0,
            "budget_blocked_events": 0,
            "symbol_count": 3,
        }
        self.assertIs(probe.evaluate_profile_gate("conservative", passing, 5000.0)["passes"], True)

        over_budget = dict(passing, max_capital_used_quote=5000.0)
        result = probe.evaluate_profile_gate("conservative", over_budget, 5000.0)
        self.assertIs(result["passes"], False)
        self.assertIn("capital 5000.00 is not below budget 5000.00", result["violations"])

        single_symbol = dict(passing, symbol_count=1)
        result = probe.evaluate_profile_gate("conservative", single_symbol, 5000.0)
        self.assertIs(result["passes"], False)
        self.assertIn("single-symbol portfolio is not allowed", result["violations"])

    def test_segment_gate_rejects_h1_only_overfit(self):
        segment_metrics = {
            "h1_2023": {"total_return_pct": 200.0, "max_drawdown_pct": 8.0},
            "h2_2023": {"total_return_pct": -10.0, "max_drawdown_pct": 8.0},
            "2024": {"total_return_pct": -20.0, "max_drawdown_pct": 8.0},
            "2025": {"total_return_pct": -30.0, "max_drawdown_pct": 8.0},
            "2026_ytd": {"total_return_pct": -5.0, "max_drawdown_pct": 8.0},
        }
        result = probe.evaluate_segment_gate("balanced", segment_metrics)
        self.assertIs(result["passes"], False)
        self.assertEqual(result["positive_segments"], 1)
        self.assertTrue(any("segments positive" in item for item in result["violations"]))
        self.assertTrue(any("2024-2026 combined return" in item for item in result["violations"]))
