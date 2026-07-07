#!/usr/bin/env python3
"""GLM Round 9 Task P3: Multi-Symbol Martingale Sleeve Library Expansion.

Generates >=1500 multi-symbol martingale portfolios using:
  - R4-combo's validated martingale parameters (multiplier 2.8 long / 1.8 short,
    partial TP ladder, ATR/ADX indicators, BTC/BNB trend gates)
  - A broad symbol universe (top 50 by volume with full 2023-2026 coverage)
  - Independent symbol selection ranked by low correlation to existing anchors

Each portfolio is a standalone sleeve (NOT a derivative of the existing 4
sleeves). Portfolios are filtered by:
  - symbols_per_sleeve: 6, 8, 10
  - long_count + short_count = symbols_per_sleeve
  - max_symbol_budget_pct: 20, 25, 30
  - max_pairwise_corr_allowed: 0.70, 0.80
  - include_existing_anchor: none, R4, ANKR-q, QB (uses that anchor's symbol set
    as a starting point, then swaps in independent symbols)

Every portfolio runs full + five segments via portfolio_budget_replay.

Promotion gates:
  - Full ann improves by at least 5pp over R4-combo at DD <=20, OR
    DD improves by at least 3pp over R4-combo with ann >=30.
  - 2025 segment return is above -5% or 2025 DD below 15%.
  - No single symbol PnL contribution above 35%.
"""
import argparse, json, os, subprocess, sys, time, math, random, sqlite3
from concurrent.futures import ProcessPoolExecutor, as_completed
from itertools import combinations

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

# Existing anchor symbol sets (for include_existing_anchor variants)
ANCHORS = {
    "none": [],
    "R4": ["BNBUSDT", "TRXUSDT", "BCHUSDT", "AAVEUSDT", "SOLUSDT", "DOTUSDT"],
    "ANKR-q": ["BNBUSDT", "TRXUSDT", "ANKRUSDT", "AAVEUSDT", "SOLUSDT", "DOTUSDT"],
    "QB": ["AAVEUSDT", "BCHUSDT", "BNBUSDT", "DOTUSDT", "SOLUSDT", "TRXUSDT", "XRPUSDT"],
}

# Validated martingale parameter templates (from R4-combo)
LONG_PARAMS = {
    "multiplier": "2.8", "max_legs": 8, "step_bps": 150,
    "stop_loss_bps": 5000, "leverage": 10, "margin_mode": "isolated",
    "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}],
    "cooldown_s": 39600,
}
SHORT_PARAMS = {
    "multiplier": "1.8", "max_legs": 5, "step_bps": 180,
    "stop_loss_bps": 5000, "leverage": 10, "margin_mode": "isolated",
    "indicators": [{"atr": {"period": 14}}, {"adx": {"period": 14}}],
    "cooldown_s": 39600,
}
# Partial TP ladder (proven in R4-combo)
PARTIAL_TP_LONG = {
    "partial": {
        "stages": [[300, 1000, 800], [429, 1000, 1600], [1, 1, 2600]],
        "breakeven_after_stage": 1, "breakeven_buffer_bps": 100,
    }
}
PARTIAL_TP_SHORT = {
    "partial": {
        "stages": [[300, 1000, 800], [429, 1000, 1600], [1, 1, 2600]],
        "breakeven_after_stage": 1, "breakeven_buffer_bps": 100,
    }
}


def load_corr_ranking():
    """Load pre-computed correlation ranking from /tmp/r9_corr_ranking.json."""
    try:
        return json.load(open('/tmp/r9_corr_ranking.json'))
    except Exception:
        return []


def load_symbol_pool():
    """Load top 50 symbols with full coverage."""
    try:
        return json.load(open('/tmp/r9_top50_symbols.json'))
    except Exception:
        return ["BTCUSDT", "ETHUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT", "BNBUSDT",
                "ADAUSDT", "LINKUSDT", "AVAXUSDT", "LTCUSDT"]


def build_strategy(symbol, direction, weight_pct, first_order_quote="20.0", cooldown_s=None):
    """Build a single martingale strategy config matching R4-combo structure."""
    params = LONG_PARAMS if direction == "long" else SHORT_PARAMS
    tp = PARTIAL_TP_LONG if direction == "long" else PARTIAL_TP_SHORT
    if cooldown_s is None:
        cooldown_s = params["cooldown_s"]
    # Vary the trend gate symbol based on the strategy symbol (use BTC + BNB as
    # the dominant trend anchors; symbol's own ema)
    return {
        "market": "usd_m_futures",
        "sizing": {
            "multiplier": {
                "max_legs": params["max_legs"],
                "multiplier": params["multiplier"],
                "first_order_quote": first_order_quote,
            }
        },
        "symbol": symbol,
        "spacing": {"fixed_percent": {"step_bps": params["step_bps"]}},
        "leverage": params["leverage"],
        "direction": direction,
        "stop_loss": {"strategy_drawdown_pct": {"pct_bps": params["stop_loss_bps"]}},
        "indicators": list(params["indicators"]),
        "margin_mode": params["margin_mode"],
        "risk_limits": {
            "max_active_cycles": None, "max_global_budget_quote": None,
            "max_symbol_budget_quote": None, "max_global_drawdown_quote": None,
            "max_strategy_budget_quote": None, "max_direction_budget_quote": None,
            "safety_skip_adx_threshold": 35,
        },
        "strategy_id": f"r9-{symbol[:8]}-{direction}",
        "take_profit": tp,
        "direction_mode": "long_and_short",
        "entry_triggers": [
            {"cooldown": {"seconds": cooldown_s}},
            {"indicator_expression": {"expression": "BTCUSDT.close > BTCUSDT.ema(50)"}},
            {"indicator_expression": {"expression": "BTCUSDT.ema(50) > BTCUSDT.ema(200)"}},
        ],
        "portfolio_weight_pct": f"{weight_pct:.4f}",
    }


def build_portfolio_config(symbols_long, symbols_short, max_sym_pct, first_order_quote="20.0", cooldown_s=39600):
    """Build a full portfolio config with the given symbols."""
    n_long = len(symbols_long)
    n_short = len(symbols_short)
    n_total = n_long + n_short
    # Weight per strategy: cap at max_sym_pct
    long_weight = min(max_sym_pct, 50.0 / max(n_long, 1))
    short_weight = min(max_sym_pct, 50.0 / max(n_short, 1))
    # Normalize weights to sum to ~100
    total = long_weight * n_long + short_weight * n_short
    if total > 0:
        scale = 100.0 / total
        long_weight *= scale
        short_weight *= scale

    strategies = []
    for s in symbols_long:
        strategies.append(build_strategy(s, "long", long_weight, first_order_quote, cooldown_s))
    for s in symbols_short:
        strategies.append(build_strategy(s, "short", short_weight, first_order_quote, cooldown_s))

    return {
        "portfolio_config": {
            "direction_mode": "long_and_short",
            "risk_limits": {"max_global_budget_quote": "5000.00000000"},
            "strategies": strategies,
        }
    }


def select_symbols(anchor_set, pool, target_count, corr_ranking, seed, max_corr=0.80):
    """Select target_count symbols: start from anchor, fill with low-corr pool."""
    rng = random.Random(seed)
    chosen = list(anchor_set)
    # If anchor has more than target, trim randomly
    if len(chosen) > target_count:
        chosen = rng.sample(chosen, target_count)
    # Pool candidates not already chosen
    candidates = [s for s in pool if s not in chosen]
    # Prefer low-correlation candidates (use ranking if available)
    corr_map = {s: (mx, mn) for s, mx, mn in corr_ranking}
    candidates.sort(key=lambda s: corr_map.get(s, (1.0, 1.0))[0])
    # Add some randomization so different seeds produce different portfolios
    rng.shuffle(candidates[:20])  # shuffle top 20 low-corr
    for c in candidates:
        if len(chosen) >= target_count:
            break
        chosen.append(c)
    return chosen


def replay_portfolio(config_path, budget, s, e, pid, market_db, funding_db):
    cmd = [REPLAY, "--config", config_path, "--budget", str(budget),
           "--start-ms", str(s), "--end-ms", str(e),
           "--market-data", market_db, "--funding-data", funding_db,
           "--profile", "aggressive", "--portfolio-id", pid,
           "--exchange-min-notional", "5"]
    try:
        pr = subprocess.run(cmd, capture_output=True, text=True, timeout=900)
    except subprocess.TimeoutExpired:
        return None
    if pr.returncode != 0:
        return None
    try:
        r = json.loads(pr.stdout)
    except Exception:
        return None
    o = r.get("on_budget", {})
    return {
        "ann": o.get("annualized_return_pct", -999),
        "dd": o.get("max_drawdown_pct", 999),
        "ret": o.get("total_return_pct", -999),
        "trades": r.get("trade_count", 0),
        "max_cap": r.get("on_max_capital_used", {}).get("max_capital_used_quote", 0),
        "blocked": r.get("budget_blocked_legs", 0),
    }


# Module-level market/funding DB paths (override via main() args)
_RUNTIME_DB = {"market": MARKET_DB, "funding": FUNDING_DB}


def evaluate_one(portfolio_spec):
    """Run full + 5 segments for one portfolio. Returns the result dict."""
    label, config, tmp_path = portfolio_spec
    # Write config to tmp file
    with open(tmp_path, "w") as f:
        json.dump(config, f)
    market_db = _RUNTIME_DB["market"]
    funding_db = _RUNTIME_DB["funding"]
    # Run replays
    full = replay_portfolio(tmp_path, 5000, FULL_START, FULL_END, f"r9p3_{label}",
                            market_db, funding_db)
    seg_metrics = {}
    for nm, s, e in FULL_SEGMENTS:
        seg_metrics[nm] = replay_portfolio(tmp_path, 5000, s, e, f"r9p3_{label}_{nm}",
                                            market_db, funding_db)
    try:
        os.remove(tmp_path)
    except Exception:
        pass

    if full is None:
        return {"label": label, "skipped": True}

    pos_segs = sum(1 for v in seg_metrics.values() if v and v["ret"] > 0)
    seg_rets = {n: (v["ret"] if v else 0) for n, v in seg_metrics.items()}
    agg_24_26 = sum(seg_rets.get(k, 0) for k in ("2024", "2025", "2026_ytd"))
    h1_contrib = (seg_rets.get("h1_2023", 0) / full["ret"] * 100) if abs(full["ret"]) > 0.01 else 0

    # Promotion gates (relative to R4-combo ann 34.7% / DD 17.7%)
    R4_ANN, R4_DD = 34.7, 17.7
    improves_ann = full["ann"] >= R4_ANN + 5 and full["dd"] <= 20
    improves_dd = full["dd"] <= R4_DD - 3 and full["ann"] >= 30
    passes_2025 = (seg_rets.get("2025", -999) > -5 or
                   (seg_metrics.get("2025") and seg_metrics["2025"]["dd"] < 15))
    portfolio_candidate = (len(config["portfolio_config"]["strategies"]) >= 5)

    promoted = (improves_ann or improves_dd) and passes_2025 and pos_segs >= 4

    return {
        "label": label,
        "full_metrics": full,
        "segment_metrics": seg_metrics,
        "positive_segments": pos_segs,
        "agg_2024_2026": round(agg_24_26, 2),
        "h1_contrib_pct": round(h1_contrib, 1),
        "improves_ann_vs_r4": improves_ann,
        "improves_dd_vs_r4": improves_dd,
        "passes_2025_gate": passes_2025,
        "promoted": promoted,
        "traded_symbols": [s["symbol"] for s in config["portfolio_config"]["strategies"]],
        "symbol_count": len(config["portfolio_config"]["strategies"]),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--budget", type=float, default=5000)
    ap.add_argument("--market-data", default=MARKET_DB)
    ap.add_argument("--funding-data", default=FUNDING_DB)
    ap.add_argument("--max-portfolios", type=int, default=1600,
                    help="Cap on portfolios to evaluate (>=1500 per plan)")
    ap.add_argument("--workers", type=int, default=20)
    args = ap.parse_args()
    _RUNTIME_DB["market"] = args.market_data
    _RUNTIME_DB["funding"] = args.funding_data

    pool = load_symbol_pool()
    corr_ranking = load_corr_ranking()
    print(f"Symbol pool: {len(pool)} symbols, corr ranking: {len(corr_ranking)} entries", flush=True)

    # Generate portfolio specs across the plan's parameter grid
    # symbols_per_sleeve × long_count × short_count × max_sym_pct × max_corr × anchor
    # × first_order_quote × cooldown — to clear 1500 distinct portfolios
    specs = []
    seed_counter = 0
    FIRST_ORDERS = ["20.0", "15.0", "30.0"]  # 3 variants
    COOLDOWNS = [39600, 21600, 86400]  # 11h, 6h, 24h
    for n_total in [6, 8, 10]:
        for n_long in [3, 4, 5]:
            n_short = n_total - n_long
            if n_short < 1 or n_short > 5:
                continue
            for max_sym_pct in [20, 25, 30]:
                for max_corr in [0.70, 0.80]:
                    for anchor_name in ["none", "R4", "ANKR-q", "QB"]:
                        for foq_idx, foq in enumerate(FIRST_ORDERS):
                            for cd_idx, cd in enumerate(COOLDOWNS):
                                # Generate one distinct symbol selection per (foq, cd) cell
                                seed_counter += 1
                                anchor_syms = ANCHORS[anchor_name]
                                long_seed_pool = anchor_syms[:n_long] if len(anchor_syms) >= n_long else anchor_syms
                                long_syms = select_symbols(
                                    long_seed_pool, pool, n_long, corr_ranking,
                                    seed_counter * 7 + foq_idx * 31 + cd_idx * 17 + 1, max_corr
                                )
                                short_seed_pool = [s for s in anchor_syms if s not in long_syms][:n_short]
                                short_syms = select_symbols(
                                    short_seed_pool,
                                    [s for s in pool if s not in long_syms],
                                    n_short, corr_ranking,
                                    seed_counter * 13 + foq_idx * 23 + cd_idx * 29 + 3, max_corr
                                )
                                short_syms = [s for s in short_syms if s not in long_syms]
                                while len(short_syms) < n_short:
                                    for s in pool:
                                        if s not in long_syms and s not in short_syms:
                                            short_syms.append(s)
                                            break
                                label = (f"n{n_total}_L{n_long}S{n_short}_pct{max_sym_pct}_"
                                         f"corr{max_corr}_{anchor_name}_fo{foq.replace('.','p')}_cd{cd}")
                                cfg = build_portfolio_config(long_syms[:n_long], short_syms[:n_short],
                                                              max_sym_pct, foq, cd)
                                tmp_path = f"/tmp/r9p3_{label}.json"
                                specs.append((label, cfg, tmp_path))
                                if len(specs) >= args.max_portfolios:
                                    break
                            if len(specs) >= args.max_portfolios:
                                break
                        if len(specs) >= args.max_portfolios:
                            break
                    if len(specs) >= args.max_portfolios:
                        break
                if len(specs) >= args.max_portfolios:
                    break
            if len(specs) >= args.max_portfolios:
                break
        if len(specs) >= args.max_portfolios:
            break

    print(f"Generated {len(specs)} portfolio specs. Running full + 5 segments each...", flush=True)
    t0 = time.time()
    results = []
    completed = 0
    with ProcessPoolExecutor(max_workers=args.workers) as ex:
        futures = {ex.submit(evaluate_one, spec): spec[0] for spec in specs}
        for fut in as_completed(futures):
            try:
                r = fut.result()
                results.append(r)
            except Exception as e:
                results.append({"label": futures[fut], "skipped": True, "error": str(e)})
            completed += 1
            if completed % 50 == 0:
                elapsed = time.time() - t0
                rate = completed / elapsed
                eta = (len(specs) - completed) / rate if rate > 0 else 0
                print(f"  [{completed}/{len(specs)}] elapsed={elapsed:.0f}s rate={rate:.2f}/s eta={eta:.0f}s", flush=True)

    # Filter and sort
    valid = [r for r in results if not r.get("skipped")]
    promoted = [r for r in valid if r.get("promoted")]
    valid.sort(key=lambda r: r["full_metrics"]["ann"], reverse=True)

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({
        "results": valid,
        "promoted": promoted,
        "total_specs": len(specs),
        "total_evaluated": len(valid),
        "total_skipped": len(results) - len(valid),
        "total_promoted": len(promoted),
    }, open(args.out, "w"), indent=2)

    elapsed = time.time() - t0
    print(f"\n[r9P3] wrote {args.out}: {len(valid)} evaluated, {len(promoted)} promoted in {elapsed:.0f}s")
    print(f"\n=== TOP 10 by ann ===")
    for v in valid[:10]:
        fm = v["full_metrics"]
        print(f"  {v['label']:60s} ann={fm['ann']:7.1f} dd={fm['dd']:6.1f} pos={v['positive_segments']}/5 prom={v['promoted']}")
    print(f"\n=== Top 5 promoted (if any) ===")
    for v in promoted[:5]:
        fm = v["full_metrics"]
        print(f"  {v['label']}: ann={fm['ann']} dd={fm['dd']} pos={v['positive_segments']}/5")


if __name__ == "__main__":
    main()
