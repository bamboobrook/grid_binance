#!/usr/bin/env python3
"""Original-margin pack search for small-capital martingale portfolios.

Build portfolios from single-symbol candidates whose original planned margin can
fit inside a 5000 USDT margin budget. Unlike curve-frontier conversion, this
keeps the original first order / multiplier / legs whenever possible, then sets
portfolio weights from actual planned margin so replay caps do not rewrite the
path.
"""
from __future__ import annotations

import argparse
import concurrent.futures as futures
import csv
import gzip
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
    "conservative": {"ann": 50.0, "dd": 10.0, "min_pos": 4, "max_seg_dd": 12.0, "h1": 0.35},
    "balanced": {"ann": 90.0, "dd": 20.0, "min_pos": 3, "max_seg_dd": 24.0, "h1": 0.45},
    "aggressive": {"ann": 110.0, "dd": 30.0, "min_pos": 3, "max_seg_dd": 36.0, "h1": 0.55},
}


@dataclass
class Candidate:
    cid: str
    symbol: str
    mode: str
    source_profile: str
    ann: float
    dd: float
    trades: int
    planned_margin: float
    max_capital_used: float
    config: dict[str, Any]


def fmt(value: Any) -> str:
    if isinstance(value, str):
        return value
    text = f"{float(value):.10f}".rstrip("0").rstrip(".")
    return text if text else "0"


def f(value: Any, default: float = 0.0) -> float:
    try:
        if value is None:
            return default
        out = float(value)
        return out if math.isfinite(out) else default
    except Exception:
        return default


def root(config: dict[str, Any]) -> dict[str, Any]:
    return config.get("portfolio_config", config)


def sizing(strategy: dict[str, Any]) -> dict[str, Any] | None:
    model = strategy.get("sizing") or {}
    mult = model.get("multiplier")
    return mult if isinstance(mult, dict) else None


def strategy_margin(strategy: dict[str, Any]) -> float:
    sz = sizing(strategy)
    if not sz:
        return 0.0
    first = f(sz.get("first_order_quote"), 0.0)
    mult = f(sz.get("multiplier"), 1.0)
    legs = max(1, int(f(sz.get("max_legs"), 1)))
    market = str(strategy.get("market") or "usd_m_futures")
    lev = 1.0 if market == "spot" else max(1.0, f(strategy.get("leverage"), 1.0))
    total = 0.0
    notional = first
    for _ in range(legs):
        total += max(5.0, notional) / lev
        notional *= mult
    return total


def live_ok(strategy: dict[str, Any], allow_atr_spacing: bool) -> bool:
    if sizing(strategy) is None:
        return False
    tp = strategy.get("take_profit") or {}
    if not (isinstance(tp, dict) and "percent" in tp):
        return False
    sl = strategy.get("stop_loss")
    if sl is not None:
        if not isinstance(sl, dict):
            return False
        if not ({"strategy_drawdown_pct", "regime_break_stop"} & set(sl)):
            return False
    spacing = strategy.get("spacing") or {}
    if "fixed_percent" in spacing:
        return True
    if allow_atr_spacing and "atr" in spacing:
        return True
    return False


def sanitize_candidate_config(config: dict[str, Any], allow_atr_spacing: bool) -> dict[str, Any] | None:
    cfg = json.loads(json.dumps(config))
    portfolio = root(cfg)
    strategies = [s for s in portfolio.get("strategies") or [] if live_ok(s, allow_atr_spacing)]
    if not strategies:
        return None
    for strategy in strategies:
        risk = strategy.setdefault("risk_limits", {})
        for key in (
            "max_global_budget_quote",
            "max_symbol_budget_quote",
            "max_direction_budget_quote",
            "max_strategy_budget_quote",
        ):
            risk.pop(key, None)
        strategy["portfolio_weight_pct"] = None
    portfolio["strategies"] = strategies
    portfolio["direction_mode"] = "long_and_short"
    return {"portfolio_config": portfolio}


def candidate_margin(candidate: Candidate) -> float:
    return sum(strategy_margin(s) for s in root(candidate.config).get("strategies") or [])


def load_candidates(path: Path, allow_atr_spacing: bool, max_margin: float) -> list[Candidate]:
    csv.field_size_limit(1024 * 1024 * 1024)
    out: list[Candidate] = []
    with gzip.open(path, "rt", newline="") as fh:
        reader = csv.DictReader(fh)
        for row in reader:
            try:
                config = sanitize_candidate_config(json.loads(row["config"]), allow_atr_spacing)
                if config is None:
                    continue
                c = Candidate(
                    cid=row["candidate_id"],
                    symbol=row["symbol"],
                    mode=row["direction_mode"],
                    source_profile=row.get("source_risk_profile") or "",
                    ann=f(row.get("annualized_return_pct"), -999.0),
                    dd=f(row.get("max_drawdown_pct"), 999.0),
                    trades=int(f(row.get("trade_count"), 0.0)),
                    planned_margin=f(row.get("planned_margin_quote"), 0.0),
                    max_capital_used=f(row.get("max_capital_used_quote"), 0.0),
                    config=config,
                )
                margin = candidate_margin(c)
                if margin <= 0 or margin > max_margin:
                    continue
                c.planned_margin = margin
                out.append(c)
            except Exception:
                continue
    return dedupe_candidates(out)


def candidate_key(c: Candidate) -> str:
    return json.dumps(root(c.config).get("strategies") or [], sort_keys=True)


def dedupe_candidates(rows: list[Candidate]) -> list[Candidate]:
    by_key: dict[str, Candidate] = {}
    for c in rows:
        key = candidate_key(c)
        prev = by_key.get(key)
        if prev is None or (c.ann - 1.5 * c.dd) > (prev.ann - 1.5 * prev.dd):
            by_key[key] = c
    return list(by_key.values())


def pool_for_profile(candidates: list[Candidate], profile: str, limit: int, max_per_symbol: int) -> list[Candidate]:
    target = TARGETS[profile]
    scored: list[tuple[float, Candidate]] = []
    for c in candidates:
        if c.ann <= -20 or c.dd > 85:
            continue
        if profile == "conservative" and c.dd > 45:
            continue
        if profile == "balanced" and c.dd > 60:
            continue
        score = (
            c.ann
            - (2.8 if profile == "conservative" else 1.7) * c.dd
            + min(c.planned_margin / 5000.0, 1.0) * 8.0
            - 0.0003 * max(0, c.trades - 15000)
        )
        # Make sure the low-DD side is represented even when ann is modest.
        if c.dd <= target["dd"] + 8:
            score += 20
        scored.append((score, c))
    scored.sort(key=lambda item: item[0], reverse=True)
    counts: dict[str, int] = {}
    out: list[Candidate] = []
    for _score, c in scored:
        if counts.get(c.symbol, 0) >= max_per_symbol:
            continue
        counts[c.symbol] = counts.get(c.symbol, 0) + 1
        out.append(c)
        if len(out) >= limit:
            break
    return out


def build_combo_config(
    combo: list[Candidate],
    budget: float,
    profile: str,
    idx: int,
    cash_reserve_pct: float,
    risk_variant: dict[str, Any],
) -> dict[str, Any]:
    strategies: list[dict[str, Any]] = []
    raw_margins: list[float] = []
    for c in combo:
        strats = json.loads(json.dumps(root(c.config).get("strategies") or []))
        margins = [strategy_margin(s) for s in strats]
        total = sum(margins)
        if total <= 0:
            continue
        for s, m in zip(strats, margins, strict=True):
            raw_margins.append(m)
            strategies.append(s)
    total_margin = sum(raw_margins)
    if total_margin <= 0:
        raise ValueError("empty combo")
    # Keep enough per-strategy cap to avoid path truncation; normalize below 100%
    # only if the combo would otherwise exceed the intended utilization.
    target_pct = min(99.5, 100.0 - cash_reserve_pct)
    for i, (strategy, margin) in enumerate(zip(strategies, raw_margins, strict=True)):
        weight_pct = margin / total_margin * target_pct
        strategy["portfolio_weight_pct"] = fmt(weight_pct)
        strategy["strategy_id"] = f"origpack-v7-{profile}-{idx:04d}-{i:02d}-{strategy.get('symbol')}-{strategy.get('direction')}"[:180]
        if risk_variant.get("max_cycle_age_hours") is not None:
            strategy.setdefault("risk_limits", {})["max_cycle_age_hours"] = risk_variant["max_cycle_age_hours"]

    risk = {
        "max_global_budget_quote": fmt(budget),
        "new_cycle_drawdown_pause_pct": risk_variant["dd_pause"],
        "new_cycle_atr_pause_pct": risk_variant["atr_pause"],
        "safety_skip_adx_threshold": risk_variant["adx_skip"],
    }
    return {
        "portfolio_config": {
            "direction_mode": "long_and_short",
            "risk_limits": risk,
            "strategies": strategies,
        },
        "_meta": {
            "ids": [c.cid for c in combo],
            "symbols": [c.symbol for c in combo],
            "candidate_ann_dd": [[c.ann, c.dd] for c in combo],
            "planned_margin_sum": total_margin,
            "target_weight_pct": target_pct,
            "risk_variant": risk_variant,
        },
    }


def sample_combos(pool: list[Candidate], profile: str, budget: float, count: int, rng: random.Random) -> list[list[Candidate]]:
    combos: list[list[Candidate]] = []
    # Deterministic frontiers first.
    top_ann = sorted(pool, key=lambda c: (-c.ann, c.dd))[:18]
    low_dd = sorted(pool, key=lambda c: (c.dd, -c.ann))[:18]
    mixed = []
    for c in itertools.chain(top_ann, low_dd, pool[:24]):
        if c not in mixed:
            mixed.append(c)
    for k in range(2, min(6, len(mixed)) + 1):
        for combo in itertools.combinations(mixed[: min(len(mixed), 18)], k):
            if len({c.symbol for c in combo}) < min(3, k):
                continue
            margin = sum(c.planned_margin for c in combo)
            if budget * 0.35 <= margin <= budget * 0.995:
                combos.append(list(combo))
                if len(combos) >= count // 3:
                    break
        if len(combos) >= count // 3:
            break

    attempts = 0
    while len(combos) < count and attempts < count * 80:
        attempts += 1
        k = rng.choice([3, 4, 5, 6] if profile != "conservative" else [3, 4, 5])
        weighted = sorted(pool, key=lambda c: max(0.1, c.ann - 1.2 * c.dd), reverse=True)
        pick_pool = weighted[: rng.choice([24, 36, 48, min(len(weighted), 60)])]
        if len(pick_pool) < k:
            continue
        combo = rng.sample(pick_pool, k)
        if len({c.symbol for c in combo}) < min(3, k):
            continue
        margin = sum(c.planned_margin for c in combo)
        if margin > budget * 0.995 or margin < budget * rng.choice([0.35, 0.45, 0.55, 0.65]):
            continue
        combos.append(combo)
    # Deduplicate by candidate ids.
    out: list[list[Candidate]] = []
    seen: set[tuple[str, ...]] = set()
    for combo in combos:
        key = tuple(sorted(c.cid for c in combo))
        if key in seen:
            continue
        seen.add(key)
        out.append(combo)
    return out[:count]


def risk_variants(profile: str, rng: random.Random) -> list[dict[str, Any]]:
    if profile == "conservative":
        base = [
            (2.0, 1.0, 30.0, 24.0),
            (3.0, 1.2, 35.0, 48.0),
            (4.0, 1.5, 45.0, 72.0),
            (6.0, 2.0, 45.0, None),
        ]
    elif profile == "balanced":
        base = [
            (3.0, 1.3, 35.0, 48.0),
            (4.0, 1.6, 45.0, 72.0),
            (6.0, 2.0, 55.0, 96.0),
            (50.0, 50.0, 100.0, None),
        ]
    else:
        base = [
            (4.0, 1.6, 45.0, 72.0),
            (6.0, 2.0, 55.0, 96.0),
            (8.0, 2.5, 60.0, 168.0),
            (50.0, 50.0, 100.0, None),
        ]
    out = [
        {
            "dd_pause": dd,
            "atr_pause": atr,
            "adx_skip": adx,
            "max_cycle_age_hours": age,
        }
        for dd, atr, adx, age in base
    ]
    while len(out) < 10:
        dd, atr, adx, age = rng.choice(base)
        out.append(
            {
                "dd_pause": rng.choice([dd, max(1.5, dd * 0.8), dd * 1.2]),
                "atr_pause": rng.choice([atr, max(0.8, atr * 0.8), atr * 1.2]),
                "adx_skip": rng.choice([adx, max(20.0, adx - 10), adx + 10]),
                "max_cycle_age_hours": rng.choice([age, 24.0, 48.0, 96.0, None]),
            }
        )
    return out


def run_replay(args: argparse.Namespace, config: dict[str, Any], start: int, end: int, timeout: int) -> dict[str, Any]:
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as fh:
        json.dump(config, fh)
        cfg_path = fh.name
    try:
        proc = subprocess.run(
            [
                str(args.replay_bin),
                "--config",
                cfg_path,
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
            return {"ok": False, "error": proc.stderr[-2000:], "returncode": proc.returncode}
        data = json.loads(proc.stdout)
        on = data.get("on_budget", {}) or {}
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
            "symbols": data.get("symbols"),
            "strategy_count": data.get("strategy_count"),
            "equity_curve_sample": data.get("equity_curve_sample"),
        }
    except subprocess.TimeoutExpired:
        return {"ok": False, "error": "timeout"}
    finally:
        try:
            os.unlink(cfg_path)
        except OSError:
            pass


def full_job(job: tuple[int, str, dict[str, Any], argparse.Namespace]) -> dict[str, Any]:
    idx, name, cfg, args = job
    return {"idx": idx, "name": name, "config": cfg, "meta": cfg.get("_meta"), "full": run_replay(args, cfg, FULL[0], FULL[1], args.timeout)}


def segment_validate(args: argparse.Namespace, item: dict[str, Any]) -> dict[str, Any]:
    segments = {}
    for name, (start, end) in SEGMENTS.items():
        segments[name] = run_replay(args, item["config"], start, end, args.segment_timeout)
    returns = [f(r.get("ret"), 0.0) for r in segments.values() if r.get("ok")]
    dds = [f(r.get("dd"), 999.0) for r in segments.values() if r.get("ok")]
    h1 = f(segments.get("h1_2023", {}).get("ret"), 0.0)
    full_ret = f(item["full"].get("ret"), 0.0)
    combined = 1.0
    ok_2426 = True
    for key in ("2024", "2025", "2026_ytd"):
        r = segments.get(key) or {}
        if not r.get("ok"):
            ok_2426 = False
            break
        combined *= 1.0 + f(r.get("ret"), 0.0) / 100.0
    return {
        **item,
        "segments": segments,
        "segment_summary": {
            "positive_segments": sum(1 for value in returns if value >= 0),
            "max_segment_dd": max(dds) if dds else None,
            "h1_contribution_ratio": h1 / full_ret if abs(full_ret) > 1e-9 else 0.0,
            "combined_2024_2026_return_pct": (combined - 1.0) * 100.0 if ok_2426 else None,
            "segment_returns": returns,
        },
    }


def passes(args: argparse.Namespace, item: dict[str, Any]) -> bool:
    t = TARGETS[args.profile]
    full = item.get("full") or {}
    if not full.get("ok") or full.get("principal_breached"):
        return False
    if f(full.get("ann"), -999) < t["ann"] or f(full.get("dd"), 999) > t["dd"]:
        return False
    if int(full.get("blocked") or 0) != 0 or f(full.get("cap"), 0) > args.budget:
        return False
    summary = item.get("segment_summary")
    if not summary:
        return True
    if summary["positive_segments"] < t["min_pos"]:
        return False
    if summary["max_segment_dd"] is None or summary["max_segment_dd"] > t["max_seg_dd"]:
        return False
    if summary["h1_contribution_ratio"] > t["h1"]:
        return False
    if args.profile in ("balanced", "aggressive") and f(summary.get("combined_2024_2026_return_pct"), -999) < 0:
        return False
    return True


def score(args: argparse.Namespace, item: dict[str, Any]) -> float:
    t = TARGETS[args.profile]
    full = item.get("full") or {}
    if not full.get("ok"):
        return -1e9
    ann = f(full.get("ann"), -999)
    dd = f(full.get("dd"), 999)
    return ann - 5.0 * max(0, dd - t["dd"]) - 0.7 * max(0, t["ann"] - ann) - 5.0 * int(full.get("blocked") or 0)


def save(path: Path, report: dict[str, Any]) -> None:
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(report, ensure_ascii=False, indent=2))
    tmp.replace(path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", choices=sorted(TARGETS), required=True)
    parser.add_argument("--csv", type=Path, default=Path("/tmp/codex_small_search/full_period_candidates.csv.gz"))
    parser.add_argument("--budget", type=float, default=5000.0)
    parser.add_argument("--count", type=int, default=240)
    parser.add_argument("--pool", type=int, default=80)
    parser.add_argument("--max-per-symbol", type=int, default=8)
    parser.add_argument("--seed", type=int, default=20260629)
    parser.add_argument("--jobs", type=int, default=20)
    parser.add_argument("--top-segment", type=int, default=24)
    parser.add_argument("--timeout", type=int, default=900)
    parser.add_argument("--segment-timeout", type=int, default=600)
    parser.add_argument("--allow-atr-spacing", action="store_true")
    parser.add_argument("--out-dir", required=True)
    parser.add_argument("--replay-bin", required=True)
    parser.add_argument("--market-data", required=True)
    parser.add_argument("--funding-data", required=True)
    args = parser.parse_args()
    args.replay_bin = Path(args.replay_bin)
    args.market_data = Path(args.market_data)
    args.funding_data = Path(args.funding_data)
    out_dir = Path(args.out_dir)
    cfg_dir = out_dir / "configs"
    cfg_dir.mkdir(parents=True, exist_ok=True)
    rng = random.Random(args.seed)

    candidates = load_candidates(args.csv, args.allow_atr_spacing, args.budget * 0.99)
    pool = pool_for_profile(candidates, args.profile, args.pool, args.max_per_symbol)
    combos = sample_combos(pool, args.profile, args.budget, args.count, rng)
    variants = risk_variants(args.profile, rng)

    jobs = []
    for idx, combo in enumerate(combos):
        risk = rng.choice(variants)
        reserve = rng.choice([0.5, 1.0, 2.0, 4.0])
        name = f"origpack-v7-{args.profile}-{idx:04d}"
        cfg = build_combo_config(combo, args.budget, args.profile, idx, reserve, risk)
        cfg_path = cfg_dir / f"{name}.json"
        cfg_path.write_text(json.dumps(cfg, ensure_ascii=False, indent=2))
        jobs.append((idx, name, cfg, args))

    report_path = out_dir / f"origpack_v7_{args.profile}_b{int(args.budget)}_seed{args.seed}.json"
    log_path = out_dir / f"origpack_v7_{args.profile}_b{int(args.budget)}_seed{args.seed}.log"
    report: dict[str, Any] = {
        "profile": args.profile,
        "budget": args.budget,
        "seed": args.seed,
        "loaded_candidates": len(candidates),
        "pool_size": len(pool),
        "combo_count": len(jobs),
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "pool_preview": [
            {
                "id": c.cid,
                "symbol": c.symbol,
                "ann": c.ann,
                "dd": c.dd,
                "planned_margin": c.planned_margin,
                "trades": c.trades,
            }
            for c in pool[:30]
        ],
        "full_results": [],
        "segment_validations": [],
        "passes": [],
    }
    save(report_path, report)

    done = 0
    with futures.ProcessPoolExecutor(max_workers=args.jobs) as pool_exec:
        futs = {pool_exec.submit(full_job, job): job[0] for job in jobs}
        with log_path.open("a") as log:
            for fut in futures.as_completed(futs):
                done += 1
                try:
                    item = fut.result()
                except Exception as exc:
                    item = {"idx": futs[fut], "name": "error", "config": None, "meta": {}, "full": {"ok": False, "error": repr(exc)}}
                report["full_results"].append(item)
                full = item["full"]
                if full.get("ok"):
                    line = (
                        f"DONE {done}/{len(jobs)} {item['name']} ann={f(full.get('ann'), -999):.2f} "
                        f"dd={f(full.get('dd'), 999):.2f} ret={f(full.get('ret'), 0):.2f} "
                        f"cap={f(full.get('cap'), 0):.1f} blocked={full.get('blocked')} trades={full.get('trades')}\n"
                    )
                else:
                    line = f"DONE {done}/{len(jobs)} {item['name']} ERROR {full.get('error')}\n"
                log.write(line)
                log.flush()
                if done % max(1, min(args.jobs, 10)) == 0:
                    save(report_path, report)

    ok = [r for r in report["full_results"] if (r.get("full") or {}).get("ok")]
    ok.sort(key=lambda item: score(args, item), reverse=True)
    full_pass = [r for r in ok if passes(args, r)]
    segment_items = []
    seen = set()
    for item in full_pass + ok[: args.top_segment]:
        if item["name"] in seen:
            continue
        seen.add(item["name"])
        segment_items.append(item)
    segment_items = segment_items[: max(args.top_segment, len(full_pass))]

    with futures.ProcessPoolExecutor(max_workers=max(1, min(args.jobs, len(segment_items)))) as pool_exec:
        futs = {pool_exec.submit(segment_validate, args, item): i for i, item in enumerate(segment_items)}
        with log_path.open("a") as log:
            for fut in futures.as_completed(futs):
                try:
                    item = fut.result()
                except Exception as exc:
                    base = segment_items[futs[fut]]
                    item = {**base, "segments": {}, "segment_summary": {"error": repr(exc), "positive_segments": 0, "max_segment_dd": None, "h1_contribution_ratio": 1.0, "combined_2024_2026_return_pct": None, "segment_returns": []}}
                report["segment_validations"].append(item)
                seg = item["segment_summary"]
                log.write(
                    f"SEG {len(report['segment_validations'])}/{len(segment_items)} {item['name']} "
                    f"pos={seg.get('positive_segments')} max_seg_dd={seg.get('max_segment_dd')} "
                    f"h1={seg.get('h1_contribution_ratio')} ret2426={seg.get('combined_2024_2026_return_pct')}\n"
                )
                log.flush()
                save(report_path, report)

    report["passes"] = [r for r in report["segment_validations"] if passes(args, r)]
    report["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    save(report_path, report)
    print(report_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
