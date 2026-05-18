# Martingale Claude Result Completeness Review Fix Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the remaining gaps in Claude's 2026-05-18 implementation before merge: true portfolio semantics, usable portfolio details, correct Top10/UI fields, long_short parameter weights, and warning-free verification.

**Architecture:** Keep Claude's current branch `feature/full-v1` as the base. Add stricter behavior tests that fail against the current implementation, then patch engine/worker/web contracts so the produced results match `docs/superpowers/specs/2026-05-18-martingale-backtest-result-completeness-fix-design.md`.

**Tech Stack:** Rust (`backtest-engine`, `backtest-worker`), Next.js/TypeScript web components, Node contract tests.

---

## Review Findings

Do not merge current `feature/full-v1` yet. Independent review found these concrete gaps:

1. `apps/backtest-engine/src/portfolio_search.rs` now combines multiple candidates, but only uses equal weights and ignores optimization/allocation search. This is weaker than the spec requirement: total capital must be allocated by explicit member weights and ranked by true portfolio curve metrics.
2. `build_weighted_portfolio()` rejects same-symbol multi-strategy combinations because it requires unique symbols. The user explicitly wants same symbol multiple strategies to be combinable, e.g. BTC 0.5% and BTC 1% can both be members.
3. `combine_equity_curves()` sums raw candidate equity with equal scalar weight. It does not normalize each member by `planned_margin_quote` and allocated capital as required by the spec.
4. `WeightedPortfolio` has no `trades`/`trades_preview`, so portfolio details cannot show portfolio trade details even though the spec requires combined trade details.
5. Worker serializes `portfolio_id` as `portfolio-{member_count}`, which can duplicate IDs across Top3. It should be stable and unique, e.g. `portfolio-{rank}-{hash}`.
6. Worker computes `portfolio_top3` from already display-truncated `outputs` after `select_top_outputs_per_symbol(...)`. This may starve portfolio search. It must use a larger eligible pool, while still displaying Top10.
7. Frontend `portfolioTop3FromTask()` still reads old single-candidate fields (`candidate_id`, `source_candidate_id`, `symbol`) and drops `members`, `allocation_pct`, `equity_curve`, `drawdown_curve`, and portfolio metrics. So current UI cannot reliably view true combination details.
8. `candidateRow()` returns keys `annualized_return_pct` and `max_leverage_used`, but columns are named `annualized` and no leverage column exists. Annualized may not display in the table.
9. `build_long_short_candidate()` receives `_long_weight_pct` and `_short_weight_pct` but ignores them. At minimum, persist these weights in strategy risk metadata/summary legs so result detail shows long/short allocation. Better: use them in sizing or portfolio leg weighting if supported by the domain model.
10. `cargo test -p backtest-worker` passes but emits an `unused variable: persisted_candidates` warning. Clean this before merge.

---

## Task 1: Add Stricter Regression Tests

**Files:**
- Modify: `apps/backtest-engine/src/portfolio_search.rs`
- Modify: `tests/verification/backtest_console_contract.test.mjs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] **Step 1: Add portfolio normalization test**

In `apps/backtest-engine/src/portfolio_search.rs` tests, add a test that proves portfolio equity is normalized by `planned_margin_quote`, not raw equity values:

```rust
#[test]
fn portfolio_curve_normalizes_member_equity_by_planned_margin_and_allocation() {
    let mut a = evaluated_candidate_with_curve("a", "BTCUSDT", 100.0, vec![100.0, 120.0]);
    let mut b = evaluated_candidate_with_curve("b", "ETHUSDT", 200.0, vec![200.0, 220.0]);
    a.planned_margin_quote = 100.0;
    b.planned_margin_quote = 200.0;

    let portfolio = build_test_weighted_portfolio(vec![(&a, 70.0), (&b, 30.0)], 10_000.0).unwrap();

    assert_eq!(portfolio.member_count, 2);
    assert!((portfolio.members.iter().map(|m| m.allocation_pct).sum::<f64>() - 100.0).abs() < 0.000001);
    assert!((portfolio.equity_curve[0].equity_quote - 10_000.0).abs() < 0.000001);
    // A: 7000 * 120/100 = 8400; B: 3000 * 220/200 = 3300; total = 11700
    assert!((portfolio.equity_curve[1].equity_quote - 11_700.0).abs() < 0.000001);
}
```

If no public helper exists, add a private `#[cfg(test)]` helper that calls the same production combination code. Do not copy a separate algorithm into the test.

- [ ] **Step 2: Add same-symbol portfolio test**

Add:

```rust
#[test]
fn portfolio_allows_multiple_strategies_on_same_symbol_when_candidate_ids_differ() {
    let a = evaluated_candidate_with_curve("btc-fast", "BTCUSDT", 100.0, vec![100.0, 115.0]);
    let b = evaluated_candidate_with_curve("btc-slow", "BTCUSDT", 100.0, vec![100.0, 108.0]);
    let c = evaluated_candidate_with_curve("eth", "ETHUSDT", 100.0, vec![100.0, 104.0]);

    let artifact = build_portfolio_top3(&[a, b, c], 20.0);

    assert!(!artifact.top3.is_empty());
    assert!(artifact.top3.iter().any(|p| {
        let btc_members = p.members.iter().filter(|m| m.symbol == "BTCUSDT").count();
        btc_members >= 2
    }));
}
```

- [ ] **Step 3: Add portfolio trade detail test**

Add:

```rust
#[test]
fn portfolio_carries_combined_trade_preview() {
    let a = evaluated_candidate_with_trade("a", "BTCUSDT", 1000);
    let b = evaluated_candidate_with_trade("b", "ETHUSDT", 2000);

    let artifact = build_portfolio_top3(&[a, b], 20.0);
    let portfolio = artifact.top3.first().expect("portfolio");

    assert!(portfolio.trades_preview.len() >= 2 || portfolio.trades.len() >= 2);
}
```

Use the field name chosen in Task 2; keep one canonical field across Rust and JSON.

- [ ] **Step 4: Strengthen web contract**

In `tests/verification/backtest_console_contract.test.mjs`, assert `portfolioTop3FromTask()` reads real portfolio fields:

```js
assert.match(consoleSource, /members/);
assert.match(consoleSource, /allocation_pct/);
assert.match(consoleSource, /portfolio_id/);
assert.match(consoleSource, /equity_curve/);
assert.match(consoleSource, /drawdown_curve/);
assert.doesNotMatch(consoleSource, /source_candidate_id:\s*readString\(record\.source_candidate_id\)/);
```

Also assert result table row key matches the column:

```js
assert.match(resultTableSource, /key:\s*["']annualized["']/);
assert.match(resultTableSource, /annualized:/);
assert.match(resultTableSource, /杠杆|Leverage/);
```

- [ ] **Step 5: Strengthen worker contract**

In `tests/verification/backtest_worker_contract.test.mjs`, assert:

```js
assert.match(worker, /portfolio_pool_outputs|eligible_pool_outputs|portfolio_outputs/);
assert.doesNotMatch(worker, /let portfolio_candidates = portfolio_candidates_from_outputs\(&outputs\)/);
assert.doesNotMatch(worker, /"portfolio_id": format!\("portfolio-\{}", portfolio\.member_count\)/);
assert.match(worker, /trades_preview/);
```

- [ ] **Step 6: Run tests and verify failures before implementation**

```bash
cargo test -p backtest-engine portfolio_curve_normalizes_member_equity_by_planned_margin_and_allocation portfolio_allows_multiple_strategies_on_same_symbol_when_candidate_ids_differ portfolio_carries_combined_trade_preview -- --nocapture
node tests/verification/backtest_console_contract.test.mjs
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: FAIL on current branch.

- [ ] **Step 7: Commit tests**

```bash
git add apps/backtest-engine/src/portfolio_search.rs tests/verification/backtest_console_contract.test.mjs tests/verification/backtest_worker_contract.test.mjs
git commit -m "test: 问题描述 锁定马丁组合真实资金曲线契约"
```

---

## Task 2: Fix Portfolio Engine Semantics

**Files:**
- Modify: `apps/backtest-engine/src/portfolio_search.rs`

- [ ] **Step 1: Replace raw equal-weight combiner**

Replace `combine_equity_curves(candidates, weight)` with a function that accepts explicit allocations and initial portfolio capital:

```rust
fn combine_equity_curves(
    members: &[(&EvaluatedCandidate, f64)],
    initial_portfolio_capital: f64,
) -> Vec<EquityPoint> {
    let min_len = members.iter().map(|(candidate, _)| candidate.equity_curve.len()).min().unwrap_or(0);
    if min_len == 0 || members.is_empty() {
        return Vec::new();
    }

    (0..min_len)
        .map(|index| {
            let timestamp_ms = members[0].0.equity_curve[index].timestamp_ms;
            let equity_quote = members
                .iter()
                .map(|(candidate, allocation_pct)| {
                    let allocated_capital = initial_portfolio_capital * (*allocation_pct / 100.0);
                    let initial_candidate_margin = candidate.planned_margin_quote.max(0.000001);
                    let candidate_equity = candidate.equity_curve[index].equity_quote;
                    allocated_capital * candidate_equity / initial_candidate_margin
                })
                .sum();
            EquityPoint { timestamp_ms, equity_quote }
        })
        .collect()
}
```

- [ ] **Step 2: Allow same-symbol multi-strategy portfolios**

Remove the unique-symbol rejection:

```rust
if unique_symbols.len() < members_data.len().min(2) {
    return None;
}
```

Replace it with candidate-id uniqueness only:

```rust
let unique_ids = members_data
    .iter()
    .map(|candidate| candidate.candidate.candidate_id.as_str())
    .collect::<std::collections::HashSet<_>>();
if unique_ids.len() < 2 {
    return None;
}
```

Add optional concentration penalty in scoring instead of hard rejection:

```rust
let unique_symbol_count = members.iter().map(|m| m.symbol.as_str()).collect::<std::collections::HashSet<_>>().len();
let concentration_penalty = if unique_symbol_count == 1 { 0.85 } else { 1.0 };
score *= concentration_penalty;
```

- [ ] **Step 3: Search multiple allocation templates**

Use explicit allocation templates instead of equal weight only:

```rust
let allocation_templates: &[&[f64]] = &[
    &[50.0, 50.0],
    &[60.0, 40.0],
    &[70.0, 30.0],
    &[40.0, 30.0, 30.0],
    &[50.0, 30.0, 20.0],
    &[35.0, 25.0, 20.0, 20.0],
];
```

For each candidate set, try compatible templates where `template.len() == member_count`. Sort/dedupe final Top3 by member IDs + allocation vector.

- [ ] **Step 4: Add portfolio trade details**

Add field to `WeightedPortfolio`:

```rust
#[serde(default)]
pub trades_preview: Vec<crate::martingale::metrics::MartingaleTradeDetail>,
```

Populate by merging member trades sorted by `timestamp_ms`, capped to a practical preview size such as 200:

```rust
let mut trades_preview = members_data.iter().flat_map(|candidate| candidate.trades.clone()).collect::<Vec<_>>();
trades_preview.sort_by_key(|trade| trade.timestamp_ms);
trades_preview.truncate(200);
```

- [ ] **Step 5: Run engine tests**

```bash
cargo test -p backtest-engine portfolio_curve_normalizes_member_equity_by_planned_margin_and_allocation portfolio_allows_multiple_strategies_on_same_symbol_when_candidate_ids_differ portfolio_carries_combined_trade_preview portfolio_top3_combines_multiple_members_not_single_pick -- --nocapture
```

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-engine/src/portfolio_search.rs
git commit -m "fix: 修复思路 修正马丁组合资金归一化与同币多策略组合"
```

---

## Task 3: Fix Worker Portfolio Input and Serialization

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Keep a separate portfolio candidate pool**

Before display truncation, keep a larger pool:

```rust
let portfolio_pool_outputs = select_top_outputs_per_symbol(
    outputs.clone(),
    task.config.per_symbol_top_n.max(20),
    &task.config.risk_profile,
);
let display_outputs = select_top_outputs_per_symbol(
    outputs,
    task.config.per_symbol_top_n.max(1),
    &task.config.risk_profile,
);
```

Use `display_outputs` for `save_candidates_and_artifacts`; use `portfolio_pool_outputs` for `portfolio_candidates_from_outputs`.

- [ ] **Step 2: Remove unused warning**

Either use the returned saved count/rows in summary or prefix with underscore:

```rust
let _persisted_candidates = poller
    .save_candidates_and_artifacts(&task.task_id, evaluated_count, &display_outputs)
    .await?;
```

- [ ] **Step 3: Serialize unique portfolio IDs and full detail fields**

When building `portfolio_rows`, include:

```rust
"portfolio_id": format!("portfolio-{}", rank + 1),
"portfolio_rank": rank + 1,
"member_count": portfolio.member_count,
"members": ...,
"total_return_pct": portfolio.return_pct,
"return_pct": portfolio.return_pct,
"max_drawdown_pct": portfolio.max_drawdown_pct,
"annualized_return_pct": portfolio.annualized_return_pct,
"score": portfolio.score,
"trade_count": portfolio.trade_count,
"equity_curve": portfolio.equity_curve,
"drawdown_curve": portfolio.drawdown_curve,
"trades_preview": portfolio.trades_preview,
"eligible_candidate_count": portfolio_top3.eligible_candidate_count,
```

Members should include `allocation_pct`, `annualized_return_pct`, `trade_count`, and if available `leverage` / `direction_mode`.

- [ ] **Step 4: Preserve long_short leg weights in summaries**

When serializing candidate summary `legs`, include the configured `long_weight_pct` and `short_weight_pct` if present. If domain model lacks dedicated fields, include them in summary metadata derived from the generated candidate so the UI can display them. Do not leave `_long_weight_pct` / `_short_weight_pct` ignored without any persisted effect.

- [ ] **Step 5: Run worker verification**

```bash
cargo test -p backtest-worker -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: pass with no `unused variable` warning from changed code.

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-worker/src/main.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "fix: 修复思路 扩大马丁组合候选池并序列化真实组合详情"
```

---

## Task 4: Fix Web Portfolio Consumption and Result Columns

**Files:**
- Modify: `apps/web/components/backtest/backtest-console.tsx`
- Modify: `apps/web/components/backtest/backtest-result-table.tsx`
- Modify: `apps/web/components/backtest/portfolio-candidate-review.tsx`
- Modify: `apps/web/lib/api-types.ts`

- [ ] **Step 1: Update portfolio type mapping**

In `backtest-console.tsx`, replace old `PortfolioTop3Row` with fields matching worker summary:

```ts
type PortfolioTop3Row = {
  portfolio_id: string;
  portfolio_rank: number;
  member_count: number;
  members: MartingalePortfolioMember[];
  total_return_pct: number;
  max_drawdown_pct: number;
  annualized_return_pct?: number | null;
  score: number;
  trade_count: number;
  equity_curve?: MartingaleEquityPoint[];
  drawdown_curve?: MartingaleEquityPoint[];
  trades_preview?: MartingaleTradeDetail[];
  eligible_candidate_count?: number | null;
};
```

Map from `summary.portfolio_top3` using `members`, `allocation_pct`, `equity_curve`, `drawdown_curve`, and `trades_preview`. Remove old `source_candidate_id` mapping.

- [ ] **Step 2: Pass real portfolio details to review component**

Ensure selected or first portfolio Top3 row passes:

- `portfolioMembers={row.members}`
- `memberCount={row.member_count}`
- `eligibleCandidateCount={row.eligible_candidate_count}`
- portfolio metrics and curves if the component supports them.

If `PortfolioCandidateReview` currently only shows members, add a compact summary card above member table:

```tsx
<p>组合收益：{fmtPct(row.total_return_pct)}</p>
<p>组合年化：{fmtPct(row.annualized_return_pct)}</p>
<p>组合最大回撤：{fmtPct(row.max_drawdown_pct)}</p>
<p>组合交易数：{row.trade_count}</p>
```

- [ ] **Step 3: Fix result table row keys**

In `backtest-result-table.tsx`, row object keys must match columns:

```ts
annualized: candidate.summary?.annualized_return_pct != null ? `${candidate.summary.annualized_return_pct.toFixed(2)}%` : "—",
leverage: candidate.summary?.max_leverage_used != null ? `${candidate.summary.max_leverage_used}x` : "—",
returnDrawdownRatio: candidate.summary?.return_drawdown_ratio != null ? candidate.summary.return_drawdown_ratio.toFixed(2) : "—",
```

Add a leverage column if absent:

```ts
{ key: "leverage", label: pickText(lang, "杠杆", "Leverage"), align: "right" as const },
```

- [ ] **Step 4: Ensure portfolio details show charts and trades**

If portfolio review does not render charts, pass portfolio summary into `BacktestCharts` or add an equivalent section. It must show portfolio equity curve, drawdown curve, and trades preview when present.

- [ ] **Step 5: Run web verification**

```bash
node tests/verification/backtest_console_contract.test.mjs
pnpm --filter web exec next build --webpack
```

- [ ] **Step 6: Commit**

```bash
git add apps/web/components/backtest/backtest-console.tsx apps/web/components/backtest/backtest-result-table.tsx apps/web/components/backtest/portfolio-candidate-review.tsx apps/web/lib/api-types.ts tests/verification/backtest_console_contract.test.mjs
git commit -m "fix: 修复思路 修正马丁组合Top3前端详情消费"
```

---

## Task 5: Final Verification Package

**Files:**
- No planned source changes unless verification fails.

- [ ] **Step 1: Run required tests**

```bash
cargo test -p backtest-engine -- --nocapture
cargo test -p backtest-worker -- --nocapture
cargo test -p api-server martingale_backtest -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
node tests/verification/martingale_backtest_rebuild_contract.test.mjs
pnpm --filter web exec next build --webpack
```

- [ ] **Step 2: Run smoke backtest after build**

Create one BTCUSDT + ETHUSDT `long_short` balanced task. Verify task summary contains:

- candidate with both long and short legs.
- non-null `annualized_return_pct` for candidates where days are valid.
- non-empty `equity_curve` and `drawdown_curve`.
- non-empty `trades_preview` where trades exist.
- `portfolio_top3` rows with `member_count >= 2`.
- each portfolio member allocation sum equals 100%.
- portfolio curve exists and max drawdown is computed from portfolio curve.

- [ ] **Step 3: Report exact evidence**

Claude must report:

- Latest commit hash.
- All command outputs summary.
- Smoke task id.
- Candidate count per symbol.
- Portfolio Top3 member counts and allocation sums.
- Any remaining limitations.

