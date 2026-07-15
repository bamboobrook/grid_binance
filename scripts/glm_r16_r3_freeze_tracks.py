#!/usr/bin/env python3
"""R3: freeze tracks + risk envelopes (plan §10)."""
import json
import os

ART = "docs/superpowers/artifacts/glm-martingale-core-round16"

T1_DIAGNOSTIC = ["BNBUSDT", "TRXUSDT", "ANKRUSDT", "AAVEUSDT", "SOLUSDT", "DOTUSDT"]  # R7 exact, never promoted
T2 = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT", "TRXUSDT", "LINKUSDT", "LTCUSDT", "BCHUSDT", "DOTUSDT"]
T3_8 = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT", "TRXUSDT"]
T3_12 = list(T2)

# Risk envelopes per plan §10 table (conservative/balanced/aggressive)
ENVELOPES = {
    "conservative": {"leverage": [3, 5], "long_fo": [10, 15, 20], "long_mult": [1.25, 1.40, 1.55], "long_legs": [4, 5],
                     "short_mult": [1.20, 1.30], "short_legs": [3, 4]},
    "balanced": {"leverage": [5, 8], "long_fo": [15, 20, 30], "long_mult": [1.40, 1.60, 1.80], "long_legs": [5, 6],
                 "short_mult": [1.25, 1.40], "short_legs": [3, 4]},
    "aggressive": {"leverage": [8, 10], "long_fo": [20, 30, 45], "long_mult": [1.60, 1.90, 2.20], "long_legs": [5, 6, 7],
                   "short_mult": [1.30, 1.50, 1.70], "short_legs": [3, 4, 5]},
}
SPACING_BPS = [80, 120, 180, 250, 400]
TP_BPS = [80, 120, 180, 250, 350]


def main():
    tracks = [
        {"name": "T1_diagnostic", "role": "diagnostic_only_never_promoted", "symbols": T1_DIAGNOSTIC,
         "note": "R7 exact universe; contains survivor-picked ANKR; never enters promotion pool"},
        {"name": "T2_primary", "role": "primary_promotion", "symbols": T2,
         "long_short_sleeve_count": len(T2) * 2, "direction_contract": "router_admission_asymmetric"},
        {"name": "T3_8", "role": "small_cap", "symbols": T3_8, "long_short_sleeve_count": len(T3_8) * 2},
        {"name": "T3_12", "role": "small_cap", "symbols": T3_12, "long_short_sleeve_count": len(T3_12) * 2},
    ]
    out = {"tracks": tracks, "risk_envelopes": ENVELOPES,
           "spacing_bps": SPACING_BPS, "tp_bps": TP_BPS,
           "short_asymmetry": {"short_mult_le_long_mult": True, "short_legs_le_long_legs": True,
                                "short_spacing_ge_long_spacing": True, "short_deadline_le_long": True,
                                "short_notional_le_40pct_budget": True},
           "note": "Tracks + risk envelopes frozen before any return search (plan §10)."}
    os.makedirs(os.path.join(ART, "r3"), exist_ok=True)
    path = os.path.join(ART, "r3", "tracks-and-envelopes.json")
    with open(path, "w") as f:
        json.dump(out, f, indent=2, sort_keys=True)
    d = os.path.join(ART, "r3", "gates")
    os.makedirs(d, exist_ok=True)
    json.dump({"tracks": tracks, "risk_envelopes_frozen": True},
              open(os.path.join(d, "tracks_and_risk_envelopes_frozen.json"), "w"), indent=2)
    print(f"R3: {len(tracks)} tracks frozen + 3 risk envelopes")


if __name__ == "__main__":
    main()
