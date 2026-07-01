import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path("scripts/dynamic_breakout_trend_probe.py")
SPEC = importlib.util.spec_from_file_location("dynamic_trend", SCRIPT)
dynamic = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = dynamic
SPEC.loader.exec_module(dynamic)


DAY = 86_400_000


def bar(day_index, open_, high, low, close):
    return {
        "timestamp_ms": day_index * DAY,
        "open": float(open_),
        "high": float(high),
        "low": float(low),
        "close": float(close),
        "volume": 1.0,
    }


def stream(name, symbol, returns):
    return {
        "name": name,
        "symbol": symbol,
        "rule": "test",
        "points": [
            {
                "timestamp_ms": index * DAY,
                "return": float(value),
                "position": 1,
                "strength": abs(float(value)),
            }
            for index, value in enumerate(returns)
        ],
        "live_parity_status": "research_only",
    }


class DynamicBreakoutTrendProbeTest(unittest.TestCase):
    def test_compress_daily_ohlc_is_deterministic(self):
        rows = [
            {"timestamp_ms": 60_000, "open": 100, "high": 102, "low": 99, "close": 101, "volume": 3},
            {"timestamp_ms": 120_000, "open": 101, "high": 105, "low": 100, "close": 104, "volume": 4},
            {"timestamp_ms": DAY + 60_000, "open": 110, "high": 112, "low": 108, "close": 111, "volume": 5},
            {"timestamp_ms": DAY + 120_000, "open": 111, "high": 113, "low": 107, "close": 109, "volume": 6},
        ]

        daily = dynamic.compress_daily_ohlc(rows)

        self.assertEqual(
            daily,
            [
                {"timestamp_ms": 0, "open": 100.0, "high": 105.0, "low": 99.0, "close": 104.0, "volume": 7.0},
                {"timestamp_ms": DAY, "open": 110.0, "high": 113.0, "low": 107.0, "close": 109.0, "volume": 11.0},
            ],
        )

    def test_momentum_stream_uses_previous_completed_day_for_next_day_position(self):
        daily = [
            bar(0, 100, 100, 100, 100),
            bar(1, 90, 90, 90, 90),
            bar(2, 110, 110, 110, 110),
            bar(3, 120, 120, 120, 120),
        ]

        result = dynamic.build_signal_stream(
            "BTCUSDT",
            daily,
            "mom1_lf",
            fee_bps=0.0,
            slippage_bps=0.0,
        )

        first = result["points"][0]
        self.assertEqual(first["timestamp_ms"], 2 * DAY)
        self.assertEqual(first["position"], 0)
        self.assertEqual(first["return"], 0.0)
        self.assertEqual(result["live_parity_status"], "research_only")

    def test_donchian_stream_channel_excludes_signal_day(self):
        daily = [
            bar(0, 100, 100, 100, 100),
            bar(1, 101, 101, 101, 101),
            bar(2, 103, 103, 103, 103),
            bar(3, 99, 99, 99, 99),
        ]

        result = dynamic.build_signal_stream(
            "ETHUSDT",
            daily,
            "donchian2_lf",
            fee_bps=0.0,
            slippage_bps=0.0,
        )

        self.assertEqual(result["points"][0]["timestamp_ms"], 3 * DAY)
        self.assertEqual(result["points"][0]["position"], 1)
        self.assertLess(result["points"][0]["return"], 0.0)


if __name__ == "__main__":
    unittest.main()
