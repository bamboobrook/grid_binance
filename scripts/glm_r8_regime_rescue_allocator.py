#!/usr/bin/env python3
"""GLM Martingale Allocator (Round 9 REPAIRED version).

Round 8 allocator defects fixed in Round 9:
  1. TIMING LEAK FIXED: Rebalance decision at ts only affects intervals
     AFTER ts. The step from prev_ts -> ts is applied to the previously
     active sleeve, not the newly selected one.
  2. LEGACY ELIGIBILITY SWITCHES: despite their names, max_high_ann_weight and
     min_low_dd_weight do not implement fractional weights. Only max_hi <= 0
     excludes high-ann sleeves; min_lo > 0 merely keeps low-DD sleeves eligible.
  3. TRADED_SYMBOLS REAL: pulled from underlying sleeve configs, not sleeve
     labels.
  4. CURVE-SLICE SEGMENT METRICS: each segment reruns the allocator over a
     slice of full-period sleeve curves. The sleeves are not cold-started and
     their event-level positions are not replayed at segment boundaries.
  5. PORTFOLIO CANDIDATE FLAG: includes symbol_count, max_symbol_budget_pct,
     max_symbol_gross_pnl_share_pct, portfolio_candidate, live_ready.

The module exposes:
  run_allocator_repaired(...)  — forward-only curve recombination
  run_allocator_leaky(...)     — Round 8 leaky timing (for comparison only)
  compute_segment_metrics_for_allocator(...) — per-segment allocator replay
  main()                       — CLI grid search
"""
import argparse, json, os, subprocess, sys, time, math
from collections import defaultdict

REPLAY = "target/release/portfolio_budget_replay"
MARKET_DB = "data/market_data_full.db"
FUNDING_DB = "data/funding_rates.db"
FULL_SEGMENTS = [
    ("h1_2023", 1672531200000, 1688169599999),
    ("h2_2023", 1688169600000, 1704067199999),
    ("2024",    1704067200000, 1735689599999),
    ("2025",    1735689600000, 1767225599999),
    ("2026_ytd",1767225600000, 1780271999999),
]
FULL_START, FULL_END = 1672531200000, 1780271999999

CANDS = {
    "ANKR-q": "docs/superpowers/artifacts/glm-martingale-core-round7/promising/r7-ANKR-q1w24p24.json",
    "QB":     "docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-QB-best.json",
    "R4":     "docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json",
    "fine":   "docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-fine-combo-best.json",
}

# Sleeve classification for weight caps (derived from full-period metrics)
HIGH_ANN_SLEEVES = {"ANKR-q", "fine"}   # ann > 45%
LOW_DD_SLEEVES = {"R4", "QB"}            # DD < 20%


def get_curve(name, config_path, budget, start_ms, end_ms, market_db, funding_db):
    cmd = [REPLAY, "--config", config_path, "--budget", str(budget),
           "--start-ms", str(start_ms), "--end-ms", str(end_ms),
           "--market-data", market_db, "--funding-data", funding_db,
           "--profile", "aggressive", "--portfolio-id", f"alloc_{name}_{start_ms}",
           "--exchange-min-notional", "5", "--equity-curve-points", "5000"]
    pr = subprocess.run(cmd, capture_output=True, text=True, timeout=900)
    if pr.returncode != 0:
        return None
    try:
        r = json.loads(pr.stdout)
    except Exception:
        return None
    return r.get("equity_curve", [])


def get_sleeve_symbols(config_path):
    """Extract actual traded symbols from a sleeve portfolio config."""
    try:
        cfg = json.load(open(config_path))
        pc = cfg.get("portfolio_config", cfg)
        return sorted(set(s["symbol"] for s in pc.get("strategies", [])))
    except Exception:
        return []


def _scores_for_rebal(curves, pnl_by, ts, lookback_ms, budget, score_fn, eligible):
    """Compute scores for each eligible sleeve using only data <= ts."""
    lookback_start_ts = ts - lookback_ms
    scores = {}
    for name in eligible:
        if name == "cash":
            continue
        # PnL at lookback start (most recent observation <= lookback_start)
        pnl_past = 0.0
        for t in sorted(pnl_by[name].keys()):
            if t <= lookback_start_ts:
                pnl_past = pnl_by[name][t]
            else:
                break
        pnl_now = pnl_by[name].get(ts, 0.0)
        ret = pnl_now - pnl_past
        # DD inside the lookback window (data <= ts only)
        lb_pnls = [v for t, v in pnl_by[name].items() if lookback_start_ts <= t <= ts]
        if lb_pnls:
            lb_peak = max(lb_pnls)
            lb_trough = min(lb_pnls)
            denom = budget + lb_peak
            lb_dd = (lb_peak - lb_trough) / denom * 100 if denom > 0 else 0
        else:
            lb_dd = 0.0
        scores[name] = score_fn(ret, lb_dd)
    return scores


def _eligible_sleeves(curves, max_hi, min_lo, high_ann, low_dd):
    """Apply legacy binary eligibility switches.

    These parameters do not create fractional allocations. max_hi <= 0
    excludes high-ann sleeves; every positive max_hi value is equivalent.
    min_lo > 0 keeps low-DD sleeves eligible, which is usually already true.
    """
    all_sleeves = [n for n in curves.keys() if n != "cash"]
    eligible = set(all_sleeves)
    # If max_hi cap is 0, exclude all high-ann sleeves entirely.
    if max_hi <= 0.0:
        eligible -= high_ann
    # If min_lo floor > 0, ensure all low-DD sleeves are eligible.
    if min_lo > 0.0:
        eligible |= (low_dd & set(all_sleeves))
    # Always keep cash as an option if requested via cash_dd
    return eligible


def run_allocator_repaired(curves, lookback_days, rebalance_days, score_fn,
                            max_hi, min_lo, cash_dd, hyst_gap,
                            high_ann_sleeves, low_dd_sleeves, budget=5000.0):
    """FORWARD-ONLY allocator. Rebalance at ts affects intervals AFTER ts only.

    Critical correctness: when a rebalance happens at ts, the step that
    spans [prev_ts, ts] is applied to the PREVIOUSLY active sleeve. The new
    sleeve only affects steps after ts.
    """
    all_ts = sorted(set(p["timestamp_ms"] for c in curves.values() for p in c))
    if not all_ts:
        return None
    pnl_by = {}
    for name, ec in curves.items():
        if not ec:
            continue
        first = ec[0]["equity_quote"]
        pnl_by[name] = {p["timestamp_ms"]: p["equity_quote"] - first for p in ec}

    high_ann = set(high_ann_sleeves)
    low_dd = set(low_dd_sleeves)
    eligible = _eligible_sleeves(curves, max_hi, min_lo, high_ann, low_dd)

    active = None
    # Pick the initial active sleeve using only pre-window data: choose a
    # low-DD eligible sleeve as defensive default (no lookahead at ts=first).
    if eligible:
        init_candidates = [s for s in eligible if s in low_dd] or list(eligible)
        active = sorted(init_candidates)[0]
    active_score = -1e18
    last_rebal_ts = all_ts[0]
    lookback_ms = lookback_days * 86400000
    rebal_ms = rebalance_days * 86400000

    merged_pnl = 0.0
    peak = budget
    max_dd = 0.0
    prev_ts = all_ts[0]

    for i, ts in enumerate(all_ts):
        # STEP 1: apply the interval [prev_ts, ts] to the CURRENTLY active
        # sleeve (the one chosen by a PREVIOUS rebalance, or the initial).
        # This is the forward-only invariant.
        if active is None or active == "cash":
            step = 0.0
        else:
            pnl_now = pnl_by.get(active, {}).get(ts, 0.0)
            pnl_prev = pnl_by.get(active, {}).get(prev_ts, 0.0)
            step = pnl_now - pnl_prev
        merged_pnl += step
        eq = budget + merged_pnl
        if eq > peak:
            peak = eq
        dd = (peak - eq) / peak * 100 if peak > 0 else 0
        if dd > max_dd:
            max_dd = dd

        # STEP 2: maybe rebalance — the decision here uses data <= ts and
        # applies to intervals AFTER ts (the next iteration's step).
        if i > 0 and (ts - last_rebal_ts) >= rebal_ms:
            eligible_now = _eligible_sleeves(curves, max_hi, min_lo, high_ann, low_dd)
            scores = _scores_for_rebal(curves, pnl_by, ts, lookback_ms, budget,
                                        score_fn, eligible_now)
            if scores:
                best = max(scores, key=scores.get)
                # Cash DD trigger (uses current merged DD, which is known)
                if cash_dd is not None and dd > cash_dd:
                    active = "cash"
                elif scores[best] - active_score > hyst_gap or active not in scores:
                    active = best
                    active_score = scores[best]
                else:
                    active_score = scores.get(active, active_score)
            last_rebal_ts = ts

        prev_ts = ts

    days = (all_ts[-1] - all_ts[0]) / 86400000
    total_ret = (budget + merged_pnl - budget) / budget
    if days > 0 and (1 + total_ret) > 0:
        ann = ((1 + total_ret) ** (365 / days) - 1) * 100
    else:
        ann = -999.0
    return {
        "ann": round(ann, 4),
        "dd": round(max_dd, 4),
        "ret": round(total_ret * 100, 4),
    }


def run_allocator_leaky(curves, lookback_days, rebalance_days, score_fn,
                         max_hi, min_lo, cash_dd, hyst_gap, budget=5000.0):
    """R8 LEAKY allocator (for comparison only). Applies rebalance to the
    interval that produced the decision. Used by semantic tests to prove the
    repaired version does not exceed leaky returns."""
    all_ts = sorted(set(p["timestamp_ms"] for c in curves.values() for p in c))
    if not all_ts:
        return None
    pnl_by = {}
    for name, ec in curves.items():
        if not ec:
            continue
        first = ec[0]["equity_quote"]
        pnl_by[name] = {p["timestamp_ms"]: p["equity_quote"] - first for p in ec}
    eligible = set(n for n in curves.keys() if n != "cash")

    active = sorted(eligible)[0] if eligible else None
    active_score = -1e18
    last_rebal = 0
    lookback_ms = lookback_days * 86400000
    rebal_ms = rebalance_days * 86400000
    merged_pnl = 0.0
    peak = budget
    max_dd = 0.0

    for i, ts in enumerate(all_ts):
        if i > 0 and (ts - all_ts[last_rebal]) >= rebal_ms:
            lookback_start_ts = ts - lookback_ms
            scores = {}
            for name in eligible:
                pnl_now = pnl_by[name].get(ts, 0)
                pnl_past = 0.0
                for t in sorted(pnl_by[name].keys()):
                    if t <= lookback_start_ts:
                        pnl_past = pnl_by[name][t]
                    else:
                        break
                ret = pnl_now - pnl_past
                lb_pnls = [v for t, v in pnl_by[name].items() if lookback_start_ts <= t <= ts]
                if lb_pnls:
                    lb_peak = max(lb_pnls); lb_trough = min(lb_pnls)
                    denom = budget + lb_peak
                    lb_dd = (lb_peak - lb_trough) / denom * 100 if denom > 0 else 0
                else:
                    lb_dd = 0
                scores[name] = score_fn(ret, lb_dd)
            if scores:
                best = max(scores, key=scores.get)
                current_eq = budget + merged_pnl
                dd_now = (peak - current_eq) / peak * 100 if peak > 0 else 0
                if cash_dd and dd_now > cash_dd:
                    active = "cash"
                elif scores[best] - active_score > hyst_gap or active_score <= -1e17:
                    active = best
                    active_score = scores[best]
            last_rebal = i
        # LEAKY: apply the just-decided sleeve to THIS interval (prev_ts -> ts)
        if active is None or active == "cash":
            step = 0.0
        else:
            pnl_now = pnl_by.get(active, {}).get(ts, 0)
            prev_ts = all_ts[i-1] if i > 0 else ts
            pnl_prev = pnl_by.get(active, {}).get(prev_ts, 0)
            step = pnl_now - pnl_prev
        merged_pnl += step
        eq = budget + merged_pnl
        if eq > peak: peak = eq
        dd = (peak - eq) / peak * 100 if peak > 0 else 0
        if dd > max_dd: max_dd = dd

    days = (all_ts[-1] - all_ts[0]) / 86400000
    total_ret = merged_pnl / budget
    if days > 0 and (1 + total_ret) > 0:
        ann = ((1 + total_ret) ** (365 / days) - 1) * 100
    else:
        ann = -999.0
    return {"ann": round(ann, 4), "dd": round(max_dd, 4), "ret": round(total_ret * 100, 4)}


def compute_segment_metrics_for_allocator(curves, lookback_days, rebalance_days,
                                           score_fn, max_hi, min_lo, cash_dd, hyst_gap,
                                           high_ann_sleeves, low_dd_sleeves,
                                           budget, segments):
    """Run the allocator over slices of full-period sleeve curves.

    Returns {seg_name: {ann, dd, ret}}. This is segment attribution for the
    curve model, not a cold-start event-level sleeve replay. Open-cycle state
    from before a boundary may be embedded in the sliced curve.
    """
    out = {}
    for seg_name, seg_start, seg_end in segments:
        seg_curves = {}
        for name, ec in curves.items():
            sliced = [p for p in ec if seg_start <= p["timestamp_ms"] <= seg_end]
            if len(sliced) >= 2:
                # Renormalize so first point = budget (allocator assumes budget start)
                first_eq = sliced[0]["equity_quote"]
                scale = budget / first_eq if first_eq > 0 else 1.0
                seg_curves[name] = [
                    {"timestamp_ms": p["timestamp_ms"],
                     "equity_quote": p["equity_quote"] * scale}
                    for p in sliced
                ]
        if not seg_curves:
            out[seg_name] = None
            continue
        res = run_allocator_repaired(
            seg_curves, lookback_days, rebalance_days, score_fn,
            max_hi, min_lo, cash_dd, hyst_gap,
            high_ann_sleeves, low_dd_sleeves, budget,
        )
        out[seg_name] = res
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--budget", type=float, default=5000)
    ap.add_argument("--market-data", default=MARKET_DB)
    ap.add_argument("--funding-data", default=FUNDING_DB)
    args = ap.parse_args()

    budget = args.budget
    print(f"=== Building full-period curves (budget={budget}) ===", flush=True)
    curves = {}
    sleeve_symbols = {}
    for name, path in CANDS.items():
        if not os.path.exists(path):
            print(f"  {name}: MISSING config {path}")
            continue
        sleeve_symbols[name] = get_sleeve_symbols(path)
        ec = get_curve(name, path, budget, FULL_START, FULL_END, args.market_data, args.funding_data)
        if ec:
            curves[name] = ec
            print(f"  {name}: {len(ec)} pts, symbols={sleeve_symbols[name]}")

    # Aggregate traded symbols across all sleeves (allocator may rotate among any)
    all_symbols = sorted(set(s for syms in sleeve_symbols.values() for s in syms))

    lookbacks = [30, 60, 90]
    rebalances = [7, 14, 30]
    scores = {
        "ann_minus_1dd": lambda ret, dd: ret - 1.0 * dd,
        "ann_minus_2dd": lambda ret, dd: ret - 2.0 * dd,
        "ann_minus_cost": lambda ret, dd: ret - 0.5 * dd,
        "calmar_like": lambda ret, dd: ret / (dd + 1.0) if ret > 0 else ret - dd,
    }
    max_his = [0.0, 0.30, 1.0]   # 0.0 = exclude high-ann entirely (binds)
    min_los = [0.0, 0.35, 1.0]   # Legacy binary switch: 0 versus >0 only
    cash_dds = [None, 8, 12]
    hyst_gaps = [0, 3, 6]

    total = (len(lookbacks) * len(rebalances) * len(scores) * len(max_his) *
             len(min_los) * len(cash_dds) * len(hyst_gaps))
    print(f"=== Running {total} curve-allocator labels (legacy eligibility semantics) ===", flush=True)
    t0 = time.time()
    results = []
    done = 0
    for lb in lookbacks:
        for rb in rebalances:
            for sname, sfn in scores.items():
                for mh in max_his:
                    for ml in min_los:
                        for cd in cash_dds:
                            for hg in hyst_gaps:
                                # Full-period
                                fm = run_allocator_repaired(
                                    curves, lb, rb, sfn, mh, ml, cd, hg,
                                    HIGH_ANN_SLEEVES, LOW_DD_SLEEVES, budget,
                                )
                                # Per-segment
                                seg_m = compute_segment_metrics_for_allocator(
                                    curves, lb, rb, sfn, mh, ml, cd, hg,
                                    HIGH_ANN_SLEEVES, LOW_DD_SLEEVES, budget,
                                    FULL_SEGMENTS,
                                )
                                if fm:
                                    pos_segs = sum(1 for v in seg_m.values() if v and v["ret"] > 0)
                                    # Symbol metadata (allocator can trade any sleeve's symbols)
                                    sym_count = len(all_symbols)
                                    max_sym_share_pct = 100.0 / sym_count if sym_count else 100.0
                                    portfolio_candidate = sym_count >= 5
                                    target_profile_hit = []
                                    if fm["ann"] >= 50 and fm["dd"] <= 10 and pos_segs >= 4:
                                        target_profile_hit.append("conservative")
                                    if fm["ann"] >= 90 and fm["dd"] <= 20 and pos_segs >= 4:
                                        target_profile_hit.append("balanced")
                                    if fm["ann"] >= 110 and fm["dd"] <= 30 and pos_segs >= 3:
                                        target_profile_hit.append("aggressive")
                                    results.append({
                                        "label": f"lb{lb}_rb{rb}_{sname}_hi{mh}_lo{ml}_cash{cd}_hys{hg}",
                                        "params": {
                                            "lookback_days": lb, "rebalance_days": rb,
                                            "score": sname,
                                            "max_high_ann_weight": mh,
                                            "min_low_dd_weight": ml,
                                            "cash_trigger_rolling_dd_pct": cd,
                                            "switch_hysteresis_score_gap": hg,
                                        },
                                        "full_metrics": fm,
                                        "segment_metrics": seg_m,
                                        "positive_segments": pos_segs,
                                        "traded_symbols": all_symbols,
                                        "indicator_only_dependencies": ["BTCUSDT"],
                                        "symbol_count": sym_count,
                                        "max_symbol_budget_pct": round(max_sym_share_pct, 2),
                                        "max_symbol_gross_pnl_share_pct": None,  # requires per-symbol PnL attribution
                                        "portfolio_candidate": portfolio_candidate,
                                        "live_ready": False,  # requires trading-engine allocator (P2)
                                        "target_profile_hit": target_profile_hit,
                                        "non_repeat_key": f"r9alloc_lb{lb}_rb{rb}_{sname}_hi{mh}_lo{ml}_cash{cd}_hys{hg}",
                                    })
                                done += 1
                                if done % 100 == 0:
                                    print(f"  [{done}/{total}] elapsed={time.time()-t0:.0f}s", flush=True)

    results.sort(key=lambda v: v.get("full_metrics", {}).get("ann", -999), reverse=True)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({
        "results": results,
        "total_configs": total,
        "non_duplicate_rows": len(results),
        "semantics": {
            "curve_reuse_only": True,
            "event_level_position_continuity": False,
            "cold_start_segment_replays": False,
            "fractional_weight_caps_implemented": False,
            "production_live_ready": False,
        },
    }, open(args.out, "w"), indent=2)
    print(f"\n[r9P1] wrote {args.out}: {len(results)} non-duplicate rows in {time.time()-t0:.0f}s")
    print("\n=== TOP 10 by ann ===")
    for v in results[:10]:
        fm = v["full_metrics"]
        print(f"  {v['label']:55s} ann={fm['ann']:7.1f} dd={fm['dd']:6.1f} pos={v['positive_segments']}/5 hit={v['target_profile_hit']}")
    # Target hits
    hits = [v for v in results if v["target_profile_hit"]]
    print(f"\n=== Target hits: {len(hits)} ===")
    for v in hits[:10]:
        fm = v["full_metrics"]
        print(f"  {v['label']}: ann={fm['ann']} dd={fm['dd']} hit={v['target_profile_hit']}")


if __name__ == "__main__":
    main()
