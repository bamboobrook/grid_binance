#!/usr/bin/env python3
"""Round 27 return-blind decomposition and historical fingerprint audit."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
from typing import Any


REPO_ARTIFACT = Path("docs/superpowers/artifacts/glm-martingale-core-round27")
EXPECTED = {
    "snapshot_count": 608,
    "leg_count": 9570,
    "cusum_pass": 0,
    "1h-14d": (123, 23, 8),
    "1h-21d": (116, 22, 8),
    "5m-14d": (24, 5, 0),
    "5m-21d": (21, 4, 1),
}
EXACT_CLOSED = "R26_EXACT_EG_KPSS_HALF_LIFE_RAW_CUSUM_BETA_DRIFT_TAIL20_CONJUNCTIVE_ADMISSION"
REOPENED = [
    "R27-C0-SOURCE-EG-COPULA-MARTIN",
    "R27-C1-STAGED-ROBUST-COPULA-MARTIN",
    "R27-P1-PBD-FINITE-PERSISTENCE-MARTIN",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--round26-root", required=True)
    parser.add_argument("--artifact-root", required=True)
    parser.add_argument("--resume", action="store_true")
    return parser.parse_args()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(8 * 1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def atomic_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(value, indent=2, sort_keys=True, allow_nan=False).encode() + b"\n"
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_bytes(payload)
    temporary.replace(path)


def decomposition(round26_root: Path) -> dict[str, Any]:
    manifest_path = round26_root / "fit-snapshot-manifests/g1.json"
    manifest = json.loads(manifest_path.read_text())
    rows: dict[str, dict[str, Any]] = {}
    snapshot_count = leg_count = cusum_pass = 0
    for item in manifest["snapshots"]:
        path = Path(item["path"])
        if sha256(path) != item["sha256"]:
            raise RuntimeError(f"snapshot hash mismatch: {path}")
        snapshot = json.loads(path.read_text())
        snapshot_count += 1
        key = f"{snapshot['frequency']}-{snapshot['formation_days']}d"
        group = rows.setdefault(key, {
            "leg_count": 0,
            "eg_kpss_half_life_legs": 0,
            "rolls_ge_2": 0,
            "rolls_ge_4": 0,
            "rolls": [],
        })
        passed = 0
        max_bars = 168 if snapshot["frequency"] == "1h" else 2016
        for leg in snapshot["legs"]:
            leg_count += 1
            group["leg_count"] += 1
            cusum_p = leg.get("cusum_p_value")
            if cusum_p is not None and float(cusum_p) >= 0.05:
                cusum_pass += 1
            half_life = leg.get("half_life_bars")
            base = bool(
                leg.get("sample_coverage") == 1.0
                and leg.get("eg_coint_p_value") is not None
                and float(leg["eg_coint_p_value"]) <= 0.05
                and leg.get("kpss_p_value") is not None
                and float(leg["kpss_p_value"]) >= 0.05
                and half_life is not None
                and math.isfinite(float(half_life))
                and 2 <= float(half_life) <= max_bars
            )
            if base:
                passed += 1
                group["eg_kpss_half_life_legs"] += 1
        group["rolls"].append({"roll_anchor_ms": snapshot["roll_anchor_ms"], "passed_legs": passed})
        group["rolls_ge_2"] += int(passed >= 2)
        group["rolls_ge_4"] += int(passed >= 4)

    observed = {
        "snapshot_count": snapshot_count,
        "leg_count": leg_count,
        "cusum_pass": cusum_pass,
        "groups": rows,
    }
    failures = []
    for field in ["snapshot_count", "leg_count", "cusum_pass"]:
        if observed[field] != EXPECTED[field]:
            failures.append(f"{field}:{observed[field]}!={EXPECTED[field]}")
    for key, expected in ((key, value) for key, value in EXPECTED.items() if isinstance(value, tuple)):
        actual = rows.get(key, {})
        values = (
            actual.get("eg_kpss_half_life_legs"),
            actual.get("rolls_ge_2"),
            actual.get("rolls_ge_4"),
        )
        if values != expected:
            failures.append(f"{key}:{values}!={expected}")
    return {
        "schema_version": 1,
        "status": "PASS" if not failures else "FAIL",
        "round26_root": str(round26_root),
        "snapshot_manifest_sha256": sha256(manifest_path),
        "expected": EXPECTED,
        "observed": observed,
        "violations": failures,
        "pnl_read": False,
    }


def historical_index() -> dict[str, Any]:
    root = Path("docs/superpowers/artifacts")
    entries = []
    seen = set()
    for path in sorted(root.rglob("*")):
        if not path.is_file() or path.stat().st_size > 10_000_000:
            continue
        if path.suffix not in {".json", ".jsonl", ".md"}:
            continue
        try:
            text = path.read_text(errors="replace")
        except OSError:
            continue
        lowered = text.lower()
        if not any(token in lowered for token in ("fingerprint", "pbd", "persistence", "cointegration", "copula")):
            continue
        identity = sha256(path)
        if identity in seen:
            continue
        seen.add(identity)
        entries.append({
            "path": str(path),
            "sha256": identity,
            "mentions_pbd_three_component": all(token in lowered for token in ("random walk", "ar(1)", "white noise")),
            "mentions_round27_exact": any(value.lower() in lowered for value in REOPENED),
        })
    exact_pbd_preexisting = any(
        row["mentions_pbd_three_component"] and "round27" not in row["path"] for row in entries
    )
    return {
        "schema_version": 1,
        "recursive_scope": "docs/superpowers/artifacts/glm-martingale-core-round1..26",
        "entry_count": len(entries),
        "entries": entries,
        "exact_pbd_fingerprint_preexisting": exact_pbd_preexisting,
        "decision": "FAIL_DUPLICATE_PBD" if exact_pbd_preexisting else "PASS_NEW_PBD_MECHANISM",
    }


def main() -> None:
    args = parse_args()
    raw = Path(args.artifact_root)
    raw.mkdir(parents=True, exist_ok=True)
    result = decomposition(Path(args.round26_root))
    index = historical_index()
    reopened = {
        "schema_version": 1,
        "closed_exact": [EXACT_CLOSED],
        "reopened_changed_mechanism": REOPENED,
        "round26_broad_family_closure_revoked": True,
        "pbd_dedup_passed": index["decision"] == "PASS_NEW_PBD_MECHANISM",
    }
    atomic_json(raw / "checkpoints/r0-round26-gate-decomposition.json", result)
    atomic_json(REPO_ARTIFACT / "r0-round26-gate-decomposition.json", result)
    atomic_json(REPO_ARTIFACT / "round27-historical-fingerprint-index.json", index)
    atomic_json(REPO_ARTIFACT / "round27-reopened-fingerprint-map.json", reopened)
    atomic_json(raw / "checkpoints/r0.json", {
        "status": "PASS" if result["status"] == "PASS" and reopened["pbd_dedup_passed"] else "FAIL",
        "decomposition_sha256": sha256(REPO_ARTIFACT / "r0-round26-gate-decomposition.json"),
        "historical_index_sha256": sha256(REPO_ARTIFACT / "round27-historical-fingerprint-index.json"),
    })
    if result["status"] != "PASS" or not reopened["pbd_dedup_passed"]:
        raise SystemExit("Round 27 R0 failed closed")
    print(json.dumps({"phase": "r0", "status": "PASS", "snapshots": 608, "legs": 9570}))


if __name__ == "__main__":
    main()
