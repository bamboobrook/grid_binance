#!/usr/bin/env python3
"""R22 DD-based position sizing + volatility-scaled entry.

Two mechanisms to improve the ann-DD frontier:
1. DD-based reduction: when current DD > threshold%, reduce fo_quote by a factor.
   This caps the worst-case DD while allowing full size when DD is low.
2. Volatility-scaled entry: scale entry_z by recent realized vol. High vol
   => higher threshold => fewer trades but safer. Low vol => more trades.

Both are tested on the S2 KSS selector with the continuous prequential engine.
"""
import sys, json, time, os, math
sys.path.insert(0, 'scripts')
import glm_r22_r3_prequential as r3mod
from glm_r22_r3_prequential import (run_prequential, BLOCKS, ContinuousAccount,
    ActiveCycle, load_log_prices_daily, FEE_BPS, SLIP_BPS, select_block_pairs,
    residual_z, cycle_realized_pnl, cycle_unrealized_pnl, cycle_unrealized_pnl_at_close,
    run_block, UNIVERSE)
from glm_r22_selectors import select_block_pairs as sel_select


def run_prequential_dd_sizing(config):
    """Prequential with DD-based position sizing."""
    budget = config.get("budget", 500.0)
    entry_z = config.get("entry_z", 1.0)
    so_step = config.get("so_step", 0.5)
    exit_z = config.get("exit_z", 0.5)
    base_fo = config.get("group_fo_quote", 30.0)
    multiplier = config.get("multiplier", 2.0)
    max_legs = config.get("max_legs", 4)
    dd_threshold = config.get("dd_threshold", 5.0)  # start reducing above this DD%
    dd_reduction = config.get("dd_reduction", 0.5)  # reduce to 50% of base
    freq_hours = config.get("freq_hours", 24)

    account = ContinuousAccount(
        initial_equity=budget, equity=budget, equity_peak=budget, realized_pnl=0.0)

    for block in BLOCKS:
        pairs = sel_select('S2_KSS', block)
        if not pairs:
            continue
        equity_before = account.equity
        # Dynamic fo based on current DD
        current_dd = (account.equity_peak - account.equity) / account.equity_peak * 100 if account.equity_peak > 0 else 0
        if current_dd > dd_threshold:
            effective_fo = base_fo * dd_reduction
        else:
            effective_fo = base_fo
        run_block(account, block, pairs, entry_z, so_step, exit_z,
                  effective_fo, multiplier, max_legs, freq_hours)

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


def run_prequential_vol_scaled(config):
    """Prequential with volatility-scaled entry_z."""
    budget = config.get("budget", 500.0)
    base_entry_z = config.get("entry_z", 1.0)
    so_step = config.get("so_step", 0.5)
    exit_z = config.get("exit_z", 0.5)
    fo = config.get("group_fo_quote", 30.0)
    multiplier = config.get("multiplier", 2.0)
    max_legs = config.get("max_legs", 4)
    vol_scale = config.get("vol_scale", 1.0)  # multiplier on entry_z
    freq_hours = config.get("freq_hours", 24)

    account = ContinuousAccount(
        initial_equity=budget, equity=budget, equity_peak=budget, realized_pnl=0.0)

    for block in BLOCKS:
        pairs = sel_select('S2_KSS', block)
        if not pairs:
            continue
        # Compute recent realized vol for scaling
        prices_data = {s: load_log_prices_daily(s, block["fit_start_ms"], block["fit_end_ms"], freq_hours)
                       for s in UNIVERSE[:5]}  # sample for speed
        all_rets = []
        for s in prices_data:
            p = sorted(prices_data[s].values())
            for i in range(1, min(len(p), 60)):
                all_rets.append(abs(p[i] - p[i-1]))
        avg_vol = sum(all_rets) / len(all_rets) if all_rets else 0.03
        # Scale entry_z inversely with vol (high vol => higher z => fewer trades)
        vol_factor = max(0.5, min(2.0, 0.03 / avg_vol)) if avg_vol > 0 else 1.0
        effective_entry_z = base_entry_z * vol_factor * vol_scale
        run_block(account, block, pairs, effective_entry_z, so_step, exit_z,
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
        "breach": account.equity <= 0,
    }


if __name__ == "__main__":
    print("=== DD-based position sizing ===", flush=True)
    for mult in [2.0, 3.0, 4.0]:
        for fo in [30, 50, 100]:
            for dd_thresh in [3.0, 5.0, 8.0]:
                cfg = {'budget':500.0,'entry_z':1.0,'so_step':0.5,'exit_z':0.5,
                       'group_fo_quote':float(fo),'multiplier':mult,'max_legs':4,
                       'dd_threshold':dd_thresh,'dd_reduction':0.5}
                r = run_prequential_dd_sizing(cfg)
                ret = r['total_return_pct']
                ann = r['annualized_return_pct']
                dd = r['max_equity_dd_pct']
                tier = ''
                if ann and ann >= 50 and dd <= 10: tier = '*** CONSERVATIVE ***'
                print(f'DD-sizing m={mult} fo={fo} dd_thresh={dd_thresh}: ret={ret:.1f}% ann={ann if ann is None else round(ann,1)}% dd={dd:.1f}% {tier}', flush=True)

    print(flush=True)
    print("=== Volatility-scaled entry ===", flush=True)
    for mult in [2.0, 3.0, 4.0]:
        for fo in [30, 50, 100]:
            for vs in [0.8, 1.0, 1.2]:
                cfg = {'budget':500.0,'entry_z':1.0,'so_step':0.5,'exit_z':0.5,
                       'group_fo_quote':float(fo),'multiplier':mult,'max_legs':4,
                       'vol_scale':vs}
                r = run_prequential_vol_scaled(cfg)
                ret = r['total_return_pct']
                ann = r['annualized_return_pct']
                dd = r['max_equity_dd_pct']
                tier = ''
                if ann and ann >= 50 and dd <= 10: tier = '*** CONSERVATIVE ***'
                print(f'Vol-scaled m={mult} fo={fo} vs={vs}: ret={ret:.1f}% ann={ann if ann is None else round(ann,1)}% dd={dd:.1f}% {tier}', flush=True)
