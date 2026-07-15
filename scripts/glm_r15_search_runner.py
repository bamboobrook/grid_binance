#!/usr/bin/env python3
"""Round 15 portfolio builder + full-window binary-replay search runner.

Builds bidirectional (long+short) Martingale sleeve portfolios for the frozen
T2/T3 universe and runs FULL-WINDOW binary replays via the canonical release
CLI (portfolio_budget_replay). No fast screens: every candidate is a real
full-window, account-constrained replay over the dev window with the
authoritative funding DB.

This module is imported by the phase runners (P1 binding, G1-G4 search).
"""

import hashlib
import json
import os
import subprocess
import time
from pathlib import Path

REPO = Path("/home/bumblebee/Project/grid_binance")
CLI = REPO / "target" / "release" / "portfolio_budget_replay"
MARKET_DB = REPO / "data" / "market_data_full.db"
FUNDING_DB = REPO / "data" / "funding_rates_round12.db"
ART = REPO / "docs" / "superpowers" / "artifacts" / "glm-martingale-core-round15"
REGISTRY = ART / "exploration-registry.jsonl"

DEV_START = 1672531200000
DEV_END = 1780271999999

# Five cold-start train segments (plan §1.1, §5). The whole dev window is also
# used as the full-window result.
SEGMENTS = [
    ("h1_2023", 1672531200000, 1688169599999),
    ("h2_2023", 1688169600000, 1704067199999),
    ("2024", 1704067200000, 1735689599999),
    ("2025", 1735689600000, 1767225599999),
    ("2026_ytd", 1767225600000, 1780271999999),
]

BUDGETS = [1000.0, 2000.0, 3000.0, 4000.0, 4999.0]


def effective_config_hash(config_value):
    """Stable hash of the resolved/effective config (for dedup + binding tests)."""
    canon = json.dumps(config_value, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canon.encode()).hexdigest()


def build_bidirectional_portfolio(
    symbols,
    first_order,
    multiplier,
    max_legs,
    spacing_bps,
    tp_bps,
    leverage=10,
    direction_mode="long_and_short",
):
    """Build a portfolio with one long and one short Martingale sleeve per symbol.

    Weights are split so long and short sleeves share the budget. With equal
    treatment every sleeve gets 1/(2*len(symbols)) of the budget.
    """
    n_sleeves = 2 * len(symbols)
    weight_pct = round(100.0 / n_sleeves, 6)
    strategies = []
    for sym in symbols:
        for direction in ("long", "short"):
            strategies.append(
                {
                    "strategy_id": f"{'L' if direction == 'long' else 'S'}-{sym}",
                    "symbol": sym,
                    "market": "usd_m_futures",
                    "direction": direction,
                    "direction_mode": direction_mode,
                    "margin_mode": "isolated",
                    "leverage": leverage,
                    "spacing": {"fixed_percent": {"step_bps": spacing_bps}},
                    "sizing": {
                        "multiplier": {
                            "first_order_quote": str(first_order),
                            "multiplier": str(multiplier),
                            "max_legs": max_legs,
                        }
                    },
                    "take_profit": {"percent": {"bps": tp_bps}},
                    "stop_loss": None,
                    "indicators": [],
                    "entry_triggers": [{"cooldown": {"seconds": 21600}}],
                    "portfolio_weight_pct": str(weight_pct),
                    "risk_limits": {},
                }
            )
    return {
        "portfolio_config": {
            "direction_mode": direction_mode,
            "risk_limits": {"max_global_budget_quote": "4999"},
            "strategies": strategies,
        }
    }


def write_config(config, label):
    """Write a config to a temp file and return its path + effective hash."""
    cfg_dir = ART / "configs"
    cfg_dir.mkdir(parents=True, exist_ok=True)
    path = cfg_dir / f"{label}.json"
    with open(path, "w") as fh:
        json.dump(config, fh, indent=2, sort_keys=True)
    eff = effective_config_hash(config["portfolio_config"])
    return path, eff


def run_replay(config_path, budget, start_ms, end_ms, timeout=1800):
    """Run ONE full-window binary replay via the canonical CLI. Returns dict."""
    cmd = [
        str(CLI),
        "--config", str(config_path),
        "--budget", str(budget),
        "--start-ms", str(start_ms),
        "--end-ms", str(end_ms),
        "--market-data", str(MARKET_DB),
        "--funding-data", str(FUNDING_DB),
        "--exchange-min-notional", "5.0",
    ]
    t0 = time.time()
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        wall = time.time() - t0
        if r.returncode != 0:
            return {"status": "error", "returncode": r.returncode, "stderr": r.stderr[:500], "wall_s": wall}
        d = json.loads(r.stdout)
        ob = d.get("on_budget", {})
        uncapped = d.get("sim_stock_metrics_on_uncapped_planned_margin_base", {})
        per_strat = d.get("per_strategy", [])
        realized_by_symbol = d.get("realized_pnl_by_symbol", [])

        def _round(v, n=6):
            try:
                return round(float(v), n)
            except (TypeError, ValueError):
                return None

        return {
            "status": "complete",
            "wall_s": round(wall, 1),
            "budget": budget,
            "ann": _round(ob.get("annualized_return_pct")),
            "dd": _round(ob.get("max_drawdown_pct")),
            "total_return": _round(ob.get("total_return_pct")),
            "min_equity": _round(ob.get("min_equity_quote")),
            "principal_breached": bool(ob.get("principal_breached", False)),
            "trade_count": uncapped.get("trade_count", d.get("trade_count", 0)),
            "budget_blocked_legs": d.get("budget_blocked_legs", 0),
            "max_capital_used": _round(d.get("max_capital_used_quote", 0.0)),
            "per_strategy": per_strat,
            "realized_pnl_by_symbol": realized_by_symbol,
        }
    except subprocess.TimeoutExpired:
        return {"status": "timeout", "wall_s": timeout}
    except json.JSONDecodeError as e:
        return {"status": "error", "returncode": -1, "stderr": f"json: {e}", "wall_s": round(time.time() - t0, 1)}


def append_registry(record):
    """Append a registry record (running or terminal) to the JSONL."""
    ART.mkdir(parents=True, exist_ok=True)
    with open(REGISTRY, "a") as fh:
        fh.write(json.dumps(record, sort_keys=True) + "\n")


def concentration_metrics(result):
    """Compute the three concentration metrics from CLI output.

    - max_configured_share: largest per-strategy effective_cap_quote / total cap
    - max_gross_profit_share: largest per-symbol positive_pnl_quote / total pos pnl
    - max_positive_net_pnl_share: largest per-symbol positive_pnl_share_pct (CLI-provided)
    """
    per_strat = result.get("per_strategy", [])
    realized = result.get("realized_pnl_by_symbol", [])
    total_cap = sum(float(s.get("effective_cap_quote", 0) or 0) for s in per_strat)
    max_cap = max((float(s.get("effective_cap_quote", 0) or 0) for s in per_strat), default=0.0)
    total_pos = sum(float(s.get("positive_pnl_quote", 0) or 0) for s in realized)
    max_pos = max((float(s.get("positive_pnl_quote", 0) or 0) for s in realized), default=0.0)
    max_share_pct = max((float(s.get("positive_pnl_share_pct", 0) or 0) for s in realized), default=0.0)
    return {
        "max_configured_share": round(max_cap / total_cap * 100, 4) if total_cap > 0 else 0.0,
        "max_gross_profit_share": round(max_pos / total_pos * 100, 4) if total_pos > 0 else 0.0,
        "max_positive_net_pnl_share": round(max_share_pct, 4),
    }


def run_candidate(label, family, hypothesis, config_value, budget, start_ms, end_ms,
                  window_label, seed=None, optimizer="manual", parent_hash=None):
    """Run a full-window candidate: write running, run replay, append terminal."""
    config_path, eff_hash = write_config({"portfolio_config": config_value}, label)
    running = {
        "experiment_id": label,
        "family": family,
        "hypothesis": hypothesis,
        "parent_config_hash": parent_hash,
        "resolved_config_hash": eff_hash,
        "effective_config_hash": eff_hash,
        "window": window_label,
        "budget": budget,
        "start_ms": start_ms,
        "end_ms": end_ms,
        "seed": seed,
        "optimizer": optimizer,
        "status": "running",
        "started_at": time.time(),
    }
    append_registry(running)
    result = run_replay(config_path, budget, start_ms, end_ms)
    record = dict(running)
    record.update(result)
    record["status"] = result.get("status", "error")
    record["actual_binary_replays"] = 1 if result.get("status") == "complete" else 0
    record["cache_hits"] = 0
    record["finished_at"] = time.time()
    if result.get("status") == "complete":
        record["concentration"] = concentration_metrics(result)
    append_registry(record)
    return record
