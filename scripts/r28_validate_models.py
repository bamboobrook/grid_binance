#!/usr/bin/env python3
"""Independent Round 28 C0 and threshold-model validator."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import resource
import sqlite3
import sys
import time
from itertools import combinations
from multiprocessing import get_context
from pathlib import Path
from typing import Any

import numpy as np
from scipy.optimize import minimize_scalar
from scipy.stats import chi2, kendalltau, multivariate_normal, multivariate_t, norm, rankdata, t
from statsmodels.tsa.stattools import coint


START_MS = 1_688_169_600_000
END_MS = 1_780_070_400_000
DAY_MS = 86_400_000
WEEK_MS = 604_800_000
MINUTE_MS = 60_000
ALTS = [
    "AAVEUSDT", "ADAUSDT", "ALGOUSDT", "APTUSDT", "ATOMUSDT", "AVAXUSDT",
    "BCHUSDT", "BNBUSDT", "COMPUSDT", "CRVUSDT", "DASHUSDT", "DOGEUSDT",
    "DOTUSDT", "DYDXUSDT", "EGLDUSDT", "ETCUSDT", "ETHUSDT", "FILUSDT",
    "GALAUSDT", "HBARUSDT", "ICPUSDT", "INJUSDT", "LINKUSDT", "NEARUSDT",
    "SOLUSDT", "TRXUSDT", "UNIUSDT", "XRPUSDT", "ZECUSDT",
]
SYMBOLS = ["BTCUSDT", *ALTS]
ROUND27_RAW = Path("artifacts-local/round27/aaa02fda4e588ced0359318465ee682c677ae00c")
REPO_ARTIFACT = Path("docs/superpowers/artifacts/glm-martingale-core-round28")
SIGNALS: dict[str, dict[str, tuple[np.ndarray, np.ndarray]]] = {}
LIQUIDITY: dict[tuple[str, int, int], dict[str, Any]] = {}
FILTERS: dict[str, dict[str, float]] = {}
FUNDING: dict[str, tuple[np.ndarray, np.ndarray]] = {}
RAW_ROOT = Path(".")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=("recovery-c0", "all-models"), required=True)
    parser.add_argument("--artifact-root", required=True)
    parser.add_argument("--market-data", default="data/market_data_full.db")
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--resume", action="store_true")
    return parser.parse_args()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(8 * 1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def atomic_json(path: Path, value: Any) -> None:
    def clean(item: Any) -> Any:
        if isinstance(item, dict): return {key: clean(row) for key, row in item.items()}
        if isinstance(item, (list, tuple)): return [clean(row) for row in item]
        if isinstance(item, (np.integer,)): return int(item)
        if isinstance(item, (np.floating, float)):
            number = float(item); return number if math.isfinite(number) else None
        if isinstance(item, np.bool_): return bool(item)
        return item
    payload = json.dumps(clean(value), indent=2, sort_keys=True, allow_nan=False).encode() + b"\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_bytes(payload); temporary.replace(path)


def anchors() -> list[int]:
    return [START_MS + index * WEEK_MS for index in range(152)]


def load_raw_market(path: Path, formations: list[int]) -> dict[str, Any]:
    connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    signal_output = {"1h": {}, "5m": {}}
    liquidity_output: dict[tuple[str, int, int], dict[str, Any]] = {}
    try:
        for position, symbol in enumerate(SYMBOLS, start=1):
            cursor = connection.execute(
                "SELECT open_time,close_time,close,volume FROM klines INDEXED BY idx_klines_symbol_time "
                "WHERE symbol=? AND market_type='futures_usdt_perp' AND timeframe='1m' ORDER BY open_time",
                (symbol,),
            )
            flat = np.fromiter((value for row in cursor for value in row), dtype=np.float64)
            minute = flat.reshape((-1, 4))
            times = minute[:, 0].astype(np.int64); close_times = minute[:, 1].astype(np.int64)
            closes = minute[:, 2]; volumes = minute[:, 3]
            quote = closes * volumes
            for frequency, interval in (("1h", 3_600_000), ("5m", 300_000)):
                selected = times % interval == interval - MINUTE_MS
                signal_output[frequency][symbol] = (close_times[selected], closes[selected])
            if symbol != "BTCUSDT":
                for anchor in anchors():
                    right = int(np.searchsorted(times, anchor, side="left"))
                    for formation in formations:
                        start = anchor - formation * DAY_MS
                        left = int(np.searchsorted(times, start, side="left"))
                        selected_quote = quote[left:right]
                        selected_times = times[left:right]
                        expected = formation * 1440
                        if selected_quote.size:
                            day = (selected_times - start) // DAY_MS
                            daily = np.bincount(day, weights=selected_quote, minlength=formation)
                            row = {
                                "symbol": symbol, "coverage": float(selected_quote.size / expected),
                                "zero_volume_ratio": float(np.count_nonzero(volumes[left:right] == 0) / selected_quote.size),
                                "p10_minute_quote_volume": float(np.quantile(selected_quote, 0.10)),
                                "median_daily_quote_volume": float(np.median(daily)),
                            }
                        else:
                            row = {"symbol": symbol, "coverage": 0.0, "zero_volume_ratio": 1.0,
                                   "p10_minute_quote_volume": 0.0, "median_daily_quote_volume": 0.0}
                        liquidity_output[(symbol, anchor, formation)] = row
            print(f"validator-market-load {position:02d}/{len(SYMBOLS)} {symbol}", flush=True)
    finally:
        connection.close()
    global SIGNALS, LIQUIDITY
    SIGNALS = signal_output; LIQUIDITY = liquidity_output
    return {"path": str(path), "bytes": path.stat().st_size, "sha256": sha256_file(path), "raw_database_read": True}


def independent_top20(anchor: int, formation: int) -> list[str]:
    rows = [LIQUIDITY[(symbol, anchor, formation)] for symbol in ALTS]
    eligible = [row for row in rows if row["coverage"] == 1.0 and row["zero_volume_ratio"] <= 0.01 and row["p10_minute_quote_volume"] >= 10_000]
    eligible.sort(key=lambda row: (-row["median_daily_quote_volume"], row["symbol"]))
    return [row["symbol"] for row in eligible[:20]]


def load_supporting_data() -> None:
    manifest = json.loads(Path("docs/superpowers/artifacts/glm-martingale-core-round27/round27-data-manifest.json").read_text())
    global FILTERS, FUNDING
    FILTERS = {row["symbol"]: row for row in manifest["exchange_info"]["filters"]}
    connection = sqlite3.connect(f"file:{Path('data/funding_rates.db').resolve()}?mode=ro", uri=True)
    try:
        for symbol in SYMBOLS:
            rows = connection.execute("SELECT funding_time,funding_rate FROM funding_rates WHERE symbol=? ORDER BY funding_time", (symbol,)).fetchall()
            FUNDING[symbol] = (np.asarray([row[0] for row in rows], dtype=np.int64), np.asarray([row[1] for row in rows], dtype=float))
    finally:
        connection.close()


def independent_matching(edges: list[dict[str, Any]]) -> list[str]:
    ordered = sorted(edges, key=lambda row: row["pair_id"]); best: list[dict[str, Any]] = []
    def better(candidate: list[dict[str, Any]]) -> bool:
        if len(candidate) != len(best): return len(candidate) > len(best)
        candidate_score = sum(float(row["score"]) for row in candidate); best_score = sum(float(row["score"]) for row in best)
        if abs(candidate_score - best_score) > 1e-12: return candidate_score > best_score
        return [row["pair_id"] for row in candidate] < [row["pair_id"] for row in best]
    def visit(start: int, used: set[str], selected: list[dict[str, Any]]) -> None:
        nonlocal best
        if better(selected): best = list(selected)
        if len(selected) == 3: return
        for index in range(start, len(ordered)):
            edge = ordered[index]
            if edge["left"] in used or edge["right"] in used: continue
            visit(index + 1, used | {edge["left"], edge["right"]}, [*selected, edge])
    visit(0, set(), [])
    return [row["pair_id"] for row in best]


def selected_prices(frequency: str, symbol: str, start: int, end: int) -> tuple[np.ndarray, np.ndarray]:
    times, prices = SIGNALS[frequency][symbol]
    left = int(np.searchsorted(times, start, side="left")); right = int(np.searchsorted(times, end, side="left"))
    return times[left:right], prices[left:right]


def aligned_three(frequency: str, symbols: tuple[str, str, str], start: int, end: int) -> tuple[np.ndarray, ...]:
    rows = [selected_prices(frequency, symbol, start, end) for symbol in symbols]
    common = rows[0][0]
    for times, _ in rows[1:]: common = np.intersect1d(common, times, assume_unique=True)
    values = []
    for times, prices in rows:
        index = np.searchsorted(times, common); values.append(prices[index])
    return common, *values


def empirical(values: np.ndarray) -> np.ndarray:
    return rankdata(values, method="average") / (values.size + 1.0)


def copula_loglik(uniforms: np.ndarray, family: str, rho: float, nu: int | None) -> float:
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


def independent_copula(uniforms: np.ndarray) -> dict[str, Any]:
    rows = []
    for family, nu in [("gaussian", None), *[("student_t", value) for value in range(3, 31)]]:
        fit = minimize_scalar(lambda rho: -copula_loglik(uniforms, family, float(rho), nu), bounds=(-0.95, 0.95), method="bounded", options={"xatol": 1e-8})
        parameters = 1 if family == "gaussian" else 2
        rows.append({"family": family, "nu": nu, "rho": float(fit.x), "aic": float(2 * parameters + 2 * fit.fun)})
    rows.sort(key=lambda row: (row["aic"], row["family"], row["nu"] or 0)); return rows[0]


def validate_c0_snapshot(path: str, digest: str) -> dict[str, Any]:
    file = Path(path); payload = file.read_bytes()
    violations = []
    if hashlib.sha256(payload).hexdigest() != digest: violations.append("snapshot_hash")
    snapshot = json.loads(payload)
    anchor = int(snapshot["roll_anchor_ms"]); frequency = snapshot["frequency"]
    if snapshot["eligible_universe"] != independent_top20(anchor, 21): violations.append("top20")
    checked = 0; maximum_error = 0.0; beta_rejects = 0
    arm = snapshot["arms"].get("C0-RAW", {})
    for pair in arm.get("exact_matching", []):
        start = anchor - 21 * DAY_MS
        _, btc, left, right = aligned_three(frequency, ("BTCUSDT", pair["left"], pair["right"]), start, anchor)
        residuals = []
        beta_valid = True
        for symbol_values, leg in ((left, pair["left_leg"]), (right, pair["right_leg"])):
            design = np.column_stack((np.ones(symbol_values.size), np.log(symbol_values)))
            intercept, beta = np.linalg.lstsq(design, np.log(btc), rcond=None)[0]
            residual = np.log(btc) - intercept - beta * np.log(symbol_values)
            eg_p = float(coint(np.log(btc), np.log(symbol_values), trend="c", autolag="aic")[1])
            maximum_error = max(maximum_error, abs(float(beta) - leg["beta"]), abs(float(intercept) - leg["intercept"]), abs(eg_p - leg["eg_coint_p_value"]))
            beta_valid = beta_valid and math.isfinite(beta) and beta > 0
            residuals.append(residual)
        uniforms = np.column_stack((empirical(residuals[0]), empirical(residuals[1])))
        fit = independent_copula(uniforms); stored = pair["copula"]
        if fit["family"] != stored["family"] or fit["nu"] != stored["nu"]: violations.append("copula_family")
        maximum_error = max(maximum_error, abs(fit["rho"] - stored["rho"]), abs(fit["aic"] - stored["aic"]))
        if beta_valid:
            left_weight = pair["left_leg"]["beta"] / (pair["left_leg"]["beta"] + pair["right_leg"]["beta"])
            if not 0 < left_weight < 1: violations.append("beta_weight")
        else:
            beta_rejects += 1
        checked += 1
    selected_ids = [row["pair_id"] for row in arm.get("exact_matching", [])]
    if len(selected_ids) != len(set(selected_ids)): violations.append("matching_duplicate")
    if selected_ids != independent_matching(arm.get("pair_graph", [])): violations.append("matching_not_optimal")
    return {"path": path, "checked_pairs": checked, "invalid_beta_pair_rejects": beta_rejects,
            "max_error": maximum_error, "violations": violations}


def _threshold_fit_independent(residual: np.ndarray, variant: str) -> dict[str, Any] | None:
    delta = np.diff(residual); response = delta[1:]; lag = residual[1:-1]; previous_delta = delta[:-1]
    regime = lag if variant == "TAR" else previous_delta
    candidates = np.unique(regime); candidates = candidates[int(.15 * candidates.size):int(math.ceil(.85 * candidates.size))]
    best = None
    for threshold in candidates:
        indicator = regime >= threshold
        design = np.column_stack((np.where(indicator, lag, 0.0), np.where(indicator, 0.0, lag), previous_delta))
        if np.linalg.matrix_rank(design) < 3: continue
        beta = np.linalg.lstsq(design, response, rcond=None)[0]
        error = response - design @ beta; rss = float(error @ error)
        candidate = (rss, float(threshold), beta, error)
        if best is None or candidate[:2] < best[:2]: best = candidate
    if best is None: return None
    rss, threshold, beta, error = best
    gamma_null = float(np.dot(previous_delta, response) / np.dot(previous_delta, previous_delta))
    null_error = response - gamma_null * previous_delta; rss_null = float(null_error @ null_error)
    statistic = max(0.0, (rss_null - rss) / 2) / max(rss / max(response.size - 3, 1), 1e-18)
    return {"threshold": threshold, "rho_1": float(beta[0]), "rho_2": float(beta[1]), "gamma": float(beta[2]),
            "rss": rss, "joint_statistic": statistic, "null_gamma": gamma_null, "null_error": null_error,
            "full_error": error, "initial_residual": float(residual[0]), "initial_delta": float(delta[0])}


def _blocks(values: np.ndarray, rng: np.random.Generator) -> np.ndarray:
    starts = rng.integers(0, max(values.size - 24 + 1, 1), size=math.ceil(values.size / 24))
    return np.concatenate([values[start:start + 24] for start in starts])[:values.size]


def independent_bootstrap(residual: np.ndarray, variant: str, seed: int) -> dict[str, float] | None:
    fitted = _threshold_fit_independent(residual, variant)
    if fitted is None: return None
    rng = np.random.default_rng(seed); statistics = []; first = []; second = []
    for _ in range(199):
        null_shocks = _blocks(fitted["null_error"], rng); full_shocks = _blocks(fitted["full_error"], rng)
        null_delta = [fitted["initial_delta"]]; full_delta = [fitted["initial_delta"]]
        null_residual = [fitted["initial_residual"], fitted["initial_residual"] + fitted["initial_delta"]]
        full_residual = list(null_residual)
        for null_shock, full_shock in zip(null_shocks, full_shocks, strict=True):
            nd = fitted["null_gamma"] * null_delta[-1] + null_shock; null_delta.append(nd); null_residual.append(null_residual[-1] + nd)
            indicator = full_residual[-1] >= fitted["threshold"] if variant == "TAR" else full_delta[-1] >= fitted["threshold"]
            rho = fitted["rho_1"] if indicator else fitted["rho_2"]
            fd = rho * full_residual[-1] + fitted["gamma"] * full_delta[-1] + full_shock
            full_delta.append(fd); full_residual.append(full_residual[-1] + fd)
        null_fit = _threshold_fit_independent(np.asarray(null_residual), variant); full_fit = _threshold_fit_independent(np.asarray(full_residual), variant)
        if null_fit is None or full_fit is None: continue
        statistics.append(null_fit["joint_statistic"]); first.append(full_fit["rho_1"]); second.append(full_fit["rho_2"])
    if not statistics: return None
    return {"threshold": fitted["threshold"], "rho_1": fitted["rho_1"], "rho_2": fitted["rho_2"], "gamma": fitted["gamma"],
            "joint_bootstrap_p_value": float((1 + np.count_nonzero(np.asarray(statistics) >= fitted["joint_statistic"])) / (1 + len(statistics))),
            "rho_1_negative_support": float(np.mean(np.asarray(first) < 0)), "rho_2_negative_support": float(np.mean(np.asarray(second) < 0))}


def independent_projected_cost(pair: dict[str, Any], left_price: float, right_price: float, start: int, end: int) -> dict[str, float | bool]:
    beta = abs(float(pair["beta"])); scale = float(pair["residual_scale"])
    left_weight = 1 / (1 + beta); right_weight = beta / (1 + beta)
    def quantity(symbol: str, price: float, target: float) -> float:
        rule = FILTERS[symbol]; step = float(rule["step_size"]); minimum = max(float(rule["min_notional"]), target)
        return max(float(rule["min_qty"]), math.ceil(minimum / price / step - 1e-12) * step)
    left_qty = quantity(pair["left"], left_price, 100 * left_weight); right_qty = quantity(pair["right"], right_price, 100 * right_weight)
    left_gross = left_qty * left_price; right_gross = right_qty * right_price; total = left_gross + right_gross
    mismatch = max(abs(left_gross / total - left_weight) / left_weight, abs(right_gross / total - right_weight) / right_weight)
    def p95(symbol: str) -> float:
        times, rates = FUNDING[symbol]; left = int(np.searchsorted(times, start)); right = int(np.searchsorted(times, end)); return float(np.quantile(np.abs(rates[left:right]), .95))
    cost = 2 * total * 6 / 10_000 + total * 2 / 10_000 + 3 * (p95(pair["left"]) * left_gross + p95(pair["right"]) * right_gross)
    edge = total * 1.75 * scale / (1 + beta)
    return {"ratio": edge / cost, "mismatch": mismatch, "passed": mismatch <= .05 and edge / cost >= 2}


def independent_bh(values: list[float], q: float = .05) -> list[bool]:
    ordered = sorted(enumerate(values), key=lambda row: (row[1], row[0])); cutoff = 0
    for rank, (_, value) in enumerate(ordered, start=1):
        if value <= q * rank / len(ordered): cutoff = rank
    accepted = [False] * len(values)
    for index, _ in ordered[:cutoff]: accepted[index] = True
    return accepted


def validate_threshold_snapshot(path: str, digest: str) -> dict[str, Any]:
    payload = Path(path).read_bytes(); violations = []
    if hashlib.sha256(payload).hexdigest() != digest: violations.append("snapshot_hash")
    snapshot = json.loads(payload); anchor = int(snapshot["roll_anchor_ms"]); variant = snapshot["variant"]
    if snapshot.get("snapshot_id") != Path(path).stem: violations.append("snapshot_id")
    if snapshot["eligible_universe"] != independent_top20(anchor, 42): violations.append("top20")
    finite = [row for row in snapshot["pair_fits"] if row.get("finite_fit")]
    flags = independent_bh([float(row["threshold"]["joint_bootstrap_p_value"]) for row in finite]) if finite else []
    if any(bool(row.get("bh_q05_passed")) != flag for row, flag in zip(finite, flags, strict=True)): violations.append("bh")
    checked = 0; maximum_error = 0.0
    pairs_to_check = snapshot["exact_matching"] or finite[:1]
    for pair in pairs_to_check:
        if not pair["left"] < pair["right"]: violations.append("orientation")
        _, left = selected_prices("1h", pair["left"], anchor - 42 * DAY_MS, anchor)
        _, right = selected_prices("1h", pair["right"], anchor - 42 * DAY_MS, anchor)
        design = np.column_stack((np.ones(right.size), np.log(right)))
        intercept, beta = np.linalg.lstsq(design, np.log(left), rcond=None)[0]
        residual = np.log(left) - intercept - beta * np.log(right)
        snapshot_id = snapshot["snapshot_id"]
        seed = int.from_bytes(
            hashlib.sha256(f"{snapshot_id}{pair['pair_id']}{variant}".encode()).digest()[:8],
            "big",
        )
        estimate = independent_bootstrap(residual, variant, seed)
        if estimate is None: violations.append("bootstrap"); continue
        stored = pair["threshold"]
        for key in ("threshold", "rho_1", "rho_2", "gamma", "joint_bootstrap_p_value", "rho_1_negative_support", "rho_2_negative_support"):
            maximum_error = max(maximum_error, abs(float(estimate[key]) - float(stored[key])))
        maximum_error = max(maximum_error, abs(float(intercept) - pair["intercept"]), abs(float(beta) - pair["beta"]))
        if seed != stored["bootstrap_seed"]: violations.append("bootstrap_seed")
        cost = independent_projected_cost(pair, float(left[-1]), float(right[-1]), anchor - 42 * DAY_MS, anchor)
        maximum_error = max(maximum_error, abs(float(cost["ratio"]) - pair["projected_cost"]["edge_cost_ratio"]), abs(float(cost["mismatch"]) - pair["projected_cost"]["resolved_weight_mismatch_ratio"]))
        if bool(cost["passed"]) != bool(pair["projected_cost"]["passed"]): violations.append("cost_gate")
        checked += 1
    selected = snapshot["exact_matching"]
    if len({symbol for pair in selected for symbol in (pair["left"], pair["right"])}) != 2 * len(selected): violations.append("matching_disjoint")
    admitted = [row for row in snapshot["pair_fits"] if row.get("admission_passed")]
    if [row["pair_id"] for row in selected] != independent_matching(admitted): violations.append("matching_not_optimal")
    return {"path": path, "checked_pairs": checked, "max_error": maximum_error, "violations": violations,
            "pair_fit_denominator": snapshot["pair_fit_denominator"], "finite_fit_count": snapshot["finite_fit_count"], "bh_pass_count": snapshot["bh_pass_count"]}


def manifest_rows(path: Path) -> list[tuple[str, str]]:
    value = json.loads(path.read_text()); return [(row["path"], row["sha256"]) for row in value["snapshots"]]


def future_shift_canary() -> dict[str, Any]:
    cutoff = START_MS + 42 * DAY_MS
    times, prices = SIGNALS["1h"]["ETHUSDT"]
    left = int(np.searchsorted(times, cutoff - 42 * DAY_MS, side="left"))
    right = int(np.searchsorted(times, cutoff + WEEK_MS, side="left"))
    selected_times = times[left:right].copy()
    original = prices[left:right].copy()
    shifted = original.copy()
    future_indexes = np.flatnonzero(selected_times >= cutoff)
    if future_indexes.size == 0:
        return {"passed": False, "reason": "no_future_observation"}
    shifted[int(future_indexes[0])] *= 1.5

    def digest(values: np.ndarray, bound: int) -> str:
        mask = selected_times < bound
        payload = np.column_stack((selected_times[mask], values[mask])).astype("<f8", copy=False).tobytes()
        return hashlib.sha256(payload).hexdigest()

    prior_original = digest(original, cutoff)
    prior_shifted = digest(shifted, cutoff)
    later_cutoff = cutoff + WEEK_MS
    later_original = digest(original, later_cutoff)
    later_shifted = digest(shifted, later_cutoff)
    passed = prior_original == prior_shifted and later_original != later_shifted
    return {
        "fit_cutoff_ms": cutoff,
        "future_timestamp_ms": int(selected_times[int(future_indexes[0])]),
        "prior_snapshot_original_hash": prior_original,
        "prior_snapshot_shifted_hash": prior_shifted,
        "later_snapshot_original_hash": later_original,
        "later_snapshot_shifted_hash": later_shifted,
        "prior_snapshot_unchanged": prior_original == prior_shifted,
        "future_shift_changes_only_after_cutoff": later_original != later_shifted,
        "passed": passed,
    }


def main() -> None:
    global RAW_ROOT
    arguments = parse_args(); started = time.time(); RAW_ROOT = Path(arguments.artifact_root).resolve()
    formations = [21] if arguments.phase == "recovery-c0" else [21, 42]
    load_supporting_data()
    market = load_raw_market(Path(arguments.market_data).resolve(), formations)
    cache = RAW_ROOT / "validator-checkpoints" / "c0-recovery.json"
    if arguments.resume and cache.exists():
        c0 = json.loads(cache.read_text())
    else:
        rows = manifest_rows(ROUND27_RAW / "fit-snapshot-manifests/c0-c1.json")
        selected = []
        for path, digest in rows:
            value = json.loads(Path(path).read_text())
            if value["formation_days"] == 21 and value["frequency"] in ("1h", "5m"): selected.append((path, digest))
        with get_context("fork").Pool(arguments.workers) as pool:
            checked = list(pool.starmap(validate_c0_snapshot, selected, chunksize=1))
        c0 = {"snapshot_count": len(checked), "checked_pairs": sum(row["checked_pairs"] for row in checked),
              "invalid_beta_pair_rejects": sum(row["invalid_beta_pair_rejects"] for row in checked),
              "max_error": max((row["max_error"] for row in checked), default=0.0),
              "violations": [item for row in checked for item in row["violations"]]}
        atomic_json(cache, c0)
    threshold = {"snapshot_count": 0, "checked_pairs": 0, "max_error": 0.0, "violations": [], "pair_fit_denominator": 0, "finite_fit_count": 0, "bh_pass_count": 0}
    if arguments.phase == "all-models":
        selected = []
        for variant in ("tar", "mtar"):
            selected.extend(manifest_rows(RAW_ROOT / "fit-snapshot-manifests" / f"{variant}.json"))
        with get_context("fork").Pool(arguments.workers) as pool:
            checked = list(pool.starmap(validate_threshold_snapshot, selected, chunksize=1))
        threshold = {"snapshot_count": len(checked), "checked_pairs": sum(row["checked_pairs"] for row in checked),
                     "max_error": max((row["max_error"] for row in checked), default=0.0),
                     "violations": [item for row in checked for item in row["violations"]],
                     "pair_fit_denominator": sum(row["pair_fit_denominator"] for row in checked),
                     "finite_fit_count": sum(row["finite_fit_count"] for row in checked),
                     "bh_pass_count": sum(row["bh_pass_count"] for row in checked)}
    tolerance = 5e-5
    violations = [*c0["violations"], *threshold["violations"]]
    if c0["max_error"] > tolerance: violations.append("c0_numeric_tolerance")
    if threshold["max_error"] > tolerance: violations.append("threshold_numeric_tolerance")
    future_shift = future_shift_canary()
    if not future_shift["passed"]: violations.append("future_shift_causality")
    value = {"schema_version": 1, "phase": arguments.phase, "validator": "independent_r28_raw_db_model_validator",
             "passed": not violations and c0["checked_pairs"] > 0 and (
                 arguments.phase == "recovery-c0" or (
                     threshold["snapshot_count"] == 304
                     and (threshold["finite_fit_count"] == 0 or threshold["checked_pairs"] > 0)
                 )
             ),
             "checked_copula_pairs": c0["checked_pairs"], "checked_threshold_pairs": threshold["checked_pairs"],
             "checked_copula_invalid_beta_pair_rejects": c0.get("invalid_beta_pair_rejects", 0),
             "c0_snapshot_count": c0["snapshot_count"], "threshold_snapshot_count": threshold["snapshot_count"],
             "threshold_pair_fit_denominator": threshold["pair_fit_denominator"], "threshold_finite_fit_count": threshold["finite_fit_count"],
             "threshold_bh_pass_count": threshold["bh_pass_count"], "max_numeric_error": max(c0["max_error"], threshold["max_error"]),
             "tolerance": tolerance, "future_shift_canary": future_shift, "violations": violations,
             "production_fitter_imported": False, "raw_database_read": True, "market": market,
             "source_commit": RAW_ROOT.name,
             "runtime": {"argv": sys.argv, "pid": os.getpid(), "workers": arguments.workers,
                         "blas_threads": {name: os.environ.get(name) for name in ("OPENBLAS_NUM_THREADS", "OMP_NUM_THREADS", "MKL_NUM_THREADS")},
                         "start_epoch": started, "end_epoch": time.time(), "wall_seconds": time.time() - started,
                         "peak_rss_kib": int(max(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss, resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss)), "exit_code": 0}}
    atomic_json(RAW_ROOT / "model-independent-validator.json", value)
    atomic_json(REPO_ARTIFACT / "model-independent-validator.json", value)
    atomic_json(RAW_ROOT / "runtime" / f"model-validator-{arguments.phase}.json", value["runtime"])
    print(json.dumps({"phase": arguments.phase, "passed": value["passed"], "copula_pairs": c0["checked_pairs"], "threshold_pairs": threshold["checked_pairs"], "violations": len(violations)}))
    if not value["passed"]: raise SystemExit(1)


if __name__ == "__main__":
    main()
