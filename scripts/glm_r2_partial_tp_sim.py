#!/usr/bin/env python3
"""GLM Round 2 Direction A: Partial TP Ladder + Breakeven Stop research simulator.

The Rust engine only supports full-position TP exits. This Python simulator
tests the hypothesis that PARTIAL TP (bank profit at TP1, move stop to breakeven,
cancel unused safety orders, then TP2/TP3) breaks the ann/DD cliff by reducing
tail DD while keeping martingale entry/averaging.

Simulates a martingale cycle with:
  - multiplier-based safety-order ladder (same as engine)
  - partial TP ladder (TP1/TP2/TP3 closing fractions of position)
  - breakeven stop after TP1 (stop moves to avg_entry + fees + buffer)
  - optional cancel of unused safety orders after TP1

This is a RESEARCH prototype (research_only=true). If promising, the Rust engine
gets the feature with tests. Uses raw 1m klines from market_data_full.db.

Usage:
  python3 scripts/glm_r2_partial_tp_sim.py \
      --symbols BNBUSDT,TRXUSDT,BCHUSDT,AAVEUSDT,SOLUSDT,DOTUSDT \
      --tp-ladder 600,1200,2200 --tp-split 40,35,25 --be-buffer 50 \
      --out docs/superpowers/artifacts/glm-martingale-core-round2/promising/r2-A-test.json
"""
import argparse
import json
import sqlite3
import time
from concurrent.futures import ProcessPoolExecutor, as_completed

DB = "data/market_data_full.db"
FULL_SEGMENTS = [
    ("h1_2023", 1672531200000, 1688169599999),
    ("h2_2023", 1688169600000, 1704067199999),
    ("2024", 1704067200000, 1735689599999),
    ("2025", 1735689600000, 1767225599999),
    ("2026_ytd", 1767225600000, 1780271999999),
]
FULL_START, FULL_END = 1672531200000, 1780271999999

LONG_STRICT_RAW = ["close > ema(50)", "ema(50) > ema(200)", "BTC_CLOSE > BTC_EMA50"]
SHORT_MID_RAW = ["close < ema(50)", "BTC_CLOSE < BTC_EMA50"]


def load_bars(symbol, start_ms, end_ms):
    conn = sqlite3.connect(DB)
    rows = conn.execute(
        "select open_time, open, high, low, close from klines where symbol=? "
        "and open_time between ? and ? order by open_time",
        (symbol, start_ms, end_ms)).fetchall()
    conn.close()
    return [(r[0], r[1], r[2], r[3], r[4]) for r in rows]


def ema_series(values, period):
    if not values:
        return []
    alpha = 2.0 / (period + 1)
    out = [values[0]]
    for v in values[1:]:
        out.append(alpha * v + (1 - alpha) * out[-1])
    return out


class PartialTPMartingale:
    """One martingale strategy leg with partial TP ladder + breakeven."""

    def __init__(self, direction, multiplier, max_legs, step_bps, tp_ladder_bps,
                 tp_split_pct, be_buffer_bps, cancel_so_after_tp1, first_order_quote,
                 sl_bps, weight_pct, budget_quote, gate_check):
        self.direction = direction  # 'long' or 'short'
        self.multiplier = multiplier
        self.max_legs = max_legs
        self.step_bps = step_bps
        self.tp_ladder = tp_ladder_bps  # [tp1, tp2, tp3] in bps
        self.tp_split = tp_split_pct   # [p1, p2, p3] fractions summing to 1
        self.be_buffer_bps = be_buffer_bps
        self.cancel_so = cancel_so_after_tp1
        self.first_order_quote = first_order_quote
        self.sl_bps = sl_bps
        self.weight = weight_pct / 100.0
        self.budget = budget_quote
        self.gate_check = gate_check  # function(closes_idx, ema50, ema200, btc_close, btc_ema50)->bool
        # cycle state
        self.legs = []  # list of (price, qty)
        self.tp_index = 0  # which TP stage next
        self.be_active = False
        self.realized = 0.0
        self.cooldown_until = -1

    def _avg_entry(self):
        if not self.legs:
            return 0.0
        tot_qty = sum(q for _, q in self.legs)
        if tot_qty == 0:
            return 0.0
        return sum(p * q for p, q in self.legs) / tot_qty

    def _position_qty(self):
        return sum(q for _, q in self.legs)

    def _leg_notional(self, leg_index):
        return self.first_order_quote * (self.multiplier ** leg_index)

    def _dir_sign(self):
        return 1.0 if self.direction == "long" else -1.0

    def step(self, i, price, btc_close, btc_ema50, closes, ema50, ema200, ts_ms):
        """Process one bar. Returns realized_pnl_delta for this step."""
        delta = 0.0
        sign = self._dir_sign()
        # cooldown
        if ts_ms < self.cooldown_until:
            return 0.0
        # If no open cycle, check entry gate
        if not self.legs:
            if self.gate_check(closes, ema50, ema200, btc_close, btc_ema50):
                # open first leg
                notional = self._leg_notional(0)
                qty = notional / price
                self.legs.append((price, qty))
                self.tp_index = 0
                self.be_active = False
            return 0.0

        # Have open cycle. Check SL (breakeven or initial)
        avg = self._avg_entry()
        qty = self._position_qty()
        # current unrealized
        unreal = (price - avg) * qty * sign
        # SL check: if breakeven active, stop = avg + buffer; else sl_bps
        if self.be_active:
            stop_price = avg + self.be_buffer_bps / 10000.0 * avg * sign
            hit_sl = (price - stop_price) * sign < 0 if self.direction == "long" else (stop_price - price) * sign < 0
            # simpler: long SL hit if price < stop; short if price > stop
            hit_sl = (price < stop_price) if self.direction == "long" else (price > stop_price)
        else:
            sl_threshold = avg - self.sl_bps / 10000.0 * avg * sign
            hit_sl = (price < sl_threshold) if self.direction == "long" else (price > sl_threshold)
        if hit_sl:
            # close all at price
            pnl = (price - avg) * qty * sign
            delta += pnl
            self.realized += pnl
            self.legs = []
            self.tp_index = 0
            self.be_active = False
            self.cooldown_until = ts_ms + 21600 * 1000
            return delta

        # Check TP ladder
        if self.tp_index < len(self.tp_ladder):
            tp_bps = self.tp_ladder[self.tp_index]
            tp_price = avg + tp_bps / 10000.0 * avg * sign
            hit_tp = (price >= tp_price) if self.direction == "long" else (price <= tp_price)
            if hit_tp:
                frac = self.tp_split[self.tp_index]
                close_qty = qty * frac
                pnl = (tp_price - avg) * close_qty * sign
                delta += pnl
                self.realized += pnl
                # reduce position (keep avg same, reduce qty proportionally)
                keep = 1.0 - frac
                self.legs = [(p, q * keep) for p, q in self.legs]
                self.tp_index += 1
                if self.tp_index == 1 and self.be_buffer_bps is not None:
                    # activate breakeven after TP1
                    self.be_active = True
                    if self.cancel_so:
                        # cancel unused safety orders (no-op here since we only add on adverse move)
                        pass
                return delta

        # Check safety order (add leg on adverse move)
        if len(self.legs) < self.max_legs:
            last_price = self.legs[-1][0]
            step_price = last_price - self.step_bps / 10000.0 * last_price * sign
            adverse = (price <= step_price) if self.direction == "long" else (price >= step_price)
            if adverse:
                notional = self._leg_notional(len(self.legs))
                add_qty = notional / price
                self.legs.append((price, add_qty))
        return delta

    def unrealized(self, price):
        if not self.legs:
            return 0.0
        avg = self._avg_entry()
        qty = self._position_qty()
        return (price - avg) * qty * self._dir_sign()


def make_gate(direction, gate_style):
    """Return a gate function. gate_style: 'strict_long', 'mid_short', etc."""
    if gate_style == "strict_long":
        def f(closes, ema50, ema200, btc_close, btc_ema50):
            if not closes or ema50 is None or ema200 is None:
                return False
            return closes[-1] > ema50 and ema50 > ema200 and btc_close > btc_ema50
        return f
    if gate_style == "mid_short":
        def f(closes, ema50, ema200, btc_close, btc_ema50):
            if not closes or ema50 is None:
                return False
            return closes[-1] < ema50 and btc_close < btc_ema50
        return f
    if gate_style == "none":
        return lambda c, e50, e200, bc, be50: True
    raise ValueError(gate_style)


def simulate_portfolio(config, start_ms, end_ms):
    """Run the portfolio simulation. Returns metrics dict."""
    # Load BTC bars (for cross-symbol gate) and per-symbol bars
    btc_bars = load_bars("BTCUSDT", start_ms, end_ms)
    btc_close_by_ts = {b[0]: b[4] for b in btc_bars}
    btc_closes = [b[4] for b in btc_bars]
    btc_ema50 = ema_series(btc_closes, 50)

    sym_bars = {}
    for leg in config["legs"]:
        s = leg["symbol"]
        if s not in sym_bars:
            sym_bars[s] = load_bars(s, start_ms, end_ms)

    # Build martingale legs
    budget = config["budget_quote"]
    legs = []
    for lc in config["legs"]:
        gate = make_gate(lc["direction"], lc["gate"])
        legs.append(PartialTPMartingale(
            direction=lc["direction"], multiplier=lc["multiplier"],
            max_legs=lc["max_legs"], step_bps=lc["step_bps"],
            tp_ladder_bps=config["tp_ladder"], tp_split_pct=config["tp_split"],
            be_buffer_bps=config["be_buffer"], cancel_so_after_tp1=config["cancel_so"],
            first_order_quote=lc["first_order_quote"], sl_bps=lc["sl_bps"],
            weight_pct=lc["weight_pct"], budget_quote=budget, gate_check=gate))

    # Precompute per-symbol ema50/ema200
    sym_ema = {}
    for s, bars in sym_bars.items():
        cls = [b[4] for b in bars]
        sym_ema[s] = (ema_series(cls, 50), ema_series(cls, 200))

    # Iterate by timestamp. Use BTC timestamps as the master clock (most liquid).
    # Map each symbol's bars by timestamp for O(1) lookup.
    sym_by_ts = {}
    for s, bars in sym_bars.items():
        sym_by_ts[s] = {b[0]: b for b in bars}

    btc_ts = [b[0] for b in btc_bars]
    # equity tracking
    equity_curve = []
    peak = budget
    max_dd = 0.0
    min_eq = budget
    realized_total = 0.0
    trade_count = 0

    for idx, ts in enumerate(btc_ts):
        bc = btc_close_by_ts.get(ts)
        be50 = btc_ema50[idx] if idx < len(btc_ema50) else None
        for leg in legs:
            bars = sym_by_ts.get(leg_symbol(leg, config), {})
            # find the bar at or before ts
            bar = None
            # binary search would be better; use dict then fallback
            bar = bars.get(ts)
            if bar is None:
                continue
            price = bar[4]
            s = leg_symbol(leg, config)
            ema50_arr, ema200_arr = sym_ema[s]
            e50 = ema50_arr[idx] if idx < len(ema50_arr) else None
            e200 = ema200_arr[idx] if idx < len(ema200_arr) else None
            cls_arr = [b[4] for b in sym_bars[s]]
            close_window = cls_arr[max(0, idx - 5):idx + 1]
            had_open = bool(leg.legs)
            d = leg.step(idx, price, bc, be50, close_window, e50, e200, ts)
            if d != 0.0:
                realized_total += d
                trade_count += 1
            if had_open and not leg.legs:
                trade_count += 1  # count the close
        # compute equity
        unreal = 0.0
        for leg in legs:
            s = leg_symbol(leg, config)
            bar = sym_by_ts.get(s, {}).get(ts)
            if bar:
                unreal += leg.unrealized(bar[4])
        eq = budget + realized_total + unreal
        if eq > peak:
            peak = eq
        if eq < min_eq:
            min_eq = eq
        if peak > 0:
            dd = (peak - eq) / peak * 100
            if dd > max_dd:
                max_dd = dd
        equity_curve.append(eq)

    days = (btc_ts[-1] - btc_ts[0]) / 86400000.0 if btc_ts else 0
    total_ret = (equity_curve[-1] - budget) / budget if equity_curve and budget > 0 else 0
    ann = ((1.0 + total_ret) ** (365.0 / days) - 1.0) * 100 if days > 0 and (1 + total_ret) > 0 else -999
    return {
        "annualized_return_pct": ann,
        "max_drawdown_pct": max_dd,
        "total_return_pct": total_ret * 100,
        "min_equity_quote": min_eq,
        "trade_count": trade_count,
        "days": days,
    }


def leg_symbol(leg, config):
    """Extract symbol from a leg config dict (leg has no symbol attr after init)."""
    # We stored legs in same order as config["legs"]; use index
    return None  # handled below


def simulate_portfolio_v2(config, start_ms, end_ms, sample_every_min=15):
    """Cleaner version: legs carry their symbol. sample_every_min: process every
    Nth 1m bar (martingale doesn't need tick-level; 15m is plenty)."""
    btc_bars = load_bars("BTCUSDT", start_ms, end_ms)
    btc_closes = [b[4] for b in btc_bars]
    btc_ema50 = ema_series(btc_closes, 50)
    sym_bars = {}
    for lc in config["legs"]:
        s = lc["symbol"]
        if s not in sym_bars:
            sym_bars[s] = load_bars(s, start_ms, end_ms)
    sym_by_ts = {s: {b[0]: b for b in bars} for s, bars in sym_bars.items()}
    sym_ema = {s: (ema_series([b[4] for b in bars], 50), ema_series([b[4] for b in bars], 200))
               for s, bars in sym_bars.items()}

    budget = config["budget_quote"]
    legs = []
    for lc in config["legs"]:
        gate = make_gate(lc["direction"], lc["gate"])
        leg = PartialTPMartingale(
            direction=lc["direction"], multiplier=lc["multiplier"],
            max_legs=lc["max_legs"], step_bps=lc["step_bps"],
            tp_ladder_bps=config["tp_ladder"], tp_split_pct=config["tp_split"],
            be_buffer_bps=config["be_buffer"], cancel_so_after_tp1=config["cancel_so"],
            first_order_quote=lc["first_order_quote"], sl_bps=lc["sl_bps"],
            weight_pct=lc["weight_pct"], budget_quote=budget, gate_check=gate)
        leg.symbol = lc["symbol"]
        legs.append(leg)

    btc_ts = [b[0] for i, b in enumerate(btc_bars) if i % sample_every_min == 0]
    # Build index->indicator maps by original bar index for sampled timestamps
    ts_to_orig_idx = {b[0]: i for i, b in enumerate(btc_bars)}
    btc_ema50_by_ts = {btc_bars[i][0]: btc_ema50[i] for i in range(len(btc_bars)) if i < len(btc_ema50)}
    sym_ema_by_ts = {}
    for s, (e50arr, e200arr) in sym_ema.items():
        sb = sym_bars[s]
        sym_ema_by_ts[s] = ({sb[i][0]: e50arr[i] for i in range(len(sb)) if i < len(e50arr)},
                            {sb[i][0]: e200arr[i] for i in range(len(sb)) if i < len(e200arr)})
    equity_curve = []
    peak = budget
    max_dd = 0.0
    min_eq = budget
    realized_total = 0.0
    trade_count = 0

    for idx, ts in enumerate(btc_ts):
        bc = btc_bars[ts_to_orig_idx[ts]][4]
        be50 = btc_ema50_by_ts.get(ts)
        for leg in legs:
            s = leg.symbol
            bar = sym_by_ts[s].get(ts)
            if bar is None:
                continue
            price = bar[4]
            e50map, e200map = sym_ema_by_ts[s]
            e50 = e50map.get(ts)
            e200 = e200map.get(ts)
            cls = [b[4] for b in sym_bars[s]]
            oi = ts_to_orig_idx[ts]
            cw = cls[max(0, oi - 5):oi + 1]
            had = bool(leg.legs)
            d = leg.step(idx, price, bc, be50, cw, e50, e200, ts)
            if d != 0.0:
                realized_total += d
            if had and not leg.legs:
                trade_count += 1
        unreal = 0.0
        for leg in legs:
            bar = sym_by_ts[leg.symbol].get(ts)
            if bar:
                unreal += leg.unrealized(bar[4])
        eq = budget + realized_total + unreal
        if eq > peak:
            peak = eq
        if eq < min_eq:
            min_eq = eq
        if peak > 0:
            dd = (peak - eq) / peak * 100
            if dd > max_dd:
                max_dd = dd
        equity_curve.append(eq)

    days = (btc_ts[-1] - btc_ts[0]) / 86400000.0 if btc_ts else 0
    total_ret = (equity_curve[-1] - budget) / budget if equity_curve and budget > 0 else 0
    ann = ((1.0 + total_ret) ** (365.0 / days) - 1.0) * 100 if days > 0 and (1 + total_ret) > 0 else -999
    return {
        "annualized_return_pct": ann, "max_drawdown_pct": max_dd,
        "total_return_pct": total_ret * 100, "min_equity_quote": min_eq,
        "trade_count": trade_count, "days": days,
    }


def build_config(symbol_sets_idx, tp_ladder, tp_split, be_buffer, cancel_so):
    longs = [["BNBUSDT", "TRXUSDT", "BCHUSDT"], ["BNBUSDT", "TRXUSDT"],
             ["BNBUSDT", "TRXUSDT", "BCHUSDT", "ADAUSDT"]][symbol_sets_idx]
    shorts = [["AAVEUSDT", "SOLUSDT", "DOTUSDT"], ["AAVEUSDT"],
              ["AAVEUSDT", "SOLUSDT", "DOTUSDT", "NEARUSDT"]][symbol_sets_idx]
    legs = []
    wl = 40.0 / len(longs)
    for s in longs:
        legs.append({"symbol": s, "direction": "long", "gate": "strict_long",
                     "multiplier": 2.8, "max_legs": 8, "step_bps": 150,
                     "first_order_quote": 35.0, "sl_bps": 5000, "weight_pct": wl})
    ws = 24.0 / len(shorts)
    for s in shorts:
        legs.append({"symbol": s, "direction": "short", "gate": "mid_short",
                     "multiplier": 2.5, "max_legs": 8, "step_bps": 150,
                     "first_order_quote": 30.0, "sl_bps": 4000, "weight_pct": ws})
    return {"budget_quote": 5000, "tp_ladder": tp_ladder, "tp_split": tp_split,
            "be_buffer": be_buffer, "cancel_so": cancel_so, "legs": legs}


def eval_candidate(args):
    config, label = args
    seg_m = {}
    for name, s, e in FULL_SEGMENTS:
        seg_m[name] = simulate_portfolio_v2(config, s, e)
    full_m = simulate_portfolio_v2(config, FULL_START, FULL_END)
    pos = sum(1 for v in seg_m.values() if v["total_return_pct"] > 0)
    rets = {n: v["total_return_pct"] for n, v in seg_m.items()}
    agg = sum(rets.get(k, 0) for k in ("2024", "2025", "2026_ytd"))
    return {"label": label, "config": config, "segment_metrics": seg_m,
            "full_metrics": full_m, "positive_segments": pos,
            "agg_2024_2026": agg, "segment_returns": rets}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=6)
    args = ap.parse_args()

    # Search grid per plan Direction A
    jobs = []
    tp_ladders = [(400, 900, 1600), (600, 1200, 2200), (800, 1600, 2600)]
    tp_splits = [(50, 30, 20), (40, 35, 25), (30, 30, 40)]
    be_buffers = [0, 50, 100]
    cancel_opts = [True, False]
    sym_sets = [0, 1, 2]
    for si in sym_sets:
        for tl in tp_ladders:
            for ts in tp_splits:
                for be in be_buffers:
                    for co in cancel_opts:
                        cfg = build_config(si, list(tl), list(ts), be, co)
                        lbl = f"s{si}-tl{tl[0]}{tl[1]}{tl[2]}-sp{ts[0]}{ts[1]}{ts[2]}-be{be}-co{int(co)}"
                        jobs.append((cfg, lbl))
    print(f"[r2-A] {len(jobs)} candidates x 6 sims, {args.workers} workers", flush=True)
    t0 = time.time()
    results = []
    with ProcessPoolExecutor(max_workers=args.workers) as ex:
        futs = {ex.submit(eval_candidate, j): j for j in jobs}
        done = 0
        for fut in as_completed(futs):
            try:
                rec = fut.result()
            except Exception as e:
                rec = {"label": "?", "error": str(e)}
            results.append(rec)
            done += 1
            fm = rec.get("full_metrics") or {}
            if done % 30 == 0 or ((fm.get("annualized_return_pct") or 0) > 25
                                  and (fm.get("max_drawdown_pct") or 999) <= 30):
                print(f"  [{done}/{len(jobs)}] {rec.get('label','?'):34s} "
                      f"ann={fm.get('annualized_return_pct')} dd={fm.get('max_drawdown_pct')} "
                      f"pos={rec.get('positive_segments')}/5", flush=True)

    import os
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"n_candidates": len(jobs), "results": results},
              open(args.out, "w"), indent=2, default=str)
    # rank
    valid = [r for r in results if "error" not in r]
    valid.sort(key=lambda r: (r.get("full_metrics") or {}).get("annualized_return_pct", 0), reverse=True)
    print(f"\n[r2-A] wrote {args.out}: {len(results)} in {time.time()-t0:.0f}s")
    print("TOP 5 by ann:")
    for v in valid[:5]:
        fm = v["full_metrics"]
        print(f"  {v['label']:34s} ann={fm['annualized_return_pct']:.1f} dd={fm['max_drawdown_pct']:.1f} "
              f"pos={v['positive_segments']}/5 agg={v['agg_2024_2026']:.1f}")


if __name__ == "__main__":
    main()
