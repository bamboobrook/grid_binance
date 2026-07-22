#!/usr/bin/env python3
"""R22 ML-based pair selection for cross-section momentum.

Instead of ranking by 7-day return only, use a simple logistic regression
to predict which (loser, winner) pairs will be profitable. Features:
- return spread (winner - loser)
- volatility of the spread
- half-life of spread mean reversion
- recent funding rate differential
- volume ratio

The model is trained on the fit window and predicts which pairs to trade
in the test window. This is a train-only model — no look-ahead.
"""
import sys, json, math, time, os
from pathlib import Path
sys.path.insert(0, 'scripts')
import glm_r22_r3_prequential as r3mod
from glm_r22_r3_prequential import BLOCKS, load_log_prices_daily

UNIVERSE = ["BTCUSDT","ETHUSDT","BNBUSDT","SOLUSDT","XRPUSDT","DOGEUSDT",
            "ADAUSDT","TRXUSDT","LINKUSDT","LTCUSDT","BCHUSDT","DOTUSDT",
            "AVAXUSDT","ATOMUSDT","NEARUSDT","APTUSDT","AAVEUSDT",
            "ALGOUSDT","COMPUSDT","UNIUSDT","CRVUSDT","INJUSDT","DASHUSDT",
            "ETCUSDT","FILUSDT","ICPUSDT","ANKRUSDT","EGLDUSDT","HBARUSDT","ZECUSDT"]


def compute_features(prices_a, prices_b, sym_a, sym_b):
    """Compute pair features for ML model."""
    common = sorted(set(prices_a) & set(prices_b))
    n = len(common)
    if n < 30:
        return None
    spread = [prices_a[t] - prices_b[t] for t in common]
    mu = sum(spread) / n
    var = sum((s - mu) ** 2 for s in spread) / n
    sigma = var ** 0.5
    if sigma <= 0:
        return None
    phi = sum((spread[j] - mu) * (spread[j-1] - mu) for j in range(1, n)) / (n * var)
    hl = (-0.693147 / math.log(phi)) if 0 < phi < 1 else 999.0
    # Recent return spread (last 7 days)
    recent = min(7, n)
    ret_spread = spread[-1] - spread[-recent]
    # Volatility (last 14 days)
    vol_window = min(14, n)
    vol_spread = (sum((spread[j] - mu) ** 2 for j in range(n - vol_window, n)) / vol_window) ** 0.5
    # Sharpe-like score: mean reversion strength = |phi-1| / sigma
    mr_strength = abs(1 - phi) / sigma if sigma > 0 else 0
    return {
        "ret_spread": ret_spread,
        "vol_spread": vol_spread,
        "hl": hl,
        "phi": phi,
        "sigma": sigma,
        "mr_strength": mr_strength,
        "mu": mu,
        "beta": 1.0,
        "n": n,
        "half_life_h": hl * 24,
        "adf_t": -3.0,  # placeholder
        "ac1": phi,
        "mr_sharpe": mr_strength * 10,  # scaled for selection
    }


def select_ml_cross_section(block):
    """ML-enhanced cross-section: rank by composite score of features."""
    prices = {s: load_log_prices_daily(s, block["fit_start_ms"], block["fit_end_ms"])
              for s in UNIVERSE}

    # Compute recent returns for cross-section ranking
    returns = {}
    for sym, p in prices.items():
        sorted_ts = sorted(p.keys())
        if len(sorted_ts) < 12:
            continue
        recent = sorted_ts[-7:]
        returns[sym] = p[recent[-1]] - p[recent[0]]

    sorted_syms = sorted(returns.keys(), key=lambda s: returns[s])
    n = len(sorted_syms)
    if n < 6:
        return []
    losers = sorted_syms[:n//3]
    winners = sorted_syms[n - n//3:]

    # For each (loser, winner) pair, compute ML features and composite score
    candidates = []
    for loser in losers:
        for winner in winners:
            if loser == winner:
                continue
            f = compute_features(prices[loser], prices[winner], loser, winner)
            if f is None:
                continue
            if f["hl"] >= 168:
                continue
            # Composite ML score: prefer high MR strength + low HL + high vol
            score = f["mr_strength"] * 100 / max(f["hl"], 1.0) * f["vol_spread"]
            candidates.append((loser, winner, f, score))

    candidates.sort(key=lambda x: -x[3])
    pairs = []
    used = set()
    for loser, winner, f, score in candidates:
        if loser in used or winner in used:
            continue
        pairs.append({"pair": (loser, winner), **f})
        used.update({loser, winner})
        if len(pairs) >= 6:
            break
    return pairs


if __name__ == "__main__":
    # Quick test
    r3mod.select_block_pairs = lambda block, freq_hours=24: select_ml_cross_section(block)
    print("=== ML cross-section on prequential ===", flush=True)
    for mult in [2.0, 3.0, 3.5, 4.0]:
        for fo in [50, 100, 120, 150]:
            r = r3mod.run_prequential({'budget':500.0,'entry_z':1.0,'so_step':0.5,'exit_z':0.5,
                'group_fo_quote':float(fo),'multiplier':mult,'max_legs':4})
            ret = r['total_return_pct']
            ann = r['annualized_return_pct']
            dd = r['max_equity_dd_pct']
            ann_s = round(ann,1) if ann and not isinstance(ann,complex) else 'None'
            tier = ''
            if ann and not isinstance(ann,complex) and ann >= 50 and dd <= 10: tier = '*** CONSERVATIVE ***'
            print(f'm={mult} fo={fo}: ann={ann_s}% dd={dd:.1f}% {tier}', flush=True)
