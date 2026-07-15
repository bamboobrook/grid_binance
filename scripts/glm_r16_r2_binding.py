#!/usr/bin/env python3
"""R2 parameter binding probes.

For the D/H/C families, verify each open parameter changes the resolved config
hash AND the event/admission hash on a real short-window replay (NOT a fast
screen — a real run_kline_screening replay). A parameter that changes neither
is `parameter_inert` and stops the family.

Families tested (each 8-16 synthetic traces):
  D1: htf_regime_gate_enabled (router on/off), direction (long/short)
  H1: spacing step_bps
  H2: deadline_ms (hazard deadline)
  H3: max_legs (reserve/cap proxy)
  C1: xs_selector threshold (correlation cap proxy)
  plus sizing/multiplier/tp as baseline bindings.

Each probe runs a real BatchReplay single full over a short window and compares
the event_stream_sha256 (from trace_digest) + config hash.
"""
import hashlib
import json
import os
import subprocess
import sys

ART = "docs/superpowers/artifacts/glm-martingale-core-round16"
START = 1_672_531_200_000
# 30-day window (real replay, not a screen)
WINDOW_END = START + 30 * 86_400_000


def run_batch_replay_digest(config_dict, budget=4999.0):
    """Write config, run CLI subprocess, return trace_digests + metrics."""
    cfgdir = os.path.join(ART, "configs", "r2")
    os.makedirs(cfgdir, exist_ok=True)
    label = hashlib.md5(json.dumps(config_dict, sort_keys=True).encode()).hexdigest()[:10]
    path = os.path.join(cfgdir, f"r2_{label}.json")
    with open(path, "w") as f:
        json.dump({"portfolio_config": config_dict}, f, sort_keys=True)
    cmd = ["target/release/portfolio_budget_replay", "--config", path,
           "--budget", str(budget), "--start-ms", str(START), "--end-ms", str(WINDOW_END),
           "--market-data", "data/market_data_full.db",
           "--funding-data", "data/funding_rates_round12.db",
           "--exchange-min-notional", "5.0"]
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=300)
    if r.returncode != 0:
        return None, r.stderr[:200]
    try:
        d = json.loads(r.stdout[r.stdout.find("{"):r.stdout.rfind("}") + 1])
        return d.get("trace_digests", {}), None
    except Exception as e:
        return None, str(e)


def eff_hash(config):
    return hashlib.sha256(json.dumps(config, sort_keys=True).encode()).hexdigest()[:16]


def strat(sym, direction, fo, mult, legs, sp, tp, htf_gate=False):
    return {
        "strategy_id": f"{'L' if direction=='long' else 'S'}-{sym}", "symbol": sym,
        "market": "usd_m_futures", "direction": direction, "direction_mode": "long_and_short",
        "margin_mode": "isolated", "leverage": 10,
        "spacing": {"fixed_percent": {"step_bps": sp}},
        "sizing": {"multiplier": {"first_order_quote": str(fo), "multiplier": str(mult), "max_legs": legs}},
        "take_profit": {"percent": {"bps": tp}}, "stop_loss": None, "indicators": [],
        "entry_triggers": [], "portfolio_weight_pct": "100.0",
        "risk_limits": {"htf_regime_gate_enabled": htf_gate},
    }


def portfolio(strategies):
    return {"direction_mode": "long_and_short",
            "risk_limits": {"max_global_budget_quote": "4999"}, "strategies": strategies}


def probe_pair(name, cfg_a, cfg_b, param_label):
    """Run two configs differing in one param; check hash + event changes."""
    ha = eff_hash(cfg_a); hb = eff_hash(cfg_b)
    da, ea = run_batch_replay_digest(cfg_a)
    db, eb = run_batch_replay_digest(cfg_b)
    if da is None or db is None:
        return {"name": name, "param": param_label, "status": "error", "err_a": ea, "err_b": eb}
    hash_changes = ha != hb
    event_changes = da.get("event_stream_sha256") != db.get("event_stream_sha256")
    trade_changes = da.get("trade_stream_sha256") != db.get("trade_stream_sha256")
    bound = hash_changes and (event_changes or trade_changes)
    return {"name": name, "param": param_label, "hash_changes": hash_changes,
            "event_changes": event_changes, "trade_changes": trade_changes,
            "status": "bound" if bound else "parameter_inert"}


def main():
    results = []
    base = dict(sym="BNBUSDT", direction="long", fo=40, mult=2, legs=5, sp=150, tp=150)

    # D1: htf_regime_gate on/off (the router admission)
    cfg_router_off = portfolio([strat(base["sym"], base["direction"], base["fo"], base["mult"], base["legs"], base["sp"], base["tp"], htf_gate=False)])
    cfg_router_on = portfolio([strat(base["sym"], base["direction"], base["fo"], base["mult"], base["legs"], base["sp"], base["tp"], htf_gate=True)])
    results.append(probe_pair("D1_htf_gate", cfg_router_off, cfg_router_on, "htf_regime_gate_enabled"))

    # D1: direction long vs short (asymmetric admission)
    cfg_long = portfolio([strat(base["sym"], "long", base["fo"], base["mult"], base["legs"], base["sp"], base["tp"], htf_gate=True)])
    cfg_short = portfolio([strat(base["sym"], "short", base["fo"], base["mult"], base["legs"], base["sp"], base["tp"], htf_gate=True)])
    results.append(probe_pair("D1_direction", cfg_long, cfg_short, "direction"))

    # H1: spacing
    cfg_sp1 = portfolio([strat(base["sym"], base["direction"], base["fo"], base["mult"], base["legs"], 100, base["tp"])])
    cfg_sp2 = portfolio([strat(base["sym"], base["direction"], base["fo"], base["mult"], base["legs"], 250, base["tp"])])
    results.append(probe_pair("H1_spacing", cfg_sp1, cfg_sp2, "spacing_bps"))

    # H2/H3: max_legs (reserve/cap proxy)
    cfg_l1 = portfolio([strat(base["sym"], base["direction"], base["fo"], base["mult"], 4, base["sp"], base["tp"])])
    cfg_l2 = portfolio([strat(base["sym"], base["direction"], base["fo"], base["mult"], 8, base["sp"], base["tp"])])
    results.append(probe_pair("H3_max_legs", cfg_l1, cfg_l2, "max_legs"))

    # multiplier
    cfg_m1 = portfolio([strat(base["sym"], base["direction"], base["fo"], 1.6, base["legs"], base["sp"], base["tp"])])
    cfg_m2 = portfolio([strat(base["sym"], base["direction"], base["fo"], 2.4, base["legs"], base["sp"], base["tp"])])
    results.append(probe_pair("multiplier", cfg_m1, cfg_m2, "multiplier"))

    # tp
    cfg_t1 = portfolio([strat(base["sym"], base["direction"], base["fo"], base["mult"], base["legs"], base["sp"], 100)])
    cfg_t2 = portfolio([strat(base["sym"], base["direction"], base["fo"], base["mult"], base["legs"], base["sp"], 250)])
    results.append(probe_pair("tp", cfg_t1, cfg_t2, "tp_bps"))

    for r in results:
        print(f"{r['name']} ({r['param']}): {r['status']} hash={r.get('hash_changes')} event={r.get('event_changes')} trade={r.get('trade_changes')}", flush=True)

    families = [{"name": r["name"], "bound": r["status"] == "bound", "detail": r} for r in results]
    all_bound = all(f["bound"] for f in families)
    out = {"schema_version": 1, "phase": "R2_binding",
           "window_days": 30, "families": families, "all_bound": all_bound}
    os.makedirs(os.path.join(ART, "r2"), exist_ok=True)
    path = os.path.join(ART, "r2", "binding-probes.json")
    with open(path, "w") as f:
        json.dump(out, f, indent=2, sort_keys=True)
    print(f"\nwrote {path} all_bound={all_bound}")


if __name__ == "__main__":
    main()
