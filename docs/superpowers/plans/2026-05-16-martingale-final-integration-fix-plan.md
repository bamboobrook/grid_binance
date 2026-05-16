# Martingale Final Integration Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this focused integration fix task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the final three integration gaps before merge/deploy: staged worker must consume task overrides, Portfolio Top3 must reference real persisted candidates with complete UI data, and frontend console must actually render `portfolio_top3` from task summary.

**Architecture:** Keep the current profit-first staged worker path. Apply task overrides before screening/refinement, persist candidates first, then derive Portfolio Top3 rows from persisted candidate records or candidate summaries, and pass task summary through the frontend console into `BacktestResultTable`.

**Tech Stack:** Rust `backtest-worker` / `shared-db`, Next.js backtest console, Node verification tests, Cargo tests.

---

## Verified Problems

Final review of `385d29c..93c6f71` found three Important issues:

1. `run_profit_first_staged_search()` does not apply task `indicators`, `entry_triggers`, or most `search_space` overrides on the real staged path.
2. `portfolio_top3` summary uses pre-persistence search candidate IDs, not persisted DB candidate IDs, and misses fields needed by frontend (`symbol`, `trade_count`).
3. Frontend `BacktestConsole` does not pass task summary `portfolio_top3` into `BacktestResultTable`; `normalizeTask()` drops summary.

Do not merge/deploy until these are fixed and verified.

---

## Task 1: Apply Task Overrides in Staged Worker Path

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] **Step 1: Add contract assertions**

In `tests/verification/backtest_worker_contract.test.mjs`, add assertions ensuring staged candidates are overridden before execution:

```js
assert.match(worker, /apply_task_overrides_to_candidate\(candidate\.candidate\.clone\(\), task\)/);
assert.match(worker, /run_candidate_kline_screening\(&overridden, context\)/);
assert.match(worker, /run_candidate_trade_refinement\(&overridden_candidate, &market_context\)/);
```

Run:

```bash
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: FAIL before implementation if overrides are not applied in staged screening and refinement.

- [ ] **Step 2: Apply overrides inside `run_profit_first_staged_search()`**

In `apps/backtest-worker/src/main.rs`, in both coarse and fine `intelligent_search()` closures, replace direct screening of raw candidates with:

```rust
|candidate| {
    let overridden = apply_task_overrides_to_candidate(candidate.candidate.clone(), task);
    run_candidate_kline_screening(&overridden, context)
}
```

If closure input is a `SearchCandidate` rather than wrapper, use:

```rust
|candidate| {
    let overridden = apply_task_overrides_to_candidate(candidate.clone(), task);
    run_candidate_kline_screening(&overridden, context)
}
```

Use whichever type matches the existing function signature. The acceptance criterion is that indicators and entry triggers from `task.martingale_template` are actually applied before kline scoring.

- [ ] **Step 3: Apply overrides before trade refinement**

In `process_task()`, before `run_candidate_trade_refinement`, create:

```rust
let overridden_candidate = apply_task_overrides_to_candidate(evaluated.candidate.clone(), &task.config);
let refined = run_candidate_trade_refinement(&overridden_candidate, &market_context)?;
```

Use `overridden_candidate` for serialized `config`, artifact source, and downstream `CandidateOutput` so stored candidates match the executed config.

- [ ] **Step 4: Ensure custom search space is consumed by staged builder**

In `search_space_from_staged()`, make sure payload search-space arrays override staged defaults for these keys when present:

```rust
spacing_bps
order_multiplier
max_legs
take_profit_bps
leverage
```

Use existing helpers such as `search_space_u32()` and `search_space_decimal()`. For example:

```rust
step_bps: search_space_u32(task, "spacing_bps").unwrap_or_else(|| staged.spacing_bps.clone()),
leverage: search_space_u32(task, "leverage").unwrap_or_else(|| staged.leverage.clone()),
max_legs: search_space_u32(task, "max_legs").unwrap_or_else(|| staged.max_legs.clone()),
```

For decimal arrays:

```rust
multiplier: search_space_decimal(task, "order_multiplier")
    .unwrap_or_else(|| staged.order_multiplier.iter().filter_map(|m| Decimal::from_f64_retain(*m)).collect()),
```

- [ ] **Step 5: Add Rust regression test**

Add/update a worker test proving staged search space consumes payload overrides:

```rust
#[test]
fn staged_search_space_uses_task_search_space_overrides() {
    let mut config = WorkerTaskConfig::default();
    config.symbols = vec!["BTCUSDT".to_owned()];
    config.martingale_template = Some(json!({
        "search_space": {
            "spacing_bps": [77],
            "order_multiplier": ["1.7"],
            "max_legs": [4],
            "take_profit_bps": [88],
            "leverage": [6]
        }
    }));
    let staged = StagedMartingaleSearchSpace::for_profile("balanced", "long_only");
    let space = search_space_from_staged(&staged, "BTCUSDT", &config);
    assert_eq!(space.step_bps, vec![77]);
    assert_eq!(space.max_legs, vec![4]);
    assert_eq!(space.take_profit_bps, vec![88]);
    assert_eq!(space.leverage, vec![6]);
    assert_eq!(space.multiplier, vec![Decimal::new(17, 1)]);
}
```

- [ ] **Step 6: Verify and commit**

Run:

```bash
cargo test -p backtest-worker -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Commit:

```bash
git add apps/backtest-worker/src/main.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "fix: 修复思路 让分阶段回测消费任务覆盖参数"
```

---

## Task 2: Make Portfolio Top3 Reference Persisted Candidate IDs

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `crates/shared-db/src/backtest.rs` if repository return shape is needed
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] **Step 1: Change save flow to get persisted candidates**

Currently `process_task()` builds `portfolio_top3` before or independently of persisted DB candidate IDs. Change the flow so `save_candidates_and_artifacts()` returns the persisted candidate summaries or a mapping from `source_candidate_id` to persisted `candidate_id`.

Preferred shape in `TaskPoller::save_candidates_and_artifacts()`:

```rust
async fn save_candidates_and_artifacts(
    &self,
    task_id: &str,
    evaluated_count: usize,
    outputs: &[CandidateOutput],
) -> Result<Vec<shared_db::BacktestCandidateRecord>, String>
```

Return the records produced by repository save calls.

If repository currently returns a candidate record per save, collect those records and return them.

- [ ] **Step 2: Build Portfolio Top3 rows from persisted candidates**

After saving candidates, construct `portfolio_top3` rows using persisted candidate IDs and candidate summaries:

```rust
let portfolio_rows = portfolio_top3
    .top3
    .iter()
    .filter_map(|entry| {
        let persisted = persisted_candidates.iter().find(|candidate| {
            candidate.summary.get("source_candidate_id").and_then(Value::as_str)
                == Some(entry.candidate.candidate_id.as_str())
                || candidate.candidate_id == entry.candidate.candidate_id
        })?;
        Some(json!({
            "candidate_id": persisted.candidate_id,
            "source_candidate_id": entry.candidate.candidate_id,
            "symbol": persisted.summary.get("symbol").cloned().unwrap_or(Value::Null),
            "score": entry.score,
            "return_pct": entry.return_pct,
            "max_drawdown_pct": entry.max_drawdown_pct,
            "trade_count": persisted.summary.get("trade_count").cloned().unwrap_or(Value::Null),
        }))
    })
    .collect::<Vec<_>>();
```

If `trade_count` is not currently in candidate summary, add it in `select_top_outputs_per_symbol()` summary patch:

```rust
"trade_count": output.trade_count,
```

- [ ] **Step 3: Persist complete Top3 summary**

Write `portfolio_top3` to task summary using `portfolio_rows`, not raw search IDs. Ensure every row has:

```json
candidate_id
source_candidate_id
symbol
score
return_pct
max_drawdown_pct
trade_count
```

- [ ] **Step 4: Strengthen contract test**

In `tests/verification/backtest_worker_contract.test.mjs`, add:

```js
assert.match(worker, /source_candidate_id/);
assert.match(worker, /"symbol"/);
assert.match(worker, /"trade_count"/);
assert.match(worker, /persisted_candidates/);
```

- [ ] **Step 5: Verify and commit**

Run:

```bash
cargo test -p backtest-worker -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Commit:

```bash
git add apps/backtest-worker/src/main.rs crates/shared-db/src/backtest.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "fix: 修复思路 让组合Top3关联真实候选ID"
```

---

## Task 3: Pass Portfolio Top3 Summary Into Frontend Result Table

**Files:**
- Modify: `apps/web/components/backtest/backtest-console.tsx`
- Modify: `apps/web/components/backtest/backtest-result-table.tsx` if type shape needs adjustment
- Modify: `tests/verification/martingale_backtest_rebuild_contract.test.mjs`
- Modify: `tests/verification/backtest_console_contract.test.mjs`

- [ ] **Step 1: Preserve task summary in normalized task**

In `backtest-console.tsx`, extend the task type to include summary:

```ts
summary?: Record<string, unknown> | null;
```

In `normalizeTask()`, preserve summary:

```ts
summary: isRecord(value.summary) ? value.summary : null,
```

If there is no `isRecord` helper, add:

```ts
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
```

- [ ] **Step 2: Extract portfolio Top3 from selected task**

In `BacktestConsole`, derive:

```ts
const portfolioTop3 = Array.isArray(selectedTask?.summary?.portfolio_top3)
  ? selectedTask.summary.portfolio_top3
  : [];
```

If TypeScript dislikes indexing, use a helper:

```ts
function portfolioTop3FromTask(task: BacktestTask | null): PortfolioTop3Row[] {
  const rows = task?.summary?.portfolio_top3;
  return Array.isArray(rows) ? rows.map(normalizePortfolioTop3Row) : [];
}
```

- [ ] **Step 3: Pass prop into result table**

Update the `BacktestResultTable` call:

```tsx
<BacktestResultTable
  ...
  portfolioTop3={portfolioTop3FromTask(selectedTask)}
/>
```

- [ ] **Step 4: Strengthen frontend contract tests**

In `tests/verification/backtest_console_contract.test.mjs`, add assertions:

```js
assert.match(consoleSource, /summary:/);
assert.match(consoleSource, /portfolioTop3FromTask/);
assert.match(consoleSource, /portfolioTop3=\{portfolioTop3FromTask\(selectedTask\)\}/);
```

In `tests/verification/martingale_backtest_rebuild_contract.test.mjs`, assert `BacktestResultTable` supports complete Top3 row fields:

```js
assert.match(tableSource, /trade_count|tradeCount/i);
assert.match(tableSource, /source_candidate_id|sourceCandidateId/i);
```

- [ ] **Step 5: Verify and commit**

Run:

```bash
node tests/verification/martingale_backtest_rebuild_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
pnpm --filter web exec next build --webpack
```

Commit:

```bash
git add apps/web/components/backtest/backtest-console.tsx apps/web/components/backtest/backtest-result-table.tsx tests/verification/martingale_backtest_rebuild_contract.test.mjs tests/verification/backtest_console_contract.test.mjs
git commit -m "fix: 修复思路 接通组合Top3前端展示链路"
```

---

## Task 4: Final Verification Gate

**Files:**
- No source change unless verification exposes a defect.

- [ ] **Step 1: Run full verification**

```bash
cargo test -p backtest-engine -- --nocapture
cargo test -p backtest-worker -- --nocapture
cargo test -p api-server martingale_auto_search -- --nocapture
node tests/verification/martingale_backtest_rebuild_contract.test.mjs
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
pnpm --filter web exec next build --webpack
```

Expected: all pass.

- [ ] **Step 2: Check clean status**

```bash
git status --short --branch
```

Expected: clean working tree.

---

## Handoff Rule

After completing this plan, ask Codex to re-review before merge/deploy. Do not merge or deploy until Codex confirms no Critical/Important issues remain.
