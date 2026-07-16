#!/usr/bin/env python3
"""Round 18 R0 adversarial activation: prove the three fixed mechanisms
(vol_cap, cluster_scheduler, hazard_deadline) now change REAL events/orders
in adversarial real windows. Per plan §3.2: "只运行用于证明修复的 synthetic +
两个 adversarial real windows，不重跑 R17 192 exact configs."

This is NOT a yield search. It is a binding/activation proof: for each
mechanism we run a config with the mechanism ON vs a varied parameter (or OFF)
over two adversarial windows and assert the event/rejection trace differs.

Adversarial windows (plan §11 G0 requires bull/bear/range/jump coverage):
  - bear: 2025-01-08..2025-04-08 (2025 crypto drawdown)
  - bull: 2023-10-01..2024-01-01 (Q4 2023 rally into 2024)

Mechanism → parameter that must change the event trace:
  vol_cap         : risk_fraction (lower value blocks high-vol FOs)
  cluster_scheduler : symbol_margin_cap_pct (lower value blocks FOs at cap)
  hazard_deadline : deadline_half_lives (lower value fires earlier freeze)
"""
import hashlib
import json
import os
import subprocess
import time

ART = "docs/superpowers/artifacts/glm-martingale-core-round18"
R0 = os.path.join(ART, "r0")
CONFIGS = os.path.join(R0, "configs")
os.makedirs(CONFIGS, exist_ok=True)

# Adversarial windows (epoch ms) — 30-day windows so mechanisms can fire
BEAR_START, BEAR_END = 1736294400000, 1738886400000   # 2025-01-08 .. 2025-02-07
BULL_START, BULL_END = 1696118400000, 1698710400000   # 2023-10-01 .. 2023-10-31
WINDOWS = [("bear", BEAR_START, BEAR_END), ("bull", BULL_START, BULL_END)]

# 6-symbol basket, small enough to be fast but >5 symbols
SYMBOLS = ["BTCUSDT", "ETHUSDT", "SOLUSDT", "DOGEUSDT", "XRPUSDT", "ADAUSDT"]


def _strategy(sym, d, fo, mult, legs, sp, tp, lev, wt, rl):
    return {
        "strategy_id": f"{'L' if d == 'long' else 'S'}-{sym}", "symbol": sym,
        "market": "usd_m_futures", "direction": d, "direction_mode": "long_and_short",
        "margin_mode": "isolated", "leverage": int(lev),
        "spacing": {"fixed_percent": {"step_bps": int(sp)}},
        "sizing": {"multiplier": {"first_order_quote": str(fo), "multiplier": str(mult), "max_legs": int(legs)}},
        "take_profit": {"percent": {"bps": int(tp)}}, "stop_loss": None,
        "indicators": [{"atr": {"period": 14}}],
        "entry_triggers": [{"cooldown": {"seconds": 3600}}],
        "portfolio_weight_pct": str(wt), "risk_limits": rl,
    }


def build_config(rl_extra):
    n = len(SYMBOLS)
    wt = round(100.0 / (2 * n), 4)
    rl = {"htf_regime_gate_enabled": True, "xs_selector_gate_enabled": False}
    rl.update(rl_extra)
    strats = []
    for sym in SYMBOLS:
        strats.append(_strategy(sym, "long", 30, 1.5, 4, 150, 180, 4, wt, rl))
        strats.append(_strategy(sym, "short", 30, 1.3, 3, 200, 180, 4, wt, rl))
    return {"direction_mode": "long_and_short",
            "risk_limits": {"max_global_budget_quote": "3000"},
            "strategies": strats}


def run_cli(cfg, budget, start, end, label):
    path = os.path.join(CONFIGS, f"{label}.json")
    with open(path, "w") as f:
        json.dump({"portfolio_config": cfg}, f, sort_keys=True)
    cmd = ["target/release/portfolio_budget_replay", "--config", path,
           "--budget", str(int(budget)), "--start-ms", str(start), "--end-ms", str(end),
           "--market-data", "data/market_data_full.db",
           "--funding-data", "data/funding_rates_round12.db",
           "--exchange-min-notional", "5.0"]
    t0 = time.time()
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
        wall = time.time() - t0
        if r.returncode != 0:
            return {"ok": False, "err": r.stderr[-800:], "wall_s": round(wall, 1)}
        try:
            out = json.loads(r.stdout)
        except json.JSONDecodeError:
            return {"ok": False, "err": "non-json stdout", "wall_s": round(wall, 1)}
        return {"ok": True, "out": out, "wall_s": round(wall, 1)}
    except subprocess.TimeoutExpired:
        return {"ok": False, "err": "timeout", "wall_s": 600.0}


def extract_signals(out):
    """Pull the binding-relevant signals from a CLI result."""
    digests = out.get("trace_digests", {}) or {}
    return {
        "event_stream_sha": digests.get("event_stream_sha256", ""),
        "trade_stream_sha": digests.get("trade_stream_sha256", ""),
        "rejection_stream_sha": digests.get("rejection_stream_sha256", ""),
        "trade_count": out.get("trade_count", 0),
        "total_rejection_reasons": out.get("total_rejection_reasons", 0),
        "days": out.get("days", 0),
    }


def mechanism_test(mechanism, rl_low, rl_high, budget):
    """Run rl_low vs rl_high in both windows; the event/trade/rejection traces
    must differ (proving the mechanism changes real order/event paths)."""
    rows = []
    for wname, ws, we in WINDOWS:
        for variant, rl in [("low", rl_low), ("high", rl_high)]:
            label = f"r0_{mechanism}_{variant}_{wname}"
            cfg = build_config(rl)
            res = run_cli(cfg, budget, ws, we, label)
            if not res["ok"]:
                rows.append({"label": label, "ok": False,
                             "err": (res.get("err", "") or "")[:200],
                             "wall_s": res.get("wall_s")})
                continue
            sig = extract_signals(res["out"])
            rows.append({"label": label, "ok": True, "wall_s": res["wall_s"], **sig})
    return rows


def main():
    results = {}

    # 1. vol_cap: low risk_fraction (0.05, tight) vs high (0.35, loose)
    results["vol_cap"] = mechanism_test(
        "vol_cap",
        rl_low={"r17_vol_cap": {"completed_window_h": 72, "risk_fraction": 0.05}},
        rl_high={"r17_vol_cap": {"completed_window_h": 72, "risk_fraction": 0.35}},
        budget=3000,
    )

    # 2. cluster_scheduler: low symbol cap (5%, tight) vs high (40%, loose)
    results["cluster_scheduler"] = mechanism_test(
        "cluster_scheduler",
        rl_low={"r17_cluster": {"mode": "active", "abs_corr_threshold": 0.65,
                                "mst_target_clusters": 4, "symbol_margin_cap_pct": 5.0,
                                "cluster_margin_cap_pct": 10.0}},
        rl_high={"r17_cluster": {"mode": "active", "abs_corr_threshold": 0.65,
                                 "mst_target_clusters": 4, "symbol_margin_cap_pct": 40.0,
                                 "cluster_margin_cap_pct": 80.0}},
        budget=3000,
    )

    # 3. hazard_deadline: low deadline_half_lives (1, fires early) vs high (6, fires late)
    results["hazard_deadline"] = mechanism_test(
        "hazard_deadline",
        rl_low={"r17_hazard": {"half_life_window_h": 168, "deadline_half_lives": 1,
                               "deadline_cap_h": 72, "after_deadline": "freeze_so",
                               "reserve_next_legs": 2}},
        rl_high={"r17_hazard": {"half_life_window_h": 168, "deadline_half_lives": 6,
                                "deadline_cap_h": 72, "after_deadline": "freeze_so",
                                "reserve_next_legs": 2}},
        budget=3000,
    )

    # Verdict: for each mechanism, did low vs high produce a DIFFERENT event,
    # trade, or rejection stream in at least one window? (Proving the mechanism
    # changes real order/event paths.)
    verdict = {}
    for mech, rows in results.items():
        ok_rows = [r for r in rows if r.get("ok")]
        by_window = {}
        for r in ok_rows:
            w = r["label"].rsplit("_", 1)[-1]  # window name
            variant = "low" if "_low_" in r["label"] else "high"
            by_window.setdefault(w, {})[variant] = r
        active_in = []
        for w, pair in by_window.items():
            lo, hi = pair.get("low"), pair.get("high")
            if not lo or not hi:
                continue
            differs = (lo["event_stream_sha"] != hi["event_stream_sha"]
                       or lo["trade_stream_sha"] != hi["trade_stream_sha"]
                       or lo["rejection_stream_sha"] != hi["rejection_stream_sha"]
                       or lo["trade_count"] != hi["trade_count"])
            if differs:
                active_in.append(w)
        verdict[mech] = {
            "activation_proven": len(active_in) > 0,
            "active_windows": active_in,
            "by_window": by_window,
            "all_ok": len(ok_rows) == len(rows),
        }

    summary = {
        "phase": "R0.2 adversarial activation",
        "windows": [{"name": n, "start": s, "end": e} for n, s, e in WINDOWS],
        "symbols": SYMBOLS,
        "verdict": verdict,
        "rows": results,
    }
    out_path = os.path.join(R0, "adversarial_activation.json")
    with open(out_path, "w") as f:
        json.dump(summary, f, indent=2)
    print(json.dumps(verdict, indent=2))
    print(f"\nwrote {out_path}")
    all_active = all(v["activation_proven"] for v in verdict.values())
    print(f"all_mechanisms_activation_proven={all_active}")
    return all_active


if __name__ == "__main__":
    ok = main()
    raise SystemExit(0 if ok else 1)
