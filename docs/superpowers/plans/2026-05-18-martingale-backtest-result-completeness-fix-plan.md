# Martingale Backtest Result Completeness Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix Martingale backtest outputs so `long_short`, annualized metrics, charts, trade details, leverage-aware capital accounting, and true multi-strategy portfolio backtests are complete and verifiable.

**Architecture:** Add contract tests first, then fix engine metrics/artifacts, rebuild real portfolio search from weighted candidate curves, persist richer summaries through worker/API, and expose candidate/portfolio details in the web console. Portfolio Top 3 must be generated from multi-member allocations, not by selecting individual candidates.

**Tech Stack:** Rust workspace (`backtest-engine`, `backtest-worker`, `api-server`), Next.js/TypeScript web UI, Node contract tests under `tests/verification`.

---

## File Structure

- `apps/backtest-engine/src/martingale/metrics.rs`: canonical annualized return, drawdown curve, planned-margin, leverage-aware helper functions, trade detail structs.
- `apps/backtest-engine/src/martingale/trade_engine.rs` and `apps/backtest-engine/src/martingale/kline_engine.rs`: ensure long/short legs execute and emit equity/trade artifacts.
- `apps/backtest-engine/src/search.rs`: generate real `long_short` candidates with both long and short strategy legs.
- `apps/backtest-engine/src/portfolio_search.rs`: replace single-candidate Top 3 selection with weighted multi-member portfolio construction.
- `apps/backtest-worker/src/main.rs`: persist richer candidate artifacts, enough eligible candidates, and real portfolio Top 3 artifacts.
- `apps/api-server/src/services/backtest_service.rs`: expose candidate and portfolio detail fields without dropping curves/trades/legs/leverage.
- `apps/web/components/backtest/backtest-result-table.tsx`: show annualized return, leverage, candidate details, and portfolio details entry.
- `apps/web/components/backtest/backtest-charts.tsx`: render equity/drawdown tooltip and trade details for both candidate and portfolio summaries.
- `apps/web/components/backtest/portfolio-candidate-review.tsx`: show true portfolio members and allocations.
- `tests/verification/backtest_worker_contract.test.mjs`: contract coverage for long_short, annualized, artifacts, true portfolio.
- `tests/verification/backtest_console_contract.test.mjs`: UI contract coverage for charts/details/portfolio allocation.
- `apps/backtest-engine/tests/search_scoring_time_splits.rs`: engine tests for leverage math, annualized return, long_short generation, portfolio combination.

---

## Task 1: Lock Failing Contracts

**Files:**
- Modify: `tests/verification/backtest_worker_contract.test.mjs`
- Modify: `tests/verification/backtest_console_contract.test.mjs`
- Modify: `apps/backtest-engine/tests/search_scoring_time_splits.rs`

- [ ] **Step 1: Add worker contract checks**

Append tests to `tests/verification/backtest_worker_contract.test.mjs`:

```js
test("worker emits complete martingale artifacts and true portfolio combinations", () => {
  const worker = read("apps/backtest-worker/src/main.rs");
  const portfolio = read("apps/backtest-engine/src/portfolio_search.rs");
  const metrics = read("apps/backtest-engine/src/martingale/metrics.rs");

  assert.match(worker, /annualized_return_pct/);
  assert.match(worker, /drawdown_curve/);
  assert.match(worker, /trades_preview|trade_details|trades/);
  assert.match(worker, /eligible_candidates|eligible_candidate_count/);
  assert.match(worker, /long_short|LongShort|MartingaleDirectionMode::LongShort/);
  assert.match(worker, /planned_margin_quote/);
  assert.match(worker, /max_leverage_used|leverage/);

  assert.match(portfolio, /PortfolioMember/);
  assert.match(portfolio, /allocation_pct/);
  assert.match(portfolio, /combine_equity_curves|weighted_portfolio_equity/);
  assert.match(portfolio, /member_count/);
  assert.doesNotMatch(portfolio, /ranked\.iter\(\)\.take\(3\).*cloned/s);

  assert.match(metrics, /calculate_annualized_return_pct/);
  assert.match(metrics, /planned_margin_quote/);
  assert.match(metrics, /notional_quote/);
});
```

- [ ] **Step 2: Add web contract checks**

Append tests to `tests/verification/backtest_console_contract.test.mjs`:

```js
test("backtest UI exposes annualized return charts trades leverage and portfolio details", () => {
  const table = read("apps/web/components/backtest/backtest-result-table.tsx");
  const charts = read("apps/web/components/backtest/backtest-charts.tsx");
  const portfolioReview = read("apps/web/components/backtest/portfolio-candidate-review.tsx");

  assert.match(table, /年化|Annualized/i);
  assert.match(table, /杠杆|Leverage/i);
  assert.match(table, /查看详情|Details/i);
  assert.match(table, /long\+short|多空|direction_mode/i);

  assert.match(charts, /资金曲线|Equity curve/i);
  assert.match(charts, /回撤曲线|Drawdown curve/i);
  assert.match(charts, /交易明细|Trade details/i);
  assert.match(charts, /title=|onMouseMove|tooltip/i);

  assert.match(portfolioReview, /allocation_pct|资金权重|Allocation/i);
  assert.match(portfolioReview, /member_count|成员|Members/i);
  assert.match(portfolioReview, /组合资金曲线|Portfolio equity/i);
});
```

- [ ] **Step 3: Add engine unit tests**

Append tests to `apps/backtest-engine/tests/search_scoring_time_splits.rs` or create a focused test module if this file is already too broad:

```rust
#[test]
fn planned_margin_and_leverage_return_use_pre_leverage_capital() {
    use backtest_engine::martingale::metrics::{leveraged_position_pnl_quote, planned_margin_quote};

    let planned = planned_margin_quote(10.0, 2.0, 4);
    assert!((planned - 150.0).abs() < 0.000001);

    let pnl = leveraged_position_pnl_quote(10.0, 2.0, 0.01);
    assert!((pnl - 0.2).abs() < 0.000001);

    let return_pct = pnl / planned * 100.0;
    assert!((return_pct - 0.13333333333333333).abs() < 0.000001);
}

#[test]
fn annualized_return_uses_backtest_days() {
    use backtest_engine::martingale::metrics::calculate_annualized_return_pct;

    let annualized = calculate_annualized_return_pct(1000.0, 1100.0, 365.0).unwrap();
    assert!((annualized - 10.0).abs() < 0.000001);

    let half_year = calculate_annualized_return_pct(1000.0, 1100.0, 182.5).unwrap();
    assert!(half_year > 20.0);

    assert!(calculate_annualized_return_pct(1000.0, 1100.0, 0.0).is_none());
}
```

- [ ] **Step 4: Add long_short and portfolio tests**

Add to the same engine test file:

```rust
#[test]
fn long_short_candidate_contains_both_direction_legs() {
    use backtest_engine::search::{generate_staged_candidates_for_symbol, StagedMartingaleSearchSpace};
    use shared_domain::martingale::{MartingaleDirection, MartingaleDirectionMode};

    let space = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");
    let candidates = generate_staged_candidates_for_symbol("BTCUSDT", "long_short", &space, 16)
        .expect("long_short candidates");

    let candidate = candidates.iter().find(|c| c.config.direction_mode == MartingaleDirectionMode::LongShort)
        .expect("at least one long_short candidate");
    assert!(candidate.config.strategies.iter().any(|s| s.direction == MartingaleDirection::Long));
    assert!(candidate.config.strategies.iter().any(|s| s.direction == MartingaleDirection::Short));
}

#[test]
fn portfolio_top3_combines_multiple_members_not_single_pick() {
    use backtest_engine::portfolio_search::{build_portfolio_top3, EvaluatedCandidate};

    let candidates = fixture_evaluated_candidates_with_curves(6);
    let artifact = build_portfolio_top3(&candidates, 20.0);

    assert!(!artifact.top3.is_empty());
    for portfolio in artifact.top3 {
        assert!(portfolio.member_count >= 2);
        let allocation_sum: f64 = portfolio.members.iter().map(|m| m.allocation_pct).sum();
        assert!((allocation_sum - 100.0).abs() < 0.000001);
        assert!(!portfolio.equity_curve.is_empty());
        assert!(!portfolio.drawdown_curve.is_empty());
        assert!(portfolio.max_drawdown_pct <= 20.0);
    }
}
```

If existing helper names differ, create local fixtures in the test module with deterministic equity curves. Do not weaken assertions to string-only tests for engine behavior.

- [ ] **Step 5: Run tests and confirm failures**

Run:

```bash
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
cargo test -p backtest-engine planned_margin_and_leverage_return_use_pre_leverage_capital annualized_return_uses_backtest_days long_short_candidate_contains_both_direction_legs portfolio_top3_combines_multiple_members_not_single_pick -- --nocapture
```

Expected: at least some tests fail before implementation.

- [ ] **Step 6: Commit tests**

```bash
git add tests/verification/backtest_worker_contract.test.mjs tests/verification/backtest_console_contract.test.mjs apps/backtest-engine/tests/search_scoring_time_splits.rs
git commit -m "test: 问题描述 锁定马丁回测完整结果契约"
```

---

## Task 2: Fix Metrics and Leverage Accounting

**Files:**
- Modify: `apps/backtest-engine/src/martingale/metrics.rs`
- Modify: `apps/backtest-engine/src/martingale/trade_engine.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`

- [ ] **Step 1: Add canonical metric helpers**

In `apps/backtest-engine/src/martingale/metrics.rs`, add or normalize these public helpers:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawdownPoint {
    pub timestamp_ms: i64,
    pub drawdown_pct: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MartingaleTradeDetail {
    pub timestamp_ms: i64,
    pub symbol: String,
    pub direction: String,
    pub event_type: String,
    pub leg_index: Option<u32>,
    pub price: f64,
    pub margin_quote: f64,
    pub notional_quote: f64,
    pub leverage: f64,
    pub fee_quote: f64,
    pub slippage_quote: f64,
    pub realized_pnl_quote: f64,
    pub equity_after_quote: f64,
}

pub fn calculate_annualized_return_pct(
    initial_equity_quote: f64,
    ending_equity_quote: f64,
    backtest_days: f64,
) -> Option<f64> {
    if !initial_equity_quote.is_finite()
        || !ending_equity_quote.is_finite()
        || !backtest_days.is_finite()
        || initial_equity_quote <= 0.0
        || backtest_days <= 0.0
    {
        return None;
    }
    if ending_equity_quote <= 0.0 {
        return Some(-100.0);
    }
    let period_return = ending_equity_quote / initial_equity_quote - 1.0;
    Some(((1.0 + period_return).powf(365.0 / backtest_days) - 1.0) * 100.0)
}

pub fn build_drawdown_curve(equity_curve: &[EquityPoint]) -> Vec<DrawdownPoint> {
    let mut peak = f64::NEG_INFINITY;
    equity_curve
        .iter()
        .filter_map(|point| {
            if !point.equity_quote.is_finite() {
                return None;
            }
            peak = peak.max(point.equity_quote);
            let drawdown_pct = if peak <= 0.0 { 0.0 } else { ((peak - point.equity_quote) / peak) * 100.0 };
            Some(DrawdownPoint { timestamp_ms: point.timestamp_ms, drawdown_pct })
        })
        .collect()
}

pub fn notional_quote(margin_quote: f64, leverage: f64) -> f64 {
    margin_quote * leverage.max(1.0)
}
```

- [ ] **Step 2: Extend result struct**

Extend `MartingaleBacktestResult`:

```rust
pub struct MartingaleBacktestResult {
    pub metrics: MartingaleMetrics,
    pub events: Vec<MartingaleBacktestEvent>,
    pub equity_curve: Vec<EquityPoint>,
    #[serde(default)]
    pub drawdown_curve: Vec<DrawdownPoint>,
    #[serde(default)]
    pub trades: Vec<MartingaleTradeDetail>,
    pub rejection_reasons: Vec<String>,
}
```

Update all constructors/tests to pass `drawdown_curve: build_drawdown_curve(&equity_curve)` and `trades`.

- [ ] **Step 3: Ensure leverage uses notional for PnL and costs**

In `trade_engine.rs` / `kline_engine.rs`, verify every leg open uses:

```rust
let margin_quote = leg_margin_quote;
let leverage = strategy.leverage.unwrap_or(1) as f64;
let notional_quote = notional_quote(margin_quote, leverage);
let fee_quote = notional_quote * fee_rate;
```

PnL for price move must use notional exposure:

```rust
let direction_sign = if is_long { 1.0 } else { -1.0 };
let price_move_pct = (exit_price - entry_price) / entry_price;
let realized_pnl_quote = notional_quote * price_move_pct * direction_sign - open_fee - close_fee - slippage_quote;
```

Metrics return/drawdown must use `planned_margin_quote`, not first order quote.

- [ ] **Step 4: Run focused tests**

```bash
cargo test -p backtest-engine planned_margin_and_leverage_return_use_pre_leverage_capital annualized_return_uses_backtest_days -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-engine/src/martingale/metrics.rs apps/backtest-engine/src/martingale/trade_engine.rs apps/backtest-engine/src/martingale/kline_engine.rs apps/backtest-engine/tests/search_scoring_time_splits.rs
git commit -m "fix: 修复思路 统一马丁杠杆本金与年化指标"
```

---

## Task 3: Generate Real Long+Short Candidates

**Files:**
- Modify: `apps/backtest-engine/src/search.rs`
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add deterministic staged candidate generator if absent**

In `apps/backtest-engine/src/search.rs`, implement a public generator used by tests and worker:

```rust
pub fn generate_staged_candidates_for_symbol(
    symbol: &str,
    direction: &str,
    space: &StagedMartingaleSearchSpace,
    limit: usize,
) -> Result<Vec<SearchCandidate>, String> {
    let mut candidates = Vec::new();
    for leverage in &space.leverage {
        for spacing_bps in &space.spacing_bps {
            for multiplier in &space.order_multiplier {
                for max_legs in &space.max_legs {
                    for take_profit_bps in &space.take_profit_bps {
                        for tail_stop_bps in &space.tail_stop_bps {
                            match direction {
                                "long" => candidates.push(build_single_direction_candidate(symbol, MartingaleDirection::Long, *leverage, *spacing_bps, *multiplier, *max_legs, *take_profit_bps, *tail_stop_bps)?),
                                "short" => candidates.push(build_single_direction_candidate(symbol, MartingaleDirection::Short, *leverage, *spacing_bps, *multiplier, *max_legs, *take_profit_bps, *tail_stop_bps)?),
                                "long_short" => {
                                    for (long_weight_pct, short_weight_pct) in &space.long_short_weight_pct {
                                        candidates.push(build_long_short_candidate(symbol, *leverage, *spacing_bps, *multiplier, *max_legs, *take_profit_bps, *tail_stop_bps, *long_weight_pct, *short_weight_pct)?);
                                    }
                                }
                                other => return Err(format!("unsupported direction: {other}")),
                            }
                            if candidates.len() >= limit {
                                return Ok(candidates);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(candidates)
}
```

Use existing candidate builder style and shared-domain types. Ensure `build_long_short_candidate` creates two strategies with `MartingaleDirectionMode::LongShort`, one `Long`, one `Short`, and both `UsdMFutures` + isolated/cross mode consistent with current domain validation.

- [ ] **Step 2: Ensure worker uses this generator for staged search**

In `apps/backtest-worker/src/main.rs`, replace any path that turns `long_short` into only `Long`. The search loop must iterate generated candidates and preserve `direction_mode=LongShort` through evaluation and persistence.

- [ ] **Step 3: Run tests**

```bash
cargo test -p backtest-engine long_short_candidate_contains_both_direction_legs -- --nocapture
cargo test -p backtest-worker -- --nocapture
```

- [ ] **Step 4: Commit**

```bash
git add apps/backtest-engine/src/search.rs apps/backtest-worker/src/main.rs apps/backtest-engine/tests/search_scoring_time_splits.rs
git commit -m "fix: 修复思路 支持马丁多空双腿候选生成"
```

---

## Task 4: Build True Weighted Portfolio Top 3

**Files:**
- Rewrite: `apps/backtest-engine/src/portfolio_search.rs`
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Replace single-candidate artifact types**

In `apps/backtest-engine/src/portfolio_search.rs`, replace `top3: Vec<EvaluatedCandidate>` with portfolio-level structs:

```rust
use crate::martingale::metrics::{build_drawdown_curve, calculate_annualized_return_pct, DrawdownPoint, EquityPoint, MartingaleTradeDetail};
use crate::search::SearchCandidate;

#[derive(Debug, Clone)]
pub struct EvaluatedCandidate {
    pub candidate: SearchCandidate,
    pub score: f64,
    pub return_pct: f64,
    pub annualized_return_pct: Option<f64>,
    pub max_drawdown_pct: f64,
    pub survival_passed: bool,
    pub planned_margin_quote: f64,
    pub trade_count: u64,
    pub equity_curve: Vec<EquityPoint>,
    pub drawdown_curve: Vec<DrawdownPoint>,
    pub trades: Vec<MartingaleTradeDetail>,
}

#[derive(Debug, Clone)]
pub struct PortfolioMember {
    pub candidate_id: String,
    pub symbol: String,
    pub direction_mode: String,
    pub allocation_pct: f64,
    pub leverage: Option<u32>,
    pub return_pct: f64,
    pub max_drawdown_pct: f64,
    pub score: f64,
}

#[derive(Debug, Clone)]
pub struct PortfolioCandidate {
    pub portfolio_id: String,
    pub rank: usize,
    pub member_count: usize,
    pub members: Vec<PortfolioMember>,
    pub total_return_pct: f64,
    pub annualized_return_pct: Option<f64>,
    pub max_drawdown_pct: f64,
    pub return_drawdown_ratio: f64,
    pub score: f64,
    pub equity_curve: Vec<EquityPoint>,
    pub drawdown_curve: Vec<DrawdownPoint>,
    pub trades: Vec<MartingaleTradeDetail>,
}

#[derive(Debug, Clone)]
pub struct PortfolioArtifact {
    pub top3: Vec<PortfolioCandidate>,
    pub total_candidates: usize,
    pub survivors: usize,
    pub attempted_combinations: usize,
}
```

- [ ] **Step 2: Implement weighted curve combination**

Add deterministic helper:

```rust
pub fn combine_equity_curves(
    members: &[(&EvaluatedCandidate, f64)],
    initial_portfolio_capital: f64,
) -> Vec<EquityPoint> {
    if members.is_empty() {
        return Vec::new();
    }
    let min_len = members.iter().map(|(candidate, _)| candidate.equity_curve.len()).min().unwrap_or(0);
    if min_len == 0 {
        return Vec::new();
    }

    (0..min_len)
        .map(|idx| {
            let timestamp_ms = members[0].0.equity_curve[idx].timestamp_ms;
            let equity_quote = members.iter().map(|(candidate, allocation_pct)| {
                let allocated_capital = initial_portfolio_capital * (*allocation_pct / 100.0);
                let initial_candidate_margin = candidate.planned_margin_quote.max(0.000001);
                allocated_capital * candidate.equity_curve[idx].equity_quote / initial_candidate_margin
            }).sum();
            EquityPoint { timestamp_ms, equity_quote }
        })
        .collect()
}
```

- [ ] **Step 3: Implement multi-member Top 3 search**

Update `build_portfolio_top3`:

```rust
pub fn build_portfolio_top3(candidates: &[EvaluatedCandidate], max_drawdown_pct: f64) -> PortfolioArtifact {
    let survivors: Vec<&EvaluatedCandidate> = candidates
        .iter()
        .filter(|c| c.survival_passed && c.return_pct > 0.0 && c.max_drawdown_pct <= max_drawdown_pct && !c.equity_curve.is_empty())
        .collect();

    let mut ranked = survivors.clone();
    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    let seed_pool: Vec<&EvaluatedCandidate> = ranked.into_iter().take(24).collect();
    let allocation_sets: Vec<Vec<f64>> = vec![
        vec![50.0, 50.0],
        vec![40.0, 30.0, 30.0],
        vec![35.0, 25.0, 20.0, 20.0],
        vec![25.0, 20.0, 20.0, 20.0, 15.0],
    ];

    let mut portfolios = Vec::new();
    let mut attempted = 0usize;
    for allocation in allocation_sets {
        if seed_pool.len() < allocation.len() {
            continue;
        }
        for start in 0..seed_pool.len() {
            let selected: Vec<&EvaluatedCandidate> = (0..allocation.len())
                .map(|offset| seed_pool[(start + offset) % seed_pool.len()])
                .collect();
            if selected.iter().map(|c| &c.candidate.candidate_id).collect::<std::collections::HashSet<_>>().len() < 2 {
                continue;
            }
            attempted += 1;
            let member_pairs: Vec<(&EvaluatedCandidate, f64)> = selected.iter().copied().zip(allocation.iter().copied()).collect();
            let equity_curve = combine_equity_curves(&member_pairs, 10_000.0);
            if equity_curve.len() < 2 {
                continue;
            }
            let drawdown_curve = build_drawdown_curve(&equity_curve);
            let initial = equity_curve.first().map(|p| p.equity_quote).unwrap_or(10_000.0);
            let ending = equity_curve.last().map(|p| p.equity_quote).unwrap_or(initial);
            let total_return_pct = (ending / initial - 1.0) * 100.0;
            let max_dd = drawdown_curve.iter().map(|p| p.drawdown_pct).fold(0.0, f64::max);
            if total_return_pct <= 0.0 || max_dd > max_drawdown_pct {
                continue;
            }
            let days = (equity_curve.last().unwrap().timestamp_ms - equity_curve.first().unwrap().timestamp_ms) as f64 / 86_400_000.0;
            let annualized_return_pct = calculate_annualized_return_pct(initial, ending, days);
            let ratio = if max_dd <= 0.0 { total_return_pct } else { total_return_pct / max_dd };
            let score = (ratio * 20.0 + annualized_return_pct.unwrap_or(0.0).min(100.0) * 0.4).clamp(0.0, 100.0);
            let members = member_pairs.iter().map(|(candidate, allocation_pct)| PortfolioMember {
                candidate_id: candidate.candidate.candidate_id.clone(),
                symbol: candidate.candidate.config.strategies.first().map(|s| s.symbol.clone()).unwrap_or_default(),
                direction_mode: format!("{:?}", candidate.candidate.config.direction_mode),
                allocation_pct: *allocation_pct,
                leverage: candidate.candidate.config.strategies.iter().filter_map(|s| s.leverage).max(),
                return_pct: candidate.return_pct,
                max_drawdown_pct: candidate.max_drawdown_pct,
                score: candidate.score,
            }).collect::<Vec<_>>();
            let trades = selected.iter().flat_map(|c| c.trades.clone()).collect::<Vec<_>>();
            portfolios.push(PortfolioCandidate {
                portfolio_id: format!("portfolio-{}-{}", allocation.len(), attempted),
                rank: 0,
                member_count: members.len(),
                members,
                total_return_pct,
                annualized_return_pct,
                max_drawdown_pct: max_dd,
                return_drawdown_ratio: ratio,
                score,
                equity_curve,
                drawdown_curve,
                trades,
            });
        }
    }

    portfolios.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    portfolios.dedup_by(|a, b| {
        let a_ids: Vec<_> = a.members.iter().map(|m| m.candidate_id.as_str()).collect();
        let b_ids: Vec<_> = b.members.iter().map(|m| m.candidate_id.as_str()).collect();
        a_ids == b_ids
    });
    for (index, portfolio) in portfolios.iter_mut().take(3).enumerate() {
        portfolio.rank = index + 1;
    }

    PortfolioArtifact {
        top3: portfolios.into_iter().take(3).collect(),
        total_candidates: candidates.len(),
        survivors: survivors.len(),
        attempted_combinations: attempted,
    }
}
```

It is acceptable for Claude to improve the search algorithm, but it must preserve: multi-member only, allocation sum 100, weighted equity curve, max drawdown from portfolio curve.

- [ ] **Step 4: Update worker persistence**

In `apps/backtest-worker/src/main.rs`, update code that serializes `PortfolioArtifact.top3`. Persist portfolio fields as portfolio summaries, not as fake candidates. Include `members`, `allocation_pct`, `equity_curve`, `drawdown_curve`, and `trades_preview`.

- [ ] **Step 5: Run tests**

```bash
cargo test -p backtest-engine portfolio_top3_combines_multiple_members_not_single_pick -- --nocapture
cargo test -p backtest-worker -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-engine/src/portfolio_search.rs apps/backtest-worker/src/main.rs apps/backtest-engine/tests/search_scoring_time_splits.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "fix: 修复思路 重构马丁组合为真实资金权重回测"
```

---

## Task 5: Preserve Candidate Artifacts and Eligible Candidate Pool

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `apps/api-server/src/services/backtest_service.rs`

- [ ] **Step 1: Persist richer candidate summaries**

When the worker saves candidates, ensure summary JSON contains:

```json
{
  "annualized_return_pct": 0.0,
  "return_drawdown_ratio": 0.0,
  "planned_margin_quote": 0.0,
  "max_leverage_used": 0,
  "legs": [],
  "equity_curve": [],
  "drawdown_curve": [],
  "trades_preview": [],
  "eligible_candidate_count_for_symbol": 0,
  "rejection_breakdown": {}
}
```

Do not drop these keys when writing DB rows or artifact JSON.

- [ ] **Step 2: Keep enough eligible candidates**

Set worker retention defaults:

```rust
const PER_SYMBOL_DISPLAY_TOP_N: usize = 10;
const PER_SYMBOL_ELIGIBLE_POOL_MIN: usize = 20;
const PORTFOLIO_TOP_N: usize = 3;
```

For each symbol, persist at least Top 10 display candidates and keep at least 20 eligible candidates in the artifact/portfolio input when available. If fewer than 20 survive, write `rejection_breakdown` with counts.

- [ ] **Step 3: API detail must expose fields**

In `apps/api-server/src/services/backtest_service.rs`, update candidate and portfolio DTO mapping so it forwards:

- `annualized_return_pct`
- `return_drawdown_ratio`
- `planned_margin_quote`
- `max_leverage_used`
- `legs`
- `equity_curve`
- `drawdown_curve`
- `trades_preview`
- `members`
- `allocation_pct`
- `eligible_candidate_count_for_symbol`
- `rejection_breakdown`

- [ ] **Step 4: Run API and worker tests**

```bash
cargo test -p backtest-worker -- --nocapture
cargo test -p api-server martingale_backtest -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-worker/src/main.rs apps/api-server/src/services/backtest_service.rs
git commit -m "fix: 修复思路 保留马丁候选曲线明细与候选池"
```

---

## Task 6: Update Web UI Details

**Files:**
- Modify: `apps/web/components/backtest/backtest-result-table.tsx`
- Modify: `apps/web/components/backtest/backtest-charts.tsx`
- Modify: `apps/web/components/backtest/portfolio-candidate-review.tsx`
- Modify if needed: `apps/web/components/backtest/request-client.ts`

- [ ] **Step 1: Extend TypeScript types**

Where candidate summary types are defined, add optional fields:

```ts
annualized_return_pct?: number | null;
return_drawdown_ratio?: number | null;
planned_margin_quote?: number | null;
max_leverage_used?: number | null;
legs?: Array<{
  direction: string;
  weight_pct?: number | null;
  leverage?: number | null;
  spacing_bps?: number | null;
  max_legs?: number | null;
  take_profit_bps?: number | null;
}>;
drawdown_curve?: Array<{ timestamp_ms: number; drawdown_pct: number }>;
trades_preview?: Array<Record<string, unknown>>;
members?: Array<{
  candidate_id: string;
  symbol: string;
  direction_mode: string;
  allocation_pct: number;
  leverage?: number | null;
  total_return_pct?: number | null;
  max_drawdown_pct?: number | null;
  score?: number | null;
}>;
eligible_candidate_count_for_symbol?: number | null;
rejection_breakdown?: Record<string, number>;
```

- [ ] **Step 2: Add table columns and details button**

In `backtest-result-table.tsx`, display:

- Direction mode: show `long+short` when legs include both directions.
- Annualized return.
- Leverage.
- Planned margin.
- Return/DD ratio.
- Details button that selects candidate/portfolio for detail panel.

- [ ] **Step 3: Make charts support candidate and portfolio summaries**

In `backtest-charts.tsx`, ensure it reads both old and new curve shapes:

```ts
function readTimestamp(point: unknown): number | null {
  const p = point as Record<string, unknown>;
  return readFiniteNumber(p.timestamp_ms) ?? readFiniteNumber(p.ts) ?? null;
}

function readEquity(point: unknown): number | null {
  const p = point as Record<string, unknown>;
  return readFiniteNumber(p.equity_quote) ?? readFiniteNumber(p.equity) ?? null;
}

function readDrawdown(point: unknown): number | null {
  const p = point as Record<string, unknown>;
  return readFiniteNumber(p.drawdown_pct) ?? readFiniteNumber(p.drawdown) ?? null;
}
```

Add hover tooltip using SVG `<title>` at minimum:

```tsx
<title>{`${new Date(point.ts).toLocaleString()} Equity ${fmtNum(point.equity)} Return ${fmtPctValue(returnPct)}`}</title>
```

Add a trade detail section:

```tsx
<h4 className="text-sm font-medium mb-1">交易明细 / Trade details</h4>
{trades.length === 0 ? <p className="text-xs text-muted-foreground">暂无交易明细</p> : <TradeDetailsTable trades={trades.slice(0, 100)} total={trades.length} />}
```

- [ ] **Step 4: Portfolio review shows true members**

In `portfolio-candidate-review.tsx`, render:

- portfolio metrics.
- member table with `allocation_pct`.
- member leverage and direction.
- portfolio charts via `BacktestCharts`.
- rejection/eligible pool diagnostics.

- [ ] **Step 5: Run web tests/build**

```bash
node tests/verification/backtest_console_contract.test.mjs
pnpm --filter web exec next build --webpack
```

- [ ] **Step 6: Commit**

```bash
git add apps/web/components/backtest/backtest-result-table.tsx apps/web/components/backtest/backtest-charts.tsx apps/web/components/backtest/portfolio-candidate-review.tsx apps/web/components/backtest/request-client.ts tests/verification/backtest_console_contract.test.mjs
git commit -m "fix: 修复思路 完善马丁回测图表明细与组合详情"
```

---

## Task 7: End-to-End Verification and Handoff

**Files:**
- Modify if needed: `docs/superpowers/plans/2026-05-18-martingale-backtest-result-completeness-fix-plan.md`

- [ ] **Step 1: Run focused verification**

```bash
cargo test -p backtest-engine -- --nocapture
cargo test -p backtest-worker -- --nocapture
cargo test -p api-server martingale_backtest -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
pnpm --filter web exec next build --webpack
```

- [ ] **Step 2: Run one smoke backtest**

Create a BTCUSDT + ETHUSDT `long_short` balanced task through the existing API or UI with a small but real date range for speed. Verify response/artifact has:

- At least one candidate with both long and short legs.
- `annualized_return_pct` present.
- `equity_curve.length > 0`.
- `drawdown_curve.length > 0`.
- `trades_preview.length > 0` or full artifact trades present.
- `portfolio_top3.length > 0`.
- Every portfolio has `member_count >= 2` and allocation sum 100.

- [ ] **Step 3: Inspect workspace**

```bash
git status --short
git log --oneline -8
```

- [ ] **Step 4: Final commit if needed**

If verification required minor fixes:

```bash
git add <changed-files>
git commit -m "fix: 修复思路 完成马丁回测结果完整性验证"
```

- [ ] **Step 5: Report to reviewer**

Claude must report:

- Tests run and exact pass/fail status.
- Smoke task id and key metrics.
- Candidate count per symbol.
- Whether `long_short` includes both legs.
- Portfolio Top 3 member counts and allocation sums.
- Any remaining known limitations.

---

## Self-Review Checklist

- Spec item 1 `long_short only long`: covered by Tasks 1 and 3.
- Spec item 2 annualized return: covered by Tasks 1 and 2.
- Spec item 3 charts/trades: covered by Tasks 2, 5, 6.
- Spec item 4 portfolio details and true combination: covered by Tasks 1, 4, 6.
- Spec item 5 too few results: covered by Task 5.
- Spec item 6 leverage and pre-leverage capital: covered by Tasks 1 and 2.
- No accepted implementation may keep `build_portfolio_top3` as direct single-candidate sorting.
- No accepted implementation may silently omit artifact curves/trades.
