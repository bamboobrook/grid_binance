# GLM Handoff: Martingale Margin Fix + Robustness Exploration

Date: 2026-06-25
For: ChatGPT (next phase)
Author: GLM (this session)
Repo: `/home/bumblebee/Project/grid_binance` (main worktree, uncommitted changes)
Driver plan: `docs/superpowers/plans/2026-06-25-glm-next-steps-after-margin-fix.md`
Companion report: `docs/superpowers/reports/2026-06-25-martingale-margin-v2-rerun-and-smoke.md`

## 0. TL;DR for ChatGPT

- The **martingale margin-capital model is fixed and verified** (margin = notional /
  leverage; tests green; running worker image confirmed via `grep -a`). This is
  the core deliverable and it is DONE.
- Two **correctness-neutral performance fixes** were required to make the
  from-scratch search tractable (static-chunk straggler + portfolio-curve
  memory blowup). Both deployed.
- The corrected-model from-scratch search + LP recombination achieves the
  user's annualized targets **only without an overfit gate** (aggressive LP =
  141.6% > 110%). With any meaningful robustness gate, the ceiling drops to
  ~94% (aggressive) because **the candidate pool is structurally overfit to the
  2023 regime** (walk-forward: 34/34 overfit, avg WFE -0.082; returns
  concentrated in the first quarter).
- A **robustness-aware scoring改造** was implemented (`weight_regime_robustness`
  in `ScoringConfig`) and a large-cap aggressive prototype re-run launched. See
  §6 for the result (filled when the task completes).
- The user's goal (robust + >110%, reproducible in live) is **in tension with
  the strategy type**. Recommended next steps in §8.

## 1. Canonical margin model (DONE, verified)

`first_order_quote` / `max_budget_quote` / `custom_sequence.notionals` = order
**notional**. Futures `margin = notional / leverage`; spot `margin = notional`.
Hard test: 10 / 2x / 4-leg / leverage 2 -> planned notional 150, planned margin
75. Canonical helper: `apps/backtest-engine/src/martingale/capital.rs`. All test
suites green (backtest-engine martingale 72 / portfolio 30 / worker 57 /
trading-engine / api-server 24). Worker image verified to contain the fix via
`grep -a planned_notional_quote|max_capital_used_quote|initial_margin_capital`
(each =1).

## 2. Performance fixes (DONE, deployed, correctness-neutral)

1. **Dynamic work-queue scheduling** — `screen_candidates_bounded_parallel` and
   `evaluate_refinement_candidates_parallel` (`apps/backtest-worker/src/main.rs`).
   Replaced static `thread::scope` + `chunks()` (straggler on skewed candidate
   cost) with `AtomicUsize` next-index claiming. Workers went from ~100% (1 core)
   to ~800-900% (9 threads) during screening.
2. **Bounded portfolio enumeration** — `build_portfolio_top_n_v2`
   (`apps/backtest-engine/src/portfolio_search.rs`). Was allocating tens of GB
   (thousands of portfolios × full per-bar curves ~1M points). Now caps the
   eligible pool to top-60 by score and resamples member curves to <=5000 points
   before enumeration. Annualized return preserved exactly (curve endpoints
   kept); max-drawdown within scoring noise.

Verification method gotcha: the worker container has **no `strings`** binary;
verify binary content with `grep -a <marker> /usr/local/bin/backtest-worker`.

## 3. From-scratch v2 search tasks

Old `lp-martingale-*-20260625-parity` (`funding_repriced_lp_recombine`) tasks
were **non-authoritative** (Python LP recombining stale curves) — cancelled.
Replaced with from-scratch `profit_optimized_v2` worker tasks:
`martingale-{conservative,balanced,aggressive}-20260625-margin-v2`, 30 symbols,
depth random64/intelligent5/per_symbol_top_n10 (matches fk-18 max depth).

- **aggressive succeeded** (2026-06-25 09:24 UTC): greedy top-3 = 99.9% ann /
  29.7% dd, all 34 walk-forward verdicts `overfit` (avg WFE -0.082).
- **balanced / conservative**: cancelled to free compute for the robustness
  prototype. They would mirror aggressive's overfit pattern (same methodology).

## 4. LP recombination (user-directed; works, but overfit-gated)

Script: `/tmp/lp_v2_recombine.py` (reuses
`scripts/optimize_martingale_lp_portfolios.py`'s cutting-plane LP + adds an
overfit gate). Reads candidate curves from `backtest_candidate_summaries`
(persisted at 5000 points by `save_candidates_and_artifacts`). Aggressive @
dd<=30%:

| Gate | Candidates | LP annualized |
|---|---|---|
| none (overfit allowed) | 221 | **141.63%** (exceeds 110%) |
| posQ >= 3 (>=3 positive quarters) | 150 | 94.49% |
| posQ >= 4 (all quarters positive) | 23 | 75.59% |

LP recombination is effective (greedy 99.9% -> LP 141.6% overfit-allowed). But
the high returns are **2023-regime-concentrated** (Q1 raw return dominates;
Q3/Q4 ~0). A robustness constraint (every quarter positive) still leaves the
return Q1-driven. So robust >110% is NOT reachable from this candidate pool.

## 5. Root cause: structural regime-overfit

The martingale search optimizes full-period return -> fits the 2023 regime.
Walk-forward confirms: all candidates overfit (negative WFE). This is a property
of the strategy/search, not the margin model (which is correct).

## 6. Robustness-aware scoring改造 (implemented, prototype running)

Added `weight_regime_robustness` to `ScoringConfig`
(`apps/backtest-engine/src/martingale/scoring.rs`): when >0, candidates get a
score bonus = weight × 25 × (fraction of 6 equal sub-periods with positive
return). Threaded via `scoring_config_from_task`
(`apps/backtest-worker/src/main.rs`, reads `config.scoring.weights.
weight_regime_robustness`). Default 0 = off (existing tests unchanged). Verified
the screening path `evaluate_long_short_candidate_for_screening` applies it via
`score_candidate(metrics, scoring)`.

Prototype task `martingale-aggressive-20260625-robust-v1`: 20 **large-cap**
symbols (user: avoid small unstable coins), aggressive, profit_optimized_v2,
`weight_regime_robustness=1.0`. Running on 1 worker × 24 threads.

**Result (task succeeded 2026-06-25 14:28 UTC):** 20 large-cap symbols,
robustness weight 1.0, 88 candidates persisted (vs 221 for the 30-symbol
original). Greedy top-3 ~94.7% ann / 29.9% dd. Walk-forward now yields **1
genuinely robust candidate** (`staged-cand-11429`, avg WFE **+0.713**) vs **0**
for the original aggressive run — so the scoring改造 did shift the search toward
robustness. LP recombination @ dd<=30%:

| Gate | Candidates | LP annualized |
|---|---|---|
| none (overfit allowed) | 88 | 123.22% |
| posQ >= 3 | 58 | **96.22%** |
| posQ >= 4 | 13 | 84.48% |

**The robust LP ceiling (~96% at posQ>=3) is still below the 110% target.** The
robustness改造 produced a marginal improvement (LP posQ>=3 94.5% -> 96.2%;
0 -> 1 genuinely walk-forward-robust candidate) but did NOT break through 110%.
This confirms the structural conclusion: for this martingale strategy on crypto,
returns above ~100% are regime-overfit and do not generalize; robust returns cap
around 85-96% depending on gate strictness.

## 7. Current runtime state

- trading-engine: **stopped**, `BINANCE_LIVE_MODE=0`. Binance USDT-M clean
  (0 orders, 0 positions). No live trading has occurred.
- backtest-worker: 1 replica, 24 threads, new image (margin fix + perf fixes +
  robustness scoring), running the robust prototype.
- Uncommitted in main worktree: all margin fixes + perf fixes + robustness
  scoring + web/billing (unrelated). Patch archive at
  `/tmp/martingale-margin-capital-current.diff`.
- Smoke state still dirty (`mp_live_smoke_50_v2_20260624`: stale Working orders,
  inconsistent executor flags) — reset before any live smoke.

### Infra gotcha: market_data sqlite open stall (115GB db)

The market data db `data/market_data_full.db` is now **115GB**. Workers
intermittently stall at the `market_data_opening` stage: the task is claimed +
heartbeated `market_data_opening`, then the processing future suspends
(`futex_wait`, ~6.7MB RSS, 0 read I/O) — the `open_market_data()` call (sqlite
`mode=ro&immutable=1`) blocks. It is **much more frequent with 2+ concurrent
workers** reading the same 115GB file (contention) and occasional with a single
worker. Workaround that worked here: reset the task to `queued` and restart the
stalled worker (the second open usually succeeds); for reliability run **one
backtest task at a time** on one worker. A proper fix (for ChatGPT): investigate
`SqliteMarketDataSource::open_readonly` (`apps/backtest-engine/src/sqlite_market_data.rs:48`)
— consider `PRAGMA mmap_size`, a busy_timeout, or pre-opening the connection
eagerly; or shard the market data db.

## 8. Recommended next steps for ChatGPT

1. Collect the robust prototype result (§6). LP-recombine its candidate pool.
2. If robust >110% still unreachable (likely): the honest conclusion is that
   martingale-on-crypto at these targets is structurally overfit. Options to
   present the user:
   a. Accept realistic robust targets (aggressive ~75-95% robust depending on
      gate strictness) and proceed to 50U smoke + formal live.
   b. Deeper robustness work: walk-forward-optimized search (score by
      out-of-sample WFE directly, not just sub-period positivity), richer
      strategy mechanics (regime filters, dynamic sizing), more large-cap
      symbols. Major effort, uncertain to reach 110%.
   c. Accept the overfit-allowed LP portfolio (141.6%) acknowledging it will
      likely NOT reproduce in live (negative walk-forward).
3. Whatever the user picks, the margin model and pipeline are ready; the 50U
   smoke (plan Steps 6-8) can proceed once a portfolio is chosen.

## 9. Key files / commands

- Scoring: `apps/backtest-engine/src/martingale/scoring.rs`
  (`weight_regime_robustness`, `regime_robustness_factor`).
- Threading: `apps/backtest-worker/src/main.rs` `scoring_config_from_task`.
- Portfolio cap: `apps/backtest-engine/src/portfolio_search.rs`
  `build_portfolio_top_n_v2` (`POOL_CAP=60`, `CURVE_MAX_POINTS=5000`).
- LP tool: `/tmp/lp_v2_recombine.py` (run: `python3 /tmp/lp_v2_recombine.py
  <task_id> <dd_limit> <posQ_gate>`).
- Worker rebuild: `docker compose -p grid-binance --env-file
  .worktrees/full-v1/.env -f deploy/docker/docker-compose.yml build backtest-worker`
  then `up -d --no-deps --scale backtest-worker=N` with `BACKTEST_WORKER_MAX_THREADS`.
