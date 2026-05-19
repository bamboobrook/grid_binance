# Martingale Long/Short Smoke Follow-up Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the post-deploy martingale smoke gaps where `long_short` backtest tasks complete but persist only long-only candidates and incomplete portfolio details.

**Architecture:** Preserve the existing staged-search + worker pipeline. Add regression tests that fail on the observed Docker smoke evidence, then fix candidate direction preservation, summary/config enrichment, and API/UI visibility without changing unrelated grid strategy behavior.

**Tech Stack:** Rust workspace (`api-server`, `backtest-worker`, `backtest-engine`), PostgreSQL persisted smoke data, Next.js web backtest console, Node verification tests.

---

## Verified Failure Evidence

Observed after merging Claude branch into `main` and deploying Docker services on 2026-05-19:

- Service health passed: `grid-binance-api-server-1`, `grid-binance-backtest-worker-1`, `grid-binance-web-1`, `grid-binance-nginx-1` were up; `/nginx-health` returned `ok`; `/api/healthz` returned `service_up{service="api-server"} 1`.
- Real smoke task created with payload:
  - `strategy_type=martingale`
  - `symbols=["BTCUSDT","ETHUSDT"]`
  - `direction=long_short`
  - `direction_mode=long_short`
  - `risk_profile=balanced`
  - `search_space.leverage=[2]`
  - `search_space.long_short_weight_pct=[[60,40],[50,50]]`
- Task IDs that completed but showed the same issue:
  - `bt_1779157687469135363`
  - `bt_1779157787246130542`
  - `bt_1779157857673905900`
  - `bt_1779157888380531730`
- Database evidence for latest task:
  ```sql
  select count(*) as candidates,
         count(*) filter (where summary->>'direction' in ('long_short','LongShort','long+short')) as long_short_candidates,
         count(*) filter (where summary->>'annualized_return_pct' is not null) as annualized_candidates,
         count(*) filter (where jsonb_array_length(coalesce(summary->'equity_curve','[]'::jsonb))>0
                           and jsonb_array_length(coalesce(summary->'drawdown_curve','[]'::jsonb))>0) as curve_candidates
  from backtest_candidate_summaries
  where task_id='bt_1779157888380531730';
  ```
  Actual result: `candidates=17`, `long_short_candidates=0`, `annualized_candidates=17`, `curve_candidates=17`.
- Candidate rows show `summary.direction = long`, `config.direction_mode = long_only`, and missing visible leverage fields:
  ```sql
  select rank, config->>'direction_mode', summary->>'symbol', summary->>'direction',
         summary->>'annualized_return_pct', summary->>'max_drawdown_pct',
         config->>'leverage', summary->>'leverage'
  from backtest_candidate_summaries
  where task_id='bt_1779157857673905900'
  order by rank
  limit 10;
  ```
  Actual direction mode is `long_only` for all top rows.

## Root Cause Hypothesis To Verify

Do not assume the fix before tests. Start by proving or disproving these likely causes:

1. `WorkerTaskConfig` or `search_space_from_staged()` is normalizing `direction_mode=long_short` into `long`/`long_only` before calling `intelligent_search`.
2. `apply_task_overrides_to_candidate()` may overwrite generated `LongAndShort` candidates with a single-direction template.
3. `select_refinement_candidates_with_drawdown_metadata()` may sort top long candidates ahead of all long_short candidates, but for a user-requested `long_short` task it must not silently return only one-sided candidates.
4. Candidate persistence stores full config but visible summary omits `recommended_leverage`/`max_leverage_used` or stores leverage only inside nested strategy, making API/UI look like leverage was ignored.
5. Portfolio details may exist in artifacts but API/UI needs a clear details route/shape for true multi-strategy portfolios, not just the top single candidate.

## File Map

- Modify: `apps/backtest-worker/src/main.rs`
  - Direction-mode parsing and staged-search execution.
  - Candidate summary enrichment.
  - Portfolio top3 summary/detail payload.
  - Unit tests for worker contracts.
- Modify if needed: `apps/backtest-engine/src/search.rs`
  - Ensure `generate_staged_candidates_for_symbol(..., "long_short", ...)` emits only `MartingaleDirectionMode::LongAndShort` candidates when requested.
  - Ensure leverage and isolated futures semantics are represented in generated configs.
- Modify if needed: `apps/backtest-engine/src/martingale/kline_engine.rs`
  - Only if leverage accounting is still wrong after tests; do not change if existing tests already prove principal/margin math.
- Modify if needed: `apps/api-server/src/services/backtest_service.rs`
  - Candidate/portfolio list/detail API response should expose persisted summary and artifact pointers without losing fields.
- Modify: `apps/api-server/tests/martingale_backtest_flow.rs`
  - Integration tests for long_short task contracts.
- Modify: `tests/verification/backtest_worker_contract.test.mjs`
  - Source contract for long_short preservation and candidate summary fields.
- Modify if UI shape requires it: `apps/web/components/backtest/backtest-console.tsx`, `apps/web/components/backtest/backtest-result-table.tsx`, `apps/web/components/backtest/portfolio-candidate-review.tsx`
  - Display portfolio member details, leverage, annualized return, equity/drawdown curves, and trades.
- Modify if UI contract changes: `tests/verification/backtest_console_contract.test.mjs`

---

### Task 1: Add failing worker regression for `long_short` preservation

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add a unit test that currently fails on the observed bug**

Add this test inside the existing `#[cfg(test)] mod tests` in `apps/backtest-worker/src/main.rs`. Adapt helper names only if the local module already has equivalent constructors; keep the assertions exact.

```rust
#[test]
fn long_short_task_keeps_long_short_candidates_after_refinement() {
    let config = WorkerTaskConfig {
        strategy_type: "martingale".to_owned(),
        symbols: vec!["BTCUSDT".to_owned()],
        direction_mode: Some("long_short".to_owned()),
        risk_profile: "balanced".to_owned(),
        random_seed: 7,
        random_candidates: 8,
        intelligent_rounds: 1,
        per_symbol_top_n: 10,
        top_n: 10,
        portfolio_top_n: 3,
        search_mode: Some("staged".to_owned()),
        market: Some("usd_m_futures".to_owned()),
        margin_mode: Some("isolated".to_owned()),
        search_space: serde_json::json!({
            "leverage": [2],
            "spacing_bps": [120],
            "order_multiplier": [1.25],
            "max_legs": [3],
            "take_profit_bps": [60],
            "tail_stop_bps": [2000],
            "long_short_weight_pct": [[60, 40], [50, 50]]
        }),
        ..WorkerTaskConfig::default_for_tests()
    };

    let staged = backtest_engine::search::StagedMartingaleSearchSpace::for_profile(
        &config.risk_profile,
        config.direction_mode.as_deref().unwrap(),
    );
    let search_space = search_space_from_staged(&staged, "BTCUSDT", &config);

    assert_eq!(search_space.directions, vec!["long_short".to_owned()]);

    let candidates = backtest_engine::search::generate_staged_candidates_for_symbol(
        "BTCUSDT",
        "long_short",
        &staged,
        20,
    )
    .expect("long_short candidates should generate");

    assert!(!candidates.is_empty());
    assert!(
        candidates.iter().all(|candidate| candidate.config.direction_mode
            == shared_domain::martingale::MartingaleDirectionMode::LongAndShort),
        "user-requested long_short search must not degrade to long_only candidates"
    );
    assert!(
        candidates.iter().all(|candidate| {
            let has_long = candidate.config.strategies.iter().any(|strategy| {
                strategy.direction == shared_domain::martingale::MartingaleDirection::Long
            });
            let has_short = candidate.config.strategies.iter().any(|strategy| {
                strategy.direction == shared_domain::martingale::MartingaleDirection::Short
            });
            has_long && has_short
        }),
        "each long_short portfolio candidate must include both long and short strategy legs"
    );
}
```

- [ ] **Step 2: Run the test and confirm it fails before implementation**

Run:

```bash
cargo test -p backtest-worker long_short_task_keeps_long_short_candidates_after_refinement -- --nocapture
```

Expected before fixing: FAIL showing either `search_space.directions` is not `long_short`, candidates are `LongOnly`, or candidates are missing one side.

- [ ] **Step 3: Fix direction-mode propagation**

In `apps/backtest-worker/src/main.rs`, inspect and fix these functions so `direction_mode=long_short` remains `long_short` through staged search:

- `search_space_from_staged()`
- `directions_from_mode()`
- `apply_task_overrides_to_candidate()`
- `run_profit_first_staged_search()`

Required behavior:

```rust
// Pseudocode contract, implement using existing local types/patterns.
match task.direction_mode.as_deref().unwrap_or("long") {
    "long_short" | "long_and_short" => vec!["long_short".to_owned()],
    "short" | "short_only" => vec!["short".to_owned()],
    _ => vec!["long".to_owned()],
}
```

If `apply_task_overrides_to_candidate()` applies market/margin/leverage, it must preserve `MartingalePortfolioConfig.direction_mode == LongAndShort` and must not replace a two-strategy portfolio with the first strategy only.

- [ ] **Step 4: Re-run the focused test**

Run:

```bash
cargo test -p backtest-worker long_short_task_keeps_long_short_candidates_after_refinement -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-worker/src/main.rs
git commit -m "fix: 问题描述 保留马丁多空双向候选方向"
```

---

### Task 2: Enrich candidate summaries with leverage, direction mode, and annualized aliases

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] **Step 1: Add source contract assertions**

Extend `tests/verification/backtest_worker_contract.test.mjs` with assertions that candidate summaries persist all UI/API-required fields:

```js
test("worker persists martingale candidate summary with direction mode and leverage fields", () => {
  const source = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(source, /"direction_mode"/);
  assert.match(source, /"recommended_leverage"/);
  assert.match(source, /"max_leverage_used"/);
  assert.match(source, /"annualized_return_pct"/);
  assert.match(source, /"return_pct"/);
  assert.match(source, /"total_return_pct"/);
  assert.match(source, /"equity_curve"/);
  assert.match(source, /"drawdown_curve"/);
  assert.match(source, /"trades_preview"/);
});
```

- [ ] **Step 2: Run contract and confirm current gap if any**

Run:

```bash
node tests/verification/backtest_worker_contract.test.mjs
```

Expected before fixing: FAIL if `direction_mode`, `recommended_leverage`, or `return_pct` aliases are missing from worker persistence.

- [ ] **Step 3: Persist the fields in `save_candidates_and_artifacts()` and output enrichment**

In `apps/backtest-worker/src/main.rs`, ensure each candidate `summary` contains these top-level fields:

```json
{
  "symbol": "BTCUSDT",
  "direction": "long_short",
  "direction_mode": "long_short",
  "recommended_leverage": 2,
  "max_leverage_used": 2,
  "return_pct": 12.34,
  "total_return_pct": 12.34,
  "annualized_return_pct": 31.8,
  "max_drawdown_pct": 17.8,
  "equity_curve": [ ... sampled preview ... ],
  "drawdown_curve": [ ... sampled preview ... ],
  "trades_preview": [ ... sampled preview ... ]
}
```

Implementation notes:

- `direction_mode` must come from `output.config.direction_mode`, not from only the first strategy.
- `direction` can remain human-readable, but for `LongAndShort` it must be exactly `long_short` for API/UI filtering.
- `recommended_leverage` should equal the highest strategy leverage inside the candidate config. Use the existing `output_leverage()` only if it handles multi-strategy configs; otherwise add a helper that scans all strategies.
- Keep preview caps: equity/drawdown `500`, trades `100`.
- Do not duplicate full artifacts into task summary.

- [ ] **Step 4: Re-run worker tests and contracts**

Run:

```bash
cargo test -p backtest-worker selected_outputs_include_ui_required_summary_fields -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: both PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-worker/src/main.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "fix: 修复思路 补齐马丁候选摘要字段"
```

---

### Task 3: Enforce portfolio combination semantics and details visibility

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify if needed: `apps/api-server/src/services/backtest_service.rs`
- Modify: `apps/api-server/tests/martingale_backtest_flow.rs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] **Step 1: Add regression assertions for true portfolio combinations**

Add or extend tests so `portfolio_top3` is not just a single best candidate. Required assertions:

```rust
assert!(portfolio["members"].as_array().unwrap().len() >= 2);
let total_weight: f64 = portfolio["members"].as_array().unwrap()
    .iter()
    .map(|member| member["weight_pct"].as_f64().unwrap_or(0.0))
    .sum();
assert!((total_weight - 100.0).abs() < 0.01);
assert!(portfolio["annualized_return_pct"].is_number() || portfolio["annualized_return_pct"].is_null());
assert!(portfolio["max_drawdown_pct"].is_number());
assert!(portfolio["equity_curve"].as_array().map(|v| !v.is_empty()).unwrap_or(false));
assert!(portfolio["drawdown_curve"].as_array().map(|v| !v.is_empty()).unwrap_or(false));
```

- [ ] **Step 2: Run the focused tests and confirm failure if portfolio detail is missing**

Run:

```bash
cargo test -p backtest-worker portfolio -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected before fixing: FAIL if portfolio members/details/curves are missing or member count is 1.

- [ ] **Step 3: Fix portfolio output shape**

In `apps/backtest-worker/src/main.rs`, update `build_portfolio_top3()` serialization so each portfolio contains:

```json
{
  "rank": 1,
  "score": 88.5,
  "return_pct": 25.0,
  "annualized_return_pct": 54.0,
  "max_drawdown_pct": 18.0,
  "return_drawdown_ratio": 3.0,
  "member_count": 3,
  "members": [
    {
      "candidate_id": "...",
      "symbol": "BTCUSDT",
      "direction": "long_short",
      "direction_mode": "long_short",
      "weight_pct": 40.0,
      "recommended_leverage": 2,
      "return_pct": 20.0,
      "annualized_return_pct": 45.0,
      "max_drawdown_pct": 16.0
    }
  ],
  "equity_curve": [ ... sampled preview ... ],
  "drawdown_curve": [ ... sampled preview ... ]
}
```

Do not store massive full arrays in `backtest_tasks.summary`; keep sampled previews and put full data in artifact JSONL/JSON as currently intended.

- [ ] **Step 4: Re-run tests**

Run:

```bash
cargo test -p backtest-worker portfolio -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-worker/src/main.rs apps/api-server/src/services/backtest_service.rs apps/api-server/tests/martingale_backtest_flow.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "fix: 问题描述 补齐马丁组合详情语义"
```

---

### Task 4: Add API integration regression for long_short task results

**Files:**
- Modify: `apps/api-server/tests/martingale_backtest_flow.rs`
- Modify if needed: `apps/api-server/src/services/backtest_service.rs`

- [ ] **Step 1: Add integration test for candidate detail contract**

Add a test using existing support helpers. It may seed records directly through repository helpers if worker execution is too slow for API tests. The test must assert the API returns persisted fields without stripping them:

```rust
#[tokio::test]
async fn martingale_long_short_candidate_detail_exposes_complete_summary() {
    // Arrange: create owner, task, and a candidate summary with direction_mode=long_short,
    // annualized_return_pct, max_leverage_used, equity_curve, drawdown_curve, trades_preview.
    // Act: GET /backtest/tasks/{task_id}/candidates and GET /backtest/candidates/{candidate_id}.
    // Assert: both responses include the same fields and arrays are non-empty.
}
```

Required response assertions:

```rust
assert_eq!(summary["direction_mode"], "long_short");
assert_eq!(summary["direction"], "long_short");
assert!(summary["annualized_return_pct"].is_number());
assert_eq!(summary["recommended_leverage"], 2);
assert!(summary["equity_curve"].as_array().unwrap().len() > 0);
assert!(summary["drawdown_curve"].as_array().unwrap().len() > 0);
assert!(summary["trades_preview"].as_array().unwrap().len() > 0);
```

- [ ] **Step 2: Run and confirm failure if API strips fields**

Run:

```bash
cargo test -p api-server martingale_long_short_candidate_detail_exposes_complete_summary -- --nocapture
```

Expected before fixing: FAIL if service response strips or renames fields.

- [ ] **Step 3: Fix API response if needed**

If the test fails, update `apps/api-server/src/services/backtest_service.rs` so list/detail candidate endpoints return `BacktestCandidateRecord.summary` and `config` intact. Do not add web-only transformations in the API service.

- [ ] **Step 4: Re-run API test**

Run:

```bash
cargo test -p api-server martingale_long_short_candidate_detail_exposes_complete_summary -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/api-server/src/services/backtest_service.rs apps/api-server/tests/martingale_backtest_flow.rs
git commit -m "test: 问题描述 锁定马丁多空候选API契约"
```

---

### Task 5: Verify frontend result/detail usability contract

**Files:**
- Modify if needed: `apps/web/components/backtest/backtest-console.tsx`
- Modify if needed: `apps/web/components/backtest/backtest-result-table.tsx`
- Modify if needed: `apps/web/components/backtest/portfolio-candidate-review.tsx`
- Modify: `tests/verification/backtest_console_contract.test.mjs`

- [ ] **Step 1: Add/extend frontend contract assertions**

Ensure `tests/verification/backtest_console_contract.test.mjs` checks:

```js
assert.match(tableSource, /annualized_return_pct|annualizedReturnPct/);
assert.match(tableSource, /recommended_leverage|max_leverage_used|maxLeverageUsed/);
assert.match(tableSource, /equity_curve|equityCurve/);
assert.match(tableSource, /drawdown_curve|drawdownCurve/);
assert.match(tableSource, /trades_preview|tradesPreview/);
assert.match(reviewSource, /member_count|members\.length/);
assert.match(reviewSource, /weight_pct|weightPct/);
```

- [ ] **Step 2: Fix UI mapping only if contract fails**

If current UI cannot show fields, update the relevant components so users can see:

- per-candidate direction and direction mode;
- leverage used/recommended leverage;
- annualized return;
- equity curve and drawdown curve;
- trade preview/details;
- portfolio top3 details with multiple members and weights.

- [ ] **Step 3: Build web**

Run:

```bash
node tests/verification/backtest_console_contract.test.mjs
pnpm --filter web exec next build --webpack
```

Expected: both PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/web/components/backtest tests/verification/backtest_console_contract.test.mjs
git commit -m "fix: 修复思路 展示马丁回测候选与组合详情"
```

---

### Task 6: Full verification and Docker smoke handoff

**Files:**
- No code changes unless a test failure requires returning to earlier tasks.

- [ ] **Step 1: Run focused backend tests**

Run:

```bash
cargo test -p backtest-engine -- --nocapture
cargo test -p backtest-worker -- --nocapture
cargo test -p api-server martingale -- --nocapture
```

Expected: all PASS.

- [ ] **Step 2: Run verification contracts**

Run:

```bash
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
pnpm --filter web exec next build --webpack
```

Expected: all PASS.

- [ ] **Step 3: Build and restart Docker services**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  build api-server backtest-worker web

docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  up -d --no-deps api-server backtest-worker web
```

Expected: build exits `0`; containers become healthy.

- [ ] **Step 4: Run the exact Docker smoke that previously failed**

Use this SQL after creating and waiting for a real `long_short` task:

```sql
select count(*) as candidates,
       count(*) filter (where summary->>'direction' in ('long_short','LongShort','long+short')) as long_short_candidates,
       count(*) filter (where summary->>'annualized_return_pct' is not null) as annualized_candidates,
       count(*) filter (where jsonb_array_length(coalesce(summary->'equity_curve','[]'::jsonb))>0
                         and jsonb_array_length(coalesce(summary->'drawdown_curve','[]'::jsonb))>0) as curve_candidates,
       count(*) filter (where (summary->>'recommended_leverage')::numeric >= 2
                         or (summary->>'max_leverage_used')::numeric >= 2) as leverage_candidates
from backtest_candidate_summaries
where task_id='<NEW_TASK_ID>';
```

Expected:

- `candidates >= 10`
- `long_short_candidates >= 1` for a `direction_mode=long_short` request
- `annualized_candidates = candidates`
- `curve_candidates = candidates`
- `leverage_candidates >= 1`

Also verify portfolio details:

```sql
select jsonb_array_length(summary->'portfolio_top3') as portfolio_count,
       jsonb_array_length((summary->'portfolio_top3'->0)->'members') as first_member_count,
       ((summary->'portfolio_top3'->0)->>'annualized_return_pct') as first_annualized,
       jsonb_array_length(coalesce((summary->'portfolio_top3'->0)->'equity_curve','[]'::jsonb)) as first_equity_len,
       jsonb_array_length(coalesce((summary->'portfolio_top3'->0)->'drawdown_curve','[]'::jsonb)) as first_drawdown_len
from backtest_tasks
where task_id='<NEW_TASK_ID>';
```

Expected:

- `portfolio_count >= 1`
- `first_member_count >= 2`
- `first_annualized` is not empty/null when candidate curves have valid timestamps
- `first_equity_len > 0`
- `first_drawdown_len > 0`

- [ ] **Step 5: Final commit if verification-only docs changed**

If only code commits from previous tasks exist, no extra commit is needed. If this plan or verification docs were updated during Claude execution, commit them:

```bash
git add docs/superpowers/plans/2026-05-19-martingale-long-short-smoke-followup-plan.md
git commit -m "docs: 复现路径 记录马丁多空烟测修复计划"
```

---

## Do Not Do

- Do not silently convert `long_short` requests to `long_only` even if long-only scores rank higher.
- Do not store full multi-year full-resolution curves in `backtest_tasks.summary`; keep sampled previews and full artifacts separate.
- Do not remove annualized-return calculation or curve/trade previews to pass size limits.
- Do not change unrelated grid strategy save/start logic.
- Do not touch unrelated host port `3000`; grid frontend is exposed via host `8080`, web container internally uses `3000`.

## Reviewer Handoff Checklist

When Claude finishes, report:

- Commit hashes for each task.
- Output of all commands in Task 6.
- New Docker smoke task id.
- SQL counts proving `long_short_candidates >= 1`, annualized/curves/leverage present, and portfolio member count >= 2.
