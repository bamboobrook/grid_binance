# GLM Next Steps After Martingale Margin Fix

Date: 2026-06-25
Remote repo: `/home/bumblebee/Project/grid_binance`
Previous plan: `docs/superpowers/plans/2026-06-25-martingale-margin-capital-parity-fix-for-glm.md`
Audit report: `docs/superpowers/reports/2026-06-25-glm-martingale-worktree-audit.md`

## Non-Negotiable Safety Rules

Do not start formal live trading.

Do not start `trading-engine` while any smoke or formal executor strategy is in an inconsistent DB state.

Before any live smoke:

- Binance USDT-M must have zero open orders.
- Binance USDT-M must have zero non-zero positions.
- Smoke portfolio and executor strategies must be internally consistent.
- Smoke order notional must be above Binance min notional.
- Smoke total required margin must be within 50 USDT.

Formal 1000 USDT conservative portfolio requires explicit user confirmation after all fixes and smoke tests pass.

Backtest rerun is a hard gate before any live smoke or formal live work. Under the corrected margin-principal model, GLM must first rerun the three strategy modes and find portfolios meeting the user's targets:

- Conservative: annualized return > 50%.
- Balanced: annualized return > 90%.
- Aggressive: annualized return > 110%.

Drawdown must also remain controlled by each mode's risk requirement. If these targets cannot be reached after a correct search, GLM must stop before live work and explain why, with the best corrected results and the optimization attempts already tried.

## Current Issues To Correct

### Issue 1: Rebacktest Path May Not Be Authoritative

Current queued/running tasks:

- `lp-martingale-conservative-20260625-parity`
- `lp-martingale-balanced-20260625-parity`
- `lp-martingale-aggressive-20260625-parity`

Observed:

- These use `search_mode=funding_repriced_lp_recombine`.
- This looks like a recombine/reprice path, not clearly a full from-scratch candidate search.
- User asked for corrected rebacktests after the capital model fix, not just recombination of old winners.

Required:

- Prove whether `funding_repriced_lp_recombine` reruns candidate metrics from raw 1m bars under the new model.
- If it only recombines existing candidate equity curves or summaries, it is not sufficient.
- Run a true from-scratch search under the fixed model for conservative, balanced, and aggressive.

### Issue 2: Backtest CPU Utilization Is Too Low

Observed:

- Worker container sees 30 CPUs.
- `BACKTEST_WORKER_MAX_THREADS=12`.
- Actual CPU was around `211%`, roughly 2 cores.
- Conservative task stayed at `search_symbol` around 35% with little visible progress.

Required:

- Raise effective parallelism and verify real CPU use.
- Add better progress logging if needed.
- Avoid running a single slow serial path when the machine has 30 CPUs.

### Issue 3: Smoke Test Is Not Clean

Observed DB state:

- `mp_live_smoke_50_v2_20260624` is `stopped`.
- `risk_summary.live_executor_started=true`.
- `risk_summary.live_executor_state=started`.
- `smoke-sol-50-v2-long-20260624` is `Running`.
- `smoke-sol-50-v2-short-20260624` is `Running`.
- Each smoke strategy has one local DB `Working` order.
- Recent events show `order notional 6.802 is below minimum 20 for SOLUSDT`.

Required:

- Do not start `trading-engine` from this state.
- Reset or recreate the smoke test from a clean DB state.
- Ensure the 50U smoke order notional is above Binance min notional and margin is within 50U.

## Step 1: Make A Checkpoint Before Further Work

Do not include unrelated web/billing changes in the Martingale core checkpoint.

Recommended:

1. Review `docs/superpowers/reports/2026-06-25-glm-martingale-worktree-audit.md`.
2. Create a patch archive or branch.
3. Stage only the Martingale capital model and required live parity files.
4. Exclude temp monitor files.

Suggested commit split:

```text
fix: align martingale backtest and live margin capital model
fix: harden martingale live preflight and smoke safety
```

If committing is not desired yet, at least create patch files:

```bash
cd /home/bumblebee/Project/grid_binance
git diff > /tmp/martingale-margin-capital-current.diff
git status --short > /tmp/martingale-margin-capital-status.txt
```

## Step 2: Reconfirm Tests Before Rerun

Run and save logs:

```bash
cd /home/bumblebee/Project/grid_binance
. "$HOME/.cargo/env"
cargo test -p backtest-engine martingale -- --nocapture
cargo test -p backtest-engine portfolio -- --nocapture
cargo test -p backtest-worker -- --nocapture
cargo test -p trading-engine martingale -- --nocapture
cargo test -p trading-engine order_sync -- --nocapture
cargo test -p api-server martingale -- --nocapture
```

Minimum must-pass cases:

- `futures_planned_margin_uses_notional_divided_by_leverage`
- 10/2x/4-leg/2x gives planned notional 150 and planned margin 75.
- Annualized return uses margin principal.
- Drawdown uses margin equity curve.
- Portfolio combined curve uses corrected margin equity.
- Long/short publish weights do not double capital.
- Start preflight rejects over-budget portfolios.
- Budget-blocked safety leg is not persisted as live-submittable `Working`.
- Existing ordinary grid tests still pass.

## Step 3: Fix Backtest Worker Parallelism

Target:

- Use most of the 30 visible CPUs without destabilizing the host.
- Start with `BACKTEST_WORKER_MAX_THREADS=24`.
- Confirm worker CPU rises materially above the current ~2 cores during screening.

Actions:

1. Update runtime environment to use 24 threads for worker:

```bash
cd /home/bumblebee/Project/grid_binance
BACKTEST_WORKER_MAX_THREADS=24 docker compose --env-file .env -f deploy/docker/docker-compose.yml up -d backtest-worker
```

If compose does not pass the override, update `.env` or the compose invocation explicitly.

2. Confirm inside container:

```bash
docker compose --env-file .env -f deploy/docker/docker-compose.yml exec -T backtest-worker sh -lc 'nproc; env | sort | grep BACKTEST_WORKER'
```

Expected:

```text
nproc >= 24
BACKTEST_WORKER_MAX_THREADS=24
```

3. Observe CPU:

```bash
docker stats --no-stream --format 'table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}'
```

Expected during screening:

```text
backtest-worker CPU substantially above 200%; ideally 800%+ depending stage
```

4. If CPU remains around 200%, inspect code path:

- `screen_candidates_bounded_parallel`
- `evaluate_refinement_candidates_parallel`
- task `search_mode`
- symbol loop scheduling
- any global lock or serial recombine stage

5. Add progress logging:

- symbol started/completed,
- candidate counts,
- active thread width,
- per-stage elapsed time,
- portfolio combine elapsed time.

## Step 4: Replace Or Prove The Current Parity Tasks

Current tasks with `funding_repriced_lp_recombine` are allowed only as quick sanity checks.

They are not final unless GLM proves all candidate metrics are recomputed from raw market bars under the corrected margin model.

Required proof:

- Show the code path from `search_mode=funding_repriced_lp_recombine` to `run_kline_screening_with_funding`.
- Show that candidate `equity_curve`, `planned_margin_quote`, `planned_notional_quote`, `annualized_return_pct`, and `max_drawdown_pct` are newly recomputed after the fix.
- Show no stale `backtest_candidate_summaries.summary.equity_curve` from old capital semantics is reused as the source of truth.

If proof is missing, create new from-scratch tasks:

```text
martingale-conservative-20260625-margin-v2
martingale-balanced-20260625-margin-v2
martingale-aggressive-20260625-margin-v2
```

Requirements for each task:

- `strategy_type=martingale_auto_search`
- `direction_mode=long_short`
- `interval=1m`
- `start_ms=1672531200000`
- `end_ms=1780271999999`
- risk profile-specific drawdown limits:
  - conservative: <= 10%
  - balanced: <= 20%
  - aggressive: <= 30%
- funding data enabled if the previous accepted strategy used funding-aware evaluation.
- enough `random_candidates`, `intelligent_rounds`, and `per_symbol_top_n` to match or exceed the previous search depth.
- use the corrected margin model.

Do not rely on old winners until these new tasks complete.

## Step 5: Backtest Completion Criteria

This step is mandatory and must finish before any smoke test, exchange preconfiguration for a formal portfolio, or live executor start.

For each of conservative, balanced, aggressive:

1. Task status is `succeeded`.
2. Candidate summaries contain:
   - `planned_margin_quote`
   - `planned_notional_quote`
   - `max_capital_used_quote`
   - corrected `annualized_return_pct`
   - corrected `max_drawdown_pct`
   - corrected equity curve
   - corrected drawdown curve
3. Portfolio candidates are recomputed from corrected member curves.
4. Top portfolio details and curves are visible in the flyingkid account.
5. Results are exported into a dated report.

Target search goals remain:

- Conservative: find a combination with annualized return > 50% and drawdown <= 10%, using corrected margin principal.
- Balanced: find a combination with annualized return > 90% and drawdown <= 20%, using corrected margin principal.
- Aggressive: find a combination with annualized return > 110% and drawdown <= 30%, using corrected margin principal.

These targets intentionally match the user's requirement to recover the previous practical performance level after fixing the本金口径. Do not downgrade the target to "better than old stale metrics"; the accepted thresholds are now the three numeric annualized-return gates above.

If a goal is not met:

- Do not proceed to live smoke or formal live trading.
- Report the best corrected portfolio for that mode.
- Report annualized return, max drawdown, planned margin, planned notional, symbol allocation, parameter set, and equity/drawdown curve.
- Explain whether the gap comes from the corrected principal denominator, drawdown filtering, funding assumptions, fees/slippage, search breadth, or a missing/changed strategy mechanic.
- Continue optimization only under the corrected model, or ask the user before accepting a lower target.

If GLM adds any new search parameters, indicator rules, position-sizing behavior, entry/exit behavior, TP/SL behavior, funding handling, fee handling, or order-generation logic while trying to reach these targets, the same behavior must be implemented in the live trading modules before smoke testing. Backtest-only behavior is not acceptable.

Before moving past Step 5, GLM must produce a parity table:

```text
Mode          Annualized target  Corrected annualized  Drawdown limit  Corrected drawdown  Pass/Fail
Conservative  > 50%              ...                   <= 10%          ...                 ...
Balanced      > 90%              ...                   <= 20%          ...                 ...
Aggressive    > 110%             ...                   <= 30%          ...                 ...
```

All three rows must be `Pass` before Step 6 begins.

## Step 6: Reset Smoke State Before Any Live Smoke

Do not reuse the current dirty `mp_live_smoke_50_v2_20260624` state as-is.

First choose one of these:

Preferred:

- Create a new smoke portfolio ID with a fresh source task and no existing executor strategies.

Acceptable if done carefully:

- Back up and reset `mp_live_smoke_50_v2_20260624`, its executor strategies, orders, runtime positions, events, and live snapshots.

Before starting smoke:

```bash
node /tmp/binance_usdm_readonly_probe_all.js
```

Must show:

```text
openOrderCount=0
nonzeroPositionCount=0
multiAssetsMargin=false
dualSidePosition=true
```

DB must show:

- smoke portfolio status is `pending_confirmation` or `paused` before start,
- `live_executor_started` is false or absent,
- no stale executor strategy `Working` orders,
- no stale runtime positions,
- no conflicting strategies for the smoke symbol.

## Step 7: Design A Valid 50U Smoke

The current SOL smoke failed because `order notional 6.802 is below minimum 20 for SOLUSDT`.

Use a smoke config satisfying both:

- initial order notional >= Binance min notional, preferably >= 25 USDT,
- required margin <= 50 USDT after leverage and buffer.

Example pattern:

```text
symbol=SOLUSDT or another liquid USDT-M symbol
direction=long_short only if hedge-mode path must be tested
first_order_quote=25  # notional
leverage=2
max_legs=1 for initial order smoke, then controlled max_legs=2 for safety-leg smoke
planned_margin_per_side=12.5
two sides total planned initial margin=25
50U smoke budget can cover this plus fees/buffer
```

If testing safety leg:

```text
first_order_quote=20 or 25 notional
multiplier=1
max_legs=2
leverage=2
planned_margin per side=20-25
```

Keep total margin plus buffer <= 50.

Do not choose a notional that Binance rejects.

## Step 8: Smoke Test Execution Protocol

Only after Step 6 and Step 7:

1. Preconfigure exchange settings.
2. Run `confirm-start` with `max_global_budget_quote=50`.
3. Start `trading-engine`.
4. Watch for exactly expected behavior:
   - no duplicate order creation,
   - order notional above min notional,
   - position side correct for hedge mode,
   - order submitted once,
   - runtime record matches Binance order,
   - fees/trades are synced,
   - funding sync path does not crash,
   - TP/SL logic is present or explicitly simulated,
   - safety leg generation respects budget.
5. Stop smoke.
6. Cancel all open orders.
7. Flatten all positions.
8. Stop `trading-engine`.
9. Probe Binance again to confirm zero open orders and zero positions.
10. Save smoke evidence in a report.

Hard stop conditions:

- Any `-2019 Margin is insufficient`.
- Any Binance min-notional rejection.
- Any duplicate order for the same leg.
- Any mismatch between DB strategy status and portfolio status.
- Any open order or position remains after cleanup.

## Step 9: Formal Portfolio Remains Blocked

Do not start `mp_funding_conservative_20260623` or any formal 1000U portfolio yet.

Formal start requires:

- corrected from-scratch backtests complete,
- top conservative portfolio selected under corrected metrics,
- 50U smoke passed,
- user explicitly confirms formal start.

## Step 10: Final Report Required From GLM

GLM should produce one report after completing the next phase:

```text
docs/superpowers/reports/2026-06-25-martingale-margin-v2-rerun-and-smoke.md
```

The report must include:

- git commit or patch IDs included,
- exact tests run and pass/fail,
- worker CPU/thread settings,
- task IDs for corrected from-scratch searches,
- final conservative/balanced/aggressive results,
- evidence that old metrics were not reused,
- smoke portfolio ID,
- smoke order IDs,
- Binance before/after probes,
- cleanup confirmation,
- remaining blockers before formal 1000U start.

## Current Recommended Decision

Do not continue the existing smoke by merely starting `trading-engine`.

First:

1. Reset/recreate the smoke state.
2. Fix smoke sizing above Binance min notional.
3. Decide whether current `funding_repriced_lp_recombine` tasks are only sanity checks.
4. Launch true from-scratch corrected searches with higher worker parallelism.

This keeps the process aligned with the user's requirement: corrected backtest first, clean 50U smoke second, formal live only after explicit confirmation.
