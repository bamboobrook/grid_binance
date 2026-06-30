import importlib.util
import sqlite3
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path("scripts/hybrid_frontier_wide_search.py")
SPEC = importlib.util.spec_from_file_location("wide_search", SCRIPT)
wide = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(wide)


class HybridFrontierWideSearchTest(unittest.TestCase):
    def test_parse_csv_strips_empty_items(self):
        self.assertEqual(wide.parse_csv(" BTCUSDT, ,ETHUSDT "), ["BTCUSDT", "ETHUSDT"])

    def test_scale_stream_scales_equity_and_capital(self):
        stream = {
            "name": "trend:BTC",
            "symbols": ["BTCUSDT"],
            "points": [
                {"timestamp_ms": 1, "equity_quote": 1.0},
                {"timestamp_ms": 2, "equity_quote": 1.1},
            ],
            "max_capital_used_quote": 1.0,
            "budget_blocked_events": 0,
        }
        scaled = wide.scale_stream(stream, 500.0)
        self.assertEqual(scaled["points"][0]["equity_quote"], 500.0)
        self.assertEqual(round(scaled["points"][1]["equity_quote"], 4), 550.0)
        self.assertEqual(scaled["max_capital_used_quote"], 500.0)

    def test_top_frontier_tracks_passes_and_near_misses(self):
        rows = [
            {"profile": "aggressive", "pass": False, "seg_pass": True, "cap": 4000, "ann": 30, "dd": 10},
            {"profile": "aggressive", "pass": False, "seg_pass": True, "cap": 4000, "ann": 40, "dd": 20},
            {"profile": "aggressive", "pass": True, "seg_pass": True, "cap": 4000, "ann": 120, "dd": 25},
        ]
        summary = wide.summarize_frontier(rows)
        self.assertEqual(summary["aggressive"]["passes"], 1)
        self.assertEqual(summary["aggressive"]["best_ann_seg_cap"]["ann"], 120)
        self.assertEqual(summary["aggressive"]["top_near_misses"][0]["ann"], 40)

    def test_candidate_to_row_keeps_gate_fields(self):
        report = {
            "profile": "balanced",
            "passes_offline": False,
            "full_metrics": {"annualized_return_pct": 12.5, "max_drawdown_pct": 8.0, "max_capital_used_quote": 3000},
            "full_gate": {"passes": False, "violations": ["annualized 12.5 <= required 90.0"]},
            "segment_gate": {"passes": True, "positive_segments": 4, "combined_2024_2026_return_pct": 10.0, "violations": []},
        }
        row = wide.candidate_to_row(report, {"profile": "balanced", "tag": "x"})
        self.assertEqual(row["ann"], 12.5)
        self.assertEqual(row["dd"], 8.0)
        self.assertEqual(row["cap"], 3000)
        self.assertFalse(row["pass"])
        self.assertTrue(row["seg_pass"])

    def test_build_trend_key_names_rule_and_symbol(self):
        self.assertEqual(wide.trend_key("mom20_ls", "BTCUSDT"), "mom20_ls:BTCUSDT")

    def test_build_rule_stream_dispatches_momentum_rule(self):
        with tempfile.TemporaryDirectory() as tmp_dir:
            db = Path(tmp_dir) / "market.db"
            con = sqlite3.connect(db)
            con.execute(
                "CREATE TABLE klines (symbol TEXT, market_type TEXT, timeframe TEXT, open_time INTEGER, open REAL, high REAL, low REAL, close REAL, volume REAL, close_time INTEGER)"
            )
            day = 86_400_000
            rows = [
                ("BTCUSDT", "futures_usdt_perp", "1m", i * day, close, close, close, close, 1.0, i * day + 60_000 - 1)
                for i, close in enumerate([100, 105, 110, 115, 120, 125, 130, 135, 140, 145, 150, 155, 160, 165, 170, 175, 180, 185, 190, 195, 200, 205, 210])
            ]
            con.executemany("INSERT INTO klines VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)", rows)
            con.commit()
            con.close()

            stream = wide.build_rule_stream(db, "BTCUSDT", "mom20_ls")

        self.assertEqual(stream["name"], "trend:BTCUSDT:mom20_long_short")
