#!/usr/bin/env python3
"""R22 Funding carry-reversal with CUSTOM entry mechanism.

The original run_block uses price residual_z as the entry signal, which
produces z=0 for (sym, sym) pairs. This module implements a standalone
prequential runner that uses the FUNDING RATE z-score as the entry signal.

Entry: when |funding_z| > entry_z, open position with direction = -sign(funding_z)
  - High positive funding (longs pay) → short (price overbought)
  - High negative funding (shorts pay) → long (price oversold)
SO: when position at loss and funding_z moved further adverse
TP: when position at profit and |funding_z| < exit_z
"""
import sys, json, math, sqlite3, time, os
from pathlib import Path
from dataclasses import dataclass, field
sys.path.insert(0, 'scripts')
from glm_r22_r3_prequential import BLOCKS, FEE_BPS, SLIP_BPS, load_log_prices_daily

ROOT = Path(__file__).resolve().parents[1]

FUNDING_SYMBOLS = ["BTCUSDT","ETHUSDT","BNBUSDT","SOLUSDT","XRPUSDT","DOGEUSDT",
    "ADAUSDT","TRXUSDT","LINKUSDT","LTCUSDT","BCHUSDT","DOTUSDT",
    "AVAXUSDT","ATOMUSDT","NEARUSDT","APTUSDT","AAVEUSDT",
    "ALGOUSDT","COMPUSDT","UNIUSDT","CRVUSDT","INJUSDT","DASHUSDT",
    "ETCUSDT","FILUSDT","ICPUSDT","ANKRUSDT","EGLDUSDT","HBARUSDT","ZECUSDT"]


def load_funding_rates(symbol, start_ms, end_ms):
    conn = sqlite3.connect(f"file:{ROOT}/data/funding_rates_round12.db?mode=ro", uri=True)
    cur = conn.cursor()
    cur.execute("SELECT funding_time, funding_rate FROM funding_rates "
                "WHERE symbol=? AND funding_time>=? AND funding_time<=? "
                "ORDER BY funding_time", (symbol, start_ms, end_ms))
    rows = cur.fetchall(); conn.close()
    return {t: r for t, r in rows}


def compute_funding_stats(funding_rates):
    vals = list(funding_rates.values())
    n = len(vals)
    if n < 10: return None
    mean = sum(vals) / n
    var = sum((v - mean) ** 2 for v in vals) / n
    std = var ** 0.5
    if std <= 0: return None
    return {"mean": mean, "std": std, "n": n}


@dataclass
class FundingAccount:
    initial_equity: float
    equity: float
    equity_peak: float
    realized_pnl: float
    events: list = field(default_factory=list)
    equity_curve: list = field(default_factory=list)
    active_positions: dict = field(default_factory=dict)  # sym -> {direction, layers, entry_prices}
    fo_count: int = 0
    so_count: int = 0
    tp_count: int = 0
    total_fee: float = 0.0
    total_slip: float = 0.0
    had_so: bool = False
    groups_with_so: set = field(default_factory=set)


def run_funding_prequential(config):
    budget = config.get("budget", 500.0)
    entry_z = config.get("entry_z", 1.5)
    so_step = config.get("so_step", 0.5)
    exit_z = config.get("exit_z", 0.5)
    fo = config.get("group_fo_quote", 30.0)
    multiplier = config.get("multiplier", 2.0)
    max_legs = config.get("max_legs", 4)

    account = FundingAccount(
        initial_equity=budget, equity=budget, equity_peak=budget, realized_pnl=0.0)

    for block in BLOCKS:
        # Load funding stats for fit window
        funding_stats = {}
        funding_data = {}
        for sym in FUNDING_SYMBOLS:
            fr = load_funding_rates(sym, block["fit_start_ms"], block["fit_end_ms"])
            stats = compute_funding_stats(fr)
            if stats:
                funding_stats[sym] = stats
                funding_data[sym] = fr
        if not funding_stats:
            continue

        # Select top-K symbols by |z| of most recent funding rate
        candidates = []
        for sym, stats in funding_stats.items():
            recent_fr = list(funding_data[sym].values())[-1] if funding_data[sym] else 0
            z = (recent_fr - stats["mean"]) / stats["std"] if stats["std"] > 0 else 0
            candidates.append((sym, z, stats))
        candidates.sort(key=lambda x: -abs(x[1]))
        selected = candidates[:8]  # top 8 by |z|

        # Load test-window funding + prices
        test_funding = {sym: load_funding_rates(sym, block["test_start_ms"], block["test_end_ms"])
                        for sym, _, _ in selected}
        test_prices = {sym: load_log_prices_daily(sym, block["test_start_ms"], block["test_end_ms"], 24)
                       for sym, _, _ in selected}

        # All timestamps (union of funding times and price times)
        all_ts = sorted(set().union(*[set(test_funding.get(s, {}).keys()) for s, _, _ in selected]))

        for ts in all_ts:
            # Record equity
            unreal = 0.0
            for sym, pos in account.active_positions.items():
                price = test_prices.get(sym, {}).get(ts)
                if price and price > 0:
                    for layer in pos["layers"]:
                        qty = layer["notional"] / layer["entry_price"] if layer["entry_price"] > 0 else 0
                        unreal += pos["direction"] * qty * (math.exp(price) - math.exp(layer["entry_price"]))
            eq = account.equity + unreal
            account.equity_peak = max(account.equity_peak, eq)
            account.equity_curve.append({"ts": ts, "equity": eq})

            for sym, _, stats in selected:
                fr = test_funding.get(sym, {}).get(ts)
                if fr is None:
                    continue
                z = (fr - stats["mean"]) / stats["std"] if stats["std"] > 0 else 0
                price = test_prices.get(sym, {}).get(ts)
                if price is None or price <= 0:
                    continue
                entry_price = price  # log price

                pos = account.active_positions.get(sym)
                if pos is None:
                    # Check FO
                    if abs(z) >= entry_z:
                        direction = -1 if z > 0 else 1  # short overbought, long oversold
                        layer_notional = fo
                        fee = layer_notional * FEE_BPS / 10_000
                        slip = layer_notional * SLIP_BPS / 10_000
                        if account.equity - fee - slip < 0:
                            continue
                        account.active_positions[sym] = {
                            "direction": direction,
                            "layers": [{"notional": layer_notional, "entry_price": entry_price}],
                            "last_z": z, "z_at_open": z,
                        }
                        account.equity -= fee + slip
                        account.realized_pnl -= fee + slip
                        account.total_fee += fee; account.total_slip += slip
                        account.fo_count += 1
                        account.events.append({"ts": ts, "type": "FO", "sym": sym, "z": z})
                else:
                    # Manage active position
                    unreal_pos = 0.0
                    for layer in pos["layers"]:
                        qty = layer["notional"] / layer["entry_price"] if layer["entry_price"] > 0 else 0
                        unreal_pos += pos["direction"] * qty * (math.exp(price) - math.exp(layer["entry_price"]))
                    depth = len(pos["layers"])

                    # TP
                    if unreal_pos > 0 and abs(z) <= exit_z and depth >= 1:
                        close_cost = sum(l["notional"] for l in pos["layers"]) * (FEE_BPS + SLIP_BPS) / 10_000
                        account.equity += unreal_pos - close_cost
                        account.realized_pnl += unreal_pos - close_cost
                        account.tp_count += 1
                        del account.active_positions[sym]
                        account.events.append({"ts": ts, "type": "TP", "sym": sym, "pnl": unreal_pos})
                        continue

                    # SO
                    if unreal_pos < 0 and depth < max_legs:
                        adverse = (z - pos["last_z"]) * (-pos["direction"])
                        if adverse >= so_step:
                            layer_notional = fo * multiplier ** depth
                            fee = layer_notional * FEE_BPS / 10_000
                            slip = layer_notional * SLIP_BPS / 10_000
                            pos["layers"].append({"notional": layer_notional, "entry_price": entry_price})
                            account.equity -= fee + slip
                            account.realized_pnl -= fee + slip
                            account.total_fee += fee; account.total_slip += slip
                            pos["last_z"] = z
                            account.so_count += 1
                            account.had_so = True
                            account.groups_with_so.add(sym)
                            account.events.append({"ts": ts, "type": "SO", "sym": sym, "depth": depth + 1})

                    pos["last_z"] = z

            # Block end: close remaining positions
        for sym in list(account.active_positions.keys()):
            pos = account.active_positions[sym]
            # Close at last known price
            prices = test_prices.get(sym, {})
            if prices:
                last_price = sorted(prices.values())[-1]
                unreal = 0.0
                for layer in pos["layers"]:
                    qty = layer["notional"] / layer["entry_price"] if layer["entry_price"] > 0 else 0
                    unreal += pos["direction"] * qty * (math.exp(last_price) - math.exp(layer["entry_price"]))
                close_cost = sum(l["notional"] for l in pos["layers"]) * (FEE_BPS + SLIP_BPS) / 10_000
                account.equity += unreal - close_cost
                account.realized_pnl += unreal - close_cost
            del account.active_positions[sym]

    total_days = sum(b["test_days"] for b in BLOCKS)
    total_return_pct = (account.equity / budget - 1) * 100
    ann = ((account.equity / budget) ** (365.0 / total_days) - 1) * 100 if total_days >= 365 and account.equity > 0 else None
    equity_values = [p["equity"] for p in account.equity_curve]
    peak = budget; max_dd = 0.0
    for eq in equity_values:
        peak = max(peak, eq)
        dd = (peak - eq) / peak * 100 if peak > 0 else 100
        max_dd = max(max_dd, dd)
    return {
        "total_return_pct": total_return_pct,
        "annualized_return_pct": ann,
        "max_equity_dd_pct": max_dd,
        "had_so": account.had_so,
        "fo_count": account.fo_count,
        "so_count": account.so_count,
        "tp_count": account.tp_count,
        "breach": account.equity <= 0,
    }


if __name__ == "__main__":
    print("=== Funding carry-reversal v2 (custom entry) ===", flush=True)
    for mult in [1.5, 2.0, 3.0, 4.0]:
        for fo in [10, 30, 50, 100]:
            for ez in [1.0, 1.5, 2.0]:
                r = run_funding_prequential({'budget':500.0,'entry_z':ez,'so_step':0.5,'exit_z':0.5,
                    'group_fo_quote':float(fo),'multiplier':mult,'max_legs':4})
                ret = r['total_return_pct']
                ann = r['annualized_return_pct']
                dd = r['max_equity_dd_pct']
                tier = ''
                if ann and ann >= 50 and dd <= 10: tier = '*** CONSERVATIVE ***'
                print(f'm={mult} fo={fo} ez={ez}: ret={ret:.1f}% ann={ann if ann is None else round(ann,1)}% dd={dd:.1f}% fo={r["fo_count"]} so={r["so_count"]} tp={r["tp_count"]} {tier}', flush=True)
