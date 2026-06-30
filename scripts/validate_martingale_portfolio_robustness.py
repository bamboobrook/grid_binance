#!/usr/bin/env python3
"""
Validate a martingale portfolio config across multiple budgets AND time segments,
applying anti-overfitting gates. Wraps the `portfolio_budget_replay` binary.

A candidate is "robust" only if it passes:
  - full-period gate (ann/DD target for its profile)
  - budget matrix (no principal breach, max_capital_used <= budget, K executable)
  - segment gate (anti-overfitting: not concentrated in H1-2023)
  - live-parity gate (only Percent TP + StrategyDrawdownPct SL)

Usage:
  python3 scripts/validate_martingale_portfolio_robustness.py \
    --config <path> --profile conservative|balanced|aggressive \
    --budgets 1000,2000,3000,4000,5000 \
    --market-data data/market_data_full.db \
    --funding-data data/funding_rates.db \
    --out docs/superpowers/reports/<name>_robustness.json
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

# Profile targets (full-period).
PROFILE_TARGETS = {
    "conservative": {"ann_min": 50.0, "dd_max": 10.0},
    "balanced": {"ann_min": 90.0, "dd_max": 20.0},
    "aggressive": {"ann_min": 110.0, "dd_max": 30.0},
}

# Anti-overfitting segment constraints (per ChatGPT plan §3 主线D).
SEGMENT_CONSTRAINTS = {
    "conservative": {
        "min_positive_segments": 4,   # of 5 segments must be non-negative return
        "max_segment_dd": 12.0,       # any segment DD <= 12%
        "no_principal_breach": True,
    },
    "balanced": {
        "min_positive_segments": 3,
        "max_segment_dd": 24.0,
        "no_principal_breach": True,
        "no_2024_2026_total_loss": True,  # 2024-2026 combined must not be a loss
    },
    "aggressive": {
        "min_positive_segments": 3,
        "max_segment_dd": 36.0,
        "no_principal_breach": True,
        "no_2024_2026_total_loss": True,
    },
}

# Standard segments for full-period validation (ms epochs).
SEGMENTS = {
    "h1_2023": (1672531200000, 1688169599999),
    "h2_2023": (1688169600000, 1704067199999),
    "2024": (1704067200000, 1735689599999),
    "2025": (1735689600000, 1767225599999),
    "2026_ytd": (1767225600000, 1780271999999),
    "full": (1672531200000, 1780271999999),
}

REPO_ROOT = Path(__file__).resolve().parent.parent
REPLAY_BIN = REPO_ROOT / "target" / "release" / "portfolio_budget_replay"


def run_replay(config: str, budget: float, profile: str, start_ms: int, end_ms: int,
               market_data: str, funding_data: str) -> dict:
    """Run portfolio_budget_replay and return the parsed JSON report."""
    cmd = [
        str(REPLAY_BIN),
        "--config", config,
        "--budget", str(budget),
        "--profile", profile,
        "--start-ms", str(start_ms),
        "--end-ms", str(end_ms),
        "--market-data", market_data,
        "--funding-data", funding_data,
        "--exchange-min-notional", "5",
        "--equity-curve-points", "10",
    ]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    if result.returncode != 0:
        return {"_error": result.returncode, "_stderr": result.stderr[-2000:]}
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as e:
        return {"_error": "json", "_stderr": result.stdout[-2000:], "_exc": str(e)}


def extract_metrics(report: dict) -> dict:
    """Pull the key metrics from a replay report."""
    if "_error" in report:
        return {"error": report["_stderr"][-500:], "ok": False}
    on = report.get("on_budget", {})
    return {
        "ok": True,
        "ann": on.get("annualized_return_pct"),
        "dd": on.get("max_drawdown_pct"),
        "total_return": on.get("total_return_pct"),
        "principal_breached": on.get("principal_breached", False),
        "min_equity": on.get("min_equity_quote"),
        "max_capital_used": report.get("max_capital_used_quote"),
        "budget_blocked_legs": report.get("budget_blocked_legs", 0),
        "min_capital": report.get("minimum_capital", {}),
        "trade_count": report.get("trade_count", 0),
        "runtime_weight_caps_applied": report.get("runtime_weight_caps_applied", False),
    }


def check_live_parity(config_path: str) -> dict:
    """Check the config uses only live-parity TP/SL models."""
    with open(config_path) as f:
        cfg = json.load(f)
    portfolio = cfg.get("portfolio_config", cfg)
    violations = []
    for strat in portfolio.get("strategies", []):
        sid = strat.get("strategy_id", "?")
        tp = strat.get("take_profit", {})
        tp_kind = next(iter(tp.keys())) if isinstance(tp, dict) and tp else str(tp)
        if tp_kind != "percent":
            violations.append(f"{sid}: TP={tp_kind} (only 'percent' has live parity)")
        sl = strat.get("stop_loss")
        if sl is not None:
            sl_kind = next(iter(sl.keys())) if isinstance(sl, dict) and sl else str(sl)
            if sl_kind != "strategy_drawdown_pct":
                violations.append(f"{sid}: SL={sl_kind} (only 'strategy_drawdown_pct' has live parity)")
    return {"passes": len(violations) == 0, "violations": violations}


def evaluate_gate(metrics: dict, ann_min: float, dd_max: float) -> bool:
    """Full-period gate for a profile."""
    budget = metrics.get("budget")
    within_budget = True if budget is None else (metrics["max_capital_used"] or 0) <= budget
    return (metrics["ok"] and metrics["ann"] is not None
            and metrics["ann"] >= ann_min
            and metrics["dd"] is not None and metrics["dd"] <= dd_max
            and not metrics["principal_breached"]
            and within_budget
            and metrics["budget_blocked_legs"] == 0)


def evaluate_segment_gate(segment_results: dict, profile: str, budget: float) -> dict:
    """Anti-overfitting gate across segments."""
    cons = SEGMENT_CONSTRAINTS[profile]
    seg_metrics = {k: extract_metrics(v) for k, v in segment_results.items()
                   if k in SEGMENTS and k != "full"}
    violations = []

    # Principal breach in any segment.
    if cons["no_principal_breach"]:
        for seg, m in seg_metrics.items():
            if m.get("principal_breached"):
                violations.append(f"{seg}: principal breached")

    # Max segment DD.
    max_dd = cons["max_segment_dd"]
    for seg, m in seg_metrics.items():
        if m.get("dd") is not None and m["dd"] > max_dd:
            violations.append(f"{seg}: DD {m['dd']:.1f}% > {max_dd}%")

    # Min positive segments.
    positive = sum(1 for m in seg_metrics.values()
                   if m.get("total_return") is not None and m["total_return"] >= 0)
    if positive < cons["min_positive_segments"]:
        violations.append(f"only {positive}/{len(seg_metrics)} segments positive "
                          f"(need {cons['min_positive_segments']})")

    # 2024-2026 combined must not be a loss (using equity growth proxy).
    if cons.get("no_2024_2026_total_loss"):
        rets = [seg_metrics.get(s, {}).get("total_return", 0) or 0
                for s in ("2024", "2025", "2026_ytd")]
        combined = 1.0
        for r in rets:
            combined *= (1 + r / 100.0)
        combined_pct = (combined - 1) * 100
        if combined_pct < 0:
            violations.append(f"2024-2026 combined return {combined_pct:.1f}% < 0")

    return {
        "passes": len(violations) == 0,
        "violations": violations,
        "positive_segments": positive,
        "total_segments": len(seg_metrics),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", required=True)
    ap.add_argument("--profile", required=True, choices=list(PROFILE_TARGETS))
    ap.add_argument("--budgets", default="1000,2000,3000,4000,5000")
    ap.add_argument("--segments", default="all",
                    help="'all' for standard 5, or comma list: h1_2023,h2_2023,2024,2025,2026_ytd")
    ap.add_argument("--market-data", default="data/market_data_full.db")
    ap.add_argument("--funding-data", default="data/funding_rates.db")
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    budgets = [float(b) for b in args.budgets.split(",")]
    if args.segments == "all":
        seg_names = ["h1_2023", "h2_2023", "2024", "2025", "2026_ytd"]
    else:
        seg_names = args.segments.split(",")

    target = PROFILE_TARGETS[args.profile]

    # 1. Live-parity check.
    parity = check_live_parity(args.config)

    # 2. Full-period gate at each budget.
    full_start, full_end = SEGMENTS["full"]
    budget_matrix = {}
    for b in budgets:
        print(f"  full-period @ budget {b}...", file=sys.stderr)
        report = run_replay(args.config, b, args.profile, full_start, full_end,
                            args.market_data, args.funding_data)
        m = extract_metrics(report)
        budget_matrix[b] = m
        status = "PASS" if (m["ok"] and (m["ann"] or 0) >= target["ann_min"]
                            and (m["dd"] or 999) <= target["dd_max"]) else "fail"
        print(f"    ann={m.get('ann')}, dd={m.get('dd')}, "
              f"blocked={m.get('budget_blocked_legs')}, {status}", file=sys.stderr)

    # 3. Segment gate at the best budget (smallest that passes full gate, else smallest tested).
    best_budget = None
    for b in budgets:
        m = budget_matrix[b]
        if (m["ok"] and (m["ann"] or 0) >= target["ann_min"]
                and (m["dd"] or 999) <= target["dd_max"] and not m["principal_breached"]):
            best_budget = b
            break
    seg_budget = best_budget or budgets[0]

    segment_results = {}
    for seg in seg_names:
        s, e = SEGMENTS[seg]
        print(f"  segment {seg} @ budget {seg_budget}...", file=sys.stderr)
        report = run_replay(args.config, seg_budget, args.profile, s, e,
                            args.market_data, args.funding_data)
        segment_results[seg] = report

    seg_gate = evaluate_segment_gate(segment_results, args.profile, seg_budget)

    # 4. Assemble verdict.
    full_gate = any(
        budget_matrix[b]["ok"]
        and (budget_matrix[b]["ann"] or 0) >= target["ann_min"]
        and (budget_matrix[b]["dd"] or 999) <= target["dd_max"]
        and not budget_matrix[b]["principal_breached"]
        and budget_matrix[b]["budget_blocked_legs"] == 0
        for b in budgets
    )

    result = {
        "config": args.config,
        "profile": args.profile,
        "target": target,
        "live_parity": parity,
        "full_period_gate": full_gate,
        "segment_gate": seg_gate,
        "budget_matrix": budget_matrix,
        "best_passing_budget": best_budget,
        "segment_budget_used": seg_budget,
        "segment_metrics": {k: extract_metrics(v) for k, v in segment_results.items()},
        "verdict": "PASS" if (full_gate and seg_gate["passes"] and parity["passes"]) else "FAIL",
    }

    out_path = args.out or f"docs/superpowers/reports/{Path(args.config).stem}_robustness.json"
    Path(out_path).parent.mkdir(parents=True, exist_ok=True)
    with open(out_path, "w") as f:
        json.dump(result, f, indent=2, default=str)
    print(f"\nVERDICT: {result['verdict']}")
    print(f"  full_period_gate: {full_gate}")
    print(f"  segment_gate: {seg_gate['passes']} ({len(seg_gate['violations'])} violations)")
    print(f"  live_parity: {parity['passes']}")
    print(f"  -> {out_path}")
    return 0 if result["verdict"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
