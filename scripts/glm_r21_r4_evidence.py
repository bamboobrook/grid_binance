#!/usr/bin/env python3
"""Round 21 R4: write the four-families evidence file the validator reads.
Plan §6 + §16: any blocked mandatory family => round BLOCKED.
"""
from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round21"
FITS = json.load(open(ART / "r4" / "families-fit.json"))
OUT = ART / "r4" / "gates" / "four_families.json"
OUT.parent.mkdir(parents=True, exist_ok=True)

# Count fits per family across all blocks
counts: dict[str, int] = {}
for ff in FITS["fold_fits"]:
    for fam in ("C1E", "B1S", "P1S", "V1B"):
        counts[fam] = counts.get(fam, 0) + len(ff[fam]["frozen_fits"])

# Plan §6 implementation status per family
status: dict[str, dict] = {}
for fam in ("C1E", "B1S", "P1S", "V1B"):
    n_fits = counts.get(fam, 0)
    if fam == "C1E":
        st = "implemented" if n_fits > 0 else "blocked_no_stable_groups"
        note = ("5 disjoint OLS residual pair groups fit across 12 R3 blocks; "
                "scheduler config frozen into the fit artifact; deficit-round-"
                "robin scheduler runtime call-site is R4.1 follow-up")
    elif fam == "B1S":
        st = "implemented" if n_fits > 0 else "blocked_no_stable_groups"
        note = ("72 spot/perp basis fits across 12 R3 blocks with distinct "
                "spot/perp MarketLegId; reverse-basis blocked_missing_borrow_"
                "data per R1 data contract")
    elif fam == "P1S":
        # Plan §16: blocked_implementation_scope vs blocked_no_stable_groups.
        # P1S produced 0 fits because MR_share is ~0.004-0.011 across ALL
        # symbols — partial cointegration against BTC factor fails empirically
        # at 1h frequency. This is blocked_no_stable_groups (no MR-dominant
        # residual exists), NOT blocked_implementation_scope.
        st = "blocked_no_stable_groups"
        note = ("0 fits across 12 R3 blocks. MR_share = 1 - AR(1)^2 ≈ 0.004-"
                "0.011 for all 11 non-BTC symbols vs BTC factor at 1h "
                "frequency. Residual is ~random-walk; partial cointegration "
                "against BTC has no MR-dominant component. The TRUE RW+MR "
                "state-space + Soft-SEL (10.3390/a19060442) cannot rescue a "
                "relationship that is empirically absent.")
    elif fam == "V1B":
        st = "blocked_no_stable_groups"
        note = ("0 fits across 12 R3 blocks. The rank-1 covariance eigenvec-"
                "tor is a directional market factor (long_gross=1.0, "
                "short_gross=0.0 on 8/12 blocks); the budget constraint "
                "long_gross>=0.35 AND short_gross>=0.35 cannot be satisfied "
                "by a rank-1 market-factor component. VECM rank>1 or a non-"
                "factor residual target is required, which is out of R4 scope.")
    status[fam] = {"status": st, "fits": n_fits, "note": note,
                   "blocks_with_fits": sum(
                       1 for ff in FITS["fold_fits"]
                       if len(ff[fam]["frozen_fits"]) > 0)}

all_four_implemented = all(
    status[f]["status"] == "implemented"
    for f in ("C1E", "B1S", "P1S", "V1B"))

evidence = {
    "phase": "R21 R4 four families evidence (plan §6, §16)",
    "generated_at_utc": datetime.now(timezone.utc).isoformat(),
    "families": status,
    "all_four_implemented": all_four_implemented,
    "round_status_per_§16": (
        "BLOCKED_ENGINE_DATA_OR_EXECUTION" if not all_four_implemented
        else "ready_for_G0"),
    "blocking_rule_§16": ("ANY mandatory family (C1E/B1S/P1S/V1B) skipped "
                          "for implementation scope => round "
                          "BLOCKED_ENGINE_DATA_OR_EXECUTION, not "
                          "CROSS_VALIDATED_RESEARCH_FINALIST or any "
                          "target-hit claim."),
    "honest_finding": (
        "C1E and B1S produced real frozen fits (5 and 72 respectively). "
        "P1S and V1B produced 0 fits across all 12 R3 blocks because the "
        "underlying statistical relationships are empirically absent at 1h "
        "frequency: P1S partial cointegration MR_share ≈ 0.01 (residual is "
        "~random-walk), V1B rank-1 VECM is a directional market factor "
        "(cannot satisfy long_gross>=0.35 AND short_gross>=0.35). This is a "
        "real mechanism-level block, not an implementation gap. Per §16 the "
        "round status is BLOCKED_ENGINE_DATA_OR_EXECUTION."),
    "followup_paths": [
        "P1S: try daily/4h frequency where MR is stronger, or use PC1 "
        "(first principal component of the universe) instead of BTC as the "
        "cointegrating factor. This is out of R21 scope.",
        "V1B: implement VECM rank>1 or use a non-factor residual target "
        "(e.g. second principal component). The rank-1 market factor is "
        "incompatible with the budget constraint by construction.",
        "C1E scheduler runtime: implement the deficit-round-robin scheduler "
        "call-site in the engine (R4.1).",
        "B1S deep wiring: load spot+perp as two distinct series in the "
        "engine (R4.3).",
    ],
}
with open(OUT, "w") as fh:
    json.dump(evidence, fh, indent=2, sort_keys=True)
print(f"wrote {OUT}")
print(f"all_four_implemented: {all_four_implemented}")
for fam, s in status.items():
    print(f"  {fam}: {s['status']} ({s['fits']} fits, "
          f"{s['blocks_with_fits']}/12 blocks)")
print(f"round_status_per_§16: {evidence['round_status_per_§16']}")
