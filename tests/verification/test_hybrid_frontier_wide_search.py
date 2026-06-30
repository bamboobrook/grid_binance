import importlib.util
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
