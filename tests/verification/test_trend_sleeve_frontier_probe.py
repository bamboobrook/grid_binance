import importlib.util
import sys
import unittest
from pathlib import Path


SCRIPT = Path("scripts/trend_sleeve_frontier_probe.py")
SPEC = importlib.util.spec_from_file_location("trend_frontier", SCRIPT)
frontier = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = frontier
SPEC.loader.exec_module(frontier)


class TrendSleeveFrontierProbeTest(unittest.TestCase):
    def test_even_weight_allocations_stay_under_budget(self):
        allocations = frontier.even_weight_allocations(["BTCUSDT", "ETHUSDT", "SOLUSDT"], 3000.0)

        self.assertEqual(allocations, {"BTCUSDT": 1000.0, "ETHUSDT": 1000.0, "SOLUSDT": 1000.0})

    def test_single_symbol_group_is_rejected(self):
        self.assertFalse(frontier.group_is_allowed(("BTCUSDT",)))
        self.assertTrue(frontier.group_is_allowed(("BTCUSDT", "ETHUSDT")))

    def test_candidate_row_exposes_profile_gates(self):
        report = {
            "streams": ["trend:BTCUSDT:mom20", "trend:ETHUSDT:mom20"],
            "symbols": ["BTCUSDT", "ETHUSDT"],
            "full_metrics": {
                "annualized_return_pct": 55.0,
                "max_drawdown_pct": 9.0,
                "max_capital_used_quote": 3000.0,
            },
            "segment_gate": {
                "passes": True,
                "positive_segments": 5,
                "combined_2024_2026_return_pct": 10.0,
                "violations": [],
            },
            "full_gate": {"passes": True, "violations": []},
            "passes_offline": True,
        }

        row = frontier.candidate_to_row("conservative", "mom20_ls", ("BTCUSDT", "ETHUSDT"), 3000.0, report)

        self.assertTrue(row["pass"])
        self.assertEqual(row["profile"], "conservative")
        self.assertEqual(row["symbol_count"], 2)
        self.assertEqual(row["pos"], 5)

    def test_scale_stream_scales_points_and_capital(self):
        stream = {
            "points": [
                {"timestamp_ms": 1, "equity_quote": 1.0},
                {"timestamp_ms": 2, "equity_quote": 1.2},
            ],
            "max_capital_used_quote": 1.0,
            "budget_blocked_events": 0,
        }

        scaled = frontier.scale_stream(stream, 250.0)

        self.assertEqual(scaled["points"][1]["equity_quote"], 300.0)
        self.assertEqual(scaled["max_capital_used_quote"], 250.0)


if __name__ == "__main__":
    unittest.main()
