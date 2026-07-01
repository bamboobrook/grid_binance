import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path("scripts/dgt_dynamic_grid_probe.py")
SPEC = importlib.util.spec_from_file_location("dgt_probe", SCRIPT)
dgt = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(dgt)


class DgtDynamicGridProbeTest(unittest.TestCase):
    def test_simulate_symbol_tracks_downside_topup_and_research_only(self):
        bars = [
            {"timestamp_ms": 0, "open": 100.0, "high": 100.0, "low": 100.0, "close": 100.0},
            {"timestamp_ms": 86_400_000, "open": 100.0, "high": 101.0, "low": 86.0, "close": 90.0},
            {"timestamp_ms": 2 * 86_400_000, "open": 90.0, "high": 98.0, "low": 89.0, "close": 96.0},
        ]
        stream = dgt.simulate_dgt_symbol(
            "BTCUSDT",
            bars,
            principal_quote=100.0,
            grid_spacing=0.05,
            half_grid_count=1,
            fee_bps=0.0,
        )
        self.assertEqual(stream["name"], "dgt:BTCUSDT:gs0.05:h1:p100.0")
        self.assertEqual(stream["live_parity_status"], "research_only")
        self.assertGreater(stream["reset_count"], 0)
        self.assertGreater(stream["max_input_quote"], 100.0)
        self.assertEqual(len(stream["points"]), 3)

    def test_simulate_symbol_tracks_upside_reset_without_external_topup(self):
        bars = [
            {"timestamp_ms": 0, "open": 100.0, "high": 100.0, "low": 100.0, "close": 100.0},
            {"timestamp_ms": 86_400_000, "open": 100.0, "high": 112.0, "low": 99.0, "close": 110.0},
            {"timestamp_ms": 2 * 86_400_000, "open": 110.0, "high": 116.0, "low": 108.0, "close": 114.0},
        ]
        stream = dgt.simulate_dgt_symbol(
            "BTCUSDT",
            bars,
            principal_quote=100.0,
            grid_spacing=0.05,
            half_grid_count=1,
            fee_bps=0.0,
        )
        self.assertGreater(stream["reset_count"], 0)
        self.assertEqual(round(stream["max_input_quote"], 6), 100.0)
        self.assertGreater(stream["points"][-1]["equity_quote"], stream["points"][0]["equity_quote"])

    def test_profile_gate_rejects_single_symbol_and_over_budget(self):
        metrics = {
            "annualized_return_pct": 120.0,
            "max_drawdown_pct": 20.0,
            "max_input_quote": 4000.0,
            "symbol_count": 1,
            "positive_segments": 5,
            "combined_2024_2026_return_pct": 10.0,
        }
        result = dgt.evaluate_profile_gate("aggressive", metrics, budget=5000.0)
        self.assertFalse(result["passes"])
        self.assertIn("single-symbol candidate is not allowed", result["violations"])

        metrics["symbol_count"] = 2
        metrics["max_input_quote"] = 5000.0
        result = dgt.evaluate_profile_gate("aggressive", metrics, budget=5000.0)
        self.assertFalse(result["passes"])
        self.assertIn("capital 5000.00 is not below budget 5000.00", result["violations"])

    def test_segment_metrics_use_required_periods(self):
        points = [
            {"timestamp_ms": 1672531200000, "equity_quote": 100.0},
            {"timestamp_ms": 1680000000000, "equity_quote": 105.0},
            {"timestamp_ms": 1688169600000, "equity_quote": 110.0},
            {"timestamp_ms": 1695000000000, "equity_quote": 115.0},
            {"timestamp_ms": 1704067200000, "equity_quote": 120.0},
            {"timestamp_ms": 1720000000000, "equity_quote": 125.0},
            {"timestamp_ms": 1735689600000, "equity_quote": 130.0},
            {"timestamp_ms": 1750000000000, "equity_quote": 135.0},
            {"timestamp_ms": 1767225600000, "equity_quote": 140.0},
            {"timestamp_ms": 1780271999999, "equity_quote": 150.0},
        ]
        segments = dgt.compute_segment_metrics(points)
        self.assertEqual(set(segments), {"h1_2023", "h2_2023", "2024", "2025", "2026_ytd"})
        self.assertGreaterEqual(dgt.positive_segment_count(segments), 4)

    def test_combine_streams_sums_equity_and_capital(self):
        a = {
            "name": "dgt:A",
            "symbols": ["A"],
            "points": [
                {"timestamp_ms": 1, "equity_quote": 100.0},
                {"timestamp_ms": 2, "equity_quote": 110.0},
            ],
            "max_input_quote": 100.0,
            "total_fee_quote": 1.0,
        }
        b = {
            "name": "dgt:B",
            "symbols": ["B"],
            "points": [
                {"timestamp_ms": 1, "equity_quote": 200.0},
                {"timestamp_ms": 2, "equity_quote": 190.0},
            ],
            "max_input_quote": 200.0,
            "total_fee_quote": 2.0,
        }
        combined = dgt.combine_streams([a, b])
        self.assertEqual(combined["symbols"], ["A", "B"])
        self.assertEqual(combined["max_input_quote"], 300.0)
        self.assertEqual(combined["total_fee_quote"], 3.0)
        self.assertEqual(combined["points"][-1]["equity_quote"], 300.0)

    def test_combine_streams_uses_union_timestamps_with_carry_forward(self):
        a = {
            "name": "dgt:A",
            "symbols": ["A"],
            "points": [
                {"timestamp_ms": 1, "equity_quote": 100.0},
                {"timestamp_ms": 3, "equity_quote": 80.0},
            ],
            "max_input_quote": 100.0,
            "total_fee_quote": 1.0,
        }
        b = {
            "name": "dgt:B",
            "symbols": ["B"],
            "points": [
                {"timestamp_ms": 1, "equity_quote": 200.0},
                {"timestamp_ms": 2, "equity_quote": 150.0},
                {"timestamp_ms": 3, "equity_quote": 210.0},
            ],
            "max_input_quote": 200.0,
            "total_fee_quote": 2.0,
        }

        combined = dgt.combine_streams([a, b])

        self.assertEqual(
            combined["points"],
            [
                {"timestamp_ms": 1, "equity_quote": 300.0},
                {"timestamp_ms": 2, "equity_quote": 250.0},
                {"timestamp_ms": 3, "equity_quote": 290.0},
            ],
        )

    def test_combine_streams_uses_equal_timestamp_fast_path(self):
        a = {
            "name": "dgt:A",
            "symbols": ["A"],
            "points": [
                {"timestamp_ms": 1, "equity_quote": 10.0},
                {"timestamp_ms": 2, "equity_quote": 11.0},
            ],
            "max_input_quote": 10.0,
            "total_fee_quote": 1.0,
        }
        b = {
            "name": "dgt:B",
            "symbols": ["B"],
            "points": [
                {"timestamp_ms": 1, "equity_quote": 20.0},
                {"timestamp_ms": 2, "equity_quote": 21.0},
            ],
            "max_input_quote": 20.0,
            "total_fee_quote": 2.0,
        }

        self.assertTrue(dgt.streams_share_timestamps([a, b]))
        combined = dgt.combine_streams([a, b])
        self.assertEqual(
            combined["points"],
            [
                {"timestamp_ms": 1, "equity_quote": 30.0},
                {"timestamp_ms": 2, "equity_quote": 32.0},
            ],
        )

    def test_build_candidate_report_stays_research_only(self):
        combined = {
            "symbols": ["BTCUSDT", "ETHUSDT"],
            "points": [
                {"timestamp_ms": 1672531200000, "equity_quote": 100.0},
                {"timestamp_ms": 1688169600000, "equity_quote": 120.0},
                {"timestamp_ms": 1704067200000, "equity_quote": 140.0},
                {"timestamp_ms": 1735689600000, "equity_quote": 170.0},
                {"timestamp_ms": 1767225600000, "equity_quote": 190.0},
                {"timestamp_ms": 1780271999999, "equity_quote": 230.0},
            ],
            "max_input_quote": 300.0,
            "total_fee_quote": 3.0,
            "live_parity_status": "research_only",
        }
        report = dgt.build_candidate_report("aggressive", combined, budget=5000.0, meta={"tag": "x"})
        self.assertEqual(report["live_parity_status"], "research_only")
        self.assertEqual(report["meta"]["tag"], "x")
        self.assertIn("passes_offline", report)
        self.assertIn("segment_metrics", report)

    def test_scaled_stream_matches_direct_principal_simulation(self):
        bars = [
            {"timestamp_ms": 0, "open": 100.0, "high": 100.0, "low": 100.0, "close": 100.0},
            {"timestamp_ms": 86_400_000, "open": 100.0, "high": 112.0, "low": 96.0, "close": 108.0},
            {"timestamp_ms": 2 * 86_400_000, "open": 108.0, "high": 109.0, "low": 91.0, "close": 94.0},
            {"timestamp_ms": 3 * 86_400_000, "open": 94.0, "high": 104.0, "low": 90.0, "close": 102.0},
        ]
        unit = dgt.simulate_dgt_symbol("BTCUSDT", bars, 1.0, 0.05, 1, fee_bps=8.0)
        scaled = dgt.scale_dgt_stream(unit, 100.0)
        direct = dgt.simulate_dgt_symbol("BTCUSDT", bars, 100.0, 0.05, 1, fee_bps=8.0)

        self.assertEqual(scaled["name"], direct["name"])
        self.assertEqual(scaled["reset_count"], direct["reset_count"])
        self.assertEqual(scaled["principal_quote"], direct["principal_quote"])
        self.assertAlmostEqual(scaled["max_input_quote"], direct["max_input_quote"], places=6)
        self.assertAlmostEqual(scaled["total_fee_quote"], direct["total_fee_quote"], places=6)
        self.assertEqual(len(scaled["points"]), len(direct["points"]))
        for scaled_point, direct_point in zip(scaled["points"], direct["points"]):
            self.assertEqual(scaled_point["timestamp_ms"], direct_point["timestamp_ms"])
            self.assertAlmostEqual(scaled_point["equity_quote"], direct_point["equity_quote"], places=6)

    def test_simulate_symbol_compacts_intraday_points_to_daily_equity(self):
        bars = [
            {"timestamp_ms": 0, "open": 100.0, "high": 101.0, "low": 99.0, "close": 100.0},
            {"timestamp_ms": 60_000, "open": 100.0, "high": 102.0, "low": 99.0, "close": 101.0},
            {"timestamp_ms": 120_000, "open": 101.0, "high": 103.0, "low": 100.0, "close": 102.0},
            {"timestamp_ms": 86_400_000, "open": 102.0, "high": 103.0, "low": 101.0, "close": 102.0},
            {"timestamp_ms": 86_460_000, "open": 102.0, "high": 104.0, "low": 101.0, "close": 103.0},
        ]

        stream = dgt.simulate_dgt_symbol(
            "BTCUSDT",
            bars,
            principal_quote=100.0,
            grid_spacing=0.05,
            half_grid_count=1,
            fee_bps=0.0,
        )

        self.assertEqual(
            [point["timestamp_ms"] for point in stream["points"]],
            [0, 120_000, 86_400_000, 86_460_000],
        )

    def test_compact_equity_points_preserves_daily_extremes(self):
        points = [
            {"timestamp_ms": 0, "equity_quote": 100.0},
            {"timestamp_ms": 60_000, "equity_quote": 130.0},
            {"timestamp_ms": 120_000, "equity_quote": 70.0},
            {"timestamp_ms": 180_000, "equity_quote": 90.0},
            {"timestamp_ms": 240_000, "equity_quote": 110.0},
        ]

        compacted = dgt.compact_equity_points(points)

        self.assertEqual(
            compacted,
            [
                {"timestamp_ms": 0, "equity_quote": 100.0},
                {"timestamp_ms": 60_000, "equity_quote": 130.0},
                {"timestamp_ms": 120_000, "equity_quote": 70.0},
                {"timestamp_ms": 240_000, "equity_quote": 110.0},
            ],
        )
