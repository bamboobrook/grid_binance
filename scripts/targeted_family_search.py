#!/usr/bin/env python3
"""Targeted family search around the best small-cap martingale candidates.

This is a read-only research runner. It mutates known near-miss portfolio JSON
configs, rescales first-order notionals to match per-strategy margin weights,
and executes live-parity `portfolio_budget_replay` in parallel.
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
    "conservative": {"ann": 50.0, "dd": 10.0, "min_pos": 4, "max_seg_dd": 12.0},
    "balanced": {"ann": 90.0, "dd": 20.0, "min_pos": 3, "max_seg_dd": 24.0},
    "aggressive": {"ann": 110.0, "dd": 30.0, "min_pos": 3, "max_seg_dd": 36.0},
}


def fmt(value: Any) -> str:
    if isinstance(value, str):
        return value
    text = f"{float(value):.10f}".rstrip("0").rstrip(".")
    return text if text else "0"


def sizing(strategy: dict[str, Any]) -> dict[str, Any]:
    return strategy["sizing"]["multiplier"]


def planned_margin(strategy: dict[str, Any]) -> float:
    sz = sizing(strategy)
    first = float(sz["first_order_quote"])
    mult = float(sz["multiplier"])
    legs = int(sz["max_legs"])
    lev = float(strategy.get("leverage") or 1)
    return sum(first * (mult**i) / lev for i in range(legs))


def rescale_to_weight(strategy: dict[str, Any], budget: float, weight_pct: float, util: float = 0.98) -> None:
    strategy["portfolio_weight_pct"] = fmt(weight_pct)
    current = planned_margin(strategy)
    if current <= 0:
        return
    target = budget * weight_pct / 100.0 * util
    factor = target / current
    sz = sizing(strategy)
    first = max(5.0, float(sz["first_order_quote"]) * factor)
    sz["first_order_quote"] = fmt(first)


def normalize_and_rescale(strategies: list[dict[str, Any]], budget: float, total_pct: float = 99.5) -> None:
    current = sum(float(s.get("portfolio_weight_pct", 0.0)) for s in strategies)
    if current <= 0:
        each = total_pct / max(1, len(strategies))
        for s in strategies:
            rescale_to_weight(s, budget, each)
        return
    for s in strategies:
        new_weight = float(s.get("portfolio_weight_pct", 0.0)) / current * total_pct
        rescale_to_weight(s, budget, new_weight)


def replace_stop(strategy: dict[str, Any], kind: str, bps: int, ema: int = 100) -> None:
    if kind == "regime":
        strategy["stop_loss"] = {"regime_break_stop": {"ema_period": ema, "drawdown_pct_bps": int(bps)}}
    else:
        strategy["stop_loss"] = {"strategy_drawdown_pct": {"pct_bps": int(bps)}}


def set_max_cycle_age(strategy: dict[str, Any], hours: float | None) -> None:
    risk = strategy.setdefault("risk_limits", {})
    if hours is None:
        risk.pop("max_cycle_age_hours", None)
    else:
        risk["max_cycle_age_hours"] = float(hours)


def clear_indicator_trigger(strategy: dict[str, Any], prefix: str) -> None:
    triggers = []
    for trig in strategy.get("entry_triggers", []):
        expr = (trig.get("indicator_expression") or {}).get("expression")
        if isinstance(expr, str) and expr.startswith(prefix):
            continue
        triggers.append(trig)
    strategy["entry_triggers"] = triggers


def add_trigger(strategy: dict[str, Any], expression: str) -> None:
    triggers = strategy.setdefault("entry_triggers", [])
    for trig in triggers:
        if (trig.get("indicator_expression") or {}).get("expression") == expression:
            return
    triggers.append({"indicator_expression": {"expression": expression}})


def ensure_btc_observer(strategies: list[dict[str, Any]], budget: float) -> None:
    if any(s.get("strategy_id") == "btcobs-family" for s in strategies):
        return
    obs = {
        "strategy_id": "btcobs-family",
        "symbol": "BTCUSDT",
        "market": "usd_m_futures",
        "direction": "long",
        "direction_mode": "long_and_short",
        "margin_mode": "isolated",
        "leverage": 3,
        "spacing": {"fixed_percent": {"step_bps": 500}},
        "sizing": {"multiplier": {"first_order_quote": "10", "multiplier": "1.1", "max_legs": 2}},
        "take_profit": {"percent": {"bps": 5000}},
        "stop_loss": {"strategy_drawdown_pct": {"pct_bps": 50000}},
        "indicators": [{"atr": {"period": 14}}],
        "entry_triggers": [{"cooldown": {"seconds": 86400000}}],
        "risk_limits": {},
        "portfolio_weight_pct": "0.5",
    }
    rescale_to_weight(obs, budget, 0.5)
    strategies.append(obs)


def find_strategy(strategies: list[dict[str, Any]], symbol: str, direction: str) -> dict[str, Any] | None:
    for strategy in strategies:
        if strategy.get("symbol") == symbol and strategy.get("direction") == direction:
            return strategy
    return None


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def portfolio_from_base(base: dict[str, Any]) -> dict[str, Any]:
    return copy.deepcopy(base.get("portfolio_config", base))


def set_family_strategy_ids(strategies: list[dict[str, Any]], tag: str) -> None:
    for i, strategy in enumerate(strategies):
        strategy["strategy_id"] = f"{tag}-{i:02d}-{strategy.get('symbol')}-{strategy.get('direction')}"


def build_variant(
    base_name: str,
    base: dict[str, Any],
    budget: float,
    rng: random.Random,
    idx: int,
    profile: str,
) -> tuple[str, dict[str, Any]]:
    portfolio = portfolio_from_base(base)
    strategies = portfolio["strategies"]

    # Remove old observer; add a normalized one after mutations if BTC refs exist.
    strategies = [s for s in strategies if not str(s.get("strategy_id", "")).startswith("btcobs")]

    mode = rng.choice(["income", "dd_cut", "robust_mix", "short_boost", "age_exit", "dd_income"])
    if profile == "conservative":
        mode = rng.choice(["dd_cut", "robust_mix", "age_exit", "dd_income"])
    if profile == "aggressive":
        mode = rng.choice(["income", "short_boost", "robust_mix"])

    # Core weights. These are applied to known engines if present.
    targets = {
        "INJUSDT:long": rng.choice([32, 36, 40, 44, 48]),
        "AAVEUSDT:long": rng.choice([12, 16, 20, 24]),
        "FILUSDT:short": rng.choice([22, 26, 30, 34]),
        "FILUSDT:long": rng.choice([2, 4, 6, 8]),
        "INJUSDT:short": rng.choice([1, 2, 3]),
        "AAVEUSDT:short": rng.choice([1, 2, 3]),
    }
    if mode == "dd_cut":
        targets["INJUSDT:long"] = rng.choice([24, 28, 32, 36])
        targets["AAVEUSDT:long"] = rng.choice([18, 22, 26])
        targets["FILUSDT:short"] = rng.choice([28, 32, 36])
    if mode == "dd_income":
        targets["INJUSDT:long"] = rng.choice([34, 38, 42, 46])
        targets["AAVEUSDT:long"] = rng.choice([18, 20, 22, 24])
        targets["FILUSDT:short"] = rng.choice([26, 30, 34])
        targets["FILUSDT:long"] = rng.choice([3, 5, 7])
    if mode == "income":
        targets["INJUSDT:long"] = rng.choice([42, 46, 50, 54])
        targets["AAVEUSDT:long"] = rng.choice([12, 16, 20])
    if mode == "short_boost":
        targets["FILUSDT:short"] = rng.choice([32, 36, 40])
        targets["INJUSDT:short"] = rng.choice([2, 4, 6])

    for key, weight in targets.items():
        symbol, direction = key.split(":")
        strategy = find_strategy(strategies, symbol, direction)
        if strategy is not None:
            strategy["portfolio_weight_pct"] = fmt(weight)

    long_stop = rng.choice([900, 1100, 1300, 1500, 1800] if profile != "aggressive" else [1200, 1500, 1800, 2400])
    if mode == "dd_income":
        long_stop = rng.choice([1200, 1400, 1600, 1800])
    aave_stop = rng.choice([1800, 2400, 3000, 3500] if profile != "conservative" else [1400, 1800, 2400])
    fil_stop = rng.choice([800, 1000, 1200, 1500])
    stop_kind = rng.choice(["strategy", "strategy", "regime"])
    ema = rng.choice([50, 100])
    age = rng.choice([48.0, 96.0, 168.0, 336.0, None])

    for s in strategies:
        if s.get("direction") != "long":
            continue
        sym = s.get("symbol")
        bps = aave_stop if sym == "AAVEUSDT" else fil_stop if sym == "FILUSDT" else long_stop
        replace_stop(s, stop_kind, bps, ema)
        if mode in ("dd_cut", "age_exit", "dd_income"):
            set_max_cycle_age(s, age)
        if mode in ("dd_cut", "robust_mix", "dd_income") and sym != "BTCUSDT":
            if rng.random() < 0.55:
                add_trigger(s, "BTCUSDT.close > BTCUSDT.ema(30)")
            if rng.random() < 0.25:
                add_trigger(s, "close > ema(50)")

    for s in strategies:
        if s.get("direction") == "short" and rng.random() < 0.65:
            clear_indicator_trigger(s, "BTCUSDT.close")
            add_trigger(s, rng.choice(["BTCUSDT.close < BTCUSDT.ema(30)", "BTCUSDT.close < BTCUSDT.ema(50)"]))
        if s.get("direction") == "short" and mode == "short_boost":
            replace_stop(s, "strategy", rng.choice([800, 1200, 1800, 2500, 4000]))

    # Mutate ladder depth for long engines.
    for sym in ["INJUSDT", "AAVEUSDT", "FILUSDT"]:
        s = find_strategy(strategies, sym, "long")
        if s is not None:
            sizing(s)["max_legs"] = rng.choice([5, 5, 6, 7] if profile != "conservative" else [4, 5, 6])
            if sym == "INJUSDT" and mode == "income":
                sizing(s)["max_legs"] = rng.choice([6, 7])
            if sym == "INJUSDT" and mode == "dd_income":
                sizing(s)["max_legs"] = rng.choice([5, 6])
            if sym == "AAVEUSDT" and mode == "dd_income":
                sizing(s)["max_legs"] = rng.choice([5, 6])

    # Optional robust satellites. Keep simple fixed-percent strategies for live parity.
    if mode == "robust_mix" or rng.random() < 0.35:
        satellite_symbols = rng.sample(["NEARUSDT", "GALAUSDT", "ADAUSDT", "TRXUSDT", "LTCUSDT", "LINKUSDT"], rng.choice([2, 3, 4]))
        sat_total = rng.choice([8, 12, 16, 20])
        per = sat_total / len(satellite_symbols)
        for sym in satellite_symbols:
            if find_strategy(strategies, sym, "long") is not None:
                continue
            strat = {
                "strategy_id": f"sat-{sym}-L",
                "symbol": sym,
                "market": "usd_m_futures",
                "direction": "long",
                "direction_mode": "long_and_short",
                "margin_mode": "isolated",
                "leverage": rng.choice([5, 8, 10]),
                "spacing": {"fixed_percent": {"step_bps": rng.choice([80, 120, 180, 260])}},
                "sizing": {
                    "multiplier": {
                        "first_order_quote": "10",
                        "multiplier": fmt(rng.choice([1.5, 1.8, 2.0, 2.3])),
                        "max_legs": rng.choice([4, 5, 6]),
                    }
                },
                "take_profit": {"percent": {"bps": rng.choice([120, 220, 350, 450])}},
                "stop_loss": {"strategy_drawdown_pct": {"pct_bps": rng.choice([900, 1200, 1500, 2200])}},
                "indicators": [{"atr": {"period": 21}}, {"adx": {"period": 14}}],
                "entry_triggers": [{"cooldown": {"seconds": rng.choice([7200, 21600, 43200])}}],
                "risk_limits": {},
                "portfolio_weight_pct": fmt(per),
            }
            if rng.random() < 0.5:
                add_trigger(strat, "BTCUSDT.close > BTCUSDT.ema(30)")
            strategies.append(strat)

    # P4 short add-ons aimed at 2025 alt downtrends, kept small.
    if mode == "short_boost" and rng.random() < 0.75:
        short_symbols = rng.sample(["APTUSDT", "DOTUSDT", "COMPUSDT", "NEARUSDT"], rng.choice([2, 3]))
        short_total = rng.choice([6, 10, 14])
        for sym in short_symbols:
            if find_strategy(strategies, sym, "short") is not None:
                continue
            strat = {
                "strategy_id": f"p4s-{sym}-S",
                "symbol": sym,
                "market": "usd_m_futures",
                "direction": "short",
                "direction_mode": "short_only",
                "margin_mode": "isolated",
                "leverage": rng.choice([5, 8, 10]),
                "spacing": {"fixed_percent": {"step_bps": rng.choice([120, 180, 260, 350])}},
                "sizing": {
                    "multiplier": {
                        "first_order_quote": "10",
                        "multiplier": fmt(rng.choice([1.25, 1.4, 1.6])),
                        "max_legs": rng.choice([4, 5, 6]),
                    }
                },
                "take_profit": {"percent": {"bps": rng.choice([120, 220, 350])}},
                "stop_loss": {"regime_break_stop": {"ema_period": 100, "drawdown_pct_bps": rng.choice([800, 1200, 1800])}},
                "indicators": [{"atr": {"period": 21}}, {"adx": {"period": 14}}],
                "entry_triggers": [{"cooldown": {"seconds": rng.choice([7200, 21600, 43200])}}],
                "risk_limits": {"max_cycle_age_hours": rng.choice([48.0, 96.0, 168.0])},
                "portfolio_weight_pct": fmt(short_total / len(short_symbols)),
            }
            strategies.append(strat)

    # If any BTC dependency exists, add observer for live kline injection.
    if "BTCUSDT." in json.dumps(strategies):
        ensure_btc_observer(strategies, budget)

    normalize_and_rescale(strategies, budget, 99.5)
    set_family_strategy_ids(strategies, f"fam-{base_name}-{idx:04d}-{mode}")
    portfolio["strategies"] = strategies
    portfolio["risk_limits"] = {"max_global_budget_quote": fmt(budget)}
    portfolio["direction_mode"] = "long_and_short"
    return f"{base_name}_{mode}_{idx:04d}", {"portfolio_config": portfolio}


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
            return {"ok": False, "error": proc.stderr[-1600:], "returncode": proc.returncode}
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
    return {"idx": idx, "name": name, "config": cfg, "full": run_replay(args, cfg, FULL[0], FULL[1], args.timeout)}


def segment_validate(args: argparse.Namespace, item: dict[str, Any]) -> dict[str, Any]:
    segments = {}
    for name, (start, end) in SEGMENTS.items():
        segments[name] = run_replay(args, item["config"], start, end, args.segment_timeout)
    returns = [float(r.get("ret") or 0.0) for r in segments.values() if r.get("ok")]
    dds = [float(r.get("dd") or 999.0) for r in segments.values() if r.get("ok")]
    pos = sum(1 for r in returns if r >= 0)
    h1 = float(segments.get("h1_2023", {}).get("ret") or 0.0)
    full_ret = float(item["full"].get("ret") or 0.0)
    h1_ratio = h1 / full_ret if abs(full_ret) > 1e-9 else 0.0
    return {
        **item,
        "segments": segments,
        "segment_summary": {
            "positive_segments": pos,
            "max_segment_dd": max(dds) if dds else None,
            "h1_contribution_ratio": h1_ratio,
            "segment_returns": returns,
        },
    }


def segment_job(job: tuple[int, dict[str, Any], argparse.Namespace]) -> dict[str, Any]:
    _, item, args = job
    return segment_validate(args, item)


def passes(args: argparse.Namespace, item: dict[str, Any]) -> bool:
    t = TARGETS[args.profile]
    full = item["full"]
    if not full.get("ok") or full.get("principal_breached"):
        return False
    if float(full.get("ann") or -999.0) < t["ann"] or float(full.get("dd") or 999.0) > t["dd"]:
        return False
    if int(full.get("blocked") or 0) != 0:
        return False
    if float(full.get("cap") or 0.0) > args.budget:
        return False
    seg = item.get("segment_summary")
    if not seg:
        return True
    if seg["positive_segments"] < t["min_pos"]:
        return False
    if seg["max_segment_dd"] is not None and seg["max_segment_dd"] > t["max_seg_dd"]:
        return False
    if seg["h1_contribution_ratio"] > 0.55:
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
    parser.add_argument("--count", type=int, default=240)
    parser.add_argument("--seed", type=int, default=20260629)
    parser.add_argument("--jobs", type=int, default=20)
    parser.add_argument("--top-segment", type=int, default=24)
    parser.add_argument("--timeout", type=int, default=900)
    parser.add_argument("--segment-timeout", type=int, default=600)
    parser.add_argument("--base", action="append", required=True)
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
    bases = [(Path(path).stem, load_json(Path(path))) for path in args.base]

    configs = []
    for idx in range(args.count):
        base_name, base = rng.choice(bases)
        name, cfg = build_variant(base_name, base, args.budget, rng, idx, args.profile)
        cfg_path = cfg_dir / f"{name}.json"
        cfg_path.write_text(json.dumps(cfg, ensure_ascii=False, indent=2))
        configs.append((idx, name, cfg, args))

    report_path = out_dir / f"targeted_{args.profile}_b{int(args.budget)}_seed{args.seed}.json"
    run_log = out_dir / f"targeted_{args.profile}_b{int(args.budget)}_seed{args.seed}.log"
    report: dict[str, Any] = {
        "profile": args.profile,
        "budget": args.budget,
        "seed": args.seed,
        "count": args.count,
        "bases": args.base,
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "full_results": [],
        "segment_validations": [],
        "passes": [],
    }

    done = 0
    with futures.ProcessPoolExecutor(max_workers=args.jobs) as pool:
        future_map = {pool.submit(full_job, job): job[0] for job in configs}
        with run_log.open("a") as log:
            for fut in futures.as_completed(future_map):
                done += 1
                try:
                    item = fut.result()
                except Exception as exc:
                    item = {"idx": future_map[fut], "name": "error", "config": None, "full": {"ok": False, "error": repr(exc)}}
                report["full_results"].append(item)
                f = item["full"]
                if f.get("ok"):
                    line = (
                        f"DONE {done}/{len(configs)} {item['name']} "
                        f"ann={float(f.get('ann') or -999):.2f} dd={float(f.get('dd') or 999):.2f} "
                        f"ret={float(f.get('ret') or 0):.2f} cap={float(f.get('cap') or 0):.1f} "
                        f"blocked={f.get('blocked')} trades={f.get('trades')}\n"
                    )
                else:
                    line = f"DONE {done}/{len(configs)} {item['name']} ERROR {f.get('error')}\n"
                log.write(line)
                log.flush()
                if done % max(1, min(args.jobs, 10)) == 0:
                    save(report_path, report)

    def score(item: dict[str, Any]) -> float:
        full = item["full"]
        ann = float(full.get("ann") or -999.0)
        dd = float(full.get("dd") or 999.0)
        t = TARGETS[args.profile]
        blocked = int(full.get("blocked") or 0)
        return ann - 4.0 * max(0.0, dd - t["dd"]) - 0.05 * blocked

    ok = [r for r in report["full_results"] if r["full"].get("ok")]
    ok.sort(key=score, reverse=True)
    segment_items = list(enumerate(ok[: args.top_segment], start=1))
    with futures.ProcessPoolExecutor(max_workers=max(1, min(args.jobs, len(segment_items)))) as pool:
        future_map = {
            pool.submit(segment_job, (idx, item, args)): idx
            for idx, item in segment_items
        }
        with run_log.open("a") as log:
            for fut in futures.as_completed(future_map):
                try:
                    validated = fut.result()
                except Exception as exc:
                    idx = future_map[fut]
                    base = segment_items[idx - 1][1]
                    validated = {
                        **base,
                        "segments": {},
                        "segment_summary": {
                            "positive_segments": 0,
                            "max_segment_dd": None,
                            "h1_contribution_ratio": 1.0,
                            "segment_returns": [],
                            "error": repr(exc),
                        },
                    }
                report["segment_validations"].append(validated)
                seg = validated.get("segment_summary", {})
                line = (
                    f"SEG {len(report['segment_validations'])}/{len(segment_items)} {validated['name']} "
                    f"pos={seg.get('positive_segments')} max_seg_dd={seg.get('max_segment_dd')} "
                    f"h1_ratio={seg.get('h1_contribution_ratio')}\n"
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
