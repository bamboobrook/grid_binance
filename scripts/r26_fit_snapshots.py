#!/usr/bin/env python3
"""Round 26 train-only weekly reference-spread and Copula snapshots."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import sqlite3
import sys
import time
import warnings
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from itertools import combinations
from multiprocessing import get_context
from pathlib import Path
from typing import Any

import numpy as np
import scipy
import statsmodels
from scipy.optimize import minimize_scalar
from scipy.stats import kendalltau, multivariate_normal, multivariate_t, norm, rankdata, t
from statsmodels.stats.diagnostic import breaks_cusumolsresid
from statsmodels.tools.sm_exceptions import InterpolationWarning
from statsmodels.tsa.stattools import adfuller, coint, kpss


FINGERPRINT = "R26_WEEKLY_TOP20_REFERENCE_SPREAD_COPULA_MARTIN"
REFERENCE = "BTCUSDT"
ALTS = [
    "AAVEUSDT", "ADAUSDT", "ALGOUSDT", "APTUSDT", "ATOMUSDT", "AVAXUSDT",
    "BCHUSDT", "BNBUSDT", "COMPUSDT", "CRVUSDT", "DASHUSDT", "DOGEUSDT",
    "DOTUSDT", "DYDXUSDT", "EGLDUSDT", "ETCUSDT", "ETHUSDT", "FILUSDT",
    "GALAUSDT", "HBARUSDT", "ICPUSDT", "INJUSDT", "LINKUSDT", "NEARUSDT",
    "SOLUSDT", "TRXUSDT", "UNIUSDT", "XRPUSDT", "ZECUSDT",
]
SYMBOLS = [REFERENCE, *ALTS]
START_MS = 1_672_531_200_000
END_MS = 1_780_272_000_000
MINUTE_MS = 60_000
DAY_MS = 86_400_000
WEEK_MS = 7 * DAY_MS
EXPECTED_MINUTES = 1_795_680
MARKET_DB_BYTES = 118_879_649_792
MARKET_DB_SHA256 = "4ceed5eb0bcfdea701a6e156d570cfb33c2ea7274413abee5c93116fc28afec4"
EXCHANGE_INFO = Path(
    "docs/superpowers/artifacts/glm-martingale-core-round25-corrected/audit/"
    "perp-exchangeInfo-30-symbols-2026-07-23.json"
)
REPO_ARTIFACT = Path("docs/superpowers/artifacts/glm-martingale-core-round26")
G0_WINDOWS = {
    "range": ("2023-08-01", "2023-08-14"),
    "bull": ("2024-02-15", "2024-02-29"),
    "shock": ("2024-08-01", "2024-08-15"),
    "bear": ("2025-02-01", "2025-02-15"),
}


@dataclass(frozen=True)
class Filter:
    tick_size: float
    step_size: float
    min_qty: float
    min_notional: float


SIGNALS: dict[str, dict[str, tuple[np.ndarray, np.ndarray]]] = {}
LIQUIDITY: dict[tuple[str, int, int], dict[str, Any]] = {}
FUNDING: dict[str, tuple[np.ndarray, np.ndarray]] = {}
FILTERS: dict[str, Filter] = {}
OUTPUT_ROOT = Path(".")
RESUME = False
VALIDATOR_CACHE_ALGORITHM = "r26-independent-statsmodels-leg-v1"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=("g0", "g1", "validate"), required=True)
    parser.add_argument("--frequency", default="1h")
    parser.add_argument("--formation-days", default="14,21")
    parser.add_argument("--artifact-root", required=True)
    parser.add_argument("--market-data", default="data/market_data_full.db")
    parser.add_argument("--funding-data", default="data/funding_rates.db")
    parser.add_argument("--workers", type=int, default=min(8, os.cpu_count() or 1))
    parser.add_argument("--resume", action="store_true")
    return parser.parse_args()


def utc_ms(value: str) -> int:
    return int(datetime.strptime(value, "%Y-%m-%d").replace(tzinfo=timezone.utc).timestamp() * 1000)


def iso(ms: int) -> str:
    return datetime.fromtimestamp(ms / 1000, timezone.utc).isoformat().replace("+00:00", "Z")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(8 * 1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def sanitize(value: Any) -> Any:
    if isinstance(value, dict):
        return {key: sanitize(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [sanitize(item) for item in value]
    if isinstance(value, (np.integer,)):
        return int(value)
    if isinstance(value, (np.floating, float)):
        number = float(value)
        return number if math.isfinite(number) else None
    if isinstance(value, (np.bool_,)):
        return bool(value)
    return value


def atomic_json(path: Path, value: Any) -> str:
    clean = sanitize(value)
    payload = json.dumps(clean, indent=2, sort_keys=True, allow_nan=False).encode() + b"\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_bytes(payload)
    temporary.replace(path)
    return hashlib.sha256(payload).hexdigest()


def anchors() -> list[int]:
    cursor = utc_ms("2023-07-01")
    last = utc_ms("2026-05-30")
    output = []
    # The plan's explicit 152/152 gate makes the textual 2026-05-30 bound exclusive.
    while cursor < last:
        output.append(cursor)
        cursor += WEEK_MS
    if len(output) != 152:
        raise RuntimeError(f"weekly anchor contract mismatch: {len(output)}")
    return output


def phase_anchors(phase: str) -> list[int]:
    if phase == "g1":
        return anchors()
    selected = []
    for anchor in anchors():
        for start, end in G0_WINDOWS.values():
            if anchor < utc_ms(end) and anchor + WEEK_MS > utc_ms(start):
                selected.append(anchor)
                break
    return selected


def read_exchange_info(path: Path) -> tuple[dict[str, Filter], dict[str, Any]]:
    payload = json.loads(path.read_text())
    rows: dict[str, Filter] = {}
    compact = []
    for symbol in payload["symbols"]:
        filters = {row["filterType"]: row for row in symbol["filters"]}
        price = filters["PRICE_FILTER"]
        lot = filters["LOT_SIZE"]
        notional = filters["MIN_NOTIONAL"]
        row = Filter(
            tick_size=float(price["tickSize"]),
            step_size=float(lot["stepSize"]),
            min_qty=float(lot["minQty"]),
            min_notional=float(notional.get("notional", notional.get("minNotional"))),
        )
        rows[symbol["symbol"]] = row
        compact.append({"symbol": symbol["symbol"], **row.__dict__})
    symbols = sorted(rows)
    return rows, {
        "path": str(path),
        "file_sha256": sha256_file(path),
        "source_sha256": payload["source_sha256"],
        "serverTime": payload["serverTime"],
        "source_url": payload["source_url"],
        "symbol_count": len(rows),
        "symbols": symbols,
        "filters": sorted(compact, key=lambda row: row["symbol"]),
        "exact_symbol_set": symbols == sorted(SYMBOLS),
    }


def sqlite_flat(connection: sqlite3.Connection, query: str, parameters: tuple[Any, ...], width: int) -> np.ndarray:
    cursor = connection.execute(query, parameters)
    flat = np.fromiter((item for row in cursor for item in row), dtype=np.float64)
    if flat.size % width:
        raise RuntimeError("sqlite row width mismatch")
    return flat.reshape((-1, width))


def load_funding(path: Path) -> tuple[dict[str, tuple[np.ndarray, np.ndarray]], dict[str, Any]]:
    connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    output = {}
    try:
        for symbol in SYMBOLS:
            rows = connection.execute(
                "SELECT funding_time,funding_rate FROM funding_rates "
                "WHERE symbol=? AND funding_time>=? AND funding_time<? ORDER BY funding_time",
                (symbol, START_MS, END_MS),
            ).fetchall()
            output[symbol] = (
                np.asarray([row[0] for row in rows], dtype=np.int64),
                np.asarray([row[1] for row in rows], dtype=np.float64),
            )
    finally:
        connection.close()
    expected = output[REFERENCE][0]
    rows = []
    for symbol in SYMBOLS:
        times = output[symbol][0]
        rows.append({
            "symbol": symbol,
            "expected": int(expected.size),
            "actual": int(times.size),
            "missing": int(np.setdiff1d(expected, times, assume_unique=True).size),
            "duplicate": int(times.size - np.unique(times).size),
            "first_ms": int(times[0]) if times.size else None,
            "last_ms": int(times[-1]) if times.size else None,
            "complete": bool(np.array_equal(times, expected)),
        })
    return output, {
        "path": str(path),
        "bytes": path.stat().st_size,
        "sha256": sha256_file(path),
        "canonical_schedule": "BTCUSDT observed timestamps in fixed D0 interval",
        "symbols": rows,
    }


def load_market(
    path: Path,
    frequencies: list[str],
    roll_anchors: list[int],
    formations: list[int],
) -> tuple[dict[str, dict[str, tuple[np.ndarray, np.ndarray]]], dict[tuple[str, int, int], dict[str, Any]], dict[str, Any]]:
    connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    signals = {frequency: {} for frequency in frequencies}
    liquidity = {}
    d0_rows = []
    try:
        for position, symbol in enumerate(SYMBOLS, start=1):
            minute = sqlite_flat(
                connection,
                "SELECT open_time,close_time,close,volume FROM klines INDEXED BY idx_klines_symbol_time "
                "WHERE symbol=? AND market_type='futures_usdt_perp' AND timeframe='1m' "
                "AND open_time>=? AND open_time<? ORDER BY open_time",
                (symbol, START_MS, END_MS),
                4,
            )
            times = minute[:, 0].astype(np.int64)
            close_times = minute[:, 1].astype(np.int64)
            closes = minute[:, 2]
            volumes = minute[:, 3]
            differences = np.diff(times)
            missing = int(np.maximum(differences // MINUTE_MS - 1, 0).sum()) if times.size else EXPECTED_MINUTES
            duplicates = int(np.count_nonzero(differences == 0))
            close_contract_errors = int(np.count_nonzero(close_times - times != MINUTE_MS - 1))
            complete = bool(
                times.size == EXPECTED_MINUTES
                and times[0] == START_MS
                and times[-1] == END_MS - MINUTE_MS
                and missing == 0
                and duplicates == 0
                and close_contract_errors == 0
            )
            d0_rows.append({
                "symbol": symbol,
                "row_count": int(times.size),
                "first_ms": int(times[0]) if times.size else None,
                "last_ms": int(times[-1]) if times.size else None,
                "missing_minutes": missing,
                "duplicate_minutes": duplicates,
                "close_time_contract_errors": close_contract_errors,
                "complete": complete,
            })
            quote_volume = closes * volumes
            for anchor in roll_anchors:
                right = int(np.searchsorted(times, anchor, side="left"))
                for formation in formations:
                    fit_start = anchor - formation * DAY_MS
                    left = int(np.searchsorted(times, fit_start, side="left"))
                    selected = quote_volume[left:right]
                    selected_times = times[left:right]
                    expected = formation * 1_440
                    if selected.size:
                        day_index = (selected_times - fit_start) // DAY_MS
                        daily = np.bincount(day_index, weights=selected, minlength=formation)
                        row = {
                            "symbol": symbol,
                            "fit_start_ms": fit_start,
                            "fit_cutoff_ms": anchor - 1,
                            "sample_count": int(selected.size),
                            "coverage": float(selected.size / expected),
                            "zero_volume_ratio": float(np.count_nonzero(volumes[left:right] == 0) / selected.size),
                            "p10_minute_quote_volume": float(np.quantile(selected, 0.10)),
                            "median_daily_quote_volume": float(np.median(daily)),
                        }
                    else:
                        row = {
                            "symbol": symbol, "fit_start_ms": fit_start,
                            "fit_cutoff_ms": anchor - 1, "sample_count": 0, "coverage": 0.0,
                            "zero_volume_ratio": 1.0, "p10_minute_quote_volume": 0.0,
                            "median_daily_quote_volume": 0.0,
                        }
                    liquidity[(symbol, anchor, formation)] = row
            for frequency in frequencies:
                interval = 3_600_000 if frequency == "1h" else 300_000
                selected = times % interval == interval - MINUTE_MS
                signals[frequency][symbol] = (close_times[selected], closes[selected])
            print(f"market-load {position:02d}/{len(SYMBOLS)} {symbol}", flush=True)
    finally:
        connection.close()
    return signals, liquidity, {
        "path": str(path),
        "bytes": path.stat().st_size,
        "sha256": MARKET_DB_SHA256,
        "sha256_evidence": "independent sha256sum before immutable replay source commit",
        "sha256_bound_to_bytes": path.stat().st_size == MARKET_DB_BYTES,
        "symbols": d0_rows,
    }


def top20(anchor: int, formation: int) -> tuple[list[str], list[dict[str, Any]]]:
    rows = [LIQUIDITY[(symbol, anchor, formation)] for symbol in ALTS]
    eligible = [
        row for row in rows
        if row["coverage"] == 1.0
        and row["zero_volume_ratio"] <= 0.01
        and row["p10_minute_quote_volume"] >= 10_000.0
        and row["fit_cutoff_ms"] < anchor
    ]
    eligible.sort(key=lambda row: (-row["median_daily_quote_volume"], row["symbol"]))
    return [row["symbol"] for row in eligible[:20]], rows


def aligned_prices(frequency: str, symbol: str, start: int, end: int) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    btc_times, btc = SIGNALS[frequency][REFERENCE]
    alt_times, alt = SIGNALS[frequency][symbol]
    left = int(np.searchsorted(btc_times, start, side="left"))
    right = int(np.searchsorted(btc_times, end, side="left"))
    times = btc_times[left:right]
    btc_values = btc[left:right]
    alt_left = int(np.searchsorted(alt_times, start, side="left"))
    alt_right = int(np.searchsorted(alt_times, end, side="left"))
    alt_selected_times = alt_times[alt_left:alt_right]
    alt_values = alt[alt_left:alt_right]
    if np.array_equal(times, alt_selected_times):
        return times, btc_values, alt_values
    common, btc_indexes, alt_indexes = np.intersect1d(
        times, alt_selected_times, assume_unique=True, return_indices=True
    )
    return common, btc_values[btc_indexes], alt_values[alt_indexes]


def crossing_excursions(uniforms: np.ndarray, prices: np.ndarray, max_bars: int) -> tuple[int, int, list[tuple[int, int | None, int]]]:
    tails = np.where(uniforms <= 0.20, -1, np.where(uniforms >= 0.80, 1, 0))
    neutral = (uniforms >= 0.35) & (uniforms <= 0.65)
    armed = True
    excursions = []
    neutral_returns = 0
    for index, tail in enumerate(tails):
        if armed and tail:
            end = None
            limit = min(uniforms.size, index + max_bars + 1)
            candidates = np.flatnonzero(neutral[index + 1:limit])
            if candidates.size:
                end = index + 1 + int(candidates[0])
                neutral_returns += 1
            excursions.append((index, end, int(tail)))
            armed = False
        elif not armed and neutral[index]:
            armed = True
    return len(excursions), neutral_returns, excursions


def diagnostics(frequency: str, symbol: str, anchor: int, formation: int) -> tuple[dict[str, Any], dict[str, Any]]:
    start = anchor - formation * DAY_MS
    times, btc, alt = aligned_prices(frequency, symbol, start, anchor)
    interval = 3_600_000 if frequency == "1h" else 300_000
    expected = formation * DAY_MS // interval
    base = {
        "symbol": symbol, "fit_start_ms": start, "fit_end_ms": anchor - 1,
        "fit_cutoff_ms": anchor, "sample_count": int(times.size),
        "sample_coverage": float(times.size / expected),
    }
    if times.size != expected or np.any(btc <= 0) or np.any(alt <= 0):
        return {**base, "common_gate_passed": False, "reason": "incomplete_or_invalid_prices"}, {}
    y = np.log(btc)
    x = np.log(alt)
    design = np.column_stack((np.ones(x.size), x))
    intercept, beta = np.linalg.lstsq(design, y, rcond=None)[0]
    residual = y - intercept - beta * x
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", InterpolationWarning)
        warnings.simplefilter("ignore", RuntimeWarning)
        eg_stat, eg_p, eg_critical = coint(y, x, trend="c", autolag="aic")
        adf_result = adfuller(residual, regression="c", autolag="AIC")
        kpss_result = kpss(residual, regression="c", nlags="auto")
        cusum_stat, cusum_p, _ = breaks_cusumolsresid(residual, ddof=2)
    lag = residual[:-1]
    delta = np.diff(residual)
    gamma = float(np.linalg.lstsq(np.column_stack((np.ones(lag.size), lag)), delta, rcond=None)[0][1])
    phi = 1.0 + gamma
    half_life = float(-np.log(2) / np.log(phi)) if 0 < phi < 1 else math.inf
    midpoint = x.size // 2
    beta_first = float(np.linalg.lstsq(np.column_stack((np.ones(midpoint), x[:midpoint])), y[:midpoint], rcond=None)[0][1])
    beta_second = float(np.linalg.lstsq(np.column_stack((np.ones(x.size - midpoint), x[midpoint:])), y[midpoint:], rcond=None)[0][1])
    beta_drift = abs(beta_second - beta_first) / max(abs(beta_first), 1e-12)
    uniforms = rankdata(residual, method="average") / (residual.size + 1.0)
    max_bars = WEEK_MS // interval
    tail_count, neutral_returns, excursions = crossing_excursions(uniforms, alt, max_bars)
    common = bool(
        math.isfinite(half_life) and 2 <= half_life <= max_bars
        and float(kpss_result[1]) >= 0.05 and float(cusum_p) >= 0.05
        and beta_drift <= 0.25 and tail_count >= 20
    )
    data_hash = hashlib.sha256(
        np.column_stack((times, btc, alt)).astype("<f8", copy=False).tobytes()
    ).hexdigest()
    public = {
        **base,
        "intercept": float(intercept), "beta": float(beta),
        "residual_sigma": float(np.std(residual, ddof=1)),
        "eg_coint_statistic": float(eg_stat), "eg_coint_p_value": float(eg_p),
        "eg_critical_values": [float(value) for value in eg_critical], "eg_autolag": "aic",
        "residual_adf_statistic": float(adf_result[0]), "residual_adf_p_value": float(adf_result[1]),
        "residual_adf_lags": int(adf_result[2]),
        "kpss_statistic": float(kpss_result[0]), "kpss_p_value": float(kpss_result[1]),
        "kpss_lags": int(kpss_result[2]), "ar1_phi": phi, "half_life_bars": half_life,
        "cusum_statistic": float(cusum_stat), "cusum_p_value": float(cusum_p),
        "first_half_beta": beta_first, "second_half_beta": beta_second,
        "absolute_beta_drift_ratio": beta_drift,
        "actual_tail_crossings_alpha_0p20": tail_count,
        "neutral_returns": neutral_returns,
        "censored_failures": tail_count - neutral_returns,
        "common_gate_passed": common, "data_hash": data_hash,
        "sorted_residuals": np.sort(residual).tolist(),
    }
    private = {
        "times": times, "btc": btc, "alt": alt, "residual": residual,
        "uniforms": uniforms, "excursions": excursions,
    }
    return public, private


def bh(p_values: list[float], q: float = 0.05) -> list[bool]:
    ordered = sorted(enumerate(p_values), key=lambda item: (item[1], item[0]))
    cutoff = 0
    for rank, (_, value) in enumerate(ordered, start=1):
        if value <= q * rank / len(ordered):
            cutoff = rank
    accepted = [False] * len(p_values)
    for index, _ in ordered[:cutoff]:
        accepted[index] = True
    return accepted


def copula_log_likelihood(uniforms: np.ndarray, family: str, rho: float, nu: int | None) -> float:
    clipped = np.clip(uniforms, 1e-8, 1 - 1e-8)
    if family == "gaussian":
        transformed = norm.ppf(clipped)
        joint = multivariate_normal.logpdf(transformed, mean=[0, 0], cov=[[1, rho], [rho, 1]])
        marginal = norm.logpdf(transformed).sum(axis=1)
    else:
        transformed = t.ppf(clipped, df=nu)
        joint = multivariate_t.logpdf(transformed, shape=[[1, rho], [rho, 1]], df=nu)
        marginal = t.logpdf(transformed, df=nu).sum(axis=1)
    return float(np.sum(joint - marginal))


def conditional_h(uniforms: np.ndarray, family: str, rho: float, nu: int | None) -> np.ndarray:
    clipped = np.clip(uniforms, 1e-8, 1 - 1e-8)
    if family == "gaussian":
        transformed = norm.ppf(clipped)
        scale = math.sqrt(max(1 - rho * rho, 1e-12))
        left = norm.cdf((transformed[:, 0] - rho * transformed[:, 1]) / scale)
        right = norm.cdf((transformed[:, 1] - rho * transformed[:, 0]) / scale)
    else:
        transformed = t.ppf(clipped, df=nu)
        left_scale = np.sqrt((nu + transformed[:, 1] ** 2) * (1 - rho * rho) / (nu + 1))
        right_scale = np.sqrt((nu + transformed[:, 0] ** 2) * (1 - rho * rho) / (nu + 1))
        left = t.cdf((transformed[:, 0] - rho * transformed[:, 1]) / left_scale, df=nu + 1)
        right = t.cdf((transformed[:, 1] - rho * transformed[:, 0]) / right_scale, df=nu + 1)
    return np.column_stack((left, right))


def fit_copula(left: dict[str, Any], right: dict[str, Any], cutoff: int) -> tuple[dict[str, Any], np.ndarray]:
    observations = np.column_stack((left["uniforms"], right["uniforms"]))
    tau = float(kendalltau(observations[:, 0], observations[:, 1]).statistic)
    fits = []
    for family, nu in [("gaussian", None), *[("student_t", value) for value in range(3, 31)]]:
        result = minimize_scalar(
            lambda rho: -copula_log_likelihood(observations, family, float(rho), nu),
            bounds=(-0.95, 0.95), method="bounded", options={"xatol": 1e-8},
        )
        parameters = 1 if family == "gaussian" else 2
        fits.append({
            "family": family, "rho": float(result.x), "nu": nu,
            "log_likelihood": float(-result.fun), "aic": float(2 * parameters + 2 * result.fun),
        })
    fits.sort(key=lambda row: (row["aic"], row["family"], row["nu"] or 0))
    best = fits[0]
    h_values = conditional_h(observations, best["family"], best["rho"], best["nu"])
    indexes = sorted(set([0, observations.shape[0] // 2, observations.shape[0] - 1]))
    canaries = [
        {"index": index, "u": observations[index].tolist(), "h": h_values[index].tolist()}
        for index in indexes
    ]
    return {
        **best, "sample_count": int(observations.shape[0]), "kendall_tau": tau,
        "fit_cutoff_ms": cutoff, "candidates": fits, "python_reference_canaries": canaries,
    }, h_values


def resolved_quantity(symbol: str, price: float, target: float = 100.0) -> tuple[float, float]:
    rule = FILTERS[symbol]
    required = max(target, rule.min_notional)
    quantity = max(rule.min_qty, math.ceil((required / price) / rule.step_size - 1e-12) * rule.step_size)
    return quantity, quantity * price


def funding_p95(symbol: str, start: int, end: int) -> float:
    times, rates = FUNDING[symbol]
    left = int(np.searchsorted(times, start, side="left"))
    right = int(np.searchsorted(times, end, side="left"))
    selected = np.abs(rates[left:right])
    return float(np.quantile(selected, 0.95)) if selected.size else math.inf


def pair_cost(
    frequency: str,
    anchor: int,
    formation: int,
    left_symbol: str,
    right_symbol: str,
    left: dict[str, Any],
    right: dict[str, Any],
    h_values: np.ndarray,
) -> dict[str, Any]:
    interval = 3_600_000 if frequency == "1h" else 300_000
    max_bars = WEEK_MS // interval
    divergence = np.where(
        (h_values[:, 0] <= 0.20) & (h_values[:, 1] >= 0.80), -1,
        np.where((h_values[:, 0] >= 0.80) & (h_values[:, 1] <= 0.20), 1, 0),
    )
    neutral = (
        (h_values[:, 0] >= 0.35) & (h_values[:, 0] <= 0.65)
        & (h_values[:, 1] >= 0.35) & (h_values[:, 1] <= 0.65)
    )
    armed = True
    rows = []
    for index, direction in enumerate(divergence):
        if armed and direction:
            limit = min(divergence.size, index + max_bars + 1)
            candidates = np.flatnonzero(neutral[index + 1:limit])
            exit_index = index + 1 + int(candidates[0]) if candidates.size else None
            left_qty, left_gross = resolved_quantity(left_symbol, float(left["alt"][index]))
            right_qty, right_gross = resolved_quantity(right_symbol, float(right["alt"][index]))
            mismatch = abs(left_gross - right_gross) / max(left_gross, right_gross)
            total_gross = left_gross + right_gross
            funding = (
                funding_p95(left_symbol, anchor - formation * DAY_MS, anchor)
                + funding_p95(right_symbol, anchor - formation * DAY_MS, anchor)
            ) * max(left_gross, right_gross) * 3
            fee_slippage = total_gross * 2 * (4 + 2) / 10_000
            legging = total_gross * 2 / 10_000
            cost = fee_slippage + funding + legging
            if exit_index is None:
                gross_edge = 0.0
                all_in = -cost
            else:
                left_return = float(left["alt"][exit_index] / left["alt"][index] - 1)
                right_return = float(right["alt"][exit_index] / right["alt"][index] - 1)
                gross_edge = direction * left_gross * left_return - direction * right_gross * right_return
                all_in = gross_edge - cost
            rows.append({
                "entry_index": index, "exit_index": exit_index, "direction": int(direction),
                "left_quantity": left_qty, "right_quantity": right_qty,
                "left_gross": left_gross, "right_gross": right_gross,
                "gross_mismatch_ratio": mismatch, "gross_edge_quote": gross_edge,
                "round_trip_cost_quote": cost, "all_in_edge_quote": all_in,
                "censored": exit_index is None,
            })
            armed = False
        elif not armed and neutral[index]:
            armed = True
    all_in = np.asarray([row["all_in_edge_quote"] for row in rows])
    costs = np.asarray([row["round_trip_cost_quote"] for row in rows])
    mismatches = np.asarray([row["gross_mismatch_ratio"] for row in rows])
    median_edge = float(np.median(all_in)) if all_in.size else -math.inf
    median_cost = float(np.median(costs)) if costs.size else math.inf
    p25 = float(np.quantile(all_in, 0.25)) if all_in.size else -math.inf
    ratio = median_edge / median_cost if median_cost > 0 and math.isfinite(median_cost) else -math.inf
    mismatch = float(np.max(mismatches)) if mismatches.size else math.inf
    return {
        "excursion_count": len(rows),
        "completed_count": sum(row["exit_index"] is not None for row in rows),
        "censored_count": sum(row["censored"] for row in rows),
        "median_all_in_edge_quote": median_edge, "p25_all_in_edge_quote": p25,
        "median_round_trip_cost_quote": median_cost, "median_edge_cost_ratio": ratio,
        "max_resolved_gross_mismatch_ratio": mismatch,
        "cost_inputs": {
            "fee_bps_each_way": 4.0, "slippage_bps_each_way": 2.0,
            "legging_allowance_bps": 2.0, "formation_funding_p95_events": 3,
            "filter_rounding": True,
        },
        "passed": bool(len(rows) >= 20 and ratio >= 2 and p25 > 0 and mismatch <= 0.05),
        "excursions": rows,
    }


def pair_score(left: dict[str, Any], right: dict[str, Any], copula: dict[str, Any], cost: dict[str, Any], liquidity: dict[str, dict[str, Any]]) -> float:
    stationarity = -math.log10(max(left["eg_coint_p_value"], 1e-12)) - math.log10(max(right["eg_coint_p_value"], 1e-12))
    speed = 1 / max(left["half_life_bars"] + right["half_life_bars"], 1)
    breaks = left["cusum_p_value"] + right["cusum_p_value"]
    tails = math.log1p(cost["excursion_count"])
    cost_ratio = max(min(cost["median_edge_cost_ratio"], 20), -20)
    aic = -copula["aic"] / max(copula["sample_count"], 1)
    liquid = math.log1p(min(liquidity[left["symbol"]]["median_daily_quote_volume"], liquidity[right["symbol"]]["median_daily_quote_volume"])) / 100
    return float(stationarity + speed + breaks + tails + cost_ratio + aic + liquid)


def exact_matching(edges: list[dict[str, Any]], max_pairs: int = 3) -> list[dict[str, Any]]:
    edges = sorted(edges, key=lambda row: row["pair_id"])
    best: list[dict[str, Any]] = []

    def better(candidate: list[dict[str, Any]], incumbent: list[dict[str, Any]]) -> bool:
        if len(candidate) != len(incumbent):
            return len(candidate) > len(incumbent)
        left = sum(row["score"] for row in candidate)
        right = sum(row["score"] for row in incumbent)
        if abs(left - right) > 1e-12:
            return left > right
        return [row["pair_id"] for row in candidate] < [row["pair_id"] for row in incumbent]

    def visit(start: int, used: set[str], selected: list[dict[str, Any]]) -> None:
        nonlocal best
        if better(selected, best):
            best = list(selected)
        if len(selected) == max_pairs:
            return
        for index in range(start, len(edges)):
            edge = edges[index]
            if edge["left"] in used or edge["right"] in used:
                continue
            visit(index + 1, used | {edge["left"], edge["right"]}, [*selected, edge])

    visit(0, set(), [])
    return best


def build_snapshot(task: tuple[str, int, int]) -> dict[str, Any]:
    frequency, formation, anchor = task
    name = f"{frequency}-{formation}d-{datetime.fromtimestamp(anchor / 1000, timezone.utc):%Y%m%d}"
    path = OUTPUT_ROOT / "fit-snapshots" / frequency / f"{formation}d" / f"{name}.json"
    if RESUME and path.exists():
        return {"name": name, "path": str(path), "sha256": sha256_file(path), "resumed": True}
    universe, liquidity_rows = top20(anchor, formation)
    liquidity_by_symbol = {row["symbol"]: row for row in liquidity_rows}
    public_legs = []
    private_legs = {}
    for symbol in universe:
        public, private = diagnostics(frequency, symbol, anchor, formation)
        public_legs.append(public)
        private_legs[symbol] = private
    fdr = bh([float(row.get("eg_coint_p_value", 1.0)) for row in public_legs])
    for row, accepted in zip(public_legs, fdr, strict=True):
        row["raw_passed"] = bool(row.get("common_gate_passed") and row.get("eg_coint_p_value", 1) <= 0.05)
        row["fdr_passed"] = bool(row.get("common_gate_passed") and accepted)
    arms = {}
    for arm in ("raw", "fdr"):
        selected = [row for row in public_legs if row[f"{arm}_passed"]]
        edges = []
        for left_index in range(len(selected)):
            for right_index in range(left_index + 1, len(selected)):
                left_public = selected[left_index]
                right_public = selected[right_index]
                left_symbol, right_symbol = sorted([left_public["symbol"], right_public["symbol"]])
                left_private = private_legs[left_symbol]
                right_private = private_legs[right_symbol]
                copula, h_values = fit_copula(left_private, right_private, anchor)
                cost = pair_cost(
                    frequency, anchor, formation, left_symbol, right_symbol,
                    left_private, right_private, h_values,
                )
                if not cost["passed"]:
                    continue
                score = pair_score(
                    next(row for row in public_legs if row["symbol"] == left_symbol),
                    next(row for row in public_legs if row["symbol"] == right_symbol),
                    copula, cost, liquidity_by_symbol,
                )
                edges.append({
                    "pair_id": f"{left_symbol}-{right_symbol}", "left": left_symbol,
                    "right": right_symbol, "score": score, "copula": copula, "cost": cost,
                    "model_input_hash": hashlib.sha256(canonical_bytes({
                        "left": next(row for row in public_legs if row["symbol"] == left_symbol),
                        "right": next(row for row in public_legs if row["symbol"] == right_symbol),
                        "copula": copula, "cost": cost,
                    })).hexdigest(),
                })
        arms[arm] = {
            "stationary_alts": [row["symbol"] for row in selected],
            "pair_graph": edges, "exact_matching": exact_matching(edges),
            "cost_rejection_count": len(selected) * (len(selected) - 1) // 2 - len(edges),
        }
    payload = {
        "schema_version": 1, "fingerprint": FINGERPRINT,
        "return_blind": True, "frequency": frequency, "formation_days": formation,
        "roll_anchor_ms": anchor, "roll_anchor": iso(anchor),
        "fit_start_ms": anchor - formation * DAY_MS, "fit_cutoff_ms": anchor,
        "reference": REFERENCE, "eligible_universe": universe,
        "liquidity": liquidity_rows, "legs": public_legs, "selector_arms": arms,
    }
    payload = sanitize(payload)
    payload["payload_sha256"] = hashlib.sha256(canonical_bytes(payload)).hexdigest()
    digest = atomic_json(path, payload)
    return {"name": name, "path": str(path), "sha256": digest, "resumed": False}


def d0_gate(market: dict[str, Any], funding: dict[str, Any], exchange: dict[str, Any]) -> dict[str, Any]:
    market_by_symbol = {row["symbol"]: row for row in market["symbols"]}
    funding_by_symbol = {row["symbol"]: row for row in funding["symbols"]}
    eligible = [
        symbol for symbol in ALTS
        if market_by_symbol[symbol]["complete"]
        and funding_by_symbol[symbol]["complete"]
        and symbol in FILTERS
    ]
    return {
        "schema_version": 1, "fingerprint": FINGERPRINT,
        "status": "PASS" if len(eligible) >= 12 and exchange["exact_symbol_set"] else "VALID_DATA_GATE_NO_SEARCH",
        "eligible_alt_count": len(eligible), "eligible_alts": eligible,
        "reference_only": REFERENCE, "market": market, "funding": funding,
        "exchange_info": exchange,
        "maintenance": {
            "source": "audited production EngineConfig plus exchangeInfo maintMarginPercent",
            "production_default_rate": 0.025, "g3_stress_multiplier": 1.05,
            "exchange_info_is_not_leverage_bracket_source": True,
        },
    }


def environment_manifest(args: argparse.Namespace, started: float) -> dict[str, Any]:
    return {
        "schema_version": 1, "fingerprint": FINGERPRINT,
        "argv": sys.argv, "pid": os.getpid(), "start_utc": iso(int(started * 1000)),
        "python": platform.python_version(), "numpy": np.__version__, "scipy": scipy.__version__,
        "statsmodels": statsmodels.__version__, "statsmodels_expected": "0.14.5",
        "script_sha256": sha256_file(Path(__file__)),
        "market_data": args.market_data, "funding_data": args.funding_data,
        "exchange_info": str(EXCHANGE_INFO),
    }


def validator_diagnostics(frequency: str, symbol: str, anchor: int, formation: int) -> dict[str, Any]:
    """Independent statsmodels path; deliberately does not call diagnostics()."""
    start = anchor - formation * DAY_MS
    times, btc, alt = aligned_prices(frequency, symbol, start, anchor)
    interval = 3_600_000 if frequency == "1h" else 300_000
    expected = formation * DAY_MS // interval
    if times.size != expected:
        return {"sample_count": int(times.size), "sample_coverage": float(times.size / expected)}
    y, x = np.log(btc), np.log(alt)
    intercept, beta = np.linalg.lstsq(np.column_stack((np.ones(x.size), x)), y, rcond=None)[0]
    residual = y - intercept - beta * x
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", InterpolationWarning)
        warnings.simplefilter("ignore", RuntimeWarning)
        eg_stat, eg_p, _ = coint(y, x, trend="c", autolag="aic")
        adf_result = adfuller(residual, regression="c", autolag="AIC")
        kpss_result = kpss(residual, regression="c", nlags="auto")
        cusum_stat, cusum_p, _ = breaks_cusumolsresid(residual, ddof=2)
    lag, delta = residual[:-1], np.diff(residual)
    gamma = float(np.linalg.lstsq(np.column_stack((np.ones(lag.size), lag)), delta, rcond=None)[0][1])
    phi = 1 + gamma
    half_life = float(-np.log(2) / np.log(phi)) if 0 < phi < 1 else math.inf
    midpoint = x.size // 2
    beta_first = float(np.linalg.lstsq(np.column_stack((np.ones(midpoint), x[:midpoint])), y[:midpoint], rcond=None)[0][1])
    beta_second = float(np.linalg.lstsq(np.column_stack((np.ones(x.size - midpoint), x[midpoint:])), y[midpoint:], rcond=None)[0][1])
    uniforms = rankdata(residual, method="average") / (residual.size + 1)
    tail_count, neutral_returns, _ = crossing_excursions(uniforms, alt, WEEK_MS // interval)
    return {
        "sample_count": int(times.size), "sample_coverage": float(times.size / expected),
        "intercept": float(intercept), "beta": float(beta),
        "residual_sigma": float(np.std(residual, ddof=1)),
        "eg_coint_statistic": float(eg_stat), "eg_coint_p_value": float(eg_p),
        "residual_adf_statistic": float(adf_result[0]), "residual_adf_p_value": float(adf_result[1]),
        "kpss_statistic": float(kpss_result[0]), "kpss_p_value": float(kpss_result[1]),
        "half_life_bars": half_life, "cusum_statistic": float(cusum_stat),
        "cusum_p_value": float(cusum_p),
        "absolute_beta_drift_ratio": abs(beta_second - beta_first) / max(abs(beta_first), 1e-12),
        "actual_tail_crossings_alpha_0p20": tail_count,
        "neutral_returns": neutral_returns, "censored_failures": tail_count - neutral_returns,
    }


def validator_matching(edges: list[dict[str, Any]]) -> list[str]:
    best: tuple[int, float, list[str]] = (0, 0.0, [])
    for count in range(1, min(3, len(edges)) + 1):
        for selected in combinations(edges, count):
            symbols = [value for edge in selected for value in (edge["left"], edge["right"])]
            if len(symbols) != len(set(symbols)):
                continue
            ids = sorted(edge["pair_id"] for edge in selected)
            candidate = (count, sum(edge["score"] for edge in selected), ids)
            if candidate[0] > best[0] or (
                candidate[0] == best[0]
                and (candidate[1] > best[1] + 1e-12 or (abs(candidate[1] - best[1]) <= 1e-12 and candidate[2] < best[2]))
            ):
                best = candidate
    return best[2]


def validator_leg_task(task: tuple[str, str, int, int]) -> tuple[tuple[str, str, int, int], dict[str, Any]]:
    frequency, symbol, anchor, formation = task
    return task, validator_diagnostics(frequency, symbol, anchor, formation)


def validator_leg_cache_path(root: Path, task: tuple[str, str, int, int]) -> Path:
    frequency, symbol, anchor, formation = task
    return root / f"{frequency}-{formation}d-{anchor}-{symbol}.json"


def validator_leg_cache_payload(task: tuple[str, str, int, int], result: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "algorithm": VALIDATOR_CACHE_ALGORITHM,
        "market_data_sha256": MARKET_DB_SHA256,
        "statsmodels": statsmodels.__version__,
        "task": list(task),
        "result": result,
    }


def load_validator_leg_cache(path: Path, task: tuple[str, str, int, int]) -> dict[str, Any] | None:
    if not path.exists():
        return None
    try:
        payload = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return None
    if (
        payload.get("schema_version") != 1
        or payload.get("algorithm") != VALIDATOR_CACHE_ALGORITHM
        or payload.get("market_data_sha256") != MARKET_DB_SHA256
        or payload.get("statsmodels") != statsmodels.__version__
        or payload.get("task") != list(task)
        or not isinstance(payload.get("result"), dict)
    ):
        return None
    return payload["result"]


def run_validation(args: argparse.Namespace) -> None:
    global SIGNALS, LIQUIDITY, FUNDING, FILTERS, OUTPUT_ROOT
    OUTPUT_ROOT = Path(args.artifact_root)
    manifest_path = OUTPUT_ROOT / "fit-snapshot-manifests" / "g1.json"
    if not manifest_path.exists():
        manifest_path = OUTPUT_ROOT / "fit-snapshot-manifests" / "g0.json"
    manifest = json.loads(manifest_path.read_text())
    snapshots = []
    violations = []
    for row in manifest["snapshots"]:
        path = Path(row["path"])
        if sha256_file(path) != row["sha256"]:
            violations.append(f"snapshot_sha256:{path}")
            continue
        snapshots.append((row, json.loads(path.read_text())))
    frequencies = sorted({snapshot["frequency"] for _, snapshot in snapshots})
    formations = sorted({int(snapshot["formation_days"]) for _, snapshot in snapshots})
    roll_anchors = sorted({int(snapshot["roll_anchor_ms"]) for _, snapshot in snapshots})
    FILTERS, _ = read_exchange_info(EXCHANGE_INFO)
    FUNDING, _ = load_funding(Path(args.funding_data))
    SIGNALS, LIQUIDITY, _ = load_market(Path(args.market_data), frequencies, roll_anchors, formations)
    leg_tasks = sorted({
        (snapshot["frequency"], leg["symbol"], int(snapshot["roll_anchor_ms"]), int(snapshot["formation_days"]))
        for _, snapshot in snapshots for leg in snapshot["legs"]
    })
    cache_root = OUTPUT_ROOT / "validator-checkpoints" / "model-legs-v1"
    cache_root.mkdir(parents=True, exist_ok=True)
    independent_leg_results = {}
    pending_leg_tasks = []
    for task in leg_tasks:
        cached = load_validator_leg_cache(validator_leg_cache_path(cache_root, task), task) if args.resume else None
        if cached is None:
            pending_leg_tasks.append(task)
        else:
            independent_leg_results[task] = cached
    print(json.dumps({
        "validator": "model", "stage": "legs", "total": len(leg_tasks),
        "cached": len(independent_leg_results), "pending": len(pending_leg_tasks),
        "workers": args.workers,
    }), flush=True)

    def record_leg(result: tuple[tuple[str, str, int, int], dict[str, Any]]) -> None:
        task, diagnostics = result
        independent_leg_results[task] = diagnostics
        atomic_json(
            validator_leg_cache_path(cache_root, task),
            validator_leg_cache_payload(task, diagnostics),
        )
        completed = len(independent_leg_results)
        if completed == len(leg_tasks) or completed % 100 == 0:
            print(json.dumps({
                "validator": "model", "stage": "legs", "completed": completed,
                "total": len(leg_tasks),
            }), flush=True)

    if args.workers > 1 and pending_leg_tasks:
        with get_context("fork").Pool(args.workers) as pool:
            for result in pool.imap_unordered(validator_leg_task, pending_leg_tasks, chunksize=1):
                record_leg(result)
    else:
        for task in pending_leg_tasks:
            record_leg(validator_leg_task(task))
    fields = [
        "sample_count", "sample_coverage", "intercept", "beta", "residual_sigma",
        "eg_coint_statistic", "eg_coint_p_value", "residual_adf_statistic",
        "residual_adf_p_value", "kpss_statistic", "kpss_p_value", "half_life_bars",
        "cusum_statistic", "cusum_p_value", "absolute_beta_drift_ratio",
        "actual_tail_crossings_alpha_0p20", "neutral_returns", "censored_failures",
    ]
    checked_legs = checked_pairs = 0
    max_error = 0.0
    for _, snapshot in snapshots:
        frequency = snapshot["frequency"]
        formation = int(snapshot["formation_days"])
        anchor = int(snapshot["roll_anchor_ms"])
        universe, _ = top20(anchor, formation)
        if universe != snapshot["eligible_universe"]:
            violations.append(f"top20:{frequency}:{formation}:{anchor}")
        independent_legs = {}
        for leg in snapshot["legs"]:
            expected = independent_leg_results[(frequency, leg["symbol"], anchor, formation)]
            independent_legs[leg["symbol"]] = expected
            checked_legs += 1
            for field in fields:
                left, right = leg.get(field), expected.get(field)
                if left is None and isinstance(right, float) and not math.isfinite(right):
                    continue
                if left is None or right is None:
                    if left != right:
                        violations.append(f"leg_null:{frequency}:{formation}:{anchor}:{leg['symbol']}:{field}")
                    continue
                if isinstance(left, (int, bool)) and isinstance(right, (int, bool)):
                    error = 0.0 if left == right else math.inf
                else:
                    error = abs(float(left) - float(right))
                max_error = max(max_error, error)
                tolerance = 1e-9 * max(1.0, abs(float(right)))
                if error > tolerance:
                    violations.append(f"leg:{frequency}:{formation}:{anchor}:{leg['symbol']}:{field}:{error}")
        p_values = [float(leg.get("eg_coint_p_value", 1)) for leg in snapshot["legs"]]
        fdr = bh(p_values)
        for leg, accepted in zip(snapshot["legs"], fdr, strict=True):
            expected_fdr = bool(leg.get("common_gate_passed") and accepted)
            if leg.get("fdr_passed") != expected_fdr:
                violations.append(f"bh:{frequency}:{formation}:{anchor}:{leg['symbol']}")
        for arm_name, arm in snapshot["selector_arms"].items():
            if validator_matching(arm["pair_graph"]) != sorted(row["pair_id"] for row in arm["exact_matching"]):
                violations.append(f"matching:{frequency}:{formation}:{anchor}:{arm_name}")
            for pair in arm["pair_graph"]:
                checked_pairs += 1
                cost = pair["cost"]
                excursions = cost["excursions"]
                all_in = np.asarray([row["all_in_edge_quote"] for row in excursions])
                costs = np.asarray([row["round_trip_cost_quote"] for row in excursions])
                mismatches = np.asarray([row["gross_mismatch_ratio"] for row in excursions])
                ratio = float(np.median(all_in) / np.median(costs)) if all_in.size else -math.inf
                p25 = float(np.quantile(all_in, 0.25)) if all_in.size else -math.inf
                mismatch = float(np.max(mismatches)) if mismatches.size else math.inf
                decision = bool(len(excursions) >= 20 and ratio >= 2 and p25 > 0 and mismatch <= 0.05)
                if not decision or not cost["passed"]:
                    violations.append(f"cost:{frequency}:{formation}:{anchor}:{pair['pair_id']}")
                copula = pair["copula"]
                for canary in copula["python_reference_canaries"]:
                    actual = conditional_h(np.asarray([canary["u"]]), copula["family"], copula["rho"], copula["nu"])[0]
                    error = float(np.max(np.abs(actual - np.asarray(canary["h"]))))
                    max_error = max(max_error, error)
                    if error > 1e-12:
                        violations.append(f"copula:{frequency}:{formation}:{anchor}:{pair['pair_id']}:{error}")
    report = {
        "schema_version": 1, "validator": "independent_python_statsmodels_raw_db",
        "passed": not violations, "snapshot_count": len(snapshots),
        "checked_leg_count": checked_legs, "checked_pair_count": checked_pairs,
        "max_numeric_error": max_error, "violations": violations,
        "production_fitter_called": False, "raw_database_read": True,
    }
    atomic_json(OUTPUT_ROOT / "model-independent-validator.json", report)
    atomic_json(REPO_ARTIFACT / "model-independent-validator.json", report)
    print(json.dumps({"validator": "model", "passed": report["passed"], "violations": len(violations)}))


def main() -> None:
    global SIGNALS, LIQUIDITY, FUNDING, FILTERS, OUTPUT_ROOT, RESUME
    args = parse_args()
    started = time.time()
    if args.phase == "validate":
        run_validation(args)
        return
    frequencies = [value for value in args.frequency.split(",") if value]
    formations = [int(value) for value in args.formation_days.split(",") if value]
    if any(value not in ("1h", "5m") for value in frequencies) or any(value not in (14, 21) for value in formations):
        raise SystemExit("frequency/formation outside frozen Round 26 population")
    OUTPUT_ROOT = Path(args.artifact_root)
    OUTPUT_ROOT.mkdir(parents=True, exist_ok=True)
    RESUME = args.resume
    roll_anchors = phase_anchors(args.phase)
    FILTERS, exchange = read_exchange_info(EXCHANGE_INFO)
    FUNDING, funding_manifest = load_funding(Path(args.funding_data))
    SIGNALS, LIQUIDITY, market_manifest = load_market(
        Path(args.market_data), frequencies, roll_anchors, formations
    )
    if args.phase == "g0":
        gate = d0_gate(market_manifest, funding_manifest, exchange)
        atomic_json(OUTPUT_ROOT / "checkpoints" / "d0.json", gate)
        REPO_ARTIFACT.mkdir(parents=True, exist_ok=True)
        atomic_json(REPO_ARTIFACT / "gates" / "d0.json", gate)
        atomic_json(REPO_ARTIFACT / "round26-data-manifest.json", {
            "market": market_manifest, "funding": funding_manifest,
            "exchange_info": exchange, "eligible_alts": gate["eligible_alts"],
        })
        if gate["status"] != "PASS":
            print(json.dumps({"status": gate["status"], "eligible": gate["eligible_alt_count"]}))
            return
    tasks = [
        (frequency, formation, anchor)
        for frequency in frequencies for formation in formations for anchor in roll_anchors
    ]
    if args.workers > 1:
        with get_context("fork").Pool(args.workers) as pool:
            rows = list(pool.imap_unordered(build_snapshot, tasks, chunksize=1))
    else:
        rows = [build_snapshot(task) for task in tasks]
    rows.sort(key=lambda row: row["name"])
    finished = time.time()
    manifest = {
        **environment_manifest(args, started),
        "end_utc": iso(int(finished * 1000)), "wall_seconds": finished - started,
        "exit_code": 0, "phase": args.phase, "snapshot_count": len(rows), "snapshots": rows,
    }
    atomic_json(OUTPUT_ROOT / "fit-snapshot-manifests" / f"{args.phase}.json", manifest)
    atomic_json(OUTPUT_ROOT / "checkpoints" / f"{args.phase}-fit-complete.json", {
        "phase": args.phase, "terminal": True, "snapshot_count": len(rows),
        "manifest_sha256": hashlib.sha256(canonical_bytes(manifest)).hexdigest(),
    })
    print(json.dumps({"phase": args.phase, "snapshot_count": len(rows), "wall_seconds": finished - started}))


if __name__ == "__main__":
    main()
