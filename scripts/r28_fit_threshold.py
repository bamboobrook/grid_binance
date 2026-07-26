#!/usr/bin/env python3
"""Round 28 formation-only TAR/MTAR threshold snapshots."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import resource
import sys
import time
from itertools import combinations
from multiprocessing import get_context
from pathlib import Path
from typing import Any

import numpy as np
from scipy.stats import chi2

sys.path.insert(0, str(Path(__file__).resolve().parent))
import r26_fit_snapshots as r26  # noqa: E402


REPO_ARTIFACT = Path("docs/superpowers/artifacts/glm-martingale-core-round28")
POLICY_PATH = REPO_ARTIFACT / "round28-policy-manifest.json"
OUTPUT_ROOT = Path(".")
RESUME = False
IDENTITY: dict[str, Any] = {}
BOOTSTRAP_REPLICATIONS = 199
BOOTSTRAP_BLOCK = 24
FORMATION_DAYS = 42


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=("tar-mtar",), required=True)
    parser.add_argument("--artifact-root", required=True)
    parser.add_argument("--market-data", default="data/market_data_full.db")
    parser.add_argument("--funding-data", default="data/funding_rates.db")
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--resume", action="store_true")
    return parser.parse_args()


def peak_rss_kib() -> int:
    return int(max(
        resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
        resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
    ))


def clean(value: Any) -> Any:
    return r26.sanitize(value)


def atomic_json(path: Path, value: Any) -> str:
    return r26.atomic_json(path, value)


def model_hash(value: dict[str, Any]) -> str:
    return hashlib.sha256(r26.canonical_bytes(clean(value))).hexdigest()


def direct_prices(symbol: str, start: int, end: int) -> tuple[np.ndarray, np.ndarray]:
    times, prices = r26.SIGNALS["1h"][symbol]
    left = int(np.searchsorted(times, start, side="left"))
    right = int(np.searchsorted(times, end, side="left"))
    return times[left:right], prices[left:right]


def aligned_pair(left_symbol: str, right_symbol: str, start: int, end: int) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    left_times, left = direct_prices(left_symbol, start, end)
    right_times, right = direct_prices(right_symbol, start, end)
    if np.array_equal(left_times, right_times):
        return left_times, left, right
    common, left_index, right_index = np.intersect1d(
        left_times, right_times, assume_unique=True, return_indices=True
    )
    return common, left[left_index], right[right_index]


def _suffix(values: np.ndarray, order: np.ndarray, indexes: np.ndarray) -> np.ndarray:
    ordered = values[order]
    prefix = np.concatenate(([0.0], np.cumsum(ordered)))
    return prefix[-1] - prefix[indexes]


def threshold_ols(residual: np.ndarray, variant: str) -> dict[str, Any] | None:
    delta = np.diff(residual)
    response = delta[1:]
    lag_e = residual[1:-1]
    lag_delta = delta[:-1]
    regime = lag_e if variant == "TAR" else lag_delta
    distinct = np.unique(regime)
    low = int(math.floor(0.15 * distinct.size))
    high = int(math.ceil(0.85 * distinct.size))
    candidates = distinct[low:high]
    if response.size < 20 or candidates.size == 0:
        return None
    order = np.argsort(regime, kind="mergesort")
    sorted_regime = regime[order]
    indexes = np.searchsorted(sorted_regime, candidates, side="left")
    lag2_hi = _suffix(lag_e * lag_e, order, indexes)
    lag2_total = float(np.dot(lag_e, lag_e))
    lag_delta_hi = _suffix(lag_e * lag_delta, order, indexes)
    lag_delta_total = float(np.dot(lag_e, lag_delta))
    lag_y_hi = _suffix(lag_e * response, order, indexes)
    lag_y_total = float(np.dot(lag_e, response))
    delta2 = float(np.dot(lag_delta, lag_delta))
    delta_y = float(np.dot(lag_delta, response))
    matrices = np.zeros((candidates.size, 3, 3), dtype=np.float64)
    matrices[:, 0, 0] = lag2_hi
    matrices[:, 1, 1] = lag2_total - lag2_hi
    matrices[:, 0, 2] = matrices[:, 2, 0] = lag_delta_hi
    matrices[:, 1, 2] = matrices[:, 2, 1] = lag_delta_total - lag_delta_hi
    matrices[:, 2, 2] = delta2
    vectors = np.column_stack((lag_y_hi, lag_y_total - lag_y_hi, np.full(candidates.size, delta_y)))
    determinants = np.linalg.det(matrices)
    valid = np.isfinite(determinants) & (np.abs(determinants) > 1e-12)
    if not np.any(valid):
        return None
    valid_indexes = np.flatnonzero(valid)
    coefficients = np.linalg.solve(matrices[valid], vectors[valid, :, None]).squeeze(-1)
    rss = float(np.dot(response, response)) - np.einsum("ij,ij->i", coefficients, vectors[valid])
    finite = np.isfinite(rss) & (rss >= -1e-8)
    if not np.any(finite):
        return None
    valid_indexes = valid_indexes[finite]
    coefficients = coefficients[finite]
    rss = np.maximum(rss[finite], 0.0)
    order_best = np.lexsort((candidates[valid_indexes], rss))
    selected = int(order_best[0])
    original_index = int(valid_indexes[selected])
    beta = coefficients[selected]
    selected_rss = float(rss[selected])
    covariance = np.linalg.inv(matrices[original_index]) * selected_rss / max(response.size - 3, 1)
    contrast = np.array([1.0, -1.0, 0.0])
    variance = float(contrast @ covariance @ contrast)
    wald = float((beta[0] - beta[1]) ** 2 / variance) if variance > 0 else math.inf
    gamma_null = float(delta_y / delta2) if delta2 > 0 else 0.0
    null_error = response - gamma_null * lag_delta
    selected_regime = regime >= candidates[original_index]
    full_prediction = (
        beta[0] * np.where(selected_regime, lag_e, 0.0)
        + beta[1] * np.where(selected_regime, 0.0, lag_e)
        + beta[2] * lag_delta
    )
    full_error = response - full_prediction
    rss_null = float(np.dot(null_error, null_error))
    statistic = max(0.0, (rss_null - selected_rss) / 2) / max(selected_rss / max(response.size - 3, 1), 1e-18)
    return {
        "threshold": float(candidates[original_index]),
        "rho_1": float(beta[0]),
        "rho_2": float(beta[1]),
        "gamma": float(beta[2]),
        "rss": selected_rss,
        "rss_null": rss_null,
        "joint_statistic": statistic,
        "asymmetry_wald_statistic": wald,
        "asymmetry_wald_p_value": float(chi2.sf(wald, 1)),
        "null_gamma": gamma_null,
        "null_innovations": null_error,
        "full_innovations": full_error,
        "initial_delta": float(delta[0]),
        "initial_residual": float(residual[0]),
        "sample_count": int(response.size),
        "candidate_count": int(candidates.size),
    }


def block_resample(values: np.ndarray, length: int, rng: np.random.Generator) -> np.ndarray:
    maximum = max(values.size - BOOTSTRAP_BLOCK + 1, 1)
    starts = rng.integers(0, maximum, size=math.ceil(length / BOOTSTRAP_BLOCK))
    return np.concatenate([values[start:start + BOOTSTRAP_BLOCK] for start in starts])[:length]


def bootstrap_threshold(residual: np.ndarray, variant: str, seed: int) -> dict[str, Any] | None:
    fitted = threshold_ols(residual, variant)
    if fitted is None:
        return None
    innovations = np.asarray(fitted.pop("null_innovations"), dtype=np.float64)
    full_innovations = np.asarray(fitted.pop("full_innovations"), dtype=np.float64)
    rng = np.random.default_rng(seed)
    statistics: list[float] = []
    rho_1: list[float] = []
    rho_2: list[float] = []
    for _ in range(BOOTSTRAP_REPLICATIONS):
        sampled_null = block_resample(innovations, innovations.size, rng)
        sampled_full = block_resample(full_innovations, full_innovations.size, rng)
        null_delta = [fitted["initial_delta"]]
        null_residual = [fitted["initial_residual"], fitted["initial_residual"] + fitted["initial_delta"]]
        full_delta = [fitted["initial_delta"]]
        full_residual = [fitted["initial_residual"], fitted["initial_residual"] + fitted["initial_delta"]]
        for null_shock, full_shock in zip(sampled_null, sampled_full, strict=True):
            next_null = fitted["null_gamma"] * null_delta[-1] + null_shock
            null_delta.append(next_null)
            null_residual.append(null_residual[-1] + next_null)
            indicator = (
                full_residual[-1] >= fitted["threshold"]
                if variant == "TAR"
                else full_delta[-1] >= fitted["threshold"]
            )
            rho = fitted["rho_1"] if indicator else fitted["rho_2"]
            next_full = rho * full_residual[-1] + fitted["gamma"] * full_delta[-1] + full_shock
            full_delta.append(next_full)
            full_residual.append(full_residual[-1] + next_full)
        null_estimate = threshold_ols(np.asarray(null_residual), variant)
        full_estimate = threshold_ols(np.asarray(full_residual), variant)
        if null_estimate is None or full_estimate is None:
            continue
        statistics.append(float(null_estimate["joint_statistic"]))
        rho_1.append(float(full_estimate["rho_1"]))
        rho_2.append(float(full_estimate["rho_2"]))
    fitted.pop("initial_delta", None)
    fitted.pop("initial_residual", None)
    fitted.pop("null_gamma", None)
    if not statistics:
        return None
    fitted["bootstrap_replications_requested"] = BOOTSTRAP_REPLICATIONS
    fitted["bootstrap_replications_valid"] = len(statistics)
    fitted["bootstrap_block_bars"] = BOOTSTRAP_BLOCK
    fitted["joint_bootstrap_p_value"] = float(
        (1 + np.count_nonzero(np.asarray(statistics) >= fitted["joint_statistic"])) / (1 + len(statistics))
    )
    fitted["rho_1_negative_support"] = float(np.mean(np.asarray(rho_1) < 0))
    fitted["rho_2_negative_support"] = float(np.mean(np.asarray(rho_2) < 0))
    fitted["bootstrap_seed"] = seed
    return fitted


def half_life(rho: float) -> float | None:
    phi = 1.0 + rho
    return float(-math.log(2) / math.log(phi)) if 0 < phi < 1 else None


def funding_p95(symbol: str, start: int, end: int) -> float:
    times, rates = r26.FUNDING[symbol]
    left = int(np.searchsorted(times, start, side="left"))
    right = int(np.searchsorted(times, end, side="left"))
    selected = np.abs(rates[left:right])
    return float(np.quantile(selected, 0.95)) if selected.size else math.inf


def projected_cost(
    left_symbol: str,
    right_symbol: str,
    left_price: float,
    right_price: float,
    beta: float,
    scale: float,
    start: int,
    end: int,
) -> dict[str, Any]:
    left_weight = 1.0 / (1.0 + abs(beta))
    right_weight = abs(beta) / (1.0 + abs(beta))
    left_qty, left_gross = r26.resolved_quantity(left_symbol, left_price, 100.0 * left_weight)
    right_qty, right_gross = r26.resolved_quantity(right_symbol, right_price, 100.0 * right_weight)
    total = left_gross + right_gross
    actual_left = left_gross / total
    actual_right = right_gross / total
    mismatch = max(
        abs(actual_left - left_weight) / max(left_weight, 1e-12),
        abs(actual_right - right_weight) / max(right_weight, 1e-12),
    )
    fee_slippage = 2 * total * 6.0 / 10_000
    legging = total * 2.0 / 10_000
    funding = 3 * (
        funding_p95(left_symbol, start, end) * left_gross
        + funding_p95(right_symbol, start, end) * right_gross
    )
    cost = fee_slippage + legging + funding
    edge = total * 1.75 * scale / max(1.0 + abs(beta), 1.0)
    return {
        "left_quantity": left_qty,
        "right_quantity": right_qty,
        "left_gross": left_gross,
        "right_gross": right_gross,
        "target_weights": [left_weight, right_weight],
        "resolved_weight_mismatch_ratio": mismatch,
        "projected_edge_quote": edge,
        "all_in_cost_quote": cost,
        "edge_cost_ratio": edge / cost if cost > 0 else math.inf,
        "passed": bool(mismatch <= 0.05 and edge / cost >= 2),
    }


def pair_fit(variant: str, anchor: int, left_symbol: str, right_symbol: str) -> dict[str, Any]:
    start = anchor - FORMATION_DAYS * r26.DAY_MS
    times, left, right = aligned_pair(left_symbol, right_symbol, start, anchor)
    expected = FORMATION_DAYS * 24
    base = {
        "pair_id": f"{left_symbol}__{right_symbol}",
        "left": left_symbol,
        "right": right_symbol,
        "variant": variant,
        "fit_start_ms": start,
        "fit_cutoff_ms": anchor,
        "sample_count": int(times.size),
        "sample_coverage": float(times.size / expected),
    }
    if times.size != expected or np.any(left <= 0) or np.any(right <= 0):
        return {**base, "finite_fit": False, "reason": "incomplete_or_invalid_prices"}
    y = np.log(left)
    x = np.log(right)
    design = np.column_stack((np.ones(x.size), x))
    intercept, beta = np.linalg.lstsq(design, y, rcond=None)[0]
    residual = y - intercept - beta * x
    center = float(np.median(residual))
    scale = float(1.4826 * np.median(np.abs(residual - center)))
    if not all(map(math.isfinite, (intercept, beta, center, scale))) or beta <= 0 or scale <= 0:
        return {**base, "finite_fit": False, "reason": "nonfinite_nonpositive_beta_or_scale"}
    snapshot_id = snapshot_path(variant, anchor).stem
    seed_payload = f"{snapshot_id}{left_symbol}__{right_symbol}{variant}".encode()
    seed = int.from_bytes(hashlib.sha256(seed_payload).digest()[:8], "big")
    threshold = bootstrap_threshold(residual, variant, seed)
    if threshold is None:
        return {**base, "finite_fit": False, "reason": "threshold_or_bootstrap_failure"}
    cost = projected_cost(left_symbol, right_symbol, float(left[-1]), float(right[-1]), float(beta), scale, start, anchor)
    value = {
        **base,
        "finite_fit": True,
        "orientation": "y=min(symbol),x=max(symbol)",
        "snapshot_id": snapshot_id,
        "intercept": float(intercept),
        "beta": float(beta),
        "residual_center": center,
        "residual_scale": scale,
        "formation_last_residual": float(residual[-1]),
        "formation_last_delta": float(residual[-1] - residual[-2]),
        "threshold": threshold,
        "projected_cost": cost,
        "minimum_liquidity": min(
            r26.LIQUIDITY[(left_symbol, anchor, FORMATION_DAYS)]["median_daily_quote_volume"],
            r26.LIQUIDITY[(right_symbol, anchor, FORMATION_DAYS)]["median_daily_quote_volume"],
        ),
        "formation_data_hash": hashlib.sha256(
            np.column_stack((times, left, right)).astype("<f8", copy=False).tobytes()
        ).hexdigest(),
        "outer_return_read": False,
    }
    value["model_hash"] = model_hash(value)
    return value


def bh(p_values: list[float], q: float = 0.05) -> list[bool]:
    return r26.bh(p_values, q)


def reliable(rho: float, support: float) -> tuple[bool, float | None]:
    value = half_life(rho)
    return bool(-1 < rho < 0 and support >= 0.90 and value is not None and 2 <= value <= 168), value


def snapshot_path(variant: str, anchor: int) -> Path:
    date = r26.iso(anchor)[:10].replace("-", "")
    return OUTPUT_ROOT / "fit-snapshots" / "t1" / variant.lower() / f"{variant.lower()}-42d-{date}.json"


def build_snapshot(task: tuple[str, int]) -> dict[str, Any]:
    variant, anchor = task
    path = snapshot_path(variant, anchor)
    identity = {"source": IDENTITY, "variant": variant, "anchor": anchor}
    if RESUME and path.exists():
        try:
            existing = json.loads(path.read_text())
            if existing.get("identity") == identity:
                return {"name": path.stem, "path": str(path), "sha256": r26.sha256_file(path), "resumed": True}
        except (OSError, json.JSONDecodeError):
            pass
    universe, liquidity = r26.top20(anchor, FORMATION_DAYS)
    fits = [pair_fit(variant, anchor, *pair) for pair in combinations(sorted(universe), 2)]
    finite = [row for row in fits if row.get("finite_fit")]
    accepted = bh([float(row["threshold"]["joint_bootstrap_p_value"]) for row in finite]) if finite else []
    candidates = []
    for row, passed in zip(finite, accepted, strict=True):
        threshold = row["threshold"]
        reliable_1, half_1 = reliable(float(threshold["rho_1"]), float(threshold["rho_1_negative_support"]))
        reliable_2, half_2 = reliable(float(threshold["rho_2"]), float(threshold["rho_2_negative_support"]))
        row["bh_q05_passed"] = bool(passed)
        row["regime_1_reliable"] = reliable_1
        row["regime_2_reliable"] = reliable_2
        row["regime_1_half_life_hours"] = half_1
        row["regime_2_half_life_hours"] = half_2
        row["reliable_regime_count"] = int(reliable_1) + int(reliable_2)
        row["admission_passed"] = bool(
            passed
            and row["reliable_regime_count"] >= 1
            and threshold["rho_1"] <= 0.05
            and threshold["rho_2"] <= 0.05
            and row["projected_cost"]["passed"]
        )
        if row["admission_passed"]:
            candidates.append(row)
    candidates.sort(key=lambda row: (
        -row["reliable_regime_count"],
        row["threshold"]["joint_bootstrap_p_value"],
        max(
            value for value in (row["regime_1_half_life_hours"], row["regime_2_half_life_hours"])
            if value is not None
        ),
        row["threshold"]["asymmetry_wald_p_value"],
        -row["projected_cost"]["edge_cost_ratio"],
        -row["minimum_liquidity"],
        row["pair_id"],
    ))
    for rank, row in enumerate(candidates):
        row["score"] = float(len(candidates) - rank)
        row["rank_ordinal"] = rank + 1
    matching = r26.exact_matching(candidates, max_pairs=3)
    snapshot = {
        "schema_version": 1,
        "fingerprint": f"R28-T1-{variant}-ASYMMETRIC-ERROR-CORRECTION-MARTIN",
        "identity": identity,
        "variant": variant,
        "snapshot_id": path.stem,
        "roll_anchor_ms": anchor,
        "fit_start_ms": anchor - FORMATION_DAYS * r26.DAY_MS,
        "fit_cutoff_ms": anchor,
        "eligible_universe": universe,
        "liquidity": liquidity,
        "pair_fit_denominator": len(fits),
        "finite_fit_count": len(finite),
        "bh_pass_count": sum(row.get("bh_q05_passed", False) for row in finite),
        "admitted_pair_count": len(candidates),
        "pair_fits": fits,
        "exact_matching": matching,
        "outer_return_read": False,
    }
    atomic_json(path, snapshot)
    return {"name": path.stem, "path": str(path), "sha256": r26.sha256_file(path), "resumed": False}


def main() -> None:
    global OUTPUT_ROOT, RESUME, IDENTITY
    arguments = parse_args()
    started = time.time()
    OUTPUT_ROOT = Path(arguments.artifact_root).resolve()
    RESUME = arguments.resume
    market_path = Path(arguments.market_data).resolve()
    funding_path = Path(arguments.funding_data).resolve()
    IDENTITY = {
        "source_commit": OUTPUT_ROOT.name,
        "market_path": str(market_path),
        "market_bytes": market_path.stat().st_size,
        "policy_sha256": r26.sha256_file(POLICY_PATH),
        "bootstrap_replications": BOOTSTRAP_REPLICATIONS,
        "bootstrap_block_bars": BOOTSTRAP_BLOCK,
    }
    anchors = r26.anchors()
    r26.FILTERS, exchange = r26.read_exchange_info(r26.EXCHANGE_INFO)
    r26.FUNDING, funding = r26.load_funding(funding_path)
    r26.SIGNALS, r26.LIQUIDITY, market = r26.load_market(
        market_path, ["1h"], anchors, [FORMATION_DAYS]
    )
    tasks = [(variant, anchor) for variant in ("TAR", "MTAR") for anchor in anchors]
    context = get_context("fork")
    with context.Pool(arguments.workers) as pool:
        rows = list(pool.imap_unordered(build_snapshot, tasks, chunksize=1))
    rows.sort(key=lambda row: row["name"])
    for variant in ("tar", "mtar"):
        selected = [row for row in rows if row["name"].startswith(variant)]
        manifest = {
            "schema_version": 1,
            "phase": f"{variant}-fit",
            "source_commit": OUTPUT_ROOT.name,
            "snapshot_count": len(selected),
            "snapshots": selected,
            "workers": arguments.workers,
            "blas_threads": {name: os.environ.get(name) for name in (
                "OPENBLAS_NUM_THREADS", "OMP_NUM_THREADS", "MKL_NUM_THREADS"
            )},
            "wall_seconds": time.time() - started,
            "peak_rss_kib": peak_rss_kib(),
            "market": market,
            "funding_sha256": funding["sha256"],
            "exchange_info_sha256": exchange["file_sha256"],
            "outer_return_read": False,
        }
        atomic_json(OUTPUT_ROOT / "fit-snapshot-manifests" / f"{variant}.json", manifest)
        atomic_json(REPO_ARTIFACT / "fit-snapshot-manifests" / f"{variant}.json", manifest)
    runtime = {
        "argv": sys.argv,
        "pid": os.getpid(),
        "start_epoch": started,
        "end_epoch": time.time(),
        "wall_seconds": time.time() - started,
        "peak_rss_kib": peak_rss_kib(),
        "workers": arguments.workers,
        "blas_threads": {name: os.environ.get(name) for name in (
            "OPENBLAS_NUM_THREADS", "OMP_NUM_THREADS", "MKL_NUM_THREADS"
        )},
        "exit_code": 0,
        "resume": arguments.resume,
        "source_commit": OUTPUT_ROOT.name,
        "data_hash": market["sha256"],
        "policy_hash": IDENTITY["policy_sha256"],
    }
    atomic_json(OUTPUT_ROOT / "runtime" / "tar-mtar-fit.json", runtime)
    print(json.dumps({"phase": arguments.phase, "snapshots": len(rows), "wall_seconds": runtime["wall_seconds"]}))


if __name__ == "__main__":
    main()
