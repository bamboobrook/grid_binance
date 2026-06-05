# FlyingKid Claude Follow-Up Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:systematic-debugging for the frontend 502 root cause, and use superpowers:executing-plans if executing this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 保留 FlyingKid 账户最早三个有效回测结果，恢复前端访问，并在真实成本和全量 1m K 线约束下继续寻找更高收益的保守、平衡、激进组合。

**Architecture:** 先做运行态和数据保全，再修复 web/nginx 502；随后只在 FlyingKid 账户下执行小批次、可复核的深搜任务。搜索优先使用已落地的 `profit_optimized_v2`、ATR/ADX、walk-forward 和组合曲线优化能力，不缩短回测区间、不移除手续费/滑点、不伪造结果。

**Tech Stack:** Docker Compose, PostgreSQL `backtest_tasks`, Rust `backtest-engine`, Rust `backtest-worker`, Next.js `apps/web`, nginx, SQLite market data `/home/bumblebee/Project/discord_c2im/pipeline/data/market_data.db`.

---

## Current Verified State

- Branch/worktree: `/home/bumblebee/Project/grid_binance`, branch `main`.
- Current dirty files:
  - `apps/backtest-engine/src/indicators.rs`
  - `apps/backtest-engine/src/martingale/kline_engine.rs`
  - `apps/backtest-engine/src/portfolio_search.rs`
  - `deploy/docker/docker-compose.yml`
- Running compose state: `api-server`, `postgres`, `redis`, `nginx`, `trading-engine`, `backtest-worker x4`, `prometheus` are up; `web` is not running.
- Frontend 502 root cause evidence: nginx logs contain `web could not be resolved`; `deploy/nginx/default.conf` routes `/` and `/api/` to `web:3000`; `docker compose ps -a web` returned no web container.
- Host port `3000` is occupied by an unrelated Next app titled `Wallet Control`; do not use host `3000` as proof that this app is healthy.
- No active backtest tasks: query for `queued/running/paused` returned 0 rows.

## FlyingKid Tasks To Preserve

Current `flyingkid2022@outlook.com` tasks are exactly three, all `succeeded`. Treat these as the protected keep-list unless the user explicitly changes it:

| Keep | Task ID | Created UTC | Risk | Search | Top1 Annualized | Top1 DD | Top1 Return | Members |
| --- | --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| yes | `fk-18-balanced-wide-seed127-20260530` | 2026-05-30 00:39:16 | balanced | `profit_optimized_v2` | 59.92% | 19.59% | 377.89% | 10 |
| yes | `fk-18-bal-v2-seed53-20260601` | 2026-06-01 14:23:09 | balanced | `profit_optimized_v2` | 65.52% | 19.32% | 435.96% | 11 |
| yes | `fk-18-agg-v2-seed173-20260601` | 2026-06-01 14:23:10 | aggressive | `profit_optimized_v2` | 77.00% | 28.03% | 570.05% | 10 |

Important caveat: the two 2026-06-01 task configs have `fee_bps` blank in `config`, but the engine default cost model charges 4.5 bps fee and 2.0 bps slippage per fill. Verify artifacts/metrics before comparing them as final baselines.

## GLM/OpenCode Optimization Check

- ATR/ADX plan exists in `docs/superpowers/plans/2026-06-02-martingale-indicator-walkforward-plan.md`.
- ATR/ADX implementation is present in `apps/backtest-engine/src/search.rs`, `apps/backtest-engine/src/walk_forward.rs`, `apps/backtest-engine/src/time_splits.rs`, and `apps/backtest-worker/src/main.rs`.
- Current uncommitted diffs add:
  - public `true_range` for incremental ATR;
  - incremental ATR cache inside `IndicatorRuntimeContext`;
  - portfolio rejection for same symbol with conflicting leverage;
  - compose market-data mount change from a single DB file to read-only data directory.
- Verified tests passed:
  - `cargo check -p backtest-worker`
  - `cargo test -p backtest-engine staged_tests -- --nocapture`
  - `cargo test -p backtest-engine walk_forward -- --nocapture`
  - `cargo test -p backtest-engine portfolio_rejects_same_symbol_different_leverage -- --nocapture`
  - `cargo test -p backtest-engine portfolio_allows_same_symbol_same_leverage -- --nocapture`
  - `cargo test -p backtest-worker final_kline_refinement_uses_full_symbol_history_not_screening_sample -- --nocapture`
  - `cargo test -p backtest-worker portfolio_candidates_prefer_full_output_curve_over_summary_preview -- --nocapture`
  - `cargo test -p backtest-engine weighted_portfolio_aligns_member_equity_by_timestamp_not_index -- --nocapture`
  - `cargo test -p backtest-worker long_short_uses_configured_risk_profile_drawdown_limits -- --nocapture`
- Known failing verification:
  - `node --test tests/verification/backtest_worker_contract.test.mjs`
  - Failing assertion expects `run_candidate_kline_screening(&overridden, context)` but current code uses `run_candidate_kline_screening(&overridden_candidate, market_context)` after applying overrides and execution model. Confirm behavior, then update the static contract or code consistently.

## Task 1: Reconfirm And Clean FlyingKid Tasks

**Files:** Runtime DB/API only.

- [ ] **Step 1: Query FlyingKid tasks before deletion**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -P pager=off -c \
"SELECT task_id, owner, status, created_at, updated_at, config->>'risk_profile' AS risk, summary->>'stage' AS stage, jsonb_array_length(COALESCE(summary->'portfolio_top3','[]'::jsonb)) AS top3_count FROM backtest_tasks WHERE owner='flyingkid2022@outlook.com' ORDER BY created_at ASC;"
```

Expected currently: exactly the three protected task IDs above.

- [ ] **Step 2: Export protected task summaries**

Run one export per protected task:

```bash
mkdir -p /tmp/grid-binance-flyingkid-keep
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -P pager=off -A -t -c \
"SELECT jsonb_pretty(to_jsonb(t)) FROM backtest_tasks t WHERE task_id IN ('fk-18-balanced-wide-seed127-20260530','fk-18-bal-v2-seed53-20260601','fk-18-agg-v2-seed173-20260601');" \
  > /tmp/grid-binance-flyingkid-keep/protected_tasks.json
```

- [ ] **Step 3: Delete only extra terminal FlyingKid tasks**

If Step 1 shows additional `failed`, `cancelled`, or unwanted later `succeeded` FlyingKid tasks, delete only those task IDs. Prefer the API route when authenticated as FlyingKid; otherwise use SQL only after printing the delete list.

Safe SQL shape:

```sql
DELETE FROM backtest_tasks
WHERE owner = 'flyingkid2022@outlook.com'
  AND status NOT IN ('queued','running','paused')
  AND task_id NOT IN (
    'fk-18-balanced-wide-seed127-20260530',
    'fk-18-bal-v2-seed53-20260601',
    'fk-18-agg-v2-seed173-20260601'
  );
```

Do not delete active tasks directly; cancel/pause policy must be explicit first.

- [ ] **Step 4: Verify exactly three remain**

Rerun Step 1 and confirm only protected task IDs remain.

## Task 2: Fix Frontend 502

**Files:** Prefer runtime fix first. Modify source only if build/runtime proves a code/config bug.

- [ ] **Step 1: Confirm root cause**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml ps
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml logs --tail=120 nginx
```

Expected current symptom: `web` missing and nginx reports `web could not be resolved`.

- [ ] **Step 2: Start or rebuild web**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml up -d --build web
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml up -d nginx
```

- [ ] **Step 3: Verify service and public route**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml ps web nginx
curl -sS -I http://127.0.0.1:8080/zh/app/backtest
curl -sS -I http://127.0.0.1:8080/api/healthz
```

Expected: no 502. If web build fails, capture the first real build/runtime error and fix that root cause.

## Task 3: Fix The Static Contract Drift

**Files:**
- Modify if behavior is correct: `tests/verification/backtest_worker_contract.test.mjs`
- Modify if behavior is wrong: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Confirm override flow**

Verify `refine_candidate_output()` applies `apply_task_overrides_to_candidate(...)`, then `enforce_task_execution_model(...)`, then calls both kline/trade refinement with the overridden candidate.

- [ ] **Step 2: Update the contract test only if runtime behavior is correct**

Current source uses:

```rust
run_candidate_kline_screening(&overridden_candidate, market_context)?
run_candidate_trade_refinement(&overridden_candidate, market_context)?
```

So the static test should accept `overridden_candidate` and `market_context`.

- [ ] **Step 3: Verify contracts**

Run:

```bash
node --test tests/verification/backtest_worker_contract.test.mjs tests/verification/backtest_proxy_routes_contract.test.mjs tests/verification/nginx_route_contract.test.mjs
```

Expected: all pass.

## Task 4: Conservative Search For >50% Annualized

**Files:** Runtime DB/API. Code changes only after tests show current search cannot generate candidates.

- [ ] **Step 1: Run conservative small smoke in FlyingKid**

Create one task first, not a large matrix:

```text
fk-18-conservative-atradx-smoke-seed211-20260605
```

Config requirements:

```json
{
  "owner": "flyingkid2022@outlook.com",
  "strategy_type": "martingale",
  "symbols": ["BTCUSDT","ETHUSDT","BNBUSDT","SOLUSDT","DOGEUSDT","XRPUSDT","ADAUSDT","ZECUSDT","DASHUSDT","NEARUSDT","BCHUSDT","LINKUSDT","AVAXUSDT","UNIUSDT","FILUSDT","DOTUSDT","AAVEUSDT","INJUSDT"],
  "risk_profile": "conservative",
  "direction_mode": "mixed_best",
  "market": "usd_m_futures",
  "margin_mode": "isolated",
  "search_mode": "profit_optimized_v2",
  "random_candidates": 256,
  "per_symbol_top_n": 30,
  "portfolio_top_n": 3,
  "fee_bps": 4.5,
  "slippage_bps": 2.0,
  "random_seed": 211
}
```

- [ ] **Step 2: Poll and summarize**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml exec -T postgres psql -U postgres -d grid_binance -P pager=off -c \
"WITH p AS (SELECT task_id, ord, elem FROM backtest_tasks, jsonb_array_elements(summary->'portfolio_top3') WITH ORDINALITY x(elem, ord) WHERE owner='flyingkid2022@outlook.com' AND task_id LIKE 'fk-18-conservative-%20260605' AND status='succeeded') SELECT task_id, ord rank, round((elem->>'annualized_return_pct')::numeric,2) ann, round((elem->>'max_drawdown_pct')::numeric,2) dd, round((elem->>'return_pct')::numeric,2) ret, elem->>'member_count' members, jsonb_array_length(COALESCE(elem->'equity_curve','[]'::jsonb)) eq_points FROM p ORDER BY ann DESC NULLS LAST LIMIT 12;"
```

Target: annualized `>50%` with portfolio max DD within conservative limit. If not achieved, report the best honest result and bottleneck.

- [ ] **Step 3: Expand seeds only after smoke succeeds**

Use seeds `211`, `307`, `409`, `521`, `677`, but keep at most two tasks active at once. Stop early if a valid conservative portfolio exceeds 50% annualized and curve checks pass.

## Task 5: Balanced And Aggressive Improvement Search

**Files:** Runtime DB/API.

- [ ] **Step 1: Use existing baselines**

Balanced baseline to beat: `fk-18-bal-v2-seed53-20260601` Top1 `65.52% / 19.32%`.

Aggressive baseline to beat: `fk-18-agg-v2-seed173-20260601` Top1 `77.00% / 28.03%`.

- [ ] **Step 2: Run focused new seeds in FlyingKid**

Balanced task IDs:

```text
fk-18-balanced-atradx-seed307-20260605
fk-18-balanced-atradx-seed521-20260605
```

Aggressive task IDs:

```text
fk-18-aggressive-atradx-seed307-20260605
fk-18-aggressive-atradx-seed521-20260605
```

Use `profit_optimized_v2`, `mixed_best`, full 18-symbol universe, `fee_bps=4.5`, `slippage_bps=2.0`, `per_symbol_top_n=40`, `portfolio_top_n=3`, `random_candidates=512`.

- [ ] **Step 3: Rank by strict constraints**

For balanced, only compare portfolios with DD `<=20%`. For aggressive, only compare DD `<=30%`. Rank by annualized return, then lower drawdown, then smoother curve and lower stop-loss frequency.

## Task 6: Final Verification And Handoff

- [ ] **Step 1: Verify build/tests**

Run:

```bash
cargo check -p backtest-worker
cargo test -p backtest-engine staged_tests -- --nocapture
cargo test -p backtest-engine walk_forward -- --nocapture
node --test tests/verification/backtest_worker_contract.test.mjs tests/verification/backtest_proxy_routes_contract.test.mjs tests/verification/nginx_route_contract.test.mjs
```

- [ ] **Step 2: Verify web**

Run:

```bash
curl -sS -I http://127.0.0.1:8080/zh/app/backtest
```

Expected: not 502.

- [ ] **Step 3: Report final table**

Report:

```text
Task ID | Risk | Seed | Annualized % | Max DD % | Return % | Members | Eq Points | WFE verdict | Notes
```

- [ ] **Step 4: Git commit if code/docs changed**

Commit message must include one of: problem description, reproduction path, or fix approach. Example:

```bash
git add <changed-files>
git commit -m "fix: 修复思路 恢复 web 服务并同步回测契约"
```

