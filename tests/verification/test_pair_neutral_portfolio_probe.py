import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path("scripts/pair_neutral_portfolio_probe.py")
SPEC = importlib.util.spec_from_file_location("pair_neutral_portfolio", SCRIPT)
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)


def make_stream(name, symbols, equities, capital=500.0):
    return {
        "name": name,
        "kind": "pair_neutral_grid",
        "symbols": symbols,
        "points": [
            {"timestamp_ms": 1672531200000 + index * probe.hybrid.MS_PER_DAY, "equity_quote": equity}
            for index, equity in enumerate(equities)
        ],
        "max_capital_used_quote": capital,
        "budget_blocked_events": 0,
        "live_parity_status": "research_only",
    }


class PairNeutralPortfolioProbeTest(unittest.TestCase):
    def test_symbol_overlap_limit_rejects_reused_symbol(self):
        streams = [
            make_stream("ab", ["A", "B"], [100.0, 110.0], 500.0),
            make_stream("ac", ["A", "C"], [100.0, 108.0], 500.0),
        ]

        self.assertFalse(probe.symbol_overlap_ok(streams, max_symbol_uses=1))
        self.assertTrue(probe.symbol_overlap_ok(streams, max_symbol_uses=2))

    def test_portfolio_key_includes_stream_names(self):
        streams = [
            make_stream("pair_grid:A:B:lb20:z1.0", ["A", "B"], [100.0, 110.0]),
            make_stream("pair_grid:C:D:lb40:z1.5", ["C", "D"], [100.0, 105.0]),
        ]

        key = probe.portfolio_key(streams)

        self.assertIn("pair_grid:A:B:lb20:z1.0", key)
        self.assertIn("pair_grid:C:D:lb40:z1.5", key)

    def test_build_portfolio_combines_equity_and_capital(self):
        streams = [
            make_stream("ab", ["A", "B"], [100.0, 110.0], 500.0),
            make_stream("cd", ["C", "D"], [100.0, 105.0], 700.0),
        ]

        combined = probe.build_portfolio(streams, budget=5000.0)

        self.assertEqual(combined["symbols"], ["A", "B", "C", "D"])
        self.assertEqual(combined["metrics"]["max_capital_used_quote"], 1200.0)
        self.assertEqual(
            [point["equity_quote"] for point in combined["points"]],
            [200.0, 215.0],
        )
        self.assertEqual(combined["live_parity_status"], "research_only")

    def test_row_from_report_includes_portfolio_fields_and_gap_score(self):
        streams = [
            make_stream("ab", ["A", "B"], [100.0, 110.0], 500.0),
            make_stream("cd", ["C", "D"], [100.0, 105.0], 700.0),
        ]
        report = {
            "passes_offline": False,
            "full_gate": {"passes": False, "violations": ["drawdown 11 > allowed 10"]},
            "segment_gate": {
                "passes": True,
                "violations": [],
                "positive_segments": 4,
                "combined_2024_2026_return_pct": 25.0,
            },
            "full_metrics": {
                "annualized_return_pct": 49.0,
                "max_drawdown_pct": 11.0,
                "max_capital_used_quote": 1200.0,
            },
            "streams": ["ab", "cd"],
        }

        row = probe.row_from_report("conservative", streams, report, {"portfolio_size": 2})

        self.assertEqual(row["pairs"], ["A,B", "C,D"])
        self.assertEqual(row["symbols"], ["A", "B", "C", "D"])
        self.assertEqual(row["portfolio_size"], 2)
        self.assertEqual(row["live_parity_status"], "research_only")
        self.assertGreater(row["gap_score"], 0.0)

    def test_gap_score_prefers_lower_gaps(self):
        near = {
            "profile": "conservative",
            "ann": 49.0,
            "dd": 10.5,
            "cap": 1200.0,
            "pos": 4,
            "c2426": 10.0,
        }
        far = {
            "profile": "conservative",
            "ann": 20.0,
            "dd": 25.0,
            "cap": 6000.0,
            "pos": 2,
            "c2426": -5.0,
        }

        self.assertLess(probe.gap_score(near), probe.gap_score(far))

    def test_summarize_and_write_outputs_stay_research_only(self):
        rows = [
            {
                "profile": "conservative",
                "pass": False,
                "ann": 49.0,
                "dd": 11.0,
                "cap": 1200.0,
                "pos": 4,
                "c2426": 20.0,
                "gap_score": 0.2,
                "pairs": ["A,B", "C,D"],
                "symbols": ["A", "B", "C", "D"],
                "portfolio_size": 2,
                "live_parity_status": "research_only",
            },
            {
                "profile": "conservative",
                "pass": True,
                "ann": 55.0,
                "dd": 9.0,
                "cap": 1200.0,
                "pos": 4,
                "c2426": 20.0,
                "gap_score": 0.0,
                "pairs": ["A,B", "C,D"],
                "symbols": ["A", "B", "C", "D"],
                "portfolio_size": 2,
                "live_parity_status": "research_only",
            },
            {
                "profile": "balanced",
                "pass": False,
                "ann": 60.0,
                "dd": 18.0,
                "cap": 1200.0,
                "pos": 4,
                "c2426": 20.0,
                "gap_score": 0.4,
                "pairs": ["A,B", "C,D"],
                "symbols": ["A", "B", "C", "D"],
                "portfolio_size": 2,
                "live_parity_status": "research_only",
            },
        ]
        result = {"live_parity_status": "research_only", "rows": rows, "summary": probe.summarize(rows)}

        self.assertEqual(result["summary"]["conservative"]["passes"], 1)
        self.assertEqual(result["summary"]["balanced"]["passes"], 0)

        with tempfile.TemporaryDirectory() as tmpdir:
            out_json = Path(tmpdir) / "probe.json"
            out_md = Path(tmpdir) / "probe.md"
            probe.write_outputs(result, out_json, out_md)
            text = out_md.read_text()

        self.assertIn("research-only", text)
        self.assertIn("live_parity_status", text)
        self.assertIn("rows: `3`", text)
        self.assertIn("passes: `1`", text)
        self.assertNotIn("live_parity_passed", text)


if __name__ == "__main__":
    unittest.main()
