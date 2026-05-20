# Martingale Portfolio Credibility Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the remaining credibility issues in martingale long_short backtest results: portfolio equity curves must use realistic capital scaling, portfolio Top3 must not collapse into one symbol when a multi-symbol task has eligible candidates, and candidate summaries must expose long/short leg parameters clearly.

**Architecture:** The latest Claude branch made the BTC/ETH long_short smoke complete with positive candidates, but the result is still not acceptable for live decision-making. Do not change the core long_short direction contract or relax risk standards. Fix result construction and acceptance contracts around portfolio combination, equity scaling, and candidate detail serialization.

**Tech Stack:** Rust `backtest-engine` portfolio search, Rust `backtest-worker` result serialization, Node verification contracts, Docker smoke on host `8080`.

---

## Failure Evidence From Deployed Smoke

Smoke task after merging `058cf2c`:

- Task id: `bt_1779238231410511363`
- Payload: BTC/ETH, `direction_mode=long_short`, balanced, exact previous smoke payload.
- API status: `succeeded`
- Candidates: `10`, all `direction=long_short`, positive annualized, drawdown under 25%.

But result is not trustworthy:

1. Portfolio equity curve is absurdly huge:

```json
{
  "suspicious_equity_first": { "equity_quote": 4337161872448.979, "timestamp_ms": 1672531200000 },
  "suspicious_equity_last": { "equity_quote": 7466953862954.451, "timestamp_ms": 1777593540000 }
}
```

A portfolio initialized around `10_000` quote should not start at trillions. This indicates `combine_equity_curves()` is mixing absolute candidate equity with planned margin incorrectly.

2. Portfolio Top3 members all came from BTC even though the task requested BTC + ETH:

```json
"members": [
  { "symbol": "BTCUSDT", "candidate_id": "staged-cand-110", "direction": "long_short" },
  { "symbol": "BTCUSDT", "candidate_id": "staged-cand-77", "direction": "long_short" },
  { "symbol": "BTCUSDT", "candidate_id": "staged-cand-15", "direction": "long_short" }
]
```

For a multi-symbol task, portfolio Top3 should prefer cross-symbol diversification when eligible candidates exist. It must not silently become multiple BTC variants unless ETH has no eligible candidate, and that reason must be visible.

3. Candidate list summaries expose `long_weight_pct=null` and `short_weight_pct=null` even for true `long_short` candidates. Users cannot verify the long/short allocation or per-leg parameters.

4. Build emits dead-code warnings in `backtest-worker`:

```text
function `interleave_candidates_by_spacing` is never used
function `strategy_take_profit_bps` is never used
```

These are not functional blockers, but they indicate cleanup is needed after stratified sampling changes.

---

## Non-Negotiable Requirements

- Do not reintroduce single-direction candidates into `long_short` requests.
- Do not relax balanced risk beyond `[25, 30]` drawdown fallback.
- Do not fabricate ETH candidates or portfolio members.
- Do not hide failed diversification; if no ETH candidate is eligible, show diagnostics.
- Portfolio equity curve must start near configured portfolio capital, not candidate raw capital or trillions.
- Candidate summaries must show per-leg long/short spacing, TP, stop, multiplier, max legs, leverage, planned margin, and weights.

---

### Task 1: Add failing tests for portfolio equity scaling

**Files:**
- Modify: `apps/backtest-engine/src/portfolio_search.rs`

- [ ] **Step 1: Add test that combined portfolio starts at configured capital**

Add to `apps/backtest-engine/src/portfolio_search.rs` tests:

```rust
#[test]
fn weighted_portfolio_equity_curve_starts_near_initial_portfolio_capital() {
    let mut btc = fixture_candidate("btc", "BTCUSDT", 30.0, 10.0, 3.0);
    btc.planned_margin_quote = 500.0;
    btc.equity_curve = vec![
        EquityPoint { timestamp_ms: 1, equity_quote: 500.0 },
        EquityPoint { timestamp_ms: 2, equity_quote: 650.0 },
    ];

    let mut eth = fixture_candidate("eth", "ETHUSDT", 20.0, 8.0, 2.0);
    eth.planned_margin_quote = 250.0;
    eth.equity_curve = vec![
        EquityPoint { timestamp_ms: 1, equity_quote: 250.0 },
        EquityPoint { timestamp_ms: 2, equity_quote: 300.0 },
    ];

    let portfolio = build_weighted_portfolio(&[&btc, &eth], &[0, 1], &[0.6, 0.4])
        .expect("portfolio should build");

    let first = portfolio.equity_curve.first().unwrap().equity_quote;
    let last = portfolio.equity_curve.last().unwrap().equity_quote;
    assert!((first - 10_000.0).abs() < 0.0001, "first equity should equal initial portfolio capital, got {first}");
    assert!(last > first, "last equity should grow proportionally, first={first}, last={last}");
    assert!(last < 13_000.0, "last equity should be realistically scaled, got {last}");
}
```

Expected before fix: FAIL if `combine_equity_curves()` uses the wrong denominator or raw candidate equity.

- [ ] **Step 2: Add test rejecting zero planned margin curves**

Add:

```rust
#[test]
fn weighted_portfolio_rejects_zero_or_missing_planned_margin() {
    let mut btc = fixture_candidate("btc", "BTCUSDT", 30.0, 10.0, 3.0);
    btc.planned_margin_quote = 0.0;
    btc.equity_curve = vec![
        EquityPoint { timestamp_ms: 1, equity_quote: 500.0 },
        EquityPoint { timestamp_ms: 2, equity_quote: 650.0 },
    ];

    let mut eth = fixture_candidate("eth", "ETHUSDT", 20.0, 8.0, 2.0);
    eth.planned_margin_quote = 250.0;
    eth.equity_curve = vec![
        EquityPoint { timestamp_ms: 1, equity_quote: 250.0 },
        EquityPoint { timestamp_ms: 2, equity_quote: 300.0 },
    ];

    assert!(build_weighted_portfolio(&[&btc, &eth], &[0, 1], &[0.6, 0.4]).is_none());
}
```

- [ ] **Step 3: Run tests to see failure**

```bash
cargo test -p backtest-engine weighted_portfolio_equity_curve_starts_near_initial_portfolio_capital -- --nocapture
cargo test -p backtest-engine weighted_portfolio_rejects_zero_or_missing_planned_margin -- --nocapture
```

Expected: at least one FAIL before implementation.

- [ ] **Step 4: Commit failing tests**

```bash
git add apps/backtest-engine/src/portfolio_search.rs
git commit -m "test: 问题描述 锁定马丁组合资金曲线可信度"
```

---

### Task 2: Fix weighted portfolio curve scaling

**Files:**
- Modify: `apps/backtest-engine/src/portfolio_search.rs`

- [ ] **Step 1: Reject invalid planned margins before building**

At the start of `build_weighted_portfolio()`, after `members_data` is built, add:

```rust
if members_data.iter().any(|candidate| !candidate.planned_margin_quote.is_finite() || candidate.planned_margin_quote <= 0.0) {
    return None;
}
if members_data.iter().any(|candidate| candidate.equity_curve.is_empty()) {
    return None;
}
```

- [ ] **Step 2: Fix `combine_equity_curves()` ratio math**

Keep the intended formula, but ensure the initial candidate equity is taken from the candidate's first equity point, not from `planned_margin_quote` unless the first point is invalid.

Replace per-member computation with:

```rust
let initial_candidate_equity = candidate
    .equity_curve
    .first()
    .map(|point| point.equity_quote)
    .filter(|value| value.is_finite() && *value > 0.0)
    .unwrap_or(candidate.planned_margin_quote);

if !initial_candidate_equity.is_finite() || initial_candidate_equity <= 0.0 {
    return 0.0;
}

let candidate_equity = candidate.equity_curve[i].equity_quote;
let candidate_return_factor = candidate_equity / initial_candidate_equity;
allocated_capital * candidate_return_factor
```

The combined curve should represent “if the portfolio allocated 60% of 10,000 to candidate A and 40% to B”, not raw candidate quote values.

- [ ] **Step 3: Add finite result guard**

After `combined_curve` is built, reject non-finite or absurd first values:

```rust
let first_equity = combined_curve.first().map(|p| p.equity_quote).unwrap_or(0.0);
if !first_equity.is_finite() || first_equity <= 0.0 || (first_equity - initial_portfolio_capital).abs() > 0.01 {
    return None;
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p backtest-engine weighted_portfolio_equity_curve_starts_near_initial_portfolio_capital -- --nocapture
cargo test -p backtest-engine weighted_portfolio_rejects_zero_or_missing_planned_margin -- --nocapture
cargo test -p backtest-engine portfolio_top3_combines_multiple_members -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-engine/src/portfolio_search.rs
git commit -m "fix: 修复思路 修正马丁组合资金曲线缩放"
```

---

### Task 3: Prefer multi-symbol portfolio diversification when available

**Files:**
- Modify: `apps/backtest-engine/src/portfolio_search.rs`
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add portfolio diversification test**

Add to `apps/backtest-engine/src/portfolio_search.rs` tests:

```rust
#[test]
fn portfolio_top3_prefers_cross_symbol_members_when_available() {
    let candidates = vec![
        fixture_candidate("btc-a", "BTCUSDT", 30.0, 10.0, 3.0),
        fixture_candidate("btc-b", "BTCUSDT", 28.0, 11.0, 2.9),
        fixture_candidate("eth-a", "ETHUSDT", 20.0, 8.0, 2.0),
        fixture_candidate("eth-b", "ETHUSDT", 18.0, 9.0, 1.8),
    ];

    let artifact = build_portfolio_top3(&candidates, 25.0);
    assert!(!artifact.top3.is_empty());
    let first = &artifact.top3[0];
    let symbols: std::collections::HashSet<&str> = first.members.iter().map(|member| member.symbol.as_str()).collect();
    assert!(symbols.contains("BTCUSDT"));
    assert!(symbols.contains("ETHUSDT"), "first portfolio should diversify across eligible requested symbols: {:?}", first.members);
}
```

- [ ] **Step 2: Adjust portfolio scoring to strongly prefer cross-symbol combinations**

In `build_weighted_portfolio()`, replace the current weak diversification logic:

```rust
let diversification_bonus = 1.0 + (portfolio_members.len() as f64 - 1.0) * 0.05;
let unique_symbol_count = portfolio_members.iter().map(|m| m.symbol.as_str()).collect::<std::collections::HashSet<_>>().len();
let concentration_penalty = if unique_symbol_count == 1 { 0.85 } else { 1.0 };
let score = calmar * diversification_bonus * concentration_penalty;
```

with:

```rust
let unique_symbol_count = portfolio_members
    .iter()
    .map(|m| m.symbol.as_str())
    .collect::<std::collections::HashSet<_>>()
    .len();
let member_count = portfolio_members.len().max(1);
let diversification_factor = unique_symbol_count as f64 / member_count as f64;
let concentration_penalty = if unique_symbol_count == 1 { 0.50 } else { 1.0 };
let diversification_bonus = 1.0 + diversification_factor * 0.35;
let score = calmar * diversification_bonus * concentration_penalty;
```

This does not fabricate ETH candidates; it only ranks cross-symbol combinations higher when they exist.

- [ ] **Step 3: Add worker diagnostic when only one symbol contributes portfolios**

When serializing `portfolio_top3`, add summary fields:

```rust
"portfolio_symbols": portfolio.members.iter().map(|m| m.symbol.clone()).collect::<Vec<_>>(),
"portfolio_unique_symbol_count": portfolio.members.iter().map(|m| m.symbol.clone()).collect::<std::collections::HashSet<_>>().len(),
```

At task summary level add:

```rust
"eligible_symbols": outputs.iter().filter(|o| o.total_return_pct > 0.0 && o.max_drawdown_pct <= o.used_drawdown_limit_pct).filter_map(|o| output_symbol(o)).collect::<std::collections::BTreeSet<_>>(),
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p backtest-engine portfolio_top3_prefers_cross_symbol_members_when_available -- --nocapture
cargo test -p backtest-engine portfolio_top3_combines_multiple_members -- --nocapture
cargo test -p backtest-worker selected_outputs_include_ui_required_summary_fields -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-engine/src/portfolio_search.rs apps/backtest-worker/src/main.rs
git commit -m "fix: 修复思路 优先构建跨币种马丁组合"
```

---

### Task 4: Expose long/short leg parameters in candidate summaries

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `tests/verification/backtest_console_contract.test.mjs`

- [ ] **Step 1: Add helper extracting long_short leg summary**

In `apps/backtest-worker/src/main.rs`, add helpers near other output helpers:

```rust
fn output_long_short_leg_summary(output: &CandidateOutput) -> Value {
    let Some(strategies) = output.config.get("strategies").and_then(|v| v.as_array()) else {
        return json!({});
    };

    let mut result = serde_json::Map::new();
    for strategy in strategies {
        let direction = strategy.get("direction").and_then(|v| v.as_str()).unwrap_or_default().to_lowercase();
        let key = if direction.contains("long") { "long" } else if direction.contains("short") { "short" } else { continue };
        result.insert(key.to_owned(), json!({
            "weight_pct": strategy_weight_pct(strategy),
            "spacing_bps": strategy_value_at_raw(strategy, &["spacing", "step_bps"]),
            "take_profit_bps": strategy_value_at_raw(strategy, &["take_profit", "bps"]),
            "stop_loss_bps": strategy_value_at_raw(strategy, &["stop_loss", "pct_bps"]),
            "first_order_quote": strategy_value_at_raw(strategy, &["sizing", "first_order_quote"]),
            "order_multiplier": strategy_value_at_raw(strategy, &["sizing", "multiplier"]),
            "max_legs": strategy_value_at_raw(strategy, &["sizing", "max_legs"]),
            "leverage": strategy.get("leverage").cloned().unwrap_or(Value::Null),
        }));
    }
    Value::Object(result)
}

fn strategy_value_at_raw(strategy: &Value, path: &[&str]) -> Value {
    let mut current = strategy;
    for key in path {
        let Some(next) = current.get(*key) else { return Value::Null };
        current = next;
    }
    current.clone()
}

fn strategy_weight_pct(strategy: &Value) -> Value {
    strategy
        .get("sizing")
        .and_then(|sizing| sizing.get("first_order_quote"))
        .cloned()
        .unwrap_or(Value::Null)
}
```

If there is already a suitable raw strategy helper, reuse it instead of adding duplicate code.

- [ ] **Step 2: Include leg summary in candidate rows and summaries**

When building candidate summary JSON, include:

```rust
"long_short_legs": output_long_short_leg_summary(output),
```

Also fill top-level convenience fields from that summary:

```rust
"long_weight_pct": output_long_short_leg_summary(output).get("long").and_then(|v| v.get("weight_pct")).cloned().unwrap_or(Value::Null),
"short_weight_pct": output_long_short_leg_summary(output).get("short").and_then(|v| v.get("weight_pct")).cloned().unwrap_or(Value::Null),
```

If values are quote amounts rather than percentages, name them `long_first_order_quote` / `short_first_order_quote` instead. Do not label quote amounts as percentages.

- [ ] **Step 3: Add contract test**

Append to `tests/verification/backtest_console_contract.test.mjs`:

```js
test("backtest result exposes long_short per-leg parameters for human review", () => {
  const consoleSource = readFileSync("apps/web/components/backtest/backtest-console.tsx", "utf8");
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /long_short_legs/);
  assert.match(worker, /spacing_bps/);
  assert.match(worker, /take_profit_bps/);
  assert.match(worker, /stop_loss_bps/);
  assert.match(consoleSource, /long_short_legs|多空腿|Long\/Short/);
});
```

If the UI already renders generic JSON detail, still add explicit labels so the user does not see null weight fields.

- [ ] **Step 4: Run tests**

```bash
cargo test -p backtest-worker selected_outputs_include_ui_required_summary_fields -- --nocapture
node tests/verification/backtest_console_contract.test.mjs
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-worker/src/main.rs tests/verification/backtest_console_contract.test.mjs
git commit -m "fix: 修复思路 展示马丁多空分腿参数"
```

---

### Task 5: Cleanup dead code warnings

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Remove unused helpers**

Remove unused `interleave_candidates_by_spacing()` if stratified sampling fully replaced it.

Remove or use `strategy_take_profit_bps()` in non-test code. If only tests need it, annotate:

```rust
#[cfg(test)]
fn strategy_take_profit_bps(...)
```

- [ ] **Step 2: Verify warning cleanup**

Run:

```bash
cargo test -p backtest-worker long_short_balanced_auto_search_expands_all_key_dimensions -- --nocapture
cargo build -p backtest-worker
```

Expected: no dead-code warnings introduced by this feature.

- [ ] **Step 3: Commit**

```bash
git add apps/backtest-worker/src/main.rs
git commit -m "chore: 修复思路 清理马丁回测Worker死代码"
```

---

### Task 6: Full verification and Docker smoke acceptance

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

Use same payload as previous smoke.

Required acceptance:

```text
task.status in ["succeeded", "completed"]
candidate count >= 10
all candidate directions are long_short
positive annualized candidates >= 1
selected candidate max_drawdown_pct <= 30
portfolio_top3 count >= 1
first portfolio member_count >= 2
first portfolio unique symbol count >= 2 if both BTC and ETH have eligible candidates
portfolio first equity is near 10,000, not trillions
portfolio equity values are finite
candidate summaries include long_short_legs.long and long_short_legs.short
planned_margin_quote is positive for every selected candidate
```

Suggested API checks:

```bash
TASK_JSON=$(docker exec grid-binance-nginx-1 wget -q -O - --header="authorization: Bearer $TOKEN" http://api-server:8080/backtest/tasks/$TASK_ID)
echo "$TASK_JSON" | jq '{status, eligible_candidate_count:.summary.eligible_candidate_count, portfolio_count:(.summary.portfolio_top3|length), first_portfolio:.summary.portfolio_top3[0] | {member_count, return_pct, annualized_return_pct, max_drawdown_pct, portfolio_unique_symbol_count, first_equity:.equity_curve[0], last_equity:.equity_curve[-1], members}}'
```

If `first_equity` is not around `10000`, reject the fix.

- [ ] **Step 4: If smoke fails, write next plan instead of merging**

Do not pass with suspiciously huge equity, null leg details, or same-symbol-only portfolio when cross-symbol eligible candidates exist.

---

## Do Not Do

- Do not tune metrics to hide unrealistic portfolio curves.
- Do not mark smoke accepted solely because task status is `succeeded`.
- Do not relax risk thresholds.
- Do not return single-direction fallback candidates.
- Do not fabricate ETH candidates or portfolio members.
- Do not touch unrelated host port `3000`.
