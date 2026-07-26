#!/usr/bin/env python3
"""Round 27 train-only C0/C1 Copula and P1 persistence snapshots."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import resource
import sys
import time
import warnings
from itertools import combinations
from multiprocessing import get_context
from pathlib import Path
from typing import Any

import numpy as np
import statsmodels
from statsmodels.tsa.statespace.structural import UnobservedComponents

sys.path.insert(0, str(Path(__file__).resolve().parent))
import r26_fit_snapshots as r26  # noqa: E402


FINGERPRINTS = {
    "c0": "R27-C0-SOURCE-EG-COPULA-MARTIN",
    "c1": "R27-C1-STAGED-ROBUST-COPULA-MARTIN",
    "p1": "R27-P1-PBD-FINITE-PERSISTENCE-MARTIN",
}
REPO_ARTIFACT = Path("docs/superpowers/artifacts/glm-martingale-core-round27")
POLICY_PATH = REPO_ARTIFACT / "round27-policy-manifest.json"
OUTPUT_ROOT = Path(".")
RESUME = False
INPUT_IDENTITY: dict[str, str] = {}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=("d0-c0-c1", "p1-pbd"), required=True)
    parser.add_argument("--artifact-root", required=True)
    parser.add_argument("--market-data", default="data/market_data_full.db")
    parser.add_argument("--funding-data", default="data/funding_rates.db")
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--resume", action="store_true")
    return parser.parse_args()


def atomic_json(path: Path, value: Any) -> str:
    return r26.atomic_json(path, value)


def peak_rss_kib() -> int:
    return int(max(
        resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
        resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
    ))


def model_hash(value: dict[str, Any]) -> str:
    return hashlib.sha256(r26.canonical_bytes(r26.sanitize(value))).hexdigest()


def snapshot_path(phase: str, task: tuple[str, int, int]) -> Path:
    frequency, formation, anchor = task
    date = r26.iso(anchor)[:10].replace("-", "")
    return OUTPUT_ROOT / "fit-snapshots" / phase / frequency / f"{formation}d" / f"{frequency}-{formation}d-{date}.json"


def snapshot_identity(phase: str, task: tuple[str, int, int]) -> dict[str, Any]:
    return {"phase": phase, "task": list(task), "inputs": INPUT_IDENTITY}


def resumed_snapshot(phase: str, task: tuple[str, int, int]) -> dict[str, Any] | None:
    path = snapshot_path(phase, task)
    if not RESUME or not path.exists():
        return None
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return None
    if value.get("identity") != snapshot_identity(phase, task):
        return None
    return {"name": path.stem, "path": str(path), "sha256": r26.sha256_file(path), "resumed": True}


def hac_break_statistic(residual: np.ndarray) -> float:
    midpoint = residual.size // 2
    centered = residual - np.mean(residual)
    lag = max(1, int(residual.size ** (1 / 3)))
    gamma0 = float(np.dot(centered, centered) / residual.size)
    long_run = gamma0
    for offset in range(1, lag + 1):
        covariance = float(np.dot(centered[offset:], centered[:-offset]) / residual.size)
        long_run += 2 * (1 - offset / (lag + 1)) * covariance
    standard_error = math.sqrt(max(long_run, 1e-18) * (1 / midpoint + 1 / (residual.size - midpoint)))
    return float(abs(np.mean(residual[:midpoint]) - np.mean(residual[midpoint:])) / standard_error)


def enrich_leg(public: dict[str, Any], private: dict[str, Any]) -> None:
    if not private:
        public.update({
            "hac_break_statistic": None,
            "robust_beta_drift": None,
            "expected_convergence_bars": None,
        })
        return
    public["hac_break_statistic"] = hac_break_statistic(private["residual"])
    public["robust_beta_drift"] = abs(public["second_half_beta"] - public["first_half_beta"]) / max(
        abs(public["beta"]), 0.10
    )
    public["expected_convergence_bars"] = public["half_life_bars"]


def deterministic_cost(
    left_symbol: str,
    right_symbol: str,
    left: dict[str, Any],
    right: dict[str, Any],
    anchor: int,
    formation: int,
) -> dict[str, Any]:
    left_price = float(left["alt"][-1])
    right_price = float(right["alt"][-1])
    left_qty, left_gross = r26.resolved_quantity(left_symbol, left_price)
    right_qty, right_gross = r26.resolved_quantity(right_symbol, right_price)
    mismatch = abs(left_gross - right_gross) / max(left_gross, right_gross)
    total_gross = left_gross + right_gross
    funding = (
        r26.funding_p95(left_symbol, anchor - formation * r26.DAY_MS, anchor)
        + r26.funding_p95(right_symbol, anchor - formation * r26.DAY_MS, anchor)
    ) * max(left_gross, right_gross) * 3
    fee_slippage = total_gross * 2 * 6 / 10_000
    legging = total_gross * 2 / 10_000
    round_trip = fee_slippage + legging + funding
    threshold_move = total_gross * max(
        float(np.std(left["residual"], ddof=1)),
        float(np.std(right["residual"], ddof=1)),
    )
    return {
        "decision": bool(mismatch <= 0.05 and threshold_move >= 2 * round_trip),
        "formula": "resolved_pair_gross*max(reference_residual_sigma)>=2*round_trip_cost",
        "left_quantity": left_qty,
        "right_quantity": right_qty,
        "left_gross": left_gross,
        "right_gross": right_gross,
        "gross_mismatch_ratio": mismatch,
        "threshold_move_quote": threshold_move,
        "round_trip_cost_quote": round_trip,
        "coverage_ratio": threshold_move / round_trip if round_trip > 0 else math.inf,
    }


def c0_score(left: dict[str, Any], right: dict[str, Any], copula: dict[str, Any], liquidity: dict[str, Any]) -> float:
    stationarity = -math.log10(max(left["eg_coint_p_value"], right["eg_coint_p_value"], 1e-12))
    left_hl = left.get("half_life_bars")
    right_hl = right.get("half_life_bars")
    speed = 0.0
    if left_hl is not None and right_hl is not None and math.isfinite(left_hl) and math.isfinite(right_hl):
        speed = 1 / max(left_hl + right_hl, 1)
    liquid = math.log1p(min(
        liquidity[left["symbol"]]["median_daily_quote_volume"],
        liquidity[right["symbol"]]["median_daily_quote_volume"],
    )) / 100
    return float(stationarity + speed - copula["aic"] / copula["sample_count"] + liquid)


def pair_model(
    arm: str,
    frequency: str,
    formation: int,
    anchor: int,
    left_public: dict[str, Any],
    right_public: dict[str, Any],
    left_private: dict[str, Any],
    right_private: dict[str, Any],
    liquidity: dict[str, dict[str, Any]],
) -> dict[str, Any] | None:
    left_symbol, right_symbol = sorted([left_public["symbol"], right_public["symbol"]])
    if left_symbol != left_public["symbol"]:
        left_public, right_public = right_public, left_public
        left_private, right_private = right_private, left_private
    copula, h_values = r26.fit_copula(left_private, right_private, anchor)
    deterministic = deterministic_cost(
        left_symbol, right_symbol, left_private, right_private, anchor, formation
    )
    if not deterministic["decision"]:
        return None
    empirical = r26.pair_cost(
        frequency, anchor, formation, left_symbol, right_symbol,
        left_private, right_private, h_values,
    )
    empirical_e1 = bool(
        empirical["excursion_count"] >= 5
        and empirical["median_edge_cost_ratio"] >= 2
        and empirical["p25_all_in_edge_quote"] > 0
        and empirical["max_resolved_gross_mismatch_ratio"] <= 0.05
    )
    values = {
        "pair_id": f"{left_symbol}__{right_symbol}",
        "left": left_symbol,
        "right": right_symbol,
        "frequency": frequency,
        "formation_days": formation,
        "fit_cutoff_ms": anchor,
        "left_leg": {key: left_public.get(key) for key in (
            "intercept", "beta", "residual_sigma", "eg_coint_p_value", "kpss_p_value",
            "half_life_bars", "hac_break_statistic", "robust_beta_drift", "sorted_residuals",
        )},
        "right_leg": {key: right_public.get(key) for key in (
            "intercept", "beta", "residual_sigma", "eg_coint_p_value", "kpss_p_value",
            "half_life_bars", "hac_break_statistic", "robust_beta_drift", "sorted_residuals",
        )},
        "copula": copula,
        "deterministic_cost": deterministic,
        "empirical_cost": {**empirical, "e1_passed": empirical_e1},
        "formation_pair_crossings": empirical["excursion_count"],
        "minimum_liquidity": min(
            liquidity[left_symbol]["median_daily_quote_volume"],
            liquidity[right_symbol]["median_daily_quote_volume"],
        ),
        "previous_roll_base_pass": False,
        "expected_convergence_bars": max(
            left_public.get("expected_convergence_bars") or math.inf,
            right_public.get("expected_convergence_bars") or math.inf,
        ),
        "hac_break_statistic": max(left_public["hac_break_statistic"], right_public["hac_break_statistic"]),
        "robust_beta_drift": max(left_public["robust_beta_drift"], right_public["robust_beta_drift"]),
    }
    if arm == "c1":
        previous_anchor = anchor - r26.WEEK_MS
        previous_universe, _ = r26.top20(previous_anchor, formation)
        previous_pass = []
        for symbol in (left_symbol, right_symbol):
            if symbol not in previous_universe:
                previous_pass.append(False)
                continue
            prior, _ = r26.diagnostics(frequency, symbol, previous_anchor, formation)
            half_life = prior.get("half_life_bars")
            previous_pass.append(bool(
                prior.get("sample_coverage") == 1.0
                and prior.get("eg_coint_p_value", 1.0) <= 0.05
                and prior.get("kpss_p_value", 0.0) >= 0.05
                and half_life is not None
                and math.isfinite(half_life)
                and 2 <= half_life <= 168
            ))
        values["previous_roll_base_pass"] = all(previous_pass)
    values["score"] = c0_score(left_public, right_public, copula, liquidity)
    values["model_hash"] = model_hash(values)
    return values


def build_c_snapshot(task: tuple[str, int, int]) -> dict[str, Any]:
    resumed = resumed_snapshot("c0-c1", task)
    if resumed:
        return resumed
    frequency, formation, anchor = task
    universe, liquidity_rows = r26.top20(anchor, formation)
    liquidity = {row["symbol"]: row for row in liquidity_rows}
    legs = []
    private = {}
    for symbol in universe:
        public, values = r26.diagnostics(frequency, symbol, anchor, formation)
        enrich_leg(public, values)
        legs.append(public)
        private[symbol] = values
    accepted_fdr = r26.bh([float(leg.get("eg_coint_p_value", 1.0)) for leg in legs]) if legs else []
    for leg, accepted in zip(legs, accepted_fdr, strict=True):
        finite = all(math.isfinite(float(leg.get(field, math.nan))) for field in ("intercept", "beta", "residual_sigma"))
        leg["c0_raw_admission"] = bool(leg["sample_coverage"] == 1.0 and finite and leg["eg_coint_p_value"] <= 0.05)
        leg["c0_fdr_admission"] = bool(leg["sample_coverage"] == 1.0 and finite and accepted)
        half_life = leg.get("half_life_bars")
        leg["c1_base_admission"] = bool(
            leg["sample_coverage"] == 1.0
            and leg["eg_coint_p_value"] <= 0.05
            and leg["kpss_p_value"] >= 0.05
            and half_life is not None
            and math.isfinite(half_life)
            and 2 <= half_life <= 168
        )

    arms: dict[str, Any] = {}
    if formation == 21:
        for selector, field in (("RAW", "c0_raw_admission"), ("FDR", "c0_fdr_admission")):
            selected = [leg for leg in legs if leg[field]]
            edges = []
            for left, right in combinations(selected, 2):
                edge = pair_model("c0", frequency, formation, anchor, left, right, private[left["symbol"]], private[right["symbol"]], liquidity)
                if edge:
                    edges.append(edge)
            arms[f"C0-{selector}"] = {
                "admitted_alts": [row["symbol"] for row in selected],
                "pair_graph": edges,
                "exact_matching": r26.exact_matching(edges),
            }
    if frequency == "1h":
        selected = [leg for leg in legs if leg["c1_base_admission"]]
        base_edges = []
        for left, right in combinations(selected, 2):
            edge = pair_model("c1", frequency, formation, anchor, left, right, private[left["symbol"]], private[right["symbol"]], liquidity)
            if edge:
                base_edges.append(edge)
        ordered = sorted(base_edges, key=lambda edge: (
            not edge["previous_roll_base_pass"], edge["expected_convergence_bars"],
            edge["hac_break_statistic"], edge["robust_beta_drift"],
            edge["copula"]["aic"] / edge["copula"]["sample_count"],
            -edge["minimum_liquidity"], edge["pair_id"],
        ))
        for rank, edge in enumerate(ordered):
            edge["ordinal_score"] = len(ordered) - rank
            edge["score"] = float(edge["ordinal_score"])
        for cost_arm in ("E0", "E1"):
            eligible = [edge for edge in ordered if cost_arm == "E0" or edge["empirical_cost"]["e1_passed"]]
            arms[f"C1-{cost_arm}"] = {
                "admitted_alts": [row["symbol"] for row in selected],
                "pair_graph": eligible,
                "exact_matching": r26.exact_matching(eligible),
            }
    snapshot = {
        "schema_version": 1,
        "fingerprints": [FINGERPRINTS["c0"], FINGERPRINTS["c1"]],
        "identity": snapshot_identity("c0-c1", task),
        "frequency": frequency,
        "formation_days": formation,
        "roll_anchor_ms": anchor,
        "fit_start_ms": anchor - formation * r26.DAY_MS,
        "fit_cutoff_ms": anchor,
        "eligible_universe": universe,
        "liquidity": liquidity_rows,
        "legs": legs,
        "arms": arms,
        "outer_return_read": False,
    }
    path = snapshot_path("c0-c1", task)
    atomic_json(path, snapshot)
    return {"name": path.stem, "path": str(path), "sha256": r26.sha256_file(path), "resumed": False}


def pbd_fit(spread: np.ndarray) -> dict[str, Any]:
    centered = np.asarray(spread, dtype=float) - float(np.mean(spread))
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        full_model = UnobservedComponents(
            centered, level=True, stochastic_level=True, irregular=True, autoregressive=1
        )
        full = full_model.fit(disp=False, maxiter=100)
        rw = UnobservedComponents(centered, level=True, stochastic_level=True, irregular=True).fit(disp=False, maxiter=100)
        ar = UnobservedComponents(centered, irregular=True, autoregressive=1).fit(disp=False, maxiter=100)
    params = dict(zip(full.param_names, map(float, full.params), strict=True))
    rho = params["ar.L1"]
    sigma_m2 = params["sigma2.ar"]
    sigma_r2 = params["sigma2.level"]
    sigma_w2 = params["sigma2.irregular"]
    total = sigma_m2 + sigma_r2 + sigma_w2
    covariance = np.asarray(full.cov_params())
    diagnostics = bool(
        full.mle_retvals.get("converged", False)
        and np.all(np.isfinite(full.params))
        and np.all(np.isfinite(covariance))
    )
    admission = bool(
        diagnostics
        and 0.5 < rho < 1
        and sigma_m2 > 0
        and sigma_r2 >= 0
        and sigma_w2 >= 0
        and full.bic < rw.bic
        and full.bic < ar.bic
        and total > 0
        and sigma_m2 / total >= 0.5
    )
    finite_sigma = math.sqrt(sigma_m2 / max(1 - rho * rho, 1e-12))
    result = {
        "library": "statsmodels.tsa.statespace.UnobservedComponents",
        "specification": "stochastic random-walk level + AR(1) + irregular white noise",
        "params": params,
        "rho": rho,
        "sigma2_m": sigma_m2,
        "sigma2_r": sigma_r2,
        "sigma2_w": sigma_w2,
        "finite_variance_share": sigma_m2 / total if total > 0 else 0.0,
        "random_walk_variance_share": sigma_r2 / total if total > 0 else 0.0,
        "finite_half_life_bars": -math.log(2) / math.log(rho) if 0 < rho < 1 else math.inf,
        "finite_component_sigma": finite_sigma,
        "full_bic": float(full.bic),
        "rw_noise_bic": float(rw.bic),
        "ar_noise_bic": float(ar.bic),
        "bic_improvement": float(min(rw.bic, ar.bic) - full.bic),
        "optimizer_converged": bool(full.mle_retvals.get("converged", False)),
        "finite_diagnostics": diagnostics,
        "admission_passed": admission,
        "center": float(np.mean(spread)),
        "filtered_state_last": np.asarray(full.filtered_state[:, -1]).tolist(),
        "filtered_state_cov_last": np.asarray(full.filtered_state_cov[:, :, -1]).tolist(),
        "filtered_m_fixture": np.asarray(full.filtered_state[1, [0, len(centered) // 2, len(centered) - 1]]).tolist(),
    }
    result["model_hash"] = model_hash(result)
    return result


def pbd_candidates(universe: list[str], anchor: int) -> list[tuple[str, str, float]]:
    start = anchor - 7 * r26.DAY_MS
    normalized = {}
    for symbol in universe:
        times, values = r26.SIGNALS["5m"][symbol]
        left = int(np.searchsorted(times, start, side="left"))
        right = int(np.searchsorted(times, anchor, side="left"))
        selected = values[left:right]
        normalized[symbol] = selected / selected[0]
    distance = {}
    for left, right in combinations(universe, 2):
        value = float(np.mean((normalized[left] - normalized[right]) ** 2))
        distance[(left, right)] = value
    selected = set()
    for symbol in universe:
        neighbors = sorted(
            ((value, pair[1] if pair[0] == symbol else pair[0], pair) for pair, value in distance.items() if symbol in pair),
            key=lambda row: (row[0], row[1]),
        )[:5]
        selected.update(row[2] for row in neighbors)
    return [(left, right, distance[(left, right)]) for left, right in sorted(selected)[:100]]


def build_p_snapshot(task: tuple[str, int, int]) -> dict[str, Any]:
    resumed = resumed_snapshot("p1", task)
    if resumed:
        return resumed
    frequency, formation, anchor = task
    universe, liquidity_rows = r26.top20(anchor, formation)
    liquidity = {row["symbol"]: row for row in liquidity_rows}
    fits = []
    start = anchor - 7 * r26.DAY_MS
    for left, right, distance in pbd_candidates(universe, anchor):
        left_times, left_values = r26.SIGNALS["5m"][left]
        right_times, right_values = r26.SIGNALS["5m"][right]
        li = int(np.searchsorted(left_times, start, side="left")); lj = int(np.searchsorted(left_times, anchor, side="left"))
        ri = int(np.searchsorted(right_times, start, side="left")); rj = int(np.searchsorted(right_times, anchor, side="left"))
        if not np.array_equal(left_times[li:lj], right_times[ri:rj]) or lj - li != 2016:
            continue
        spread = np.log(left_values[li:lj]) - np.log(right_values[ri:rj])
        try:
            fitted = pbd_fit(spread)
        except Exception as error:
            fits.append({"pair_id": f"{left}__{right}", "admission_passed": False, "fit_error": str(error)})
            continue
        left_qty, left_gross = r26.resolved_quantity(left, float(left_values[lj - 1]))
        right_qty, right_gross = r26.resolved_quantity(right, float(right_values[rj - 1]))
        mismatch = abs(left_gross - right_gross) / max(left_gross, right_gross)
        row = {
            "pair_id": f"{left}__{right}", "left": left, "right": right,
            "fit_start_ms": start, "fit_cutoff_ms": anchor,
            "sample_count": int(spread.size), "normalized_price_distance": distance,
            "data_hash": hashlib.sha256(np.column_stack((left_times[li:lj], left_values[li:lj], right_values[ri:rj])).astype("<f8").tobytes()).hexdigest(),
            "left_quantity": left_qty, "right_quantity": right_qty,
            "left_gross": left_gross, "right_gross": right_gross,
            "gross_mismatch_ratio": mismatch,
            "minimum_liquidity": min(liquidity[left]["median_daily_quote_volume"], liquidity[right]["median_daily_quote_volume"]),
            **fitted,
        }
        row["admission_passed"] = bool(row["admission_passed"] and mismatch <= 0.05)
        fits.append(row)
    admitted = [row for row in fits if row.get("admission_passed")]
    admitted.sort(key=lambda row: (
        -row["bic_improvement"], -row["finite_variance_share"], row["random_walk_variance_share"],
        row["finite_half_life_bars"], -row["minimum_liquidity"], row["pair_id"],
    ))
    for rank, row in enumerate(admitted):
        row["score"] = float(len(admitted) - rank)
    snapshot = {
        "schema_version": 1,
        "fingerprint": FINGERPRINTS["p1"],
        "identity": snapshot_identity("p1", task),
        "frequency": frequency, "formation_days": formation,
        "roll_anchor_ms": anchor, "fit_start_ms": start, "fit_cutoff_ms": anchor,
        "eligible_universe": universe, "liquidity": liquidity_rows,
        "raw_pair_fit_count": len(fits), "pair_fits": fits,
        "exact_matching": r26.exact_matching(admitted),
        "outer_return_read": False,
    }
    path = snapshot_path("p1", task)
    atomic_json(path, snapshot)
    return {"name": path.stem, "path": str(path), "sha256": r26.sha256_file(path), "resumed": False}


def main() -> None:
    global OUTPUT_ROOT, RESUME, INPUT_IDENTITY
    args = parse_args()
    started = time.time()
    OUTPUT_ROOT = Path(args.artifact_root)
    OUTPUT_ROOT.mkdir(parents=True, exist_ok=True)
    RESUME = args.resume
    anchors = r26.anchors()
    if args.phase == "d0-c0-c1":
        frequencies, formations = ["1h", "5m"], [14, 21]
        tasks = [
            (frequency, formation, anchor)
            for frequency, formation in (("1h", 14), ("1h", 21), ("5m", 21))
            for anchor in anchors
        ]
        builder, manifest_name = build_c_snapshot, "c0-c1"
    else:
        frequencies, formations = ["5m"], [7]
        tasks = [("5m", 7, anchor) for anchor in anchors]
        builder, manifest_name = build_p_snapshot, "p1"
    r26.FILTERS, exchange = r26.read_exchange_info(r26.EXCHANGE_INFO)
    r26.FUNDING, funding_manifest = r26.load_funding(Path(args.funding_data))
    liquidity_anchors = (
        sorted(set([*anchors, *(anchor - r26.WEEK_MS for anchor in anchors)]))
        if args.phase == "d0-c0-c1"
        else anchors
    )
    r26.SIGNALS, r26.LIQUIDITY, market_manifest = r26.load_market(
        Path(args.market_data), frequencies, liquidity_anchors, formations
    )
    INPUT_IDENTITY = {
        "source_commit": OUTPUT_ROOT.name,
        "market_data_sha256": market_manifest["sha256"],
        "funding_data_sha256": funding_manifest["sha256"],
        "exchange_info_sha256": r26.sha256_file(r26.EXCHANGE_INFO),
        "policy_sha256": r26.sha256_file(POLICY_PATH),
    }
    if args.phase == "d0-c0-c1":
        gate = r26.d0_gate(market_manifest, funding_manifest, exchange)
        gate["fingerprint"] = "ROUND27_DATA_GATE"
        atomic_json(OUTPUT_ROOT / "checkpoints/d0.json", gate)
        atomic_json(REPO_ARTIFACT / "gates/d0.json", gate)
        atomic_json(REPO_ARTIFACT / "round27-data-manifest.json", {
            "market": market_manifest, "funding": funding_manifest,
            "exchange_info": exchange, "eligible_alts": gate["eligible_alts"],
        })
        if gate["status"] != "PASS":
            raise SystemExit(gate["status"])
    if args.workers > 1:
        with get_context("fork").Pool(args.workers) as pool:
            rows = list(pool.imap_unordered(builder, tasks, chunksize=1))
    else:
        rows = [builder(task) for task in tasks]
    rows.sort(key=lambda row: row["name"])
    finished = time.time()
    manifest = {
        "schema_version": 1, "phase": args.phase, "identity": INPUT_IDENTITY,
        "argv": sys.argv, "pid": os.getpid(), "workers": args.workers,
        "blas_threads": {
            "OPENBLAS_NUM_THREADS": os.environ.get("OPENBLAS_NUM_THREADS"),
            "OMP_NUM_THREADS": os.environ.get("OMP_NUM_THREADS"),
            "MKL_NUM_THREADS": os.environ.get("MKL_NUM_THREADS"),
        },
        "python": sys.version, "statsmodels": statsmodels.__version__,
        "start_utc": r26.iso(int(started * 1000)), "end_utc": r26.iso(int(finished * 1000)),
        "wall_seconds": finished - started, "peak_rss_kib": peak_rss_kib(), "exit_code": 0,
        "snapshot_count": len(rows), "snapshots": rows,
    }
    manifest_path = OUTPUT_ROOT / "fit-snapshot-manifests" / f"{manifest_name}.json"
    atomic_json(manifest_path, manifest)
    atomic_json(OUTPUT_ROOT / "checkpoints" / f"{args.phase}-fit-complete.json", {
        "phase": args.phase, "terminal": True, "snapshot_count": len(rows),
        "manifest_sha256": r26.sha256_file(manifest_path),
    })
    print(json.dumps({"phase": args.phase, "snapshots": len(rows), "wall_seconds": finished - started}))


if __name__ == "__main__":
    main()
