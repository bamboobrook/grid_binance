import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path("scripts/martingale_target_gap_audit.py")
SPEC = importlib.util.spec_from_file_location("target_gap_audit", SCRIPT)
audit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(audit)


class MartingaleTargetGapAuditTest(unittest.TestCase):
    def test_normalizes_trend_and_funding_rows_into_profile_rows(self):
        trend = audit.normalize_candidate(
            "trend_sleeve",
            {
                "profile": "aggressive",
                "ann": 41.9,
                "dd": 36.2,
                "cap": 2000,
                "pos": 4,
                "c2426": 116.0,
                "rule": "mom60_ls",
                "symbols": "ETHUSDT,BNBUSDT",
            },
        )
        funding = audit.normalize_candidate(
            "funding_sleeve",
            {
                "annualized_return_pct": 9.3,
                "max_drawdown_pct": 0.2,
                "positive_segments": 5,
                "combined_2024_2026_return_pct": 22.3,
                "symbol": "DYDXUSDT",
                "side": "short",
            },
            profile="conservative",
        )

        self.assertEqual(trend["profile"], "aggressive")
        self.assertEqual(trend["annualized_return_pct"], 41.9)
        self.assertEqual(trend["max_drawdown_pct"], 36.2)
        self.assertEqual(trend["label"], "mom60_ls ETHUSDT,BNBUSDT")
        self.assertEqual(funding["profile"], "conservative")
        self.assertEqual(funding["label"], "funding DYDXUSDT short")
        self.assertEqual(funding["capital_quote"], 1000.0)

    def test_gap_score_separates_return_drawdown_capital_and_segments(self):
        row = audit.score_candidate(
            {
                "source": "sample",
                "profile": "balanced",
                "annualized_return_pct": 72.0,
                "max_drawdown_pct": 27.5,
                "capital_quote": 6200.0,
                "positive_segments": 3,
                "combined_2024_2026_return_pct": -4.0,
                "label": "sample row",
            }
        )

        self.assertEqual(row["ann_gap"], 18.0)
        self.assertEqual(row["dd_excess"], 7.5)
        self.assertEqual(row["capital_excess"], 1200.0)
        self.assertEqual(row["segment_gap"], 1)
        self.assertEqual(row["c2426_gap"], 4.0)
        self.assertGreater(row["gap_score"], 0)
        self.assertFalse(row["target_pass"])

    def test_audits_json_sources_and_picks_nearest_rows_per_profile(self):
        with tempfile.TemporaryDirectory() as td:
            trend_path = Path(td) / "trend.json"
            funding_path = Path(td) / "funding.json"
            trend_path.write_text(
                json.dumps(
                    {
                        "rows": [
                            {
                                "profile": "aggressive",
                                "ann": 42.0,
                                "dd": 36.0,
                                "cap": 2000.0,
                                "pos": 4,
                                "c2426": 100.0,
                                "rule": "mom",
                                "symbols": "BTCUSDT,ETHUSDT",
                            },
                            {
                                "profile": "aggressive",
                                "ann": 10.0,
                                "dd": 5.0,
                                "cap": 1000.0,
                                "pos": 5,
                                "c2426": 20.0,
                                "rule": "slow",
                                "symbols": "BTCUSDT",
                            },
                        ]
                    }
                )
            )
            funding_path.write_text(
                json.dumps(
                    {
                        "rows": [
                            {
                                "annualized_return_pct": 9.0,
                                "max_drawdown_pct": 0.2,
                                "positive_segments": 5,
                                "combined_2024_2026_return_pct": 20.0,
                                "symbol": "DYDXUSDT",
                                "side": "short",
                            }
                        ]
                    }
                )
            )

            result = audit.audit_sources(
                [
                    ("trend_sleeve", trend_path),
                    ("funding_sleeve", funding_path),
                ]
            )

        self.assertEqual(result["total_rows"], 5)
        self.assertEqual(result["final_passes"], 0)
        self.assertEqual(result["summary"]["aggressive"]["nearest"][0]["label"], "mom BTCUSDT,ETHUSDT")
        self.assertEqual(result["summary"]["conservative"]["nearest"][0]["label"], "funding DYDXUSDT short")

    def test_pair_neutral_grid_rows_are_normalized(self):
        row = audit.normalize_candidate(
            "pair_neutral_grid",
            {
                "profile": "balanced",
                "pair": "BNBUSDT,SOLUSDT",
                "ann": 54.4,
                "dd": 23.6,
                "cap": 1000.0,
                "pos": 5,
                "c2426": 142.5,
            },
        )

        self.assertEqual(row["label"], "BNBUSDT,SOLUSDT")
        self.assertEqual(row["profile"], "balanced")

    def test_frontier_bounds_show_return_limit_and_drawdown_cost(self):
        rows = [
            audit.score_candidate(
                {
                    "source": "sample",
                    "profile": "balanced",
                    "annualized_return_pct": 54.0,
                    "max_drawdown_pct": 18.0,
                    "capital_quote": 1000.0,
                    "positive_segments": 5,
                    "combined_2024_2026_return_pct": 10.0,
                    "label": "low-dd",
                }
            ),
            audit.score_candidate(
                {
                    "source": "sample",
                    "profile": "balanced",
                    "annualized_return_pct": 92.0,
                    "max_drawdown_pct": 27.0,
                    "capital_quote": 1000.0,
                    "positive_segments": 5,
                    "combined_2024_2026_return_pct": 10.0,
                    "label": "target-ann",
                }
            ),
        ]

        bounds = audit.frontier_bounds(rows)

        self.assertEqual(bounds["balanced"]["max_ann_within_dd"]["annualized_return_pct"], 54.0)
        self.assertEqual(bounds["balanced"]["min_dd_at_target_ann"]["max_drawdown_pct"], 27.0)


if __name__ == "__main__":
    unittest.main()
