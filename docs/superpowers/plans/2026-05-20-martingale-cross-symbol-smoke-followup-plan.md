# Martingale Cross-Symbol Smoke Follow-up Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the deployed martingale `long_short` smoke so BTCUSDT+ETHUSDT produces eligible candidates for both symbols and portfolio Top3 actually combines symbols when both are eligible.

**Architecture:** Keep the existing staged search and portfolio builder, but add failing contract tests around the exact smoke payload. First make symbol-level candidate persistence preserve all eligible symbols, then make portfolio construction refuse same-symbol-only Top3 when cross-symbol combinations are possible. Do not weaken the user’s risk standard: balanced `long_short` selected candidates must stay within the existing `[25, 30]` drawdown fallback, and negative-return candidates must remain excluded.

**Tech Stack:** Rust workspace (`backtest-engine`, `backtest-worker`), Node contract tests, Docker Compose deployment.

---

## Verification Evidence From Current Failed Build

Latest merged main: `1a08f02 merge: 修复思路 合并马丁跨币种组合修复`.

Deployed smoke task:

```text
bt_1779247743122840659
```

Payload:

```json
{
  "strategy_type": "martingale",
  "symbols": ["BTCUSDT", "ETHUSDT"],
  "direction": "long_short",
  "direction_mode": "long_short",
  "risk_profile": "balanced",
  "search_space": {
    "leverage": [2],
    "spacing_bps": [120],
    "order_multiplier": [1.25],
    "max_legs": [3],
    "take_profit_bps": [60],
    "tail_stop_bps": [2000],
    "long_short_weight_pct": [[60, 40], [50, 50]]
  }
}
```

Observed final summary:

```json
{
  "status": "succeeded",
  "eligible_candidate_count": 10,
  "eligible_symbols": ["BTCUSDT"],
  "unique_eligible_symbol_count": 1,
  "portfolio_count": 3,
  "portfolio_top3[0].portfolio_unique_symbol_count": 1,
  "portfolio_top3[0].portfolio_symbols": ["BTCUSDT", "BTCUSDT"]
}
```

Observed candidates endpoint:

```json
{
  "count": 10,
  "directions": ["long_short"],
  "symbols": ["BTCUSDT"],
  "all_have_legs": true,
  "best_annualized": 18.8257396691943,
  "worst_drawdown": 24.982973905570688
}
```

This is still a failure because a two-symbol request returns only BTC candidates and portfolio Top3 remains BTC+BTC. The current branch should not be accepted or pushed as complete.

## Acceptance Criteria

- BTCUSDT+ETHUSDT `long_short` smoke with the payload above finishes as `succeeded` or `completed`.
- `/backtest/tasks/{id}/candidates` includes at least one BTCUSDT candidate and at least one ETHUSDT candidate when both symbols produce positive candidates within allowed drawdown.
- Task summary contains `eligible_symbols` with both `BTCUSDT` and `ETHUSDT`, and `unique_eligible_symbol_count >= 2`.
- `portfolio_top3[0].portfolio_unique_symbol_count >= 2` and `portfolio_top3[0].portfolio_symbols` includes both BTCUSDT and ETHUSDT when both have eligible candidates.
- Candidate `direction` remains `long_short`; no fallback to `long` or `short`.
- Each selected `long_short` candidate includes both `long_short_legs.long` and `long_short_legs.short`.
- Candidate and portfolio summaries include finite `annualized_return_pct`, `return_pct`, `max_drawdown_pct`, `planned_margin_quote > 0`, equity curve, drawdown curve, and trade preview.
- Balanced drawdown standard is not weakened: strict target is 25%; fallback may use 30% only when strict produces insufficient cross-symbol candidates. Do not raise balanced fallback above 30%.
- Negative-return candidates must not be used to satisfy portfolio count.

## File Map

- Modify: `apps/backtest-worker/src/main.rs`
  - Ensure multi-symbol candidate search and persistence do not drop ETHUSDT before portfolio construction.
  - Ensure display candidates and portfolio pool candidates are selected per symbol, not globally BTC-only.
  - Add explicit diagnostics for `searched_symbols`, `positive_symbols`, `eligible_symbols`, `dropped_symbols`, and per-symbol rejection counts.

- Modify: `apps/backtest-engine/src/portfolio_search.rs`
  - Ensure cross-symbol combinations are ranked ahead of same-symbol combinations whenever at least two eligible symbols exist.
  - Add a hard invariant for portfolio Top3: if `unique_eligible_symbol_count >= 2`, Top1 must contain at least two symbols unless no valid weighted cross-symbol portfolio can be built; in that exception emit a diagnostic reason.

- Modify: `apps/backtest-engine/tests/search_scoring_time_splits.rs`
  - Add/adjust tests around `long_short` candidate generation and per-symbol selection.

- Modify: `apps/backtest-engine/src/portfolio_search.rs` tests module
  - Add tests that reproduce BTC high score + ETH lower score still yields Top1 BTC+ETH.
  - Add tests that same-symbol multi-strategy is allowed only after cross-symbol portfolios have priority.

- Modify: `apps/backtest-worker/tests` or `tests/verification/backtest_worker_contract.test.mjs`
  - Add a contract test that the worker cannot report success for a multi-symbol task with only one eligible/display symbol unless diagnostics explicitly prove the other symbol has zero positive eligible candidates.

---

### Task 1: Reproduce the Current Failure in Tests

**Files:**
- Modify: `apps/backtest-engine/src/portfolio_search.rs`
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] **Step 1: Add a portfolio unit test for cross-symbol Top1 priority**

Add this test in the `#[cfg(test)]` module of `apps/backtest-engine/src/portfolio_search.rs`:

```rust
#[test]
fn portfolio_top1_uses_cross_symbol_when_two_symbols_are_eligible() {
    let btc_a = candidate_with_symbol("btc-a", "BTCUSDT", 80.0, 20.0, 500.0);
    let btc_b = candidate_with_symbol("btc-b", "BTCUSDT", 70.0, 19.0, 500.0);
    let eth_a = candidate_with_symbol("eth-a", "ETHUSDT", 12.0, 18.0, 500.0);

    let artifact = build_portfolio_top3(&[btc_a, btc_b, eth_a], 25.0);

    assert_eq!(artifact.unique_eligible_symbol_count, 2);
    assert_eq!(artifact.eligible_symbols, vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()]);
    assert!(!artifact.top3.is_empty(), "expected cross-symbol portfolio");

    let symbols = artifact.top3[0]
        .members
        .iter()
        .map(|member| member.symbol.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(symbols.contains("BTCUSDT"), "Top1 symbols: {:?}", symbols);
    assert!(symbols.contains("ETHUSDT"), "Top1 symbols: {:?}", symbols);
}
```

If helper `candidate_with_symbol` does not exist with that exact signature, add a small helper in the test module that builds `EvaluatedCandidate` with:

```rust
candidate_id = id.to_owned()
config.strategies[0].symbol = symbol.to_owned()
score = annualized_return_pct / max_drawdown_pct.max(1.0)
return_pct = annualized_return_pct * 3.0
max_drawdown_pct = drawdown_pct
survival_passed = true
planned_margin_quote = planned_margin
trade_count = 100
annualized_return_pct = Some(annualized_return_pct)
equity_curve = vec![EquityPoint { timestamp_ms: 1672531200000, equity_quote: 10000.0 }, EquityPoint { timestamp_ms: 1777593540000, equity_quote: 10000.0 * (1.0 + annualized_return_pct / 100.0) }]
drawdown_curve = build_drawdown_curve(&equity_curve)
trades = Vec::new()
```

- [ ] **Step 2: Add a worker contract test for multi-symbol success**

In `tests/verification/backtest_worker_contract.test.mjs`, add a static contract test that fails if the worker saves only `display_outputs` globally before building portfolio, or if per-symbol selection can drop non-top symbols. The test must assert these code patterns exist:

```js
test("multi-symbol martingale success preserves per-symbol candidates for portfolio", () => {
  const worker = fs.readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /select_top_outputs_per_symbol/);
  assert.match(worker, /portfolio_pool_outputs/);
  assert.match(worker, /eligible_symbols/);
  assert.match(worker, /unique_eligible_symbol_count/);
  assert.doesNotMatch(worker, /outputs\.into_iter\(\)\.take\(task\.config\.per_symbol_top_n\)/);
});
```

- [ ] **Step 3: Run tests and confirm failure before code changes**

Run:

```bash
cargo test -p backtest-engine portfolio_top1_uses_cross_symbol_when_two_symbols_are_eligible -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected before implementation: at least one test fails or exposes missing invariant.

- [ ] **Step 4: Commit failing tests**

```bash
git add apps/backtest-engine/src/portfolio_search.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "test: 复现路径 锁定马丁跨币种组合冒烟失败"
```

---

### Task 2: Preserve Per-Symbol Candidate Pools in Worker

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Inspect selection flow**

Read the code around `run_long_short_staged_search`, `select_top_outputs_per_symbol`, `portfolio_pool_outputs`, and `save_candidates_and_artifacts`.

Confirm whether ETH is dropped because:

1. ETH produces no positive candidate within drawdown, or
2. ETH candidates exist in `outputs` but are dropped by display/pool selection, or
3. ETH evaluation times out before being persisted.

- [ ] **Step 2: Add per-symbol diagnostics**

In the final task summary fragment, include these fields exactly:

```rust
"searched_symbols": task.config.symbols.clone(),
"display_symbols": display_outputs.iter().map(|row| row.symbol.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect::<Vec<_>>(),
"portfolio_pool_symbols": portfolio_pool_outputs.iter().map(|row| row.symbol.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect::<Vec<_>>(),
"eligible_symbols": portfolio_top3.eligible_symbols.clone(),
"unique_eligible_symbol_count": portfolio_top3.unique_eligible_symbol_count,
```

If `CandidateOutput` does not expose `symbol` directly, derive it from `row.config` using the same helper currently used by `portfolio_candidates_from_outputs`.

- [ ] **Step 3: Make pool selection per-symbol and not globally truncating**

Ensure `portfolio_pool_outputs` uses all symbols independently:

```rust
let portfolio_pool_outputs = select_top_outputs_per_symbol(
    outputs.clone(),
    task.config.per_symbol_top_n.max(20),
    &task.config.risk_profile,
);
```

Then verify `select_top_outputs_per_symbol` groups by symbol before sorting/truncating. If it sorts globally first and truncates before grouping, replace it with:

```rust
fn select_top_outputs_per_symbol(
    outputs: Vec<CandidateOutput>,
    per_symbol_top_n: usize,
    risk_profile: &str,
) -> Vec<CandidateOutput> {
    let mut grouped: std::collections::BTreeMap<String, Vec<CandidateOutput>> = std::collections::BTreeMap::new();
    for output in outputs {
        let symbol = candidate_output_symbol(&output).unwrap_or_else(|| "UNKNOWN".to_owned());
        grouped.entry(symbol).or_default().push(output);
    }

    let mut selected = Vec::new();
    for (_symbol, mut rows) in grouped {
        rows.sort_by(|a, b| candidate_output_rank_score(b, risk_profile)
            .partial_cmp(&candidate_output_rank_score(a, risk_profile))
            .unwrap_or(std::cmp::Ordering::Equal));
        selected.extend(rows.into_iter().take(per_symbol_top_n.max(1)));
    }

    selected.sort_by(|a, b| candidate_output_rank_score(b, risk_profile)
        .partial_cmp(&candidate_output_rank_score(a, risk_profile))
        .unwrap_or(std::cmp::Ordering::Equal));
    selected
}
```

Do not use a global `.take(top_n)` before grouping.

- [ ] **Step 4: Do not hide non-BTC display candidates**

Ensure `display_outputs` also keeps top N per symbol, not top N total. The user needs enough candidates for later portfolio combination:

```rust
let display_outputs = select_top_outputs_per_symbol(
    outputs,
    task.config.per_symbol_top_n.max(1),
    &task.config.risk_profile,
);
```

- [ ] **Step 5: Run focused tests**

```bash
node tests/verification/backtest_worker_contract.test.mjs
cargo test -p backtest-worker selected_outputs_include_ui_required_summary_fields -- --nocapture
```

Expected: pass.

- [ ] **Step 6: Commit worker fix**

```bash
git add apps/backtest-worker/src/main.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "fix: 修复思路 保留马丁跨币种候选池"
```

---

### Task 3: Enforce Cross-Symbol Portfolio Priority

**Files:**
- Modify: `apps/backtest-engine/src/portfolio_search.rs`

- [ ] **Step 1: Update ranking invariant**

In `build_portfolio_top3`, when `unique_eligible_symbol_count >= 2`, split portfolios into diversified and same-symbol before final Top3:

```rust
let mut diversified = Vec::new();
let mut concentrated = Vec::new();
for portfolio in scored_portfolios {
    let unique_symbols = portfolio.members
        .iter()
        .map(|member| member.symbol.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    if unique_symbols >= 2 {
        diversified.push(portfolio);
    } else {
        concentrated.push(portfolio);
    }
}

diversified.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
concentrated.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

let mut top = Vec::new();
if unique_eligible_symbol_count >= 2 && !diversified.is_empty() {
    top.extend(diversified.into_iter().take(3));
    if top.len() < 3 {
        top.extend(concentrated.into_iter().take(3 - top.len()));
    }
} else {
    top.extend(concentrated.into_iter().take(3));
}
```

Make sure this replaces any final sort that can put BTC+BTC ahead of BTC+ETH solely by score.

- [ ] **Step 2: Preserve same-symbol combinations after diversified Top1**

Do not delete same-symbol portfolios entirely. Same-symbol multi-strategy can remain as fallback or lower-ranked entries, but not as Top1 when cross-symbol portfolios are valid.

- [ ] **Step 3: Run portfolio tests**

```bash
cargo test -p backtest-engine portfolio_top1_uses_cross_symbol_when_two_symbols_are_eligible -- --nocapture
cargo test -p backtest-engine portfolio_top1_uses_cross_symbol_even_when_second_symbol_has_lower_return -- --nocapture
cargo test -p backtest-engine portfolio_artifact_reports_eligible_symbols -- --nocapture
```

Expected: all pass.

- [ ] **Step 4: Commit engine fix**

```bash
git add apps/backtest-engine/src/portfolio_search.rs
git commit -m "fix: 修复思路 强制马丁组合优先跨币种"
```

---

### Task 4: Add Exact Smoke Verification Script

**Files:**
- Create or modify: `scripts/verify_martingale_cross_symbol_smoke.sh`

- [ ] **Step 1: Add a reusable smoke script**

Create `scripts/verify_martingale_cross_symbol_smoke.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

EMAIL="codex-smoke-$(date +%s)@example.com"
PASS="pass1234"
BASE_INTERNAL="http://api-server:8080"

register_json=$(docker exec grid-binance-nginx-1 wget -q -O - \
  --header='content-type: application/json' \
  --post-data="{\"email\":\"${EMAIL}\",\"password\":\"${PASS}\"}" \
  "${BASE_INTERNAL}/auth/register")

token=$(docker exec grid-binance-nginx-1 wget -q -O - \
  --header='content-type: application/json' \
  --post-data="{\"email\":\"${EMAIL}\",\"password\":\"${PASS}\"}" \
  "${BASE_INTERNAL}/auth/login" | jq -r .session_token)

payload='{"strategy_type":"martingale","symbols":["BTCUSDT","ETHUSDT"],"direction":"long_short","direction_mode":"long_short","risk_profile":"balanced","search_space":{"leverage":[2],"spacing_bps":[120],"order_multiplier":[1.25],"max_legs":[3],"take_profit_bps":[60],"tail_stop_bps":[2000],"long_short_weight_pct":[[60,40],[50,50]]}}'

task_json=$(docker exec grid-binance-nginx-1 wget -q -O - \
  --header="authorization: Bearer ${token}" \
  --header='content-type: application/json' \
  --post-data="${payload}" \
  "${BASE_INTERNAL}/backtest/tasks")

task_id=$(echo "${task_json}" | jq -r .task_id)
echo "task_id=${task_id}"

for i in $(seq 1 90); do
  response=$(docker exec grid-binance-nginx-1 wget -q -O - \
    --header="authorization: Bearer ${token}" \
    "${BASE_INTERNAL}/backtest/tasks/${task_id}")
  status=$(echo "${response}" | jq -r .status)
  echo "poll=${i} status=${status} stage=$(echo "${response}" | jq -r '.summary.stage // ""')"
  if [[ "${status}" == "succeeded" || "${status}" == "completed" || "${status}" == "failed" || "${status}" == "cancelled" ]]; then
    break
  fi
  sleep 10
done

final=$(docker exec grid-binance-nginx-1 wget -q -O - \
  --header="authorization: Bearer ${token}" \
  "${BASE_INTERNAL}/backtest/tasks/${task_id}")

echo "${final}" | jq '{task_id,status,error_message,eligible_symbols:.summary.eligible_symbols,unique_eligible_symbol_count:.summary.unique_eligible_symbol_count,portfolio_top3:.summary.portfolio_top3[0]}'

echo "${final}" | jq -e '
  (.status == "succeeded" or .status == "completed") and
  (.summary.unique_eligible_symbol_count >= 2) and
  ((.summary.eligible_symbols // []) | index("BTCUSDT") != null) and
  ((.summary.eligible_symbols // []) | index("ETHUSDT") != null) and
  (.summary.portfolio_top3[0].portfolio_unique_symbol_count >= 2) and
  ((.summary.portfolio_top3[0].portfolio_symbols // []) | index("BTCUSDT") != null) and
  ((.summary.portfolio_top3[0].portfolio_symbols // []) | index("ETHUSDT") != null) and
  (.summary.portfolio_top3[0].equity_curve[0].equity_quote == 10000)
'

candidates=$(docker exec grid-binance-nginx-1 wget -q -O - \
  --header="authorization: Bearer ${token}" \
  "${BASE_INTERNAL}/backtest/tasks/${task_id}/candidates")

echo "${candidates}" | jq '{count:length,symbols:(map(.summary.symbol // .symbol)|unique),directions:(map(.summary.direction // .direction)|unique)}'

echo "${candidates}" | jq -e '
  length >= 2 and
  (map(.summary.symbol // .symbol) | unique | index("BTCUSDT") != null) and
  (map(.summary.symbol // .symbol) | unique | index("ETHUSDT") != null) and
  all(.[]; (.summary.direction // .direction) == "long_short") and
  all(.[]; (.summary.planned_margin_quote // 0) > 0) and
  all(.[]; .summary.long_short_legs.long != null and .summary.long_short_legs.short != null)
'
```

- [ ] **Step 2: Make script executable**

```bash
chmod +x scripts/verify_martingale_cross_symbol_smoke.sh
```

- [ ] **Step 3: Commit script**

```bash
git add scripts/verify_martingale_cross_symbol_smoke.sh
git commit -m "test: 复现路径 增加马丁跨币种上线冒烟脚本"
```

---

### Task 5: Final Verification Before Handoff

**Files:**
- No code files unless tests reveal a mistake in prior tasks.

- [ ] **Step 1: Run focused local tests**

```bash
cargo test -p backtest-engine portfolio_top1_uses_cross_symbol_when_two_symbols_are_eligible -- --nocapture
cargo test -p backtest-engine portfolio_top1_uses_cross_symbol_even_when_second_symbol_has_lower_return -- --nocapture
cargo test -p backtest-engine portfolio_artifact_reports_eligible_symbols -- --nocapture
cargo test -p backtest-worker selected_outputs_include_ui_required_summary_fields -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: all pass.

- [ ] **Step 2: Build production images**

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  build api-server backtest-worker web
```

Expected: all three images build.

- [ ] **Step 3: Recreate only target services**

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  up -d --no-deps --force-recreate api-server backtest-worker web
```

Expected: `api-server`, `backtest-worker`, and `web` are running; do not touch unrelated host port 3000 services.

- [ ] **Step 4: Run exact smoke script**

```bash
scripts/verify_martingale_cross_symbol_smoke.sh
```

Expected: script exits 0 and prints:

```text
unique_eligible_symbol_count >= 2
eligible_symbols contains BTCUSDT and ETHUSDT
portfolio_top3[0].portfolio_unique_symbol_count >= 2
portfolio_top3[0].portfolio_symbols contains BTCUSDT and ETHUSDT
candidate symbols contain BTCUSDT and ETHUSDT
candidate directions are only long_short
```

- [ ] **Step 5: Commit final verification note if any docs changed**

Only commit if Task 5 modifies docs or scripts beyond Task 4:

```bash
git add <changed-files>
git commit -m "docs: 复现路径 记录马丁跨币种冒烟验收"
```

---

## Notes for Claude

- Do not solve the failure by hiding ETH from diagnostics or by marking one-symbol portfolios as diversified.
- Do not convert `long_short` to single `long` or `short` candidates.
- Do not weaken balanced drawdown beyond 30% fallback.
- Do not include negative-return candidates just to make ETH appear.
- If ETH truly has zero positive eligible candidates under this exact smoke payload, the system must report that explicitly via diagnostics and must not present a BTC+BTC portfolio as a successful cross-symbol portfolio.
- The preferred behavior is to continue search expansion for the weak symbol until it finds positive eligible candidates under the allowed drawdown or exhausts the configured search budget with clear diagnostics.
