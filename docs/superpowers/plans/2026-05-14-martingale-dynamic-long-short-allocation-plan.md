# Martingale Dynamic Long/Short Allocation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a dynamic long/short martingale backtest and portfolio optimizer that searches per-symbol Top10 strategy packages, computes portfolio Top10 allocations, visualizes long/short weights, and publishes live-compatible dynamic rules.

**Architecture:** Add small, focused modules to `apps/backtest-engine/src/martingale` for market regime classification, allocation policy, dynamic stop/forced-exit risk rules, and portfolio optimization. Keep existing `kline_engine.rs` as the execution loop but feed it optional dynamic allocation state, then surface new summary/artifact fields through `apps/backtest-worker`, API persistence, and React backtest UI.

**Tech Stack:** Rust workspace (`backtest-engine`, `backtest-worker`, `api-server`, `shared-domain`), SQLite market data reader, TypeScript/React/Next.js web components, Recharts, Node verification tests, Cargo tests.

---

## File Structure

Create focused Rust modules instead of growing `kline_engine.rs` further:

- Create `apps/backtest-engine/src/martingale/regime.rs`  
  Computes 4H/1D market regime labels from K-line bars for symbol and BTC filter.
- Create `apps/backtest-engine/src/martingale/allocation.rs`  
  Converts regime snapshots into long/short target weights, cooldown decisions, pause/cancel/forced-exit actions, and weight curve points.
- Create `apps/backtest-engine/src/martingale/portfolio_optimizer.rs`  
  Selects portfolio Top10 from per-symbol Top10 candidates using 10% coarse then 5% fine weight search and symbol/package caps.
- Modify `apps/backtest-engine/src/martingale/metrics.rs`  
  Adds allocation curve, regime timeline, cost summary, rebalance metrics, and contribution rows to backtest result artifacts.
- Modify `apps/backtest-engine/src/martingale/kline_engine.rs`  
  Applies dynamic allocation actions during the existing K-line loop and records forced exits with fees/slippage/PnL.
- Modify `apps/backtest-engine/src/search.rs`  
  Adds optional dynamic allocation search knobs and default short-side stop-loss generation.
- Modify `apps/backtest-engine/src/martingale/scoring.rs`  
  Enforces drawdown as a hard constraint and penalizes rebalance/forced-exit/stop-loss churn.
- Modify `apps/backtest-engine/src/lib.rs` and `apps/backtest-engine/src/martingale/mod.rs`  
  Exposes new modules.
- Modify `apps/backtest-worker/src/main.rs`  
  Emits per-symbol Top10 dynamic packages, portfolio Top10 summaries, max drawdown defaults, dynamic publish payload fields, and progress stages.
- Modify `apps/api-server/src/services/martingale_publish_service.rs` and `apps/api-server/src/routes/martingale_portfolios.rs`  
  Persists dynamic allocation rules in publish intent and rejects live publish when required execution capability is missing.
- Modify `apps/web/components/backtest/backtest-wizard.tsx`  
  Adds risk-profile drawdown defaults and manual override behavior.
- Modify `apps/web/components/backtest/backtest-charts.tsx`  
  Adds hoverable long/short allocation chart and regime timeline.
- Modify `apps/web/components/backtest/backtest-result-table.tsx`, `portfolio-candidate-review.tsx`, and `live-portfolio-controls.tsx`  
  Displays single-symbol Top10, portfolio Top10, costs, actions, selected dynamic package, and live-readiness warnings.
- Add/modify tests in `apps/backtest-engine/tests/search_scoring_time_splits.rs`, `apps/api-server/tests/martingale_backtest_flow.rs`, `tests/verification/martingale_backtest_rebuild_contract.test.mjs`, `tests/verification/martingale_portfolio_contract.test.mjs`, and `tests/verification/backtest_worker_contract.test.mjs`.

## Task 1: Domain Result Types And Serialization

**Files:**
- Modify: `apps/backtest-engine/src/martingale/metrics.rs`
- Modify: `apps/backtest-engine/src/martingale/mod.rs`
- Test: `apps/backtest-engine/tests/search_scoring_time_splits.rs`

- [ ] **Step 1: Add failing serialization test**

Add this test to `apps/backtest-engine/tests/search_scoring_time_splits.rs`:

```rust
use backtest_engine::martingale::metrics::{
    AllocationAction, AllocationCurvePoint, CostSummary, MarketRegimeLabel,
    MartingaleBacktestResult, MartingaleMetrics, RegimeTimelinePoint,
};

#[test]
fn dynamic_allocation_metrics_serialize_for_worker_artifacts() {
    let result = MartingaleBacktestResult {
        metrics: MartingaleMetrics {
            total_return_pct: 18.4,
            max_drawdown_pct: 12.1,
            global_drawdown_pct: Some(12.1),
            max_strategy_drawdown_pct: Some(9.7),
            data_quality_score: Some(1.0),
            trade_count: 42,
            stop_count: 2,
            max_capital_used_quote: 1500.0,
            survival_passed: true,
        },
        events: Vec::new(),
        equity_curve: Vec::new(),
        rejection_reasons: Vec::new(),
        allocation_curve: vec![AllocationCurvePoint {
            timestamp_ms: 1_704_067_200_000,
            symbol: "BTCUSDT".to_owned(),
            long_weight_pct: 80.0,
            short_weight_pct: 20.0,
            action: AllocationAction::Rebalance,
            reason: "btc_range_symbol_uptrend".to_owned(),
            in_cooldown: false,
        }],
        regime_timeline: vec![RegimeTimelinePoint {
            timestamp_ms: 1_704_067_200_000,
            symbol: "BTCUSDT".to_owned(),
            btc_regime: MarketRegimeLabel::Range,
            symbol_regime: MarketRegimeLabel::Uptrend,
            extreme_risk: false,
        }],
        cost_summary: CostSummary {
            fee_quote: 4.2,
            slippage_quote: 2.1,
            stop_loss_quote: 8.0,
            forced_exit_quote: 0.0,
        },
        rebalance_count: 1,
        forced_exit_count: 0,
        average_allocation_hold_hours: Some(16.0),
    };

    let json = serde_json::to_value(result).unwrap();
    assert_eq!(json["allocation_curve"][0]["long_weight_pct"], 80.0);
    assert_eq!(json["regime_timeline"][0]["btc_regime"], "range");
    assert_eq!(json["cost_summary"]["fee_quote"], 4.2);
    assert_eq!(json["rebalance_count"], 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p backtest-engine dynamic_allocation_metrics_serialize_for_worker_artifacts -- --nocapture`  
Expected: FAIL because allocation/cost/regime types and result fields do not exist.

- [ ] **Step 3: Implement result types**

In `apps/backtest-engine/src/martingale/metrics.rs`, add serializable types and extend `MartingaleBacktestResult`:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarketRegimeLabel {
    StrongUptrend,
    Uptrend,
    Range,
    Downtrend,
    StrongDowntrend,
    HighVolatility,
    ExtremeRisk,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AllocationAction {
    None,
    Rebalance,
    DirectionPaused,
    DirectionOrdersCancelled,
    DirectionForcedExit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AllocationCurvePoint {
    pub timestamp_ms: i64,
    pub symbol: String,
    pub long_weight_pct: f64,
    pub short_weight_pct: f64,
    pub action: AllocationAction,
    pub reason: String,
    pub in_cooldown: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegimeTimelinePoint {
    pub timestamp_ms: i64,
    pub symbol: String,
    pub btc_regime: MarketRegimeLabel,
    pub symbol_regime: MarketRegimeLabel,
    pub extreme_risk: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct CostSummary {
    pub fee_quote: f64,
    pub slippage_quote: f64,
    pub stop_loss_quote: f64,
    pub forced_exit_quote: f64,
}
```

Extend `MartingaleBacktestResult` with serde defaults so old artifacts still deserialize:

```rust
#[serde(default)]
pub allocation_curve: Vec<AllocationCurvePoint>,
#[serde(default)]
pub regime_timeline: Vec<RegimeTimelinePoint>,
#[serde(default)]
pub cost_summary: CostSummary,
#[serde(default)]
pub rebalance_count: u64,
#[serde(default)]
pub forced_exit_count: u64,
#[serde(default)]
pub average_allocation_hold_hours: Option<f64>,
```

Update every constructor in `kline_engine.rs`, `trade_engine.rs`, and tests to fill these fields with defaults when dynamic allocation is not used.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p backtest-engine dynamic_allocation_metrics_serialize_for_worker_artifacts -- --nocapture`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-engine/src/martingale/metrics.rs apps/backtest-engine/src/martingale/mod.rs apps/backtest-engine/tests/search_scoring_time_splits.rs
git commit -m "feat: add dynamic allocation backtest metrics

问题描述: 马丁动态多空回测缺少权重曲线、市场状态和成本指标的统一结果结构。
修复思路: 新增可序列化指标类型并保持旧结果默认兼容。"
```

## Task 2: Regime Classifier

**Files:**
- Create: `apps/backtest-engine/src/martingale/regime.rs`
- Modify: `apps/backtest-engine/src/martingale/mod.rs`
- Test: `apps/backtest-engine/tests/search_scoring_time_splits.rs`

- [ ] **Step 1: Add failing regime tests**

Add tests:

```rust
use backtest_engine::market_data::KlineBar;
use backtest_engine::martingale::metrics::MarketRegimeLabel;
use backtest_engine::martingale::regime::{classify_regime, RegimeConfig};

fn bar(symbol: &str, t: i64, open: f64, high: f64, low: f64, close: f64) -> KlineBar {
    KlineBar { symbol: symbol.to_owned(), open_time_ms: t, open, high, low, close, volume: 1000.0 }
}

#[test]
fn regime_classifier_detects_strong_uptrend_and_range() {
    let up = (0..80)
        .map(|i| bar("BTCUSDT", i * 14_400_000, 100.0 + i as f64, 102.0 + i as f64, 99.0 + i as f64, 101.0 + i as f64))
        .collect::<Vec<_>>();
    let range = (0..80)
        .map(|i| bar("ETHUSDT", i * 14_400_000, 100.0, 101.0, 99.0, 100.0 + (i % 2) as f64 * 0.2))
        .collect::<Vec<_>>();

    assert_eq!(classify_regime(&up, &RegimeConfig::default()).unwrap().label, MarketRegimeLabel::StrongUptrend);
    assert_eq!(classify_regime(&range, &RegimeConfig::default()).unwrap().label, MarketRegimeLabel::Range);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p backtest-engine regime_classifier_detects_strong_uptrend_and_range -- --nocapture`  
Expected: FAIL because `regime` module does not exist.

- [ ] **Step 3: Implement classifier**

Create `apps/backtest-engine/src/martingale/regime.rs` with:

```rust
use crate::indicators::{adx, atr, ema, IndicatorCandle};
use crate::market_data::KlineBar;
use crate::martingale::metrics::MarketRegimeLabel;

#[derive(Debug, Clone, Copy)]
pub struct RegimeConfig {
    pub fast_ema_period: usize,
    pub slow_ema_period: usize,
    pub adx_period: usize,
    pub atr_period: usize,
    pub strong_adx: f64,
    pub high_volatility_atr_pct: f64,
    pub slope_bps: f64,
}

impl Default for RegimeConfig {
    fn default() -> Self {
        Self {
            fast_ema_period: 20,
            slow_ema_period: 50,
            adx_period: 14,
            atr_period: 14,
            strong_adx: 25.0,
            high_volatility_atr_pct: 6.0,
            slope_bps: 20.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegimeSnapshot {
    pub timestamp_ms: i64,
    pub label: MarketRegimeLabel,
    pub ema_spread_bps: f64,
    pub adx: f64,
    pub atr_pct: f64,
}

pub fn classify_regime(bars: &[KlineBar], config: &RegimeConfig) -> Result<RegimeSnapshot, String> {
    let Some(last) = bars.last() else { return Err("regime requires at least one bar".to_owned()); };
    let candles = bars.iter().map(|bar| IndicatorCandle {
        high: bar.high,
        low: bar.low,
        close: bar.close,
    }).collect::<Vec<_>>();
    let fast = ema(&candles, config.fast_ema_period).last().and_then(|value| *value).unwrap_or(last.close);
    let slow = ema(&candles, config.slow_ema_period).last().and_then(|value| *value).unwrap_or(last.close);
    let latest_adx = adx(&candles, config.adx_period).last().and_then(|value| *value).unwrap_or(0.0);
    let latest_atr = atr(&candles, config.atr_period).last().and_then(|value| *value).unwrap_or(0.0);
    let ema_spread_bps = if slow.abs() > f64::EPSILON { (fast - slow) / slow * 10_000.0 } else { 0.0 };
    let atr_pct = if last.close > 0.0 { latest_atr / last.close * 100.0 } else { 0.0 };
    let label = if atr_pct >= config.high_volatility_atr_pct {
        MarketRegimeLabel::HighVolatility
    } else if ema_spread_bps >= config.slope_bps && latest_adx >= config.strong_adx {
        MarketRegimeLabel::StrongUptrend
    } else if ema_spread_bps >= config.slope_bps {
        MarketRegimeLabel::Uptrend
    } else if ema_spread_bps <= -config.slope_bps && latest_adx >= config.strong_adx {
        MarketRegimeLabel::StrongDowntrend
    } else if ema_spread_bps <= -config.slope_bps {
        MarketRegimeLabel::Downtrend
    } else {
        MarketRegimeLabel::Range
    };

    Ok(RegimeSnapshot { timestamp_ms: last.open_time_ms, label, ema_spread_bps, adx: latest_adx, atr_pct })
}
```

Add `pub mod regime;` to `apps/backtest-engine/src/martingale/mod.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p backtest-engine regime_classifier_detects_strong_uptrend_and_range -- --nocapture`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-engine/src/martingale/regime.rs apps/backtest-engine/src/martingale/mod.rs apps/backtest-engine/tests/search_scoring_time_splits.rs
git commit -m "feat: classify martingale market regimes

问题描述: 动态多空配比需要透明的本币和BTC市场状态识别。
修复思路: 新增EMA/ADX/ATR规则分类器并覆盖趋势与震荡测试。"
```

## Task 3: Allocation Policy And Cooldown

**Files:**
- Create: `apps/backtest-engine/src/martingale/allocation.rs`
- Modify: `apps/backtest-engine/src/martingale/mod.rs`
- Test: `apps/backtest-engine/tests/search_scoring_time_splits.rs`

- [ ] **Step 1: Add failing allocation tests**

Add tests:

```rust
use backtest_engine::martingale::allocation::{AllocationConfig, AllocationState, decide_allocation};
use backtest_engine::martingale::metrics::{AllocationAction, MarketRegimeLabel};

#[test]
fn allocation_closes_short_weight_when_btc_and_symbol_are_strong_up() {
    let mut state = AllocationState::default();
    let decision = decide_allocation(
        1_704_067_200_000,
        "ETHUSDT",
        MarketRegimeLabel::StrongUptrend,
        MarketRegimeLabel::StrongUptrend,
        0.0,
        &AllocationConfig::balanced(),
        &mut state,
    );
    assert_eq!(decision.long_weight_pct, 100.0);
    assert_eq!(decision.short_weight_pct, 0.0);
    assert_eq!(decision.action, AllocationAction::DirectionOrdersCancelled);
}

#[test]
fn allocation_cooldown_blocks_small_weight_flip() {
    let mut state = AllocationState::default();
    let config = AllocationConfig::balanced();
    let first = decide_allocation(0, "ETHUSDT", MarketRegimeLabel::Range, MarketRegimeLabel::Range, 0.0, &config, &mut state);
    let second = decide_allocation(4 * 3_600_000, "ETHUSDT", MarketRegimeLabel::Uptrend, MarketRegimeLabel::Uptrend, 0.0, &config, &mut state);
    assert_eq!(first.long_weight_pct, 60.0);
    assert_eq!(second.long_weight_pct, 60.0);
    assert!(second.in_cooldown);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p backtest-engine allocation_closes_short_weight_when_btc_and_symbol_are_strong_up allocation_cooldown_blocks_small_weight_flip -- --nocapture`  
Expected: FAIL because `allocation` module does not exist.

- [ ] **Step 3: Implement allocation policy**

Create `apps/backtest-engine/src/martingale/allocation.rs` with:

```rust
use crate::martingale::metrics::{AllocationAction, AllocationCurvePoint, MarketRegimeLabel};

#[derive(Debug, Clone, Copy)]
pub struct AllocationConfig {
    pub cooldown_hours: i64,
    pub forced_exit_loss_pct: f64,
}

impl AllocationConfig {
    pub fn conservative() -> Self { Self { cooldown_hours: 24, forced_exit_loss_pct: 20.0 } }
    pub fn balanced() -> Self { Self { cooldown_hours: 16, forced_exit_loss_pct: 25.0 } }
    pub fn aggressive() -> Self { Self { cooldown_hours: 12, forced_exit_loss_pct: 30.0 } }
}

#[derive(Debug, Clone, Default)]
pub struct AllocationState {
    pub last_change_ms: Option<i64>,
    pub long_weight_pct: f64,
    pub short_weight_pct: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AllocationDecision {
    pub point: AllocationCurvePoint,
    pub long_weight_pct: f64,
    pub short_weight_pct: f64,
    pub action: AllocationAction,
    pub force_exit_long: bool,
    pub force_exit_short: bool,
    pub in_cooldown: bool,
}

pub fn decide_allocation(
    timestamp_ms: i64,
    symbol: &str,
    btc_regime: MarketRegimeLabel,
    symbol_regime: MarketRegimeLabel,
    adverse_direction_loss_pct: f64,
    config: &AllocationConfig,
    state: &mut AllocationState,
) -> AllocationDecision {
    let (target_long, target_short, mut action, reason) = target_weights(btc_regime, symbol_regime);
    let cooldown_ms = config.cooldown_hours.saturating_mul(3_600_000);
    let in_cooldown = state
        .last_change_ms
        .map(|last| timestamp_ms.saturating_sub(last) < cooldown_ms)
        .unwrap_or(false);
    let extreme = is_extreme(btc_regime, symbol_regime) || adverse_direction_loss_pct >= config.forced_exit_loss_pct;
    let mut long_weight = target_long;
    let mut short_weight = target_short;
    if in_cooldown && !extreme {
        long_weight = state.long_weight_pct;
        short_weight = state.short_weight_pct;
        action = AllocationAction::None;
    } else if state.last_change_ms.is_none()
        || (state.long_weight_pct - target_long).abs() >= 20.0
        || (state.short_weight_pct - target_short).abs() >= 20.0
        || extreme
    {
        state.last_change_ms = Some(timestamp_ms);
        state.long_weight_pct = target_long;
        state.short_weight_pct = target_short;
    }
    if state.last_change_ms.is_none() {
        state.long_weight_pct = long_weight;
        state.short_weight_pct = short_weight;
        state.last_change_ms = Some(timestamp_ms);
    }
    let force_exit_short = extreme && short_weight == 0.0 && matches!(symbol_regime, MarketRegimeLabel::StrongUptrend | MarketRegimeLabel::ExtremeRisk);
    let force_exit_long = extreme && long_weight == 0.0 && matches!(symbol_regime, MarketRegimeLabel::StrongDowntrend | MarketRegimeLabel::ExtremeRisk);
    if force_exit_long || force_exit_short {
        action = AllocationAction::DirectionForcedExit;
    }
    let point = AllocationCurvePoint {
        timestamp_ms,
        symbol: symbol.to_owned(),
        long_weight_pct: long_weight,
        short_weight_pct: short_weight,
        action,
        reason,
        in_cooldown,
    };
    AllocationDecision { point, long_weight_pct: long_weight, short_weight_pct: short_weight, action, force_exit_long, force_exit_short, in_cooldown }
}

fn target_weights(btc: MarketRegimeLabel, symbol: MarketRegimeLabel) -> (f64, f64, AllocationAction, String) {
    use MarketRegimeLabel::*;
    match (btc, symbol) {
        (StrongUptrend, _) | (_, StrongUptrend) => (100.0, 0.0, AllocationAction::DirectionOrdersCancelled, "strong_uptrend_filter".to_owned()),
        (StrongDowntrend, _) | (_, StrongDowntrend) => (0.0, 100.0, AllocationAction::DirectionOrdersCancelled, "strong_downtrend_filter".to_owned()),
        (Uptrend, Uptrend) => (80.0, 20.0, AllocationAction::Rebalance, "confirmed_uptrend".to_owned()),
        (Downtrend, Downtrend) => (20.0, 80.0, AllocationAction::Rebalance, "confirmed_downtrend".to_owned()),
        (HighVolatility, _) | (_, HighVolatility) => (60.0, 40.0, AllocationAction::DirectionPaused, "high_volatility_reduce_risk".to_owned()),
        _ => (60.0, 40.0, AllocationAction::None, "range_default".to_owned()),
    }
}

fn is_extreme(btc: MarketRegimeLabel, symbol: MarketRegimeLabel) -> bool {
    matches!(btc, MarketRegimeLabel::StrongUptrend | MarketRegimeLabel::StrongDowntrend | MarketRegimeLabel::ExtremeRisk)
        && matches!(symbol, MarketRegimeLabel::StrongUptrend | MarketRegimeLabel::StrongDowntrend | MarketRegimeLabel::ExtremeRisk)
}
```

Add `pub mod allocation;` to `mod.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p backtest-engine allocation_ -- --nocapture`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-engine/src/martingale/allocation.rs apps/backtest-engine/src/martingale/mod.rs apps/backtest-engine/tests/search_scoring_time_splits.rs
git commit -m "feat: add dynamic long short allocation policy

问题描述: long+short回测缺少可解释的动态权重和冷却规则。
修复思路: 新增市场状态到多空权重的决策模块并覆盖冷却行为。"
```

## Task 4: Apply Allocation In K-Line Engine

**Files:**
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Modify: `apps/backtest-engine/src/martingale/metrics.rs`
- Test: `apps/backtest-engine/tests/search_scoring_time_splits.rs`

- [ ] **Step 1: Add failing forced-exit cost test**

Add test with a synthetic long+short futures portfolio where strong uptrend forces short exit:

```rust
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleMarginMode, MartingaleMarketKind,
    MartingalePortfolioConfig, MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel,
    MartingaleStrategyConfig, MartingaleTakeProfitModel,
};
use backtest_engine::market_data::KlineBar;
use backtest_engine::martingale::kline_engine::run_kline_screening;

#[test]
fn dynamic_allocation_forced_exit_records_costs_and_weight_curve() {
    let strategies = vec![MartingaleDirection::Long, MartingaleDirection::Short]
        .into_iter()
        .map(|direction| MartingaleStrategyConfig {
            strategy_id: format!("BTCUSDT-{direction:?}"),
            symbol: "BTCUSDT".to_owned(),
            market: MartingaleMarketKind::UsdMFutures,
            direction,
            direction_mode: MartingaleDirectionMode::LongAndShort,
            margin_mode: Some(MartingaleMarginMode::Isolated),
            leverage: Some(2),
            spacing: MartingaleSpacingModel::FixedPercent { step_bps: 100 },
            sizing: MartingaleSizingModel::Multiplier { first_order_quote: Decimal::new(10, 0), multiplier: Decimal::new(2, 0), max_legs: 3 },
            take_profit: MartingaleTakeProfitModel::Percent { bps: 200 },
            stop_loss: None,
            indicators: Vec::new(),
            entry_triggers: Vec::new(),
            risk_limits: MartingaleRiskLimits::default(),
        })
        .collect();
    let portfolio = MartingalePortfolioConfig {
        direction_mode: MartingaleDirectionMode::LongAndShort,
        strategies,
        risk_limits: MartingaleRiskLimits::default(),
    };
    let bars = (0..80)
        .map(|i| KlineBar {
            symbol: "BTCUSDT".to_owned(),
            open_time_ms: i * 14_400_000,
            open: 100.0 + i as f64,
            high: 103.0 + i as f64,
            low: 99.0 + i as f64,
            close: 102.0 + i as f64,
            volume: 1000.0,
        })
        .collect::<Vec<_>>();

    let result = run_kline_screening(portfolio, &bars).unwrap();
    assert!(!result.allocation_curve.is_empty());
    assert!(result.forced_exit_count > 0);
    assert!(result.cost_summary.fee_quote > 0.0);
    assert!(result.cost_summary.slippage_quote > 0.0);
    assert!(result.events.iter().any(|event| event.event_type == "direction_forced_exit"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p backtest-engine dynamic_allocation_forced_exit_records_costs_and_weight_curve -- --nocapture`  
Expected: FAIL because engine does not apply allocation decisions.

- [ ] **Step 3: Integrate allocation decisions**

In `kline_engine.rs`:

1. Maintain `BTreeMap<String, AllocationState>` keyed by symbol.
2. At each timestamp group, classify BTC and each symbol using recent 4H bars. Use existing grouped bars; if BTC is missing, treat BTC regime as `Range` and add a rejection warning only for missing BTC filter data when direction mode is `LongAndShort`.
3. Call `decide_allocation` per symbol.
4. Push `AllocationCurvePoint` and `RegimeTimelinePoint` into result vectors.
5. Before opening first leg or safety order, skip strategies whose direction currently has 0% target weight or whose action is `DirectionPaused` for that direction. Record `direction_paused` events.
6. On `DirectionForcedExit`, close active legs for the adverse direction at latest close, calculate `close_pnl - entry_cost - exit_cost`, decrement capital, increment trade count and forced exit count, add fee/slippage to `CostSummary`, record `direction_forced_exit`, and reset cycle.
7. Track `rebalance_count` when allocation point action is `Rebalance`, `DirectionOrdersCancelled`, or `DirectionForcedExit`.
8. Compute `average_allocation_hold_hours` from changes in allocation curve.

Use existing helpers: `close_pnl`, `entry_cost_quote`, `exit_cost_quote`, `active_capital_used_quote`, and `reset_cycle`.

- [ ] **Step 4: Run targeted tests**

Run: `cargo test -p backtest-engine dynamic_allocation_forced_exit_records_costs_and_weight_curve -- --nocapture`  
Expected: PASS.

- [ ] **Step 5: Run engine regression tests**

Run: `cargo test -p backtest-engine -- --nocapture`  
Expected: PASS. Existing non-dynamic tests should still pass because default allocation must not break long-only or short-only behavior.

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-engine/src/martingale/kline_engine.rs apps/backtest-engine/src/martingale/metrics.rs apps/backtest-engine/tests/search_scoring_time_splits.rs
git commit -m "feat: apply dynamic allocation in martingale backtests

问题描述: long+short回测没有模拟动态权重、暂停和极端趋势强平。
修复思路: 在K线循环中应用配比决策并把强平成本计入资金曲线。"
```

## Task 5: Search Space With Short Stop-Loss Defaults

**Files:**
- Modify: `apps/backtest-engine/src/search.rs`
- Modify: `apps/backtest-worker/src/main.rs`
- Test: `apps/backtest-engine/tests/search_scoring_time_splits.rs`
- Test: `apps/backtest-worker/src/main.rs` unit tests

- [ ] **Step 1: Add failing search tests**

Add tests proving short candidates contain both stop models and long+short keeps separate params:

```rust
use backtest_engine::search::{generate_candidates, SearchSpace};
use rust_decimal::Decimal;
use shared_domain::martingale::{MartingaleDirection, MartingaleDirectionMode, MartingaleMarketKind, MartingaleMarginMode, MartingaleStopLossModel};

#[test]
fn long_short_search_generates_short_candidates_with_drawdown_or_atr_stop() {
    let space = SearchSpace {
        symbols: vec!["BTCUSDT".to_owned()],
        direction_mode: MartingaleDirectionMode::LongAndShort,
        directions: vec![MartingaleDirection::Long, MartingaleDirection::Short],
        market: Some(MartingaleMarketKind::UsdMFutures),
        margin_mode: Some(MartingaleMarginMode::Isolated),
        step_bps: vec![100],
        first_order_quote: vec![Decimal::new(10, 0)],
        multiplier: vec![Decimal::new(2, 0)],
        take_profit_bps: vec![100],
        leverage: vec![2],
        max_legs: vec![4],
    };
    let candidates = generate_candidates(&space, 1, 7).unwrap();
    let short = candidates[0].config.strategies.iter().find(|strategy| strategy.direction == MartingaleDirection::Short).unwrap();
    assert!(matches!(short.stop_loss, Some(MartingaleStopLossModel::StrategyDrawdownPct { .. } | MartingaleStopLossModel::Atr { .. })));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p backtest-engine long_short_search_generates_short_candidates_with_drawdown_or_atr_stop -- --nocapture`  
Expected: FAIL because generated candidates currently use `stop_loss: None`.

- [ ] **Step 3: Implement short stop defaults**

In `search.rs`, when `direction == Short` and market is futures, set default short stop loss from dynamic search knobs:

```rust
let stop_loss = if direction == MartingaleDirection::Short {
    Some(MartingaleStopLossModel::StrategyDrawdownPct { pct_bps: 2_000 })
} else {
    None
};
```

If the current domain only supports one `stop_loss` field, use `StrategyDrawdownPct` in the base candidate and add ATR stop-search metadata in worker summary until a multi-stop enum is added. Do not fake execution of two stops; Task 6 adds candidate expansion to evaluate both variants and keep the better one.

- [ ] **Step 4: Extend worker search config**

In `apps/backtest-worker/src/main.rs`, add search knobs to task config parsing:

- `dynamic_allocation_enabled: bool`, default true for `long_and_short`.
- `short_stop_drawdown_pct_candidates: Vec<f64>`, default generated around max drawdown limit.
- `short_atr_stop_multiplier_candidates: Vec<f64>`, default `[1.5, 2.0, 2.5, 3.0]`.
- `allocation_cooldown_hours_candidates: Vec<u32>`, default from risk profile.

Add unit tests in `main.rs` near existing search-space tests to assert balanced 25% max drawdown produces drawdown stop candidates including `16`, `20`, `25`, `30` and does not include `12.5` as “half max drawdown”.

- [ ] **Step 5: Run tests**

Run: `cargo test -p backtest-engine long_short_search_generates_short_candidates_with_drawdown_or_atr_stop -- --nocapture`  
Expected: PASS.  
Run: `cargo test -p backtest-worker dynamic_ -- --nocapture`  
Expected: PASS for new dynamic config tests.

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-engine/src/search.rs apps/backtest-worker/src/main.rs apps/backtest-engine/tests/search_scoring_time_splits.rs
git commit -m "feat: search short-side martingale risk controls

问题描述: 自动搜索没有把做空侧止损作为候选变量，导致short风险失真。
修复思路: 为long+short搜索注入short止损默认和动态候选配置。"
```

## Task 6: Scoring Hard Drawdown Constraint And Churn Penalty

**Files:**
- Modify: `apps/backtest-engine/src/martingale/scoring.rs`
- Test: `apps/backtest-engine/tests/search_scoring_time_splits.rs`

- [ ] **Step 1: Add failing scoring tests**

Add tests:

```rust
use backtest_engine::martingale::metrics::{CostSummary, MartingaleBacktestResult, MartingaleMetrics};
use backtest_engine::martingale::scoring::{score_candidate, ScoringConfig};

fn scored_result(drawdown: f64, return_pct: f64, rebalance_count: u64, forced_exit_count: u64) -> MartingaleBacktestResult {
    MartingaleBacktestResult {
        metrics: MartingaleMetrics {
            total_return_pct: return_pct,
            max_drawdown_pct: drawdown,
            global_drawdown_pct: Some(drawdown),
            max_strategy_drawdown_pct: Some(drawdown),
            data_quality_score: Some(1.0),
            trade_count: 120,
            stop_count: 1,
            max_capital_used_quote: 1000.0,
            survival_passed: true,
        },
        events: Vec::new(),
        equity_curve: Vec::new(),
        rejection_reasons: Vec::new(),
        allocation_curve: Vec::new(),
        regime_timeline: Vec::new(),
        cost_summary: CostSummary::default(),
        rebalance_count,
        forced_exit_count,
        average_allocation_hold_hours: Some(16.0),
    }
}

#[test]
fn scoring_rejects_candidates_above_drawdown_limit_even_with_high_return() {
    let mut config = ScoringConfig::default();
    config.max_global_drawdown_pct = 20.0;
    config.max_strategy_drawdown_pct = 20.0;
    let score = score_candidate(&scored_result(21.0, 500.0, 1, 0), &config);
    assert!(!score.survival_valid);
    assert!(score.rejection_reasons.contains(&"global_drawdown_exceeded".to_owned()));
}

#[test]
fn scoring_penalizes_rebalance_and_forced_exit_churn() {
    let config = ScoringConfig::default();
    let stable = score_candidate(&scored_result(12.0, 40.0, 2, 0), &config);
    let churn = score_candidate(&scored_result(12.0, 40.0, 40, 8), &config);
    assert!(stable.raw_score > churn.raw_score);
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p backtest-engine scoring_rejects_candidates_above_drawdown_limit_even_with_high_return scoring_penalizes_rebalance_and_forced_exit_churn -- --nocapture`  
Expected: second test FAIL until churn penalties are implemented.

- [ ] **Step 3: Implement scoring penalties**

Extend `ScoringConfig`:

```rust
pub weight_rebalance_churn: f64,
pub weight_forced_exit: f64,
pub min_allocation_hold_hours: f64,
```

Set defaults:

```rust
weight_rebalance_churn: 0.35,
weight_forced_exit: 1.25,
min_allocation_hold_hours: 12.0,
```

In `score_candidate`, subtract:

```rust
let rebalance_penalty = result.rebalance_count as f64 * config.weight_rebalance_churn;
let forced_exit_penalty = result.forced_exit_count as f64 * config.weight_forced_exit * 10.0;
let hold_penalty = result.average_allocation_hold_hours
    .map(|hours| ((config.min_allocation_hold_hours - hours).max(0.0) / config.min_allocation_hold_hours) * 20.0)
    .unwrap_or(0.0);
```

Include these in `raw_score` as negative terms.

- [ ] **Step 4: Run tests**

Run: `cargo test -p backtest-engine scoring_ -- --nocapture`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-engine/src/martingale/scoring.rs apps/backtest-engine/tests/search_scoring_time_splits.rs
git commit -m "feat: penalize dynamic allocation churn in scoring

问题描述: 动态多空组合不能只看收益回撤比，还要惩罚频繁调仓和强平磨损。
修复思路: 在评分中加入调仓、强平和持有时长惩罚。"
```

## Task 7: Portfolio Optimizer

**Files:**
- Create: `apps/backtest-engine/src/martingale/portfolio_optimizer.rs`
- Modify: `apps/backtest-engine/src/martingale/mod.rs`
- Test: `apps/backtest-engine/tests/search_scoring_time_splits.rs`

- [ ] **Step 1: Add failing optimizer tests**

Add tests:

```rust
use backtest_engine::martingale::portfolio_optimizer::{optimize_portfolios, OptimizerCandidate, OptimizerConfig};

#[test]
fn portfolio_optimizer_can_drop_bad_symbols_and_caps_same_symbol() {
    let candidates = vec![
        OptimizerCandidate::new("btc-1", "BTCUSDT", 60.0, 12.0, vec![100.0, 110.0, 120.0]),
        OptimizerCandidate::new("btc-2", "BTCUSDT", 55.0, 13.0, vec![100.0, 108.0, 116.0]),
        OptimizerCandidate::new("btc-3", "BTCUSDT", 52.0, 14.0, vec![100.0, 107.0, 114.0]),
        OptimizerCandidate::new("eth-1", "ETHUSDT", 40.0, 10.0, vec![100.0, 106.0, 112.0]),
        OptimizerCandidate::new("doge-1", "DOGEUSDT", -5.0, 30.0, vec![100.0, 95.0, 90.0]),
    ];
    let config = OptimizerConfig::balanced(25.0);
    let portfolios = optimize_portfolios(&candidates, &config, 10).unwrap();
    let best = &portfolios[0];
    assert!(best.items.iter().all(|item| item.symbol != "DOGEUSDT"));
    assert!(best.weight_by_symbol("BTCUSDT") <= 35.0);
    assert_eq!(best.total_weight_pct(), 100.0);
    assert!(best.max_drawdown_pct <= 25.0);
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p backtest-engine portfolio_optimizer_can_drop_bad_symbols_and_caps_same_symbol -- --nocapture`  
Expected: FAIL because optimizer module does not exist.

- [ ] **Step 3: Implement optimizer**

Create `portfolio_optimizer.rs` with public structs:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct OptimizerCandidate {
    pub candidate_id: String,
    pub symbol: String,
    pub total_return_pct: f64,
    pub max_drawdown_pct: f64,
    pub equity_curve: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PortfolioItem {
    pub candidate_id: String,
    pub symbol: String,
    pub weight_pct: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PortfolioCandidate {
    pub items: Vec<PortfolioItem>,
    pub total_return_pct: f64,
    pub max_drawdown_pct: f64,
    pub return_drawdown_ratio: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct OptimizerConfig {
    pub max_drawdown_pct: f64,
    pub max_packages_per_symbol: usize,
    pub max_symbol_weight_pct: f64,
    pub max_package_weight_pct: f64,
}
```

Implement:

- `OptimizerConfig::conservative(max_drawdown_pct)` -> caps 25/10.
- `OptimizerConfig::balanced(max_drawdown_pct)` -> caps 35/15.
- `OptimizerConfig::aggressive(max_drawdown_pct)` -> caps 50/25.
- `OptimizerCandidate::new` test constructor.
- `optimize_portfolios(candidates, config, limit)`:
  - Drop candidates with non-finite metrics or negative return.
  - Generate 10% coarse combinations up to caps.
  - Refine best combinations with 5% weights.
  - Enforce total 100%, max packages per symbol 3, symbol cap, package cap.
  - Estimate portfolio return as weighted return.
  - Estimate drawdown using weighted equity curve peak-to-trough if curves align; fallback weighted drawdown.
  - Sort by drawdown <= limit, return/drawdown ratio, then return.

- [ ] **Step 4: Run optimizer test**

Run: `cargo test -p backtest-engine portfolio_optimizer_can_drop_bad_symbols_and_caps_same_symbol -- --nocapture`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-engine/src/martingale/portfolio_optimizer.rs apps/backtest-engine/src/martingale/mod.rs apps/backtest-engine/tests/search_scoring_time_splits.rs
git commit -m "feat: optimize martingale portfolio weights

问题描述: 多币种组合不能平均分配资金，需要根据回测结果选择和加权。
修复思路: 新增组合优化器，支持舍弃弱币种、同币种上限和10/5粒度搜索。"
```

## Task 8: Worker Output Contract For Dynamic Top10 And Portfolio Top10

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Test: `apps/backtest-worker/src/main.rs`
- Test: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] **Step 1: Add failing worker tests**

In worker Rust tests, add assertions that selected outputs include:

```rust
assert!(summary.get("allocation_curve").is_some());
assert!(summary.get("regime_timeline").is_some());
assert!(summary.get("cost_summary").is_some());
assert_eq!(summary["per_symbol_rank"], 1);
assert_eq!(summary["portfolio_top_n"], 10);
assert!(summary.get("dynamic_allocation_rules").is_some());
assert!(summary.get("max_drawdown_limit_pct").is_some());
```

In `tests/verification/backtest_worker_contract.test.mjs`, add contract checks for JSON fields:

```js
assert.match(source, /allocation_curve/);
assert.match(source, /regime_timeline/);
assert.match(source, /portfolio_top_n/);
assert.match(source, /dynamic_allocation_rules/);
assert.match(source, /max_drawdown_limit_pct/);
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p backtest-worker selected_outputs_include_ui_required_summary_fields -- --nocapture`  
Expected: FAIL due missing fields.  
Run: `node tests/verification/backtest_worker_contract.test.mjs`  
Expected: FAIL due missing contract fields.

- [ ] **Step 3: Implement worker summary fields**

In `apps/backtest-worker/src/main.rs`:

1. Default `per_symbol_top_n` to 10 for current dynamic mode.
2. Store per-symbol Top10, not Top5, when `dynamic_allocation_enabled` is true.
3. Add `portfolio_top_n: 10` to summary.
4. Copy result `allocation_curve`, `regime_timeline`, `cost_summary`, `rebalance_count`, `forced_exit_count`, `average_allocation_hold_hours` into candidate summary and artifact.
5. Add `dynamic_allocation_rules` object:

```json
{
  "timeframes": ["4h", "1d"],
  "btc_filter": true,
  "funding_rate_used": false,
  "weight_buckets": [[100,0],[80,20],[60,40],[50,50],[40,60],[20,80],[0,100]],
  "cooldown_hours": 16,
  "existing_position_policy": "tiered_pause_cancel_force_exit"
}
```

6. Add human-readable risk summary fields for stop-loss count, forced-exit count, and cost burden.
7. Call `portfolio_optimizer::optimize_portfolios` after per-symbol outputs are selected and persist portfolio candidates into task artifact summary.

- [ ] **Step 4: Run worker tests**

Run: `cargo test -p backtest-worker -- --nocapture`  
Expected: PASS.

- [ ] **Step 5: Run contract test**

Run: `node tests/verification/backtest_worker_contract.test.mjs`  
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-worker/src/main.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "feat: expose dynamic martingale top10 summaries

问题描述: 前端无法展示动态权重、市场状态和组合Top10。
修复思路: worker输出单币种Top10、组合Top10和动态配比摘要字段。"
```

## Task 9: Wizard Defaults And Payload

**Files:**
- Modify: `apps/web/components/backtest/backtest-wizard.tsx`
- Modify: `apps/web/components/backtest/search-config-editor.tsx`
- Test: `tests/verification/martingale_backtest_rebuild_contract.test.mjs`

- [ ] **Step 1: Add failing frontend contract test**

In `tests/verification/martingale_backtest_rebuild_contract.test.mjs`, assert source contains:

```js
assert.match(wizardSource, /conservative[^\n]+20/);
assert.match(wizardSource, /balanced[^\n]+25/);
assert.match(wizardSource, /aggressive[^\n]+30/);
assert.match(wizardSource, /manualDrawdownOverride/);
assert.match(wizardSource, /dynamic_allocation_enabled/);
assert.match(wizardSource, /per_symbol_top_n:\s*10/);
assert.match(wizardSource, /portfolio_top_n:\s*10/);
```

- [ ] **Step 2: Run test to verify failure**

Run: `node tests/verification/martingale_backtest_rebuild_contract.test.mjs`  
Expected: FAIL due missing new default/payload names.

- [ ] **Step 3: Implement defaults and payload**

In `backtest-wizard.tsx`:

1. Add mapping:

```ts
const DEFAULT_MAX_DRAWDOWN_BY_RISK = {
  conservative: 20,
  balanced: 25,
  aggressive: 30,
} as const;
```

2. Add `manualDrawdownOverride` state.
3. When risk profile changes and manual override is false, update `max_drawdown_pct` to mapped default.
4. When user edits max drawdown field, set manual override true.
5. Payload must include:

```ts
per_symbol_top_n: 10,
portfolio_top_n: 10,
dynamic_allocation_enabled: directionMode === "long_and_short",
scoring: {
  ...existingScoring,
  max_drawdown_pct: maxDrawdownPct,
},
```

6. UI copy must say max drawdown is a hard constraint.

- [ ] **Step 4: Run frontend contract test**

Run: `node tests/verification/martingale_backtest_rebuild_contract.test.mjs`  
Expected: PASS.

- [ ] **Step 5: Typecheck web**

Run: `pnpm --filter web exec tsc --noEmit`  
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/web/components/backtest/backtest-wizard.tsx apps/web/components/backtest/search-config-editor.tsx tests/verification/martingale_backtest_rebuild_contract.test.mjs
git commit -m "feat: default dynamic martingale drawdown limits

问题描述: 最大回撤限制需要按风险档位自动填充，并传递动态配比搜索参数。
修复思路: 前端增加20/25/30默认值、手动覆盖状态和Top10动态payload。"
```

## Task 10: Charts And Result Explanation

**Files:**
- Modify: `apps/web/components/backtest/backtest-charts.tsx`
- Modify: `apps/web/components/backtest/backtest-result-table.tsx`
- Modify: `apps/web/components/backtest/portfolio-candidate-review.tsx`
- Test: `tests/verification/martingale_backtest_rebuild_contract.test.mjs`

- [ ] **Step 1: Add failing chart contract checks**

Add checks:

```js
assert.match(chartsSource, /Long\/Short Allocation/);
assert.match(chartsSource, /long_weight_pct/);
assert.match(chartsSource, /short_weight_pct/);
assert.match(chartsSource, /btc_regime/);
assert.match(chartsSource, /symbol_regime/);
assert.match(chartsSource, /forced_exit_count/);
assert.match(chartsSource, /cost_summary/);
```

- [ ] **Step 2: Run test to verify failure**

Run: `node tests/verification/martingale_backtest_rebuild_contract.test.mjs`  
Expected: FAIL until chart/source strings exist.

- [ ] **Step 3: Implement allocation chart**

In `backtest-charts.tsx`:

1. Add `AllocationPoint` type with `timestamp_ms`, `symbol`, `long_weight_pct`, `short_weight_pct`, `action`, `reason`, `in_cooldown`.
2. Normalize `summary.allocation_curve` and candidate artifact allocation curve.
3. Render Recharts `AreaChart` or `LineChart` with Long and Short percentages.
4. Tooltip shows time, long %, short %, action, reason, cooldown.
5. Add regime summary cards using `summary.regime_timeline`.
6. Add cost summary cards for `fee_quote`, `slippage_quote`, `stop_loss_quote`, `forced_exit_quote`, `rebalance_count`, `forced_exit_count`, `average_allocation_hold_hours`.

- [ ] **Step 4: Update result and review UI**

In `backtest-result-table.tsx` and `portfolio-candidate-review.tsx`:

- Display `收益回撤比`.
- Display `调仓次数`.
- Display `强平次数`.
- Display `交易成本`.
- Display `是否满足最大回撤限制`.
- Display `是否可推荐实盘`.
- Explain discarded symbols from portfolio Top10 if present.

- [ ] **Step 5: Run tests**

Run: `node tests/verification/martingale_backtest_rebuild_contract.test.mjs`  
Expected: PASS.  
Run: `pnpm --filter web exec tsc --noEmit`  
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/web/components/backtest/backtest-charts.tsx apps/web/components/backtest/backtest-result-table.tsx apps/web/components/backtest/portfolio-candidate-review.tsx tests/verification/martingale_backtest_rebuild_contract.test.mjs
git commit -m "feat: visualize dynamic long short allocation

问题描述: 用户无法看到动态多空比例、调仓原因和交易成本。
修复思路: 新增多空权重曲线、状态说明和成本/强平摘要展示。"
```

## Task 11: Publish Dynamic Strategy Package

**Files:**
- Modify: `apps/api-server/src/services/martingale_publish_service.rs`
- Modify: `apps/api-server/src/routes/martingale_portfolios.rs`
- Modify: `apps/web/components/backtest/live-portfolio-controls.tsx`
- Test: `apps/api-server/tests/martingale_backtest_flow.rs`
- Test: `tests/verification/martingale_portfolio_contract.test.mjs`

- [ ] **Step 1: Add failing API test**

In `apps/api-server/tests/martingale_backtest_flow.rs`, add a test that posts a portfolio publish payload containing:

```json
{
  "dynamic_allocation_rules": {
    "btc_filter": true,
    "funding_rate_used": false,
    "timeframes": ["4h", "1d"],
    "existing_position_policy": "tiered_pause_cancel_force_exit"
  }
}
```

Assert response includes the same `dynamic_allocation_rules` and either `live_ready: true` or a human-readable `live_readiness_blockers` array.

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p api-server --test martingale_backtest_flow martingale_dynamic_publish -- --nocapture`  
Expected: FAIL because API does not preserve/validate dynamic rules.

- [ ] **Step 3: Implement publish payload preservation**

In publish service:

1. Accept `dynamic_allocation_rules` JSON object in portfolio publish request.
2. Store it in portfolio metadata or strategy parameter snapshot without losing fields.
3. Add live readiness checks:
   - dynamic allocation rules present.
   - direction mode is `long_and_short` for dynamic long/short package.
   - futures hedge/margin prerequisites are represented.
   - forced exit capability is explicitly marked supported or blocked.
4. Return `live_readiness_blockers` if trading engine cannot yet execute any required action.

- [ ] **Step 4: Update frontend publish UI**

In `live-portfolio-controls.tsx`:

- Include `dynamic_allocation_rules` in publish request.
- Show `live_readiness_blockers` before final confirmation.
- Disable “直接发布实盘” if blockers exist; allow “保存为待启用组合”。

- [ ] **Step 5: Run API and contract tests**

Run: `cargo test -p api-server --test martingale_backtest_flow -- --nocapture`  
Expected: PASS.  
Run: `node tests/verification/martingale_portfolio_contract.test.mjs`  
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/api-server/src/services/martingale_publish_service.rs apps/api-server/src/routes/martingale_portfolios.rs apps/api-server/tests/martingale_backtest_flow.rs apps/web/components/backtest/live-portfolio-controls.tsx tests/verification/martingale_portfolio_contract.test.mjs
git commit -m "feat: publish dynamic martingale allocation packages

问题描述: 动态回测结果发布到实盘时不能丢失配比和风控规则。
修复思路: 发布API保留动态规则并返回实盘能力校验结果。"
```

## Task 12: End-To-End Verification And Service Restart

**Files:**
- No source changes unless earlier verification exposes defects.
- Use commands only.

- [ ] **Step 1: Run Rust test suite for changed crates**

Run:

```bash
cargo test -p backtest-engine -- --nocapture
cargo test -p backtest-worker -- --nocapture
cargo test -p api-server --test martingale_backtest_flow -- --nocapture
```

Expected: all PASS.

- [ ] **Step 2: Run frontend and contract tests**

Run:

```bash
pnpm --filter web exec tsc --noEmit
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/martingale_backtest_rebuild_contract.test.mjs
node tests/verification/martingale_portfolio_contract.test.mjs
```

Expected: all PASS.

- [ ] **Step 3: Rebuild and restart only grid services**

Run:

```bash
docker compose --env-file .env -f deploy/docker/docker-compose.yml up -d --build web api-server backtest-worker
```

Expected: `web`, `api-server`, and `backtest-worker` are recreated or healthy. Do not touch unrelated host port 3000 service. Grid frontend remains available through nginx on host port 8080.

- [ ] **Step 4: Smoke check frontend route**

Run:

```bash
curl -I http://127.0.0.1:8080/zh/app/backtest
```

Expected: HTTP response is reachable, normally redirecting to login when not authenticated.

- [ ] **Step 5: Run one quality-oriented backtest**

Create a `long_and_short` futures task through the existing API or UI using:

- Symbols: `BTCUSDT,ETHUSDT`
- Risk profile: `balanced`
- Max drawdown: `25%`
- Per-symbol Top10: `10`
- Portfolio Top10: `10`
- Dynamic allocation enabled: `true`

Expected result:

- BTC and ETH each show Top10 or a clear data-quality exclusion reason.
- Portfolio Top10 appears.
- Allocation curve exists.
- Regime timeline exists.
- Cost summary is non-empty.
- Max drawdown limit status is visible.
- No result is marked live-recommended if it violates the hard drawdown limit.

- [ ] **Step 6: Final commit if verification-only fixes were needed**

Run: `git status --short`  
If verification required additional source fixes, stage only the files changed for those fixes. Use explicit paths from `git status --short`, then commit with this message:

```bash
git add apps/backtest-engine/src/martingale/kline_engine.rs apps/backtest-worker/src/main.rs apps/web/components/backtest/backtest-charts.tsx tests/verification/martingale_backtest_rebuild_contract.test.mjs
git commit -m "fix: stabilize dynamic martingale verification

复现路径: 完整回测与前端契约验证暴露动态配比字段或展示不一致。
修复思路: 对齐结果字段、契约测试和页面展示。"
```

If those exact files were not changed, replace the `git add` paths with the precise files shown by `git status --short`; do not stage unrelated local files.

- [ ] **Step 7: Final status summary**

Run: `git status --short`  
Expected: clean except intentionally uncommitted local environment files. Summarize any remaining uncommitted files explicitly.
