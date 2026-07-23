#!/usr/bin/env python3
"""Causal, return-blind stationarity activation scan for Round 25R."""

from __future__ import annotations

import argparse
import json
import sqlite3
import warnings
from datetime import datetime, timedelta, timezone
from pathlib import Path

import numpy as np
import scipy
import statsmodels
from statsmodels.tools.sm_exceptions import InterpolationWarning
from statsmodels.tsa.stattools import adfuller, coint, kpss


REFERENCE = "BTCUSDT"
ALTS = [
    "ETHUSDT",
    "BNBUSDT",
    "SOLUSDT",
    "XRPUSDT",
    "DOGEUSDT",
    "LINKUSDT",
]
BLOCK_STARTS = [
    "2023-07-01",
    "2023-10-01",
    "2024-01-01",
    "2024-04-01",
    "2024-07-01",
    "2024-10-01",
    "2025-01-01",
    "2025-04-01",
    "2025-07-01",
    "2025-10-01",
    "2026-01-01",
    "2026-04-01",
]


def utc_ms(value: str) -> int:
    return int(datetime.strptime(value, "%Y-%m-%d").replace(tzinfo=timezone.utc).timestamp() * 1000)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--database", default="data/market_data_full.db")
    parser.add_argument("--frequency", choices=("1h", "5m"), default="1h")
    parser.add_argument("--alts", default=",".join(ALTS))
    parser.add_argument("--lookbacks", default="14,21,30,45,60,90,120")
    parser.add_argument(
        "--fdr-q",
        type=float,
        default=0.0,
        help="Apply Benjamini-Hochberg correction to EG p-values within each roll/lookback.",
    )
    parser.add_argument("--roll-days", type=int, default=0)
    parser.add_argument("--scan-start", default="2023-07-01")
    parser.add_argument("--scan-end", default="2026-05-31")
    parser.add_argument(
        "--fit-cache-root",
        help="Audit the selected models in a Round 25R fit-cache instead of running a calendar scan.",
    )
    parser.add_argument(
        "--output",
        default=(
            "docs/superpowers/artifacts/glm-martingale-core-round25-corrected/"
            "audit/round25r-independent-stationarity-scan.json"
        ),
    )
    return parser.parse_args()


def load_closes(database: Path, symbols: list[str], interval_ms: int) -> dict[str, tuple[np.ndarray, np.ndarray]]:
    start_ms = utc_ms("2023-01-01")
    end_ms = utc_ms("2026-06-01")
    close_offset = interval_ms - 60_000
    connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True)
    output: dict[str, tuple[np.ndarray, np.ndarray]] = {}
    try:
        for symbol in symbols:
            rows = connection.execute(
                """
                SELECT close_time, close
                FROM klines INDEXED BY idx_klines_symbol_time
                WHERE symbol=? AND market_type='futures_usdt_perp' AND timeframe='1m'
                  AND open_time>=? AND open_time<? AND open_time % ? = ?
                ORDER BY open_time
                """,
                (symbol, start_ms, end_ms, interval_ms, close_offset),
            ).fetchall()
            output[symbol] = (
                np.asarray([row[0] for row in rows], dtype=np.int64),
                np.asarray([row[1] for row in rows], dtype=np.float64),
            )
    finally:
        connection.close()
    return output


def aligned_window(
    data: dict[str, tuple[np.ndarray, np.ndarray]],
    alt: str,
    fit_start_ms: int,
    fit_end_ms: int,
) -> tuple[np.ndarray, np.ndarray]:
    btc_times, btc_prices = data[REFERENCE]
    alt_times, alt_prices = data[alt]
    left = int(np.searchsorted(btc_times, fit_start_ms, side="left"))
    right = int(np.searchsorted(btc_times, fit_end_ms, side="right"))
    selected_times = btc_times[left:right]
    selected_btc = btc_prices[left:right]
    alt_left = int(np.searchsorted(alt_times, fit_start_ms, side="left"))
    alt_right = int(np.searchsorted(alt_times, fit_end_ms, side="right"))
    selected_alt_times = alt_times[alt_left:alt_right]
    selected_alt = alt_prices[alt_left:alt_right]
    if np.array_equal(selected_times, selected_alt_times):
        return selected_btc, selected_alt
    common, btc_indexes, alt_indexes = np.intersect1d(
        selected_times, selected_alt_times, assume_unique=True, return_indices=True
    )
    if common.size == 0:
        return np.asarray([]), np.asarray([])
    return selected_btc[btc_indexes], selected_alt[alt_indexes]


def diagnostics(btc: np.ndarray, alt: np.ndarray) -> dict[str, float | int | bool]:
    if btc.size < 200 or btc.size != alt.size or np.any(btc <= 0) or np.any(alt <= 0):
        return {"sample_count": int(btc.size), "passed": False, "reason": "insufficient_or_invalid"}
    y = np.log(btc)
    x = np.log(alt)
    design = np.column_stack((np.ones(x.size), x))
    intercept, beta = np.linalg.lstsq(design, y, rcond=None)[0]
    residual = y - intercept - beta * x
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", InterpolationWarning)
        _, coint_p, _ = coint(y, x, trend="c", autolag="aic")
        adf_stat, adf_p, *_ = adfuller(residual, regression="c", autolag="AIC")
        kpss_stat, kpss_p, _, _ = kpss(residual, regression="c", nlags="auto")
    lag = residual[:-1]
    delta = np.diff(residual)
    ar_design = np.column_stack((np.ones(lag.size), lag))
    gamma = float(np.linalg.lstsq(ar_design, delta, rcond=None)[0][1])
    phi = 1.0 + gamma
    half_life = float(-np.log(2.0) / np.log(phi)) if 0.0 < phi < 1.0 else float("inf")
    passed = bool(coint_p <= 0.05 and kpss_p >= 0.05 and np.isfinite(half_life))
    return {
        "sample_count": int(residual.size),
        "intercept": float(intercept),
        "beta": float(beta),
        "eg_coint_p_value": float(coint_p),
        "residual_adf_stat": float(adf_stat),
        "residual_adf_p_value": float(adf_p),
        "kpss_stat": float(kpss_stat),
        "kpss_p_value": float(kpss_p),
        "half_life_bars": half_life if np.isfinite(half_life) else None,
        "residual_sigma": float(np.std(residual, ddof=1)),
        "passed": passed,
    }


def benjamini_hochberg(p_values: list[float], q: float) -> list[bool]:
    accepted = [False] * len(p_values)
    if not p_values or q <= 0.0:
        return accepted
    ordered = sorted(enumerate(p_values), key=lambda item: item[1])
    cutoff_rank = 0
    for rank, (_, p_value) in enumerate(ordered, start=1):
        if p_value <= q * rank / len(ordered):
            cutoff_rank = rank
    for index, _ in ordered[:cutoff_rank]:
        accepted[index] = True
    return accepted


def audit_fit_cache(
    database: Path,
    fit_cache_root: Path,
    frequency: str,
    alts: list[str],
) -> dict[str, object]:
    interval_ms = 3_600_000 if frequency == "1h" else 300_000
    data = load_closes(database, [REFERENCE, *alts], interval_ms)
    cache_rows: list[dict[str, object]] = []
    selected_leg_count = 0
    independently_passing_leg_count = 0
    independently_passing_pair_count = 0

    for cache_path in sorted(fit_cache_root.glob("*.json")):
        payload = json.loads(cache_path.read_text(encoding="utf-8"))
        models = payload.get("models", [])
        if not models:
            continue
        model_rows = []
        for model in models:
            leg_rows = []
            for side in ("left", "right"):
                rust_leg = model[side]
                aligned_spreads = rust_leg["aligned_spreads"]
                fit_start_ms = int(aligned_spreads[0][0])
                fit_end_ms = int(aligned_spreads[-1][0])
                btc, alt = aligned_window(
                    data,
                    rust_leg["alt"],
                    fit_start_ms,
                    fit_end_ms,
                )
                independent = diagnostics(btc, alt)
                independent_pass = bool(independent["passed"])
                selected_leg_count += 1
                independently_passing_leg_count += int(independent_pass)
                rust_stationarity = rust_leg["stationarity"]
                leg_rows.append(
                    {
                        "side": side,
                        "symbol": rust_leg["alt"],
                        "fit_start_ms": fit_start_ms,
                        "fit_end_ms": fit_end_ms,
                        "rust": {
                            "adf_p_value": rust_stationarity["adf_p_value"],
                            "kpss_p_value": rust_stationarity["kpss_p_value"],
                            "half_life_bars": rust_stationarity["half_life_bars"],
                            "passed": rust_stationarity["passed"],
                        },
                        "independent": independent,
                        "decision_disagrees": bool(rust_stationarity["passed"]) != independent_pass,
                    }
                )
            pair_pass = all(bool(leg["independent"]["passed"]) for leg in leg_rows)
            independently_passing_pair_count += int(pair_pass)
            model_rows.append(
                {
                    "pair": f'{model["left"]["alt"]}-{model["right"]["alt"]}',
                    "independently_stationary": pair_pass,
                    "rust_independent_reference_error": model["copula"][
                        "independent_reference_error"
                    ],
                    "legs": leg_rows,
                }
            )
        cache_rows.append({"cache_file": cache_path.name, "models": model_rows})

    selected_pair_count = sum(len(row["models"]) for row in cache_rows)
    return {
        "schema_version": 1,
        "return_blind": True,
        "audit_scope": "selected Round 25R production fit-cache models",
        "database": str(database),
        "fit_cache_root": str(fit_cache_root),
        "frequency": frequency,
        "method": {
            "cointegration": "statsmodels.tsa.stattools.coint trend=c autolag=aic",
            "residual_adf": "statsmodels.tsa.stattools.adfuller regression=c autolag=AIC",
            "stationarity_confirmation": "statsmodels KPSS regression=c nlags=auto",
            "pass_rule": "EG p<=0.05 and KPSS p>=0.05 and finite AR(1) half-life",
            "statsmodels_version": statsmodels.__version__,
            "scipy_version": scipy.__version__,
        },
        "summary": {
            "selected_cache_count": len(cache_rows),
            "selected_pair_count": selected_pair_count,
            "independently_passing_pair_count": independently_passing_pair_count,
            "selected_leg_count": selected_leg_count,
            "independently_passing_leg_count": independently_passing_leg_count,
            "independently_failing_leg_count": selected_leg_count
            - independently_passing_leg_count,
        },
        "caches": cache_rows,
    }


def main() -> None:
    args = parse_args()
    alts = [value.strip() for value in args.alts.split(",") if value.strip()]
    if args.fit_cache_root:
        output = audit_fit_cache(
            Path(args.database),
            Path(args.fit_cache_root),
            args.frequency,
            alts,
        )
        output_path = Path(args.output)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(json.dumps(output, indent=2, sort_keys=True), encoding="utf-8")
        print(json.dumps(output["summary"], indent=2))
        return

    lookbacks = [int(value) for value in args.lookbacks.split(",")]
    interval_ms = 3_600_000 if args.frequency == "1h" else 300_000
    data = load_closes(Path(args.database), [REFERENCE, *alts], interval_ms)
    if args.roll_days > 0:
        cursor = datetime.strptime(args.scan_start, "%Y-%m-%d")
        scan_end = datetime.strptime(args.scan_end, "%Y-%m-%d")
        scan_points = []
        while cursor <= scan_end:
            scan_points.append(cursor.strftime("%Y-%m-%d"))
            cursor += timedelta(days=args.roll_days)
    else:
        scan_points = BLOCK_STARTS
    rows = []
    for scan_start in scan_points:
        fit_end_ms = utc_ms(scan_start) - 1
        for lookback_days in lookbacks:
            fit_start_ms = fit_end_ms - lookback_days * 86_400_000 + 1
            alt_rows = []
            for alt in alts:
                btc, alt_prices = aligned_window(data, alt, fit_start_ms, fit_end_ms)
                alt_rows.append({"symbol": alt, **diagnostics(btc, alt_prices)})
            if args.fdr_q > 0.0:
                eg_fdr_passes = benjamini_hochberg(
                    [float(row.get("eg_coint_p_value", 1.0)) for row in alt_rows],
                    args.fdr_q,
                )
                for row, eg_fdr_passed in zip(alt_rows, eg_fdr_passes, strict=True):
                    row["raw_passed"] = row["passed"]
                    row["eg_fdr_passed"] = eg_fdr_passed
                    row["passed"] = bool(
                        eg_fdr_passed
                        and float(row.get("kpss_p_value", 0.0)) >= 0.05
                        and row.get("half_life_bars") is not None
                    )
            passed = [row["symbol"] for row in alt_rows if row["passed"]]
            rows.append(
                {
                    "scan_start": scan_start,
                    "lookback_days": lookback_days,
                    "passed_alts": passed,
                    "passed_alt_count": len(passed),
                    "maximum_possible_disjoint_pairs": len(passed) // 2,
                    "alts": alt_rows,
                }
            )
    summaries = []
    for lookback_days in lookbacks:
        selected = [row for row in rows if row["lookback_days"] == lookback_days]
        summaries.append(
            {
                "lookback_days": lookback_days,
                "total_passed_alt_blocks": sum(row["passed_alt_count"] for row in selected),
                "rolls_with_at_least_2_alts": sum(row["passed_alt_count"] >= 2 for row in selected),
                "rolls_with_at_least_4_alts": sum(row["passed_alt_count"] >= 4 for row in selected),
                "rolls_with_at_least_6_alts": sum(
                    row["passed_alt_count"] >= 6 for row in selected
                ),
                "max_passed_alts_in_block": max(row["passed_alt_count"] for row in selected),
                "distinct_passed_alts": sorted(
                    {symbol for row in selected for symbol in row["passed_alts"]}
                ),
            }
        )
    output = {
        "schema_version": 1,
        "return_blind": True,
        "database": str(Path(args.database)),
        "frequency": args.frequency,
        "reference": REFERENCE,
        "alts": alts,
        "roll_days": args.roll_days,
        "roll_count": len(scan_points),
        "lookbacks": lookbacks,
        "method": {
            "cointegration": "statsmodels.tsa.stattools.coint trend=c autolag=aic",
            "stationarity_confirmation": "statsmodels KPSS regression=c nlags=auto",
            "pass_rule": "EG p<=0.05 and KPSS p>=0.05 and finite AR(1) half-life",
            "eg_multiple_testing": (
                f"Benjamini-Hochberg q={args.fdr_q}" if args.fdr_q > 0.0 else "none"
            ),
            "statsmodels_version": statsmodels.__version__,
            "scipy_version": scipy.__version__,
        },
        "summaries": summaries,
        "rows": rows,
    }
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(output, indent=2, sort_keys=True), encoding="utf-8")
    print(json.dumps(summaries, indent=2))


if __name__ == "__main__":
    main()
