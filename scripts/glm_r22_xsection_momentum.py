#!/usr/bin/env python3
"""R22 Cross-section momentum reversal selector.

Signal: rank assets by recent return (e.g. 7-day momentum). Short the
top winners (overbought) and long the bottom losers (oversold). This is
a cross-section contrarian signal, different from pair cointegration.

Pairs: (winner, loser) — short winner, long loser. The "residual" is
the spread between winner and loser returns.
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


def compute_cross_section_returns(prices, lookback_days=7):
    """Compute recent return for each asset and rank them."""
    returns = {}
    for sym, p in prices.items():
        sorted_ts = sorted(p.keys())
        if len(sorted_ts) < lookback_days + 5:
            continue
        recent = sorted_ts[-lookback_days:]
        start_price = p[recent[0]]
        end_price = p[recent[-1]]
        if start_price > 0:
            returns[sym] = end_price - start_price  # log return
    return returns


def select_cross_section_pairs(block):
    """Select (winner, loser) pairs based on cross-section momentum."""
    prices = {s: load_log_prices_daily(s, block["fit_start_ms"], block["fit_end_ms"])
              for s in UNIVERSE}
    returns = compute_cross_section_returns(prices, 7)
    if len(returns) < 6:
        return []

    sorted_syms = sorted(returns.keys(), key=lambda s: returns[s])
    n = len(sorted_syms)
    # Bottom 1/3 = losers (long), top 1/3 = winners (short)
    losers = sorted_syms[:n//3]
    winners = sorted_syms[n - n//3:]

    pairs = []
    # Match strongest loser with strongest winner
    for i in range(min(len(losers), len(winners), 6)):
        loser = losers[i]  # long this (low return = oversold)
        winner = winners[-(i+1)]  # short this (high return = overbought)
        # Compute pair stats
        la = prices[loser]; lb = prices[winner]
        common = sorted(set(la) & set(lb))
        if len(common) < 30:
            continue
        # Simple spread: log(loser) - log(winner)
        spread = [la[t] - lb[t] for t in common]
        mu = sum(spread) / len(spread)
        var = sum((s - mu) ** 2 for s in spread) / len(spread)
        sigma = var ** 0.5
        if sigma <= 0:
            continue
        # Z-score of current spread
        z = (spread[-1] - mu) / sigma
        # MR stats
        phi = sum((spread[j] - mu) * (spread[j-1] - mu) for j in range(1, len(spread))) / (len(spread) * var)
        hl = (-0.693147 / math.log(phi)) if 0 < phi < 1 else 999.0
        # Simple Sharpe
        pnl = []; pos = 0
        zs = [(s - mu) / sigma for s in spread]
        for j in range(1, len(zs)):
            if pos == 0 and abs(zs[j-1]) > 1.0: pos = -1 if zs[j-1] > 0 else 1
            elif pos != 0 and abs(zs[j-1]) < 0.5: pos = 0
            if pos != 0: pnl.append(pos * (zs[j] - zs[j-1]) * sigma)
        sharpe = 0.0
        if len(pnl) > 5:
            m = sum(pnl)/len(pnl); v = sum((p-m)**2 for p in pnl)/len(pnl)
            sharpe = (m/v**0.5)*(len(pnl)**0.5) if v > 0 else 0

        pairs.append({
            "pair": (loser, winner),
            "beta": 1.0,  # spread is log-log, beta=1
            "mu": mu,
            "sigma": sigma,
            "half_life_h": hl * 24,
            "adf_t": -3.0,  # placeholder
            "ac1": phi,
            "mr_sharpe": sharpe,
            "n": len(common),
            "z_current": z,
        })

    pairs.sort(key=lambda x: -x["mr_sharpe"])
    return pairs[:6]


if __name__ == "__main__":
    # Quick test: select pairs on block 0
    PROTOCOL = json.load(open(Path(__file__).resolve().parents[1] /
        "docs/superpowers/artifacts/glm-martingale-core-round22/r2/prequential-protocol.json"))
    block = PROTOCOL["test_blocks"][0]
    pairs = select_cross_section_pairs(block)
    print(f"Block {block['block_id']}: {len(pairs)} cross-section pairs", flush=True)
    for p in pairs[:3]:
        print(f"  long={p['pair'][0]} short={p['pair'][1]} hl={p['half_life_h']:.1f}h sharpe={p['mr_sharpe']:.2f}", flush=True)

    # Run prequential with this selector
    import glm_r22_r3_prequential as r3mod
    from glm_r22_r3_prequential import run_prequential

    orig_select = r3mod.select_block_pairs
    r3mod.select_block_pairs = lambda block, freq_hours=24: select_cross_section_pairs(block)
    print(flush=True)
    print("=== Cross-section momentum reversal on prequential ===", flush=True)
    for mult in [1.5, 2.0, 3.0, 4.0]:
        for fo in [10, 30, 50, 100]:
            for ez in [0.5, 1.0, 1.5]:
                r = run_prequential({'budget':500.0,'entry_z':ez,'so_step':0.5,'exit_z':0.5,
                    'group_fo_quote':float(fo),'multiplier':mult,'max_legs':4})
                ret = r['total_return_pct']
                ann = r['annualized_return_pct']
                dd = r['max_equity_dd_pct']
                tier = ''
                if ann and not isinstance(ann, complex) and ann >= 50 and dd <= 10: tier = '*** CONSERVATIVE ***'
                if ret > 0:
                    print(f'm={mult} fo={fo} ez={ez}: ret={ret:.1f}% ann={ann if ann is None or isinstance(ann,complex) else round(ann,1)}% dd={dd:.1f}% so={r["had_so"]} {tier}', flush=True)
    r3mod.select_block_pairs = orig_select
