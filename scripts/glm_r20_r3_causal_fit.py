#!/usr/bin/env python3
"""Round 20 P3: causal nested fit + inner replay (plan §5).

Anti-overfit contract (fixes R19 audit §7 "inner-train look-ahead"):
  - fit uses ONLY data <= inner_fit_end
  - purge >= max(signal lookback, settlement latency, 1 completed decision bar)
  - replay starts strictly AFTER fit_end + purge
  - roll/expand fit before each later block; NEVER replay an earlier block with
    a later fit (R19 bug: used outer-train-end fit for all 5 subblocks)
  - blocks <180 days: report raw return/DD/PF/cost/SO only, NOT annualized
"""
import hashlib
import json
import os

ART = "docs/superpowers/artifacts/glm-martingale-core-round20"
P3 = os.path.join(ART, "p3")
os.makedirs(P3, exist_ok=True)

FOLDS = [
    ("F1", 1672531200000, 1688169599999, 1688774400000, 1704067199999),
    ("F2", 1672531200000, 1704067199999, 1704662400000, 1735689599999),
    ("F3", 1672531200000, 1735689599999, 1736294400000, 1767225599999),
    ("F4", 1672531200000, 1767225599999, 1767820800000, 1780271999999),
]

MIN_LOOKBACK_DAYS = 30
SETTLEMENT_LATENCY_MS = 8 * 3600 * 1000
DECISION_BAR_MS = 5 * 60 * 1000


def purge_ms(lookback_days):
    return max(lookback_days * 86_400_000, SETTLEMENT_LATENCY_MS, DECISION_BAR_MS)


def causal_inner_blocks(train_start, train_end, lookback_days=60, n_blocks=5):
    blocks = []
    initial_fit_end = train_start + lookback_days * 86_400_000
    if initial_fit_end >= train_end:
        return blocks
    replay_span = train_end - initial_fit_end
    block_chunk = replay_span // n_blocks
    purge = purge_ms(lookback_days)
    for i in range(n_blocks):
        replay_start = initial_fit_end + i * block_chunk
        replay_end = initial_fit_end + (i + 1) * block_chunk if i < n_blocks - 1 else train_end
        fit_end = replay_start - purge
        fit_start = max(train_start, fit_end - lookback_days * 86_400_000)
        if fit_end <= fit_start or replay_end <= replay_start:
            continue
        blocks.append({
            "block": f"ib{i+1}",
            "fit_start": fit_start, "fit_end": fit_end,
            "replay_start": replay_start, "replay_end": replay_end,
            "purge_ms": purge,
            "replay_days": (replay_end - replay_start) / 86_400_000,
            "causal_ok": fit_end < replay_start,
        })
    return blocks


def validate_causal_no_leakage(blocks):
    """Causal check: each block's fit must end strictly before its own replay
    starts (fit_end < replay_start), and the fit window must not extend into
    the replay window. Rolling/expanding fit (where a later block's fit_end is
    later than an earlier block's replay) is ALLOWED — each block only sees
    data up to its own fit_end, which is before its own replay."""
    problems = []
    for b in blocks:
        if b["fit_end"] >= b["replay_start"]:
            problems.append(f"{b['block']}: fit_end {b['fit_end']} >= replay_start {b['replay_start']} (look-ahead)")
        if b["fit_start"] > b["replay_start"]:
            problems.append(f"{b['block']}: fit_start {b['fit_start']} > replay_start {b['replay_start']}")
    return problems


def inject_leakage_counterexample():
    train_start = 1672531200000
    train_end = 1704067199999
    leaky_block = {
        "block": "leaky_ib1",
        "fit_start": train_end - 60 * 86_400_000,
        "fit_end": train_end,
        "replay_start": train_start + 60 * 86_400_000,
        "replay_end": train_start + 90 * 86_400_000,
        "purge_ms": 7 * 86_400_000,
    }
    problems = validate_causal_no_leakage([leaky_block])
    return {"leaky_block": leaky_block, "validator_caught_leak": len(problems) > 0, "problems": problems}


def main():
    all_folds = []
    for fname, ts, te, vs, ve in FOLDS:
        blocks = causal_inner_blocks(ts, te, lookback_days=60, n_blocks=5)
        problems = validate_causal_no_leakage(blocks)
        replay_days = [b["replay_days"] for b in blocks]
        stitched_days = sum(replay_days)
        all_folds.append({
            "fold": fname, "train_start": ts, "train_end": te,
            "val_start": vs, "val_end": ve,
            "inner_blocks": blocks,
            "causal_valid": len(problems) == 0,
            "causal_problems": problems,
            "stitched_replay_days": stitched_days,
            "report_ann_eligible": stitched_days >= 180,
        })
    leakage_test = inject_leakage_counterexample()
    out = {
        "phase": "P3 causal nested fit + inner replay",
        "folds": [{"fold": n, "train_start": ts, "train_end": te, "val_start": vs, "val_end": ve} for n, ts, te, vs, ve in FOLDS],
        "contract": "fit uses ONLY data <= inner_fit_end; purge >= max(lookback, settlement, 1 bar); replay starts strictly after fit_end+purge; never replay earlier block with later fit (R19 bug)",
        "fold_schedules": all_folds,
        "leakage_counterexample_injection": leakage_test,
        "validated": all(f["causal_valid"] for f in all_folds) and leakage_test["validator_caught_leak"],
    }
    out_path = os.path.join(P3, "causal-fit.json")
    with open(out_path, "w") as f:
        json.dump(out, f, indent=2)
    print(f"wrote {out_path}")
    for f in all_folds:
        print(f"  {f['fold']}: {len(f['inner_blocks'])} causal blocks, stitched={f['stitched_replay_days']:.0f}d, ann_eligible={f['report_ann_eligible']}, causal_valid={f['causal_valid']}")
    print(f"leakage counterexample caught: {leakage_test['validator_caught_leak']}")
    print(f"validated: {out['validated']}")


if __name__ == "__main__":
    main()
