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
