#!/usr/bin/env python3
"""Targeted live-parity guard sweep for small-capital martingale portfolios.

This research-only runner starts from real portfolio configs that are already
near a profile frontier, mutates only live-reproducible controls, and validates
with `portfolio_budget_replay`.
"""
from __future__ import annotations

import argparse
import concurrent.futures as futures
import copy
import json
import os
import random
import subprocess
import tempfile
import time
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


def fmt(value: Any) -> str:
    if isinstance(value, str):
        return value
    text = f"{float(value):.10f}".rstrip("0").rstrip(".")
    return text if text else "0"


def as_float(value: Any, default: float = 0.0) -> float:
    try:
        if value is None:
            return default
        return float(value)
    except Exception:
        return default


def sizing(strategy: dict[str, Any]) -> dict[str, Any] | None:
    model = strategy.get("sizing") or {}
    mult = model.get("multiplier")
    return mult if isinstance(mult, dict) else None


def scale_first_order(strategy: dict[str, Any], scale: float) -> None:
    sz = sizing(strategy)
    if not sz:
        return
    current = as_float(sz.get("first_order_quote"), 0.0)
    if current <= 0:
        return
    sz["first_order_quote"] = fmt(max(5.0, current * scale))


def scale_take_profit(strategy: dict[str, Any], scale: float) -> None:
    tp = strategy.get("take_profit") or {}
    percent = tp.get("percent") if isinstance(tp, dict) else None
    if not isinstance(percent, dict):
        return
    bps = int(as_float(percent.get("bps"), 0))
    if bps > 0:
        percent["bps"] = max(10, int(round(bps * scale)))


def scale_spacing(strategy: dict[str, Any], scale: float) -> None:
    spacing = strategy.get("spacing") or {}
    fixed = spacing.get("fixed_percent") if isinstance(spacing, dict) else None
    if not isinstance(fixed, dict):
        return
    step = int(as_float(fixed.get("step_bps"), 0))
    if step > 0:
        fixed["step_bps"] = max(10, int(round(step * scale)))


def scale_stop(strategy: dict[str, Any], scale: float) -> None:
    sl = strategy.get("stop_loss")
    if not isinstance(sl, dict):
        return
    if "strategy_drawdown_pct" in sl and isinstance(sl["strategy_drawdown_pct"], dict):
        bps = int(as_float(sl["strategy_drawdown_pct"].get("pct_bps"), 0))
        if bps > 0:
            sl["strategy_drawdown_pct"]["pct_bps"] = max(100, int(round(bps * scale)))
    if "regime_break_stop" in sl and isinstance(sl["regime_break_stop"], dict):
        bps = int(as_float(sl["regime_break_stop"].get("drawdown_pct_bps"), 0))
        if bps > 0:
            sl["regime_break_stop"]["drawdown_pct_bps"] = max(100, int(round(bps * scale)))


def cap_legs(strategy: dict[str, Any], cap: int | None) -> None:
    if cap is None:
        return
    sz = sizing(strategy)
    if not sz:
        return
    current = int(as_float(sz.get("max_legs"), 1))
    sz["max_legs"] = max(1, min(current, cap))


def set_age(strategy: dict[str, Any], age: float | None, scope: str) -> None:
    direction = str(strategy.get("direction") or "")
    if scope == "long" and direction != "long":
        return
    if scope == "short" and direction != "short":
        return
    risk = strategy.setdefault("risk_limits", {})
    if age is None:
        risk.pop("max_cycle_age_hours", None)
    else:
        risk["max_cycle_age_hours"] = float(age)


def normalize_weights(config: dict[str, Any], total_pct: float = 99.5) -> None:
    strategies = config.get("portfolio_config", config).get("strategies") or []
    weights = []
    for strategy in strategies:
        weights.append(max(0.0, as_float(strategy.get("portfolio_weight_pct"), 0.0)))
    current = sum(weights)
    if current <= 0 or not strategies:
        each = total_pct / max(1, len(strategies))
        for strategy in strategies:
            strategy["portfolio_weight_pct"] = fmt(each)
        return
    for strategy, weight in zip(strategies, weights, strict=True):
        strategy["portfolio_weight_pct"] = fmt(weight / current * total_pct)


def live_exit_ok(strategy: dict[str, Any]) -> bool:
    tp = strategy.get("take_profit") or {}
    if not (isinstance(tp, dict) and "percent" in tp):
        return False
    sl = strategy.get("stop_loss")
    if sl is None:
        return True
    if not isinstance(sl, dict):
        return False
    return bool({"strategy_drawdown_pct", "regime_break_stop"} & set(sl))


def sanitize_config(config: dict[str, Any], budget: float) -> dict[str, Any] | None:
    cfg = copy.deepcopy(config)
    portfolio = cfg.get("portfolio_config", cfg)
    strategies = [s for s in portfolio.get("strategies", []) if live_exit_ok(s)]
    if not strategies:
        return None
    portfolio["strategies"] = strategies
    portfolio["direction_mode"] = "long_and_short"
    risk = portfolio.setdefault("risk_limits", {})
    risk["max_global_budget_quote"] = fmt(budget)
    normalize_weights({"portfolio_config": portfolio})
    return {"portfolio_config": portfolio}


def mutate_config(
    source_name: str,
    source_cfg: dict[str, Any],
    budget: float,
    profile: str,
    rng: random.Random,
    idx: int,
) -> tuple[str, dict[str, Any], dict[str, Any]]:
    cfg = sanitize_config(source_cfg, budget)
    if cfg is None:
        raise ValueError("source has no live-parity strategies")
    portfolio = cfg["portfolio_config"]
    risk = portfolio.setdefault("risk_limits", {})

    if profile == "conservative":
        dd_pause = rng.choice([1.5, 2.0, 2.5, 3.0, 4.0, 6.0, 50.0])
        atr_pause = rng.choice([0.8, 1.0, 1.2, 1.5, 2.0, 50.0])
        adx_skip = rng.choice([20.0, 25.0, 30.0, 35.0, 45.0, 100.0])
        stop_scale = rng.choice([0.45, 0.6, 0.75, 0.9, 1.0])
        tp_scale = rng.choice([0.6, 0.8, 1.0, 1.15])
        step_scale = rng.choice([1.0, 1.25, 1.5, 2.0])
        first_scale = rng.choice([0.7, 0.85, 1.0])
        leg_cap = rng.choice([3, 4, 5, None])
        age = rng.choice([12.0, 24.0, 48.0, 72.0, 96.0, None])
    elif profile == "balanced":
        dd_pause = rng.choice([2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 50.0])
        atr_pause = rng.choice([1.0, 1.3, 1.6, 2.0, 2.5, 50.0])
        adx_skip = rng.choice([25.0, 30.0, 35.0, 45.0, 55.0, 100.0])
        stop_scale = rng.choice([0.6, 0.75, 0.9, 1.0, 1.15])
        tp_scale = rng.choice([0.75, 0.9, 1.0, 1.2])
        step_scale = rng.choice([0.9, 1.0, 1.25, 1.5])
        first_scale = rng.choice([0.8, 0.95, 1.0, 1.1])
        leg_cap = rng.choice([4, 5, 6, None])
        age = rng.choice([24.0, 48.0, 72.0, 96.0, 168.0, None])
    else:
        dd_pause = rng.choice([3.0, 4.0, 5.0, 6.0, 8.0, 50.0])
        atr_pause = rng.choice([1.2, 1.6, 2.0, 2.5, 3.5, 50.0])
        adx_skip = rng.choice([30.0, 35.0, 45.0, 60.0, 100.0])
        stop_scale = rng.choice([0.75, 0.9, 1.0, 1.15, 1.3])
        tp_scale = rng.choice([0.85, 1.0, 1.15, 1.35])
        step_scale = rng.choice([0.85, 1.0, 1.15, 1.35])
        first_scale = rng.choice([0.9, 1.0, 1.1, 1.2])
        leg_cap = rng.choice([5, 6, None])
        age = rng.choice([48.0, 72.0, 96.0, 168.0, None])

    age_scope = rng.choice(["all", "long", "short"])
    stop_scope = rng.choice(["all", "long", "short"])
    for strategy in portfolio["strategies"]:
        direction = str(strategy.get("direction") or "")
        if stop_scope == "all" or stop_scope == direction:
            scale_stop(strategy, stop_scale)
        scale_take_profit(strategy, tp_scale)
        scale_spacing(strategy, step_scale)
        scale_first_order(strategy, first_scale)
        cap_legs(strategy, leg_cap)
        set_age(strategy, age, age_scope)

    risk["new_cycle_drawdown_pause_pct"] = dd_pause
    risk["new_cycle_atr_pause_pct"] = atr_pause
    risk["safety_skip_adx_threshold"] = adx_skip
    risk["max_global_budget_quote"] = fmt(budget)
    normalize_weights(cfg)

    tag = (
        f"guardv6-{profile}-{idx:04d}-dd{dd_pause}-atr{atr_pause}-adx{adx_skip}"
        f"-st{stop_scale}-tp{tp_scale}-sp{step_scale}-fo{first_scale}-legs{leg_cap}-age{age}"
    )
    for i, strategy in enumerate(portfolio["strategies"]):
        strategy["strategy_id"] = (
            f"{tag}-{i:02d}-{strategy.get('symbol')}-{strategy.get('direction')}"
        )[:180]
    meta = {
        "source": source_name,
        "dd_pause": dd_pause,
        "atr_pause": atr_pause,
        "adx_skip": adx_skip,
        "stop_scale": stop_scale,
        "tp_scale": tp_scale,
        "step_scale": step_scale,
        "first_scale": first_scale,
        "leg_cap": leg_cap,
        "age": age,
        "age_scope": age_scope,
        "stop_scope": stop_scope,
    }
    return tag, cfg, meta


def load_json(path: Path) -> Any:
    return json.loads(path.read_text())


def extract_sources(paths: list[str], budget: float, profile: str, max_sources: int) -> list[dict[str, Any]]:
    sources: list[dict[str, Any]] = []
    for text in paths:
        for path in sorted(Path().glob(text) if not text.startswith("/") else Path("/").glob(text[1:])):
            try:
                data = load_json(path)
            except Exception:
                continue
            if isinstance(data, dict) and "portfolio_config" in data:
                cfg = sanitize_config(data, budget)
                if cfg:
                    sources.append({"name": path.stem, "config": cfg, "full": {}, "path": str(path)})
                continue
            if not isinstance(data, dict):
                continue
            rows = list(data.get("full_results") or [])
            rows.extend(data.get("segment_validations") or [])
            for row in rows:
                cfg = row.get("config")
                full = row.get("full") or {}
                if not isinstance(cfg, dict) or not full.get("ok"):
                    continue
                clean = sanitize_config(cfg, budget)
                if clean is None:
                    continue
                sources.append(
                    {
                        "name": str(row.get("name") or f"{path.stem}_{row.get('idx')}"),
                        "config": clean,
                        "full": full,
                        "path": str(path),
                    }
                )
    dedup: dict[str, dict[str, Any]] = {}
    for source in sources:
        key = json.dumps(source["config"], sort_keys=True)
        current = dedup.get(key)
        if current is None or source_score(source, profile) > source_score(current, profile):
            dedup[key] = source
    selected = sorted(dedup.values(), key=lambda s: source_score(s, profile), reverse=True)

    # Blend in frontier buckets so the sweep sees both low-DD and high-return sides.
    target = TARGETS[profile]
    low_dd = [
        s
        for s in selected
        if s["full"].get("ok") and as_float(s["full"].get("dd"), 999.0) <= target["dd"] + 8.0
    ][: max(3, max_sources // 3)]
    high_ann = [
        s
        for s in selected
        if s["full"].get("ok") and as_float(s["full"].get("ann"), -999.0) >= target["ann"] * 0.7
    ][: max(3, max_sources // 3)]
    merged: list[dict[str, Any]] = []
    seen: set[str] = set()
    for bucket in (selected, low_dd, high_ann):
        for source in bucket:
            key = json.dumps(source["config"], sort_keys=True)
            if key in seen:
                continue
            seen.add(key)
            merged.append(source)
            if len(merged) >= max_sources:
                return merged
    return merged[:max_sources]


def source_score(source: dict[str, Any], profile: str) -> float:
    full = source.get("full") or {}
    target = TARGETS[profile]
    ann = as_float(full.get("ann"), target["ann"] * 0.65)
    dd = as_float(full.get("dd"), target["dd"] + 8.0)
    cap = as_float(full.get("cap"), 0.0)
    blocked = int(full.get("blocked") or 0)
    return (
        ann
        - 4.0 * max(0.0, dd - target["dd"])
        - 0.8 * max(0.0, target["ann"] - ann)
        - 2.0 * blocked
        + min(cap / 5000.0, 1.0)
    )


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
            "gate": (data.get("gate") or {}).get("passed"),
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


def full_job(job: tuple[int, str, dict[str, Any], dict[str, Any], argparse.Namespace]) -> dict[str, Any]:
    idx, name, cfg, meta, args = job
    return {"idx": idx, "name": name, "config": cfg, "meta": meta, "full": run_replay(args, cfg, FULL[0], FULL[1], args.timeout)}


def segment_validate(args: argparse.Namespace, item: dict[str, Any]) -> dict[str, Any]:
    segments = {}
    for name, (start, end) in SEGMENTS.items():
        segments[name] = run_replay(args, item["config"], start, end, args.segment_timeout)
    returns = [as_float(r.get("ret"), 0.0) for r in segments.values() if r.get("ok")]
    dds = [as_float(r.get("dd"), 999.0) for r in segments.values() if r.get("ok")]
    positive = sum(1 for value in returns if value >= 0)
    h1 = as_float(segments.get("h1_2023", {}).get("ret"), 0.0)
    full_ret = as_float(item["full"].get("ret"), 0.0)
    h1_ratio = h1 / full_ret if abs(full_ret) > 1e-9 else 0.0
    combined_24_26 = 0.0
    if all(segments.get(k, {}).get("ok") for k in ("2024", "2025", "2026_ytd")):
        acc = 1.0
        for k in ("2024", "2025", "2026_ytd"):
            acc *= 1.0 + as_float(segments[k].get("ret"), 0.0) / 100.0
        combined_24_26 = (acc - 1.0) * 100.0
    return {
        **item,
        "segments": segments,
        "segment_summary": {
            "positive_segments": positive,
            "max_segment_dd": max(dds) if dds else None,
            "h1_contribution_ratio": h1_ratio,
            "combined_2024_2026_return_pct": combined_24_26,
            "segment_returns": returns,
        },
    }


def segment_job(job: tuple[int, dict[str, Any], argparse.Namespace]) -> dict[str, Any]:
    _idx, item, args = job
    return segment_validate(args, item)


def score(args: argparse.Namespace, item: dict[str, Any]) -> float:
    target = TARGETS[args.profile]
    full = item.get("full") or {}
    if not full.get("ok"):
        return -1e9
    ann = as_float(full.get("ann"), -999.0)
    dd = as_float(full.get("dd"), 999.0)
    cap = as_float(full.get("cap"), 0.0)
    trades = as_float(full.get("trades"), 0.0)
    blocked = int(full.get("blocked") or 0)
    return (
        ann
        - 6.5 * max(0.0, dd - target["dd"])
        - 0.8 * max(0.0, target["ann"] - ann)
        - 4.0 * blocked
        - 0.0008 * max(0.0, trades - 12000.0)
        + min(cap / max(args.budget, 1.0), 1.0) * 2.0
    )


def passes(args: argparse.Namespace, item: dict[str, Any]) -> bool:
    target = TARGETS[args.profile]
    full = item.get("full") or {}
    if not full.get("ok") or full.get("principal_breached"):
        return False
    if as_float(full.get("ann"), -999.0) < target["ann"]:
        return False
    if as_float(full.get("dd"), 999.0) > target["dd"]:
        return False
    if int(full.get("blocked") or 0) != 0:
        return False
    if as_float(full.get("cap"), 0.0) > args.budget:
        return False
    summary = item.get("segment_summary")
    if not summary:
        return True
    if summary["positive_segments"] < target["min_pos"]:
        return False
    if summary["max_segment_dd"] is not None and summary["max_segment_dd"] > target["max_seg_dd"]:
        return False
    if summary["h1_contribution_ratio"] > target["h1"]:
        return False
    if args.profile in ("balanced", "aggressive") and summary["combined_2024_2026_return_pct"] < 0:
        return False
    return True


def save(path: Path, report: dict[str, Any]) -> None:
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(report, ensure_ascii=False, indent=2))
    tmp.replace(path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", choices=sorted(TARGETS), required=True)
    parser.add_argument("--budget", type=float, default=5000.0)
    parser.add_argument("--count", type=int, default=300)
    parser.add_argument("--seed", type=int, default=20260629)
    parser.add_argument("--jobs", type=int, default=20)
    parser.add_argument("--top-segment", type=int, default=24)
    parser.add_argument("--max-sources", type=int, default=18)
    parser.add_argument("--timeout", type=int, default=900)
    parser.add_argument("--segment-timeout", type=int, default=600)
    parser.add_argument("--source", action="append", required=True)
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
    sources = extract_sources(args.source, args.budget, args.profile, args.max_sources)
    if not sources:
        raise SystemExit("no source configs loaded")

    jobs = []
    for idx in range(args.count):
        source = rng.choice(sources)
        name, cfg, meta = mutate_config(source["name"], source["config"], args.budget, args.profile, rng, idx)
        cfg_path = cfg_dir / f"{name}.json"
        cfg_path.write_text(json.dumps(cfg, ensure_ascii=False, indent=2))
        jobs.append((idx, name, cfg, meta, args))

    report_path = out_dir / f"guard_v6_{args.profile}_b{int(args.budget)}_seed{args.seed}.json"
    run_log = out_dir / f"guard_v6_{args.profile}_b{int(args.budget)}_seed{args.seed}.log"
    report: dict[str, Any] = {
        "profile": args.profile,
        "budget": args.budget,
        "seed": args.seed,
        "count": args.count,
        "source_count": len(sources),
        "sources": [{"name": s["name"], "full": s.get("full"), "path": s.get("path")} for s in sources],
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "full_results": [],
        "segment_validations": [],
        "passes": [],
    }
    save(report_path, report)

    done = 0
    with futures.ProcessPoolExecutor(max_workers=args.jobs) as pool:
        future_map = {pool.submit(full_job, job): job[0] for job in jobs}
        with run_log.open("a") as log:
            for fut in futures.as_completed(future_map):
                done += 1
                try:
                    item = fut.result()
                except Exception as exc:
                    item = {"idx": future_map[fut], "name": "error", "config": None, "meta": {}, "full": {"ok": False, "error": repr(exc)}}
                report["full_results"].append(item)
                full = item["full"]
                if full.get("ok"):
                    line = (
                        f"DONE {done}/{len(jobs)} {item['name']} "
                        f"ann={as_float(full.get('ann'), -999):.2f} dd={as_float(full.get('dd'), 999):.2f} "
                        f"ret={as_float(full.get('ret'), 0):.2f} cap={as_float(full.get('cap'), 0):.1f} "
                        f"blocked={full.get('blocked')} trades={full.get('trades')}\n"
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
    seen_names: set[str] = set()
    for item in full_pass + ok[: args.top_segment]:
        if item["name"] in seen_names:
            continue
        seen_names.add(item["name"])
        segment_items.append(item)
    segment_items = segment_items[: max(args.top_segment, len(full_pass))]

    with futures.ProcessPoolExecutor(max_workers=max(1, min(args.jobs, len(segment_items)))) as pool:
        future_map = {
            pool.submit(segment_job, (idx, item, args)): idx
            for idx, item in enumerate(segment_items, start=1)
        }
        with run_log.open("a") as log:
            for fut in futures.as_completed(future_map):
                try:
                    validated = fut.result()
                except Exception as exc:
                    idx = future_map[fut]
                    base = segment_items[idx - 1]
                    validated = {
                        **base,
                        "segments": {},
                        "segment_summary": {
                            "positive_segments": 0,
                            "max_segment_dd": None,
                            "h1_contribution_ratio": 1.0,
                            "combined_2024_2026_return_pct": -999.0,
                            "segment_returns": [],
                            "error": repr(exc),
                        },
                    }
                report["segment_validations"].append(validated)
                seg = validated.get("segment_summary", {})
                line = (
                    f"SEG {len(report['segment_validations'])}/{len(segment_items)} {validated['name']} "
                    f"pos={seg.get('positive_segments')} max_seg_dd={seg.get('max_segment_dd')} "
                    f"h1_ratio={seg.get('h1_contribution_ratio')} ret2426={seg.get('combined_2024_2026_return_pct')}\n"
                )
                log.write(line)
                log.flush()
                save(report_path, report)

    report["passes"] = [r for r in report["segment_validations"] if passes(args, r)]
    report["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    save(report_path, report)
    print(report_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
