#!/usr/bin/env python3
"""Convert curve-frontier martingale portfolios into real budget replays.

This research-only runner takes optimistic curve-combination frontiers, rebuilds
live-parity portfolio JSON configs from the source candidate configs, sizes
them to a real margin budget, and validates them with `portfolio_budget_replay`.
It never touches DB application state, Binance, live trading, orders, or
positions.
"""

from __future__ import annotations

import argparse
import concurrent.futures as futures
import csv
import gzip
import json
import math
import os
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
    "conservative": {"ann": 50.0, "dd": 10.0, "min_pos": 4, "max_seg_dd": 12.0},
    "balanced": {"ann": 90.0, "dd": 20.0, "min_pos": 3, "max_seg_dd": 24.0},
    "aggressive": {"ann": 110.0, "dd": 30.0, "min_pos": 3, "max_seg_dd": 36.0},
}


@dataclass
class SourceCandidate:
    cid: str
    symbol: str
    direction_mode: str
    ann: float
    dd: float
    planned_margin: float
    max_capital_used: float
    config: dict[str, Any]


def fmt_decimal(value: Any) -> str:
    if isinstance(value, str):
        return value
    text = f"{float(value):.10f}".rstrip("0").rstrip(".")
    return text if text else "0"


def finite_float(value: Any, default: float = 0.0) -> float:
    try:
        out = float(value)
        return out if math.isfinite(out) else default
    except Exception:
        return default


def load_candidates(path: Path) -> dict[str, SourceCandidate]:
    csv.field_size_limit(1024 * 1024 * 1024)
    out: dict[str, SourceCandidate] = {}
    with gzip.open(path, "rt", newline="") as fh:
        reader = csv.DictReader(fh)
        for row in reader:
            try:
                out[row["candidate_id"]] = SourceCandidate(
                    cid=row["candidate_id"],
                    symbol=row["symbol"],
                    direction_mode=row["direction_mode"],
                    ann=finite_float(row.get("annualized_return_pct")),
                    dd=finite_float(row.get("max_drawdown_pct")),
                    planned_margin=finite_float(row.get("planned_margin_quote")),
                    max_capital_used=finite_float(row.get("max_capital_used_quote")),
                    config=json.loads(row["config"]),
                )
            except Exception:
                continue
    return out


def portfolio_root(config: dict[str, Any]) -> dict[str, Any]:
    return config.get("portfolio_config") or config


def sizing_multiplier(strategy: dict[str, Any]) -> dict[str, Any] | None:
    sizing = strategy.get("sizing") or {}
    mult = sizing.get("multiplier")
    return mult if isinstance(mult, dict) else None


def leverage(strategy: dict[str, Any]) -> float:
    market = str(strategy.get("market") or "usd_m_futures").lower()
    if market == "spot":
        return 1.0
    return max(1.0, finite_float(strategy.get("leverage"), 1.0))


def margin_coeff(multiplier: float, legs: int, lev: float) -> float:
    return sum((multiplier**i) / lev for i in range(max(0, legs)))


def planned_strategy_margin(strategy: dict[str, Any]) -> float:
    mult = sizing_multiplier(strategy)
    if not mult:
        return 0.0
    first = finite_float(mult.get("first_order_quote"), 0.0)
    multiplier = finite_float(mult.get("multiplier"), 1.0)
    legs = int(finite_float(mult.get("max_legs"), 0.0))
    return first * margin_coeff(multiplier, legs, leverage(strategy))


def has_live_parity(strategy: dict[str, Any], convert_tp: bool) -> bool:
    tp = strategy.get("take_profit")
    if not isinstance(tp, dict):
        return False
    if "percent" not in tp and not (convert_tp and "atr" in tp):
        return False
    sl = strategy.get("stop_loss")
    if sl is not None:
        if not isinstance(sl, dict):
            return False
        if "strategy_drawdown_pct" not in sl and "regime_break_stop" not in sl:
            return False
    return sizing_multiplier(strategy) is not None


def normalize_tp(strategy: dict[str, Any], profile: str, policy: str) -> bool:
    tp = strategy.get("take_profit") or {}
    if "percent" in tp:
        return True
    if policy != "convert" or "atr" not in tp:
        return False
    fallback = {"conservative": 120, "balanced": 180, "aggressive": 240}[profile]
    strategy["take_profit"] = {"percent": {"bps": fallback}}
    return True


def normalize_spacing(strategy: dict[str, Any], profile: str, policy: str) -> bool:
    spacing = strategy.get("spacing") or {}
    if "fixed_percent" in spacing:
        return True
    if policy != "fixed":
        return True
    fallback = {"conservative": 160, "balanced": 120, "aggressive": 90}[profile]
    strategy["spacing"] = {"fixed_percent": {"step_bps": fallback}}
    return True


def fit_strategy_size(
    strategy: dict[str, Any],
    allocation_margin: float,
    profile: str,
    sizing_policy: str,
    exchange_min_notional: float,
) -> dict[str, Any] | None:
    mult = sizing_multiplier(strategy)
    if not mult:
        return None
    lev = leverage(strategy)
    original_multiplier = finite_float(mult.get("multiplier"), 1.0)
    original_legs = int(finite_float(mult.get("max_legs"), 1.0))
    multiplier = original_multiplier
    if sizing_policy == "soft":
        cap = {"conservative": 1.45, "balanced": 1.60, "aggressive": 1.80}[profile]
        multiplier = min(multiplier, cap)

    # Use a little headroom because runtime first-leg cap floors and fee/slippage
    # can make exact cap fits fragile.
    target_margin = max(0.0, allocation_margin * 0.94)
    min_first = max(exchange_min_notional, 5.0)
    best: tuple[int, float, float] | None = None
    for legs in range(max(1, original_legs), 0, -1):
        coeff = margin_coeff(multiplier, legs, lev)
        if coeff <= 0:
            continue
        first = max(min_first, target_margin / coeff)
        margin = first * coeff
        if margin <= allocation_margin * 0.995 + 1e-9:
            best = (legs, first, margin)
            break
    if best is None:
        # If even a single min-notional leg does not fit its allocation, the
        # runtime will floor the cap at first-leg margin and distort weights.
        # Skip this conversion instead of producing a misleading replay.
        first_leg_margin = min_first / lev
        if first_leg_margin > allocation_margin * 1.05 + 1e-9:
            return None
        best = (1, min_first, first_leg_margin)

    legs, first, _margin = best
    mult["first_order_quote"] = fmt_decimal(round(first, 6))
    mult["multiplier"] = fmt_decimal(round(multiplier, 8))
    mult["max_legs"] = int(legs)
    return strategy


def internal_strategy_weights(strategies: list[dict[str, Any]], mode: str) -> list[float]:
    if not strategies:
        return []
    if mode == "equal":
        return [1.0 / len(strategies)] * len(strategies)
    margins = [max(0.0, planned_strategy_margin(strategy)) for strategy in strategies]
    total = sum(margins)
    if total <= 0:
        return [1.0 / len(strategies)] * len(strategies)
    return [margin / total for margin in margins]


def build_real_config(
    *,
    profile: str,
    budget: float,
    combo: dict[str, Any],
    candidates: dict[str, SourceCandidate],
    split_mode: str,
    tp_policy: str,
    spacing_policy: str,
    sizing_policy: str,
    exchange_min_notional: float,
    name: str,
) -> tuple[dict[str, Any] | None, str | None]:
    strategies: list[dict[str, Any]] = []
    ids = combo.get("ids") or []
    weights = combo.get("weights") or []
    if len(ids) != len(weights):
        return None, "ids/weights length mismatch"
    for cid, candidate_weight in zip(ids, weights, strict=True):
        source = candidates.get(cid)
        if source is None:
            return None, f"missing source candidate {cid}"
        root = portfolio_root(source.config)
        source_strategies = [json.loads(json.dumps(s)) for s in root.get("strategies") or []]
        source_strategies = [
            s for s in source_strategies if has_live_parity(s, convert_tp=(tp_policy == "convert"))
        ]
        if not source_strategies:
            return None, f"candidate {cid} has no live-parity strategies"
        internal = internal_strategy_weights(source_strategies, split_mode)
        for local_idx, (strategy, inner_weight) in enumerate(zip(source_strategies, internal, strict=True)):
            if not normalize_tp(strategy, profile, tp_policy):
                return None, f"candidate {cid} TP rejected"
            if not normalize_spacing(strategy, profile, spacing_policy):
                return None, f"candidate {cid} spacing rejected"
            strategy_weight = float(candidate_weight) * inner_weight
            allocation_margin = budget * strategy_weight
            fitted = fit_strategy_size(
                strategy,
                allocation_margin,
                profile,
                sizing_policy,
                exchange_min_notional,
            )
            if fitted is None:
                return None, f"candidate {cid} cannot fit min first leg into allocation"
            fitted["strategy_id"] = (
                f"{name}-{len(strategies):02d}-{source.symbol}-{fitted.get('direction')}-{local_idx}"
            )
            fitted["portfolio_weight_pct"] = fmt_decimal(strategy_weight * 100.0)
            risk = fitted.setdefault("risk_limits", {})
            # Existing source configs often carry search-time nulls. Preserve
            # structural live-parity fields such as max_cycle_age_hours, but let
            # the replay/live runtime apply fresh budget caps from weights.
            for key in (
                "max_global_budget_quote",
                "max_symbol_budget_quote",
                "max_strategy_budget_quote",
                "max_direction_budget_quote",
            ):
                risk.pop(key, None)
            strategies.append(fitted)
    if not strategies:
        return None, "empty strategy list"
    return {
        "portfolio_config": {
            "direction_mode": "long_and_short",
            "risk_limits": {"max_global_budget_quote": fmt_decimal(budget)},
            "strategies": strategies,
        }
    }, None


def run_replay(
    *,
    replay_bin: Path,
    config: dict[str, Any],
    budget: float,
    profile: str,
    start_ms: int,
    end_ms: int,
    market_data: Path,
    funding_data: Path,
    exchange_min_notional: float,
    timeout: int,
    equity_curve_points: int = 0,
) -> dict[str, Any]:
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as fh:
        json.dump(config, fh)
        cfg_path = fh.name
    try:
        cmd = [
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
            fmt_decimal(exchange_min_notional),
            "--equity-curve-points",
            str(equity_curve_points),
        ]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        if proc.returncode != 0:
            return {
                "ok": False,
                "returncode": proc.returncode,
                "error": (proc.stderr or proc.stdout)[-2400:],
            }
        raw = json.loads(proc.stdout)
        on = raw.get("on_budget") or {}
        return {
            "ok": True,
            "ann": on.get("annualized_return_pct"),
            "dd": on.get("max_drawdown_pct"),
            "ret": on.get("total_return_pct"),
            "principal_breached": on.get("principal_breached"),
            "min_equity": on.get("min_equity_quote"),
            "cap": raw.get("max_capital_used_quote"),
            "blocked": raw.get("budget_blocked_legs"),
            "trades": raw.get("trade_count"),
            "stops": raw.get("stop_count"),
            "gate": (raw.get("gate") or {}).get("passed"),
            "symbols": raw.get("symbols"),
            "strategy_count": raw.get("strategy_count"),
            "fee": raw.get("total_fee_quote"),
            "slippage": raw.get("total_slippage_quote"),
            "funding": raw.get("total_funding_quote"),
            "first_leg_margin_total": raw.get("first_leg_margin_total_quote"),
            "full_series_margin": raw.get("full_series_margin_quote"),
            "budget_capped_projected_margin": raw.get("budget_capped_projected_margin_quote"),
            "minimum_capital": raw.get("minimum_capital"),
            "rejection_breakdown": raw.get("rejection_breakdown"),
            "equity_curve": raw.get("equity_curve"),
        }
    except subprocess.TimeoutExpired:
        return {"ok": False, "error": "timeout"}
    except Exception as exc:
        return {"ok": False, "error": f"{type(exc).__name__}: {exc}"}
    finally:
        try:
            os.unlink(cfg_path)
        except OSError:
            pass


def target_pass(profile: str, result: dict[str, Any]) -> bool:
    target = TARGETS[profile]
    return (
        result.get("ok")
        and finite_float(result.get("ann"), -999) > target["ann"]
        and finite_float(result.get("dd"), 999) <= target["dd"]
        and not result.get("principal_breached")
        and int(result.get("blocked") or 0) == 0
    )


def full_score(profile: str, result: dict[str, Any]) -> float:
    target = TARGETS[profile]
    ann = finite_float(result.get("ann"), -999.0)
    dd = finite_float(result.get("dd"), 999.0)
    blocked = int(result.get("blocked") or 0)
    breach = 1 if result.get("principal_breached") else 0
    return ann - 3.0 * max(0.0, dd - target["dd"]) - 20.0 * blocked - 100.0 * breach


def segment_summary(full: dict[str, Any], segments: dict[str, dict[str, Any]]) -> dict[str, Any]:
    returns = []
    dds = []
    for name in SEGMENTS:
        result = segments.get(name) or {}
        if result.get("ok"):
            returns.append(finite_float(result.get("ret")))
            dds.append(finite_float(result.get("dd")))
    positives = sum(1 for ret in returns if ret >= 0)
    h1 = finite_float((segments.get("h1_2023") or {}).get("ret"))
    full_ret = finite_float(full.get("ret"))
    h1_ratio = h1 / full_ret if abs(full_ret) > 1e-9 else 999.0
    compound_24_26 = 0.0
    has_24_26 = True
    for name in ("2024", "2025", "2026_ytd"):
        result = segments.get(name) or {}
        if not result.get("ok"):
            has_24_26 = False
            break
        compound_24_26 = (1 + compound_24_26 / 100.0) * (1 + finite_float(result.get("ret")) / 100.0)
        compound_24_26 = (compound_24_26 - 1.0) * 100.0
    return {
        "positive_segments": positives,
        "max_segment_dd": max(dds) if dds else None,
        "segment_returns": returns,
        "h1_contribution_ratio": h1_ratio,
        "combined_24_26": compound_24_26 if has_24_26 else None,
    }


def segment_pass(profile: str, full: dict[str, Any], summary: dict[str, Any]) -> bool:
    target = TARGETS[profile]
    if not target_pass(profile, full):
        return False
    if int(summary.get("positive_segments") or 0) < target["min_pos"]:
        return False
    max_seg_dd = summary.get("max_segment_dd")
    if max_seg_dd is None or finite_float(max_seg_dd, 999.0) > target["max_seg_dd"]:
        return False
    combined = summary.get("combined_24_26")
    if combined is None or finite_float(combined, -999.0) < 0:
        return False
    if finite_float(summary.get("h1_contribution_ratio"), 999.0) > 0.70:
        return False
    return True


def save_json(path: Path, data: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(data, ensure_ascii=False, indent=2))
    tmp.replace(path)


def load_frontier_jobs(args: argparse.Namespace, candidates: dict[str, SourceCandidate]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    frontier = json.loads(args.frontier.read_text())
    configs: list[dict[str, Any]] = []
    rejected: list[dict[str, Any]] = []
    variant_specs = [
        ("strict_crop_equal", "equal", "strict", "keep", "crop"),
        ("convert_crop_equal", "equal", "convert", "keep", "crop"),
        ("convert_fixed_crop_equal", "equal", "convert", "fixed", "crop"),
        ("convert_fixed_soft_equal", "equal", "convert", "fixed", "soft"),
        ("convert_fixed_crop_margin", "margin", "convert", "fixed", "crop"),
    ]
    buckets_by_profile = {
        "conservative": ["strict", "profile_full_dd", "segment_relaxed"],
        "balanced": ["strict", "profile_full_dd", "full_dd_plus5", "segment_relaxed"],
        "aggressive": ["strict", "profile_full_dd", "full_dd_plus5", "segment_relaxed"],
    }
    seq = 0
    for profile in args.profiles:
        profile_frontiers = (frontier.get("profiles") or {}).get(profile, {}).get("frontiers") or {}
        for bucket in buckets_by_profile[profile]:
            for rank, combo in enumerate((profile_frontiers.get(bucket) or [])[: args.top_per_bucket], 1):
                for budget in args.budgets:
                    for variant, split_mode, tp_policy, spacing_policy, sizing_policy in variant_specs:
                        seq += 1
                        name = f"cfreal-{profile}-{bucket}-{rank:02d}-{variant}-b{int(budget)}"
                        cfg, reason = build_real_config(
                            profile=profile,
                            budget=budget,
                            combo=combo,
                            candidates=candidates,
                            split_mode=split_mode,
                            tp_policy=tp_policy,
                            spacing_policy=spacing_policy,
                            sizing_policy=sizing_policy,
                            exchange_min_notional=args.exchange_min_notional,
                            name=name,
                        )
                        meta = {
                            "seq": seq,
                            "name": name,
                            "profile": profile,
                            "budget": budget,
                            "bucket": bucket,
                            "rank": rank,
                            "variant": variant,
                            "curve_full": combo.get("full"),
                            "curve_summary": {
                                "positives": combo.get("positives"),
                                "combined_24_26": combo.get("combined_24_26"),
                                "max_seg_dd": combo.get("max_seg_dd"),
                                "h1_ratio": combo.get("h1_ratio"),
                            },
                            "ids": combo.get("ids"),
                            "symbols": combo.get("symbols"),
                            "weights": combo.get("weights"),
                        }
                        if cfg is None:
                            rejected.append({**meta, "reason": reason})
                            continue
                        configs.append({**meta, "config": cfg})
    return configs, rejected


def full_worker(payload: tuple[dict[str, Any], dict[str, Any]]) -> dict[str, Any]:
    job, opts = payload
    result = run_replay(
        replay_bin=Path(opts["replay_bin"]),
        config=job["config"],
        budget=job["budget"],
        profile=job["profile"],
        start_ms=FULL[0],
        end_ms=FULL[1],
        market_data=Path(opts["market_data"]),
        funding_data=Path(opts["funding_data"]),
        exchange_min_notional=opts["exchange_min_notional"],
        timeout=opts["timeout"],
        equity_curve_points=opts["equity_curve_points"],
    )
    return {**job, "full": result}


def segment_worker(payload: tuple[dict[str, Any], dict[str, Any]]) -> dict[str, Any]:
    item, opts = payload
    segments = {}
    for name, (start_ms, end_ms) in SEGMENTS.items():
        segments[name] = run_replay(
            replay_bin=Path(opts["replay_bin"]),
            config=item["config"],
            budget=item["budget"],
            profile=item["profile"],
            start_ms=start_ms,
            end_ms=end_ms,
            market_data=Path(opts["market_data"]),
            funding_data=Path(opts["funding_data"]),
            exchange_min_notional=opts["exchange_min_notional"],
            timeout=opts["segment_timeout"],
            equity_curve_points=0,
        )
    summary = segment_summary(item["full"], segments)
    return {
        **item,
        "segments": segments,
        "segment_summary": summary,
        "segment_pass": segment_pass(item["profile"], item["full"], summary),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--csv", type=Path, default=Path("/tmp/codex_small_search/full_period_candidates.csv.gz"))
    parser.add_argument("--frontier", type=Path, default=Path("/tmp/codex_small_search/parallel_curve_frontier_sampled_v1.json"))
    parser.add_argument("--out-dir", type=Path, default=Path("/tmp/codex_curve_real_v1"))
    parser.add_argument("--profiles", nargs="+", default=["conservative", "balanced", "aggressive"])
    parser.add_argument("--budgets", nargs="+", type=float, default=[5000.0])
    parser.add_argument("--top-per-bucket", type=int, default=8)
    parser.add_argument("--jobs", type=int, default=20)
    parser.add_argument("--top-segment", type=int, default=24)
    parser.add_argument("--timeout", type=int, default=900)
    parser.add_argument("--segment-timeout", type=int, default=600)
    parser.add_argument("--exchange-min-notional", type=float, default=5.0)
    parser.add_argument("--equity-curve-points", type=int, default=0)
    parser.add_argument("--replay-bin", type=Path, required=True)
    parser.add_argument("--market-data", type=Path, required=True)
    parser.add_argument("--funding-data", type=Path, required=True)
    args = parser.parse_args()

    args.out_dir.mkdir(parents=True, exist_ok=True)
    report_path = args.out_dir / "report.json"
    started = time.time()
    print(f"LOAD candidates from {args.csv}", flush=True)
    candidates = load_candidates(args.csv)
    print(f"loaded_candidates={len(candidates)}", flush=True)
    jobs, rejected = load_frontier_jobs(args, candidates)
    print(f"generated_jobs={len(jobs)} rejected_configs={len(rejected)}", flush=True)

    opts = {
        "replay_bin": str(args.replay_bin),
        "market_data": str(args.market_data),
        "funding_data": str(args.funding_data),
        "exchange_min_notional": args.exchange_min_notional,
        "timeout": args.timeout,
        "segment_timeout": args.segment_timeout,
        "equity_curve_points": args.equity_curve_points,
    }
    report: dict[str, Any] = {
        "started_at": started,
        "args": {
            "profiles": args.profiles,
            "budgets": args.budgets,
            "top_per_bucket": args.top_per_bucket,
            "jobs": args.jobs,
        },
        "generated_jobs": len(jobs),
        "rejected_configs": rejected[:200],
        "full_results": [],
        "segment_results": [],
        "passes": [],
    }
    save_json(report_path, report)

    with futures.ProcessPoolExecutor(max_workers=args.jobs) as executor:
        futs = [executor.submit(full_worker, (job, opts)) for job in jobs]
        for done, fut in enumerate(futures.as_completed(futs), 1):
            item = fut.result()
            report["full_results"].append(item)
            full = item.get("full") or {}
            status = "OK" if full.get("ok") else "ERR"
            print(
                "DONE {}/{} {} {} ann={:.2f} dd={:.2f} ret={:.2f} cap={:.1f} blocked={} pass={}".format(
                    done,
                    len(futs),
                    status,
                    item["name"],
                    finite_float(full.get("ann"), -999),
                    finite_float(full.get("dd"), 999),
                    finite_float(full.get("ret"), 0),
                    finite_float(full.get("cap"), 0),
                    full.get("blocked"),
                    target_pass(item["profile"], full),
                ),
                flush=True,
            )
            if done % 5 == 0 or done == len(futs):
                save_json(report_path, report)

    full_results = [item for item in report["full_results"] if (item.get("full") or {}).get("ok")]
    selected: list[dict[str, Any]] = []
    seen_names: set[str] = set()
    for profile in args.profiles:
        rows = [item for item in full_results if item["profile"] == profile]
        rows.sort(key=lambda item: (target_pass(profile, item["full"]), full_score(profile, item["full"])), reverse=True)
        for item in rows[: args.top_segment]:
            if item["name"] not in seen_names:
                selected.append(item)
                seen_names.add(item["name"])

    print(f"segment_validation_selected={len(selected)}", flush=True)
    with futures.ProcessPoolExecutor(max_workers=min(args.jobs, max(1, len(selected)))) as executor:
        futs = [executor.submit(segment_worker, (item, opts)) for item in selected]
        for done, fut in enumerate(futures.as_completed(futs), 1):
            item = fut.result()
            report["segment_results"].append(item)
            summary = item.get("segment_summary") or {}
            full = item.get("full") or {}
            print(
                "SEG {}/{} {} ann={:.2f} dd={:.2f} pos={} maxsegdd={} h1={:.2f} pass={}".format(
                    done,
                    len(futs),
                    item["name"],
                    finite_float(full.get("ann"), -999),
                    finite_float(full.get("dd"), 999),
                    summary.get("positive_segments"),
                    summary.get("max_segment_dd"),
                    finite_float(summary.get("h1_contribution_ratio"), 999),
                    item.get("segment_pass"),
                ),
                flush=True,
            )
            if item.get("segment_pass"):
                report["passes"].append(item)
            save_json(report_path, report)

    report["finished_at"] = time.time()
    report["elapsed_seconds"] = report["finished_at"] - started
    save_json(report_path, report)
    print(f"wrote {report_path} passes={len(report['passes'])}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
