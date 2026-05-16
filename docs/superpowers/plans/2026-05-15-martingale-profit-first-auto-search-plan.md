# Martingale Profit-First Auto Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the martingale backtest workflow so the user only selects symbols, direction, and risk profile, then the system finds positive-return per-symbol Top 10 strategies and portfolio Top 3 combinations using conservative futures-isolated backtesting.

**Architecture:** Keep the existing API/task/worker shape, but replace manual parameter assumptions with a staged search pipeline in `backtest-engine` and `backtest-worker`. Store richer summaries in existing candidate JSON/artifacts first to avoid unnecessary schema churn, then render them in the existing Next.js backtest console.

**Tech Stack:** Rust workspace (`backtest-engine`, `backtest-worker`, `api-server`), SQLite market-data reader, shared DB repositories, Next.js/TypeScript frontend, Node contract tests, Cargo tests.

---

## File Structure

### Backend Engine
- Modify: `apps/backtest-engine/src/martingale/scoring.rs`
  - Convert rank scoring to 0–100 human-readable safety score.
  - Reject negative return, liquidation, and over-drawdown candidates before ranking.
- Modify: `apps/backtest-engine/src/martingale/metrics.rs`
  - Ensure annualized return, total-return, max-drawdown, cost, stop, liquidation, and plan-capital metrics are present and serialized.
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
  - Enforce futures-isolated margin semantics and full-period 1m validation metrics.
- Modify: `apps/backtest-engine/src/martingale/trade_engine.rs`
  - Ensure final validation uses full 1m/trade-derived data and records trade details.
- Modify: `apps/backtest-engine/src/search.rs`
  - Add deterministic staged coarse/fine search spaces by risk profile.
- Create: `apps/backtest-engine/src/martingale/portfolio_search.rs`
  - Build Top 3 portfolio combinations from per-symbol Top 10 candidates.
- Modify: `apps/backtest-engine/src/martingale/mod.rs`
  - Export portfolio search module.
- Modify: `apps/backtest-engine/src/time_splits.rs`
  - Ensure auto end date resolves to previous month end.

### Worker
- Modify: `apps/backtest-worker/src/main.rs`
  - Accept profit-first payload contract.
  - Force futures market and 1m final validation.
  - Run per-symbol staged search, risk relaxation, Top 10 persistence, and portfolio Top 3 artifact creation.
  - Persist rejection reasons for symbols with no usable candidates.

### API Server
- Modify: `apps/api-server/src/services/backtest_service.rs`
  - Normalize task config to `market=futures`, `per_symbol_top_n=10`, `portfolio_top_n=3`, staged mode.
  - Reject spot for this workflow.
- Modify: `apps/api-server/src/routes/backtest.rs`
  - Expose portfolio Top 3 result payload through task/candidate response if not already returned.
- Modify: `apps/api-server/tests/martingale_backtest_flow.rs`
  - Add contract tests for new payload normalization and no negative candidates.

### Frontend
- Modify: `apps/web/components/backtest/backtest-wizard.tsx`
  - Reduce default UI to symbols, direction, risk profile, and start button.
  - Remove spot choice from default flow.
  - Build Top 10/Top 3 futures-only payload.
- Modify: `apps/web/components/backtest/backtest-result-table.tsx`
  - Render per-symbol Top 10 instead of Top 5.
  - Show leverage, long/short parameters, annualized return, return/drawdown ratio, 0–100 score.
- Modify: `apps/web/components/backtest/backtest-charts.tsx`
  - Ensure equity/drawdown tooltip shows date, return %, drawdown %.
- Modify: `apps/web/components/backtest/portfolio-candidate-review.tsx`
  - Add portfolio Top 3 section and selected-strategy weight breakdown.
- Modify: `apps/web/components/backtest/backtest-console.tsx`
  - Display failed-symbol reasons, task progress, delete result actions, and portfolio Top 3.
- Modify: `apps/web/components/backtest/martingale-parameter-editor.tsx`
  - Move manual fields into advanced/manual mode; default flow should not require them.

### Verification
- Modify: `tests/verification/martingale_backtest_rebuild_contract.test.mjs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`
- Modify: `tests/verification/backtest_console_contract.test.mjs`
- Add/modify Rust tests under:
  - `apps/backtest-engine/tests/search_scoring_time_splits.rs`
  - `apps/backtest-engine/tests/portfolio_search.rs`

---

## Task 1: Lock the New Contract With Tests

**Files:**
- Modify: `tests/verification/martingale_backtest_rebuild_contract.test.mjs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`
- Modify: `tests/verification/backtest_console_contract.test.mjs`
- Modify: `apps/api-server/tests/martingale_backtest_flow.rs`

- [ ] **Step 1: Update frontend contract test expectations**

In `tests/verification/martingale_backtest_rebuild_contract.test.mjs`, change Top 5 expectations to Top 10/Top 3 and futures-only defaults:

```js
assert.match(source, /开始自动搜索 Top 10|Start automatic Top 10 search/);
assert.match(source, /per_symbol_top_n:\s*10/);
assert.match(source, /portfolio_top_n:\s*3/);
assert.match(source, /market:\s*["']futures["']|market:\s*["']usd_m_futures["']/);
assert.doesNotMatch(source, /<option value="spot">|Spot<\/option>/);
assert.match(source, /risk_profile/);
assert.match(source, /auto_since_2023_to_last_month_end|auto_previous_month_end/);
```

Also update result table assertions:

```js
assert.match(resultSource, /每个币种 Top 10|Per-symbol Top 10/i);
assert.match(resultSource, /组合 Top 3|Portfolio Top 3/i);
assert.match(resultSource, /年化收益|Annualized/i);
assert.match(resultSource, /收益回撤比|Return\/DD/i);
assert.match(resultSource, /0–100|0-100|百分制/i);
```

- [ ] **Step 2: Update worker contract expectations**

In `tests/verification/backtest_worker_contract.test.mjs`, assert the worker contains the new staged flow symbols:

```js
assert.match(worker, /per_symbol_top_n/);
assert.match(worker, /portfolio_top_n/);
assert.match(worker, /run_profit_first_staged_search/);
assert.match(worker, /relax_drawdown_limit/);
assert.match(worker, /reject_negative_return|positive_return/);
assert.match(worker, /build_portfolio_top3/);
assert.match(worker, /interval.*1m|"1m"/);
assert.match(worker, /usd_m_futures|futures/);
```

- [ ] **Step 3: Update console contract expectations**

In `tests/verification/backtest_console_contract.test.mjs`, assert the default page is human-readable and not manual-first:

```js
assert.match(wizardSource, /交易对|Symbols/);
assert.match(wizardSource, /方向|Direction/);
assert.match(wizardSource, /风险档位|Risk profile/);
assert.match(wizardSource, /系统自动搜索杠杆、间隔、倍率、层数、止盈、尾部保护、多空比例|automatically searches leverage/i);
assert.match(wizardSource, /per_symbol_top_n:\s*10/);
assert.match(wizardSource, /portfolio_top_n:\s*3/);
assert.doesNotMatch(wizardSource, /name="market"[\s\S]{0,300}<option value="spot"/);
```

- [ ] **Step 4: Add API normalization test**

In `apps/api-server/tests/martingale_backtest_flow.rs`, add a test named:

```rust
#[test]
fn martingale_auto_search_normalizes_profit_first_contract() {
    let payload = serde_json::json!({
        "symbols": ["BTCUSDT", "ETHUSDT"],
        "market": "spot",
        "direction": "long_short",
        "risk_profile": "conservative",
        "per_symbol_top_n": 5,
        "portfolio_top_n": 1,
        "time_range_mode": "manual"
    });

    let normalized = normalize_martingale_auto_search_config(payload).unwrap();

    assert_eq!(normalized["market"], "futures");
    assert_eq!(normalized["per_symbol_top_n"], 10);
    assert_eq!(normalized["portfolio_top_n"], 3);
    assert_eq!(normalized["time_range_mode"], "auto_since_2023_to_last_month_end");
    assert_eq!(normalized["search_mode"], "staged");
    assert_eq!(normalized["execution_model"], "conservative_futures_isolated");
}
```

If `normalize_martingale_auto_search_config` does not exist yet, Task 4 creates it.

- [ ] **Step 5: Run failing tests**

Run:

```bash
node tests/verification/martingale_backtest_rebuild_contract.test.mjs
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
cargo test -p api-server martingale_auto_search_normalizes_profit_first_contract -- --nocapture
```

Expected: FAIL because implementation has not been updated yet.

- [ ] **Step 6: Commit tests**

```bash
git add tests/verification/martingale_backtest_rebuild_contract.test.mjs tests/verification/backtest_worker_contract.test.mjs tests/verification/backtest_console_contract.test.mjs apps/api-server/tests/martingale_backtest_flow.rs
git commit -m "test: 问题描述 锁定马丁收益优先自动搜索契约"
```

---

## Task 2: Implement 0–100 Positive-Only Scoring

**Files:**
- Modify: `apps/backtest-engine/src/martingale/scoring.rs`
- Modify: `apps/backtest-engine/src/martingale/metrics.rs`
- Modify: `apps/backtest-engine/tests/search_scoring_time_splits.rs`

- [ ] **Step 1: Add scoring tests**

In `apps/backtest-engine/tests/search_scoring_time_splits.rs`, add tests:

```rust
#[test]
fn scoring_rejects_negative_return_candidates() {
    let mut result = fixture_martingale_result();
    result.metrics.total_return_pct = -0.01;
    result.metrics.annualized_return_pct = Some(-0.02);

    let score = score_candidate(&result, &ScoringConfig::default());

    assert!(!score.survival_valid);
    assert!(score.rejection_reasons.iter().any(|reason| reason == "negative_return"));
    assert_eq!(score.rank_score, 0.0);
}

#[test]
fn scoring_outputs_human_readable_zero_to_one_hundred_score() {
    let mut result = fixture_martingale_result();
    result.metrics.total_return_pct = 42.0;
    result.metrics.annualized_return_pct = Some(30.0);
    result.metrics.max_drawdown_pct = 12.0;
    result.metrics.global_drawdown_pct = Some(12.0);
    result.metrics.trade_count = 240;
    result.metrics.stop_count = 1;

    let score = score_candidate(&result, &ScoringConfig::default());

    assert!(score.survival_valid);
    assert!(score.rank_score >= 0.0 && score.rank_score <= 100.0);
    assert!(score.raw_score >= 0.0 && score.raw_score <= 100.0);
}
```

Create `fixture_martingale_result()` in the same test file if absent, filling only required fields with safe defaults copied from existing fixtures.

- [ ] **Step 2: Run tests to verify failure**

```bash
cargo test -p backtest-engine scoring_rejects_negative_return_candidates scoring_outputs_human_readable_zero_to_one_hundred_score -- --nocapture
```

Expected: FAIL because current scoring uses large rank bases and does not hard-reject negative return.

- [ ] **Step 3: Replace scoring formula**

In `apps/backtest-engine/src/martingale/scoring.rs`, remove `VALID_RANK_BASE`, `INVALID_RANK_BASE`, and `RANK_SCORE_SPREAD` usage. Implement:

```rust
fn clamp_score(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 100.0)
    } else {
        0.0
    }
}

fn return_drawdown_ratio(total_return_pct: f64, drawdown_pct: f64) -> f64 {
    if total_return_pct <= 0.0 {
        0.0
    } else {
        total_return_pct / drawdown_pct.max(1.0)
    }
}
```

Inside `score_candidate()` add hard rejection:

```rust
if metrics.total_return_pct <= 0.0 {
    push_reason(&mut reasons, "negative_return");
}
```

Set invalid candidates to score zero:

```rust
if !survival_valid {
    return CandidateScore {
        survival_valid,
        rank_score: 0.0,
        raw_score: 0.0,
        rejection_reasons: reasons,
    };
}
```

Use a 100-point formula:

```rust
let ratio = return_drawdown_ratio(metrics.total_return_pct, drawdown);
let annualized = metrics.annualized_return_pct.unwrap_or(metrics.total_return_pct);
let stop_penalty = stop_frequency * 20.0;
let leverage_penalty = metrics.max_leverage_used.unwrap_or(1.0).max(1.0).ln() * 4.0;
let liquidation_penalty = metrics.min_liquidation_buffer_pct.unwrap_or(100.0).lt(&15.0) as i32 as f64 * 20.0;

let raw_score = 35.0 * (ratio / 4.0).clamp(0.0, 1.0)
    + 25.0 * (annualized / 80.0).clamp(0.0, 1.0)
    + 20.0 * ((100.0 - drawdown) / 100.0).clamp(0.0, 1.0)
    + 10.0 * (metrics.monthly_win_rate_pct.unwrap_or(50.0) / 100.0).clamp(0.0, 1.0)
    + 10.0 * trade_stability
    - stop_penalty
    - leverage_penalty
    - liquidation_penalty;
let raw_score = clamp_score(raw_score);
let rank_score = raw_score;
```

If `max_leverage_used`, `min_liquidation_buffer_pct`, or `monthly_win_rate_pct` do not exist, add them as optional fields in `MartingaleMetrics` with serde defaults in Step 4.

- [ ] **Step 4: Add optional metrics fields**

In `apps/backtest-engine/src/martingale/metrics.rs`, extend `MartingaleMetrics`:

```rust
#[serde(default)]
pub annualized_return_pct: Option<f64>,
#[serde(default)]
pub monthly_win_rate_pct: Option<f64>,
#[serde(default)]
pub max_leverage_used: Option<f64>,
#[serde(default)]
pub min_liquidation_buffer_pct: Option<f64>,
#[serde(default)]
pub total_fee_quote: Option<f64>,
#[serde(default)]
pub total_slippage_quote: Option<f64>,
#[serde(default)]
pub planned_margin_quote: Option<f64>,
```

Update constructors/default fixtures in the file to initialize these with `None` or computed values.

- [ ] **Step 5: Run scoring tests**

```bash
cargo test -p backtest-engine scoring_rejects_negative_return_candidates scoring_outputs_human_readable_zero_to_one_hundred_score -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit scoring**

```bash
git add apps/backtest-engine/src/martingale/scoring.rs apps/backtest-engine/src/martingale/metrics.rs apps/backtest-engine/tests/search_scoring_time_splits.rs
git commit -m "fix: 修复思路 使用百分制并淘汰负收益马丁候选"
```

---

## Task 3: Add Risk Profile Drawdown Relaxation Rules

**Files:**
- Modify: `apps/backtest-engine/src/search.rs`
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `apps/backtest-engine/tests/search_scoring_time_splits.rs`

- [ ] **Step 1: Add tests for drawdown limits**

Add to `apps/backtest-engine/tests/search_scoring_time_splits.rs`:

```rust
#[test]
fn risk_profile_drawdown_limits_relax_only_one_step() {
    assert_eq!(drawdown_limit_sequence("conservative"), vec![20.0, 25.0]);
    assert_eq!(drawdown_limit_sequence("balanced"), vec![25.0, 30.0]);
    assert_eq!(drawdown_limit_sequence("aggressive"), vec![30.0]);
}

#[test]
fn unknown_risk_profile_defaults_to_balanced_limits() {
    assert_eq!(drawdown_limit_sequence("unknown"), vec![25.0, 30.0]);
}
```

- [ ] **Step 2: Implement drawdown sequence helper**

In `apps/backtest-engine/src/search.rs`, add:

```rust
pub fn drawdown_limit_sequence(risk_profile: &str) -> Vec<f64> {
    match risk_profile {
        "conservative" => vec![20.0, 25.0],
        "balanced" => vec![25.0, 30.0],
        "aggressive" => vec![30.0],
        _ => vec![25.0, 30.0],
    }
}
```

Export it through `lib.rs` if tests import from crate root.

- [ ] **Step 3: Wire worker scoring config**

In `apps/backtest-worker/src/main.rs`, when processing each symbol, loop:

```rust
for max_drawdown_pct in drawdown_limit_sequence(&task.config.risk_profile) {
    let scoring = ScoringConfig {
        max_global_drawdown_pct: max_drawdown_pct,
        max_strategy_drawdown_pct: max_drawdown_pct,
        ..ScoringConfig::default()
    };
    let candidates = run_profit_first_staged_search(..., &scoring, max_drawdown_pct).await?;
    if candidates.iter().any(|candidate| candidate.score.survival_valid) {
        used_drawdown_limit_pct = max_drawdown_pct;
        break;
    }
}
```

Persist `used_drawdown_limit_pct` and `risk_relaxed: used_drawdown_limit_pct > first_limit` in candidate summaries.

- [ ] **Step 4: Run tests**

```bash
cargo test -p backtest-engine risk_profile_drawdown_limits_relax_only_one_step unknown_risk_profile_defaults_to_balanced_limits -- --nocapture
cargo test -p backtest-worker -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-engine/src/search.rs apps/backtest-engine/src/lib.rs apps/backtest-engine/tests/search_scoring_time_splits.rs apps/backtest-worker/src/main.rs
git commit -m "feat: 修复思路 增加风险档位回撤逐级放宽规则"
```

---

## Task 4: Normalize API Task Payload

**Files:**
- Modify: `apps/api-server/src/services/backtest_service.rs`
- Modify: `apps/api-server/src/routes/backtest.rs`
- Modify: `apps/api-server/tests/martingale_backtest_flow.rs`

- [ ] **Step 1: Implement normalizer**

In `apps/api-server/src/services/backtest_service.rs`, add a public helper near task creation helpers:

```rust
pub fn normalize_martingale_auto_search_config(mut config: serde_json::Value) -> Result<serde_json::Value, String> {
    let object = config
        .as_object_mut()
        .ok_or_else(|| "backtest config must be a JSON object".to_owned())?;

    object.insert("market".to_owned(), serde_json::Value::String("futures".to_owned()));
    object.insert("per_symbol_top_n".to_owned(), serde_json::Value::Number(10.into()));
    object.insert("portfolio_top_n".to_owned(), serde_json::Value::Number(3.into()));
    object.insert("time_range_mode".to_owned(), serde_json::Value::String("auto_since_2023_to_last_month_end".to_owned()));
    object.insert("search_mode".to_owned(), serde_json::Value::String("staged".to_owned()));
    object.insert("execution_model".to_owned(), serde_json::Value::String("conservative_futures_isolated".to_owned()));
    object.insert("interval".to_owned(), serde_json::Value::String("1m".to_owned()));

    if !object.contains_key("symbols") {
        return Err("symbols are required".to_owned());
    }

    Ok(config)
}
```

- [ ] **Step 2: Call normalizer during task creation**

In the task creation path, before saving `config`, detect martingale auto search:

```rust
let normalized_config = if is_martingale_auto_search(&request.config) {
    normalize_martingale_auto_search_config(request.config.clone())?
} else {
    request.config.clone()
};
```

Add helper:

```rust
fn is_martingale_auto_search(config: &serde_json::Value) -> bool {
    config.get("strategy_type").and_then(|value| value.as_str()) == Some("martingale")
        || config.get("search_mode").and_then(|value| value.as_str()) == Some("staged")
        || config.get("risk_profile").is_some()
}
```

- [ ] **Step 3: Run API tests**

```bash
cargo test -p api-server martingale_auto_search_normalizes_profit_first_contract -- --nocapture
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/api-server/src/services/backtest_service.rs apps/api-server/src/routes/backtest.rs apps/api-server/tests/martingale_backtest_flow.rs
git commit -m "fix: 修复思路 统一马丁自动搜索任务契约"
```

---

## Task 5: Implement Profit-First Staged Search Spaces

**Files:**
- Modify: `apps/backtest-engine/src/search.rs`
- Modify: `apps/backtest-worker/src/main.rs`
- Add/modify: `apps/backtest-engine/tests/search_scoring_time_splits.rs`

- [ ] **Step 1: Add search-space tests**

Add tests:

```rust
#[test]
fn staged_search_space_covers_required_futures_ranges() {
    let space = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");

    assert!(space.leverage.contains(&2));
    assert!(space.leverage.contains(&10));
    assert!(space.spacing_bps.iter().any(|value| *value <= 80));
    assert!(space.spacing_bps.iter().any(|value| *value >= 220));
    assert!(space.order_multiplier.contains(&1.4));
    assert!(space.order_multiplier.contains(&2.0));
    assert!(space.max_legs.contains(&4));
    assert!(space.max_legs.contains(&8));
    assert!(space.long_short_weight_pct.contains(&(80, 20)));
    assert!(space.long_short_weight_pct.contains(&(50, 50)));
}

#[test]
fn fine_search_expands_around_coarse_winner() {
    let winner = CoarseParameterPoint {
        leverage: 4,
        spacing_bps: 120,
        order_multiplier: 1.6,
        max_legs: 5,
        take_profit_bps: 100,
        tail_stop_bps: 1800,
        long_weight_pct: 70,
        short_weight_pct: 30,
    };

    let fine = fine_space_around(&winner);

    assert!(fine.spacing_bps.contains(&100));
    assert!(fine.spacing_bps.contains(&120));
    assert!(fine.spacing_bps.contains(&150));
    assert!(fine.max_legs.contains(&4));
    assert!(fine.max_legs.contains(&6));
    assert!(fine.long_short_weight_pct.contains(&(65, 35)));
    assert!(fine.long_short_weight_pct.contains(&(75, 25)));
}
```

- [ ] **Step 2: Add types and builders**

In `apps/backtest-engine/src/search.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct StagedMartingaleSearchSpace {
    pub leverage: Vec<u32>,
    pub spacing_bps: Vec<u32>,
    pub order_multiplier: Vec<f64>,
    pub max_legs: Vec<u32>,
    pub take_profit_bps: Vec<u32>,
    pub tail_stop_bps: Vec<u32>,
    pub long_short_weight_pct: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoarseParameterPoint {
    pub leverage: u32,
    pub spacing_bps: u32,
    pub order_multiplier: f64,
    pub max_legs: u32,
    pub take_profit_bps: u32,
    pub tail_stop_bps: u32,
    pub long_weight_pct: u32,
    pub short_weight_pct: u32,
}
```

Implement:

```rust
impl StagedMartingaleSearchSpace {
    pub fn for_profile(risk_profile: &str, direction: &str) -> Self {
        let mut space = match risk_profile {
            "conservative" => Self {
                leverage: vec![2, 3, 4, 5, 6],
                spacing_bps: vec![120, 160, 220, 300, 420],
                order_multiplier: vec![1.25, 1.4, 1.6],
                max_legs: vec![3, 4, 5, 6],
                take_profit_bps: vec![60, 80, 100, 130],
                tail_stop_bps: vec![1500, 2000, 2500],
                long_short_weight_pct: vec![(80, 20), (70, 30), (60, 40)],
            },
            "aggressive" => Self {
                leverage: vec![3, 4, 5, 6, 8, 10],
                spacing_bps: vec![50, 80, 120, 160, 220],
                order_multiplier: vec![1.4, 1.6, 2.0, 2.4],
                max_legs: vec![4, 5, 6, 8],
                take_profit_bps: vec![80, 100, 130, 180, 240],
                tail_stop_bps: vec![2000, 2500, 3000],
                long_short_weight_pct: vec![(60, 40), (50, 50), (40, 60)],
            },
            _ => Self {
                leverage: vec![2, 3, 4, 5, 6, 8, 10],
                spacing_bps: vec![80, 120, 160, 220, 300],
                order_multiplier: vec![1.25, 1.4, 1.6, 2.0],
                max_legs: vec![4, 5, 6, 8],
                take_profit_bps: vec![80, 100, 130, 180],
                tail_stop_bps: vec![1800, 2200, 2600],
                long_short_weight_pct: vec![(80, 20), (70, 30), (60, 40), (50, 50)],
            },
        };

        if direction == "long" || direction == "long_only" {
            space.long_short_weight_pct = vec![(100, 0)];
        }
        if direction == "short" || direction == "short_only" {
            space.long_short_weight_pct = vec![(0, 100)];
        }
        space
    }
}
```

Implement `fine_space_around()` with bounded neighbor values exactly as tested.

- [ ] **Step 3: Worker uses staged spaces**

In `apps/backtest-worker/src/main.rs`, add a function:

```rust
async fn run_profit_first_staged_search(
    context: &MarketDataContext,
    symbol: &str,
    task: &WorkerTaskConfig,
    scoring: &ScoringConfig,
    drawdown_limit_pct: f64,
) -> Result<Vec<EvaluatedCandidate>, String> {
    let coarse_space = StagedMartingaleSearchSpace::for_profile(&task.risk_profile, task.direction_mode.as_deref().unwrap_or("long"));
    let coarse = evaluate_parameter_space(context, symbol, &coarse_space, task, scoring, drawdown_limit_pct, "coarse").await?;
    let survivors = select_diverse_coarse_survivors(coarse, 24);
    let mut refined = Vec::new();
    for survivor in survivors {
        let fine_space = fine_space_around(&survivor.parameter_point);
        refined.extend(evaluate_parameter_space(context, symbol, &fine_space, task, scoring, drawdown_limit_pct, "fine").await?);
    }
    Ok(select_top_positive_candidates(refined, task.per_symbol_top_n.max(10)))
}
```

If helper types need to live in engine instead of worker, put them in `search.rs` and import.

- [ ] **Step 4: Run tests**

```bash
cargo test -p backtest-engine staged_search_space_covers_required_futures_ranges fine_search_expands_around_coarse_winner -- --nocapture
cargo test -p backtest-worker -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-engine/src/search.rs apps/backtest-engine/tests/search_scoring_time_splits.rs apps/backtest-worker/src/main.rs
git commit -m "feat: 修复思路 增加马丁分阶段参数搜索空间"
```

---

## Task 6: Fix Futures-Isolated Margin, Costs, and Full 1m Validation

**Files:**
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Modify: `apps/backtest-engine/src/martingale/trade_engine.rs`
- Modify: `apps/backtest-engine/src/martingale/metrics.rs`
- Modify: `apps/backtest-worker/src/main.rs`
- Add/modify: `apps/backtest-engine/tests/search_scoring_time_splits.rs`

- [ ] **Step 1: Add leverage accounting test**

Add:

```rust
#[test]
fn leveraged_margin_return_uses_planned_total_margin_not_first_order_only() {
    let plan = planned_margin_quote(10.0, 2.0, 4);
    assert_eq!(plan, 150.0);

    let pnl = leveraged_position_pnl_quote(10.0, 2.0, 0.01);
    assert_eq!(pnl, 0.2);

    let return_pct = pnl / plan * 100.0;
    assert!((return_pct - 0.13333333333333333).abs() < 0.000001);
}
```

- [ ] **Step 2: Implement helper functions**

In `apps/backtest-engine/src/martingale/metrics.rs` or a focused existing module, add:

```rust
pub fn planned_margin_quote(first_margin_quote: f64, order_multiplier: f64, max_legs: u32) -> f64 {
    (0..max_legs)
        .map(|leg| first_margin_quote * order_multiplier.powi(leg as i32))
        .sum()
}

pub fn leveraged_position_pnl_quote(margin_quote: f64, leverage: f64, price_move_pct: f64) -> f64 {
    margin_quote * leverage * price_move_pct
}
```

- [ ] **Step 3: Apply cost model**

In `kline_engine.rs` and `trade_engine.rs`, ensure every simulated fill applies:

```rust
let notional_quote = margin_quote * leverage;
let fee_quote = notional_quote * fee_rate;
let slippage_quote = notional_quote * slippage_bps / 10_000.0;
realized_pnl_quote -= fee_quote + slippage_quote;
metrics.total_fee_quote = Some(metrics.total_fee_quote.unwrap_or(0.0) + fee_quote);
metrics.total_slippage_quote = Some(metrics.total_slippage_quote.unwrap_or(0.0) + slippage_quote);
```

Use existing fee/slippage config values if already present; otherwise add conservative defaults in the martingale config summary:

```rust
const DEFAULT_TAKER_FEE_RATE: f64 = 0.0005;
const DEFAULT_SLIPPAGE_BPS: f64 = 2.0;
```

- [ ] **Step 4: Enforce full 1m final validation**

In `apps/backtest-worker/src/main.rs`, force:

```rust
let interval = "1m";
let bars = context.load_bars(symbol, interval, task.start_ms, task.end_ms).await?;
if bars.len() < minimum_required_1m_bars(task.start_ms, task.end_ms) {
    return Err(format!("insufficient 1m data for {symbol}"));
}
```

Define minimum coverage as at least 85% of expected minutes:

```rust
fn minimum_required_1m_bars(start_ms: i64, end_ms: i64) -> usize {
    (((end_ms - start_ms).max(0) / 60_000) as f64 * 0.85) as usize
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p backtest-engine leveraged_margin_return_uses_planned_total_margin_not_first_order_only -- --nocapture
cargo test -p backtest-worker -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-engine/src/martingale/kline_engine.rs apps/backtest-engine/src/martingale/trade_engine.rs apps/backtest-engine/src/martingale/metrics.rs apps/backtest-engine/tests/search_scoring_time_splits.rs apps/backtest-worker/src/main.rs
git commit -m "fix: 修复思路 按逐仓保证金和真实成本计算马丁收益"
```

---

## Task 7: Implement Real Long + Short Dual-Leg Simulation Contract

**Files:**
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Modify: `apps/backtest-engine/src/martingale/trade_engine.rs`
- Modify: `apps/backtest-worker/src/main.rs`
- Add/modify: `apps/backtest-engine/tests/search_scoring_time_splits.rs`

- [ ] **Step 1: Add dual-direction test**

Add:

```rust
#[test]
fn long_short_config_keeps_independent_leg_parameters() {
    let config = build_long_short_config_for_test(
        LegParameters { spacing_bps: 120, take_profit_bps: 90, max_legs: 5, weight_pct: 70 },
        LegParameters { spacing_bps: 180, take_profit_bps: 120, max_legs: 4, weight_pct: 30 },
    );

    assert_eq!(config.long_leg.as_ref().unwrap().spacing_bps, 120);
    assert_eq!(config.short_leg.as_ref().unwrap().spacing_bps, 180);
    assert_eq!(config.long_leg.as_ref().unwrap().weight_pct, 70);
    assert_eq!(config.short_leg.as_ref().unwrap().weight_pct, 30);
}
```

- [ ] **Step 2: Ensure config summary supports separate legs**

When worker builds candidate config for `long_short`, include:

```json
{
  "direction": "long_short",
  "long_leg": {
    "spacing_bps": 120,
    "order_multiplier": 1.6,
    "max_legs": 5,
    "take_profit_bps": 90,
    "tail_stop_bps": 1800,
    "weight_pct": 70
  },
  "short_leg": {
    "spacing_bps": 180,
    "order_multiplier": 1.4,
    "max_legs": 4,
    "take_profit_bps": 120,
    "tail_stop_bps": 2200,
    "weight_pct": 30
  }
}
```

- [ ] **Step 3: Simulate both legs**

In the engine, when direction is long-short:

```rust
let long_result = simulate_leg(Direction::Long, &config.long_leg, bars, capital * long_weight)?;
let short_result = simulate_leg(Direction::Short, &config.short_leg, bars, capital * short_weight)?;
let combined = combine_leg_results(long_result, short_result)?;
```

`combine_leg_results()` must:

```rust
combined.total_return_quote = long.total_return_quote + short.total_return_quote;
combined.total_fee_quote = long.total_fee_quote + short.total_fee_quote;
combined.total_slippage_quote = long.total_slippage_quote + short.total_slippage_quote;
combined.trade_count = long.trade_count + short.trade_count;
combined.equity_curve = merge_equity_curves_by_timestamp(long.equity_curve, short.equity_curve);
combined.max_drawdown_pct = max_drawdown_from_curve(&combined.equity_curve);
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p backtest-engine long_short_config_keeps_independent_leg_parameters -- --nocapture
cargo test -p backtest-engine -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-engine/src/martingale/kline_engine.rs apps/backtest-engine/src/martingale/trade_engine.rs apps/backtest-worker/src/main.rs apps/backtest-engine/tests/search_scoring_time_splits.rs
git commit -m "fix: 复现路径 多空双向回测同时模拟独立多空腿"
```

---

## Task 8: Build Portfolio Top 3 Optimizer

**Files:**
- Create: `apps/backtest-engine/src/martingale/portfolio_search.rs`
- Modify: `apps/backtest-engine/src/martingale/mod.rs`
- Modify: `apps/backtest-worker/src/main.rs`
- Add: `apps/backtest-engine/tests/portfolio_search.rs`

- [ ] **Step 1: Add portfolio optimizer tests**

Create `apps/backtest-engine/tests/portfolio_search.rs`:

```rust
use backtest_engine::martingale::portfolio_search::{build_portfolio_top3, PortfolioCandidateInput};

#[test]
fn portfolio_top3_uses_positive_candidates_and_weights_sum_to_one_hundred() {
    let inputs = vec![
        input("BTCUSDT", "btc-a", 40.0, 12.0, 88.0),
        input("BTCUSDT", "btc-b", 32.0, 10.0, 84.0),
        input("ETHUSDT", "eth-a", 35.0, 11.0, 86.0),
        input("SOLUSDT", "sol-a", 55.0, 22.0, 80.0),
    ];

    let portfolios = build_portfolio_top3(&inputs, 25.0);

    assert!(!portfolios.is_empty());
    assert!(portfolios.len() <= 3);
    for portfolio in portfolios {
        assert_eq!(portfolio.weights.iter().map(|weight| weight.weight_pct).sum::<u32>(), 100);
        assert!(portfolio.total_return_pct > 0.0);
        assert!(portfolio.max_drawdown_pct <= 25.0);
        assert!(portfolio.score >= 0.0 && portfolio.score <= 100.0);
    }
}

fn input(symbol: &str, id: &str, annualized: f64, drawdown: f64, score: f64) -> PortfolioCandidateInput {
    PortfolioCandidateInput {
        symbol: symbol.to_owned(),
        candidate_id: id.to_owned(),
        annualized_return_pct: annualized,
        total_return_pct: annualized,
        max_drawdown_pct: drawdown,
        score,
        equity_curve: Vec::new(),
    }
}
```

- [ ] **Step 2: Implement optimizer data types**

In `portfolio_search.rs`:

```rust
#[derive(Debug, Clone)]
pub struct PortfolioCandidateInput {
    pub symbol: String,
    pub candidate_id: String,
    pub annualized_return_pct: f64,
    pub total_return_pct: f64,
    pub max_drawdown_pct: f64,
    pub score: f64,
    pub equity_curve: Vec<(i64, f64)>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PortfolioWeight {
    pub symbol: String,
    pub candidate_id: String,
    pub weight_pct: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PortfolioSearchResult {
    pub rank: usize,
    pub score: f64,
    pub annualized_return_pct: f64,
    pub total_return_pct: f64,
    pub max_drawdown_pct: f64,
    pub return_drawdown_ratio: f64,
    pub weights: Vec<PortfolioWeight>,
}
```

- [ ] **Step 3: Implement deterministic combination search**

Implement `build_portfolio_top3(inputs, max_drawdown_pct)`:

```rust
pub fn build_portfolio_top3(inputs: &[PortfolioCandidateInput], max_drawdown_pct: f64) -> Vec<PortfolioSearchResult> {
    let positive: Vec<_> = inputs
        .iter()
        .filter(|candidate| candidate.total_return_pct > 0.0 && candidate.max_drawdown_pct <= max_drawdown_pct)
        .collect();

    let mut portfolios = Vec::new();
    for size in 1..=positive.len().min(8) {
        for group in combinations(&positive, size) {
            for weights in candidate_weight_grids(&group) {
                if weights.iter().sum::<u32>() != 100 { continue; }
                if violates_single_strategy_limit(&weights, 20) { continue; }
                if violates_symbol_limit(&group, &weights, 35) { continue; }
                let result = score_portfolio(&group, &weights, max_drawdown_pct);
                if result.total_return_pct > 0.0 && result.max_drawdown_pct <= max_drawdown_pct {
                    portfolios.push(result);
                }
            }
        }
    }
    portfolios.sort_by(|left, right| right.score.partial_cmp(&left.score).unwrap_or(std::cmp::Ordering::Equal));
    portfolios.truncate(3);
    for (index, portfolio) in portfolios.iter_mut().enumerate() {
        portfolio.rank = index + 1;
    }
    portfolios
}
```

Use deterministic small weight grids: 5%, 10%, 15%, 20%; normalize selected groups to total 100. This is enough for first implementation and testable.

- [ ] **Step 4: Export module**

In `apps/backtest-engine/src/martingale/mod.rs`:

```rust
pub mod portfolio_search;
```

- [ ] **Step 5: Worker writes portfolio artifact**

In `apps/backtest-worker/src/main.rs`, after all per-symbol candidates are saved:

```rust
let portfolio_inputs = build_portfolio_inputs(&saved_candidates);
let portfolio_top3 = build_portfolio_top3(&portfolio_inputs, selected_portfolio_drawdown_limit_pct);
write_task_json_artifact(&config.artifact_root, &task.task_id, "portfolio_top3.json", &portfolio_top3)?;
poller.heartbeat_with_summary(&task.task_id, "portfolio_top3_ready", json!({
    "portfolio_top_n": 3,
    "portfolio_top3": portfolio_top3,
})).await?;
```

- [ ] **Step 6: Run tests**

```bash
cargo test -p backtest-engine portfolio_top3_uses_positive_candidates_and_weights_sum_to_one_hundred -- --nocapture
cargo test -p backtest-worker -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add apps/backtest-engine/src/martingale/portfolio_search.rs apps/backtest-engine/src/martingale/mod.rs apps/backtest-engine/tests/portfolio_search.rs apps/backtest-worker/src/main.rs
git commit -m "feat: 修复思路 从单策略候选生成组合Top3"
```

---

## Task 9: Simplify Wizard and Payload

**Files:**
- Modify: `apps/web/components/backtest/backtest-wizard.tsx`
- Modify: `apps/web/components/backtest/martingale-parameter-editor.tsx`
- Modify: `tests/verification/martingale_backtest_rebuild_contract.test.mjs`
- Modify: `tests/verification/backtest_console_contract.test.mjs`

- [ ] **Step 1: Update wizard defaults**

In `backtest-wizard.tsx`, set defaults:

```ts
const DEFAULT_FORM: WizardForm = {
  whitelist: "BTCUSDT, ETHUSDT",
  blacklist: "",
  market: "usd_m_futures",
  directionMode: "long_and_short",
  parameterPreset: "conservative",
  timeMode: "auto_recent",
  trainStart: "2023-01-01",
  perSymbolTopN: 10,
  portfolioTopN: 3,
};
```

If `WizardForm` does not have `perSymbolTopN` and `portfolioTopN`, add them.

- [ ] **Step 2: Remove spot selector from default panel**

In `AutomaticSearchPanel`, remove the market select and replace with a read-only badge:

```tsx
<div className="rounded-xl border border-border bg-muted/30 p-3 text-sm">
  <p className="text-xs uppercase tracking-wide text-muted-foreground">Market</p>
  <p className="font-semibold">USDT-M Futures · 逐仓模型</p>
</div>
```

Keep any old market editor only inside advanced/manual panel if needed for legacy tasks, but default create payload must always use futures.

- [ ] **Step 3: Build profit-first payload**

Update `buildWizardPayload()`:

```ts
return {
  strategy_type: "martingale",
  symbols,
  market: "futures",
  direction: directionForPayload(form.directionMode),
  direction_mode: form.directionMode,
  risk_profile: form.parameterPreset === "custom" ? "balanced" : form.parameterPreset,
  per_symbol_top_n: 10,
  portfolio_top_n: 3,
  time_range_mode: "auto_since_2023_to_last_month_end",
  search_mode: "staged",
  execution_model: "conservative_futures_isolated",
  interval: "1m",
  start_date: "2023-01-01",
  end_date: autoRange.testEnd,
};
```

- [ ] **Step 4: Update user text**

Replace Top 5 wording with:

```tsx
系统会自动搜索杠杆、间隔、倍率、层数、止盈、尾部保护、多空比例，并输出每个币种 Top 10 与组合 Top 3。
```

- [ ] **Step 5: Run frontend contract tests**

```bash
node tests/verification/martingale_backtest_rebuild_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/web/components/backtest/backtest-wizard.tsx apps/web/components/backtest/martingale-parameter-editor.tsx tests/verification/martingale_backtest_rebuild_contract.test.mjs tests/verification/backtest_console_contract.test.mjs
git commit -m "feat: 修复思路 简化马丁自动搜索入口"
```

---

## Task 10: Upgrade Result UI for Top 10, Top 3, Charts, and Trades

**Files:**
- Modify: `apps/web/components/backtest/backtest-result-table.tsx`
- Modify: `apps/web/components/backtest/backtest-charts.tsx`
- Modify: `apps/web/components/backtest/portfolio-candidate-review.tsx`
- Modify: `apps/web/components/backtest/backtest-console.tsx`
- Modify: `tests/verification/martingale_backtest_rebuild_contract.test.mjs`

- [ ] **Step 1: Render Top 10 per symbol**

In `backtest-result-table.tsx`, change grouping slice:

```ts
.slice(0, 10)
```

Change labels to `每个币种 Top 10` / `Per-symbol Top 10`.

- [ ] **Step 2: Add readable metrics columns**

Extend `candidateColumns()` with:

```ts
{ key: "annualized", label: pickText(lang, "年化收益", "Annualized"), align: "right" as const },
{ key: "returnDrawdownRatio", label: pickText(lang, "收益回撤比", "Return/DD"), align: "right" as const },
{ key: "leverage", label: pickText(lang, "杠杆", "Leverage"), align: "right" as const },
{ key: "cost", label: pickText(lang, "成本", "Cost"), align: "right" as const },
```

In `candidateRow()` map:

```ts
annualized: formatPct(candidate.summary?.annualized_return_pct),
returnDrawdownRatio: formatRatio(candidate.summary?.return_drawdown_ratio),
leverage: `${candidate.summary?.leverage ?? candidate.summary?.recommended_leverage ?? "—"}x`,
cost: formatQuote(candidate.summary?.total_fee_quote + candidate.summary?.total_slippage_quote),
score: formatScore100(candidate.score),
```

Add helpers:

```ts
function formatScore100(value: unknown) {
  const numeric = Number(value);
  return Number.isFinite(numeric) ? `${numeric.toFixed(1)}/100` : "—";
}
```

- [ ] **Step 3: Add portfolio Top 3 panel**

In `portfolio-candidate-review.tsx`, add a section that reads `portfolioTop3` prop or task summary:

```tsx
<h3 className="text-base font-semibold">{pickText(lang, "组合 Top 3", "Portfolio Top 3")}</h3>
```

Render rank, score, annualized return, max drawdown, return/DD, and weights.

- [ ] **Step 4: Improve chart tooltip**

In `backtest-charts.tsx`, ensure hover title/value text includes:

```ts
`${point.date} · 收益 ${formatPct(point.returnPct)} · 回撤 ${formatPct(point.drawdownPct)}`
```

If current charts are SVG-only, add `<title>` inside each point/segment so native browser tooltip works without adding dependencies.

- [ ] **Step 5: Show failure reasons and delete actions**

In `backtest-console.tsx`, when task summary contains `failed_symbols` or `rejection_reasons`, render:

```tsx
<section className="rounded-2xl border border-amber-500/30 bg-amber-500/5 p-4">
  <h3 className="font-semibold">未入选原因</h3>
  ...
</section>
```

Ensure existing delete/cancel task action remains visible for completed tasks.

- [ ] **Step 6: Run UI tests and build**

```bash
node tests/verification/martingale_backtest_rebuild_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
pnpm --filter web exec next build --webpack
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add apps/web/components/backtest/backtest-result-table.tsx apps/web/components/backtest/backtest-charts.tsx apps/web/components/backtest/portfolio-candidate-review.tsx apps/web/components/backtest/backtest-console.tsx tests/verification/martingale_backtest_rebuild_contract.test.mjs
git commit -m "feat: 修复思路 展示马丁Top10和组合Top3结果"
```

---

## Task 11: End-to-End Worker Result Validation

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`
- Optional modify: `docs/user-guide/zh/martingale-backtest.md`

- [ ] **Step 1: Add final result guards in worker**

Before saving any candidate:

```rust
if result.metrics.total_return_pct <= 0.0 {
    rejected.push(rejection(symbol, "negative_return", result.metrics.total_return_pct));
    continue;
}
if result.metrics.max_drawdown_pct > used_drawdown_limit_pct {
    rejected.push(rejection(symbol, "drawdown_exceeded", result.metrics.max_drawdown_pct));
    continue;
}
if result.rejection_reasons.iter().any(|reason| reason.contains("liquidation")) {
    rejected.push(rejection(symbol, "liquidation_risk", 0.0));
    continue;
}
```

- [ ] **Step 2: Ensure per-symbol Top 10 grouping**

After evaluating each symbol:

```rust
let mut symbol_candidates = select_top_positive_candidates(symbol_results, 10);
for (index, candidate) in symbol_candidates.iter_mut().enumerate() {
    candidate.summary["parameter_rank_for_symbol"] = json!(index + 1);
    candidate.summary["per_symbol_top_n"] = json!(10);
}
```

- [ ] **Step 3: Persist failed symbol summaries**

At task completion summary:

```rust
json!({
  "completed_symbols": completed_symbols,
  "failed_symbols": failed_symbols,
  "per_symbol_top_n": 10,
  "portfolio_top_n": 3,
  "positive_only": true,
  "execution_model": "conservative_futures_isolated"
})
```

- [ ] **Step 4: Run worker contract and Rust tests**

```bash
node tests/verification/backtest_worker_contract.test.mjs
cargo test -p backtest-worker -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-worker/src/main.rs tests/verification/backtest_worker_contract.test.mjs docs/user-guide/zh/martingale-backtest.md
git commit -m "fix: 修复思路 Worker只保存正收益完整验证候选"
```

---

## Task 12: Full Verification and One Real Backtest Smoke Run

**Files:**
- No source change unless verification exposes a defect.

- [ ] **Step 1: Run backend tests**

```bash
cargo test -p backtest-engine -- --nocapture
cargo test -p backtest-worker -- --nocapture
cargo test -p api-server martingale -- --nocapture
```

Expected: PASS.

- [ ] **Step 2: Run frontend contract tests**

```bash
node tests/verification/martingale_backtest_rebuild_contract.test.mjs
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
```

Expected: PASS.

- [ ] **Step 3: Run frontend build**

```bash
pnpm --filter web exec next build --webpack
```

Expected: PASS.

- [ ] **Step 4: Start/restart only grid services if needed**

Before touching services, inspect ports:

```bash
ss -ltnp | rg ':8080|:3000|:8081|:8000'
```

Do not kill or restart port `3000` unless the user explicitly asks.

- [ ] **Step 5: Create a two-symbol smoke backtest**

Use the app/API path already used by the project. Payload:

```json
{
  "strategy_type": "martingale",
  "symbols": ["BTCUSDT", "ETHUSDT"],
  "market": "futures",
  "direction": "long_short",
  "risk_profile": "conservative",
  "per_symbol_top_n": 10,
  "portfolio_top_n": 3,
  "time_range_mode": "auto_since_2023_to_last_month_end",
  "search_mode": "staged",
  "execution_model": "conservative_futures_isolated",
  "interval": "1m"
}
```

Expected result:

- Task completes or reports clear data insufficiency.
- Saved candidates, if any, are positive return only.
- BTC and ETH each show up to Top 10.
- Portfolio Top 3 appears when at least one valid combination exists.
- Failed symbols show explicit reasons.

- [ ] **Step 6: Document observed default parameters**

In final response, summarize:

- User-editable inputs.
- System-searched parameters.
- Risk drawdown limits and relaxation.
- Cost model defaults.
- Ranking formula.
- Where to view Top 10, Top 3, curves, trades, and delete results.

- [ ] **Step 7: Commit any verification fixes**

If fixes were needed:

```bash
git add <changed files>
git commit -m "fix: 修复思路 完成马丁收益优先回测验证"
```

---

## Final Gate

Before claiming completion:

```bash
git status --short --branch
cargo test -p backtest-engine -- --nocapture
cargo test -p backtest-worker -- --nocapture
node tests/verification/martingale_backtest_rebuild_contract.test.mjs
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
pnpm --filter web exec next build --webpack
```

Expected:

- Git status clean except intentional unpushed commits.
- All tests pass.
- Build passes.
- If a real smoke task cannot produce positive strategies because market data is insufficient or strategy is genuinely unprofitable, UI must show that reason clearly instead of fabricating positive results.
