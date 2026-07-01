#!/usr/bin/env python3
"""Small targeted P4 combo probe.

Runs only a handful of mixes so each result can be inspected quickly.
"""
from __future__ import annotations

import copy
import json
import subprocess
import tempfile
from pathlib import Path

from probe_p4_combo_mix import (
    SEGMENTS,
    build_mix,
    fmt_decimal,
    load_short_sets,
    normalize_weights,
    run_replay,
    strip_old_shorts,
)


REPO = Path("/home/bumblebee/Project/grid_binance/.claude/worktrees/p4-cycle-exit")
MAIN = Path("/home/bumblebee/Project/grid_binance")
BUDGET = 5000.0
PROFILE = "balanced"
OUT = REPO / "docs/superpowers/reports/2026-06-29-p4-combo-targeted-probe.json"


def base_config(name: str) -> dict:
    path = MAIN / f"docs/superpowers/artifacts/glm-balanced-candidate/{name}.json"
    return json.loads(path.read_text())


def no_old_shorts(cfg: dict, base_total_weight: float = 100.0) -> dict:
    out = copy.deepcopy(cfg)
    strategies = strip_old_shorts(out["portfolio_config"]["strategies"])
    normalize_weights(strategies, base_total_weight)
    out["portfolio_config"]["strategies"] = strategies
    out["portfolio_config"]["risk_limits"]["max_global_budget_quote"] = fmt_decimal(BUDGET)
    return out


def replay_full(name: str, cfg: dict) -> dict:
    start, end = SEGMENTS["full"]
    result = run_replay(
        REPO / "target/release/portfolio_budget_replay",
        cfg,
        BUDGET,
        PROFILE,
        start,
        end,
        MAIN / "data/market_data_full.db",
        MAIN / "data/funding_rates.db",
        900,
    )
    result["name"] = name
    return result


def main() -> int:
    search_sets = load_short_sets(Path("/tmp/2025_p4_3000_allrows.json"))
    bases = {
        "floor1500": base_config("best_balanced_floor1500_b5000"),
        "l5_robust": base_config("best_balanced_l5_robust_b5000"),
    }
    experiments: list[tuple[str, dict]] = []
    for base_name, cfg in bases.items():
        experiments.append((f"{base_name}__as_is", copy.deepcopy(cfg)))
        experiments.append((f"{base_name}__no_old_shorts", no_old_shorts(cfg)))
        for set_name in ("low3", "mid3", "high3", "low3_gala", "rsi_low4"):
            short_set = search_sets.get(set_name) or []
            if not short_set:
                continue
            for mode, short_weight in (
                ("norm", 20.0),
                ("norm", 35.0),
                ("drop_old_shorts_norm", 20.0),
                ("drop_old_shorts_norm", 35.0),
            ):
                experiments.append(
                    (
                        f"{base_name}__{mode}__{set_name}__short{int(short_weight)}",
                        build_mix(cfg, short_set, mode, short_weight, BUDGET, f"{set_name}-{int(short_weight)}"),
                    )
                )

    report = {
        "budget": BUDGET,
        "profile": PROFILE,
        "short_sets": {
            key: [
                {
                    "symbol": row["symbol"],
                    "ann_2025": row["annualized_return_pct"],
                    "dd_2025": row["max_drawdown_pct"],
                    "foq": row["first_order_quote"],
                    "filter": row["entry_filter"],
                    "rb": row.get("regime_break_ema_period"),
                    "age": row.get("max_cycle_age_hours"),
                }
                for row in value
            ]
            for key, value in search_sets.items()
        },
        "full_results": [],
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    for idx, (name, cfg) in enumerate(experiments, 1):
        result = replay_full(name, cfg)
        report["full_results"].append(result)
        OUT.write_text(json.dumps(report, ensure_ascii=False, indent=2))
        if result.get("ok"):
            print(
                f"[{idx:02d}/{len(experiments)}] {name} "
                f"ann={result['ann']:.2f} dd={result['dd']:.2f} ret={result['ret']:.2f} "
                f"cap={result['cap']:.1f} blocked={result['blocked']} gate={result['gate_passed']}",
                flush=True,
            )
        else:
            print(f"[{idx:02d}/{len(experiments)}] {name} ERROR {result.get('error')}", flush=True)
    print(f"wrote {OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
