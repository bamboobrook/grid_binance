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

    def test_rank_streams_excludes_current_and_future_returns(self):
        weak_future = stream("trend:BTCUSDT:test", "BTCUSDT", [-0.02, -0.02, 0.80, 0.80])
        steady_past = stream("trend:ETHUSDT:test", "ETHUSDT", [0.03, 0.02, -0.10, -0.10])

        ranked = dynamic.rank_streams([weak_future, steady_past], as_of_ts=2 * DAY, lookback_days=10)

        self.assertEqual(ranked[0]["name"], "trend:ETHUSDT:test")
        self.assertGreater(ranked[0]["score"], ranked[1]["score"])

    def test_select_top_streams_enforces_two_symbols_when_possible(self):
        ranked = [
            {"name": "trend:BTCUSDT:a", "symbol": "BTCUSDT", "score": 10.0},
            {"name": "trend:BTCUSDT:b", "symbol": "BTCUSDT", "score": 9.0},
            {"name": "trend:ETHUSDT:a", "symbol": "ETHUSDT", "score": 8.0},
        ]

        selected = dynamic.select_top_streams(ranked, top_n=2, max_symbol_weight=0.5, min_symbols=2)

        self.assertEqual([item["name"] for item in selected], ["trend:BTCUSDT:a", "trend:ETHUSDT:a"])
        self.assertEqual({item["symbol"] for item in selected}, {"BTCUSDT", "ETHUSDT"})

    def test_capped_equal_weights_rejects_single_symbol_concentration(self):
        selected = [
            {"name": "trend:BTCUSDT:a", "symbol": "BTCUSDT", "score": 10.0},
            {"name": "trend:ETHUSDT:a", "symbol": "ETHUSDT", "score": 9.0},
            {"name": "trend:SOLUSDT:a", "symbol": "SOLUSDT", "score": 8.0},
        ]

        weights = dynamic.capped_equal_weights(selected, max_symbol_weight=0.5)

        self.assertAlmostEqual(sum(weights.values()), 1.0)
        self.assertLessEqual(weights["trend:BTCUSDT:a"], 0.5)
        self.assertLessEqual(weights["trend:ETHUSDT:a"], 0.5)
        self.assertLessEqual(weights["trend:SOLUSDT:a"], 0.5)

    def test_volatility_target_scales_down_high_realized_volatility(self):
        returns = [0.10, -0.10, 0.08, -0.08, 0.09, -0.09]

        scale = dynamic.volatility_scale(returns, target_vol_pct=20.0)

        self.assertGreater(scale, 0.0)
        self.assertLess(scale, 1.0)

    def test_dynamic_portfolio_dd_stop_freezes_exposure_and_records_event(self):
        streams = [
            stream("trend:BTCUSDT:a", "BTCUSDT", [0.05, 0.05, -0.30, 0.40, 0.10]),
            stream("trend:ETHUSDT:a", "ETHUSDT", [0.04, 0.04, -0.20, 0.30, 0.10]),
        ]

        portfolio = dynamic.build_dynamic_portfolio(
            streams,
            allocation_quote=3000.0,
            rebalance_days=1,
            score_lookback_days=2,
            top_n=2,
            max_symbol_weight=0.5,
            target_vol_pct=100.0,
            vol_lookback_days=2,
            dd_stop_pct=10.0,
            cooldown_days=2,
        )

        self.assertEqual(portfolio["risk_events"], 1)
        frozen_points = [point for point in portfolio["points"] if point["in_cooldown"]]
        self.assertGreaterEqual(len(frozen_points), 1)
        self.assertEqual(frozen_points[0]["gross_weight"], 0.0)
        self.assertEqual(portfolio["live_parity_status"], "research_only")

    def test_gate_evaluation_rejects_high_drawdown_even_with_high_return(self):
        points = [
            {"timestamp_ms": dynamic.hybrid.SEGMENTS["full"][0], "equity_quote": 3000.0},
            {"timestamp_ms": dynamic.hybrid.SEGMENTS["full"][0] + DAY, "equity_quote": 6000.0},
            {"timestamp_ms": dynamic.hybrid.SEGMENTS["full"][0] + 2 * DAY, "equity_quote": 3600.0},
        ]
        portfolio = {
            "streams": ["trend:BTCUSDT:a", "trend:ETHUSDT:a"],
            "symbols": ["BTCUSDT", "ETHUSDT"],
            "points": points,
            "metrics": {
                **dynamic.hybrid.compute_metrics(points),
                "max_capital_used_quote": 3000.0,
                "budget_blocked_events": 0,
                "symbol_count": 2,
                "max_symbol_weight_observed": 0.5,
                "risk_events": 0,
            },
            "live_parity_status": "research_only",
        }

        report = dynamic.build_candidate_report("conservative", portfolio, budget=5000.0, max_symbol_weight=0.5)

        self.assertFalse(report["passes_offline"])
        self.assertIn("drawdown", " ".join(report["full_gate"]["violations"]))

    def test_row_from_report_preserves_research_only_and_segment_fields(self):
        report = {
            "profile": "balanced",
            "live_parity_status": "research_only",
            "streams": ["trend:BTCUSDT:a", "trend:ETHUSDT:a"],
            "symbols": ["BTCUSDT", "ETHUSDT"],
            "full_metrics": {
                "annualized_return_pct": 95.0,
                "max_drawdown_pct": 15.0,
                "max_capital_used_quote": 3000.0,
                "max_symbol_weight_observed": 0.5,
                "risk_events": 1,
            },
            "segment_gate": {
                "passes": True,
                "positive_segments": 4,
                "combined_2024_2026_return_pct": 12.0,
                "violations": [],
            },
            "full_gate": {"passes": True, "violations": []},
            "passes_offline": True,
            "config": {"top_n": 2, "rebalance_days": 7},
        }

        row = dynamic.row_from_report(report)

        self.assertEqual(row["live_parity_status"], "research_only")
        self.assertEqual(row["symbol_count"], 2)
        self.assertEqual(row["pos"], 4)
        self.assertTrue(row["pass"])

    def test_summarize_counts_profile_passes(self):
        rows = [
            {"profile": "conservative", "pass": False, "ann": 40.0, "dd": 8.0},
            {"profile": "conservative", "pass": True, "ann": 55.0, "dd": 9.0},
            {"profile": "balanced", "pass": False, "ann": 80.0, "dd": 18.0},
        ]

        summary = dynamic.summarize(rows)

        self.assertEqual(summary["conservative"]["rows"], 2)
        self.assertEqual(summary["conservative"]["passes"], 1)
        self.assertEqual(summary["balanced"]["passes"], 0)

    def test_write_outputs_marks_research_only(self):
        result = {
            "live_parity_status": "research_only",
            "rows": [
                {
                    "profile": "conservative",
                    "pass": False,
                    "ann": 40.0,
                    "dd": 8.0,
                    "cap": 3000.0,
                    "pos": 4,
                    "c2426": 2.0,
                    "symbols": "BTCUSDT,ETHUSDT",
                    "top_n": 2,
                    "rebalance_days": 7,
                    "target_vol_pct": 20.0,
                    "dd_stop_pct": 10.0,
                    "cooldown_days": 15,
                    "risk_events": 0,
                }
            ],
            "summary": {
                "conservative": {"rows": 1, "passes": 0, "best_ann": None, "best_dd": None, "passes_rows": []},
                "balanced": {"rows": 0, "passes": 0, "best_ann": None, "best_dd": None, "passes_rows": []},
                "aggressive": {"rows": 0, "passes": 0, "best_ann": None, "best_dd": None, "passes_rows": []},
            },
        }
        with tempfile.TemporaryDirectory() as tmp:
            out_json = Path(tmp) / "out.json"
            out_md = Path(tmp) / "out.md"

            dynamic.write_outputs(result, out_json, out_md)

            payload = json.loads(out_json.read_text())
            text = out_md.read_text()
            self.assertEqual(payload["live_parity_status"], "research_only")
            self.assertIn("research-only", text)
            self.assertIn("live_parity_status: `research_only`", text)
            self.assertNotIn("live_parity_passed", text)


if __name__ == "__main__":
    unittest.main()
