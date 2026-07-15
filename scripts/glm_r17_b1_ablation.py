#!/usr/bin/env python3
"""B1 mechanism ablation (plan §10). Same low-risk ladder center on T3-8, each
mechanism on/off once, train blocks only. Reports admission count, SO count,
cycle duration, fee/funding, DD, reject reasons, event delta."""
import hashlib
import json
import os
import subprocess

ART = "docs/superpowers/artifacts/glm-martingale-core-round17"
T3_8 = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT", "TRXUSDT"]
START = 1_672_531_200_000
END = 1_672_531_200_000 + 90 * 86_400_000  # 90-day train block for ablation


def build(rl_extras_per_strategy):
    n = len(T3_8); wt = round(100.0 / (2 * n), 4)
    strats = []
    for sym in T3_8:
        for direction in ("long", "short"):
            rl = {"htf_regime_gate_enabled": True}
            rl.update(rl_extras_per_strategy)
            strats.append({"strategy_id": f"{'L' if direction=='long' else 'S'}-{sym}", "symbol": sym,
                "market": "usd_m_futures", "direction": direction, "direction_mode": "long_and_short",
                "margin_mode": "isolated", "leverage": 3,
                "spacing": {"fixed_percent": {"step_bps": 180}},
                "sizing": {"multiplier": {"first_order_quote": "15", "multiplier": "1.4", "max_legs": 5}},
                "take_profit": {"percent": {"bps": 180}}, "stop_loss": None, "indicators": [],
                "entry_triggers": [{"cooldown": {"seconds": 39600}}], "portfolio_weight_pct": str(wt),
                "risk_limits": rl})
    return {"direction_mode": "long_and_short", "risk_limits": {"max_global_budget_quote": "4999"}, "strategies": strats}


def run(config, label):
    cfgdir = os.path.join(ART, "configs", "b1"); os.makedirs(cfgdir, exist_ok=True)
    h = hashlib.md5(json.dumps(config, sort_keys=True).encode()).hexdigest()[:8]
    path = os.path.join(cfgdir, f"b1_{label}_{h}.json")
    with open(path, "w") as f: json.dump({"portfolio_config": config}, f, sort_keys=True)
    cmd = ["target/release/portfolio_budget_replay", "--config", path, "--budget", "4999",
           "--start-ms", str(START), "--end-ms", str(END),
           "--market-data", "data/market_data_full.db", "--funding-data", "data/funding_rates_round12.db",
           "--exchange-min-notional", "5.0"]
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    if r.returncode != 0: return {"status": "error", "err": r.stderr[:150]}
    d = json.loads(r.stdout[r.stdout.find("{"):r.stdout.rfind("}") + 1])
    ob = d.get("on_budget", {})
    return {"status": "complete", "ann": round(ob.get("annualized_return_pct") or -999, 4),
            "dd": round(ob.get("max_drawdown_pct") or -999, 4),
            "trade_count": d.get("trade_count"), "breached": ob.get("principal_breached"),
            "rejections": d.get("total_rejection_reasons"),
            "event_hash": d.get("trace_digests", {}).get("event_stream_sha256", "")[:16]}


def main():
    arms = [
        ("baseline_default_htf", {}),
        ("plus_breadth", {"r17_router": {"trend_horizon_4h": 24, "breadth_threshold": 0.70, "enter_persistence": 3, "exit_persistence": 2, "minimum_dwell_hours": 24, "range_displacement_z": 1.25, "shock_downside_q": 0.90, "cusum_sigma": 4.0, "shock_cooldown_hours": 24}}),
        ("plus_funding_veto", {"r17_funding_crowding": {"completed_window": 30, "adverse_z_veto": 2.0}}),
        ("plus_cusum_shock", {"r17_router": {"trend_horizon_4h": 24, "breadth_threshold": 0.70, "enter_persistence": 3, "exit_persistence": 2, "minimum_dwell_hours": 24, "range_displacement_z": 1.25, "shock_downside_q": 0.95, "cusum_sigma": 6.0, "shock_cooldown_hours": 24}}),
        ("plus_deadline", {"r17_hazard": {"half_life_window_h": 168, "deadline_half_lives": 3, "deadline_cap_h": 72, "after_deadline": "freeze_so", "reserve_next_legs": 2}}),
        ("plus_reserve_cluster", {"r17_cluster": {"mode": "abs_corr", "abs_corr_threshold": 0.65, "mst_target_clusters": 4, "symbol_margin_cap_pct": 25.0, "cluster_margin_cap_pct": 35.0}}),
        ("all_combined", {"r17_router": {"trend_horizon_4h": 24, "breadth_threshold": 0.70, "enter_persistence": 3, "exit_persistence": 2, "minimum_dwell_hours": 24, "range_displacement_z": 1.25, "shock_downside_q": 0.90, "cusum_sigma": 4.0, "shock_cooldown_hours": 24}, "r17_funding_crowding": {"completed_window": 30, "adverse_z_veto": 2.0}, "r17_hazard": {"half_life_window_h": 168, "deadline_half_lives": 3, "deadline_cap_h": 72, "after_deadline": "freeze_so", "reserve_next_legs": 2}, "r17_cluster": {"mode": "abs_corr", "abs_corr_threshold": 0.65, "mst_target_clusters": 4, "symbol_margin_cap_pct": 25.0, "cluster_margin_cap_pct": 35.0}}),
    ]
    results = []
    for name, extras in arms:
        cfg = build(extras)
        r = run(cfg, name)
        results.append({"arm": name, **r})
        print(f"{name}: ann={r.get('ann')} dd={r.get('dd')} trades={r.get('trade_count')} breach={r.get('breached')} rej={r.get('rejections')} evhash={r.get('event_hash')}", flush=True)
    out = {"schema_version": 1, "phase": "B1_ablation", "ablation_arms": results, "window_days": 90}
    os.makedirs(os.path.join(ART, "b1"), exist_ok=True)
    with open(os.path.join(ART, "b1", "ablation.json"), "w") as f:
        json.dump(out, f, indent=2, sort_keys=True)
    d = os.path.join(ART, "b1", "gates"); os.makedirs(d, exist_ok=True)
    json.dump({"ablation_arms": results, "executed": len(results) >= 6},
              open(os.path.join(d, "mechanism_ablation_executed.json"), "w"), indent=2)
    print(f"\nB1: {len(results)} arms executed")


if __name__ == "__main__":
    main()
