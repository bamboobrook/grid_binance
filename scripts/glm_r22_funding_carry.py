#!/usr/bin/env python3
"""R22 Funding rate carry-reversal selector.

Signal: when funding rate is extremely positive (longs pay shorts), the
asset is overbought → short. When extremely negative, overbought shorts → long.
This is a carry-reversal: the funding cost itself creates mean-reversion pressure.

The selector picks single-asset "pairs" where one leg is the perpetual and
the "pair" is the funding rate z-score. Entry when |funding z| > threshold.
"""
import sys, json, math, sqlite3, time
from pathlib import Path
sys.path.insert(0, 'scripts')
import glm_r22_r3_prequential as r3mod
from glm_r22_r3_prequential import (run_prequential, BLOCKS, ContinuousAccount,
    ActiveCycle, load_log_prices_daily, FEE_BPS, SLIP_BPS,
    residual_z, cycle_realized_pnl, cycle_unrealized_pnl,
    cycle_unrealized_pnl_at_close, run_block, UNIVERSE)

ROOT = Path(__file__).resolve().parents[1]

FUNDING_SYMBOLS = ["BTCUSDT","ETHUSDT","BNBUSDT","SOLUSDT","XRPUSDT","DOGEUSDT",
    "ADAUSDT","TRXUSDT","LINKUSDT","LTCUSDT","BCHUSDT","DOTUSDT",
    "AVAXUSDT","ATOMUSDT","NEARUSDT","APTUSDT","AAVEUSDT",
    "ALGOUSDT","COMPUSDT","UNIUSDT","CRVUSDT","INJUSDT","DASHUSDT",
    "ETCUSDT","FILUSDT","ICPUSDT","ANKRUSDT","EGLDUSDT","HBARUSDT","ZECUSDT"]


def load_funding_rates(symbol, start_ms, end_ms):
    """Load funding rates for a symbol from the funding DB."""
    conn = sqlite3.connect(f"file:{ROOT}/data/funding_rates_round12.db?mode=ro", uri=True)
    cur = conn.cursor()
    cur.execute("SELECT funding_time, funding_rate FROM funding_rates "
                "WHERE symbol=? AND funding_time>=? AND funding_time<=? "
                "ORDER BY funding_time", (symbol, start_ms, end_ms))
    rows = cur.fetchall()
    conn.close()
    return {t: r for t, r in rows}


def compute_funding_stats(funding_rates):
    """Compute mean, std, z-scores of funding rates."""
    vals = list(funding_rates.values())
    n = len(vals)
    if n < 10:
        return None
    mean = sum(vals) / n
    var = sum((v - mean) ** 2 for v in vals) / n
    std = var ** 0.5
    if std <= 0:
        return None
    return {"mean": mean, "std": std, "n": n}


def select_funding_carry_pairs(block):
    """Select assets with extreme funding rates for carry-reversal.

    For each asset, compute funding rate z-score. Select those with
    |z| > 1.5 (extreme funding). These become single-asset "pairs" where
    the signal is the funding z-score, not a cointegration residual.
    """
    candidates = []
    for sym in FUNDING_SYMBOLS:
        fr = load_funding_rates(sym, block["fit_start_ms"], block["fit_end_ms"])
        stats = compute_funding_stats(fr)
        if stats is None:
            continue
        # Load price data for this asset
        prices = load_log_prices_daily(sym, block["fit_start_ms"], block["fit_end_ms"])
        if len(prices) < 30:
            continue
        # The "residual" is the funding rate z-score
        # Use recent funding rate as the current signal
        recent_fr = list(fr.values())[-1] if fr else 0
        z = (recent_fr - stats["mean"]) / stats["std"] if stats["std"] > 0 else 0
        # Compute price half-life for gating
        price_vals = sorted(prices.values())
        if len(price_vals) < 30:
            continue
        # Simple price momentum check: is the asset mean-reverting?
        rets = [price_vals[i] - price_vals[i-1] for i in range(1, min(len(price_vals), 60))]
        if not rets:
            continue
        mean_ret = sum(rets) / len(rets)
        var_ret = sum((r - mean_ret) ** 2 for r in rets) / len(rets)
        if var_ret <= 0:
            continue
        phi = sum((rets[i]) * (rets[i-1]) for i in range(1, len(rets))) / (len(rets) * var_ret)
        hl = (-0.693147 / math.log(phi)) if 0 < phi < 1 else 999.0
        if hl >= 168:
            continue
        # Score: prefer high |z| and short half-life
        score = abs(z) / max(hl / 24, 1.0)
        candidates.append((sym, z, stats, hl, score, prices))

    candidates.sort(key=lambda x: -x[4])
    groups = []
    used = set()
    for sym, z, stats, hl, score, prices in candidates:
        if sym in used:
            continue
        # Create a "pair" with the asset vs itself (single-asset carry trade)
        # The residual sigma is the funding rate std
        groups.append({
            "pair": (sym, sym),  # single-asset (carry on perp only)
            "beta": 1.0,
            "mu": stats["mean"],
            "sigma": stats["std"],
            "half_life_h": hl * 24,
            "adf_t": -3.0,  # placeholder (funding-based, not ADF)
            "ac1": phi,
            "mr_sharpe": score,
            "n": stats["n"],
            "funding_z": z,
        })
        used.add(sym)
        if len(groups) >= 6:
            break
    return groups


def run_prequential_funding(config):
    """Run prequential with funding carry-reversal selector."""
    budget = config.get("budget", 500.0)
    entry_z = config.get("entry_z", 1.5)  # funding z threshold
    so_step = config.get("so_step", 0.5)
    exit_z = config.get("exit_z", 0.5)
    fo = config.get("group_fo_quote", 30.0)
    multiplier = config.get("multiplier", 2.0)
    max_legs = config.get("max_legs", 4)
    freq_hours = config.get("freq_hours", 24)

    account = ContinuousAccount(
        initial_equity=budget, equity=budget, equity_peak=budget, realized_pnl=0.0)

    for block in BLOCKS:
        pairs = select_funding_carry_pairs(block)
        if not pairs:
            continue
        run_block(account, block, pairs, entry_z, so_step, exit_z,
                  fo, multiplier, max_legs, freq_hours)

    total_days = sum(b["test_days"] for b in BLOCKS)
    total_return_pct = (account.equity / budget - 1) * 100
    ann = ((account.equity / budget) ** (365.0 / total_days) - 1) * 100 if total_days >= 365 else None
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
        "positive_blocks": sum(1 for b in account.events if b.get("type") == "TP"),
        "had_so": account.had_so,
        "fo_count": account.fo_count,
        "so_count": account.so_count,
        "breach": account.equity <= 0,
    }


if __name__ == "__main__":
    print("=== Funding carry-reversal selector ===", flush=True)
    for mult in [1.5, 2.0, 3.0, 4.0]:
        for fo in [10, 30, 50, 100]:
            for ez in [1.0, 1.5, 2.0]:
                cfg = {'budget':500.0,'entry_z':ez,'so_step':0.5,'exit_z':0.5,
                       'group_fo_quote':float(fo),'multiplier':mult,'max_legs':4}
                r = run_prequential_funding(cfg)
                ret = r['total_return_pct']
                ann = r['annualized_return_pct']
                dd = r['max_equity_dd_pct']
                tier = ''
                if ann and ann >= 50 and dd <= 10: tier = '*** CONSERVATIVE ***'
                if ret > 0:
                    print(f'm={mult} fo={fo} ez={ez}: ret={ret:.1f}% ann={ann if ann is None else round(ann,1)}% dd={dd:.1f}% so={r["had_so"]} {tier}', flush=True)
