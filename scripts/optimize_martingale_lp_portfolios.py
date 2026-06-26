#!/usr/bin/env python3
"""Optimize and optionally save martingale LP recombination portfolios.

This script recombines existing `backtest_candidate_summaries` equity curves with a
linear program. It maximizes final portfolio equity under a true curve drawdown
constraint and a per-symbol weight cap, then can save deterministic portfolio
records for live review.

Default mode is read-only. Use `--save` to upsert the three curated portfolios.
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

DB = ["docker", "exec", "-i", "grid-binance-postgres-1", "psql", "-U", "postgres", "-d", "grid_binance"]
DB_AT = ["docker", "exec", "grid-binance-postgres-1", "psql", "-U", "postgres", "-d", "grid_binance", "-qAt", "-c"]
OWNER = "flyingkid2022@outlook.com"
REPORT_MD = Path("docs/superpowers/reports/2026-06-23-martingale-lp-portfolios.md")
REPORT_JSON = Path("docs/superpowers/reports/2026-06-23-martingale-lp-portfolios.json")


@dataclass(frozen=True)
class Case:
    label: str
    task_id: str
    portfolio_id: str
    name: str
    risk: str
    start_ms: int
    end_ms: int
    directions: tuple[str, ...] | None
    dd_limit: float
    baseline_ann: float | None
    hard_dd: float
    min_symbols: int = 8
    min_symbol_weight: float = 0.01


CASES = [
    Case(
        label="conservative_long_short_dd9_80",
        task_id="lp-martingale-conservative-20260623",
        portfolio_id="mp_lp_conservative_20260623",
        name="LP Conservative 67.7 ann / 9.8 dd",
        risk="conservative",
        start_ms=1672531200000,
        end_ms=1780271999999,
        directions=("long_short",),
        dd_limit=9.8,
        baseline_ann=None,
        hard_dd=10.0,
        min_symbols=8,
    ),
    Case(
        label="balanced_full_dd19_50",
        task_id="lp-martingale-balanced-20260623",
        portfolio_id="mp_lp_balanced_20260623",
        name="LP Balanced 105.5 ann / 18.8 dd",
        risk="balanced",
        start_ms=1672531200000,
        end_ms=1777593599999,
        directions=None,
        dd_limit=19.5,
        baseline_ann=65.52,
        hard_dd=20.0,
        min_symbols=8,
    ),
    Case(
        label="aggressive_full_dd29_50",
        task_id="lp-martingale-aggressive-20260623",
        portfolio_id="mp_lp_aggressive_20260623",
        name="LP Aggressive 118.7 ann / 29.5 dd",
        risk="aggressive",
        start_ms=1672531200000,
        end_ms=1777593599999,
        directions=None,
        dd_limit=29.5,
        baseline_ann=77.00,
        hard_dd=30.0,
        min_symbols=8,
    ),
]


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


def fetch_candidates(case: Case) -> list[dict[str, Any]]:
    where = [
        "bcs.status = 'ready'",
        f"bt.config->>'risk_profile' = {sql_string(case.risk)}",
        "jsonb_array_length(bcs.summary->'equity_curve') >= 100",
        "bcs.candidate_id not like 'lp\\_%' escape '\\'",
        "bcs.summary->>'search_mode' is distinct from 'lp_recombine'",
        "bcs.summary->>'search_mode' is distinct from 'lp_recombine_member'",
    ]
    if case.directions:
        dirs = ",".join(sql_string(direction) for direction in case.directions)
        where.append(f"coalesce(bt.config->>'direction_mode','') in ({dirs})")
    sql = f"""
copy (
select bcs.candidate_id,
       bcs.task_id,
       coalesce(bt.config->>'direction_mode','') as task_direction_mode,
       coalesce(bcs.summary->>'symbol', bcs.config->'strategies'->0->>'symbol') as symbol,
       coalesce(bcs.summary->>'direction_mode', bcs.config->>'direction_mode', '') as candidate_direction_mode,
       (bcs.summary->>'annualized_return_pct')::float8 as annualized_return_pct,
       (bcs.summary->>'max_drawdown_pct')::float8 as max_drawdown_pct,
       coalesce((bcs.summary->>'trade_count')::float8, 0) as trade_count,
       jsonb_array_length(bcs.summary->'equity_curve') as equity_curve_len,
       (bcs.summary->'equity_curve')::text as equity_curve,
       bcs.config::text as config,
       bcs.summary::text as summary
from backtest_candidate_summaries bcs
join backtest_tasks bt on bt.task_id = bcs.task_id
where {' and '.join(where)}
) to stdout with csv
"""
    output = psql_at(sql)
    rows: list[dict[str, Any]] = []
    for record in csv.reader(io.StringIO(output)):
        if not record:
            continue
        (
            candidate_id,
            task_id,
            task_direction_mode,
            symbol,
            candidate_direction_mode,
            ann,
            dd,
            trade_count,
            equity_curve_len,
            equity_curve_text,
            config_text,
            summary_text,
        ) = record
        equity_curve = json.loads(equity_curve_text)
        timestamps = np.array([int(point["timestamp_ms"]) for point in equity_curve], dtype=np.int64)
        values = np.array([float(point["equity_quote"]) for point in equity_curve], dtype=np.float64)
        years = (int(timestamps[-1]) - int(timestamps[0])) / (365.25 * 24 * 3600 * 1000)
        if abs(int(timestamps[0]) - case.start_ms) > 3_600_000:
            continue
        if abs(int(timestamps[-1]) - (case.end_ms - 59_999)) > 3_600_000:
            continue
        if years < 3.0 or values[0] <= 0.0 or values[-1] <= 0.0:
            continue
        rows.append(
            {
                "candidate_id": candidate_id,
                "task_id": task_id,
                "task_direction_mode": task_direction_mode,
                "symbol": symbol,
                "candidate_direction_mode": candidate_direction_mode,
                "annualized_return_pct": float(ann),
                "max_drawdown_pct": float(dd),
                "trade_count": int(float(trade_count)),
                "equity_curve_len": int(equity_curve_len),
                "timestamps_raw": timestamps,
                "equity_raw": values / values[0],
                "config": json.loads(config_text),
                "summary": json.loads(summary_text),
            }
        )
    return rows


def reduce_rows(rows: list[dict[str, Any]], max_rows: int = 1500) -> list[dict[str, Any]]:
    by_symbol: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        by_symbol[row["symbol"]].append(row)
    keep: list[dict[str, Any]] = []
    seen: set[str] = set()
    for candidates in by_symbol.values():
        picks = []
        picks += sorted(candidates, key=lambda row: row["annualized_return_pct"], reverse=True)[:120]
        picks += sorted(candidates, key=lambda row: row["max_drawdown_pct"])[:50]
        picks += sorted(
            candidates,
            key=lambda row: row["annualized_return_pct"] / max(row["max_drawdown_pct"], 1e-9),
            reverse=True,
        )[:120]
        for pick in picks:
            if pick["candidate_id"] not in seen:
                keep.append(pick)
                seen.add(pick["candidate_id"])
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


def solve_weight_program(
    case: Case,
    matrix: np.ndarray,
    rows: list[dict[str, Any]],
    a_ub: list[list[float]],
    b_ub: list[float],
) -> np.ndarray:
    n = len(rows)
    if case.min_symbols <= 1:
        result = linprog(
            -matrix[-1],
            A_ub=np.array(a_ub),
            b_ub=np.array(b_ub),
            A_eq=np.array([np.ones(n)]),
            b_eq=np.array([1.0]),
            bounds=[(0.0, 1.0)] * n,
            method="highs",
        )
        if not result.success:
            raise RuntimeError(f"{case.label}: {result.message}")
        return result.x

    symbols = sorted({row["symbol"] for row in rows})
    symbol_count = len(symbols)
    symbol_index = {symbol: index for index, symbol in enumerate(symbols)}
    variable_count = n + symbol_count

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

    cap = 0.40
    for symbol in symbols:
        y_var = n + symbol_index[symbol]

        row = [0.0] * variable_count
        for candidate_index, candidate in enumerate(rows):
            if candidate["symbol"] == symbol:
                row[candidate_index] = 1.0
        row[y_var] = -cap
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
    upper_bounds.append(float(symbol_count))

    objective = np.concatenate([-matrix[-1], np.zeros(symbol_count)])
    lb = np.zeros(variable_count)
    ub = np.ones(variable_count)
    integrality = np.concatenate([np.zeros(n), np.ones(symbol_count)])
    result = milp(
        c=objective,
        integrality=integrality,
        bounds=Bounds(lb, ub),
        constraints=LinearConstraint(
            np.array(constraints),
            np.array(lower_bounds),
            np.array(upper_bounds),
        ),
        options={"time_limit": 120.0, "mip_rel_gap": 0.001},
    )
    if not result.success:
        raise RuntimeError(f"{case.label}: {result.message}")
    return result.x[:n]


def solve_case(case: Case) -> dict[str, Any]:
    rows = reduce_rows(fetch_candidates(case), max_rows=800 if case.min_symbols > 1 else 1500)
    if not rows:
        raise RuntimeError(f"{case.label}: no candidates")
    grid = align_rows(rows)
    matrix = np.column_stack([row["equity"] for row in rows])
    n = len(rows)
    a_ub: list[list[float]] = []
    b_ub: list[float] = []
    alpha = 1.0 - case.dd_limit / 100.0
    added: set[tuple[int, int]] = set()

    for iteration in range(3000):
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
                "iterations": iteration,
                "constraints": len(a_ub),
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
        config = candidate_portfolio_config(rows[index]["config"])
        for strategy in config.get("strategies", []):
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


def candidate_portfolio_config(config: dict[str, Any]) -> dict[str, Any]:
    return config.get("portfolio_config") or config


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
        item_id = f"msi_{case.risk}_lp_{rank:02d}"
        cfg = candidate_portfolio_config(row["config"])
        parameter_snapshot = {
            "portfolio_config": cfg,
            "source_task_id": row["task_id"],
            "source_candidate_id": row["candidate_id"],
            "lp_weight_pct": format(weight_pct, "f"),
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
            strategies.append(live_strategy)
        items.append(
            {
                "strategy_instance_id": item_id,
                "candidate_id": row["candidate_id"],
                "symbol": row["symbol"],
                "weight_pct": format(weight_pct, "f"),
                "leverage": max(
                    int(strategy.get("leverage") or 1)
                    for strategy in cfg.get("strategies", [{"leverage": 1}])
                ),
                "parameter_snapshot": parameter_snapshot,
                "metrics_snapshot": row["summary"],
                "source_task_id": row["task_id"],
                "candidate_ann_pct": row["annualized_return_pct"],
                "candidate_dd_pct": row["max_drawdown_pct"],
                "trade_count": row["trade_count"],
                "task_direction_mode": row["task_direction_mode"],
            }
        )

    equity_curve = [
        {"timestamp_ms": int(ts), "equity_quote": float(eq * 10000.0)}
        for ts, eq in zip(result["grid"], result["equity"])
    ]
    drawdown_curve = [
        {"timestamp_ms": int(ts), "drawdown_pct": float(dd * 100.0)}
        for ts, dd in zip(result["grid"], result["drawdowns"])
    ]
    summary = {
        "source": "linear_program_recombine_existing_candidate_equity_curves",
        "risk_profile": case.risk,
        "annualized_return_pct": result["annualized_return_pct"],
        "max_drawdown_pct": result["max_drawdown_pct"],
        "return_pct": result["return_pct"],
        "trade_count": sum(item["trade_count"] for item in items),
        "years": result["years"],
        "drawdown_limit_pct": case.dd_limit,
        "hard_drawdown_limit_pct": case.hard_dd,
        "baseline_annualized_return_pct": case.baseline_ann,
        "baseline_exceeded": None if case.baseline_ann is None else result["annualized_return_pct"] > case.baseline_ann,
        "member_count": len(items),
        "distinct_symbol_count": len({item["symbol"] for item in items}),
        "min_symbol_count_required": case.min_symbols,
        "symbols": [item["symbol"] for item in items],
        "equity_curve": equity_curve,
        "drawdown_curve": drawdown_curve,
        "members": [
            {
                "rank": rank,
                "candidate_id": item["candidate_id"],
                "source_task_id": item["source_task_id"],
                "symbol": item["symbol"],
                "weight_pct": item["weight_pct"],
                "candidate_annualized_return_pct": item["candidate_ann_pct"],
                "candidate_max_drawdown_pct": item["candidate_dd_pct"],
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
            "source": "lp_recombined_backtest_candidate_parameter_snapshot",
        },
    }
    risk_summary = {
        "source": "lp_recombine",
        "strategy_count": len(strategies),
        "enabled_strategy_count": len(strategies),
        "candidate_count": len(items),
        "distinct_symbol_count": len({item["symbol"] for item in items}),
        "min_symbol_count_required": case.min_symbols,
        "symbols": [item["symbol"] for item in items],
        "max_leverage": max((item["leverage"] for item in items), default=1),
        "total_weight_pct": "100",
        "annualized_return_pct": result["annualized_return_pct"],
        "max_drawdown_pct": result["max_drawdown_pct"],
        "return_pct": result["return_pct"],
        "trade_count": summary["trade_count"],
        "drawdown_limit_pct": case.dd_limit,
        "baseline_annualized_return_pct": case.baseline_ann,
        "baseline_exceeded": summary["baseline_exceeded"],
    }
    task_config = {
        "strategy_type": "martingale_grid",
        "risk_profile": case.risk,
        "direction_mode": direction,
        "search_mode": "lp_recombine_existing_candidates",
        "start_ms": case.start_ms,
        "end_ms": case.end_ms,
        "drawdown_limit_pct": case.dd_limit,
        "per_symbol_weight_cap_pct": 40.0,
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


def save_payload(payload: dict[str, Any]) -> None:
    case: Case = payload["case"]
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
    sql = f"""
BEGIN;
INSERT INTO backtest_tasks (task_id, owner, status, strategy_type, config, summary, created_at, updated_at, started_at, completed_at)
VALUES ({sql_string(case.task_id)}, {sql_string(OWNER)}, 'succeeded', 'martingale_grid', {jsonb(payload['task_config'])}, {jsonb(payload['summary'])}, now(), now(), now(), now())
ON CONFLICT (task_id) DO UPDATE SET
  owner = EXCLUDED.owner,
  status = EXCLUDED.status,
  strategy_type = EXCLUDED.strategy_type,
  config = EXCLUDED.config,
  summary = EXCLUDED.summary,
  updated_at = now(),
  completed_at = now(),
  error_message = NULL;
DELETE FROM martingale_portfolios WHERE portfolio_id = {sql_string(case.portfolio_id)};
INSERT INTO martingale_portfolios (
  portfolio_id, owner, name, status, source_task_id, market, direction, risk_profile, total_weight_pct, config, risk_summary, created_at, updated_at
) VALUES (
  {sql_string(case.portfolio_id)}, {sql_string(OWNER)}, {sql_string(case.name)}, 'pending_confirmation', {sql_string(case.task_id)},
  'usd_m_futures', {sql_string(payload['direction'])}, {sql_string(case.risk)}, 100::NUMERIC, {jsonb(payload['live_config'])}, {jsonb(payload['risk_summary'])}, now(), now()
);
{';'.join(item_sql)};
COMMIT;
"""
    psql_exec(sql)


def write_reports(payloads: list[dict[str, Any]]) -> None:
    REPORT_MD.parent.mkdir(parents=True, exist_ok=True)
    serializable = []
    lines = [
        "# Martingale LP Portfolio Results (2026-06-23)",
        "",
        "All three portfolios are recombinations of existing full-window candidate equity curves. Drawdown is computed from the blended equity curve, not from weighted single-candidate drawdowns.",
        "",
    ]
    for payload in payloads:
        case: Case = payload["case"]
        summary = payload["summary"]
        lines.extend(
            [
                f"## {case.risk.title()} - `{case.portfolio_id}`",
                f"- Status: `pending_confirmation`",
                f"- Annualized return: {summary['annualized_return_pct']:.4f}%",
                f"- Max drawdown: {summary['max_drawdown_pct']:.4f}% (limit {case.hard_dd:.2f}%)",
                f"- Total return: {summary['return_pct']:.2f}% over {summary['years']:.3f} years",
                f"- Direction: `{payload['direction']}`; source task: `{case.task_id}`",
            ]
        )
        if case.baseline_ann is not None:
            lines.append(f"- Baseline ann: {case.baseline_ann:.2f}% -> exceeded: `{summary['baseline_exceeded']}`")
        lines.append("")
        lines.append("| Weight | Symbol | Candidate | Source task | Candidate ann | Candidate DD |")
        lines.append("|---:|---|---|---|---:|---:|")
        for member in summary["members"]:
            lines.append(
                f"| {member['weight_pct']}% | {member['symbol']} | `{member['candidate_id']}` | `{member['source_task_id']}` | "
                f"{member['candidate_annualized_return_pct']:.2f}% | {member['candidate_max_drawdown_pct']:.2f}% |"
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
    parser.add_argument("--save", action="store_true", help="upsert the LP tasks and martingale portfolio records")
    args = parser.parse_args()

    payloads = []
    for case in CASES:
        result = solve_case(case)
        payload = build_saved_payload(result)
        payloads.append(payload)
        summary = payload["summary"]
        print(
            f"{case.risk}: ann={summary['annualized_return_pct']:.4f}% "
            f"dd={summary['max_drawdown_pct']:.4f}% ret={summary['return_pct']:.2f}% "
            f"members={len(summary['members'])} direction={payload['direction']}"
        )
        for member in summary["members"]:
            print(
                f"  {member['weight_pct']}% {member['symbol']} {member['candidate_id']} "
                f"ann={member['candidate_annualized_return_pct']:.2f}% dd={member['candidate_max_drawdown_pct']:.2f}%"
            )
    write_reports(payloads)
    print(f"wrote {REPORT_MD}")
    print(f"wrote {REPORT_JSON}")
    if args.save:
        for payload in payloads:
            save_payload(payload)
        print("saved LP portfolios to martingale_portfolios")


if __name__ == "__main__":
    main()
