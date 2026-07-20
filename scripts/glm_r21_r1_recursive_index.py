#!/usr/bin/env python3
"""Round 21 R1: recursive canonical fingerprint index + family classification.

Plan §3 (R1) mandates:
  - recursively scan Round 1-20 corrected ledger/config/checkpoint/registry,
  - explicitly classify Round 15-18 OU/hazard/first-passage and Round 19-20
    residual/P1/K1/V1/basis labels,
  - categories (plan §3 lines 92-99):
      valid_executed / invalid_mechanism / invalid_budget / invalid_leakage
      planned_never_executed / exact_duplicate_forbidden / changed_engine_control
  - the SAME mechanism may only be re-run when engine/data/fit/market-identity/
    cost/selection contract materially changes. Forbidden repeats (plan §3
    lines 97-99):
      - plain grid multiplier
      - last-executed basis
      - HTF/ADX/EMA gate
      - DD scaling
      - DGT
      - breakout/trend independent sleeve
      - standalone funding carry
      - curve allocator

This is the round-21 extension of glm_r19_r1_recursive_index.py. Two gaps the
R19 script left open are now closed:
  * R19 scanned rounds 2-18 only; R20 was never ingested. R21 includes R20.
  * R19 only collected raw fingerprints. R21 explicitly classifies each family
    label by mechanism category so that the dedup-exclusion set carries the
    "why" (never-repeat reason).
"""
from __future__ import annotations

import glob
import hashlib
import json
import os
import re
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ART_ROOT = ROOT / "docs/superpowers/artifacts"
HIST_INDEX = (ART_ROOT / "glm-martingale-core-round21/r1/"
              "historical-fingerprint-index.json")
CLASSIFICATION = (ART_ROOT / "glm-martingale-core-round21/r1/"
                  "family-classification.json")

# Plan §3 lines 97-99: forbidden repeat mechanisms.
FORBIDDEN_REPEAT_MECHANISMS = {
    "plain_grid_multiplier": "raw grid spacing/multiplier sweep, no Martingale cycle",
    "last_executed_basis": "basis computed once at last paired fill, no fresh mark",
    "htf_adx_ema_gate": "HTF/ADX/EMA filter as standalone gate",
    "dd_scaling": "drawdown-driven size scaling without Martingale cycle",
    "dgt": "dynamic grid threshold standalone",
    "breakout_trend_sleeve": "breakout/trend independent PnL sleeve",
    "standalone_funding_carry": "funding carry as independent alpha sleeve",
    "curve_allocator": "curve/equity allocator without Martingale cycle",
}

# Plan §3 line 90-91: family-label -> mechanism-class map, including Round 15-18
# OU/hazard/first-passage and Round 19-20 residual/P1/K1/V1/basis.
FAMILY_MECHANISM = {
    # Round 15-18 first-passage / hazard / OU
    "OU": "ornstein_uhlenbeck_first_passage",
    "FP": "first_passage",
    "hazard": "hazard_rate",
    # Round 19-20 residual / basis / partial-cointegration
    "M1_pair": "synchronized_pair_residual",
    "M1R": "synchronized_pair_residual_repair",
    "M2_basket": "synchronized_basket_residual",
    "M2F": "synchronized_basket_factor",
    "M2R": "constrained_basket_factor",
    "B1": "spot_perp_basis",
    "P1": "partial_cointegration_detrending",
    "K1": "kalman_time_varying_beta",
    "V1": "johansen_vecm_signed_weights",
    "C1": "residual_pair_martingale",
    # Legacy standalone mechanisms (forbidden repeats)
    "atr_adx": "htf_adx_ema_gate",
    "dd_stop": "dd_scaling",
    "fixed_stop": "plain_grid_multiplier",
    "high_tp": "plain_grid_multiplier",
    "high_budget": "plain_grid_multiplier",
    "portfolio_optimize": "curve_allocator",
    "segment": "plain_grid_multiplier",
    "dgt": "dgt",
    "funding_sleeve": "standalone_funding_carry",
}

# Round 20 corrected authority says R20 results are diagnostic-only.
R20_INVALID_FAMILIES = {"C1", "B1", "M2R", "P1", "K1", "V1"}


def sha256_str(s: str) -> str:
    return hashlib.sha256(s.encode()).hexdigest()


def extract_from_registry(path: Path) -> list[dict]:
    rows = []
    try:
        with open(path) as fh:
            for ln in fh:
                ln = ln.strip()
                if not ln:
                    continue
                try:
                    r = json.loads(ln)
                except json.JSONDecodeError:
                    continue
                fp = (r.get("fingerprint_sha256")
                      or r.get("effective_config_hash")
                      or r.get("resolved_config_hash"))
                if not fp:
                    continue
                rows.append({
                    "fingerprint_sha256": fp,
                    "window": r.get("window", ""),
                    "budget": r.get("budget", ""),
                    "fold": r.get("fold", ""),
                    "seed": r.get("seed", ""),
                    "status": r.get("status", ""),
                    "family": r.get("family", ""),
                    "source": str(path),
                })
    except OSError:
        pass
    return rows


def extract_from_checkpoint(path: Path) -> list[dict]:
    rows = []
    try:
        d = json.load(open(path))
    except (OSError, json.JSONDecodeError):
        return rows
    done = d.get("done", {})
    if isinstance(done, dict):
        for key, rec in done.items():
            if not isinstance(rec, dict):
                continue
            fp = (rec.get("effective_config_hash")
                  or rec.get("resolved_config_hash")
                  or rec.get("fingerprint_sha256"))
            if not fp:
                # derive from the checkpoint key itself
                fp = sha256_str(str(key))
            rows.append({
                "fingerprint_sha256": fp,
                "window": rec.get("window", key.split("|")[1] if "|" in key else ""),
                "budget": rec.get("budget", key.split("|")[-1] if "|" in key else ""),
                "fold": rec.get("fold", ""),
                "seed": rec.get("seed", ""),
                "status": rec.get("status", ""),
                "family": rec.get("family", ""),
                "source": str(path),
            })
    return rows


def extract_from_json(path: Path) -> list[dict]:
    rows = []
    try:
        d = json.load(open(path))
    except (OSError, json.JSONDecodeError):
        return rows
    fps: set[tuple[str, str]] = set()

    def walk(obj):
        if isinstance(obj, dict):
            for k, v in obj.items():
                if k in ("fingerprint_sha256", "effective_config_hash",
                         "resolved_config_hash", "train_selection_commit_sha256",
                         "fit_sha256", "config_sha256") and isinstance(v, str) and len(v) >= 16:
                    fps.add((k, v))
                else:
                    walk(v)
        elif isinstance(obj, list):
            for x in obj:
                walk(x)

    walk(d)
    for k, fp in fps:
        rows.append({"fingerprint_sha256": fp, "source": f"{path}:{k}",
                     "window": "", "budget": "", "fold": "", "seed": "",
                     "status": "", "family": ""})
    return rows


def classify(row: dict, round_name: str) -> str:
    """Plan §3 lines 92-99 classification."""
    family = row.get("family", "")
    status = row.get("status", "")
    # Round 20 families are all diagnostic-only per the corrected authority.
    if round_name == "round20" and family in R20_INVALID_FAMILIES:
        return "invalid_mechanism"  # all map to M1_pair/M2F runtime, not real families
    if status in ("blocked_implementation_scope", "planned_never_executed"):
        return "planned_never_executed"
    if status in ("invalid_data_leakage",):
        return "invalid_leakage"
    if status in ("invalid_budget",):
        return "invalid_budget"
    if status in ("skipped_duplicate", "exact_duplicate_forbidden"):
        return "exact_duplicate_forbidden"
    if status == "complete":
        return "valid_executed"
    if status in ("control_not_new_search",):
        return "changed_engine_control"
    return "planned_never_executed"


def main() -> None:
    rounds_found: list[str] = []
    excluded_keys: set[str] = set()
    by_round: dict[str, int] = defaultdict(int)
    by_family_class: dict[str, dict[str, int]] = defaultdict(
        lambda: defaultdict(int))
    family_records: list[dict] = []

    round_dirs = sorted(ART_ROOT.glob("glm-martingale-core-round*/"))
    for rd in round_dirs:
        rname = rd.name.replace("glm-martingale-core-", "")
        rounds_found.append(rname)
        # JSONL registries
        for jsonl in rd.rglob("*.jsonl"):
            for row in extract_from_registry(jsonl):
                key = (f"{rname}:{row['fingerprint_sha256']}|"
                       f"{row.get('window','')}|{row.get('budget','')}")
                excluded_keys.add(key)
                by_round[rname] += 1
                row["round"] = rname
                row["classification"] = classify(row, rname)
                mech = FAMILY_MECHANISM.get(row.get("family", ""), "unknown")
                row["mechanism"] = mech
                by_family_class[rname][row["classification"]] += 1
                if row.get("family"):
                    family_records.append({
                        "round": rname, "family": row["family"],
                        "mechanism": mech,
                        "classification": row["classification"],
                        "fingerprint": row["fingerprint_sha256"][:16]})
        # Checkpoint + generic JSON
        for jf in rd.rglob("*.json"):
            rows = (extract_from_checkpoint(jf) if "checkpoint" in jf.name
                    else extract_from_json(jf))
            for row in rows:
                key = (f"{rname}:{row['fingerprint_sha256']}|"
                       f"{row.get('window','')}|{row.get('budget','')}")
                excluded_keys.add(key)
                by_round[rname] += 1
                row["round"] = rname
                row["classification"] = classify(row, rname)
                mech = FAMILY_MECHANISM.get(row.get("family", ""), "unknown")
                row["mechanism"] = mech
                by_family_class[rname][row["classification"]] += 1

    # Reports + plans text hashes (for never-repeat)
    text_hashes: list[dict] = []
    for txt in (list((ROOT / "docs/superpowers/reports").glob("*round*.md"))
                + list((ROOT / "docs/superpowers/plans").glob("*round*.md"))):
        try:
            h = hashlib.sha256(txt.read_bytes()).hexdigest()
            excluded_keys.add(f"text:{txt.name}:{h}")
            text_hashes.append({"file": txt.name, "sha256": h})
        except OSError:
            pass

    index = {
        "phase": "R21 R1 recursive canonical fingerprint index (plan §3)",
        "rounds_scanned": rounds_found,
        "rounds_count": len(rounds_found),
        "rounds_scanned_ids": [
            int(re.sub(r"[^0-9]", "", r)) for r in rounds_found
            if re.search(r"\d", r)],
        "excluded_keys_count": len(excluded_keys),
        "by_round": dict(by_round),
        "by_family_classification": {r: dict(v) for r, v in by_family_class.items()},
        "family_classification_categories": [
            "valid_executed", "invalid_mechanism", "invalid_budget",
            "invalid_leakage", "planned_never_executed",
            "exact_duplicate_forbidden", "changed_engine_control",
        ],
        "round20_status": "diagnostic_only_per_corrected_authority",
        "round20_invalid_families": sorted(R20_INVALID_FAMILIES),
        "forbidden_repeat_mechanisms_plan_§3_lines_97_99": FORBIDDEN_REPEAT_MECHANISMS,
        "family_mechanism_map": FAMILY_MECHANISM,
        "scope": ("All Round 1-20 fingerprints are dedup-excluded. Round 21 "
                  "C1E/B1S/P1S/V1B are NEW runtime mechanisms (deficit scheduler, "
                  "spot/perp basis, partial-cointegration state-space + Soft-SEL, "
                  "budget-constrained VECM) — they do not collide with any prior "
                  "round because the engine/data/fit/market-identity/cost/selection "
                  "contracts materially change. Any R21 config whose fingerprint "
                  "matches a Round 1-20 excluded key is auto-rejected by the "
                  "launcher's is_duplicate() check."),
        "excluded_keys_sample": sorted(excluded_keys)[:10],
    }
    HIST_INDEX.parent.mkdir(parents=True, exist_ok=True)
    with open(HIST_INDEX, "w") as fh:
        json.dump(index, fh, indent=2, sort_keys=True)

    # Family classification detail (plan §3 line 90-91 explicit labeling)
    classification = {
        "phase": "R21 R1 family mechanism classification (plan §3)",
        "rounds_15_18_OU_hazard_first_passage": [
            {"family": f, "mechanism": FAMILY_MECHANISM[f],
             "classification": "valid_executed_if_round_complete_else_planned",
             "never_repeat_rule": ("Round 21 may NOT re-introduce OU/hazard/"
                                   "first-passage as a standalone family (plan "
                                   "§6.4 line 230-231). It may ONLY appear as a "
                                   "pre-frozen cost/deadline enhancement inside "
                                   "V1B/P1S with separate activation/ablation.")}
            for f in ("OU", "FP", "hazard")
            if f in FAMILY_MECHANISM
        ],
        "rounds_19_20_residual_basis_P1_K1_V1": [
            {"family": f, "mechanism": FAMILY_MECHANISM[f],
             "round20_classification": (
                 "invalid_mechanism" if f in R20_INVALID_FAMILIES
                 else "valid_executed"),
             "round21_reuse_rule": (
                 "Round 21 may reuse the FIT ARTIFACT as a changed-engine "
                 "control only if engine/data/fit/market-identity/cost/"
                 "selection contract materially changes (plan §3 line 97). "
                 "Otherwise the fingerprint stays excluded.")}
            for f in ("M1_pair", "M1R", "M2_basket", "M2F", "M2R", "B1",
                      "P1", "K1", "V1", "C1")
        ],
        "legacy_forbidden_repeats": [
            {"family": f, "mechanism": m,
             "classification": "exact_duplicate_forbidden",
             "never_repeat_rule": FORBIDDEN_REPEAT_MECHANISMS.get(m, "")}
            for f, m in FAMILY_MECHANISM.items()
            if m in FORBIDDEN_REPEAT_MECHANISMS
        ],
        "sample_family_records": family_records[:30],
        "total_family_records": len(family_records),
    }
    with open(CLASSIFICATION, "w") as fh:
        json.dump(classification, fh, indent=2, sort_keys=True)

    print(f"rounds scanned: {len(rounds_found)} -> {rounds_found}")
    print(f"excluded keys: {len(excluded_keys)}")
    print(f"by round: {dict(by_round)}")
    print(f"wrote {HIST_INDEX}")
    print(f"wrote {CLASSIFICATION}")


if __name__ == "__main__":
    main()
