import importlib.util
import sys
import unittest
from pathlib import Path


SCRIPT = Path("scripts/pair_neutral_grid_probe.py")
SPEC = importlib.util.spec_from_file_location("pair_neutral_grid", SCRIPT)
probe = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = probe
SPEC.loader.exec_module(probe)


class PairNeutralGridProbeTest(unittest.TestCase):
    def test_zscore_returns_none_until_warmup_then_signed_distance(self):
        values = [10.0, 11.0, 12.0, 13.0]

        scores = probe.rolling_zscores(values, lookback=3)

        self.assertEqual(scores[:3], [None, None, None])
        self.assertGreater(scores[3], 0)

    def test_pair_grid_stream_uses_two_symbols_and_caps_capital(self):
        rows = [
            {"timestamp_ms": 1, "a": 100.0, "b": 100.0},
            {"timestamp_ms": 2, "a": 101.0, "b": 100.0},
            {"timestamp_ms": 3, "a": 99.0, "b": 100.0},
            {"timestamp_ms": 4, "a": 120.0, "b": 100.0},
            {"timestamp_ms": 5, "a": 100.0, "b": 100.0},
            {"timestamp_ms": 6, "a": 100.0, "b": 100.0},
        ]

        stream = probe.build_pair_grid_stream_from_rows(
            rows,
            symbol_a="AAAUSDT",
            symbol_b="BBBUSDT",
            allocation_quote=1000.0,
            lookback=3,
            entry_z=1.0,
            exit_z=0.2,
            fee_bps=0.0,
        )

        self.assertEqual(stream["symbols"], ["AAAUSDT", "BBBUSDT"])
        self.assertEqual(stream["max_capital_used_quote"], 1000.0)
        self.assertEqual(stream["budget_blocked_events"], 0)
        self.assertGreater(stream["points"][-1]["equity_quote"], 1000.0)

    def test_candidate_row_exposes_original_profile_gates(self):
        report = {
            "streams": ["pair_grid:AAAUSDT:BBBUSDT"],
            "symbols": ["AAAUSDT", "BBBUSDT"],
            "full_metrics": {
                "annualized_return_pct": 60.0,
                "max_drawdown_pct": 8.0,
                "max_capital_used_quote": 1000.0,
            },
            "segment_gate": {
                "passes": True,
                "positive_segments": 5,
                "combined_2024_2026_return_pct": 5.0,
                "violations": [],
            },
            "full_gate": {"passes": True, "violations": []},
            "passes_offline": True,
        }

        row = probe.candidate_to_row("conservative", ("AAAUSDT", "BBBUSDT"), 1000.0, 20, 1.5, report)

        self.assertTrue(row["pass"])
        self.assertEqual(row["profile"], "conservative")
        self.assertEqual(row["symbol_count"], 2)
        self.assertEqual(row["pos"], 5)


if __name__ == "__main__":
    unittest.main()
