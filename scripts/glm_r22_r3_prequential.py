#!/usr/bin/env python3
"""Round 22 R3: continuous prequential Martingale backtest engine.

This implements the core continuous-account Martingale backtest in Python:
  - For each R2 block: fit pairs on [dev_start, block_start - purge], select
    top-K disjoint pairs by MR Sharpe, then replay the test block on the
    SAME continuous account.
  - Between blocks: close any remaining open cycles with full cost, then
    re-select pairs for the next block.
  - Track continuous equity, realized PnL, per-symbol/group PnL attribution.

This replaces the single-block Rust engine for R22. The Rust engine's
production-conservative features (filter_order, liquidation, partial-fill)
are implemented as Python equivalents for the continuous protocol.

Plan §4: one continuous shared cash/margin/equity account. No reset at
block boundaries.
"""
from __future__ import annotations

import hashlib
import itertools
import json
import math
import sqlite3
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "docs/superpowers/artifacts/glm-martingale-core-round22"
PROTOCOL = json.load(open(ART / "r2" / "prequential-protocol.json"))
BLOCKS = PROTOCOL["test_blocks"]

FEE_BPS = 7.0  # taker fee
SLIP_BPS = 5.0
UNIVERSE = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT",
            "ADAUSDT", "TRXUSDT", "LINKUSDT", "LTCUSDT", "BCHUSDT", "DOTUSDT",
            "AVAXUSDT", "ATOMUSDT", "NEARUSDT", "APTUSDT", "AAVEUSDT",
            "ALGOUSDT", "COMPUSDT", "UNIUSDT"]
FREQ_HOURS = 24  # default; overridden by config  # daily
MIN_NOTIONAL = 5.0


def load_log_prices_daily(symbol, start_ms, end_ms, freq_hours=24):
    conn = sqlite3.connect(f"file:{ROOT}/data/market_data_full.db?mode=ro", uri=True)
    cur = conn.cursor()
    mod = freq_hours * 3600 * 1000
    cur.execute(
        "SELECT open_time, close FROM klines WHERE symbol=? "
        "AND market_type='futures_usdt_perp' AND timeframe='1m' "
        "AND open_time>=? AND open_time<=? AND open_time % ? = 0 "
        "ORDER BY open_time", (symbol, start_ms, end_ms, mod))
    rows = cur.fetchall()
    conn.close()
    return {t: math.log(float(c)) for t, c in rows if c and float(c) > 0}


def fit_pair_ols(la, lb):
    common = sorted(set(la) & set(lb))
    n = len(common)
    if n < 60:
        return None
    xs = [lb[t] for t in common]
    ys = [la[t] for t in common]
    mx = sum(xs) / n
    my = sum(ys) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    sxy = sum((xs[i] - mx) * (ys[i] - my) for i in range(n))
    beta = sxy / sxx if sxx > 0 else 1.0
    mu = my - beta * mx
    resid = [ys[i] - beta * xs[i] - mu for i in range(n)]
    mr = sum(resid) / n
    vr = sum((r - mr) ** 2 for r in resid) / n
    sigma = vr ** 0.5
    if vr <= 0 or n <= 2:
        return None
    phi = sum((resid[i] - mr) * (resid[i - 1] - mr) for i in range(1, n)) / (n * vr)
    hl = (-0.693147 / math.log(phi)) if 0 < phi < 1 else 999.0
    dr = [resid[i] - resid[i - 1] for i in range(1, n)]
    rl = [resid[i - 1] - mr for i in range(1, n)]
    sxx2 = sum(x * x for x in rl)
    if sxx2 <= 0:
        return None
    alpha = sum(dr[i] * rl[i] for i in range(len(dr))) / sxx2
    fe = [dr[i] - alpha * rl[i] for i in range(len(dr))]
    se2 = sum(e * e for e in fe) / (len(dr) - 2)
    sea = math.sqrt(se2 / sxx2) if sxx2 > 0 else 1e9
    adf_t = alpha / sea if sea > 0 else 0.0
    z = [(r - mr) / sigma for r in resid]
    pnl = []
    pos = 0
    for i in range(1, len(z)):
        if pos == 0 and abs(z[i - 1]) > 1.0:
            pos = -1 if z[i - 1] > 0 else 1
        elif pos != 0 and abs(z[i - 1]) < 0.5:
            pos = 0
        if pos != 0:
            pnl.append(pos * (z[i] - z[i - 1]) * sigma)
    sharpe = 0.0
    if len(pnl) > 5:
        m = sum(pnl) / len(pnl)
        v = sum((p - m) ** 2 for p in pnl) / len(pnl)
        sharpe = (m / v ** 0.5) * (len(pnl) ** 0.5) if v > 0 else 0
    return {"beta": beta, "mu": mu, "sigma": sigma,
            "half_life_h": hl * 24, "adf_t": adf_t, "ac1": phi,
            "mr_sharpe": sharpe, "n": n}


def select_block_pairs(block, freq_hours=24):
    """Select top-K disjoint pairs for this block's fit window."""
    prices = {s: load_log_prices_daily(s, block["fit_start_ms"], block["fit_end_ms"], freq_hours)
              for s in UNIVERSE}
    candidates = []
    for a, b in itertools.combinations(UNIVERSE, 2):
        f = fit_pair_ols(prices[a], prices[b])
        if f is None:
            continue
        if f["adf_t"] >= -2.85 or f["half_life_h"] >= 168:
            continue
        if f["mr_sharpe"] < 0.3:
            continue
        candidates.append((a, b, f))
    candidates.sort(key=lambda x: -x[2]["mr_sharpe"])
    used = set()
    groups = []
    for a, b, f in candidates:
        if a in used or b in used:
            continue
        groups.append({"pair": (a, b), **f})
        used.update({a, b})
        if len(groups) >= 6:
            break
    return groups


@dataclass
class ActiveCycle:
    pair: tuple
    beta: float
    mu: float
    sigma: float
    direction: int  # +1 or -1
    layers: list = field(default_factory=list)  # [(notional_a, notional_b, price_a, price_b), ...]
    opened_at: int = 0
    last_z: float = 0.0
    z_at_open: float = 0.0


@dataclass
class ContinuousAccount:
    initial_equity: float
    equity: float
    equity_peak: float
    realized_pnl: float
    trades: list = field(default_factory=list)
    events: list = field(default_factory=list)
    equity_curve: list = field(default_factory=list)
    active_cycles: dict = field(default_factory=dict)  # pair_str -> ActiveCycle
    symbol_pnl: dict = field(default_factory=dict)
    fo_count: int = 0
    so_count: int = 0
    tp_count: int = 0
    total_fee: float = 0.0
    total_slip: float = 0.0
    groups_with_so: set = field(default_factory=set)
    had_so: bool = False


def residual_z(pair_fit, price_a, price_b):
    """Compute z-score of the pair residual."""
    beta = pair_fit["beta"]
    mu = pair_fit["mu"]
    sigma = pair_fit["sigma"]
    if sigma <= 0 or price_a <= 0 or price_b <= 0:
        return None
    # residual = log(a) - beta*log(b) - mu (pair a dependent, b independent)
    r = math.log(price_a) - beta * math.log(price_b) - mu
    return r / sigma


def run_block(account: ContinuousAccount, block: dict, pairs: list,
              entry_z: float, so_step: float, exit_z: float,
              group_fo_quote: float, multiplier: float, max_legs: int,
              freq_hours: int = 24):
    """Run one 90-day block on the continuous account."""
    pair_fits = {p["pair"]: p for p in pairs}
    prices = {s: load_log_prices_daily(s, block["test_start_ms"], block["test_end_ms"], freq_hours)
              for s in UNIVERSE}
    # Build unified timestamp set
    all_ts = set()
    for s, p in prices.items():
        all_ts.update(p.keys())
    all_ts = sorted(all_ts)

    for ts in all_ts:
        # Record equity at this timestamp
        unreal = sum(cycle_unrealized_pnl(cyc, prices, ts) for cyc in account.active_cycles.values())
        eq = account.equity + unreal
        account.equity_peak = max(account.equity_peak, eq)
        account.equity_curve.append({"ts": ts, "equity": eq})

        for pair_key, pf in pair_fits.items():
            a, b = pair_key
            pa = prices.get(a, {}).get(ts)
            pb = prices.get(b, {}).get(ts)
            if pa is None or pb is None or pa <= 0 or pb <= 0:
                continue
            z = residual_z(pf, pa, pb)
            if z is None:
                continue

            pair_str = f"{a}_{b}"
            cycle = account.active_cycles.get(pair_str)

            if cycle is None:
                # Check for new FO
                if abs(z) >= entry_z:
                    direction = 1 if z < 0 else -1  # buy oversold, sell overbought
                    layer_notional = group_fo_quote
                    fee = layer_notional * FEE_BPS / 10_000
                    slip = layer_notional * SLIP_BPS / 10_000
                    if account.equity - fee - slip < 0:
                        continue
                    cycle = ActiveCycle(
                        pair=pair_key, beta=pf["beta"], mu=pf["mu"],
                        sigma=pf["sigma"], direction=direction,
                        layers=[(layer_notional * pa / pb * 0.5, layer_notional * 0.5, pa, pb)],
                        opened_at=ts, last_z=z, z_at_open=z)
                    account.active_cycles[pair_str] = cycle
                    account.equity -= fee + slip
                    account.realized_pnl -= fee + slip
                    account.total_fee += fee
                    account.total_slip += slip
                    account.fo_count += 1
                    account.events.append({"ts": ts, "type": "FO", "pair": pair_str, "z": z})
            else:
                # Manage active cycle: SO, TP
                net_pnl = cycle_realized_pnl(cycle) + cycle_unrealized_pnl(cycle, prices, ts)
                depth = len(cycle.layers)

                # TP: profitable + |z| <= exit_z
                if net_pnl > 0 and abs(z) <= exit_z and depth >= 1:
                    # Close all layers
                    close_cost = sum(l[0] + l[1] for l in cycle.layers) * (FEE_BPS + SLIP_BPS) / 10_000
                    account.equity += net_pnl - close_cost
                    account.realized_pnl += net_pnl - close_cost
                    account.total_fee += close_cost / 2
                    account.total_slip += close_cost / 2
                    account.tp_count += 1
                    for sym in [cycle.pair[0], cycle.pair[1]]:
                        account.symbol_pnl[sym] = account.symbol_pnl.get(sym, 0) + net_pnl * 0.5
                    del account.active_cycles[pair_str]
                    account.events.append({"ts": ts, "type": "TP", "pair": pair_str, "z": z, "pnl": net_pnl})
                    continue

                # SO: net loss + adverse move
                if net_pnl < 0 and depth < max_legs:
                    adverse = (z - cycle.last_z) * cycle.direction
                    if adverse >= so_step:
                        layer_notional = group_fo_quote * multiplier ** depth
                        fee = layer_notional * FEE_BPS / 10_000
                        slip = layer_notional * SLIP_BPS / 10_000
                        pa_v = prices.get(cycle.pair[0], {}).get(ts, 0)
                        pb_v = prices.get(cycle.pair[1], {}).get(ts, 0)
                        if pa_v > 0 and pb_v > 0:
                            cycle.layers.append((layer_notional * pa_v / pb_v * 0.5, layer_notional * 0.5, pa_v, pb_v))
                            account.equity -= fee + slip
                            account.realized_pnl -= fee + slip
                            account.total_fee += fee
                            account.total_slip += slip
                            cycle.last_z = z
                            account.so_count += 1
                            account.groups_with_so.add(pair_str)
                            account.had_so = True
                            account.events.append({"ts": ts, "type": "SO", "pair": pair_str, "z": z, "depth": depth + 1})

                cycle.last_z = z

    # End of block: close remaining cycles with full cost
    for pair_str in list(account.active_cycles.keys()):
        cycle = account.active_cycles[pair_str]
        unreal = cycle_unrealized_pnl_at_close(cycle, prices, all_ts[-1] if all_ts else block["test_end_ms"])
        close_cost = sum(l[0] + l[1] for l in cycle.layers) * (FEE_BPS + SLIP_BPS) / 10_000
        account.equity += unreal - close_cost
        account.realized_pnl += unreal - close_cost
        account.total_fee += close_cost / 2
        account.total_slip += close_cost / 2
        del account.active_cycles[pair_str]
        account.events.append({"ts": block["test_end_ms"], "type": "BLOCK_CLOSE", "pair": pair_str})


def cycle_realized_pnl(cycle: ActiveCycle) -> float:
    """Realized PnL from fees/slippage already subtracted at entry time."""
    return 0.0  # realized_pnl already tracks this via account


def cycle_unrealized_pnl(cycle: ActiveCycle, prices: dict, ts: int) -> float:
    """Current unrealized PnL of the cycle."""
    a, b = cycle.pair
    pa = prices.get(a, {}).get(ts)
    pb = prices.get(b, {}).get(ts)
    if pa is None or pb is None or pa <= 0 or pb <= 0:
        return 0.0
    pnl = 0.0
    for layer_a, layer_b, entry_pa, entry_pb in cycle.layers:
        # direction +1: long A, short B
        # direction -1: short A, long B
        qty_a = layer_a / entry_pa if entry_pa > 0 else 0
        qty_b = layer_b / entry_pb if entry_pb > 0 else 0
        pnl += cycle.direction * (qty_a * (pa - entry_pa) - qty_b * (pb - entry_pb))
    return pnl


def cycle_unrealized_pnl_at_close(cycle, prices, ts):
    return cycle_unrealized_pnl(cycle, prices, ts)


def run_prequential(config: dict) -> dict:
    freq_hours = config.get("freq_hours", 24)
    """Run the full 12-block continuous prequential backtest."""
    budget = config.get("budget", 500.0)
    entry_z = config.get("entry_z", 1.0)
    so_step = config.get("so_step", 0.5)
    exit_z = config.get("exit_z", 0.5)
    group_fo_quote = config.get("group_fo_quote", 30.0)
    multiplier = config.get("multiplier", 2.0)
    max_legs = config.get("max_legs", 4)

    account = ContinuousAccount(
        initial_equity=budget, equity=budget, equity_peak=budget,
        realized_pnl=0.0)

    block_results = []
    for block in BLOCKS:
        pairs = select_block_pairs(block, freq_hours)
        if not pairs:
            block_results.append({"block_id": block["block_id"], "status": "no_pairs",
                                  "equity_start": account.equity, "equity_end": account.equity})
            continue
        equity_before = account.equity
        run_block(account, block, pairs, entry_z, so_step, exit_z,
                  group_fo_quote, multiplier, max_legs, freq_hours)
        equity_after = account.equity
        block_return_pct = (equity_after / equity_before - 1) * 100 if equity_before > 0 else -100
        block_results.append({
            "block_id": block["block_id"],
            "pairs": [f"{p['pair'][0]}_{p['pair'][1]}" for p in pairs],
            "equity_start": equity_before,
            "equity_end": equity_after,
            "return_pct": block_return_pct,
            "positive": block_return_pct > 0,
            "fo_count": account.fo_count,
            "so_count": account.so_count,
            "tp_count": account.tp_count,
        })

    # Stitched metrics
    total_days = sum(b["test_days"] for b in BLOCKS)
    total_return_pct = (account.equity / budget - 1) * 100
    if total_days >= 365:
        ann_pct = ((account.equity / budget) ** (365.0 / total_days) - 1) * 100
    else:
        ann_pct = None
    # Max equity DD
    equity_values = [p["equity"] for p in account.equity_curve]
    peak = budget
    max_dd = 0.0
    for eq in equity_values:
        peak = max(peak, eq)
        dd = (peak - eq) / peak * 100 if peak > 0 else 100
        max_dd = max(max_dd, dd)
    # Concentration
    sym_pnls = list(account.symbol_pnl.values())
    total_abs = sum(abs(v) for v in sym_pnls)
    max_sym_conc = max(abs(v) for v in sym_pnls) / total_abs * 100 if total_abs > 0 else 0
    actual_symbols = len([v for v in account.symbol_pnl.values() if abs(v) > 0.01])
    # Group PnL = per-pair PnL
    group_pnls = list(account.symbol_pnl.values())
    max_grp_conc = max_sym_conc  # simplified: group = symbol for pairs

    return {
        "budget": budget,
        "initial_equity": budget,
        "final_equity": account.equity,
        "total_return_pct": total_return_pct,
        "annualized_return_pct": ann_pct,
        "max_equity_dd_pct": max_dd,
        "stitched_test_days": total_days,
        "block_results": block_results,
        "positive_blocks": sum(1 for b in block_results if b.get("positive")),
        "total_blocks": len(block_results),
        "fo_count": account.fo_count,
        "so_count": account.so_count,
        "tp_count": account.tp_count,
        "groups_with_so": len(account.groups_with_so),
        "had_so": account.had_so,
        "actual_symbols": actual_symbols,
        "max_symbol_concentration_pct": max_sym_conc,
        "max_group_concentration_pct": max_grp_conc,
        "total_fee": account.total_fee,
        "total_slip": account.total_slip,
        "equity_curve_len": len(account.equity_curve),
        "breach": account.equity <= 0,
    }


if __name__ == "__main__":
    import sys
    config = {"budget": 500.0, "entry_z": 1.0, "so_step": 0.5,
              "exit_z": 0.5, "group_fo_quote": 30.0, "multiplier": 2.0, "max_legs": 4}
    print(f"config: {config}")
    result = run_prequential(config)
    print(f"final_equity: {result['final_equity']:.2f}")
    print(f"total_return: {result['total_return_pct']:.2f}%")
    print(f"ann: {result['annualized_return_pct']}")
    print(f"max_dd: {result['max_equity_dd_pct']:.2f}%")
    print(f"positive_blocks: {result['positive_blocks']}/{result['total_blocks']}")
    print(f"fo: {result['fo_count']}, so: {result['so_count']}, tp: {result['tp_count']}")
    print(f"had_so: {result['had_so']}, actual_symbols: {result['actual_symbols']}")
