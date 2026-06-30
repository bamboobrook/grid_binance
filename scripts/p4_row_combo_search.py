#!/usr/bin/env python3
"""Build live-parity portfolio combos from P4 single-row search reports.

This reads `search_small_capital_martingale` JSON outputs, rebuilds the same
strategy configs, scales first orders to a shared margin budget, and validates
with `portfolio_budget_replay`.
"""
from __future__ import annotations

import argparse
import concurrent.futures as futures
import glob
import itertools
import json
import math
import os
import random
import subprocess
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


FULL = (1672531200000, 1780271999999)
SEGMENTS = {
    "h1_2023": (1672531200000, 1688169599999),
    "h2_2023": (1688169600000, 1704067199999),
    "2024": (1704067200000, 1735689599999),
    "2025": (1735689600000, 1767225599999),
    "2026_ytd": (1767225600000, 1780271999999),
}
TARGETS = {
    "conservative": {"ann": 50.0, "dd": 10.0, "min_pos": 4, "max_seg_dd": 12.0, "h1": 0.45},
    "balanced": {"ann": 90.0, "dd": 20.0, "min_pos": 4, "max_seg_dd": 24.0, "h1": 0.50},
    "aggressive": {"ann": 110.0, "dd": 30.0, "min_pos": 3, "max_seg_dd": 36.0, "h1": 0.55},
}


def f(value: Any, default: float = 0.0) -> float:
    try:
        if value is None:
            return default
        out = float(value)
        return out if math.isfinite(out) else default
    except Exception:
        return default


def fmt(value: Any) -> str:
    if isinstance(value, str):
        return value
    text = f"{float(value):.10f}".rstrip("0").rstrip(".")
    return text if text else "0"


def entry_filter_expressions(entry_filter: str, direction: str) -> list[str]:
    if entry_filter in ("trend", "trend_rsi"):
        out = ["close > ema(200)" if direction == "long" else "close < ema(200)"]
        if entry_filter == "trend_rsi":
            out.append("rsi(14) < 65" if direction == "long" else "rsi(14) > 35")
        return out
    if entry_filter == "rsi_extreme":
        return ["rsi(14) < 30" if direction == "long" else "rsi(14) > 70"]
    if entry_filter == "rsi_moderate":
        return ["rsi(14) < 35" if direction == "long" else "rsi(14) > 65"]
    if entry_filter == "bb_extreme":
        return ["close < bb_lower(20,2.5)" if direction == "long" else "close > bb_upper(20,2.5)"]
    if entry_filter == "bb_moderate":
        return ["close < bb_lower(20,2)" if direction == "long" else "close > bb_upper(20,2)"]
    if entry_filter == "rsi_bb_extreme":
        return [
            "rsi(14) < 35" if direction == "long" else "rsi(14) > 65",
            "close < bb_lower(20,2)" if direction == "long" else "close > bb_upper(20,2)",
        ]
    if entry_filter == "rsi_bb_moderate":
        return [
            "rsi(14) < 40" if direction == "long" else "rsi(14) > 60",
            "close < bb_lower(20,1.5)" if direction == "long" else "close > bb_upper(20,1.5)",
        ]
    return []


def planned_margin(first_order: float, multiplier: float, legs: int, leverage: int) -> float:
    total = 0.0
    notional = first_order
    for _ in range(legs):
        total += max(5.0, notional) / max(1, leverage)
        notional *= multiplier
    return total


def margin_per_first(multiplier: float, legs: int, leverage: int) -> float:
    total = 0.0
    notional = 1.0
    for _ in range(legs):
        total += notional / max(1, leverage)
        notional *= multiplier
    return total


@dataclass(frozen=True)
class RowCand:
    idx: int
    row: dict[str, Any]
    symbol: str
    mode: str
    ann: float
    dd: float
    ret: float
    cap: float
    margin: float


def row_key(row: dict[str, Any]) -> tuple[Any, ...]:
    fields = (
        "symbol",
        "direction_mode",
        "entry_filter",
        "new_cycle_drawdown_pause_pct",
        "new_cycle_atr_pause_pct",
        "safety_skip_adx_threshold",
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
    )
    return tuple(row.get(k) for k in fields)


def rows_from_report(report: dict[str, Any]) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    for bucket in (report.get("best_by_budget") or {}).values():
        out.extend(bucket or [])
    for frontiers in (report.get("frontier_by_budget") or {}).values():
        for rows in (frontiers or {}).values():
            out.extend(rows or [])
    out.extend(report.get("pass_candidates") or [])
    return out


def load_rows(out_dir: Path) -> list[RowCand]:
    best_by_key: dict[tuple[Any, ...], dict[str, Any]] = {}
    for path in sorted(out_dir.glob("*.json")):
        if path.name == "summary.json":
            continue
        try:
            report = json.loads(path.read_text())
        except Exception:
            continue
        for row in rows_from_report(report):
            if row.get("principal_breached"):
                continue
            if f(row.get("max_capital_used_quote"), 1e9) > f(row.get("budget"), 5000.0) * 1.01:
                continue
            key = row_key(row)
            old = best_by_key.get(key)
            if old is None:
                best_by_key[key] = row
                continue
            if f(row.get("annualized_return_pct"), -999) - f(row.get("max_drawdown_pct"), 999) > f(old.get("annualized_return_pct"), -999) - f(old.get("max_drawdown_pct"), 999):
                best_by_key[key] = row
    out = []
    for idx, row in enumerate(best_by_key.values()):
        out.append(
            RowCand(
                idx=idx,
                row=row,
                symbol=str(row.get("symbol")),
                mode=str(row.get("direction_mode")),
                ann=f(row.get("annualized_return_pct"), -999),
                dd=f(row.get("max_drawdown_pct"), 999),
                ret=f(row.get("total_return_pct"), 0),
                cap=f(row.get("max_capital_used_quote"), 0),
                margin=f(row.get("planned_margin_quote"), 0),
            )
        )
    return out


def build_strategy(row: dict[str, Any], direction: str, first_order: float, weight_pct: float, tag: str) -> dict[str, Any]:
    triggers: list[dict[str, Any]] = [{"cooldown": {"seconds": int(f(row.get("cooldown_seconds"), 21600))}}]
    adx = row.get("adx_min")
    if adx is not None:
        triggers.append({"indicator_expression": {"expression": f"adx(14) > {int(f(adx))}"}})
    for expr in entry_filter_expressions(str(row.get("entry_filter") or "none"), direction):
        triggers.append({"indicator_expression": {"expression": expr}})
    rb = row.get("regime_break_ema_period")
    stop_loss = (
        {"regime_break_stop": {"ema_period": int(f(rb)), "drawdown_pct_bps": int(f(row.get("stop_loss_bps"), 2000))}}
        if rb is not None
        else {"strategy_drawdown_pct": {"pct_bps": int(f(row.get("stop_loss_bps"), 2000))}}
    )
    risk: dict[str, Any] = {}
    age = row.get("max_cycle_age_hours")
    if age is not None:
        risk["max_cycle_age_hours"] = f(age)
    symbol = str(row.get("symbol"))
    return {
        "strategy_id": f"p4row-{tag}-{symbol}-{direction}"[:180],
        "symbol": symbol,
        "market": "usd_m_futures",
        "direction": direction,
        "direction_mode": str(row.get("direction_mode") or "long_and_short"),
        "margin_mode": "isolated",
        "leverage": int(f(row.get("leverage"), 1)),
        "spacing": {"fixed_percent": {"step_bps": int(f(row.get("step_bps"), 100))}},
        "sizing": {
            "multiplier": {
                "first_order_quote": fmt(first_order),
                "multiplier": fmt(f(row.get("multiplier"), 1.0)),
                "max_legs": int(f(row.get("max_legs"), 1)),
            }
        },
        "take_profit": {"percent": {"bps": int(f(row.get("take_profit_bps"), 100))}},
        "stop_loss": stop_loss,
        "indicators": [{"atr": {"period": 21}}, {"adx": {"period": 14}}],
        "entry_triggers": triggers,
        "risk_limits": risk,
        "portfolio_weight_pct": fmt(weight_pct),
    }


def scaled_first_order(row: dict[str, Any], allocation: float, directions: int, util: float) -> float:
    leverage = int(f(row.get("leverage"), 1))
    multiplier = f(row.get("multiplier"), 1.0)
    legs = int(f(row.get("max_legs"), 1))
    denom = margin_per_first(multiplier, legs, leverage) * directions
    return max(5.0, allocation * util / max(denom, 1e-9))


def build_config(combo: list[RowCand], weights: list[float], budget: float, util: float, idx: int) -> dict[str, Any]:
    strategies: list[dict[str, Any]] = []
    for c, weight in zip(combo, weights, strict=True):
        mode = c.mode
        dirs = ["long"] if mode == "long_only" else ["short"] if mode == "short_only" else ["long", "short"]
        allocation = budget * weight
        first = scaled_first_order(c.row, allocation, len(dirs), util)
        for seq, direction in enumerate(dirs):
            strategies.append(build_strategy(c.row, direction, first, weight * 100.0 / len(dirs), f"{idx:05d}-{c.idx}-{seq}"))
    return {
        "portfolio_config": {
            "direction_mode": "long_and_short",
            "risk_limits": {
                "max_global_budget_quote": fmt(budget),
                "new_cycle_drawdown_pause_pct": combo[0].row.get("new_cycle_drawdown_pause_pct", 6.0),
                "new_cycle_atr_pause_pct": combo[0].row.get("new_cycle_atr_pause_pct", 2.0),
                "safety_skip_adx_threshold": combo[0].row.get("safety_skip_adx_threshold", 45.0),
            },
            "strategies": strategies,
        },
        "_meta": {
            "candidate_ids": [c.idx for c in combo],
            "symbols": [c.symbol for c in combo],
            "modes": [c.mode for c in combo],
            "source_ann_dd": [[c.ann, c.dd] for c in combo],
            "weights": weights,
            "util": util,
        },
    }


def pool_for_profile(rows: list[RowCand], profile: str, limit: int) -> list[RowCand]:
    target = TARGETS[profile]
    selected: list[RowCand] = []
    seen: set[int] = set()

    def add(items: list[RowCand]) -> None:
        for c in items:
            if c.idx in seen:
                continue
            selected.append(c)
            seen.add(c.idx)

    feasible = [c for c in rows if c.ann > -20 and c.dd < 90]
    def quality(c: RowCand) -> float:
        if profile == "conservative":
            return (
                c.ann
                - 6.0 * max(0.0, c.dd - target["dd"])
                - 0.6 * max(0.0, target["ann"] - c.ann)
                + min(c.ann / max(c.dd, 1.0), 8.0)
            )
        if profile == "balanced":
            return (
                c.ann
                - 3.2 * max(0.0, c.dd - target["dd"])
                - 0.35 * max(0.0, target["ann"] - c.ann)
                + min(c.ann / max(c.dd, 1.0), 8.0)
            )
        return (
            c.ann
            - 2.0 * max(0.0, c.dd - target["dd"])
            - 0.25 * max(0.0, target["ann"] - c.ann)
            + min(c.ann / max(c.dd, 1.0), 8.0)
        )

    if profile == "conservative":
        add(sorted(feasible, key=quality, reverse=True)[:80])
        add(sorted(feasible, key=lambda c: (c.dd, -c.ann))[:40])
        add(sorted(feasible, key=lambda c: (-c.ann, c.dd))[:40])
        add(sorted(feasible, key=lambda c: (-(c.ann / max(c.dd, 1.0)), c.dd))[:40])
        add(sorted([c for c in feasible if c.dd <= 25], key=lambda c: -c.ann)[:40])
    elif profile == "balanced":
        add(sorted(feasible, key=quality, reverse=True)[:90])
        add(sorted(feasible, key=lambda c: (c.dd, -c.ann))[:50])
        add(sorted(feasible, key=lambda c: (-c.ann, c.dd))[:50])
        add(sorted([c for c in feasible if c.dd <= 35], key=lambda c: -c.ann)[:50])
    else:
        add(sorted(feasible, key=quality, reverse=True)[:100])
        add(sorted(feasible, key=lambda c: -c.ann)[:50])
        add(sorted(feasible, key=lambda c: (c.dd, -c.ann))[:50])
        add(sorted([c for c in feasible if c.dd <= 45], key=lambda c: -c.ann)[:50])

    selected.sort(key=quality, reverse=True)
    # Keep a few ultra-low-DD rows, but do not let near-zero-turnover rows fill
    # the deterministic prefix used by the combination sampler.
    tail = sorted([c for c in feasible if c.idx not in seen], key=lambda c: (c.dd, -c.ann))[: max(10, limit // 5)]
    return (selected + tail)[:limit]


def sample_configs(pool: list[RowCand], profile: str, budget: float, count: int, rng: random.Random) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    top = pool[: min(len(pool), 24)]
    for k in range(2, min(6, len(top)) + 1):
        for combo in itertools.combinations(top, k):
            if len({c.symbol for c in combo}) < min(k, 3):
                continue
            weights = [1.0 / k] * k
            out.append(build_config(list(combo), weights, budget, rng.choice([0.70, 0.82, 0.94]), len(out)))
            if len(out) >= count // 3:
                break
        if len(out) >= count // 3:
            break
    attempts = 0
    while len(out) < count and attempts < count * 120:
        attempts += 1
        k = rng.choice([2, 3, 4, 5] if profile == "conservative" else [3, 4, 5, 6])
        if len(pool) < k:
            break
        combo = rng.sample(pool[: rng.choice([30, 45, min(len(pool), 70)])], k)
        if len({c.symbol for c in combo}) < min(k, 3):
            continue
        raw = [rng.uniform(0.4, 1.8) for _ in combo]
        s = sum(raw)
        weights = [x / s for x in raw]
        if max(weights) > (0.55 if profile == "conservative" else 0.65):
            continue
        out.append(build_config(combo, weights, budget, rng.choice([0.55, 0.70, 0.82, 0.94]), len(out)))
    return out[:count]


def run_replay(args: argparse.Namespace, cfg: dict[str, Any], start: int, end: int, timeout: int) -> dict[str, Any]:
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as fh:
        json.dump(cfg, fh)
        path = fh.name
    try:
        proc = subprocess.run(
            [
                str(args.replay_bin),
                "--config",
                path,
                "--budget",
                fmt(args.budget),
                "--profile",
                args.profile,
                "--start-ms",
                str(start),
                "--end-ms",
                str(end),
                "--market-data",
                str(args.market_data),
                "--funding-data",
                str(args.funding_data),
                "--exchange-min-notional",
                "5",
                "--equity-curve-points",
                "16",
            ],
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        if proc.returncode != 0:
            return {"ok": False, "error": proc.stderr[-1800:], "returncode": proc.returncode}
        data = json.loads(proc.stdout)
        on = data.get("on_budget", {}) or {}
        return {
            "ok": True,
            "ann": on.get("annualized_return_pct"),
            "dd": on.get("max_drawdown_pct"),
            "ret": on.get("total_return_pct"),
            "principal_breached": on.get("principal_breached"),
            "cap": data.get("max_capital_used_quote"),
            "blocked": data.get("budget_blocked_legs"),
            "trades": data.get("trade_count"),
            "stops": data.get("stop_count"),
            "symbols": data.get("symbols"),
            "strategy_count": data.get("strategy_count"),
            "equity_curve_sample": data.get("equity_curve_sample"),
        }
    except subprocess.TimeoutExpired:
        return {"ok": False, "error": "timeout"}
    finally:
        try:
            os.unlink(path)
        except OSError:
            pass


def full_job(job: tuple[int, dict[str, Any], argparse.Namespace]) -> dict[str, Any]:
    idx, cfg, args = job
    return {"idx": idx, "config": cfg, "meta": cfg.get("_meta"), "full": run_replay(args, cfg, FULL[0], FULL[1], args.timeout)}


def seg_validate(args: argparse.Namespace, item: dict[str, Any]) -> dict[str, Any]:
    segments = {}
    for name, (start, end) in SEGMENTS.items():
        segments[name] = run_replay(args, item["config"], start, end, args.segment_timeout)
    returns = [f(v.get("ret"), 0) for v in segments.values() if v.get("ok")]
    dds = [f(v.get("dd"), 999) for v in segments.values() if v.get("ok")]
    h1 = f(segments.get("h1_2023", {}).get("ret"), 0)
    full_ret = f(item["full"].get("ret"), 0)
    combined = 1.0
    ok_2426 = True
    for key in ("2024", "2025", "2026_ytd"):
        r = segments.get(key) or {}
        if not r.get("ok"):
            ok_2426 = False
            break
        combined *= 1.0 + f(r.get("ret"), 0) / 100.0
    return {
        **item,
        "segments": segments,
        "segment_summary": {
            "positive_segments": sum(1 for r in returns if r >= 0),
            "max_segment_dd": max(dds) if dds else None,
            "h1_contribution_ratio": h1 / full_ret if abs(full_ret) > 1e-9 else 0,
            "combined_2024_2026_return_pct": (combined - 1.0) * 100.0 if ok_2426 else None,
            "segment_returns": returns,
        },
    }


def passes(profile: str, item: dict[str, Any]) -> bool:
    t = TARGETS[profile]
    full = item.get("full") or {}
    if not full.get("ok") or full.get("principal_breached"):
        return False
    if f(full.get("ann"), -999) < t["ann"] or f(full.get("dd"), 999) > t["dd"]:
        return False
    if f(full.get("cap"), 0) > 5000 or int(full.get("blocked") or 0) > 0:
        return False
    s = item.get("segment_summary")
    if not s:
        return True
    if s["positive_segments"] < t["min_pos"]:
        return False
    if s["max_segment_dd"] is None or s["max_segment_dd"] > t["max_seg_dd"]:
        return False
    if s["h1_contribution_ratio"] > t["h1"]:
        return False
    if profile in ("balanced", "aggressive") and f(s.get("combined_2024_2026_return_pct"), -999) < 0:
        return False
    return True


def score(profile: str, item: dict[str, Any]) -> float:
    t = TARGETS[profile]
    full = item.get("full") or {}
    if not full.get("ok"):
        return -1e12
    return f(full.get("ann"), -999) - 4.0 * max(0.0, f(full.get("dd"), 999) - t["dd"]) - 0.8 * max(0.0, t["ann"] - f(full.get("ann"), -999)) - 10.0 * int(full.get("blocked") or 0)


def save(path: Path, report: dict[str, Any]) -> None:
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(report, ensure_ascii=False, indent=2))
    tmp.replace(path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", choices=sorted(TARGETS), required=True)
    parser.add_argument("--row-dir", type=Path, required=True)
    parser.add_argument("--budget", type=float, default=5000.0)
    parser.add_argument("--count", type=int, default=240)
    parser.add_argument("--pool", type=int, default=120)
    parser.add_argument("--seed", type=int, default=20260630)
    parser.add_argument("--jobs", type=int, default=20)
    parser.add_argument("--top-segment", type=int, default=20)
    parser.add_argument("--timeout", type=int, default=900)
    parser.add_argument("--segment-timeout", type=int, default=600)
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--replay-bin", type=Path, required=True)
    parser.add_argument("--market-data", type=Path, required=True)
    parser.add_argument("--funding-data", type=Path, required=True)
    args = parser.parse_args()

    rows = load_rows(args.row_dir)
    pool = pool_for_profile(rows, args.profile, args.pool)
    rng = random.Random(args.seed)
    configs = sample_configs(pool, args.profile, args.budget, args.count, rng)
    args.out_dir.mkdir(parents=True, exist_ok=True)
    report_path = args.out_dir / f"p4row_combo_{args.profile}_b{int(args.budget)}_seed{args.seed}.json"
    log_path = args.out_dir / f"p4row_combo_{args.profile}_b{int(args.budget)}_seed{args.seed}.log"
    report: dict[str, Any] = {
        "profile": args.profile,
        "budget": args.budget,
        "row_dir": str(args.row_dir),
        "row_count": len(rows),
        "pool_count": len(pool),
        "config_count": len(configs),
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "full_results": [],
        "segment_validations": [],
        "passes": [],
    }
    jobs = [(idx, cfg, args) for idx, cfg in enumerate(configs)]
    done = 0
    with futures.ProcessPoolExecutor(max_workers=args.jobs) as pool_exec:
        fmap = {pool_exec.submit(full_job, job): job[0] for job in jobs}
        with log_path.open("a") as log:
            for fut in futures.as_completed(fmap):
                done += 1
                try:
                    item = fut.result()
                except Exception as exc:
                    item = {"idx": fmap[fut], "config": None, "meta": None, "full": {"ok": False, "error": repr(exc)}}
                report["full_results"].append(item)
                full = item["full"]
                if full.get("ok"):
                    log.write(
                        f"DONE {done}/{len(jobs)} idx={item['idx']} ann={f(full.get('ann'), -999):.2f} "
                        f"dd={f(full.get('dd'), 999):.2f} ret={f(full.get('ret'), 0):.2f} "
                        f"cap={f(full.get('cap'), 0):.1f} blocked={full.get('blocked')} trades={full.get('trades')}\n"
                    )
                else:
                    log.write(f"DONE {done}/{len(jobs)} idx={item['idx']} ERROR {full.get('error')}\n")
                log.flush()
                if done % max(1, min(10, args.jobs)) == 0:
                    save(report_path, report)
    full_ok = [r for r in report["full_results"] if (r.get("full") or {}).get("ok")]
    full_ok.sort(key=lambda r: score(args.profile, r), reverse=True)
    for item in full_ok[: args.top_segment]:
        validated = seg_validate(args, item)
        report["segment_validations"].append(validated)
        save(report_path, report)
    report["passes"] = [r for r in report["segment_validations"] if passes(args.profile, r)]
    report["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    save(report_path, report)
    print(report_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
