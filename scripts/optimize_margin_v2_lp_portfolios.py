#!/usr/bin/env python3
"""Build corrected margin-v2 martingale LP portfolios for review.

This script is intentionally backtest/display only. It reads already persisted
candidate equity curves from corrected margin-v2 worker tasks, solves a linear
program under portfolio drawdown constraints, and optionally writes three
reviewable flyingkid backtest tasks / pending_confirmation portfolios.

It does not start trading, preconfigure Binance, or touch live executor state.
"""
from __future__ import annotations

import argparse
import csv
import io
import json
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass
from decimal import Decimal, getcontext
from pathlib import Path
from typing import Any

import numpy as np
from scipy.optimize import Bounds, LinearConstraint, linprog, milp


csv.field_size_limit(sys.maxsize)
getcontext().prec = 28

OWNER = "flyingkid2022@outlook.com"
DB = ["docker", "exec", "-i", "grid-binance-postgres-1", "psql", "-U", "postgres", "-d", "grid_binance"]
DB_AT = ["docker", "exec", "grid-binance-postgres-1", "psql", "-U", "postgres", "-d", "grid_binance", "-qAt", "-c"]

SOURCE_TASK_IDS = (
    "martingale-aggressive-20260625-margin-v2",
    "martingale-aggressive-20260625-robust-v1",
    "martingale-balanced-20260625-margin-v3",
)

REPORT_MD = Path("docs/superpowers/reports/2026-06-26-margin-v2-lp-portfolios.md")
REPORT_JSON = Path("docs/superpowers/reports/2026-06-26-margin-v2-lp-portfolios.json")


@dataclass(frozen=True)
class Case:
    label: str
    task_id: str
    portfolio_id: str
    name: str
    risk: str
    annualized_target: float
    dd_limit: float
    start_ms: int = 1672531200000
    end_ms: int = 1780271999999
    min_symbols: int = 8
    min_symbol_weight: float = 0.01
    per_symbol_cap: float = 0.40
    quarter_floor: float = 0.0


CASES = (
    Case(
        label="margin_v2_conservative_lp_dd10",
        task_id="lp-martingale-conservative-20260626-margin-v2",
        portfolio_id="mp_margin_v2_lp_conservative_20260626",
        name="Margin-v2 LP Conservative 50%+ / DD<=10",
        risk="conservative",
        annualized_target=50.0,
        dd_limit=10.0,
    ),
    Case(
        label="margin_v2_balanced_lp_dd20",
        task_id="lp-martingale-balanced-20260626-margin-v2",
        portfolio_id="mp_margin_v2_lp_balanced_20260626",
        name="Margin-v2 LP Balanced 90%+ / DD<=20",
        risk="balanced",
        annualized_target=90.0,
        dd_limit=20.0,
    ),
    Case(
        label="margin_v2_aggressive_lp_dd30",
        task_id="lp-martingale-aggressive-20260626-margin-v2",
        portfolio_id="mp_margin_v2_lp_aggressive_20260626",
        name="Margin-v2 LP Aggressive 110%+ / DD<=30",
        risk="aggressive",
        annualized_target=110.0,
        dd_limit=30.0,
    ),
)


def sql_string(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def jsonb(value: Any) -> str:
    text = json.dumps(value, ensure_ascii=False, separators=(",", ":"))
    if "$lpjson$" in text:
        raise ValueError("json contains dollar quote tag")
    return f"$lpjson${text}$lpjson$::jsonb"


def psql_at(sql: str) -> str:
    result = subprocess.run(DB_AT + [sql], capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(result.stderr)
    return result.stdout


def psql_exec(sql: str) -> None:
    result = subprocess.run(DB + ["-v", "ON_ERROR_STOP=1", "-q"], input=sql, text=True, capture_output=True)
    if result.returncode != 0:
        raise RuntimeError(result.stderr + "\n" + result.stdout)


def candidate_portfolio_config(config: dict[str, Any]) -> dict[str, Any]:
    return config.get("portfolio_config") or config


def fetch_candidates() -> list[dict[str, Any]]:
    source_ids = ",".join(sql_string(task_id) for task_id in SOURCE_TASK_IDS)
    sql = f"""
copy (
select bcs.candidate_id,
       bcs.task_id,
       bt.config->>'risk_profile' as source_risk_profile,
       coalesce(bt.config->>'direction_mode','') as task_direction_mode,
       coalesce(bcs.summary->>'symbol', bcs.config->'strategies'->0->>'symbol') as symbol,
       coalesce(bcs.summary->>'direction_mode', bcs.config->>'direction_mode', '') as candidate_direction_mode,
       (bcs.summary->>'annualized_return_pct')::float8 as annualized_return_pct,
       (bcs.summary->>'max_drawdown_pct')::float8 as max_drawdown_pct,
       coalesce((bcs.summary->>'trade_count')::float8, 0) as trade_count,
       coalesce((bcs.summary->>'planned_margin_quote')::float8, 0) as planned_margin_quote,
       coalesce((bcs.summary->>'planned_notional_quote')::float8, 0) as planned_notional_quote,
       coalesce((bcs.summary->>'max_capital_used_quote')::float8, 0) as max_capital_used_quote,
       jsonb_array_length(bcs.summary->'equity_curve') as equity_curve_len,
       (bcs.summary->'equity_curve')::text as equity_curve,
       bcs.config::text as config,
       bcs.summary::text as summary
from backtest_candidate_summaries bcs
join backtest_tasks bt on bt.task_id = bcs.task_id
where bcs.task_id in ({source_ids})
  and bcs.status = 'ready'
  and jsonb_array_length(bcs.summary->'equity_curve') >= 100
  and bcs.candidate_id not like 'lp\\_%' escape '\\'
) to stdout with csv
"""
    rows: list[dict[str, Any]] = []
    for record in csv.reader(io.StringIO(psql_at(sql))):
        if not record:
            continue
        (
            candidate_id,
            task_id,
            source_risk_profile,
            task_direction_mode,
            symbol,
            candidate_direction_mode,
            ann,
            dd,
            trade_count,
            planned_margin_quote,
            planned_notional_quote,
            max_capital_used_quote,
            equity_curve_len,
            equity_curve_text,
            config_text,
            summary_text,
        ) = record
        equity_curve = json.loads(equity_curve_text)
        timestamps = np.array([int(point["timestamp_ms"]) for point in equity_curve], dtype=np.int64)
        values = np.array([float(point["equity_quote"]) for point in equity_curve], dtype=np.float64)
        if len(timestamps) < 100 or values[0] <= 0.0 or values[-1] <= 0.0:
            continue
        years = (int(timestamps[-1]) - int(timestamps[0])) / (365.25 * 24 * 3600 * 1000)
        if years < 3.0:
            continue
        rows.append(
            {
                "candidate_id": candidate_id,
                "task_id": task_id,
                "source_risk_profile": source_risk_profile,
                "task_direction_mode": task_direction_mode,
                "symbol": symbol,
                "candidate_direction_mode": candidate_direction_mode,
                "annualized_return_pct": float(ann),
                "max_drawdown_pct": float(dd),
                "trade_count": int(float(trade_count)),
                "planned_margin_quote": float(planned_margin_quote),
                "planned_notional_quote": float(planned_notional_quote),
                "max_capital_used_quote": float(max_capital_used_quote),
                "equity_curve_len": int(equity_curve_len),
                "timestamps_raw": timestamps,
                "equity_raw": values / values[0],
                "config": json.loads(config_text),
                "summary": json.loads(summary_text),
            }
        )
    return rows


def reduce_rows(rows: list[dict[str, Any]], max_rows: int = 900) -> list[dict[str, Any]]:
    by_symbol: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        by_symbol[row["symbol"]].append(row)
    keep: list[dict[str, Any]] = []
    seen: set[str] = set()
    for candidates in by_symbol.values():
        picks = []
        picks += sorted(candidates, key=lambda row: row["annualized_return_pct"], reverse=True)[:80]
        picks += sorted(candidates, key=lambda row: row["max_drawdown_pct"])[:40]
        picks += sorted(
            candidates,
            key=lambda row: row["annualized_return_pct"] / max(row["max_drawdown_pct"], 1e-9),
            reverse=True,
        )[:80]
        for pick in picks:
            key = pick["candidate_id"]
            if key not in seen:
                keep.append(pick)
                seen.add(key)
    if len(keep) > max_rows:
        keep = sorted(
            keep,
            key=lambda row: (
                row["annualized_return_pct"] / max(row["max_drawdown_pct"], 1e-9),
                row["annualized_return_pct"],
            ),
            reverse=True,
        )[:max_rows]
    return keep


def align_rows(rows: list[dict[str, Any]]) -> np.ndarray:
    grid_owner = max(rows, key=lambda row: (len(row["timestamps_raw"]), row["annualized_return_pct"]))
    grid = grid_owner["timestamps_raw"]
    for row in rows:
        if len(row["timestamps_raw"]) == len(grid) and np.array_equal(row["timestamps_raw"], grid):
            row["equity"] = row["equity_raw"]
        else:
            row["equity"] = np.interp(grid, row["timestamps_raw"], row["equity_raw"])
        row["timestamps"] = grid
    return grid


def quarter_return_matrix(grid: np.ndarray, matrix: np.ndarray) -> np.ndarray:
    t0, t1 = int(grid[0]), int(grid[-1])
    bounds = np.linspace(t0, t1, 5)
    quarter_returns = np.zeros((4, matrix.shape[1]))
    for q in range(4):
        mask = (grid >= bounds[q]) & (grid <= bounds[q + 1])
        segment = matrix[mask, :]
        if segment.shape[0] < 2:
            continue
        quarter_returns[q, :] = segment[-1, :] / segment[0, :] - 1.0
    return quarter_returns


def solve_weight_program(
    case: Case,
    matrix: np.ndarray,
    rows: list[dict[str, Any]],
    a_ub: list[list[float]],
    b_ub: list[float],
) -> np.ndarray:
    n = len(rows)
    symbols = sorted({row["symbol"] for row in rows})
    symbol_index = {symbol: index for index, symbol in enumerate(symbols)}
    variable_count = n + len(symbols)

    constraints: list[list[float]] = []
    lower_bounds: list[float] = []
    upper_bounds: list[float] = []

    row = [0.0] * variable_count
    row[:n] = [1.0] * n
    constraints.append(row)
    lower_bounds.append(1.0)
    upper_bounds.append(1.0)

    for source, bound in zip(a_ub, b_ub):
        row = [0.0] * variable_count
        row[:n] = source
        constraints.append(row)
        lower_bounds.append(-np.inf)
        upper_bounds.append(bound)

    for symbol in symbols:
        y_var = n + symbol_index[symbol]

        row = [0.0] * variable_count
        for candidate_index, candidate in enumerate(rows):
            if candidate["symbol"] == symbol:
                row[candidate_index] = 1.0
        row[y_var] = -case.per_symbol_cap
        constraints.append(row)
        lower_bounds.append(-np.inf)
        upper_bounds.append(0.0)

        row = [0.0] * variable_count
        for candidate_index, candidate in enumerate(rows):
            if candidate["symbol"] == symbol:
                row[candidate_index] = -1.0
        row[y_var] = case.min_symbol_weight
        constraints.append(row)
        lower_bounds.append(-np.inf)
        upper_bounds.append(0.0)

    row = [0.0] * variable_count
    for symbol_var in range(n, variable_count):
        row[symbol_var] = 1.0
    constraints.append(row)
    lower_bounds.append(float(case.min_symbols))
    upper_bounds.append(float(len(symbols)))

    objective = np.concatenate([-matrix[-1], np.zeros(len(symbols))])
    result = milp(
        c=objective,
        integrality=np.concatenate([np.zeros(n), np.ones(len(symbols))]),
        bounds=Bounds(np.zeros(variable_count), np.ones(variable_count)),
        constraints=LinearConstraint(np.array(constraints), np.array(lower_bounds), np.array(upper_bounds)),
        options={"time_limit": 180.0, "mip_rel_gap": 0.001},
    )
    if result.success:
        return result.x[:n]

    # Fallback for diagnostics if min-symbol MILP cannot solve.
    result_lp = linprog(
        -matrix[-1],
        A_ub=np.array(a_ub) if a_ub else None,
        b_ub=np.array(b_ub) if b_ub else None,
        A_eq=np.array([np.ones(n)]),
        b_eq=np.array([1.0]),
        bounds=[(0.0, 1.0)] * n,
        method="highs",
    )
    if result_lp.success:
        return result_lp.x
    raise RuntimeError(f"{case.label}: {result.message}; fallback: {result_lp.message}")


def solve_case(case: Case, base_rows: list[dict[str, Any]]) -> dict[str, Any]:
    rows = reduce_rows(base_rows)
    if len({row["symbol"] for row in rows}) < case.min_symbols:
        raise RuntimeError(f"{case.label}: not enough symbols")

    grid = align_rows(rows)
    matrix = np.column_stack([row["equity"] for row in rows])
    qret = quarter_return_matrix(grid, matrix)
    a_ub: list[list[float]] = []
    b_ub: list[float] = []

    if case.quarter_floor is not None:
        for q in range(4):
            a_ub.append((-qret[q]).tolist())
            b_ub.append(-case.quarter_floor)

    alpha = 1.0 - case.dd_limit / 100.0
    added: set[tuple[int, int]] = set()
    for iteration in range(4000):
        weights = solve_weight_program(case, matrix, rows, a_ub, b_ub)
        equity = matrix @ weights
        peaks = np.maximum.accumulate(equity)
        drawdowns = (peaks - equity) / peaks
        worst_index = int(np.argmax(drawdowns))
        max_dd = float(drawdowns[worst_index] * 100.0)
        if max_dd <= case.dd_limit + 1e-8:
            years = (int(grid[-1]) - int(grid[0])) / (365.25 * 24 * 3600 * 1000)
            annualized = ((float(equity[-1]) / float(equity[0])) ** (1.0 / years) - 1.0) * 100.0
            active = [(i, float(weight)) for i, weight in enumerate(weights) if weight > 1e-7]
            active.sort(key=lambda pair: pair[1], reverse=True)
            realized_quarters = [float(qret[q] @ weights) for q in range(4)]
            return {
                "case": case,
                "rows": rows,
                "grid": grid,
                "equity": equity,
                "drawdowns": drawdowns,
                "weights": weights,
                "active": active,
                "annualized_return_pct": annualized,
                "max_drawdown_pct": max_dd,
                "return_pct": (float(equity[-1]) / float(equity[0]) - 1.0) * 100.0,
                "years": years,
                "quarter_returns": realized_quarters,
                "iterations": iteration,
                "candidate_count": len(rows),
            }
        peak_index = int(np.argmax(equity[: worst_index + 1]))
        key = (peak_index, worst_index)
        if key in added:
            bad = np.where(drawdowns > case.dd_limit / 100.0 + 1e-8)[0]
            new_count = 0
            for t in bad[np.argsort(drawdowns[bad])[-30:]]:
                p = int(np.argmax(equity[: int(t) + 1]))
                new_key = (p, int(t))
                if new_key not in added:
                    a_ub.append((alpha * matrix[p] - matrix[int(t)]).tolist())
                    b_ub.append(0.0)
                    added.add(new_key)
                    new_count += 1
            if new_count == 0:
                raise RuntimeError(f"{case.label}: repeated drawdown constraint {max_dd}")
        else:
            a_ub.append((alpha * matrix[peak_index] - matrix[worst_index]).tolist())
            b_ub.append(0.0)
            added.add(key)
    raise RuntimeError(f"{case.label}: max iterations")


def portfolio_direction(rows: list[dict[str, Any]], active: list[tuple[int, float]]) -> str:
    has_long = False
    has_short = False
    for index, _ in active:
        cfg = candidate_portfolio_config(rows[index]["config"])
        for strategy in cfg.get("strategies", []):
            direction = strategy.get("direction")
            has_long = has_long or direction == "long"
            has_short = has_short or direction == "short"
    if has_long and has_short:
        return "long_short"
    if has_short:
        return "short"
    return "long"


def portfolio_direction_mode(direction: str) -> str:
    return {"long_short": "long_and_short", "short": "short_only"}.get(direction, "long_only")


def weight_decimals(active: list[tuple[int, float]]) -> list[Decimal]:
    values = [Decimal(str(weight * 100.0)).quantize(Decimal("0.000001")) for _, weight in active]
    if values:
        values[-1] = Decimal("100.000000") - sum(values[:-1])
    return values


def build_saved_payload(result: dict[str, Any]) -> dict[str, Any]:
    case: Case = result["case"]
    rows = result["rows"]
    active = result["active"]
    weights_pct = weight_decimals(active)
    direction = portfolio_direction(rows, active)
    direction_mode = portfolio_direction_mode(direction)
    strategies: list[dict[str, Any]] = []
    items: list[dict[str, Any]] = []
    used_strategy_ids: set[str] = set()

    for rank, ((row_index, _), weight_pct) in enumerate(zip(active, weights_pct), start=1):
        row = rows[row_index]
        item_id = f"msi_margin_v2_{case.risk}_{rank:02d}"
        cfg = candidate_portfolio_config(row["config"])
        parameter_snapshot = {
            "portfolio_config": cfg,
            "source_task_id": row["task_id"],
            "source_candidate_id": row["candidate_id"],
            "source_risk_profile": row["source_risk_profile"],
            "lp_weight_pct": format(weight_pct, "f"),
            "margin_v2_capital_model": "first_order_quote is notional; margin = notional / leverage",
        }
        for strategy_index, strategy in enumerate(cfg.get("strategies", []), start=1):
            live_strategy = json.loads(json.dumps(strategy))
            original_strategy_id = live_strategy.get("strategy_id") or f"{row['candidate_id']}-{strategy_index}"
            strategy_id = original_strategy_id
            if strategy_id in used_strategy_ids:
                strategy_id = f"{original_strategy_id}-{rank:02d}-{strategy_index:02d}"
            used_strategy_ids.add(strategy_id)
            live_strategy["strategy_id"] = strategy_id
            live_strategy["source_strategy_id"] = original_strategy_id
            live_strategy["portfolio_weight_pct"] = format(weight_pct, "f")
            live_strategy["strategy_instance_id"] = item_id
            live_strategy["source_candidate_id"] = row["candidate_id"]
            live_strategy["source_task_id"] = row["task_id"]
            strategies.append(live_strategy)
        items.append(
            {
                "strategy_instance_id": item_id,
                "candidate_id": row["candidate_id"],
                "symbol": row["symbol"],
                "weight_pct": format(weight_pct, "f"),
                "leverage": max(int(strategy.get("leverage") or 1) for strategy in cfg.get("strategies", [{"leverage": 1}])),
                "parameter_snapshot": parameter_snapshot,
                "metrics_snapshot": row["summary"],
                "source_task_id": row["task_id"],
                "source_risk_profile": row["source_risk_profile"],
                "candidate_ann_pct": row["annualized_return_pct"],
                "candidate_dd_pct": row["max_drawdown_pct"],
                "candidate_planned_margin_quote": row["planned_margin_quote"],
                "candidate_planned_notional_quote": row["planned_notional_quote"],
                "candidate_max_capital_used_quote": row["max_capital_used_quote"],
                "trade_count": row["trade_count"],
                "task_direction_mode": row["task_direction_mode"],
            }
        )

    equity_curve = [{"timestamp_ms": int(ts), "equity_quote": float(eq * 10000.0)} for ts, eq in zip(result["grid"], result["equity"])]
    drawdown_curve = [{"timestamp_ms": int(ts), "drawdown_pct": float(dd * 100.0)} for ts, dd in zip(result["grid"], result["drawdowns"])]
    symbol_weights: dict[str, Decimal] = defaultdict(lambda: Decimal("0"))
    for item in items:
        symbol_weights[item["symbol"]] += Decimal(item["weight_pct"])
    summary = {
        "source": "margin_v2_linear_program_recombine_corrected_candidate_equity_curves",
        "risk_profile": case.risk,
        "annualized_return_target_pct": case.annualized_target,
        "annualized_return_pct": result["annualized_return_pct"],
        "annualized_target_passed": result["annualized_return_pct"] > case.annualized_target,
        "max_drawdown_pct": result["max_drawdown_pct"],
        "drawdown_limit_pct": case.dd_limit,
        "drawdown_limit_passed": result["max_drawdown_pct"] <= case.dd_limit + 1e-8,
        "return_pct": result["return_pct"],
        "trade_count": sum(item["trade_count"] for item in items),
        "years": result["years"],
        "member_count": len(items),
        "distinct_symbol_count": len(symbol_weights),
        "min_symbol_count_required": case.min_symbols,
        "per_symbol_cap_pct": case.per_symbol_cap * 100.0,
        "min_symbol_weight_pct": case.min_symbol_weight * 100.0,
        "quarter_floor_raw_return": case.quarter_floor,
        "quarter_returns_raw": result["quarter_returns"],
        "source_task_ids": list(SOURCE_TASK_IDS),
        "candidate_pool_count": result["candidate_count"],
        "symbols": list(symbol_weights.keys()),
        "symbol_weights_pct": {symbol: float(weight) for symbol, weight in sorted(symbol_weights.items())},
        "capital_model": {
            "first_order_quote": "order notional",
            "futures_margin": "notional / leverage",
            "returns_drawdown_denominator": "planned margin capital",
        },
        "equity_curve": equity_curve,
        "drawdown_curve": drawdown_curve,
        "members": [
            {
                "rank": rank,
                "candidate_id": item["candidate_id"],
                "source_task_id": item["source_task_id"],
                "source_risk_profile": item["source_risk_profile"],
                "symbol": item["symbol"],
                "weight_pct": item["weight_pct"],
                "candidate_annualized_return_pct": item["candidate_ann_pct"],
                "candidate_max_drawdown_pct": item["candidate_dd_pct"],
                "candidate_planned_margin_quote": item["candidate_planned_margin_quote"],
                "candidate_planned_notional_quote": item["candidate_planned_notional_quote"],
                "candidate_max_capital_used_quote": item["candidate_max_capital_used_quote"],
                "candidate_trade_count": item["trade_count"],
                "task_direction_mode": item["task_direction_mode"],
            }
            for rank, item in enumerate(items, start=1)
        ],
    }
    live_config = {
        "kind": "martingale_batch_portfolio",
        "market": "usd_m_futures",
        "direction": direction,
        "risk_profile": case.risk,
        "total_weight_pct": "100",
        "portfolio_config": {
            "direction_mode": direction_mode,
            "strategies": strategies,
            "risk_limits": {},
        },
        "execution": {
            "requires_connected_strategy_executor": True,
            "source": "margin_v2_lp_recombined_backtest_candidate_parameter_snapshot",
        },
    }
    risk_summary = {
        "source": "margin_v2_lp_recombine",
        "strategy_count": len(strategies),
        "enabled_strategy_count": len(strategies),
        "candidate_count": len(items),
        "distinct_symbol_count": len(symbol_weights),
        "min_symbol_count_required": case.min_symbols,
        "symbols": list(symbol_weights.keys()),
        "max_leverage": max((item["leverage"] for item in items), default=1),
        "total_weight_pct": "100",
        "annualized_return_target_pct": case.annualized_target,
        "annualized_return_pct": result["annualized_return_pct"],
        "annualized_target_passed": summary["annualized_target_passed"],
        "max_drawdown_pct": result["max_drawdown_pct"],
        "drawdown_limit_pct": case.dd_limit,
        "drawdown_limit_passed": summary["drawdown_limit_passed"],
        "return_pct": result["return_pct"],
        "trade_count": summary["trade_count"],
        "source_task_ids": list(SOURCE_TASK_IDS),
        "capital_model": summary["capital_model"],
    }
    task_config = {
        "strategy_type": "martingale_grid",
        "risk_profile": case.risk,
        "direction_mode": direction,
        "search_mode": "margin_v2_lp_recombine_existing_corrected_candidates",
        "start_ms": case.start_ms,
        "end_ms": case.end_ms,
        "drawdown_limit_pct": case.dd_limit,
        "annualized_return_target_pct": case.annualized_target,
        "per_symbol_weight_cap_pct": case.per_symbol_cap * 100.0,
        "min_symbols": case.min_symbols,
        "quarter_floor_raw_return": case.quarter_floor,
        "source_task_ids": list(SOURCE_TASK_IDS),
        "display_name": case.name,
        "curated_lp_portfolio_id": case.portfolio_id,
    }
    return {
        "case": case,
        "direction": direction,
        "direction_mode": direction_mode,
        "items": items,
        "live_config": live_config,
        "risk_summary": risk_summary,
        "summary": summary,
        "task_config": task_config,
    }


def portfolio_candidate_summary(payload: dict[str, Any]) -> dict[str, Any]:
    case: Case = payload["case"]
    summary = payload["summary"]
    top_row = {
        "portfolio_id": case.portfolio_id,
        "portfolio_rank": 1,
        "member_count": summary["member_count"],
        "members": [
            {
                "candidate_id": f"lp_member_{case.portfolio_id}_{member['candidate_id']}",
                "source_candidate_id": member["candidate_id"],
                "symbol": member["symbol"],
                "allocation_pct": float(member["weight_pct"]),
                "return_pct": member["candidate_annualized_return_pct"],
                "max_drawdown_pct": member["candidate_max_drawdown_pct"],
                "annualized_return_pct": member["candidate_annualized_return_pct"],
                "trade_count": member["candidate_trade_count"],
            }
            for member in summary["members"]
        ],
        "total_return_pct": summary["return_pct"],
        "return_pct": summary["return_pct"],
        "max_drawdown_pct": summary["max_drawdown_pct"],
        "annualized_return_pct": summary["annualized_return_pct"],
        "score": summary["annualized_return_pct"] / max(summary["max_drawdown_pct"], 1.0),
        "trade_count": summary["trade_count"],
        "equity_curve": summary["equity_curve"],
        "drawdown_curve": summary["drawdown_curve"],
        "eligible_candidate_count": summary["candidate_pool_count"],
        "capital_model": summary["capital_model"],
    }
    return top_row


def save_payloads(payloads: list[dict[str, Any]], hide_other_flyingkid_tasks: bool) -> None:
    sql_parts = ["BEGIN;"]
    sql_parts.append(
        """
CREATE TABLE IF NOT EXISTS backtest_tasks_backup_margin_v2_lp_display_20260626 AS
SELECT now() AS backup_created_at, *
FROM backtest_tasks
WHERE owner = 'flyingkid2022@outlook.com';
"""
    )
    keep_task_ids = ",".join(sql_string(payload["case"].task_id) for payload in payloads)
    if hide_other_flyingkid_tasks:
        sql_parts.append(
            f"""
UPDATE backtest_tasks
SET owner = 'archive+flyingkid2022@outlook.com',
    updated_at = now(),
    summary = summary || jsonb_build_object(
      'archived_for_margin_v2_lp_display', true,
      'archived_at', to_jsonb(now()::text),
      'archive_reason', 'Hidden so flyingkid sees only the three corrected margin-v2 LP backtest portfolios.'
    )
WHERE owner = {sql_string(OWNER)}
  AND task_id NOT IN ({keep_task_ids});
"""
        )
    for payload in payloads:
        case: Case = payload["case"]
        summary = payload["summary"]
        top_row = portfolio_candidate_summary(payload)
        task_summary = {
            **summary,
            "stage": "margin_v2_lp_portfolio_ready",
            "stage_label": "Corrected margin-v2 LP portfolio ready for review",
            "progress_pct": 100,
            "display_name": case.name,
            "portfolio_top_n": 1,
            "portfolio_top3": [top_row],
            "portfolio_top10": [top_row],
            "curated_lp_portfolio_id": case.portfolio_id,
        }
        item_sql = []
        for item in payload["items"]:
            item_sql.append(
                "INSERT INTO martingale_portfolio_items ("
                "strategy_instance_id, portfolio_id, candidate_id, symbol, weight_pct, leverage, enabled, status, parameter_snapshot, metrics_snapshot, created_at, updated_at"
                ") VALUES ("
                f"{sql_string(item['strategy_instance_id'])}, {sql_string(case.portfolio_id)}, {sql_string(item['candidate_id'])}, "
                f"{sql_string(item['symbol'])}, {item['weight_pct']}::NUMERIC, {item['leverage']}, true, 'pending_confirmation', "
                f"{jsonb(item['parameter_snapshot'])}, {jsonb(item['metrics_snapshot'])}, now(), now())"
            )
        sql_parts.append(
            f"""
INSERT INTO backtest_tasks (task_id, owner, status, strategy_type, config, summary, created_at, updated_at, started_at, completed_at)
VALUES ({sql_string(case.task_id)}, {sql_string(OWNER)}, 'succeeded', 'martingale_grid',
        {jsonb(payload['task_config'])}, {jsonb(task_summary)}, now(), now(), now(), now())
ON CONFLICT (task_id) DO UPDATE SET
  owner = EXCLUDED.owner,
  status = EXCLUDED.status,
  strategy_type = EXCLUDED.strategy_type,
  config = EXCLUDED.config,
  summary = EXCLUDED.summary,
  updated_at = now(),
  completed_at = now(),
  error_message = NULL;

DELETE FROM martingale_portfolio_items WHERE portfolio_id = {sql_string(case.portfolio_id)};
DELETE FROM martingale_portfolios WHERE portfolio_id = {sql_string(case.portfolio_id)};
INSERT INTO martingale_portfolios (
  portfolio_id, owner, name, status, source_task_id, market, direction, risk_profile, total_weight_pct, config, risk_summary, created_at, updated_at
) VALUES (
  {sql_string(case.portfolio_id)}, {sql_string(OWNER)}, {sql_string(case.name)}, 'pending_confirmation',
  {sql_string(case.task_id)}, 'usd_m_futures', {sql_string(payload['direction'])}, {sql_string(case.risk)},
  100::NUMERIC, {jsonb(payload['live_config'])}, {jsonb(payload['risk_summary'])}, now(), now()
);
{';'.join(item_sql)};

DELETE FROM backtest_candidate_summaries WHERE task_id = {sql_string(case.task_id)} AND candidate_id LIKE 'lp\\_%' ESCAPE '\\';
INSERT INTO backtest_candidate_summaries (candidate_id, task_id, status, rank, config, summary, created_at, updated_at)
VALUES ({sql_string('lp_portfolio_' + case.portfolio_id)}, {sql_string(case.task_id)}, 'ready', 0,
        {jsonb(payload['live_config'])},
        {jsonb({**summary, 'symbol': case.name, 'market': 'usd_m_futures', 'direction': payload['direction'], 'result_mode': 'Corrected margin-v2 LP recombined portfolio', 'search_mode': 'margin_v2_lp_recombine', 'score': top_row['score'], 'total_return_pct': summary['return_pct'], 'publishable': True, 'portfolio_id': case.portfolio_id, 'portfolio_group_key': case.portfolio_id, 'parameter_rank_for_symbol': 0})},
        now(), now())
ON CONFLICT (candidate_id) DO UPDATE SET
  task_id = EXCLUDED.task_id,
  status = EXCLUDED.status,
  rank = EXCLUDED.rank,
  config = EXCLUDED.config,
  summary = EXCLUDED.summary,
  updated_at = now();
"""
        )
        for rank, member in enumerate(summary["members"], start=1):
            member_id = f"lp_member_{case.portfolio_id}_{member['candidate_id']}"
            member_summary = {
                **member,
                "symbol": member["symbol"],
                "market": "usd_m_futures",
                "direction": payload["direction"],
                "result_mode": "Corrected margin-v2 LP portfolio member",
                "search_mode": "margin_v2_lp_recombine_member",
                "source_candidate_id": member["candidate_id"],
                "source_task_id": member["source_task_id"],
                "portfolio_id": case.portfolio_id,
                "portfolio_group_key": case.portfolio_id,
                "risk_profile": case.risk,
                "recommended_weight_pct": member["weight_pct"],
                "annualized_return_pct": member["candidate_annualized_return_pct"],
                "max_drawdown_pct": member["candidate_max_drawdown_pct"],
                "trade_count": member["candidate_trade_count"],
                "publishable": True,
            }
            sql_parts.append(
                f"""
INSERT INTO backtest_candidate_summaries (candidate_id, task_id, status, rank, config, summary, created_at, updated_at)
SELECT {sql_string(member_id)}, {sql_string(case.task_id)}, 'ready', {rank}, src.config, {jsonb(member_summary)}, now(), now()
FROM backtest_candidate_summaries src
WHERE src.candidate_id = {sql_string(member['candidate_id'])}
ON CONFLICT (candidate_id) DO UPDATE SET
  task_id = EXCLUDED.task_id,
  status = EXCLUDED.status,
  rank = EXCLUDED.rank,
  config = EXCLUDED.config,
  summary = EXCLUDED.summary,
  updated_at = now();
"""
            )
    sql_parts.append("COMMIT;")
    psql_exec("\n".join(sql_parts))


def write_reports(payloads: list[dict[str, Any]]) -> None:
    REPORT_MD.parent.mkdir(parents=True, exist_ok=True)
    serializable = []
    lines = [
        "# Corrected Margin-v2 LP Martingale Portfolios",
        "",
        "Date: 2026-06-26",
        "",
        "These portfolios are built only from corrected margin-v2 candidate curves. The capital model is: first_order_quote is order notional, futures margin is notional / leverage, and return/drawdown use planned margin capital as principal.",
        "",
        "| Mode | Target | Annualized | Max DD | Result | Members | Symbols |",
        "|---|---:|---:|---:|---|---:|---:|",
    ]
    for payload in payloads:
        case: Case = payload["case"]
        summary = payload["summary"]
        result = "PASS" if summary["annualized_target_passed"] and summary["drawdown_limit_passed"] else "FAIL"
        lines.append(
            f"| {case.risk} | >{case.annualized_target:.0f}% / <={case.dd_limit:.0f}% DD | "
            f"{summary['annualized_return_pct']:.4f}% | {summary['max_drawdown_pct']:.4f}% | "
            f"{result} | {summary['member_count']} | {summary['distinct_symbol_count']} |"
        )
    lines.append("")
    for payload in payloads:
        case = payload["case"]
        summary = payload["summary"]
        lines.extend(
            [
                f"## {case.risk.title()} - `{case.portfolio_id}`",
                f"- Task: `{case.task_id}`",
                f"- Annualized return: {summary['annualized_return_pct']:.4f}% (target > {case.annualized_target:.0f}%)",
                f"- Max drawdown: {summary['max_drawdown_pct']:.4f}% (limit <= {case.dd_limit:.0f}%)",
                f"- Total return: {summary['return_pct']:.2f}% over {summary['years']:.3f} years",
                f"- Portfolio quarter raw returns: {', '.join(f'{x * 100:.2f}%' for x in summary['quarter_returns_raw'])}",
                f"- Source tasks: {', '.join(summary['source_task_ids'])}",
                "",
                "| Weight | Symbol | Candidate | Source task | Source profile | Candidate ann | Candidate DD | Planned margin |",
                "|---:|---|---|---|---|---:|---:|---:|",
            ]
        )
        for member in summary["members"]:
            lines.append(
                f"| {member['weight_pct']}% | {member['symbol']} | `{member['candidate_id']}` | "
                f"`{member['source_task_id']}` | {member['source_risk_profile']} | "
                f"{member['candidate_annualized_return_pct']:.2f}% | {member['candidate_max_drawdown_pct']:.2f}% | "
                f"{member['candidate_planned_margin_quote']:.2f} |"
            )
        lines.append("")
        serializable.append(
            {
                "portfolio_id": case.portfolio_id,
                "task_id": case.task_id,
                "risk_profile": case.risk,
                "direction": payload["direction"],
                "summary": {k: v for k, v in summary.items() if k not in {"equity_curve", "drawdown_curve"}},
                "risk_summary": payload["risk_summary"],
            }
        )
    REPORT_MD.write_text("\n".join(lines), encoding="utf-8")
    REPORT_JSON.write_text(json.dumps(serializable, ensure_ascii=False, indent=2), encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--save", action="store_true", help="write the three corrected LP results to backtest/portfolio tables")
    parser.add_argument("--hide-other-flyingkid-tasks", action="store_true", help="archive other flyingkid backtest tasks from the visible list")
    args = parser.parse_args()

    base_rows = fetch_candidates()
    if not base_rows:
        raise RuntimeError("no corrected margin-v2 source candidates")
    print(f"source candidates={len(base_rows)} symbols={len({row['symbol'] for row in base_rows})} tasks={SOURCE_TASK_IDS}")

    payloads = []
    for case in CASES:
        result = solve_case(case, base_rows)
        payload = build_saved_payload(result)
        payloads.append(payload)
        summary = payload["summary"]
        status = "PASS" if summary["annualized_target_passed"] and summary["drawdown_limit_passed"] else "FAIL"
        print(
            f"{case.risk}: {status} ann={summary['annualized_return_pct']:.4f}% "
            f"target>{case.annualized_target:.0f}% dd={summary['max_drawdown_pct']:.4f}% "
            f"limit<={case.dd_limit:.0f}% members={summary['member_count']} "
            f"symbols={summary['distinct_symbol_count']} direction={payload['direction']}"
        )
        print("  quarter raw returns:", ", ".join(f"{x * 100:.2f}%" for x in summary["quarter_returns_raw"]))
        for member in summary["members"]:
            print(
                f"  {member['weight_pct']}% {member['symbol']:10s} {member['candidate_id']} "
                f"src={member['source_task_id']} ann={member['candidate_annualized_return_pct']:.2f}% "
                f"dd={member['candidate_max_drawdown_pct']:.2f}%"
            )
    write_reports(payloads)
    print(f"wrote {REPORT_MD}")
    print(f"wrote {REPORT_JSON}")
    if args.save:
        save_payloads(payloads, hide_other_flyingkid_tasks=args.hide_other_flyingkid_tasks)
        print("saved corrected margin-v2 LP portfolios")


if __name__ == "__main__":
    main()
