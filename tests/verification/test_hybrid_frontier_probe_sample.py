import importlib.util
import json
import sqlite3
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path("scripts/hybrid_martingale_frontier_probe.py")
SPEC = importlib.util.spec_from_file_location("hybrid_probe", SCRIPT)
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)


class HybridFrontierProbeSampleTest(unittest.TestCase):
    def setUp(self):
        self.tmpdir = tempfile.TemporaryDirectory()
        self.tmp_path = Path(self.tmpdir.name)

    def tearDown(self):
        self.tmpdir.cleanup()

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

    def test_load_martingale_stream_from_replay_json(self):
        replay = {
            "portfolio_id": "demo_m",
            "symbols": ["BTCUSDT", "ETHUSDT"],
            "budget_quote": 4000.0,
            "max_capital_used_quote": 1234.0,
            "budget_blocked_legs": 0,
            "equity_curve": [
                {"timestamp_ms": 1000, "equity_quote": 1000.0},
                {"timestamp_ms": 2000, "equity_quote": 1010.0},
                {"timestamp_ms": 3000, "equity_quote": 990.0},
            ],
        }
        path = self.tmp_path / "replay.json"
        path.write_text(json.dumps(replay))
        stream = probe.load_martingale_stream(path, allocation_quote=2000.0)
        self.assertEqual(stream["name"], "martingale:demo_m")
        self.assertEqual(stream["symbols"], ["BTCUSDT", "ETHUSDT"])
        self.assertEqual(stream["max_capital_used_quote"], 1234.0)
        self.assertEqual(stream["budget_blocked_events"], 0)
        self.assertEqual(stream["points"][0]["equity_quote"], 2000.0)
        self.assertEqual(round(stream["points"][1]["equity_quote"], 4), 2020.0)
        self.assertEqual(round(stream["points"][2]["equity_quote"], 4), 1980.0)

    def test_resample_equity_curve_forward_fills_without_lookahead(self):
        points = [
            {"timestamp_ms": 1000, "equity_quote": 100.0},
            {"timestamp_ms": 3000, "equity_quote": 120.0},
        ]
        sampled = probe.resample_equity_curve(points, [500, 1000, 2000, 3000, 4000])
        self.assertEqual(
            sampled,
            [
                {"timestamp_ms": 1000, "equity_quote": 100.0},
                {"timestamp_ms": 2000, "equity_quote": 100.0},
                {"timestamp_ms": 3000, "equity_quote": 120.0},
                {"timestamp_ms": 4000, "equity_quote": 120.0},
            ],
        )

    def make_market_db(self, path, rows):
        con = sqlite3.connect(path)
        con.execute(
            "CREATE TABLE klines (symbol TEXT, market_type TEXT, timeframe TEXT, open_time INTEGER, open REAL, high REAL, low REAL, close REAL, volume REAL, close_time INTEGER)"
        )
        con.executemany("INSERT INTO klines VALUES (?,?,?,?,?,?,?,?,?,?)", rows)
        con.commit()
        con.close()

    def test_build_trend_stream_uses_previous_close_for_signal(self):
        db = self.tmp_path / "market.db"
        day = 86_400_000
        rows = []
        closes = [100, 101, 102, 103, 104, 90, 89, 88, 110, 111, 112, 113]
        for i, close in enumerate(closes):
            rows.append(("BTCUSDT", "futures_usdt_perp", "1m", i * day, close, close, close, close, 1.0, i * day + 60_000 - 1))
        self.make_market_db(db, rows)
        stream = probe.build_trend_stream(
            market_db=db,
            symbol="BTCUSDT",
            allocation_quote=1000.0,
            fast=2,
            slow=4,
            fee_bps=0.0,
        )
        self.assertEqual(stream["name"], "trend:BTCUSDT:ema2_4")
        self.assertEqual(stream["symbols"], ["BTCUSDT"])
        self.assertGreaterEqual(len(stream["points"]), 8)
        self.assertGreaterEqual(stream["points"][0]["timestamp_ms"], 4 * day)
        self.assertIs(stream["no_lookahead"], True)

    def make_funding_db(self, path, rows):
        con = sqlite3.connect(path)
        con.execute("CREATE TABLE funding_rates (symbol TEXT, funding_time INTEGER, funding_rate REAL, mark_price REAL)")
        con.executemany("INSERT INTO funding_rates VALUES (?,?,?,?)", rows)
        con.commit()
        con.close()

    def test_build_funding_stream_short_perp_receives_positive_funding(self):
        db = self.tmp_path / "funding.db"
        rows = [
            ("BTCUSDT", 1000, 0.001, 100.0),
            ("BTCUSDT", 2000, 0.001, 100.0),
            ("BTCUSDT", 3000, -0.0005, 100.0),
        ]
        self.make_funding_db(db, rows)
        stream = probe.build_funding_stream(db, "BTCUSDT", allocation_quote=1000.0, start_ms=0, end_ms=4000)
        self.assertEqual(stream["name"], "funding:BTCUSDT:short_perp")
        self.assertEqual(stream["symbols"], ["BTCUSDT"])
        self.assertEqual(round(stream["points"][-1]["equity_quote"], 4), 1001.5)
        self.assertEqual(stream["funding_events"], 3)

    def test_combine_streams_aligns_points_and_sums_capital(self):
        streams = [
            {
                "name": "a",
                "symbols": ["BTCUSDT"],
                "points": [{"timestamp_ms": 1000, "equity_quote": 100.0}, {"timestamp_ms": 2000, "equity_quote": 110.0}],
                "max_capital_used_quote": 100.0,
                "budget_blocked_events": 0,
            },
            {
                "name": "b",
                "symbols": ["ETHUSDT"],
                "points": [{"timestamp_ms": 1000, "equity_quote": 200.0}, {"timestamp_ms": 2000, "equity_quote": 190.0}],
                "max_capital_used_quote": 200.0,
                "budget_blocked_events": 0,
            },
        ]
        combined = probe.combine_streams(streams, budget=500.0)
        self.assertEqual(combined["symbols"], ["BTCUSDT", "ETHUSDT"])
        self.assertEqual(combined["points"][-1]["equity_quote"], 300.0)
        self.assertEqual(combined["metrics"]["max_capital_used_quote"], 300.0)
        self.assertEqual(combined["metrics"]["symbol_count"], 2)

    def test_slice_points_for_segment_includes_boundary_points(self):
        points = [
            {"timestamp_ms": 1000, "equity_quote": 100.0},
            {"timestamp_ms": 2000, "equity_quote": 110.0},
            {"timestamp_ms": 3000, "equity_quote": 120.0},
        ]
        sliced = probe.slice_points(points, 1500, 2500)
        self.assertEqual(sliced[0]["timestamp_ms"], 1500)
        self.assertEqual(sliced[0]["equity_quote"], 100.0)
        self.assertEqual(sliced[-1]["timestamp_ms"], 2500)
        self.assertEqual(sliced[-1]["equity_quote"], 110.0)

    def test_build_candidate_report_marks_research_only(self):
        combined = {
            "streams": ["martingale:a", "trend:BTCUSDT:ema2_4"],
            "symbols": ["BTCUSDT", "ETHUSDT"],
            "points": [
                {"timestamp_ms": 1672531200000, "equity_quote": 1000.0},
                {"timestamp_ms": 1672617600000, "equity_quote": 1010.0},
            ],
            "metrics": {
                "annualized_return_pct": 60.0,
                "max_drawdown_pct": 5.0,
                "total_return_pct": 1.0,
                "max_capital_used_quote": 3000.0,
                "budget_blocked_events": 0,
                "symbol_count": 2,
            },
            "live_parity_status": "research_only",
        }
        report = probe.build_candidate_report("conservative", combined, budget=5000.0)
        self.assertEqual(report["profile"], "conservative")
        self.assertEqual(report["live_parity_status"], "research_only")
        self.assertIs(report["full_gate"]["passes"], True)
        self.assertIn("segment_gate", report)
        self.assertIn("sleeve_attribution", report)
