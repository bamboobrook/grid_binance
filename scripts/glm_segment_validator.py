#!/usr/bin/env python3
"""GLM Martingale Core — Segment-First Validator.

Runs a portfolio config across the 5 anti-overfit segments and the full period,
then applies the plan's segment-first scoring and hard-reject rules. This is the
shared gate every candidate must pass before being recorded as promising.

Segments (from the plan):
  h1_2023   1672531200000 - 1688169599999
  h2_2023   1688169600000 - 1704067199999
  2024      1704067200000 - 1735689599999
  2025      1735689600000 - 1767225599999
  2026_ytd  1767225600000 - 1780271999999
  full      1672531200000 - 1780271999999

Usage:
  python3 scripts/glm_segment_validator.py \
      --config path/to/config.json \
      --budget 5000 \
      --profile balanced \
      --out /tmp/glm_seg/result.json

Each segment runs portfolio_budget_replay as a subprocess (real engine, no
lookahead inside a segment). Segment boundaries are inclusive start / exclusive
end (end_ms = segment_end_ms - 1).
"""
import argparse
import json
import os
import subprocess
import sys
import time

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"

SEGMENTS = [
    ("h1_2023", 1672531200000, 1688169599999),
    ("h2_2023", 1688169600000, 1704067199999),
    ("2024", 1704067200000, 1735689599999),
    ("2025", 1735689600000, 1767225599999),
    ("2026_ytd", 1767225600000, 1780271999999),
]
FULL_START = 1672531200000
FULL_END = 1780271999999

PROFILES = {
    "conservative": {"ann_min": 50.0, "dd_max": 10.0, "min_pos_seg": 4},
    "balanced": {"ann_min": 90.0, "dd_max": 20.0, "min_pos_seg": 4},
    "aggressive": {"ann_min": 110.0, "dd_max": 30.0, "min_pos_seg": 3},
}


def run_replay(config_path, budget, start_ms, end_ms, profile, portfolio_id,
               env_extra=None, equity_curve=False):
    cmd = [
        REPLAY,
        "--config", config_path,
        "--budget", str(budget),
        "--start-ms", str(start_ms),
        "--end-ms", str(end_ms),
        "--market-data", MARKET_DB,
        "--funding-data", FUNDING_DB,
        "--profile", profile,
        "--portfolio-id", portfolio_id,
        "--exchange-min-notional", "5",
    ]
    if equity_curve:
        cmd += ["--equity-curve-points", "2000"]
    env = dict(os.environ)
    if env_extra:
        env.update(env_extra)
    proc = subprocess.run(cmd, capture_output=True, text=True, env=env)
    if proc.returncode != 0:
        return {"error": proc.stderr.strip()[:1000], "stdout": proc.stdout[:500]}
    result = json.loads(proc.stdout)
    return result


def extract_metrics(result):
    """Pull the canonical metrics from a replay result."""
    if "error" in result:
        return None
    ob = result.get("on_budget", {})
    return {
        "annualized_return_pct": ob.get("annualized_return_pct"),
        "max_drawdown_pct": ob.get("max_drawdown_pct"),
        "total_return_pct": ob.get("total_return_pct"),
        "min_equity_quote": ob.get("min_equity_quote"),
        "principal_breached": ob.get("principal_breached"),
        "trade_count": result.get("trade_count"),
        "stop_count": result.get("stop_count"),
        "max_capital_used_quote": result.get("on_max_capital_used", {}).get(
            "max_capital_used_quote"),
    }


def validate(config_path, budget, profile, portfolio_id, env_extra=None,
             out_path=None):
    prof = PROFILES[profile]
    t0 = time.time()
    # Run segments in sequence (engine reloads bars each time; this is the
    # correct anti-overfit semantics — each segment is independent).
    seg_metrics = {}
    for name, s, e in SEGMENTS:
        r = run_replay(config_path, budget, s, e, profile,
                       f"{portfolio_id}_{name}", env_extra)
        m = extract_metrics(r)
        seg_metrics[name] = m
        ann = m["annualized_return_pct"] if m else None
        dd = m["max_drawdown_pct"] if m else None
        print(f"  seg {name:8s} ann={ann if ann is not None else 'ERR':>8} "
              f"dd={dd if dd is not None else 'ERR':>7}", flush=True)

    # Full period
    full_r = run_replay(config_path, budget, FULL_START, FULL_END, profile,
                        f"{portfolio_id}_full", env_extra)
    full_m = extract_metrics(full_r)
    full_curve = full_r.get("equity_curve") if full_r else None

    # Scoring
    positive_segs = sum(
        1 for m in seg_metrics.values()
        if m and m["total_return_pct"] is not None and m["total_return_pct"] > 0
    )
    seg_returns = {n: (m["total_return_pct"] if m and m["total_return_pct"] else 0.0)
                   for n, m in seg_metrics.items()}
    seg_anns = [m["annualized_return_pct"] for m in seg_metrics.values()
                if m and m["annualized_return_pct"] is not None]
    seg_ann_stddev = (sum((x - sum(seg_anns)/len(seg_anns))**2 for x in seg_anns)
                      / len(seg_anns)) ** 0.5 if seg_anns else None

    full_ann = full_m["annualized_return_pct"] if full_m else None
    full_dd = full_m["max_drawdown_pct"] if full_m else None
    agg_2024_2026 = (seg_returns.get("2024", 0) + seg_returns.get("2025", 0)
                     + seg_returns.get("2026_ytd", 0))
    h1_contrib = (seg_returns.get("h1_2023", 0) /
                  (sum(seg_returns.values()) or 1.0) * 100.0
                  if sum(seg_returns.values()) > 0 else 0.0)

    # Hard rejects
    rejects = []
    if positive_segs < prof["min_pos_seg"]:
        rejects.append(f"positive_segments {positive_segs} < {prof['min_pos_seg']}")
    if agg_2024_2026 < 0:
        rejects.append(f"2024-2026 aggregate {agg_2024_2026:.1f} < 0")
    if h1_contrib > 60.0:
        rejects.append(f"h1_2023 contribution {h1_contrib:.1f}% > 60%")
    if full_dd and full_dd > prof["dd_max"] * 1.25:
        rejects.append(f"full DD {full_dd:.1f} > {prof['dd_max']*1.25:.1f} (1.25x)")

    # Gate pass
    full_pass = (full_ann is not None and full_dd is not None
                 and full_ann > prof["ann_min"] and full_dd <= prof["dd_max"]
                 and not rejects and full_m
                 and not full_m.get("principal_breached", True))

    rec = {
        "profile": profile,
        "budget_quote": budget,
        "full_metrics": full_m,
        "segment_metrics": seg_metrics,
        "positive_segments": positive_segs,
        "min_positive_segments": prof["min_pos_seg"],
        "segment_returns": seg_returns,
        "aggregate_return_2024_2026": agg_2024_2026,
        "h1_2023_contribution_pct": h1_contrib,
        "segment_ann_stddev": seg_ann_stddev,
        "gate_pass": full_pass,
        "rejects": rejects,
        "elapsed_sec": round(time.time() - t0, 1),
    }
    if out_path:
        os.makedirs(os.path.dirname(out_path), exist_ok=True)
        with open(out_path, "w") as f:
            json.dump(rec, f, indent=2)
    return rec


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", required=True)
    ap.add_argument("--budget", type=float, required=True)
    ap.add_argument("--profile", required=True, choices=PROFILES.keys())
    ap.add_argument("--portfolio-id", default="glm_seg")
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    print(f"Validating {args.config} budget={args.budget} profile={args.profile}",
          flush=True)
    rec = validate(args.config, args.budget, args.profile, args.portfolio_id,
                   out_path=args.out)
    fa = rec["full_metrics"]["annualized_return_pct"] if rec["full_metrics"] else None
    fd = rec["full_metrics"]["max_drawdown_pct"] if rec["full_metrics"] else None
    print(f"\nRESULT: full ann={fa} dd={fd} pos_segs={rec['positive_segments']}/5 "
          f"agg24-26={rec['aggregate_return_2024_2026']:.1f} "
          f"h1_contrib={rec['h1_2023_contribution_pct']:.1f}% "
          f"PASS={rec['gate_pass']}", flush=True)
    if rec["rejects"]:
        print("REJECTS:", rec["rejects"])
    return 0


if __name__ == "__main__":
    sys.exit(main())
