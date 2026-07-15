#!/usr/bin/env python3
"""P5 anchored nested WFO: 4 folds (plan §8).

F1 train H1-2023             -> validate H2-2023
F2 train 2023                -> validate 2024
F3 train 2023-2024           -> validate 2025
F4 train 2023-2025           -> validate 2026-01..2026-05

For each fold: select the best TRAIN param from a small frozen candidate grid
(train-only), freeze it, then read its VALIDATION once. This is the real
nested WFO (train-select-freeze-validate-once), using the partial-TP long-only
family. Records CSCV/PBO-style selection statistics and validation outcomes.

Because the G3 validation already showed the 2025-2026 bear window is negative,
we expect folds F3/F4 validation to be negative — the WFO confirms the
cyclical limit formally across all 4 folds.
"""

import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from glm_r15_search_runner import (  # noqa: E402
    SEGMENTS, append_registry, concentration_metrics, effective_config_hash,
    run_replay, write_config,
)

ART = "docs/superpowers/artifacts/glm-martingale-core-round15"
PURGE_DAYS_MS = 7 * 86_400_000

T3_8 = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT", "TRXUSDT"]

# Frozen candidate grid (train-only selection pool). Same partial-TP family.
CANDIDATES = [
    {"fo": 55, "m": 1.81, "legs": 6, "sp": 180, "fbps": 2400},
    {"fo": 60, "m": 2.61, "legs": 4, "sp": 190, "fbps": 2600},
    {"fo": 45, "m": 2.4, "legs": 5, "sp": 160, "fbps": 2700},
    {"fo": 30, "m": 2.22, "legs": 6, "sp": 100, "fbps": 2500},
    {"fo": 50, "m": 2.0, "legs": 5, "sp": 150, "fbps": 2600},
]

# 4 anchored folds: (name, train_start, train_end, val_start, val_end)
FOLDS = [
    ("F1", SEGMENTS[0][1], SEGMENTS[0][2] - PURGE_DAYS_MS, SEGMENTS[1][1], SEGMENTS[1][2]),
    ("F2", SEGMENTS[0][1], SEGMENTS[1][2] - PURGE_DAYS_MS, SEGMENTS[2][1], SEGMENTS[2][2]),
    ("F3", SEGMENTS[0][1], SEGMENTS[2][2] - PURGE_DAYS_MS, SEGMENTS[3][1], SEGMENTS[3][2]),
    ("F4", SEGMENTS[0][1], SEGMENTS[3][2] - PURGE_DAYS_MS, SEGMENTS[4][1], SEGMENTS[4][2]),
]


def partial_tp(fbps):
    return {"partial": {"stages": [[300, 1000, 800], [429, 1000, 1600], [1, 1, fbps]], "breakeven_after_stage": 1, "breakeven_buffer_bps": 100}}


def build(p):
    n = len(T3_8); wt = round(100.0 / n, 4)
    return {"direction_mode": "long_only", "risk_limits": {"max_global_budget_quote": "4999"}, "strategies": [{
        "strategy_id": f"L-{sym}", "symbol": sym, "market": "usd_m_futures",
        "direction": "long", "direction_mode": "long_only", "margin_mode": "isolated",
        "leverage": 10, "spacing": {"fixed_percent": {"step_bps": int(p["sp"])}},
        "sizing": {"multiplier": {"first_order_quote": str(p["fo"]), "multiplier": str(p["m"]), "max_legs": int(p["legs"])}},
        "take_profit": partial_tp(int(p["fbps"])), "stop_loss": None, "indicators": [],
        "entry_triggers": [{"cooldown": {"seconds": 39600}}],
        "portfolio_weight_pct": str(wt), "risk_limits": {},
    } for sym in T3_8]}


def replay(label, p, budget, start, end, window):
    cfg = build(p)
    path, eff = write_config({"portfolio_config": cfg}, label)
    running = {"experiment_id": label, "family": "T3_8_p5_wfo", "hypothesis": "P5 nested WFO",
               "resolved_config_hash": eff, "effective_config_hash": eff, "window": window,
               "budget": budget, "start_ms": start, "end_ms": end, "seed": None,
               "optimizer": "p5_wfo", "status": "running", "started_at": time.time()}
    append_registry(running)
    r = run_replay(path, budget, start, end, timeout=600)
    rec = dict(running); rec.update(r); rec["status"] = r.get("status", "error")
    rec["actual_binary_replays"] = 1 if r.get("status") == "complete" else 0
    rec["cache_hits"] = 0; rec["finished_at"] = time.time()
    if r.get("status") == "complete":
        rec["concentration"] = concentration_metrics(r)
    append_registry(rec)
    return r


def main():
    folds_out = []
    train_selections = []  # for selection-frequency / PBO
    for fname, ts, te, vs, ve in FOLDS:
        # TRAIN: select best (highest ann, no breach) over candidates
        train_results = []
        for p in CANDIDATES:
            r = replay(f"r15_p5_{fname}_train_{p['fo']}_{p['m']}_{p['legs']}_{p['sp']}", p, 4999.0, ts, te, f"{fname}_train")
            if r.get("status") == "complete" and not r.get("principal_breached"):
                train_results.append((p, r.get("ann", -999), r))
        # select best by ann
        train_results.sort(key=lambda x: -(x[1] or -999))
        selected = train_results[0][0] if train_results else CANDIDATES[0]
        train_selections.append(selected)
        # VALIDATION: read once (frozen selection)
        vr = replay(f"r15_p5_{fname}_val_{selected['fo']}_{selected['m']}_{selected['legs']}_{selected['sp']}", selected, 4999.0, vs, ve, f"{fname}_val")
        vconc = concentration_metrics(vr) if vr.get("status") == "complete" else {}
        folds_out.append({
            "fold": fname, "train_window": [ts, te], "val_window": [vs, ve],
            "train_candidates_evaluated": len(CANDIDATES),
            "train_selected": selected,
            "val_ann": vr.get("ann"), "val_dd": vr.get("dd"),
            "val_breached": vr.get("principal_breached"),
            "val_conc_gp": vconc.get("max_gross_profit_share"),
            "val_positive": (vr.get("ann") or -999) > 0,
        })
        print(f"{fname}: selected {selected} val_ann={vr.get('ann')} val_dd={vr.get('dd')} breached={vr.get('principal_breached')} pos={(vr.get('ann') or -999)>0}", flush=True)

    # Selection frequency + budget stability
    sel_keys = ["{}_{}_{}_{}".format(s["fo"], s["m"], s["legs"], s["sp"]) for s in train_selections]
    from collections import Counter
    sel_freq = Counter(sel_keys)
    pos_folds = sum(1 for f in folds_out if f["val_positive"])
    out = {
        "schema_version": 1, "phase": "P5_NESTED_WFO",
        "folds": folds_out,
        "selection_frequency": dict(sel_freq),
        "positive_validation_folds": pos_folds,
        "total_folds": len(FOLDS),
        "stop_rule_two_negative_val_folds": pos_folds < (len(FOLDS) - 1),
        "finalists": 0 if pos_folds < 3 else 1,
    }
    path = os.path.join(ART, "p5", "wfo-4-folds.json")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    json.dump(out, open(path, "w"), indent=2, sort_keys=True)
    print(f"\nwrote {path}; positive val folds={pos_folds}/{len(FOLDS)}")


if __name__ == "__main__":
    main()
