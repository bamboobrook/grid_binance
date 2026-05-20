# Martingale Cross-Symbol Portfolio Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure multi-symbol martingale backtest portfolios actually include multiple eligible symbols when they exist, and expose eligible symbol diagnostics so users can trust why a portfolio is diversified or not.

**Architecture:** The previous fix corrected portfolio equity scaling and per-leg summaries, but Docker smoke still ranked only BTC+BTC portfolios even though ETH eligible candidates existed. Fix portfolio generation/ranking so cross-symbol combinations are generated and prioritized for multi-symbol tasks. Keep the long_short contract, risk limits, and realistic curve scaling unchanged.

**Tech Stack:** Rust `backtest-engine` portfolio search, Rust `backtest-worker` summary serialization, Node verification contracts, Docker smoke on host `8080`.

---

## Failure Evidence From Latest Smoke

After merging `551a10d` and deploying, smoke task:

- Task id: `bt_1779243337870019358`
- Status: `succeeded`
- Candidate count: `12`
- Candidate symbols: `BTCUSDT`, `ETHUSDT`
- All candidates: `direction=long_short`
- Candidate summaries include `long_short_legs`
- Portfolio curve first equity fixed: `10000.0`

But Top3 portfolios still all use only BTC:

```json
{
  "eligible_candidate_count": 10,
  "eligible_symbols": null,
  "portfolio_top3": [
    {
      "rank": 1,
      "portfolio_unique_symbol_count": 1,
      "portfolio_symbols": ["BTCUSDT", "BTCUSDT"],
      "members": [
        { "symbol": "BTCUSDT", "candidate_id": "staged-cand-34" },
        { "symbol": "BTCUSDT", "candidate_id": "staged-cand-9" }
      ]
    }
  ]
}
```

This is unacceptable because ETH eligible candidates existed:

```json
{
  "symbol": "ETHUSDT",
  "direction": "long_short",
  "annualized_return_pct": 4.2911,
  "return_pct": 15.0058,
  "max_drawdown_pct": 28.993,
  "planned_margin_quote": 744.16,
  "long_short_legs": { "long": {}, "short": {} }
}
```

The portfolio algorithm is still optimizing raw return too strongly and not enforcing/meaningfully prioritizing cross-symbol diversification.

---

## Non-Negotiable Requirements

- If a multi-symbol task has eligible candidates from at least two symbols, `portfolio_top3[0]` must include at least two symbols.
- Do not fabricate candidates or include ineligible ETH candidates. Only use candidates with positive return, survival passed, and drawdown within limit.
- If only one eligible symbol exists, expose diagnostics explaining that diversification was impossible.
- Keep portfolio equity curve realistic: first equity near `10000`, finite values.
- Keep all selected candidates `direction=long_short` for `long_short` tasks.
- Do not relax balanced drawdown beyond `[25, 30]`.

---

### Task 1: Add failing tests for cross-symbol Top1 enforcement

**Files:**
- Modify: `apps/backtest-engine/src/portfolio_search.rs`

- [ ] **Step 1: Add high-BTC low-ETH diversification test**

Add this test to `apps/backtest-engine/src/portfolio_search.rs` tests. It reproduces the smoke shape: BTC candidates are stronger, ETH candidates are weaker but still eligible.

```rust
#[test]
fn portfolio_top1_uses_cross_symbol_even_when_second_symbol_has_lower_return() {
    let mut btc_a = fixture_candidate("btc-a", "BTCUSDT", 62.0, 19.0, 62.0);
    btc_a.annualized_return_pct = Some(15.6);
    let mut btc_b = fixture_candidate("btc-b", "BTCUSDT", 64.0, 20.0, 62.0);
    btc_b.annualized_return_pct = Some(16.0);
    let mut btc_c = fixture_candidate("btc-c", "BTCUSDT", 53.0, 20.0, 55.0);
    btc_c.annualized_return_pct = Some(13.6);

    let mut eth_a = fixture_candidate("eth-a", "ETHUSDT", 15.0, 28.9, 20.0);
    eth_a.annualized_return_pct = Some(4.2);
    let mut eth_b = fixture_candidate("eth-b", "ETHUSDT", 1.1, 29.2, 5.0);
    eth_b.annualized_return_pct = Some(0.3);

    let artifact = build_portfolio_top3(&[btc_a, btc_b, btc_c, eth_a, eth_b], 30.0);
    assert!(!artifact.top3.is_empty());
    let first = &artifact.top3[0];
    let symbols: std::collections::HashSet<&str> = first.members.iter().map(|m| m.symbol.as_str()).collect();
    assert!(symbols.contains("BTCUSDT"));
    assert!(symbols.contains("ETHUSDT"), "Top1 must diversify when ETH eligible exists: {:?}", first.members);
}
```

Expected before fix: FAIL, matching latest smoke.

- [ ] **Step 2: Add eligible symbol diagnostics artifact test**

If `PortfolioTop3Artifact` does not currently expose eligible symbols, extend the struct with:

```rust
pub eligible_symbols: Vec<String>,
pub unique_eligible_symbol_count: usize,
```

Add test:

```rust
#[test]
fn portfolio_artifact_reports_eligible_symbols() {
    let artifact = build_portfolio_top3(&[
        fixture_candidate("btc", "BTCUSDT", 30.0, 10.0, 3.0),
        fixture_candidate("eth", "ETHUSDT", 10.0, 20.0, 2.0),
    ], 30.0);

    assert_eq!(artifact.unique_eligible_symbol_count, 2);
    assert_eq!(artifact.eligible_symbols, vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()]);
}
```

- [ ] **Step 3: Run failing tests**

```bash
cargo test -p backtest-engine portfolio_top1_uses_cross_symbol_even_when_second_symbol_has_lower_return -- --nocapture
cargo test -p backtest-engine portfolio_artifact_reports_eligible_symbols -- --nocapture
```

Expected: first fails before implementation; second may fail until struct fields are added.

- [ ] **Step 4: Commit tests**

```bash
git add apps/backtest-engine/src/portfolio_search.rs
git commit -m "test: 问题描述 锁定马丁组合跨币种Top1"
```

---

### Task 2: Generate and prioritize cross-symbol portfolios

**Files:**
- Modify: `apps/backtest-engine/src/portfolio_search.rs`

- [ ] **Step 1: Track eligible symbols**

In `build_portfolio_top3()`, after filtering eligible candidates, compute:

```rust
let eligible_symbols: Vec<String> = eligible
    .iter()
    .map(|candidate| candidate_symbol(candidate))
    .collect::<std::collections::BTreeSet<_>>()
    .into_iter()
    .collect();
let unique_eligible_symbol_count = eligible_symbols.len();
```

Add helper:

```rust
fn candidate_symbol(candidate: &EvaluatedCandidate) -> String {
    candidate
        .candidate
        .config
        .strategies
        .first()
        .map(|strategy| strategy.symbol.clone())
        .unwrap_or_default()
}
```

- [ ] **Step 2: Force cross-symbol candidates into the search pool when possible**

Before generic top combinations, generate dedicated cross-symbol combinations from the best candidates per symbol.

Add helper:

```rust
fn best_indices_by_symbol(eligible: &[&EvaluatedCandidate], per_symbol: usize) -> Vec<usize> {
    let mut grouped: std::collections::BTreeMap<String, Vec<(usize, &EvaluatedCandidate)>> = std::collections::BTreeMap::new();
    for (index, candidate) in eligible.iter().enumerate() {
        grouped.entry(candidate_symbol(candidate)).or_default().push((index, *candidate));
    }
    let mut result = Vec::new();
    for (_symbol, mut rows) in grouped {
        rows.sort_by(|a, b| b.1.score.partial_cmp(&a.1.score).unwrap_or(std::cmp::Ordering::Equal));
        result.extend(rows.into_iter().take(per_symbol).map(|(index, _)| index));
    }
    result.sort_unstable();
    result.dedup();
    result
}
```

At the start of portfolio generation, if `unique_eligible_symbol_count >= 2`, build combinations from these indices first:

```rust
if unique_eligible_symbol_count >= 2 {
    let diversified_indices = best_indices_by_symbol(&eligible, 3);
    for i_pos in 0..diversified_indices.len() {
        for j_pos in (i_pos + 1)..diversified_indices.len() {
            let i = diversified_indices[i_pos];
            let j = diversified_indices[j_pos];
            if candidate_symbol(eligible[i]) == candidate_symbol(eligible[j]) {
                continue;
            }
            for template in &allocation_templates {
                if template.len() == 2 {
                    if let Some(portfolio) = build_weighted_portfolio(&eligible, &[i, j], template) {
                        scored_portfolios.push(portfolio);
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 3: Rank cross-symbol portfolios above same-symbol portfolios when eligible**

After all portfolios are generated, split/rank:

```rust
scored_portfolios.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

if unique_eligible_symbol_count >= 2 {
    let mut diversified: Vec<_> = scored_portfolios
        .iter()
        .cloned()
        .filter(|p| p.members.iter().map(|m| m.symbol.as_str()).collect::<std::collections::HashSet<_>>().len() >= 2)
        .collect();
    let mut concentrated: Vec<_> = scored_portfolios
        .iter()
        .cloned()
        .filter(|p| p.members.iter().map(|m| m.symbol.as_str()).collect::<std::collections::HashSet<_>>().len() < 2)
        .collect();
    diversified.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    concentrated.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored_portfolios = diversified.into_iter().chain(concentrated.into_iter()).collect();
}
scored_portfolios.truncate(3);
```

This is intentional: for multi-symbol portfolios, diversification is part of the product requirement, not a weak optional bonus.

- [ ] **Step 4: Return eligible symbol diagnostics**

Set the new artifact fields:

```rust
PortfolioTop3Artifact {
    top3: scored_portfolios,
    eligible_candidate_count: eligible_count,
    eligible_symbols,
    unique_eligible_symbol_count,
}
```

Update any tests/constructors accordingly.

- [ ] **Step 5: Run tests**

```bash
cargo test -p backtest-engine portfolio_top1_uses_cross_symbol_even_when_second_symbol_has_lower_return -- --nocapture
cargo test -p backtest-engine portfolio_artifact_reports_eligible_symbols -- --nocapture
cargo test -p backtest-engine portfolio_top3_prefers_cross_symbol_members_when_available -- --nocapture
cargo test -p backtest-engine portfolio_top3_combines_multiple_members -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-engine/src/portfolio_search.rs
git commit -m "fix: 修复思路 强制优先马丁跨币种组合"
```

---

### Task 3: Serialize eligible-symbol diagnostics to task summary

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] **Step 1: Add portfolio diagnostics to summary**

When building task summary after `build_portfolio_top3()`, include:

```rust
"eligible_symbols": portfolio_top3.eligible_symbols,
"unique_eligible_symbol_count": portfolio_top3.unique_eligible_symbol_count,
```

Be careful not to move `portfolio_top3.eligible_symbols` before later use; clone if needed.

- [ ] **Step 2: Keep per-portfolio symbol fields**

Ensure each portfolio row contains:

```rust
"portfolio_symbols": portfolio.members.iter().map(|m| m.symbol.clone()).collect::<Vec<_>>(),
"portfolio_unique_symbol_count": portfolio.members.iter().map(|m| m.symbol.clone()).collect::<std::collections::BTreeSet<_>>().len(),
```

Use `BTreeSet` for stable output.

- [ ] **Step 3: Add contract test**

Append to `tests/verification/backtest_worker_contract.test.mjs`:

```js
test("worker summary exposes eligible symbols and portfolio unique symbol count", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /eligible_symbols/);
  assert.match(worker, /unique_eligible_symbol_count/);
  assert.match(worker, /portfolio_unique_symbol_count/);
  assert.match(worker, /portfolio_symbols/);
});
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p backtest-worker selected_outputs_include_ui_required_summary_fields -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-worker/src/main.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "fix: 修复思路 输出马丁组合可用币种诊断"
```

---

### Task 4: Full verification and Docker smoke acceptance

**Files:**
- No source changes unless tests fail.

- [ ] **Step 1: Full verification**

Run:

```bash
cargo test -p backtest-engine -- --nocapture
cargo test -p backtest-worker -- --nocapture
cargo test -p api-server martingale -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
pnpm --filter web exec next build --webpack
```

Expected: all PASS.

- [ ] **Step 2: Docker build/restart**

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  build api-server backtest-worker web

docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  up -d --no-deps --force-recreate api-server backtest-worker web
```

- [ ] **Step 3: Repeat exact BTC/ETH balanced long_short smoke**

Use the exact prior payload.

Acceptance requires:

```text
task.status in ["succeeded", "completed"]
summary.eligible_symbols includes BTCUSDT and ETHUSDT when both have eligible candidates
summary.unique_eligible_symbol_count >= 2
summary.portfolio_top3[0].portfolio_unique_symbol_count >= 2
summary.portfolio_top3[0].portfolio_symbols includes BTCUSDT and ETHUSDT
summary.portfolio_top3[0].equity_curve[0].equity_quote == about 10000
all candidates direction == long_short
all selected candidates planned_margin_quote > 0
candidate summaries include long_short_legs.long and long_short_legs.short
```

Suggested check:

```bash
TASK_JSON=$(docker exec grid-binance-nginx-1 wget -q -O - --header="authorization: Bearer $TOKEN" http://api-server:8080/backtest/tasks/$TASK_ID)
echo "$TASK_JSON" | jq '{status, eligible_symbols:.summary.eligible_symbols, unique_eligible_symbol_count:.summary.unique_eligible_symbol_count, first_portfolio:.summary.portfolio_top3[0] | {portfolio_unique_symbol_count, portfolio_symbols, first_equity:.equity_curve[0].equity_quote, last_equity:.equity_curve[-1].equity_quote, members}}'
```

Reject if Top1 remains BTC+BTC while ETH eligible candidates are present.

---

## Do Not Do

- Do not lower the risk filter just to include ETH.
- Do not fabricate ETH candidates.
- Do not reintroduce single-direction fallbacks.
- Do not accept same-symbol-only Top1 when cross-symbol eligible candidates exist.
- Do not touch unrelated host port `3000`.
