# Martingale Portfolio Backtest System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the confirmed mixed martingale portfolio system: two-stage backtesting, random/intelligent search, dedicated UI, quota-controlled worker execution, Portfolio candidate publishing, and conservative live runtime support.

**Architecture:** Add an independent `martingale_grid` strategy family with shared pure rule evaluators used by both backtest and live runtime. Store task/candidate metadata in the existing app database, store large result artifacts outside the business tables, run heavy backtests in a standalone worker process, and keep `/home/bumblebee/Project/discord_c2im/pipeline/data/market_data.db` strictly read-only.

**Tech Stack:** Rust workspace (`shared-domain`, `backtest-engine`, `shared-db`, `api-server`, `trading-engine`, new `backtest-worker`), PostgreSQL/Redis through existing `SharedDb`, Next.js App Router UI, Node verification tests, Rust unit/integration tests.

---

## Source Spec

This plan implements `docs/superpowers/specs/2026-05-09-martingale-portfolio-backtest-design.md`.

## Important Constraints

- Work in `/home/bumblebee/Project/grid_binance/.worktrees/full-v1`.
- Do not change existing ordinary/classic grid behavior except for additive compatibility with `StrategyType::MartingaleGrid`.
- Do not write to `/home/bumblebee/Project/discord_c2im/pipeline/data/market_data.db`.
- Do not silently switch Binance margin mode, leverage, or position mode.
- Do not auto-start a Portfolio without explicit user confirmation.
- For futures live publish, enforce Hedge Mode for long+short and same-symbol `margin_mode`/`leverage` compatibility.
- Use TDD for pure logic and API behavior before implementation.
- Commit steps are listed for workers, but do not commit unless the user explicitly asks in this session.

## File Structure

### Shared Domain

- `crates/shared-domain/src/martingale.rs`  
  Owns serializable martingale configs, Portfolio configs, runtime IDs, risk limits, status enums, and validation helpers.

- `crates/shared-domain/src/strategy.rs`  
  Adds `StrategyType::MartingaleGrid` only. Existing enum variants and serde strings remain stable.

- `crates/shared-domain/src/lib.rs`  
  Exports `martingale`.

### Backtest Engine

- `apps/backtest-engine/src/martingale/mod.rs`  
  Module root for martingale simulation.

- `apps/backtest-engine/src/martingale/rules.rs`  
  Pure spacing, sizing, entry, take-profit, stop-loss helpers shared by tests and live runtime adapter.

- `apps/backtest-engine/src/martingale/state.rs`  
  Portfolio/symbol/strategy/cycle/leg simulation state.

- `apps/backtest-engine/src/martingale/kline_engine.rs`  
  Conservative OHLCV screening simulation.

- `apps/backtest-engine/src/martingale/trade_engine.rs`  
  Agg-trade refinement simulation.

- `apps/backtest-engine/src/martingale/metrics.rs`  
  Metrics and curves.

- `apps/backtest-engine/src/martingale/scoring.rs`  
  Survival-first filters and weighted scoring.

- `apps/backtest-engine/src/indicators.rs`  
  ATR, SMA, EMA, RSI, Bollinger, ADX.

- `apps/backtest-engine/src/market_data.rs`  
  Market data traits and data quality models.

- `apps/backtest-engine/src/sqlite_market_data.rs`  
  Read-only SQLite adapter with diagnostics.

- `apps/backtest-engine/src/search.rs`  
  Search space, random search, deterministic sampling.

- `apps/backtest-engine/src/intelligent_search.rs`  
  Iterative survival-aware intelligent search.

- `apps/backtest-engine/src/time_splits.rs`  
  Manual, walk-forward, and stress windows.

- `apps/backtest-engine/src/artifacts.rs`  
  Large result artifact writer/reader with checksums.

- `apps/backtest-engine/src/bin/market_data_probe.rs`  
  Read-only CLI to diagnose external market database.

### Persistence and Worker

- `db/migrations/0017_martingale_backtest_portfolios.sql`  
  App DB schema for tasks, quotas, candidates, artifacts, live portfolios, audit/orphans.

- `crates/shared-db/src/backtest.rs`  
  Repository records and CRUD for backtest/Portfolio tables, plus ephemeral support.

- `apps/backtest-worker/Cargo.toml` and `apps/backtest-worker/src/main.rs`  
  Standalone worker polling queued tasks, running search/refinement, writing artifacts.

### API

- `apps/api-server/src/services/backtest_service.rs`  
  User task creation, quota checks, task actions, candidate reads, publish-intent risk checks.

- `apps/api-server/src/services/martingale_publish_service.rs`  
  Candidate-to-pending-Portfolio conversion and live Portfolio creation after confirmation.

- `apps/api-server/src/routes/backtest.rs`  
  Task/candidate/publish endpoints; keeps legacy `/backtest/run` compatibility.

- `apps/api-server/src/routes/admin_backtest.rs`  
  Admin quota/resource endpoints.

- `apps/api-server/src/routes/martingale_portfolios.rs`  
  Live Portfolio operations.

- `apps/api-server/src/lib.rs`  
  Wires services and routes.

### Trading Engine

- `apps/trading-engine/src/martingale_runtime.rs`  
  Live `martingale_grid` order/cycle runtime.

- `apps/trading-engine/src/martingale_recovery.rs`  
  Conservative recovery and orphan detection.

- Existing sync/runtime files branch to these modules without changing ordinary grid semantics.

### Web UI

- `apps/web/app/[locale]/app/backtest/page.tsx`  
  Dedicated backtest console page.

- `apps/web/components/backtest/*.tsx`  
  Wizard, professional panel, parameter editors, task list, charts, candidate review.

- `apps/web/app/[locale]/app/martingale-portfolios/page.tsx`  
  Live Portfolio list.

- `apps/web/app/[locale]/app/martingale-portfolios/[id]/page.tsx`  
  Live Portfolio detail.

- `apps/web/app/api/user/backtest/**/route.ts`  
  Next.js API proxies to Rust API.

### Verification and Docs

- `apps/api-server/tests/martingale_backtest_flow.rs`
- `apps/api-server/tests/backtest_flow.rs`
- `apps/trading-engine/tests/martingale_runtime.rs`
- `apps/trading-engine/tests/martingale_recovery.rs`
- `tests/verification/backtest_console_contract.test.mjs`
- `tests/verification/strategy_surface_contract.test.mjs`
- `docs/user-guide/zh/martingale-backtest.md`
- `docs/user-guide/martingale-backtest.md`
- `docs/deployment/env-and-secrets.md`
- `docs/deployment/docker-compose.md`

---

## Task 1: Add Martingale Domain Model

**Files:**
- Create: `crates/shared-domain/src/martingale.rs`
- Modify: `crates/shared-domain/src/lib.rs`
- Modify: `crates/shared-domain/src/strategy.rs`

- [ ] **Step 1: Write domain tests in `crates/shared-domain/src/martingale.rs`**

Add a `#[cfg(test)]` module covering serde, validation, and conflicts:

```rust
#[test]
fn futures_long_short_portfolio_round_trips() {
    let portfolio = MartingalePortfolioConfig::example_futures_long_short("BTCUSDT");
    let encoded = serde_json::to_string(&portfolio).expect("serialize portfolio");
    assert!(encoded.contains("BTCUSDT"));
    assert!(encoded.contains("long_and_short"));
    let decoded: MartingalePortfolioConfig = serde_json::from_str(&encoded).expect("deserialize portfolio");
    assert_eq!(decoded.strategies.len(), 2);
    assert_eq!(decoded.validate().unwrap(), ());
}

#[test]
fn same_symbol_futures_margin_or_leverage_conflict_is_rejected() {
    let mut portfolio = MartingalePortfolioConfig::example_futures_long_short("BTCUSDT");
    portfolio.strategies[1].leverage = Some(5);
    let error = portfolio.validate().expect_err("conflicting leverage must fail");
    assert!(error.contains("BTCUSDT"));
    assert!(error.contains("leverage"));
}

#[test]
fn spot_rejects_futures_only_fields() {
    let mut strategy = MartingaleStrategyConfig::example_spot_long("ETHUSDT");
    strategy.margin_mode = Some(MartingaleMarginMode::Isolated);
    strategy.leverage = Some(2);
    let error = strategy.validate().expect_err("spot cannot use futures fields");
    assert!(error.contains("spot"));
}
```

- [ ] **Step 2: Run tests and verify failure**

Run: `cargo test -p shared-domain martingale -- --nocapture`  
Expected: FAIL because `martingale` module and types do not exist.

- [ ] **Step 3: Implement `crates/shared-domain/src/martingale.rs`**

Define these public types with serde `snake_case` where enum strings are user/API visible:

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum MartingaleMarketKind { Spot, UsdMFutures }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum MartingaleDirection { Long, Short }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MartingaleDirectionMode { LongOnly, ShortOnly, LongAndShort, IndicatorSelected }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MartingaleMarginMode { Isolated, Cross }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MartingaleSpacingModel {
    FixedPercent { step_bps: u32 },
    Multiplier { first_step_bps: u32, multiplier: f64 },
    Atr { multiplier: f64, min_step_bps: u32, max_step_bps: u32 },
    CustomSequence { steps_bps: Vec<u32> },
    Mixed { phases: Vec<MartingaleSpacingModel> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MartingaleSizingModel {
    Multiplier { first_order_quote: f64, multiplier: f64, max_legs: u32 },
    CustomSequence { notionals: Vec<f64> },
    BudgetScaled { first_order_quote: f64, multiplier: f64, max_legs: u32, max_budget_quote: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MartingaleTakeProfitModel {
    Percent { target_bps: u32 },
    Amount { quote_profit: f64 },
    Atr { multiplier: f64 },
    Trailing { activation_bps: u32, callback_bps: u32 },
    Mixed { rules: Vec<MartingaleTakeProfitModel> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MartingaleStopLossModel {
    PriceRange { lower: Option<f64>, upper: Option<f64> },
    Atr { multiplier: f64 },
    Indicator { expression: String },
    StrategyDrawdownPct { max_drawdown_bps: u32 },
    SymbolDrawdownAmount { max_loss_quote: f64 },
    GlobalDrawdownAmount { max_loss_quote: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MartingaleIndicatorConfig {
    Atr { period: u32 },
    Sma { period: u32 },
    Ema { period: u32 },
    Rsi { period: u32 },
    Bollinger { period: u32, stddev: f64 },
    Adx { period: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MartingaleEntryTrigger {
    Immediate,
    IndicatorExpression { expression: String },
    PriceRange { lower: Option<f64>, upper: Option<f64> },
    TimeWindow { start_utc: String, end_utc: String },
    Cooldown { seconds: u64 },
    Capacity { max_active_cycles: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MartingaleRiskLimits {
    pub max_global_budget_quote: f64,
    pub max_symbol_budget_quote: BTreeMap<String, f64>,
    pub max_direction_budget_quote: BTreeMap<MartingaleDirection, f64>,
    pub max_strategy_budget_quote: f64,
    pub max_global_drawdown_quote: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MartingaleStrategyConfig {
    pub strategy_instance_id: String,
    pub symbol: String,
    pub market: MartingaleMarketKind,
    pub direction: MartingaleDirection,
    pub margin_mode: Option<MartingaleMarginMode>,
    pub leverage: Option<u32>,
    pub entry_triggers: Vec<MartingaleEntryTrigger>,
    pub spacing: MartingaleSpacingModel,
    pub sizing: MartingaleSizingModel,
    pub take_profit: MartingaleTakeProfitModel,
    pub stop_loss: Vec<MartingaleStopLossModel>,
    pub indicators: Vec<MartingaleIndicatorConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MartingalePortfolioConfig {
    pub portfolio_id: String,
    pub name: String,
    pub direction_mode: MartingaleDirectionMode,
    pub risk_limits: MartingaleRiskLimits,
    pub strategies: Vec<MartingaleStrategyConfig>,
    pub random_seed: u64,
}
```

Implement `validate()` on strategy and portfolio. Strategy validation rejects spot with `margin_mode` or `leverage`, rejects futures with missing `margin_mode`/`leverage`, and rejects leverage `0`. Portfolio validation calls strategy validation and rejects same-symbol USDT-M configs with conflicting `margin_mode` or `leverage`.

- [ ] **Step 4: Export module**

In `crates/shared-domain/src/lib.rs`, add:

```rust
pub mod martingale;
```

- [ ] **Step 5: Add strategy type variant**

In `crates/shared-domain/src/strategy.rs`, extend `StrategyType`:

```rust
pub enum StrategyType {
    OrdinaryGrid,
    ClassicBilateralGrid,
    MartingaleGrid,
}
```

Do not rename existing variants.

- [ ] **Step 6: Run domain tests**

Run: `cargo test -p shared-domain martingale -- --nocapture`  
Expected: PASS.

- [ ] **Step 7: Commit if requested**

```bash
git add crates/shared-domain/src/martingale.rs crates/shared-domain/src/lib.rs crates/shared-domain/src/strategy.rs
git commit -m "feat: add martingale portfolio domain model"
```

---

## Task 2: Protect Existing Strategy Surface

**Files:**
- Modify: `tests/verification/strategy_surface_contract.test.mjs`
- Test: `tests/verification/strategy_surface_contract.test.mjs`

- [ ] **Step 1: Add surface assertions**

Append assertions that verify existing strategy strings remain present and `martingale_grid` is additive. Use simple text checks against `crates/shared-domain/src/strategy.rs` and existing UI/API files:

```js
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const strategyDomain = readFileSync("crates/shared-domain/src/strategy.rs", "utf8");
assert.match(strategyDomain, /OrdinaryGrid/);
assert.match(strategyDomain, /ClassicBilateralGrid/);
assert.match(strategyDomain, /MartingaleGrid/);
assert.match(strategyDomain, /rename_all = "snake_case"/);
```

If this file already has helpers, integrate with those helpers instead of duplicating imports.

- [ ] **Step 2: Run verification**

Run: `node --test tests/verification/strategy_surface_contract.test.mjs`  
Expected: PASS.

- [ ] **Step 3: Commit if requested**

```bash
git add tests/verification/strategy_surface_contract.test.mjs
git commit -m "test: lock additive martingale strategy surface"
```

---

## Task 3: Add Pure Martingale Rules

**Files:**
- Create: `apps/backtest-engine/src/martingale/mod.rs`
- Create: `apps/backtest-engine/src/martingale/rules.rs`
- Modify: `apps/backtest-engine/src/lib.rs`
- Modify: `apps/backtest-engine/Cargo.toml`

- [ ] **Step 1: Add failing rule tests**

In `apps/backtest-engine/src/martingale/rules.rs`, add tests:

```rust
#[test]
fn long_fixed_percent_triggers_move_below_anchor() {
    let prices = compute_leg_trigger_prices(100.0, MartingaleDirection::Long, &MartingaleSpacingModel::FixedPercent { step_bps: 100 }, None, 3).unwrap();
    assert_eq!(prices, vec![99.0, 98.0, 97.0]);
}

#[test]
fn short_fixed_percent_triggers_move_above_anchor() {
    let prices = compute_leg_trigger_prices(100.0, MartingaleDirection::Short, &MartingaleSpacingModel::FixedPercent { step_bps: 100 }, None, 3).unwrap();
    assert_eq!(prices, vec![101.0, 102.0, 103.0]);
}

#[test]
fn multiplier_sizing_matches_martingale_example() {
    let notionals = compute_leg_notionals(&MartingaleSizingModel::Multiplier { first_order_quote: 10.0, multiplier: 2.0, max_legs: 4 }, 1_000.0, 5.0).unwrap();
    assert_eq!(notionals, vec![10.0, 20.0, 40.0, 80.0]);
}

#[test]
fn budget_scaled_rejects_when_scaled_leg_below_min_notional() {
    let error = compute_leg_notionals(&MartingaleSizingModel::BudgetScaled { first_order_quote: 10.0, multiplier: 2.0, max_legs: 4, max_budget_quote: 8.0 }, 8.0, 5.0).expect_err("too small");
    assert!(error.contains("minimum notional"));
}
```

- [ ] **Step 2: Run tests and verify failure**

Run: `cargo test -p backtest-engine martingale::rules -- --nocapture`  
Expected: FAIL because module/functions do not exist.

- [ ] **Step 3: Implement rules**

Implement:

```rust
pub fn compute_leg_trigger_prices(
    anchor_price: f64,
    direction: MartingaleDirection,
    spacing: &MartingaleSpacingModel,
    latest_atr: Option<f64>,
    max_legs: u32,
) -> Result<Vec<f64>, String>
```

Rules:
- Long triggers below anchor.
- Short triggers above anchor.
- `FixedPercent` uses the same bps for each leg.
- `Multiplier` multiplies step distance each leg.
- `Atr` requires `latest_atr` and clamps to min/max bps using `anchor_price`.
- `CustomSequence` uses provided bps, up to `max_legs`.
- `Mixed` flattens phases in order until `max_legs`.

Implement:

```rust
pub fn compute_leg_notionals(
    sizing: &MartingaleSizingModel,
    portfolio_budget_quote: f64,
    exchange_min_notional: f64,
) -> Result<Vec<f64>, String>
```

Rules:
- `Multiplier` returns geometric notional series.
- `CustomSequence` returns the sequence.
- `BudgetScaled` scales the full geometric series to `min(max_budget_quote, portfolio_budget_quote)` when needed.
- Any leg below `exchange_min_notional` returns an error.
- Sum above budget returns an error unless using `BudgetScaled`.

- [ ] **Step 4: Wire module**

In `apps/backtest-engine/src/martingale/mod.rs`:

```rust
pub mod rules;
```

In `apps/backtest-engine/src/lib.rs`:

```rust
pub mod martingale;
```

Add `shared-domain` martingale imports in rule file.

- [ ] **Step 5: Run rules tests**

Run: `cargo test -p backtest-engine martingale::rules -- --nocapture`  
Expected: PASS.

- [ ] **Step 6: Commit if requested**

```bash
git add apps/backtest-engine/src/martingale apps/backtest-engine/src/lib.rs apps/backtest-engine/Cargo.toml
git commit -m "feat: add martingale spacing and sizing rules"
```

---

## Task 4: Add Indicator Engine

**Files:**
- Create: `apps/backtest-engine/src/indicators.rs`
- Modify: `apps/backtest-engine/src/lib.rs`

- [ ] **Step 1: Add deterministic indicator tests**

Add tests that use small fixed candles:

```rust
#[test]
fn sma_and_ema_return_none_until_warmup() {
    let closes = [1.0, 2.0, 3.0, 4.0];
    assert_eq!(sma(&closes, 3), vec![None, None, Some(2.0), Some(3.0)]);
    assert_eq!(ema(&closes, 3)[0], None);
}

#[test]
fn atr_returns_values_after_warmup() {
    let candles = vec![
        IndicatorCandle { high: 11.0, low: 9.0, close: 10.0 },
        IndicatorCandle { high: 12.0, low: 10.0, close: 11.0 },
        IndicatorCandle { high: 13.0, low: 11.0, close: 12.0 },
    ];
    let values = atr(&candles, 2);
    assert!(values[0].is_none());
    assert!(values[1].unwrap() > 0.0);
}

#[test]
fn rsi_bollinger_and_adx_handle_warmup() {
    let closes = [1.0, 1.1, 1.2, 1.1, 1.3, 1.4, 1.2, 1.5];
    assert!(rsi(&closes, 3).iter().any(Option::is_some));
    assert!(bollinger(&closes, 3, 2.0).iter().any(Option::is_some));
    let candles: Vec<_> = closes.iter().map(|close| IndicatorCandle { high: close + 0.1, low: close - 0.1, close: *close }).collect();
    assert!(adx(&candles, 3).iter().any(Option::is_some));
}
```

- [ ] **Step 2: Run tests and verify failure**

Run: `cargo test -p backtest-engine indicators -- --nocapture`  
Expected: FAIL because module/functions do not exist.

- [ ] **Step 3: Implement indicator functions**

Implement public functions:

```rust
pub struct IndicatorCandle { pub high: f64, pub low: f64, pub close: f64 }
pub fn sma(values: &[f64], period: usize) -> Vec<Option<f64>>
pub fn ema(values: &[f64], period: usize) -> Vec<Option<f64>>
pub fn atr(candles: &[IndicatorCandle], period: usize) -> Vec<Option<f64>>
pub fn rsi(closes: &[f64], period: usize) -> Vec<Option<f64>>
pub fn bollinger(closes: &[f64], period: usize, stddev: f64) -> Vec<Option<BollingerPoint>>
pub fn adx(candles: &[IndicatorCandle], period: usize) -> Vec<Option<f64>>
```

Use explicit `None` during warmup. Return all `None` for period `0`.

- [ ] **Step 4: Export module**

In `apps/backtest-engine/src/lib.rs`, add:

```rust
pub mod indicators;
```

- [ ] **Step 5: Run indicator tests**

Run: `cargo test -p backtest-engine indicators -- --nocapture`  
Expected: PASS.

- [ ] **Step 6: Commit if requested**

```bash
git add apps/backtest-engine/src/indicators.rs apps/backtest-engine/src/lib.rs
git commit -m "feat: add martingale indicator engine"
```

---

## Task 5: Add Exit Rules and Simulation State

**Files:**
- Create: `apps/backtest-engine/src/martingale/exit_rules.rs`
- Create: `apps/backtest-engine/src/martingale/state.rs`
- Modify: `apps/backtest-engine/src/martingale/mod.rs`

- [ ] **Step 1: Add exit rule tests**

Add tests:

```rust
#[test]
fn weighted_entry_for_long_cycle_uses_notional_weighting() {
    let legs = vec![open_leg(100.0, 0.1), open_leg(90.0, 0.2)];
    let avg = weighted_average_entry(&legs).unwrap();
    assert!((avg - 93.3333333333).abs() < 0.0001);
}

#[test]
fn percent_take_profit_for_short_is_below_average_entry() {
    let trigger = take_profit_price(100.0, MartingaleDirection::Short, &MartingaleTakeProfitModel::Percent { target_bps: 100 }, None).unwrap();
    assert_eq!(trigger, 99.0);
}

#[test]
fn global_stop_has_priority_over_take_profit() {
    let decision = evaluate_exit_priority(true, true, true, true);
    assert_eq!(decision, ExitDecision::GlobalStop);
}
```

- [ ] **Step 2: Run tests and verify failure**

Run: `cargo test -p backtest-engine martingale::exit_rules -- --nocapture`  
Expected: FAIL.

- [ ] **Step 3: Implement state structs**

In `state.rs`, define serializable structs:

```rust
pub struct MartingalePortfolioState { pub cash_quote: f64, pub reserved_margin_quote: f64, pub realized_pnl_quote: f64, pub equity_peak_quote: f64, pub symbols: BTreeMap<String, MartingaleSymbolState> }
pub struct MartingaleSymbolState { pub gross_exposure_quote: f64, pub net_exposure_quote: f64, pub long_exposure_quote: f64, pub short_exposure_quote: f64 }
pub struct MartingaleCycleState { pub cycle_id: String, pub direction: MartingaleDirection, pub anchor_price: f64, pub legs: Vec<MartingaleLegState>, pub trailing_high_watermark: Option<f64>, pub trailing_low_watermark: Option<f64> }
pub struct MartingaleLegState { pub leg_index: u32, pub price: f64, pub quantity: f64, pub notional_quote: f64, pub fee_quote: f64, pub slippage_quote: f64 }
```

- [ ] **Step 4: Implement exit rules**

Implement weighted entry, take-profit price, ATR/amount/trailing helpers, stop priority:

```rust
pub enum ExitDecision { None, TakeProfit, StrategyStop, SymbolStop, GlobalStop }
```

Priority: `GlobalStop > SymbolStop > StrategyStop > TakeProfit > None`.

- [ ] **Step 5: Wire module**

In `mod.rs`:

```rust
pub mod exit_rules;
pub mod state;
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p backtest-engine martingale::exit_rules -- --nocapture`  
Expected: PASS.

- [ ] **Step 7: Commit if requested**

```bash
git add apps/backtest-engine/src/martingale/exit_rules.rs apps/backtest-engine/src/martingale/state.rs apps/backtest-engine/src/martingale/mod.rs
git commit -m "feat: add martingale exit rules and simulation state"
```

---

## Task 6: Add Read-Only Market Data Adapter

**Files:**
- Create: `apps/backtest-engine/src/market_data.rs`
- Create: `apps/backtest-engine/src/sqlite_market_data.rs`
- Create: `apps/backtest-engine/src/bin/market_data_probe.rs`
- Modify: `apps/backtest-engine/src/lib.rs`
- Modify: `apps/backtest-engine/Cargo.toml`

- [ ] **Step 1: Add dependencies**

Add to `apps/backtest-engine/Cargo.toml`:

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
clap = { version = "4", features = ["derive"] }
sha2 = { workspace = true }
```

If workspace already pins a different compatible `rusqlite`, use that version instead.

- [ ] **Step 2: Add fixture tests**

Create tests in `sqlite_market_data.rs` that open a temp SQLite DB read-only and assert rows load. The fixture tables should be created in a temp file during test setup, then reopened read-only.

Test cases:
- `readonly_adapter_lists_symbols_from_fixture`
- `readonly_adapter_loads_klines_from_fixture`
- `readonly_adapter_rejects_missing_file_with_diagnostic`

- [ ] **Step 3: Run tests and verify failure**

Run: `cargo test -p backtest-engine sqlite_market_data -- --nocapture`  
Expected: FAIL because adapter does not exist.

- [ ] **Step 4: Implement market data trait**

In `market_data.rs` define:

```rust
pub struct KlineBar { pub symbol: String, pub open_time_ms: i64, pub open: f64, pub high: f64, pub low: f64, pub close: f64, pub volume: f64 }
pub struct AggTrade { pub symbol: String, pub trade_time_ms: i64, pub price: f64, pub quantity: f64, pub is_buyer_maker: bool }
pub struct DataQualityReport { pub missing_bars: u64, pub duplicate_bars: u64, pub zero_price_bars: u64, pub completeness_score: f64 }
pub trait MarketDataSource { fn list_symbols(&self) -> Result<Vec<String>, String>; fn load_klines(&self, symbol: &str, start_ms: i64, end_ms: i64, interval: &str) -> Result<Vec<KlineBar>, String>; fn load_agg_trades(&self, symbol: &str, start_ms: i64, end_ms: i64) -> Result<Vec<AggTrade>, String>; fn schema_fingerprint(&self) -> Result<String, String>; }
```

- [ ] **Step 5: Implement SQLite adapter**

`SqliteMarketDataSource::open_readonly(path)` must use read-only open flags and must not execute writes. If the real schema differs, adapter should inspect table names and return a clear diagnostic that lists tables and expected alternatives. The probe CLI should help finalize schema mapping later without changing the DB.

- [ ] **Step 6: Implement probe CLI**

CLI arguments:

```text
--db-path <path> --symbols BTCUSDT,ETHUSDT --from 2024-01-01 --to 2024-01-02 --interval 1m
```

It prints: sqlite version, file size, schema fingerprint, table list, selected symbol coverage, diagnostic errors.

- [ ] **Step 7: Run fixture tests**

Run: `cargo test -p backtest-engine sqlite_market_data -- --nocapture`  
Expected: PASS.

- [ ] **Step 8: Manual read-only diagnostic**

Run only when ready, never with write flags:

```bash
cargo run -p backtest-engine --bin market_data_probe -- --db-path /home/bumblebee/Project/discord_c2im/pipeline/data/market_data.db --symbols BTCUSDT --from 2024-01-01 --to 2024-01-02 --interval 1m
```

Expected: either schema/sample output or a clear diagnostic. No source DB files are modified.

- [ ] **Step 9: Commit if requested**

```bash
git add apps/backtest-engine/Cargo.toml apps/backtest-engine/src/market_data.rs apps/backtest-engine/src/sqlite_market_data.rs apps/backtest-engine/src/bin/market_data_probe.rs apps/backtest-engine/src/lib.rs
git commit -m "feat: add read-only market data adapter"
```

---

## Task 7: Add K-Line Screening Engine

**Files:**
- Create: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Create: `apps/backtest-engine/src/martingale/metrics.rs`
- Modify: `apps/backtest-engine/src/martingale/mod.rs`

- [ ] **Step 1: Add k-line engine tests**

Tests:

```rust
#[test]
fn long_cycle_adds_safety_order_and_takes_profit() {
    let config = MartingaleStrategyConfig::example_spot_long("BTCUSDT");
    let bars = vec![bar(100.0, 100.0, 100.0, 100.0), bar(100.0, 100.0, 98.9, 99.0), bar(99.0, 101.0, 99.0, 100.5)];
    let result = run_kline_screening(single_strategy_portfolio(config), &bars).unwrap();
    assert!(result.metrics.trade_count >= 2);
    assert!(result.events.iter().any(|event| event.event_type == "take_profit"));
}

#[test]
fn global_budget_blocks_new_leg() {
    let mut portfolio = single_strategy_portfolio(MartingaleStrategyConfig::example_spot_long("BTCUSDT"));
    portfolio.risk_limits.max_global_budget_quote = 15.0;
    let result = run_kline_screening(portfolio, &falling_bars()).unwrap();
    assert!(result.rejection_reasons.iter().any(|reason| reason.contains("budget")));
}
```

- [ ] **Step 2: Run tests and verify failure**

Run: `cargo test -p backtest-engine martingale::kline_engine -- --nocapture`  
Expected: FAIL.

- [ ] **Step 3: Implement screening result models**

Define:

```rust
pub struct MartingaleBacktestResult { pub metrics: MartingaleMetrics, pub events: Vec<MartingaleBacktestEvent>, pub equity_curve: Vec<EquityPoint>, pub rejection_reasons: Vec<String> }
pub struct MartingaleMetrics { pub total_return_pct: f64, pub max_drawdown_pct: f64, pub trade_count: u64, pub stop_count: u64, pub max_capital_used_quote: f64, pub survival_passed: bool }
pub struct MartingaleBacktestEvent { pub timestamp_ms: i64, pub event_type: String, pub symbol: String, pub strategy_instance_id: String, pub cycle_id: Option<String>, pub detail: String }
```

- [ ] **Step 4: Implement conservative OHLC flow**

For each bar:
- Evaluate stops before take-profit when both could be hit in same bar.
- For long, safety orders trigger when low <= trigger price.
- For short, safety orders trigger when high >= trigger price.
- Apply configured fees/slippage as simple conservative defaults for this task.
- Enforce budget before adding each leg.
- Update equity and max drawdown.

- [ ] **Step 5: Run k-line tests**

Run: `cargo test -p backtest-engine martingale::kline_engine -- --nocapture`  
Expected: PASS.

- [ ] **Step 6: Commit if requested**

```bash
git add apps/backtest-engine/src/martingale/kline_engine.rs apps/backtest-engine/src/martingale/metrics.rs apps/backtest-engine/src/martingale/mod.rs
git commit -m "feat: add martingale kline screening engine"
```

---

## Task 8: Add Trade Refinement, Scoring, and Search

**Files:**
- Create: `apps/backtest-engine/src/martingale/trade_engine.rs`
- Create: `apps/backtest-engine/src/martingale/scoring.rs`
- Create: `apps/backtest-engine/src/search.rs`
- Create: `apps/backtest-engine/src/intelligent_search.rs`
- Create: `apps/backtest-engine/src/time_splits.rs`
- Modify: `apps/backtest-engine/src/lib.rs`
- Modify: `apps/backtest-engine/src/martingale/mod.rs`

- [ ] **Step 1: Add search/scoring tests**

Tests:

```rust
#[test]
fn random_search_is_reproducible() {
    let space = SearchSpace::small_btc_eth_fixture();
    let first = sample_random_candidates(&space, 42, 10).unwrap();
    let second = sample_random_candidates(&space, 42, 10).unwrap();
    assert_eq!(first, second);
}

#[test]
fn survival_failure_never_outranks_valid_candidate() {
    let rejected = CandidateScoreInput { total_return_pct: 500.0, max_drawdown_pct: 90.0, liquidation_hit: true, ..CandidateScoreInput::fixture() };
    let valid = CandidateScoreInput { total_return_pct: 5.0, max_drawdown_pct: 2.0, liquidation_hit: false, ..CandidateScoreInput::fixture() };
    assert!(score_candidate(&valid, &ScoreWeights::default()).is_rankable());
    assert!(!score_candidate(&rejected, &ScoreWeights::default()).is_rankable());
}

#[test]
fn walk_forward_windows_are_generated_in_order() {
    let windows = walk_forward_windows("2024-01-01", "2024-06-30", 90, 30).unwrap();
    assert!(windows.len() >= 2);
    assert!(windows[0].train_start < windows[0].validation_start);
}
```

- [ ] **Step 2: Run tests and verify failure**

Run: `cargo test -p backtest-engine search scoring time_splits -- --nocapture`  
Expected: FAIL.

- [ ] **Step 3: Implement scoring**

Implement survival filters:
- liquidation hit.
- global drawdown exceeded.
- strategy drawdown exceeded.
- budget exceeded.
- excessive stop count.
- insufficient data quality.

Then compute weighted score from return, Calmar, Sortino, drawdown, stop frequency, capital utilization, trade stability.

- [ ] **Step 4: Implement random search**

Use a deterministic RNG seeded by `random_seed`. If adding `rand`, add to `apps/backtest-engine/Cargo.toml`:

```toml
rand = "0.8"
```

Search must enforce same-symbol futures margin/leverage conflict before returning candidates.

- [ ] **Step 5: Implement intelligent search**

Implement iterative process:
- Random round.
- Keep top survival-valid percentile.
- Mutate spacing/sizing/take-profit/leverage near winners.
- Stop by rounds, candidates, timeout, cancellation flag.

- [ ] **Step 6: Implement time splits**

Support manual windows, walk-forward windows, and named stress windows.

- [ ] **Step 7: Implement trade refinement skeleton**

`trade_engine.rs` replays `AggTrade` events and uses the same rules as k-line engine. It saves key events and metrics. Initial implementation can share most state transitions with k-line engine but must consume ordered trade prices.

- [ ] **Step 8: Run tests**

Run: `cargo test -p backtest-engine -- --nocapture`  
Expected: PASS.

- [ ] **Step 9: Commit if requested**

```bash
git add apps/backtest-engine/src/martingale/trade_engine.rs apps/backtest-engine/src/martingale/scoring.rs apps/backtest-engine/src/search.rs apps/backtest-engine/src/intelligent_search.rs apps/backtest-engine/src/time_splits.rs apps/backtest-engine/src/lib.rs apps/backtest-engine/src/martingale/mod.rs apps/backtest-engine/Cargo.toml
git commit -m "feat: add martingale search and refinement engine"
```

---

## Task 9: Add Persistence Schema and Repository

**Files:**
- Create: `db/migrations/0017_martingale_backtest_portfolios.sql`
- Create: `crates/shared-db/src/backtest.rs`
- Modify: `crates/shared-db/src/lib.rs`

- [ ] **Step 1: Add repository tests**

In `crates/shared-db/src/backtest.rs`, add tests for the ephemeral backend:

```rust
#[test]
fn ephemeral_backtest_repo_creates_and_updates_task() {
    let db = SharedDb::ephemeral().unwrap();
    let repo = db.backtest_repo();
    let task = repo.create_task(NewBacktestTaskRecord::fixture("user@example.com")).unwrap();
    assert_eq!(task.status, "queued");
    repo.transition_task(&task.task_id, "running").unwrap();
    assert_eq!(repo.find_task(&task.task_id).unwrap().unwrap().status, "running");
}

#[test]
fn ephemeral_backtest_repo_saves_candidate_and_artifact() {
    let db = SharedDb::ephemeral().unwrap();
    let repo = db.backtest_repo();
    let task = repo.create_task(NewBacktestTaskRecord::fixture("user@example.com")).unwrap();
    let candidate = repo.save_candidate(NewBacktestCandidateRecord::fixture(&task.task_id)).unwrap();
    let artifact = repo.save_artifact(NewBacktestArtifactRecord::fixture(&candidate.candidate_id)).unwrap();
    assert_eq!(artifact.candidate_id, candidate.candidate_id);
}
```

- [ ] **Step 2: Run tests and verify failure**

Run: `cargo test -p shared-db backtest -- --nocapture`  
Expected: FAIL because repo does not exist.

- [ ] **Step 3: Create migration**

Create tables:

```sql
CREATE TABLE IF NOT EXISTS backtest_quota_policies (...);
CREATE TABLE IF NOT EXISTS backtest_tasks (...);
CREATE TABLE IF NOT EXISTS backtest_task_events (...);
CREATE TABLE IF NOT EXISTS backtest_candidate_summaries (...);
CREATE TABLE IF NOT EXISTS backtest_artifacts (...);
CREATE TABLE IF NOT EXISTS martingale_portfolio_candidates (...);
CREATE TABLE IF NOT EXISTS martingale_portfolio_publish_records (...);
CREATE TABLE IF NOT EXISTS martingale_live_portfolios (...);
CREATE TABLE IF NOT EXISTS martingale_live_strategy_instances (...);
CREATE TABLE IF NOT EXISTS martingale_orphan_orders (...);
```

Use JSONB for configs/summaries, timestamptz for timestamps, and indexes by owner, status, task_id, candidate_id, portfolio_id.

- [ ] **Step 4: Implement repository records and methods**

Records:
- `BacktestTaskRecord`
- `NewBacktestTaskRecord`
- `BacktestCandidateRecord`
- `NewBacktestCandidateRecord`
- `BacktestArtifactRecord`
- `NewBacktestArtifactRecord`
- `BacktestQuotaPolicyRecord`

Methods:
- `create_task`
- `find_task`
- `list_tasks_for_owner`
- `transition_task`
- `append_task_event`
- `save_candidate`
- `list_candidates`
- `save_artifact`
- `find_quota_policy`

- [ ] **Step 5: Wire `SharedDb`**

In `crates/shared-db/src/lib.rs`, add:

```rust
pub mod backtest;
pub fn backtest_repo(&self) -> BacktestRepository { ... }
```

Add ephemeral storage vectors/maps to the existing ephemeral state.

- [ ] **Step 6: Run repository tests**

Run: `cargo test -p shared-db backtest -- --nocapture`  
Expected: PASS.

- [ ] **Step 7: Commit if requested**

```bash
git add db/migrations/0017_martingale_backtest_portfolios.sql crates/shared-db/src/backtest.rs crates/shared-db/src/lib.rs
git commit -m "feat: add martingale backtest persistence"
```

---

## Task 10: Add Backtest Worker

**Files:**
- Create: `apps/backtest-worker/Cargo.toml`
- Create: `apps/backtest-worker/src/main.rs`
- Modify: `Cargo.toml`
- Modify: `deploy/docker/docker-compose.yml`
- Create: `apps/backtest-engine/src/artifacts.rs`
- Modify: `apps/backtest-engine/src/lib.rs`

- [ ] **Step 1: Add artifact tests**

Tests:

```rust
#[test]
fn artifact_manifest_detects_checksum_mismatch() {
    let temp = tempfile::tempdir().unwrap();
    let manifest = write_json_artifact(temp.path(), "candidate-1", "equity", &[serde_json::json!({"equity": 100.0})]).unwrap();
    std::fs::write(&manifest.path, b"corrupted").unwrap();
    assert!(verify_artifact(&manifest).is_err());
}
```

Add `tempfile` dev dependency if needed.

- [ ] **Step 2: Run tests and verify failure**

Run: `cargo test -p backtest-engine artifacts -- --nocapture`  
Expected: FAIL.

- [ ] **Step 3: Implement artifact store**

Functions:

```rust
pub fn write_json_artifact(root: &Path, candidate_id: &str, kind: &str, rows: &[serde_json::Value]) -> Result<ArtifactManifest, String>
pub fn verify_artifact(manifest: &ArtifactManifest) -> Result<(), String>
```

Use SHA-256 checksums and store compact JSONL.

- [ ] **Step 4: Add worker crate**

Add workspace member `apps/backtest-worker`. Worker reads env:
- `DATABASE_URL`
- `REDIS_URL`
- `BACKTEST_ARTIFACT_ROOT`
- `BACKTEST_WORKER_MAX_THREADS`
- `BACKTEST_WORKER_POLL_MS`

- [ ] **Step 5: Implement worker loop**

Worker behavior:
- Poll queued task by priority.
- Mark running and heartbeat.
- Load task config.
- Run random/intelligent search.
- Run k-line screening.
- Run trade refinement for Top N.
- Save candidates and artifacts.
- Respect pause/cancel status between batches.
- Mark completed/failed.

- [ ] **Step 6: Add docker service**

In `deploy/docker/docker-compose.yml`, add `backtest-worker` service using the same image/build pattern as other Rust services, with artifact volume.

- [ ] **Step 7: Run worker compile check**

Run: `cargo check -p backtest-worker`  
Expected: PASS.

- [ ] **Step 8: Commit if requested**

```bash
git add Cargo.toml apps/backtest-worker apps/backtest-engine/src/artifacts.rs apps/backtest-engine/src/lib.rs deploy/docker/docker-compose.yml
git commit -m "feat: add martingale backtest worker"
```

---

## Task 11: Add API Services and Routes

**Files:**
- Create: `apps/api-server/src/services/backtest_service.rs`
- Create: `apps/api-server/src/services/martingale_publish_service.rs`
- Create: `apps/api-server/src/routes/admin_backtest.rs`
- Create: `apps/api-server/src/routes/martingale_portfolios.rs`
- Modify: `apps/api-server/src/routes/backtest.rs`
- Modify: `apps/api-server/src/lib.rs`
- Create: `apps/api-server/tests/backtest_flow.rs`
- Create: `apps/api-server/tests/martingale_backtest_flow.rs`

- [ ] **Step 1: Add API tests**

Test cases:

```rust
#[tokio::test]
async fn user_can_create_martingale_backtest_task() { /* register/login, POST /backtest/tasks, expect 201 queued */ }

#[tokio::test]
async fn quota_rejects_too_many_symbols() { /* configure quota, request more symbols, expect 403 or 400 with quota message */ }

#[tokio::test]
async fn task_pause_resume_cancel_transitions_status() { /* create task, pause, resume, cancel */ }

#[tokio::test]
async fn publish_intent_returns_risk_summary() { /* save refined candidate, POST publish-intent, expect pending summary */ }

#[tokio::test]
async fn publish_rejects_same_symbol_leverage_conflict() { /* existing live BTCUSDT 3x, candidate BTCUSDT 5x, expect conflict */ }

#[tokio::test]
async fn legacy_backtest_run_still_works() { /* POST /backtest/run with simple grid body, expect legacy result shape */ }
```

- [ ] **Step 2: Run tests and verify failure**

Run: `cargo test -p api-server --test martingale_backtest_flow -- --nocapture`  
Expected: FAIL.

- [ ] **Step 3: Implement `BacktestService`**

Methods:
- `create_task(owner, request)`
- `list_tasks(owner)`
- `get_task(owner, task_id)`
- `pause_task(owner, task_id)`
- `resume_task(owner, task_id)`
- `cancel_task(owner, task_id)`
- `list_candidates(owner, task_id)`
- `get_candidate(owner, candidate_id)`
- `create_publish_intent(owner, candidate_id)`

Validate quota before creating task.

- [ ] **Step 4: Implement publish service**

Methods:
- `risk_summary_for_candidate`
- `create_pending_portfolio`
- `confirm_start_portfolio`
- `validate_futures_symbol_compatibility`

- [ ] **Step 5: Implement routes**

Routes:
- `POST /backtest/tasks`
- `GET /backtest/tasks`
- `GET /backtest/tasks/:id`
- `POST /backtest/tasks/:id/pause`
- `POST /backtest/tasks/:id/resume`
- `POST /backtest/tasks/:id/cancel`
- `GET /backtest/tasks/:id/candidates`
- `GET /backtest/candidates/:id`
- `POST /backtest/candidates/:id/publish-intent`
- `POST /backtest/portfolios/:id/confirm-start`
- Admin quota endpoints.
- Portfolio operation endpoints.

Keep legacy `/backtest/run` endpoint working.

- [ ] **Step 6: Wire services in app state**

Add services to `AppState` and `FromRef` in `apps/api-server/src/lib.rs`.

- [ ] **Step 7: Run API tests**

Run: `cargo test -p api-server --test backtest_flow --test martingale_backtest_flow -- --nocapture`  
Expected: PASS.

- [ ] **Step 8: Commit if requested**

```bash
git add apps/api-server/src/services/backtest_service.rs apps/api-server/src/services/martingale_publish_service.rs apps/api-server/src/routes/backtest.rs apps/api-server/src/routes/admin_backtest.rs apps/api-server/src/routes/martingale_portfolios.rs apps/api-server/src/lib.rs apps/api-server/tests/backtest_flow.rs apps/api-server/tests/martingale_backtest_flow.rs
git commit -m "feat: add martingale backtest API"
```

---

## Task 12: Add Dedicated Backtest UI

**Files:**
- Replace: `apps/web/app/[locale]/app/backtest/page.tsx`
- Create: `apps/web/components/backtest/backtest-console.tsx`
- Create: `apps/web/components/backtest/backtest-wizard.tsx`
- Create: `apps/web/components/backtest/backtest-professional-panel.tsx`
- Create: `apps/web/components/backtest/martingale-parameter-editor.tsx`
- Create: `apps/web/components/backtest/indicator-rule-editor.tsx`
- Create: `apps/web/components/backtest/risk-rule-editor.tsx`
- Create: `apps/web/components/backtest/search-config-editor.tsx`
- Create: `apps/web/components/backtest/time-split-editor.tsx`
- Create: `apps/web/components/backtest/backtest-task-list.tsx`
- Create: `apps/web/components/backtest/backtest-result-table.tsx`
- Create: `apps/web/components/backtest/backtest-charts.tsx`
- Create: `apps/web/components/backtest/portfolio-candidate-review.tsx`
- Create/modify: `apps/web/app/api/user/backtest/**/route.ts`
- Create: `tests/verification/backtest_console_contract.test.mjs`

- [ ] **Step 1: Add UI contract test**

Create `tests/verification/backtest_console_contract.test.mjs`:

```js
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";

const page = readFileSync("apps/web/app/[locale]/app/backtest/page.tsx", "utf8");
assert.match(page, /BacktestConsole/);
assert.ok(existsSync("apps/web/components/backtest/backtest-console.tsx"));
const consoleSource = readFileSync("apps/web/components/backtest/backtest-console.tsx", "utf8");
for (const text of ["随机搜索", "智能搜索", "Hedge Mode", "逐仓", "全仓", "Portfolio", "生存优先"]) {
  assert.match(consoleSource, new RegExp(text));
}
```

- [ ] **Step 2: Run contract test and verify failure**

Run: `node --test tests/verification/backtest_console_contract.test.mjs`  
Expected: FAIL.

- [ ] **Step 3: Implement console shell**

`page.tsx` should render `<BacktestConsole lang={lang} />`.

`BacktestConsole` should include:
- Wizard tab.
- Professional console tab.
- Task list panel.
- Candidate result table.
- Portfolio candidate review panel.

- [ ] **Step 4: Implement wizard sections**

Wizard must show controls/labels for:
- Data ranges.
- Symbol pool all USDT/whitelist/blacklist.
- Spot/futures.
- Long/short/long+short.
- Hedge Mode warning.
- Isolated/cross.
- Leverage search range.
- Spacing/sizing/take-profit/stop-loss.
- Indicators ATR, MA/EMA, RSI, Bollinger, ADX.
- Random/intelligent search.
- Walk-forward/stress windows.

- [ ] **Step 5: Implement API proxy route handlers**

Proxy to Rust API for task create/list/detail/actions and candidate publish intent. Follow existing cookie/session style in `apps/web/app/api/user/*`.

- [ ] **Step 6: Run web verification**

Run: `node --test tests/verification/backtest_console_contract.test.mjs tests/verification/web_app_shell.test.mjs`  
Expected: PASS.

- [ ] **Step 7: Run build if available**

Run from `apps/web`: `npm run build`  
Expected: PASS or report pre-existing unrelated build issues.

- [ ] **Step 8: Commit if requested**

```bash
git add apps/web/app/[locale]/app/backtest/page.tsx apps/web/components/backtest apps/web/app/api/user/backtest tests/verification/backtest_console_contract.test.mjs
git commit -m "feat: add martingale backtest console"
```

---

## Task 13: Add Live Martingale Runtime

**Files:**
- Create: `apps/trading-engine/src/martingale_runtime.rs`
- Create: `apps/trading-engine/src/martingale_recovery.rs`
- Modify: `apps/trading-engine/src/runtime.rs`
- Modify: `apps/trading-engine/src/order_sync.rs`
- Modify: `apps/trading-engine/src/trade_sync.rs`
- Modify: `apps/trading-engine/src/lib.rs`
- Create: `apps/trading-engine/tests/martingale_runtime.rs`
- Create: `apps/trading-engine/tests/martingale_recovery.rs`

- [ ] **Step 1: Add runtime tests**

Test cases:

```rust
#[test]
fn long_cycle_places_first_order_then_safety_order() { /* fixture exchange, price moves down, expect leg 0 and leg 1 */ }

#[test]
fn short_cycle_places_safety_order_above_anchor() { /* price moves up, expect short leg 1 */ }

#[test]
fn long_and_short_cycles_remain_independent() { /* same symbol, hedge mode, separate cycle ids */ }

#[test]
fn global_drawdown_pauses_new_entries() { /* drawdown exceeded, no new cycle */ }

#[test]
fn orphan_order_pauses_strategy() { /* unknown client order id, strategy needs_attention */ }
```

- [ ] **Step 2: Run tests and verify failure**

Run: `cargo test -p trading-engine --test martingale_runtime --test martingale_recovery -- --nocapture`  
Expected: FAIL.

- [ ] **Step 3: Implement runtime module**

Runtime must:
- Branch only for `StrategyType::MartingaleGrid`.
- Use shared martingale rule evaluators.
- Generate client order IDs containing `portfolio_id`, `strategy_instance_id`, `cycle_id`, `direction`, `leg_index`.
- Enforce strategy/symbol/direction/global budgets before placing orders.
- Keep long and short cycles independent.
- Respect Portfolio pause-new-entries and strategy pause/resume/stop.

- [ ] **Step 4: Implement futures checks adapter**

Before start:
- Read position mode.
- Require Hedge Mode for long+short.
- Read symbol margin type/leverage.
- Reject conflicts with existing live same-symbol settings.
- Do not silently modify settings.

- [ ] **Step 5: Implement conservative recovery**

Recovery must:
- Sync positions, open orders, recent trades.
- Match only known client order IDs.
- Mark ambiguous orders as orphan.
- Pause affected strategy.
- Block new legs while recovery is incomplete.

- [ ] **Step 6: Run trading tests**

Run: `cargo test -p trading-engine --test martingale_runtime --test martingale_recovery -- --nocapture`  
Expected: PASS.

- [ ] **Step 7: Commit if requested**

```bash
git add apps/trading-engine/src/martingale_runtime.rs apps/trading-engine/src/martingale_recovery.rs apps/trading-engine/src/runtime.rs apps/trading-engine/src/order_sync.rs apps/trading-engine/src/trade_sync.rs apps/trading-engine/src/lib.rs apps/trading-engine/tests/martingale_runtime.rs apps/trading-engine/tests/martingale_recovery.rs
git commit -m "feat: add live martingale portfolio runtime"
```

---

## Task 14: Add Portfolio UI and Operations

**Files:**
- Create: `apps/web/app/[locale]/app/martingale-portfolios/page.tsx`
- Create: `apps/web/app/[locale]/app/martingale-portfolios/[id]/page.tsx`
- Create: `apps/web/components/backtest/live-portfolio-controls.tsx`
- Modify: `apps/web/components/layout/sidebar.tsx`
- Create/modify: `apps/web/app/api/user/martingale-portfolios/**/route.ts`
- Modify: `tests/verification/web_app_shell.test.mjs`

- [ ] **Step 1: Add shell verification**

Add assertions that navigation includes Martingale Portfolios and pages exist.

- [ ] **Step 2: Run verification and verify failure**

Run: `node --test tests/verification/web_app_shell.test.mjs`  
Expected: FAIL until pages/nav exist.

- [ ] **Step 3: Implement Portfolio list page**

Show:
- Portfolio name.
- Status.
- Total equity/risk summary.
- Active strategy count.
- Needs-attention/orphan warnings.

- [ ] **Step 4: Implement Portfolio detail page**

Show:
- Strategy instances grouped by symbol/direction.
- Strategy-level metrics.
- Symbol-level exposure.
- Global Portfolio drawdown.
- Pause new entries, stop Portfolio, pause/resume/stop single strategy.

- [ ] **Step 5: Add API proxies**

Proxy list/detail/operations to Rust API.

- [ ] **Step 6: Run web verification**

Run: `node --test tests/verification/web_app_shell.test.mjs tests/verification/backtest_console_contract.test.mjs`  
Expected: PASS.

- [ ] **Step 7: Commit if requested**

```bash
git add apps/web/app/[locale]/app/martingale-portfolios apps/web/components/backtest/live-portfolio-controls.tsx apps/web/components/layout/sidebar.tsx apps/web/app/api/user/martingale-portfolios tests/verification/web_app_shell.test.mjs
git commit -m "feat: add martingale portfolio operations UI"
```

---

## Task 15: Documentation and Deployment

**Files:**
- Create: `docs/user-guide/zh/martingale-backtest.md`
- Create: `docs/user-guide/martingale-backtest.md`
- Modify: `docs/deployment/env-and-secrets.md`
- Modify: `docs/deployment/docker-compose.md`

- [ ] **Step 1: Write user docs**

Docs must explain:
- What martingale grid means.
- Why it is risky.
- How two-stage backtest works.
- How survival-first scoring works.
- How Portfolio publish confirmation works.
- Why live results may differ from backtest.

- [ ] **Step 2: Write deployment docs**

Docs must list env vars:
- `BACKTEST_ARTIFACT_ROOT`
- `BACKTEST_WORKER_MAX_THREADS`
- `BACKTEST_WORKER_POLL_MS`
- `BACKTEST_MARKET_DATA_DB_PATH`

Docs must state external market DB is read-only.

- [ ] **Step 3: Run doc grep checks**

Run:

```bash
rg -n "martingale|马丁|BACKTEST_ARTIFACT_ROOT|read-only|只读" docs/user-guide docs/deployment
```

Expected: relevant docs found.

- [ ] **Step 4: Commit if requested**

```bash
git add docs/user-guide/zh/martingale-backtest.md docs/user-guide/martingale-backtest.md docs/deployment/env-and-secrets.md docs/deployment/docker-compose.md
git commit -m "docs: add martingale backtest operations guide"
```

---

## Task 16: Final Verification

**Files:**
- No new files unless fixing issues found by tests.

- [ ] **Step 1: Run targeted domain tests**

Run: `cargo test -p shared-domain martingale -- --nocapture`  
Expected: PASS.

- [ ] **Step 2: Run backtest engine tests**

Run: `cargo test -p backtest-engine -- --nocapture`  
Expected: PASS.

- [ ] **Step 3: Run API tests**

Run: `cargo test -p api-server --test backtest_flow --test martingale_backtest_flow -- --nocapture`  
Expected: PASS.

- [ ] **Step 4: Run trading engine tests**

Run: `cargo test -p trading-engine --test martingale_runtime --test martingale_recovery -- --nocapture`  
Expected: PASS.

- [ ] **Step 5: Run web verification tests**

Run: `node --test tests/verification/backtest_console_contract.test.mjs tests/verification/web_app_shell.test.mjs tests/verification/strategy_surface_contract.test.mjs`  
Expected: PASS.

- [ ] **Step 6: Run workspace tests**

Run: `cargo test --workspace --tests`  
Expected: PASS. Some existing tests may require escalation because they bind local ports or start containers.

- [ ] **Step 7: Run web build**

Run from `apps/web`: `npm run build`  
Expected: PASS or document unrelated pre-existing build failures.

- [ ] **Step 8: Manual acceptance checklist**

Verify manually:
- Create random-search task.
- Create intelligent-search task.
- See survival rejection reasons.
- See train/validation/test/stress split metrics.
- Refine Top N candidates.
- Create publish intent and risk summary.
- Confirm Portfolio start.
- Pause Portfolio new entries.
- Pause/resume single strategy.
- Reject same-symbol leverage conflict.
- Mark orphan order and pause strategy.

---

## Plan Self-Review

### Spec Coverage

- Mixed martingale strategy parameters: covered by Tasks 1, 3, 5, 7, 8, 12.
- Spot + USDT-M futures: covered by Tasks 1, 11, 13.
- Hedge Mode and isolated/cross/leverage constraints: covered by Tasks 1, 11, 13.
- Same-symbol conflict rule: covered by Tasks 1, 11, 13.
- Two-stage K-line and trade refinement: covered by Tasks 7 and 8.
- Random and intelligent search: covered by Task 8.
- Anti-overfitting time splits: covered by Task 8.
- Dedicated UI: covered by Tasks 12 and 14.
- Worker process: covered by Task 10.
- Mixed storage: covered by Tasks 9 and 10.
- External market DB read-only: covered by Task 6.
- Semi-automatic Portfolio publish: covered by Task 11.
- Live runtime and conservative recovery: covered by Task 13.
- Docs/deployment: covered by Task 15.
- Final verification: covered by Task 16.

### Placeholder Scan

This plan intentionally contains no TBD/TODO placeholders. Every task has concrete files, test intent, commands, and expected outcomes.

### Type Consistency

The main shared types are consistently named `MartingalePortfolioConfig`, `MartingaleStrategyConfig`, `MartingaleDirection`, `MartingaleMarginMode`, `MartingaleSpacingModel`, `MartingaleSizingModel`, `MartingaleTakeProfitModel`, and `MartingaleStopLossModel` across domain, engine, API, runtime, and UI tasks.
