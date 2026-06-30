#!/usr/bin/env python3
"""Probe whether existing P4 2025 short legs can rescue GLM small-cap bases.

This is a research-only script intended to run on the WSL repo. It builds
portfolio configs from existing artifacts, runs portfolio_budget_replay, and
writes a compact JSON report.
"""
from __future__ import annotations

import argparse
import copy
import json
import math
import os
import subprocess
import tempfile
from pathlib import Path


SEGMENTS = {
    "full": (1672531200000, 1780271999999),
    "h1_2023": (1672531200000, 1688169599999),
    "h2_2023": (1688169600000, 1704067199999),
    "2024": (1704067200000, 1735689599999),
    "2025": (1735689600000, 1767225599999),
    "2026_ytd": (1767225600000, 1780271999999),
}

PARAM_KEYS = [
    "first_order_quote",
    "leverage",
    "multiplier",
    "max_legs",
    "step_bps",
    "take_profit_bps",
    "cooldown_seconds",
    "adx_min",
    "stop_loss_bps",
    "entry_filter",
    "regime_break_ema_period",
    "max_cycle_age_hours",
]


def fmt_decimal(value) -> str:
    if isinstance(value, str):
        return value
    f = float(value)
    text = f"{f:.10f}".rstrip("0").rstrip(".")
    return text if text else "0"


def filter_expressions(entry_filter: str, direction: str) -> list[str]:
    expressions: list[str] = []
    if entry_filter in ("trend", "trend_rsi"):
        expressions.append("close > ema(200)" if direction == "long" else "close < ema(200)")
        if entry_filter == "trend_rsi":
            expressions.append("rsi(14) < 65" if direction == "long" else "rsi(14) > 35")
    elif entry_filter == "rsi_extreme":
        expressions.append("rsi(14) < 30" if direction == "long" else "rsi(14) > 70")
    elif entry_filter == "rsi_moderate":
        expressions.append("rsi(14) < 35" if direction == "long" else "rsi(14) > 65")
    elif entry_filter == "bb_extreme":
        expressions.append("close < bb_lower(20,2.5)" if direction == "long" else "close > bb_upper(20,2.5)")
    elif entry_filter == "bb_moderate":
        expressions.append("close < bb_lower(20,2)" if direction == "long" else "close > bb_upper(20,2)")
    elif entry_filter == "rsi_bb_extreme":
        expressions.append("rsi(14) < 35" if direction == "long" else "rsi(14) > 65")
        expressions.append("close < bb_lower(20,2)" if direction == "long" else "close > bb_upper(20,2)")
    elif entry_filter == "rsi_bb_moderate":
        expressions.append("rsi(14) < 40" if direction == "long" else "rsi(14) > 60")
        expressions.append("close < bb_lower(20,1.5)" if direction == "long" else "close > bb_upper(20,1.5)")
    return expressions


def build_stop_loss(row: dict) -> dict:
    rb = row.get("regime_break_ema_period")
    if rb is not None:
        return {
            "regime_break_stop": {
                "ema_period": int(rb),
                "drawdown_pct_bps": int(row["stop_loss_bps"]),
            }
        }
    return {"strategy_drawdown_pct": {"pct_bps": int(row["stop_loss_bps"])}}


def build_risk_limits(row: dict) -> dict:
    age = row.get("max_cycle_age_hours")
    if age is None:
        return {}
    return {"max_cycle_age_hours": float(age)}


def build_short_strategy(row: dict, weight_pct: float, tag: str) -> dict:
    direction = "short"
    triggers: list[dict] = [{"cooldown": {"seconds": int(row["cooldown_seconds"])}}]
    adx = row.get("adx_min")
    if adx is not None:
        triggers.append({"indicator_expression": {"expression": f"adx(14) > {adx}"}})
    for expr in filter_expressions(row["entry_filter"], direction):
        triggers.append({"indicator_expression": {"expression": expr}})
    symbol = row["symbol"]
    return {
        "strategy_id": (
            f"p4mix-{tag}-{symbol}-S-foq{fmt_decimal(row['first_order_quote'])}"
            f"-lev{row['leverage']}-m{fmt_decimal(row['multiplier'])}"
            f"-legs{row['max_legs']}-step{row['step_bps']}-tp{row['take_profit_bps']}"
            f"-sl{row['stop_loss_bps']}-rb{row.get('regime_break_ema_period')}"
            f"-age{row.get('max_cycle_age_hours')}"
        ),
        "symbol": symbol,
        "market": "usd_m_futures",
        "direction": direction,
        "direction_mode": "short_only",
        "margin_mode": "isolated",
        "leverage": int(row["leverage"]),
        "spacing": {"fixed_percent": {"step_bps": int(row["step_bps"])}},
        "sizing": {
            "multiplier": {
                "first_order_quote": fmt_decimal(row["first_order_quote"]),
                "multiplier": fmt_decimal(row["multiplier"]),
                "max_legs": int(row["max_legs"]),
            }
        },
        "take_profit": {"percent": {"bps": int(row["take_profit_bps"])}},
        "stop_loss": build_stop_loss(row),
        "indicators": [{"atr": {"period": 21}}, {"adx": {"period": 14}}],
        "entry_triggers": triggers,
        "risk_limits": build_risk_limits(row),
        "portfolio_weight_pct": fmt_decimal(weight_pct),
    }


def weight(strategy: dict) -> float:
    raw = strategy.get("portfolio_weight_pct", strategy.get("weight_pct", 1))
    try:
        return float(raw)
    except Exception:
        return 1.0


def normalize_weights(strategies: list[dict], total_pct: float) -> None:
    current = sum(max(weight(s), 0.0) for s in strategies)
    if current <= 0:
        each = total_pct / max(len(strategies), 1)
        for strategy in strategies:
            strategy["portfolio_weight_pct"] = fmt_decimal(each)
        return
    for strategy in strategies:
        strategy["portfolio_weight_pct"] = fmt_decimal(weight(strategy) / current * total_pct)


def strip_old_shorts(strategies: list[dict]) -> list[dict]:
    return [s for s in strategies if s.get("direction") != "short"]


def select_best_by_symbol(rows: list[dict], symbols: list[str], predicate) -> list[dict]:
    out: list[dict] = []
    for symbol in symbols:
        pool = [
            row
            for row in rows
            if row.get("symbol") == symbol
            and row.get("direction_mode") == "short_only"
            and not row.get("principal_breached")
            and predicate(row)
        ]
        if pool:
            out.append(max(pool, key=lambda r: (float(r["annualized_return_pct"]), -float(r["max_drawdown_pct"]))))
    return out


def load_short_sets(search_path: Path) -> dict[str, list[dict]]:
    search = json.loads(search_path.read_text())
    rows = search["best_by_budget"]["3000"]
    core = ["APTUSDT", "COMPUSDT", "DOTUSDT"]
    low = select_best_by_symbol(
        rows,
        core,
        lambda r: float(r["annualized_return_pct"]) >= 20 and float(r["max_drawdown_pct"]) <= 15,
    )
    mid = select_best_by_symbol(
        rows,
        core,
        lambda r: float(r["annualized_return_pct"]) >= 50 and float(r["max_drawdown_pct"]) <= 30,
    )
    high = select_best_by_symbol(
        rows,
        core,
        lambda r: float(r["annualized_return_pct"]) >= 90 and float(r["max_drawdown_pct"]) <= 35,
    )
    gala = select_best_by_symbol(
        rows,
        ["GALAUSDT"],
        lambda r: float(r["annualized_return_pct"]) >= 5 and float(r["max_drawdown_pct"]) <= 30,
    )
    rsi_low = select_best_by_symbol(
        rows,
        ["APTUSDT", "DOTUSDT", "ETCUSDT", "GALAUSDT"],
        lambda r: float(r["annualized_return_pct"]) >= 3
        and float(r["max_drawdown_pct"]) <= 20
        and r.get("entry_filter") in ("rsi_moderate", "trend_rsi", "none"),
    )
    return {
        "low3": low,
        "mid3": mid,
        "high3": high,
        "low3_gala": low + gala,
        "rsi_low4": rsi_low,
    }


def run_replay(
    replay_bin: Path,
    config: dict,
    budget: float,
    profile: str,
    start_ms: int,
    end_ms: int,
    market_data: Path,
    funding_data: Path,
    timeout: int,
) -> dict:
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as fh:
        json.dump(config, fh)
        cfg_path = fh.name
    try:
        proc = subprocess.run(
            [
                str(replay_bin),
                "--config",
                cfg_path,
                "--budget",
                fmt_decimal(budget),
                "--profile",
                profile,
                "--start-ms",
                str(start_ms),
                "--end-ms",
                str(end_ms),
                "--market-data",
                str(market_data),
                "--funding-data",
                str(funding_data),
                "--exchange-min-notional",
                "5",
                "--equity-curve-points",
                "8",
            ],
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        if proc.returncode != 0:
            return {"ok": False, "error": proc.stderr[-2000:], "returncode": proc.returncode}
        data = json.loads(proc.stdout)
        on = data.get("on_budget", {})
        return {
            "ok": True,
            "ann": on.get("annualized_return_pct"),
            "dd": on.get("max_drawdown_pct"),
            "ret": on.get("total_return_pct"),
            "principal_breached": on.get("principal_breached"),
            "min_equity": on.get("min_equity_quote"),
            "cap": data.get("max_capital_used_quote"),
            "blocked": data.get("budget_blocked_legs"),
            "trades": data.get("trade_count"),
            "stops": data.get("stop_count"),
            "gate_passed": (data.get("gate") or {}).get("passed"),
            "strategy_count": data.get("strategy_count"),
            "symbols": data.get("symbols"),
        }
    except subprocess.TimeoutExpired:
        return {"ok": False, "error": "timeout"}
    finally:
        os.unlink(cfg_path)


def summarize_short_set(short_set: list[dict]) -> list[dict]:
    return [
        {
            "symbol": row["symbol"],
            "ann_2025": row["annualized_return_pct"],
            "dd_2025": row["max_drawdown_pct"],
            "foq": row["first_order_quote"],
            "leverage": row["leverage"],
            "multiplier": row["multiplier"],
            "max_legs": row["max_legs"],
            "entry_filter": row["entry_filter"],
            "rb": row.get("regime_break_ema_period"),
            "age": row.get("max_cycle_age_hours"),
        }
        for row in short_set
    ]


def build_mix(
    base_cfg: dict,
    short_set: list[dict],
    mode: str,
    short_total_weight: float,
    budget: float,
    tag: str,
) -> dict:
    portfolio = copy.deepcopy(base_cfg["portfolio_config"])
    base_strategies = copy.deepcopy(portfolio.get("strategies", []))
    if mode == "drop_old_shorts_norm":
        base_strategies = strip_old_shorts(base_strategies)
        normalize_weights(base_strategies, max(0.0, 100.0 - short_total_weight))
    elif mode == "norm":
        normalize_weights(base_strategies, max(0.0, 100.0 - short_total_weight))
    elif mode == "keep":
        pass
    else:
        raise ValueError(mode)

    short_each = short_total_weight / max(len(short_set), 1)
    added = [build_short_strategy(row, short_each, tag) for row in short_set]
    portfolio["strategies"] = base_strategies + added
    portfolio["direction_mode"] = "long_and_short"
    portfolio.setdefault("risk_limits", {})["max_global_budget_quote"] = fmt_decimal(budget)
    return {"portfolio_config": portfolio}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", default="/home/bumblebee/Project/grid_binance/.claude/worktrees/p4-cycle-exit")
    parser.add_argument("--main-repo", default="/home/bumblebee/Project/grid_binance")
    parser.add_argument("--search", default="/tmp/2025_p4_3000_allrows.json")
    parser.add_argument("--out", default="docs/superpowers/reports/2026-06-29-p4-combo-mix-probe.json")
    parser.add_argument("--budget", type=float, default=5000)
    parser.add_argument("--profile", default="balanced")
    parser.add_argument("--full-only", action="store_true")
    parser.add_argument("--timeout", type=int, default=900)
    args = parser.parse_args()

    repo = Path(args.repo)
    main_repo = Path(args.main_repo)
    replay_bin = repo / "target" / "release" / "portfolio_budget_replay"
    market_data = main_repo / "data" / "market_data_full.db"
    funding_data = main_repo / "data" / "funding_rates.db"

    bases = {
        "floor1500": main_repo / "docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_floor1500_b5000.json",
        "l5_robust": main_repo / "docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_l5_robust_b5000.json",
    }
    short_sets = load_short_sets(Path(args.search))

    experiments: list[tuple[str, dict]] = []
    for base_name, base_path in bases.items():
        base_cfg = json.loads(base_path.read_text())
        experiments.append((f"{base_name}__as_is", copy.deepcopy(base_cfg)))
        for set_name, short_set in short_sets.items():
            if not short_set:
                continue
            for mode in ("keep", "norm", "drop_old_shorts_norm"):
                for short_weight in (10.0, 20.0, 30.0, 40.0):
                    exp_name = f"{base_name}__{mode}__{set_name}__short{int(short_weight)}"
                    experiments.append(
                        (
                            exp_name,
                            build_mix(
                                base_cfg,
                                short_set,
                                mode,
                                short_weight,
                                args.budget,
                                f"{set_name}-sw{int(short_weight)}",
                            ),
                        )
                    )

    report: dict = {
        "budget": args.budget,
        "profile": args.profile,
        "short_sets": {k: summarize_short_set(v) for k, v in short_sets.items()},
        "full_results": [],
        "segment_results": [],
    }

    full_start, full_end = SEGMENTS["full"]
    for index, (name, config) in enumerate(experiments, 1):
        result = run_replay(
            replay_bin,
            config,
            args.budget,
            args.profile,
            full_start,
            full_end,
            market_data,
            funding_data,
            args.timeout,
        )
        result["name"] = name
        report["full_results"].append(result)
        if result.get("ok"):
            print(
                f"[{index:03d}/{len(experiments)}] {name} "
                f"ann={result['ann']:.2f} dd={result['dd']:.2f} ret={result['ret']:.2f} "
                f"cap={result['cap']:.1f} blocked={result['blocked']} gate={result['gate_passed']}"
            )
        else:
            print(f"[{index:03d}/{len(experiments)}] {name} ERROR {result.get('error')}")

    ok_results = [r for r in report["full_results"] if r.get("ok")]
    candidates = [
        r
        for r in ok_results
        if r.get("ann") is not None
        and r.get("dd") is not None
        and r["ann"] >= 60.0
        and r["dd"] <= 30.0
        and not r.get("principal_breached")
    ]
    candidates.sort(key=lambda r: (r["gate_passed"] is True, r["ann"] - 3.0 * max(0.0, r["dd"] - 20.0)), reverse=True)
    selected_names = [r["name"] for r in candidates[:8]]

    if not args.full_only and selected_names:
        exp_map = dict(experiments)
        for name in selected_names:
            config = exp_map[name]
            item = {"name": name, "segments": {}}
            for seg_name, (start, end) in SEGMENTS.items():
                item["segments"][seg_name] = run_replay(
                    replay_bin,
                    config,
                    args.budget,
                    args.profile,
                    start,
                    end,
                    market_data,
                    funding_data,
                    args.timeout,
                )
            report["segment_results"].append(item)

    out_path = repo / args.out
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(report, ensure_ascii=False, indent=2))
    print(f"wrote {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
