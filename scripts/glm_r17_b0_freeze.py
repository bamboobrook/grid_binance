#!/usr/bin/env python3
"""B0: freeze data/split/search contract (plan §9). Universe + 4 anchored folds
+ future lock window. B0 is a freeze, no return search."""
import json
import os

ART = "docs/superpowers/artifacts/glm-martingale-core-round17"

T2 = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT", "TRXUSDT", "LINKUSDT", "LTCUSDT", "BCHUSDT", "DOTUSDT"]
T3_8 = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT", "TRXUSDT"]

FOLDS = [
    {"name": "F1", "train": "H1-2023", "validate": "H2-2023"},
    {"name": "F2", "train": "2023", "validate": "2024"},
    {"name": "F3", "train": "2023-2024", "validate": "2025"},
    {"name": "F4", "train": "2023-2025", "validate": "2026-01..2026-05"},
]


def main():
    tracks = [
        {"name": "T1_diagnostic", "role": "regression_only_never_promoted"},
        {"name": "T2_primary", "role": "promotion", "symbols": T2},
        {"name": "T3_8", "role": "small_cap", "symbols": T3_8},
        {"name": "T3_12", "role": "small_cap", "symbols": T2},
    ]
    out = {"tracks": tracks, "folds_frozen": len(FOLDS), "folds": FOLDS,
           "holdout_diagnostic": "2026-06-01..2026-07-10 (opened, diagnostic only)",
           "future_lock": "2026-07-11+ locked until 30 consecutive days; earliest read 2026-08-10+",
           "per_fold_fit": "breadth threshold, funding z, shock percentile, half-life bucket, clusters fit on train only"}
    os.makedirs(os.path.join(ART, "b0"), exist_ok=True)
    with open(os.path.join(ART, "b0", "contract.json"), "w") as f:
        json.dump(out, f, indent=2, sort_keys=True)
    d = os.path.join(ART, "b0", "gates")
    os.makedirs(d, exist_ok=True)
    json.dump({"tracks": tracks, "folds_frozen": len(FOLDS)},
              open(os.path.join(d, "data_split_search_contract_frozen.json"), "w"), indent=2)
    print(f"B0: {len(tracks)} tracks, {len(FOLDS)} folds frozen")


if __name__ == "__main__":
    main()
