#!/usr/bin/env python3
"""Run budget-aware P4 martingale searches in parallel by symbol and guard set."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import time
from pathlib import Path
from typing import Any


DEFAULT_REPO = Path("/home/bumblebee/Project/grid_binance/.claude/worktrees/p4-cycle-exit")
MAIN_REPO = Path("/home/bumblebee/Project/grid_binance")
FULL_START = "1672531200000"
FULL_END = "1780271999999"
DEFAULT_SYMBOLS = [
    "BTCUSDT",
    "TRXUSDT",
    "XRPUSDT",
    "BCHUSDT",
    "ETCUSDT",
    "LTCUSDT",
    "HBARUSDT",
    "DOTUSDT",
    "UNIUSDT",
    "ETHUSDT",
    "DYDXUSDT",
    "LINKUSDT",
    "AVAXUSDT",
    "BNBUSDT",
    "CRVUSDT",
    "NEARUSDT",
    "GALAUSDT",
    "AAVEUSDT",
    "FILUSDT",
    "SOLUSDT",
    "ADAUSDT",
    "INJUSDT",
    "COMPUSDT",
    "APTUSDT",
    "ICPUSDT",
    "ALGOUSDT",
    "DOGEUSDT",
]
GUARD_SETS = {
    "default": {"dd": "6.0", "atr": "2.0", "adx": "45.0"},
    "strict": {"dd": "4.0", "atr": "1.5", "adx": "40.0"},
    "no_atr_pause": {"dd": "5.5", "atr": "50.0", "adx": "45.0"},
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", default=str(DEFAULT_REPO))
    parser.add_argument("--out-dir", required=True)
    parser.add_argument("--symbols", default=",".join(DEFAULT_SYMBOLS))
    parser.add_argument("--guards", default="default,strict")
    parser.add_argument("--workers", type=int, default=18)
    parser.add_argument("--budgets", default="5000")
    parser.add_argument("--grid", default="small")
    parser.add_argument("--max-params", type=int, default=40)
    parser.add_argument("--top-n", type=int, default=200)
    parser.add_argument("--start-ms", default=FULL_START)
    parser.add_argument("--end-ms", default=FULL_END)
    parser.add_argument(
        "--entry-filters",
        default="none,trend,trend_rsi,rsi_moderate,bb_moderate,rsi_bb_moderate",
    )
    parser.add_argument(
        "--direction-modes",
        default="long_only,short_only,long_and_short",
    )
    parser.add_argument("--regime-break", default="none,ema50,ema100")
    parser.add_argument("--max-cycle-age", default="none,24,48,72,120,168")
    return parser.parse_args()


def safe_name(text: str) -> str:
    return "".join(ch if ch.isalnum() or ch in "._-" else "_" for ch in text)


def command_for(args: argparse.Namespace, symbol: str, guard_name: str, out_path: Path) -> list[str]:
    repo = Path(args.repo)
    return [
        str(repo / "target/release/search_small_capital_martingale"),
        "--budgets",
        args.budgets,
        "--symbols",
        symbol,
        "--direction-modes",
        args.direction_modes,
        "--entry-filters",
        args.entry_filters,
        "--regime-break",
        args.regime_break,
        "--max-cycle-age",
        args.max_cycle_age,
        "--start-ms",
        args.start_ms,
        "--end-ms",
        args.end_ms,
        "--market-data",
        str(MAIN_REPO / "data/market_data_full.db"),
        "--funding-data",
        str(MAIN_REPO / "data/funding_rates.db"),
        "--output",
        str(out_path),
        "--top-n",
        str(args.top_n),
        "--grid",
        args.grid,
        "--max-params-per-symbol-budget",
        str(args.max_params),
    ]


def env_for(guard_name: str) -> dict[str, str]:
    guard = GUARD_SETS[guard_name]
    env = os.environ.copy()
    env["MARTINGALE_BT_NEW_CYCLE_DD_PAUSE_PCT"] = guard["dd"]
    env["MARTINGALE_BT_NEW_CYCLE_ATR_PAUSE_PCT"] = guard["atr"]
    env["MARTINGALE_BT_SAFETY_SKIP_ADX"] = guard["adx"]
    return env


def collect_rows(report: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for bucket in (report.get("best_by_budget") or {}).values():
        rows.extend(bucket or [])
    for frontiers in (report.get("frontier_by_budget") or {}).values():
        for bucket in (frontiers or {}).values():
            rows.extend(bucket or [])
    rows.extend(report.get("pass_candidates") or [])
    return rows


def dedupe_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    seen: set[tuple[Any, ...]] = set()
    keys = [
        "symbol",
        "direction_mode",
        "entry_filter",
        "first_order_quote",
        "leverage",
        "multiplier",
        "max_legs",
        "step_bps",
        "take_profit_bps",
        "cooldown_seconds",
        "adx_min",
        "stop_loss_bps",
        "regime_break_ema_period",
        "max_cycle_age_hours",
        "new_cycle_drawdown_pause_pct",
        "new_cycle_atr_pause_pct",
        "safety_skip_adx_threshold",
    ]
    for row in rows:
        key = tuple(row.get(k) for k in keys)
        if key in seen:
            continue
        seen.add(key)
        out.append(row)
    return out


def summarize(out_dir: Path) -> dict[str, Any]:
    rows: list[dict[str, Any]] = []
    reports = 0
    for path in sorted(out_dir.glob("*.json")):
        if path.name == "summary.json":
            continue
        try:
            report = json.loads(path.read_text())
        except Exception:
            continue
        reports += 1
        for row in collect_rows(report):
            row = dict(row)
            row["_source_file"] = str(path)
            rows.append(row)
    rows = dedupe_rows(rows)

    def ok(row: dict[str, Any]) -> bool:
        return not row.get("principal_breached") and float(row.get("annualized_return_pct", -999)) > -900

    def top_by_ann(pred, limit: int = 30) -> list[dict[str, Any]]:
        subset = [row for row in rows if ok(row) and pred(row)]
        subset.sort(key=lambda row: (-float(row.get("annualized_return_pct", -999)), float(row.get("max_drawdown_pct", 999))))
        return subset[:limit]

    def low_dd_over(target: float, limit: int = 30) -> list[dict[str, Any]]:
        subset = [row for row in rows if ok(row) and float(row.get("annualized_return_pct", -999)) >= target]
        subset.sort(key=lambda row: (float(row.get("max_drawdown_pct", 999)), -float(row.get("annualized_return_pct", -999))))
        return subset[:limit]

    summary = {
        "reports": reports,
        "rows": len(rows),
        "passes": {
            "conservative": top_by_ann(lambda r: float(r.get("annualized_return_pct", -999)) >= 50 and float(r.get("max_drawdown_pct", 999)) <= 10, 50),
            "balanced": top_by_ann(lambda r: float(r.get("annualized_return_pct", -999)) >= 90 and float(r.get("max_drawdown_pct", 999)) <= 20, 50),
            "aggressive": top_by_ann(lambda r: float(r.get("annualized_return_pct", -999)) >= 110 and float(r.get("max_drawdown_pct", 999)) <= 30, 50),
        },
        "frontier": {
            "best_under_dd10": top_by_ann(lambda r: float(r.get("max_drawdown_pct", 999)) <= 10),
            "best_under_dd20": top_by_ann(lambda r: float(r.get("max_drawdown_pct", 999)) <= 20),
            "best_under_dd30": top_by_ann(lambda r: float(r.get("max_drawdown_pct", 999)) <= 30),
            "lowest_dd_over_ann50": low_dd_over(50),
            "lowest_dd_over_ann90": low_dd_over(90),
            "lowest_dd_over_ann110": low_dd_over(110),
        },
    }
    (out_dir / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n")
    return summary


def print_row(row: dict[str, Any]) -> None:
    print(
        "  {symbol:<10} {direction_mode:<14} {entry_filter:<16} "
        "ann={annualized_return_pct:>7.2f} dd={max_drawdown_pct:>6.2f} "
        "ret={total_return_pct:>7.2f} cap={max_capital_used_quote:>8.1f} "
        "fo={first_order_quote} lev={leverage} m={multiplier} legs={max_legs} "
        "step={step_bps} tp={take_profit_bps} sl={stop_loss_bps} rb={regime_break_ema_period} "
        "age={max_cycle_age_hours} guard={new_cycle_drawdown_pause_pct}/{new_cycle_atr_pause_pct}/{safety_skip_adx_threshold}".format(**row),
        flush=True,
    )


def main() -> int:
    args = parse_args()
    repo = Path(args.repo)
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    symbols = [s.strip().upper() for s in args.symbols.split(",") if s.strip()]
    guard_names = [g.strip() for g in args.guards.split(",") if g.strip()]
    for guard in guard_names:
        if guard not in GUARD_SETS:
            raise SystemExit(f"unknown guard set {guard}; choose {sorted(GUARD_SETS)}")

    tasks = [(symbol, guard) for guard in guard_names for symbol in symbols]
    running: list[dict[str, Any]] = []
    pending = tasks[:]
    done = 0
    started = time.time()
    print(
        f"START parallel_search tasks={len(tasks)} workers={args.workers} "
        f"repo={repo} out={out_dir} budgets={args.budgets} grid={args.grid} max_params={args.max_params}",
        flush=True,
    )
    while pending or running:
        while pending and len(running) < args.workers:
            symbol, guard = pending.pop(0)
            name = f"{safe_name(guard)}_{safe_name(symbol)}"
            out_path = out_dir / f"{name}.json"
            err_path = out_dir / f"{name}.stderr.log"
            if out_path.exists() and out_path.stat().st_size > 0:
                print(f"SKIP {name}", flush=True)
                done += 1
                continue
            err_f = err_path.open("w")
            proc = subprocess.Popen(
                command_for(args, symbol, guard, out_path),
                cwd=repo,
                env=env_for(guard),
                stdout=subprocess.DEVNULL,
                stderr=err_f,
                text=True,
            )
            running.append(
                {
                    "name": name,
                    "symbol": symbol,
                    "guard": guard,
                    "proc": proc,
                    "err_f": err_f,
                    "out": out_path,
                    "t0": time.time(),
                }
            )
            print(f"RUN {name} pid={proc.pid}", flush=True)

        still: list[dict[str, Any]] = []
        for job in running:
            proc = job["proc"]
            rc = proc.poll()
            if rc is None:
                still.append(job)
                continue
            proc.wait()
            job["err_f"].close()
            elapsed = time.time() - job["t0"]
            done += 1
            if rc == 0:
                try:
                    report = json.loads(job["out"].read_text())
                    rows = dedupe_rows(collect_rows(report))
                    best = sorted(
                        rows,
                        key=lambda r: (
                            -float(r.get("annualized_return_pct", -999)),
                            float(r.get("max_drawdown_pct", 999)),
                        ),
                    )[:1]
                    if best:
                        b = best[0]
                        print(
                            f"DONE {job['name']} rows={len(rows)} "
                            f"best_ann={float(b.get('annualized_return_pct', 0)):.2f} "
                            f"best_dd={float(b.get('max_drawdown_pct', 0)):.2f} "
                            f"sec={elapsed:.1f} ({done}/{len(tasks)})",
                            flush=True,
                        )
                    else:
                        print(f"DONE {job['name']} rows=0 sec={elapsed:.1f}", flush=True)
                except Exception as exc:
                    print(f"BAD {job['name']} rc=0 err={exc} sec={elapsed:.1f}", flush=True)
            else:
                print(f"ERROR {job['name']} rc={rc} sec={elapsed:.1f}", flush=True)
        running = still
        if pending or running:
            if int(time.time() - started) % 60 < 2:
                print(
                    f"PROGRESS done={done}/{len(tasks)} running={len(running)} pending={len(pending)} elapsed={time.time()-started:.1f}s",
                    flush=True,
                )
            time.sleep(2)

    summary = summarize(out_dir)
    print(
        f"SUMMARY reports={summary['reports']} rows={summary['rows']} "
        f"passes C/B/A={len(summary['passes']['conservative'])}/"
        f"{len(summary['passes']['balanced'])}/{len(summary['passes']['aggressive'])}",
        flush=True,
    )
    for key in ["best_under_dd10", "best_under_dd20", "best_under_dd30", "lowest_dd_over_ann50", "lowest_dd_over_ann90", "lowest_dd_over_ann110"]:
        print(f"\n{key}", flush=True)
        for row in summary["frontier"][key][:8]:
            print_row(row)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
