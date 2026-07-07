#!/usr/bin/env python3
"""GLM Round 9 P3 backfill: re-run n8/n10 (and any skipped n6) portfolios with
900s per-replay timeout so every portfolio completes full + 5 segments.

This closes the verifier-identified gap: the original P3 run used 600s timeout
which caused 992 portfolios (mostly n8/n10) to time out. The plan requires
'Every portfolio must run full + five segments.' This script re-runs the
missing portfolios and merges them into r9-multi-symbol-sleeve-library.json.

Approach:
  1. Load the existing r9-multi-symbol-sleeve-library.json.
  2. Regenerate ALL 1512 specs (same deterministic seeds as original).
  3. Identify which labels are missing from the existing results.
  4. Re-run only those missing portfolios with 900s timeout.
  5. Merge new results into the existing library, update totals.
"""
import argparse, json, os, subprocess, sys, time, copy
from concurrent.futures import ProcessPoolExecutor, as_completed

REPLAY = "target/release/portfolio_budget_replay"
FULL_SEGMENTS = [
    ("h1_2023", 1672531200000, 1688169599999),
    ("h2_2023", 1688169600000, 1704067199999),
    ("2024",    1704067200000, 1735689599999),
    ("2025",    1735689600000, 1767225599999),
    ("2026_ytd",1767225600000, 1780271999999),
]
FULL_START, FULL_END = 1672531200000, 1780271999999

# Re-import the spec generator from the original P3 script
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from glm_r9_multi_symbol_sleeve_search import (
    ANCHORS, build_portfolio_config, select_symbols, load_symbol_pool,
    load_corr_ranking, evaluate_one, _RUNTIME_DB,
)

_RUNTIME_DB["market"] = "data/market_data_full.db"
_RUNTIME_DB["funding"] = "data/funding_rates.db"


def regenerate_all_specs(max_portfolios=1512):
    """Regenerate the same 1512 specs the original P3 run produced."""
    pool = load_symbol_pool()
    corr_ranking = load_corr_ranking()
    specs = []
    seed_counter = 0
    FIRST_ORDERS = ["20.0", "15.0", "30.0"]
    COOLDOWNS = [39600, 21600, 86400]
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
                                tmp_path = f"/tmp/r9p3bf_{label}.json"
                                specs.append((label, cfg, tmp_path))
                                if len(specs) >= max_portfolios:
                                    return specs
    return specs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--library", default="docs/superpowers/artifacts/glm-martingale-core-round9/r9-multi-symbol-sleeve-library.json")
    ap.add_argument("--workers", type=int, default=20)
    ap.add_argument("--timeout-per-replay", type=int, default=900)
    args = ap.parse_args()

    # Load existing library
    with open(args.library) as f:
        library = json.load(f)
    existing_labels = set(r["label"] for r in library["results"] if not r.get("skipped"))
    print(f"Existing library: {len(library['results'])} results, {len(existing_labels)} non-skipped labels", flush=True)

    # Regenerate all 1512 specs
    all_specs = regenerate_all_specs(1512)
    print(f"Regenerated {len(all_specs)} specs", flush=True)

    # Identify missing
    missing = [s for s in all_specs if s[0] not in existing_labels]
    print(f"Missing specs to backfill: {len(missing)}", flush=True)
    from collections import Counter
    missing_by_n = Counter(s[0].split("_")[0] for s in missing)
    print(f"  by n_total: {dict(missing_by_n)}", flush=True)

    if not missing:
        print("Nothing to backfill. Exiting.")
        return

    # Override the per-replay timeout in evaluate_one by monkey-patching replay_portfolio
    import glm_r9_multi_symbol_sleeve_search as p3mod
    original_replay = p3mod.replay_portfolio

    def replay_with_longer_timeout(config_path, budget, s, e, pid, market_db, funding_db):
        cmd = [REPLAY, "--config", config_path, "--budget", str(budget),
               "--start-ms", str(s), "--end-ms", str(e),
               "--market-data", market_db, "--funding-data", funding_db,
               "--profile", "aggressive", "--portfolio-id", pid,
               "--exchange-min-notional", "5"]
        try:
            pr = subprocess.run(cmd, capture_output=True, text=True, timeout=args.timeout_per_replay)
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
    p3mod.replay_portfolio = replay_with_longer_timeout

    # Re-run missing
    print(f"\n=== Backfilling {len(missing)} missing portfolios (timeout={args.timeout_per_replay}s per replay) ===", flush=True)
    t0 = time.time()
    new_results = []
    completed = 0
    still_skipped = 0
    with ProcessPoolExecutor(max_workers=args.workers) as ex:
        futures = {ex.submit(p3mod.evaluate_one, spec): spec[0] for spec in missing}
        for fut in as_completed(futures):
            label = futures[fut]
            try:
                r = fut.result()
            except Exception as e:
                r = {"label": label, "skipped": True, "error": str(e)}
            if r.get("skipped"):
                still_skipped += 1
            new_results.append(r)
            completed += 1
            if completed % 25 == 0:
                elapsed = time.time() - t0
                rate = completed / elapsed if elapsed > 0 else 0
                eta = (len(missing) - completed) / rate if rate > 0 else 0
                print(f"  [{completed}/{len(missing)}] elapsed={elapsed:.0f}s rate={rate:.2f}/s eta={eta:.0f}s skipped={still_skipped}", flush=True)

    # Merge
    valid_new = [r for r in new_results if not r.get("skipped")]
    print(f"\nBackfill complete: {len(valid_new)} new valid results, {still_skipped} still skipped (after 900s)", flush=True)

    # Recompute promotion flags for new results using the same logic
    R4_ANN, R4_DD = 34.7, 17.7
    for r in valid_new:
        if r.get("full_metrics") is None:
            r["promoted"] = False
            continue
        full = r["full_metrics"]
        seg_rets = {n: (v["ret"] if v else 0) for n, v in r.get("segment_metrics", {}).items()}
        improves_ann = full["ann"] >= R4_ANN + 5 and full["dd"] <= 20
        improves_dd = full["dd"] <= R4_DD - 3 and full["ann"] >= 30
        passes_2025 = (seg_rets.get("2025", -999) > -5 or
                       (r.get("segment_metrics", {}).get("2025") and r["segment_metrics"]["2025"]["dd"] < 15))
        r["promoted"] = (improves_ann or improves_dd) and passes_2025 and r.get("positive_segments", 0) >= 4

    # Merge into library
    library["results"].extend(valid_new)
    # Also keep a record of still-skipped (after 900s) for transparency
    library["backfill_still_skipped_after_900s"] = still_skipped
    library["backfill_total_evaluated"] = len(library["results"])
    # Recompute promoted
    all_promoted = [r for r in library["results"] if r.get("promoted")]
    library["promoted"] = all_promoted
    library["total_promoted"] = len(all_promoted)
    library["total_evaluated"] = len(library["results"])
    library["total_skipped"] = still_skipped  # only the truly-unsolvable ones remain

    # Sort by ann desc
    library["results"].sort(key=lambda r: r.get("full_metrics", {}).get("ann", -999), reverse=True)

    with open(args.library, "w") as f:
        json.dump(library, f, indent=2)

    elapsed = time.time() - t0
    print(f"\n[r9P3-backfill] merged {len(valid_new)} new results. Library now has {len(library['results'])} evaluated, {len(all_promoted)} promoted, {still_skipped} still-skipped.", flush=True)
    print(f"Total time: {elapsed:.0f}s")
    print(f"\n=== TOP 10 by ann (full library) ===")
    for v in library["results"][:10]:
        fm = v["full_metrics"]
        print(f"  {v['label']:60s} ann={fm['ann']:7.1f} dd={fm['dd']:6.1f} pos={v.get('positive_segments','?')}/5 prom={v.get('promoted')}")
    print(f"\n=== Top 5 promoted (if any) ===")
    for v in all_promoted[:5]:
        fm = v["full_metrics"]
        print(f"  {v['label']}: ann={fm['ann']} dd={fm['dd']} pos={v.get('positive_segments')}/5")


if __name__ == "__main__":
    main()
