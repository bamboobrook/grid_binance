use backtest_engine::martingale::metrics::{
    AllocationAction, AllocationCurvePoint, CostSummary, MarketRegimeLabel,
    MartingaleBacktestResult, MartingaleMetrics, RegimeTimelinePoint,
};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};

use backtest_engine::intelligent_search::{intelligent_search, IntelligentSearchConfig};
use backtest_engine::market_data::{AggTrade, KlineBar};
use backtest_engine::martingale::allocation::{
    decide_allocation, AllocationConfig, AllocationState,
};
use backtest_engine::martingale::kline_engine::run_kline_screening;
use backtest_engine::martingale::regime::{classify_regime, RegimeConfig};
use backtest_engine::martingale::scoring::{score_candidate, ScoringConfig};
use backtest_engine::martingale::trade_engine::{
    run_trade_refinement, trades_to_ordered_price_bars,
};
use backtest_engine::search::{random_search, SearchSpace};
use backtest_engine::time_splits::{named_stress_windows, walk_forward_windows, WalkForwardConfig};
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleEntryTrigger, MartingaleMarginMode,
    MartingaleMarketKind, MartingalePortfolioConfig, MartingaleRiskLimits, MartingaleSizingModel,
    MartingaleSpacingModel, MartingaleStopLossModel, MartingaleStrategyConfig,
    MartingaleTakeProfitModel,
};

fn kline_bar(open_time_ms: i64, open: f64, high: f64, low: f64, close: f64) -> KlineBar {
    KlineBar {
        symbol: "BTCUSDT".to_string(),
        open_time_ms,
        open,
        high,
        low,
        close,
        volume: 1.0,
    }
}

fn synthetic_trend_bars() -> Vec<KlineBar> {
    (0..80)
        .map(|index| {
            let open = 100.0 + index as f64 * 1.0;
            let close = open + 0.8;
            kline_bar(index * 60_000, open, close + 0.3, open - 0.2, close)
        })
        .collect()
}

fn synthetic_range_bars() -> Vec<KlineBar> {
    (0..80)
        .map(|index| {
            let center = 100.0 + (index % 6) as f64 * 0.04;
            let close = center + if index % 2 == 0 { 0.03 } else { -0.03 };
            kline_bar(index * 60_000, center, center + 0.15, center - 0.15, close)
        })
        .collect()
}

fn symbol_kline_bar(
    symbol: &str,
    open_time_ms: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
) -> KlineBar {
    KlineBar {
        symbol: symbol.to_string(),
        open_time_ms,
        open,
        high,
        low,
        close,
        volume: 1.0,
    }
}

fn dynamic_allocation_long_short_portfolio() -> MartingalePortfolioConfig {
    let base = MartingaleStrategyConfig {
        strategy_id: "BTCUSDT-long".to_string(),
        symbol: "BTCUSDT".to_string(),
        market: MartingaleMarketKind::UsdMFutures,
        direction: MartingaleDirection::Long,
        direction_mode: MartingaleDirectionMode::LongAndShort,
        margin_mode: Some(MartingaleMarginMode::Cross),
        leverage: Some(3),
        spacing: MartingaleSpacingModel::FixedPercent { step_bps: 50 },
        sizing: MartingaleSizingModel::CustomSequence {
            notionals: vec![Decimal::new(100, 0), Decimal::new(200, 0)],
        },
        take_profit: MartingaleTakeProfitModel::Percent { bps: 9_000 },
        stop_loss: None,
        indicators: Vec::new(),
        entry_triggers: Vec::new(),
        risk_limits: MartingaleRiskLimits::default(),
    };
    let mut short = base.clone();
    short.strategy_id = "BTCUSDT-short".to_string();
    short.direction = MartingaleDirection::Short;

    MartingalePortfolioConfig {
        direction_mode: MartingaleDirectionMode::LongAndShort,
        strategies: vec![base, short],
        risk_limits: MartingaleRiskLimits {
            max_global_budget_quote: Some(Decimal::new(10_000, 0)),
            ..MartingaleRiskLimits::default()
        },
    }
}

fn dynamic_allocation_rising_bars() -> Vec<KlineBar> {
    (0..80)
        .map(|index| {
            let open = 100.0 + index as f64;
            let close = open + 0.8;
            symbol_kline_bar(
                "BTCUSDT",
                index * 60_000,
                open,
                close + 0.3,
                open - 0.2,
                close,
            )
        })
        .collect()
}

fn dynamic_allocation_two_symbol_stale_btc_bars() -> Vec<KlineBar> {
    let mut bars = Vec::new();
    for index in 0..80 {
        let open = 100.0 + index as f64;
        let close = open + 0.8;
        bars.push(symbol_kline_bar(
            "BTCUSDT",
            index * 60_000,
            open,
            close + 0.3,
            open - 0.2,
            close,
        ));
    }
    for index in 80..90 {
        let center = 200.0 + (index % 4) as f64 * 0.02;
        bars.push(symbol_kline_bar(
            "ETHUSDT",
            index * 60_000,
            center,
            center + 0.1,
            center - 0.1,
            center,
        ));
    }
    bars
}

fn dynamic_allocation_two_symbol_portfolio(symbol: &str) -> MartingalePortfolioConfig {
    let mut portfolio = dynamic_allocation_long_short_portfolio();
    for strategy in &mut portfolio.strategies {
        strategy.symbol = symbol.to_string();
        strategy.strategy_id = format!("{symbol}-{:?}", strategy.direction).to_ascii_lowercase();
    }
    portfolio
}

fn dynamic_allocation_warmup_bars(count: i64) -> Vec<KlineBar> {
    (0..count)
        .map(|index| symbol_kline_bar("BTCUSDT", index * 60_000, 100.0, 100.2, 99.8, 100.0))
        .collect()
}

fn dynamic_allocation_two_symbol_parallel_bars(count: i64) -> Vec<KlineBar> {
    let mut bars = Vec::new();
    for index in 0..count {
        let open = 100.0 + index as f64;
        let close = open + 0.8;
        for symbol in ["BTCUSDT", "ETHUSDT"] {
            bars.push(symbol_kline_bar(
                symbol,
                index * 60_000,
                open,
                close + 0.3,
                open - 0.2,
                close,
            ));
        }
    }
    bars
}

fn dynamic_allocation_multi_symbol_portfolio() -> MartingalePortfolioConfig {
    let mut portfolio = dynamic_allocation_long_short_portfolio();
    let mut eth_strategies = portfolio.strategies.clone();
    for strategy in &mut eth_strategies {
        strategy.symbol = "ETHUSDT".to_string();
        strategy.strategy_id = format!("ETHUSDT-{:?}", strategy.direction).to_ascii_lowercase();
    }
    portfolio.strategies.extend(eth_strategies);
    portfolio
}

fn dynamic_allocation_pause_recover_pause_bars() -> Vec<KlineBar> {
    let mut bars = dynamic_allocation_rising_bars();
    bars.extend(
        (80..160)
            .map(|index| symbol_kline_bar("BTCUSDT", index * 60_000, 100.0, 100.2, 99.8, 100.0)),
    );
    bars.extend((160..240).map(|index| {
        let open = 100.0 + (index - 160) as f64;
        let close = open + 0.8;
        symbol_kline_bar(
            "BTCUSDT",
            index * 60_000,
            open,
            close + 0.3,
            open - 0.2,
            close,
        )
    }));
    bars
}

fn event_detail_number(detail: &str, key: &str) -> f64 {
    detail
        .split(';')
        .find_map(|part| part.strip_prefix(&format!("{key}=")))
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or_else(|| panic!("missing {key} in {detail}"))
}

#[test]
fn random_search_is_reproducible() {
    let space = SearchSpace {
        symbols: vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
        direction_mode: MartingaleDirectionMode::IndicatorSelected,
        directions: vec![MartingaleDirection::Long, MartingaleDirection::Short],
        market: None,
        margin_mode: None,
        step_bps: vec![50, 100, 150],
        first_order_quote: vec![Decimal::new(25, 0), Decimal::new(50, 0)],
        multiplier: vec![Decimal::new(15, 1)],
        take_profit_bps: vec![60, 90],
        leverage: vec![1, 3],
        max_legs: vec![3, 4],
        dynamic_allocation_enabled: false,
        short_stop_drawdown_pct_candidates: Vec::new(),
        short_atr_stop_multiplier_candidates: Vec::new(),
        allocation_cooldown_hours_candidates: Vec::new(),
    };

    let first = random_search(&space, 8, 42).expect("first search");
    let second = random_search(&space, 8, 42).expect("second search");

    assert_eq!(first, second);
    assert_eq!(first.len(), 8);
    assert!(first
        .iter()
        .all(|candidate| candidate.config.validate().is_ok()));
}

#[test]
fn long_and_short_search_builds_one_candidate_with_two_directional_legs() {
    let space = SearchSpace {
        symbols: vec!["BTCUSDT".to_string()],
        direction_mode: MartingaleDirectionMode::LongAndShort,
        directions: vec![MartingaleDirection::Long, MartingaleDirection::Short],
        market: Some(MartingaleMarketKind::UsdMFutures),
        margin_mode: None,
        step_bps: vec![80, 120],
        first_order_quote: vec![Decimal::new(10, 0)],
        multiplier: vec![Decimal::new(2, 0)],
        take_profit_bps: vec![80, 120],
        leverage: vec![3],
        max_legs: vec![4, 5],
        dynamic_allocation_enabled: true,
        short_stop_drawdown_pct_candidates: Vec::new(),
        short_atr_stop_multiplier_candidates: Vec::new(),
        allocation_cooldown_hours_candidates: Vec::new(),
    };

    let candidates = random_search(&space, 1, 7).expect("search");
    let strategies = &candidates[0].config.strategies;

    assert_eq!(
        candidates[0].config.direction_mode,
        MartingaleDirectionMode::LongAndShort
    );
    assert_eq!(strategies.len(), 2);
    assert_eq!(strategies[0].direction, MartingaleDirection::Long);
    assert_eq!(strategies[1].direction, MartingaleDirection::Short);
    assert_ne!(strategies[0].strategy_id, strategies[1].strategy_id);
}

#[test]
fn long_short_search_generates_short_candidates_with_drawdown_or_atr_stop() {
    let space = SearchSpace {
        symbols: vec!["BTCUSDT".to_string()],
        direction_mode: MartingaleDirectionMode::LongAndShort,
        directions: vec![MartingaleDirection::Long, MartingaleDirection::Short],
        market: Some(MartingaleMarketKind::UsdMFutures),
        margin_mode: None,
        step_bps: vec![100],
        first_order_quote: vec![Decimal::new(10, 0)],
        multiplier: vec![Decimal::new(2, 0)],
        take_profit_bps: vec![100],
        leverage: vec![3],
        max_legs: vec![4],
        dynamic_allocation_enabled: true,
        short_stop_drawdown_pct_candidates: Vec::new(),
        short_atr_stop_multiplier_candidates: Vec::new(),
        allocation_cooldown_hours_candidates: Vec::new(),
    };

    let candidates = random_search(&space, 3, 11).expect("search");
    let short_strategies: Vec<_> = candidates
        .iter()
        .flat_map(|candidate| candidate.config.strategies.iter())
        .filter(|strategy| strategy.direction == MartingaleDirection::Short)
        .collect();

    assert!(!short_strategies.is_empty());
    assert!(short_strategies.iter().all(|strategy| matches!(
        strategy.stop_loss,
        Some(MartingaleStopLossModel::StrategyDrawdownPct { .. })
            | Some(MartingaleStopLossModel::Atr { .. })
    )));
}

#[test]
fn futures_short_search_generates_atr_stop_candidate_when_configured() {
    let mut space = short_stop_search_space(
        MartingaleDirectionMode::ShortOnly,
        vec![MartingaleDirection::Short],
        Some(MartingaleMarketKind::UsdMFutures),
    );
    space.short_atr_stop_multiplier_candidates = vec![2.0];

    let candidates = random_search(&space, 8, 15).expect("search");

    assert!(candidates.iter().any(|candidate| matches!(
        candidate.config.strategies[0].stop_loss,
        Some(MartingaleStopLossModel::Atr { multiplier }) if multiplier == Decimal::new(2, 0)
    )));
}

#[test]
fn allocation_cooldown_candidates_are_metadata_only_for_search_generation() {
    let mut without_metadata = short_stop_search_space(
        MartingaleDirectionMode::ShortOnly,
        vec![MartingaleDirection::Short],
        Some(MartingaleMarketKind::UsdMFutures),
    );
    let mut with_metadata = without_metadata.clone();
    with_metadata.allocation_cooldown_hours_candidates = vec![6, 12, 24];

    let left = random_search(&without_metadata, 4, 16).expect("search without metadata");
    let right = random_search(&with_metadata, 4, 16).expect("search with metadata");

    without_metadata.allocation_cooldown_hours_candidates = vec![6, 12, 24];
    assert_eq!(left, right);
}

#[test]
fn short_only_futures_search_generates_short_candidates_with_drawdown_stop() {
    let space = short_stop_search_space(
        MartingaleDirectionMode::ShortOnly,
        vec![MartingaleDirection::Short],
        Some(MartingaleMarketKind::UsdMFutures),
    );

    let candidates = random_search(&space, 2, 12).expect("search");

    assert!(candidates.iter().all(|candidate| {
        let strategy = &candidate.config.strategies[0];
        strategy.direction == MartingaleDirection::Short
            && matches!(
                strategy.stop_loss,
                Some(MartingaleStopLossModel::StrategyDrawdownPct { pct_bps: 2_000 })
            )
    }));
}

#[test]
fn long_only_futures_search_does_not_inject_short_stop() {
    let space = short_stop_search_space(
        MartingaleDirectionMode::LongOnly,
        vec![MartingaleDirection::Long],
        Some(MartingaleMarketKind::UsdMFutures),
    );

    let candidates = random_search(&space, 2, 13).expect("search");

    assert!(candidates.iter().all(|candidate| {
        let strategy = &candidate.config.strategies[0];
        strategy.direction == MartingaleDirection::Long && strategy.stop_loss.is_none()
    }));
}

#[test]
fn spot_short_search_does_not_inject_futures_short_stop() {
    let space = short_stop_search_space(
        MartingaleDirectionMode::ShortOnly,
        vec![MartingaleDirection::Short],
        Some(MartingaleMarketKind::Spot),
    );

    let candidates = random_search(&space, 2, 14).expect("search");

    assert!(candidates.iter().all(|candidate| {
        let strategy = &candidate.config.strategies[0];
        strategy.direction == MartingaleDirection::Short && strategy.stop_loss.is_none()
    }));
}

#[test]
fn allocation_closes_short_weight_when_btc_and_symbol_are_strong_up() {
    let config = AllocationConfig::balanced();
    let state = AllocationState::default();

    let decision = decide_allocation(
        0,
        "ETHUSDT",
        MarketRegimeLabel::StrongUptrend,
        MarketRegimeLabel::StrongUptrend,
        0.0,
        &config,
        &state,
    );

    assert_eq!(decision.long_weight_pct, 100.0);
    assert_eq!(decision.short_weight_pct, 0.0);
    assert_eq!(decision.action, AllocationAction::DirectionForcedExit);
    assert!(decision.force_exit_short);
    assert!(!decision.force_exit_long);
    assert!(!decision.in_cooldown);
    assert_eq!(decision.point.symbol, "ETHUSDT");
    assert!(decision.point.reason.contains("both"));
}

#[test]
fn allocation_closes_short_weight_when_btc_is_strong_up_and_symbol_ranges() {
    let config = AllocationConfig::balanced();
    let state = AllocationState::default();

    let decision = decide_allocation(
        0,
        "ETHUSDT",
        MarketRegimeLabel::StrongUptrend,
        MarketRegimeLabel::Range,
        0.0,
        &config,
        &state,
    );

    assert_eq!(decision.long_weight_pct, 100.0);
    assert_eq!(decision.short_weight_pct, 0.0);
    assert_eq!(decision.action, AllocationAction::DirectionForcedExit);
    assert!(decision.force_exit_short);
    assert!(!decision.force_exit_long);
    assert!(decision.point.reason.contains("btc"));
    assert!(!decision.point.reason.contains("both"));
}

#[test]
fn allocation_closes_long_weight_when_btc_is_strong_down_and_symbol_ranges() {
    let config = AllocationConfig::balanced();
    let state = AllocationState::default();

    let decision = decide_allocation(
        0,
        "ETHUSDT",
        MarketRegimeLabel::StrongDowntrend,
        MarketRegimeLabel::Range,
        0.0,
        &config,
        &state,
    );

    assert_eq!(decision.long_weight_pct, 0.0);
    assert_eq!(decision.short_weight_pct, 100.0);
    assert_eq!(decision.action, AllocationAction::DirectionForcedExit);
    assert!(decision.force_exit_long);
    assert!(!decision.force_exit_short);
    assert!(decision.point.reason.contains("btc"));
    assert!(!decision.point.reason.contains("both"));
}

#[test]
fn allocation_symbol_strong_up_reason_names_symbol_source() {
    let config = AllocationConfig::balanced();
    let state = AllocationState::default();

    let decision = decide_allocation(
        0,
        "ETHUSDT",
        MarketRegimeLabel::Range,
        MarketRegimeLabel::StrongUptrend,
        0.0,
        &config,
        &state,
    );

    assert_eq!(decision.short_weight_pct, 0.0);
    assert!(decision.force_exit_short);
    assert!(decision.point.reason.contains("symbol"));
    assert!(!decision.point.reason.contains("both"));
}

#[test]
fn allocation_first_neutral_default_state_does_not_rebalance() {
    let config = AllocationConfig::balanced();
    let state = AllocationState::default();

    let decision = decide_allocation(
        0,
        "ETHUSDT",
        MarketRegimeLabel::Range,
        MarketRegimeLabel::Range,
        0.0,
        &config,
        &state,
    );

    assert_eq!(decision.long_weight_pct, 60.0);
    assert_eq!(decision.short_weight_pct, 40.0);
    assert_eq!(decision.action, AllocationAction::None);
    assert!(!decision.force_exit_long);
    assert!(!decision.force_exit_short);
}

#[test]
fn allocation_loss_threshold_forced_exit_has_direction_flag() {
    let config = AllocationConfig::balanced();
    let state = AllocationState {
        last_change_ms: Some(0),
        long_weight_pct: 100.0,
        short_weight_pct: 0.0,
    };

    let decision = decide_allocation(
        4 * 60 * 60 * 1000,
        "ETHUSDT",
        MarketRegimeLabel::Range,
        MarketRegimeLabel::Range,
        config.forced_exit_loss_pct,
        &config,
        &state,
    );

    assert_eq!(decision.action, AllocationAction::DirectionForcedExit);
    assert!(decision.force_exit_short || decision.force_exit_long);
    assert!(decision.force_exit_short);
    assert_eq!(decision.short_weight_pct, 0.0);
    assert_eq!(decision.point.short_weight_pct, 0.0);
    assert!(decision.point.reason.contains("forced exit loss"));
}

#[test]
fn allocation_loss_threshold_ambiguous_does_not_force_exit() {
    let config = AllocationConfig::balanced();
    let state = AllocationState {
        last_change_ms: Some(0),
        long_weight_pct: 60.0,
        short_weight_pct: 40.0,
    };

    let decision = decide_allocation(
        4 * 60 * 60 * 1000,
        "ETHUSDT",
        MarketRegimeLabel::Range,
        MarketRegimeLabel::Range,
        config.forced_exit_loss_pct,
        &config,
        &state,
    );

    assert_ne!(decision.action, AllocationAction::DirectionForcedExit);
    assert!(!decision.force_exit_long);
    assert!(!decision.force_exit_short);
    assert!(decision.point.reason.contains("loss_threshold_ambiguous"));
}

#[test]
fn allocation_ignores_nan_or_negative_loss_for_forced_exit() {
    let config = AllocationConfig::balanced();
    let state = AllocationState::default();

    for adverse_loss in [f64::NAN, -1.0] {
        let decision = decide_allocation(
            0,
            "ETHUSDT",
            MarketRegimeLabel::Range,
            MarketRegimeLabel::Range,
            adverse_loss,
            &config,
            &state,
        );

        assert_ne!(decision.action, AllocationAction::DirectionForcedExit);
        assert!(!decision.force_exit_long);
        assert!(!decision.force_exit_short);
    }
}

#[test]
fn allocation_cooldown_blocks_small_weight_flip() {
    let config = AllocationConfig::balanced();
    let state = AllocationState {
        last_change_ms: Some(0),
        long_weight_pct: 60.0,
        short_weight_pct: 40.0,
    };

    let decision = decide_allocation(
        4 * 60 * 60 * 1000,
        "ETHUSDT",
        MarketRegimeLabel::Uptrend,
        MarketRegimeLabel::Uptrend,
        0.0,
        &config,
        &state,
    );

    assert_eq!(decision.long_weight_pct, 60.0);
    assert_eq!(decision.short_weight_pct, 40.0);
    assert_eq!(decision.action, AllocationAction::None);
    assert!(decision.in_cooldown);
    assert!(!decision.force_exit_long);
    assert!(!decision.force_exit_short);
    assert!(decision.point.in_cooldown);
    assert!(!decision.point.reason.is_empty());
}

#[test]
fn dynamic_allocation_forced_exit_records_costs_and_weight_curve() {
    let result = run_kline_screening(
        dynamic_allocation_long_short_portfolio(),
        &dynamic_allocation_rising_bars(),
    )
    .expect("dynamic allocation kline backtest");

    assert!(!result.allocation_curve.is_empty());
    assert!(result.forced_exit_count > 0);
    assert!(result.cost_summary.fee_quote > 0.0);
    assert!(result.cost_summary.slippage_quote > 0.0);
    assert!(result
        .events
        .iter()
        .any(|event| event.event_type == "direction_forced_exit"));
}

#[test]
fn dynamic_allocation_ignores_stale_btc_regime_when_current_group_lacks_btc() {
    let result = run_kline_screening(
        dynamic_allocation_two_symbol_portfolio("ETHUSDT"),
        &dynamic_allocation_two_symbol_stale_btc_bars(),
    )
    .expect("dynamic allocation with stale BTC bars");

    assert_eq!(result.forced_exit_count, 0);
    assert!(result
        .regime_timeline
        .iter()
        .filter(|point| point.symbol == "ETHUSDT")
        .all(|point| point.btc_regime == point.symbol_regime));
}

#[test]
fn dynamic_allocation_forced_exit_cost_summary_splits_entry_and_exit_costs() {
    let result = run_kline_screening(
        dynamic_allocation_long_short_portfolio(),
        &dynamic_allocation_rising_bars(),
    )
    .expect("dynamic allocation kline backtest");

    assert!(result.cost_summary.fee_quote > 0.0);
    assert!(result.cost_summary.slippage_quote > 0.0);
    assert!(
        (result.cost_summary.fee_quote - result.cost_summary.slippage_quote * 2.0).abs() < 1.0e-9,
        "fee/slippage should respect configured bps split: {:?}",
        result.cost_summary
    );
    assert!(result.cost_summary.slippage_quote > 0.12);
}

#[test]
fn dynamic_allocation_warmup_groups_emit_range_points() {
    let bars = dynamic_allocation_warmup_bars(5);
    let result = run_kline_screening(dynamic_allocation_long_short_portfolio(), &bars)
        .expect("warmup dynamic allocation");

    assert_eq!(result.allocation_curve.len(), bars.len());
    assert_eq!(result.regime_timeline.len(), bars.len());
    assert!(result
        .regime_timeline
        .iter()
        .all(|point| point.btc_regime == MarketRegimeLabel::Range
            && point.symbol_regime == MarketRegimeLabel::Range));
}

#[test]
fn dynamic_allocation_weights_scale_entry_notionals() {
    let result = run_kline_screening(
        dynamic_allocation_long_short_portfolio(),
        &dynamic_allocation_rising_bars(),
    )
    .expect("weighted dynamic allocation");

    let long_entry = result
        .events
        .iter()
        .find(|event| event.event_type == "entry" && event.strategy_instance_id.contains("long"))
        .expect("long entry");
    let short_entry = result
        .events
        .iter()
        .find(|event| event.event_type == "entry" && event.strategy_instance_id.contains("short"))
        .expect("short entry");
    let long_notional = event_detail_number(&long_entry.detail, "notional_quote");
    let short_notional = event_detail_number(&short_entry.detail, "notional_quote");

    assert_eq!(long_notional, 180.0);
    assert_eq!(short_notional, 120.0);
    assert_ne!(long_notional, short_notional);
}

#[test]
fn dynamic_allocation_deduplicates_rebalance_holds_and_pause_events() {
    let mut portfolio = dynamic_allocation_long_short_portfolio();
    for strategy in &mut portfolio.strategies {
        strategy.entry_triggers = vec![MartingaleEntryTrigger::TimeWindow {
            start: "00:00".to_string(),
            end: "00:00".to_string(),
        }];
    }
    let result = run_kline_screening(portfolio, &dynamic_allocation_rising_bars())
        .expect("deduplicated dynamic allocation");

    let paused_count = result
        .events
        .iter()
        .filter(|event| event.event_type == "direction_paused")
        .count();

    assert!(
        result.rebalance_count < 5,
        "rebalance_count={}",
        result.rebalance_count
    );
    assert!(paused_count <= 2, "paused_count={paused_count}");
    assert!(result.average_allocation_hold_hours.unwrap_or_default() > 0.0);
}

#[test]
fn dynamic_allocation_hold_hours_are_tracked_per_symbol() {
    let result = run_kline_screening(
        dynamic_allocation_multi_symbol_portfolio(),
        &dynamic_allocation_two_symbol_parallel_bars(80),
    )
    .expect("multi-symbol dynamic allocation");

    let average_hold_hours = result.average_allocation_hold_hours.unwrap_or_default();
    assert!(
        average_hold_hours > 0.0,
        "average_hold_hours={average_hold_hours}"
    );
    assert!(
        average_hold_hours >= 0.5,
        "average_hold_hours={average_hold_hours}"
    );
}

#[test]
fn dynamic_allocation_pause_event_records_new_episode_after_recovery() {
    let mut portfolio = dynamic_allocation_long_short_portfolio();
    portfolio
        .strategies
        .retain(|strategy| strategy.direction == MartingaleDirection::Short);
    for strategy in &mut portfolio.strategies {
        strategy.entry_triggers = vec![MartingaleEntryTrigger::TimeWindow {
            start: "00:00".to_string(),
            end: "00:00".to_string(),
        }];
    }

    let result = run_kline_screening(portfolio, &dynamic_allocation_pause_recover_pause_bars())
        .expect("pause recover pause dynamic allocation");
    let paused_count = result
        .events
        .iter()
        .filter(|event| event.event_type == "direction_paused")
        .count();

    assert_eq!(paused_count, 2);
}

#[test]
fn regime_classifier_detects_strong_uptrend_and_range() {
    let config = RegimeConfig::default();

    let uptrend = classify_regime(&synthetic_trend_bars(), &config).expect("uptrend regime");
    assert_eq!(uptrend.label, MarketRegimeLabel::StrongUptrend);
    assert!(uptrend.ema_spread_bps > 0.0);
    assert!(uptrend.adx >= config.strong_adx);

    let range = classify_regime(&synthetic_range_bars(), &config).expect("range regime");
    assert_eq!(range.label, MarketRegimeLabel::Range);
    assert!(range.ema_spread_bps.abs() < config.slope_bps);
}

#[test]
fn regime_classifier_default_config_matches_plan() {
    let config = RegimeConfig::default();

    assert_eq!(config.fast_ema_period, 20);
    assert_eq!(config.slow_ema_period, 50);
    assert_eq!(config.adx_period, 14);
    assert_eq!(config.atr_period, 14);
    assert_eq!(config.strong_adx, 25.0);
    assert_eq!(config.high_volatility_atr_pct, 6.0);
    assert_eq!(config.slope_bps, 20.0);
}

#[test]
fn regime_classifier_rejects_invalid_or_insufficient_bars() {
    let config = RegimeConfig::default();
    assert!(classify_regime(&[], &config).is_err());

    let invalid_bar_cases: Vec<(&str, Box<dyn Fn(&mut KlineBar)>)> = vec![
        ("open_nan", Box::new(|bar| bar.open = f64::NAN)),
        ("open_zero", Box::new(|bar| bar.open = 0.0)),
        ("high_infinite", Box::new(|bar| bar.high = f64::INFINITY)),
        ("high_zero", Box::new(|bar| bar.high = 0.0)),
        ("low_nan", Box::new(|bar| bar.low = f64::NAN)),
        ("low_zero", Box::new(|bar| bar.low = 0.0)),
        ("close_nan", Box::new(|bar| bar.close = f64::NAN)),
        ("close_zero", Box::new(|bar| bar.close = 0.0)),
        ("high_below_low", Box::new(|bar| bar.high = bar.low - 0.01)),
    ];

    for (case, mutate_latest) in invalid_bar_cases {
        let mut bars = synthetic_trend_bars();
        mutate_latest(bars.last_mut().expect("latest bar"));
        assert!(classify_regime(&bars, &config).is_err(), "case {case}");
    }

    let insufficient = vec![kline_bar(0, 100.0, 101.0, 99.0, 100.5)];
    let error = classify_regime(&insufficient, &config).expect_err("insufficient bars");
    assert!(error.contains("insufficient") || error.contains("indicator unavailable"));
}

#[test]
fn intelligent_search_keeps_same_symbol_long_short_futures_leverage_consistent() {
    let space = SearchSpace {
        symbols: vec!["BTCUSDT".to_string()],
        direction_mode: MartingaleDirectionMode::LongAndShort,
        directions: vec![MartingaleDirection::Long, MartingaleDirection::Short],
        market: Some(MartingaleMarketKind::UsdMFutures),
        margin_mode: None,
        step_bps: vec![80, 120],
        first_order_quote: vec![Decimal::new(10, 0)],
        multiplier: vec![Decimal::new(2, 0)],
        take_profit_bps: vec![80, 120],
        leverage: vec![2, 3, 4, 5, 6, 7, 8, 9, 10],
        max_legs: vec![4, 5],
        dynamic_allocation_enabled: true,
        short_stop_drawdown_pct_candidates: Vec::new(),
        short_atr_stop_multiplier_candidates: Vec::new(),
        allocation_cooldown_hours_candidates: Vec::new(),
    };

    let result = intelligent_search(
        &space,
        &IntelligentSearchConfig {
            seed: 7,
            random_round_size: 4,
            max_rounds: 3,
            max_candidates: 12,
            survivor_percentile: 0.5,
            timeout: None,
            scoring: ScoringConfig {
                min_trade_count: 1,
                min_data_quality_score: 0.0,
                ..ScoringConfig::default()
            },
        },
        None,
        |candidate| {
            candidate.config.validate()?;
            Ok(result(true, 5.0, 5.0, 10, 0, 100.0, Vec::new()))
        },
    )
    .expect("intelligent search");

    assert!(!result.candidates.is_empty());
    assert!(result.candidates.iter().all(|candidate| candidate
        .candidate
        .config
        .validate()
        .is_ok()));
}

#[test]
fn survival_failure_never_outranks_valid_candidate() {
    let valid = result(true, 5.0, 4.0, 120, 1, 500.0, vec![]);
    let failed = result(
        false,
        50.0,
        1.0,
        120,
        0,
        500.0,
        vec!["global_drawdown_exceeded".to_string()],
    );

    let valid_score = score_candidate(&valid, &ScoringConfig::default());
    let failed_score = score_candidate(&failed, &ScoringConfig::default());

    assert!(valid_score.survival_valid);
    assert!(!failed_score.survival_valid);
    assert!(valid_score.rank_score > failed_score.rank_score);
}

#[test]
fn invalid_candidate_never_outranks_extreme_negative_valid_candidate() {
    let mut valid = result(true, -1.0e300, 1.0e200, 100, 0, 100.0, vec![]);
    valid.metrics.global_drawdown_pct = Some(1.0);
    valid.metrics.max_strategy_drawdown_pct = Some(1.0);
    let failed = result(
        false,
        1.0e300,
        0.0,
        100,
        0,
        100.0,
        vec!["liquidation_hit".to_string()],
    );

    let valid_score = score_candidate(&valid, &ScoringConfig::default());
    let failed_score = score_candidate(&failed, &ScoringConfig::default());

    assert!(valid_score.raw_score.is_finite());
    assert!(failed_score.raw_score.is_finite());
    assert!(valid_score.survival_valid);
    assert!(!failed_score.survival_valid);
    assert!(valid_score.rank_score > failed_score.rank_score);
}

#[test]
fn walk_forward_windows_are_generated_in_order() {
    let windows = walk_forward_windows(WalkForwardConfig {
        start_ms: 0,
        end_ms: 10_000,
        train_ms: 3_000,
        validate_ms: 1_000,
        test_ms: 1_000,
        step_ms: 2_000,
    })
    .expect("windows");

    assert_eq!(windows.len(), 3);
    assert_eq!(windows[0].train.start_ms, 0);
    assert_eq!(windows[0].train.end_ms, 3_000);
    assert_eq!(windows[0].validate.start_ms, 3_000);
    assert_eq!(windows[0].test.end_ms, 5_000);
    assert!(windows
        .windows(2)
        .all(|pair| pair[0].train.start_ms < pair[1].train.start_ms));
    assert!(windows
        .iter()
        .all(|window| window.train.end_ms <= window.validate.start_ms));
    assert!(windows
        .iter()
        .all(|window| window.validate.end_ms <= window.test.start_ms));
}

#[test]
fn strategy_drawdown_rejects_when_global_drawdown_is_valid() {
    let mut candidate = result(true, 8.0, 4.0, 20, 0, 500.0, vec![]);
    candidate.metrics.global_drawdown_pct = Some(4.0);
    candidate.metrics.max_strategy_drawdown_pct = Some(25.0);

    let score = score_candidate(
        &candidate,
        &ScoringConfig {
            max_global_drawdown_pct: 10.0,
            max_strategy_drawdown_pct: 20.0,
            ..ScoringConfig::default()
        },
    );

    assert!(!score.survival_valid);
    assert!(score
        .rejection_reasons
        .iter()
        .any(|reason| reason == "strategy_drawdown_exceeded"));
    assert!(!score
        .rejection_reasons
        .iter()
        .any(|reason| reason == "global_drawdown_exceeded"));
}

#[test]
fn low_data_quality_rejects_even_with_enough_trades() {
    let mut candidate = result(true, 8.0, 4.0, 20, 0, 500.0, vec![]);
    candidate.metrics.data_quality_score = Some(0.75);

    let score = score_candidate(
        &candidate,
        &ScoringConfig {
            min_trade_count: 10,
            min_data_quality_score: 0.95,
            ..ScoringConfig::default()
        },
    );

    assert!(!score.survival_valid);
    assert!(score
        .rejection_reasons
        .iter()
        .any(|reason| reason == "insufficient_data_quality"));
}

#[test]
fn scoring_rejects_candidates_above_drawdown_limit_even_with_high_return() {
    let mut candidate = result(true, 1_000.0, 55.0, 200, 0, 500.0, vec![]);
    candidate.metrics.global_drawdown_pct = Some(55.0);
    candidate.metrics.max_strategy_drawdown_pct = Some(12.0);

    let score = score_candidate(
        &candidate,
        &ScoringConfig {
            max_global_drawdown_pct: 40.0,
            max_strategy_drawdown_pct: 40.0,
            min_trade_count: 1,
            min_data_quality_score: 0.0,
            ..ScoringConfig::default()
        },
    );

    assert!(!score.survival_valid);
    assert!(score.rank_score < 0.0);
    assert!(score
        .rejection_reasons
        .iter()
        .any(|reason| reason == "global_drawdown_exceeded"));
}

#[test]
fn scoring_penalizes_rebalance_and_forced_exit_churn() {
    let stable = result(true, 12.0, 4.0, 200, 0, 500.0, vec![]);
    let mut churned = stable.clone();
    churned.rebalance_count = 30;
    churned.forced_exit_count = 3;
    churned.average_allocation_hold_hours = Some(3.0);

    let config = ScoringConfig {
        min_trade_count: 1,
        min_data_quality_score: 0.0,
        ..ScoringConfig::default()
    };
    let stable_score = score_candidate(&stable, &config);
    let churned_score = score_candidate(&churned, &config);

    assert!(stable_score.survival_valid);
    assert!(churned_score.survival_valid);
    assert!(stable_score.raw_score > churned_score.raw_score);
    assert!(stable_score.rank_score > churned_score.rank_score);
}

#[test]
fn dynamic_allocation_metrics_serialize_for_worker_artifacts() {
    let result = MartingaleBacktestResult {
        metrics: MartingaleMetrics {
            total_return_pct: 1.5,
            max_drawdown_pct: 2.5,
            global_drawdown_pct: Some(2.5),
            max_strategy_drawdown_pct: Some(1.0),
            data_quality_score: Some(0.99),
            trade_count: 8,
            stop_count: 1,
            max_capital_used_quote: 250.0,
            survival_passed: true,
        },
        events: Vec::new(),
        equity_curve: Vec::new(),
        rejection_reasons: Vec::new(),
        allocation_curve: vec![AllocationCurvePoint {
            timestamp_ms: 1_700_000_000_000,
            symbol: "BTCUSDT".to_string(),
            long_weight_pct: 65.0,
            short_weight_pct: 35.0,
            action: AllocationAction::Rebalance,
            reason: "btc_range_symbol_uptrend".to_string(),
            in_cooldown: false,
        }],
        regime_timeline: vec![RegimeTimelinePoint {
            timestamp_ms: 1_700_000_000_000,
            symbol: "BTCUSDT".to_string(),
            btc_regime: MarketRegimeLabel::Uptrend,
            symbol_regime: MarketRegimeLabel::HighVolatility,
            extreme_risk: true,
        }],
        cost_summary: CostSummary {
            fee_quote: 1.25,
            slippage_quote: 0.5,
            stop_loss_quote: 2.0,
            forced_exit_quote: 3.0,
        },
        rebalance_count: 4,
        forced_exit_count: 2,
        average_allocation_hold_hours: Some(6.5),
    };

    let json = serde_json::to_value(&result).expect("serialize dynamic allocation metrics");

    assert_eq!(
        json["allocation_curve"][0]["timestamp_ms"],
        1_700_000_000_000_i64
    );
    assert_eq!(json["allocation_curve"][0]["symbol"], "BTCUSDT");
    assert_eq!(json["allocation_curve"][0]["long_weight_pct"], 65.0);
    assert_eq!(json["allocation_curve"][0]["short_weight_pct"], 35.0);
    assert_eq!(json["allocation_curve"][0]["action"], "rebalance");
    assert_eq!(
        json["allocation_curve"][0]["reason"],
        "btc_range_symbol_uptrend"
    );
    assert_eq!(json["allocation_curve"][0]["in_cooldown"], false);
    assert_eq!(json["regime_timeline"][0]["btc_regime"], "uptrend");
    assert_eq!(
        json["regime_timeline"][0]["symbol_regime"],
        "high_volatility"
    );
    assert_eq!(json["regime_timeline"][0]["extreme_risk"], true);
    assert_eq!(json["cost_summary"]["fee_quote"], 1.25);
    assert_eq!(json["cost_summary"]["slippage_quote"], 0.5);
    assert_eq!(json["cost_summary"]["stop_loss_quote"], 2.0);
    assert_eq!(json["cost_summary"]["forced_exit_quote"], 3.0);
    assert_eq!(json["rebalance_count"], 4);
    assert_eq!(json["forced_exit_count"], 2);
    assert_eq!(json["average_allocation_hold_hours"], 6.5);
}

#[test]
fn legacy_backtest_result_json_deserializes_dynamic_metrics_defaults() {
    let json = serde_json::json!({
        "metrics": {
            "total_return_pct": 1.0,
            "max_drawdown_pct": 2.0,
            "global_drawdown_pct": null,
            "max_strategy_drawdown_pct": null,
            "data_quality_score": null,
            "trade_count": 3,
            "stop_count": 0,
            "max_capital_used_quote": 100.0,
            "survival_passed": true
        },
        "events": [],
        "equity_curve": [],
        "rejection_reasons": []
    });

    let result = serde_json::from_value::<MartingaleBacktestResult>(json)
        .expect("deserialize legacy backtest result");

    assert_eq!(result.allocation_curve, Vec::new());
    assert_eq!(result.regime_timeline, Vec::new());
    assert_eq!(result.cost_summary, CostSummary::default());
    assert_eq!(result.rebalance_count, 0);
    assert_eq!(result.forced_exit_count, 0);
    assert_eq!(result.average_allocation_hold_hours, None);
}

#[test]
fn default_scoring_rejects_sparse_multi_year_trade_samples() {
    let candidate = result(true, 8.0, 4.0, 40, 0, 500.0, vec![]);

    let score = score_candidate(&candidate, &ScoringConfig::default());

    assert!(!score.survival_valid);
    assert!(score
        .rejection_reasons
        .iter()
        .any(|reason| reason == "insufficient_data_quality"));
}

#[test]
fn trade_refinement_preserves_same_timestamp_trade_order() {
    let result = run_trade_refinement(
        spot_portfolio("BTCUSDT"),
        &[trade(1_000, 100.0), trade(1_000, 101.5), trade(1_000, 98.5)],
    )
    .expect("trade refinement");

    let first_exit = result
        .events
        .iter()
        .find(|event| event.event_type == "take_profit" || event.event_type == "safety_order")
        .expect("exit or safety event");

    assert_eq!(first_exit.event_type, "take_profit");
}

#[test]
fn trade_refinement_rejects_timestamp_increment_overflow() {
    let error = trades_to_ordered_price_bars(&[trade(i64::MAX, 100.0), trade(i64::MAX, 101.0)])
        .expect_err("timestamp conflict at i64::MAX must fail");

    assert!(error.contains("timestamp"));
    assert!(error.contains("overflow"));
}

#[test]
fn walk_forward_windows_reject_timestamp_overflow() {
    let error = walk_forward_windows(WalkForwardConfig {
        start_ms: i64::MAX - 10,
        end_ms: i64::MAX,
        train_ms: 8,
        validate_ms: 8,
        test_ms: 8,
        step_ms: 1,
    })
    .expect_err("span overflow must fail");

    assert!(error.contains("overflow"));
}

#[test]
fn intelligent_search_stops_at_max_candidates_inside_round() {
    let space = small_search_space();
    let mut evaluations = 0;

    let result = intelligent_search(
        &space,
        &IntelligentSearchConfig {
            random_round_size: 8,
            max_rounds: 2,
            max_candidates: 3,
            ..IntelligentSearchConfig::default()
        },
        None,
        |_| {
            evaluations += 1;
            Ok(result(true, 1.0, 1.0, 10, 0, 100.0, vec![]))
        },
    )
    .expect("intelligent search");

    assert_eq!(evaluations, 3);
    assert_eq!(result.candidates.len(), 3);
    assert_eq!(result.stopped_reason, "max_candidates");
}

#[test]
fn intelligent_search_stops_inside_candidate_loop_when_cancelled() {
    let space = small_search_space();
    let cancel = AtomicBool::new(false);
    let mut evaluations = 0;

    let result = intelligent_search(
        &space,
        &IntelligentSearchConfig {
            random_round_size: 8,
            max_rounds: 2,
            max_candidates: 16,
            ..IntelligentSearchConfig::default()
        },
        Some(&cancel),
        |_| {
            evaluations += 1;
            cancel.store(true, Ordering::Relaxed);
            Ok(result(true, 1.0, 1.0, 10, 0, 100.0, vec![]))
        },
    )
    .expect("intelligent search");

    assert_eq!(evaluations, 1);
    assert_eq!(result.stopped_reason, "cancelled");
}

#[test]
fn named_stress_windows_cover_required_regimes() {
    let names: BTreeSet<_> = named_stress_windows()
        .into_iter()
        .map(|window| window.name)
        .collect();

    for expected in [
        "crash",
        "melt_up",
        "high_volatility_chop",
        "low_volatility_range",
        "long_unidirectional_trend",
        "wick_spike",
    ] {
        assert!(names.contains(expected), "missing {expected}");
    }
}

fn result(
    survival_passed: bool,
    total_return_pct: f64,
    max_drawdown_pct: f64,
    trade_count: u64,
    stop_count: u64,
    max_capital_used_quote: f64,
    rejection_reasons: Vec<String>,
) -> MartingaleBacktestResult {
    MartingaleBacktestResult {
        metrics: MartingaleMetrics {
            total_return_pct,
            max_drawdown_pct,
            global_drawdown_pct: None,
            max_strategy_drawdown_pct: None,
            data_quality_score: None,
            trade_count,
            stop_count,
            max_capital_used_quote,
            survival_passed,
        },
        events: Vec::new(),
        equity_curve: Vec::new(),
        rejection_reasons,
        allocation_curve: Vec::new(),
        regime_timeline: Vec::new(),
        cost_summary: CostSummary::default(),
        rebalance_count: 0,
        forced_exit_count: 0,
        average_allocation_hold_hours: None,
    }
}

#[allow(dead_code)]
fn spot_portfolio(symbol: &str) -> MartingalePortfolioConfig {
    MartingalePortfolioConfig {
        direction_mode: MartingaleDirectionMode::LongOnly,
        strategies: vec![MartingaleStrategyConfig {
            strategy_id: format!("{symbol}-long"),
            symbol: symbol.to_string(),
            market: MartingaleMarketKind::Spot,
            direction: MartingaleDirection::Long,
            direction_mode: MartingaleDirectionMode::LongOnly,
            margin_mode: None,
            leverage: None,
            spacing: MartingaleSpacingModel::FixedPercent { step_bps: 100 },
            sizing: MartingaleSizingModel::Multiplier {
                first_order_quote: Decimal::new(25, 0),
                multiplier: Decimal::new(15, 1),
                max_legs: 3,
            },
            take_profit: MartingaleTakeProfitModel::Percent { bps: 80 },
            stop_loss: None,
            indicators: Vec::new(),
            entry_triggers: Vec::new(),
            risk_limits: MartingaleRiskLimits::default(),
        }],
        risk_limits: MartingaleRiskLimits::default(),
    }
}

fn small_search_space() -> SearchSpace {
    SearchSpace {
        symbols: vec!["BTCUSDT".to_string()],
        direction_mode: MartingaleDirectionMode::LongOnly,
        directions: vec![MartingaleDirection::Long],
        market: None,
        margin_mode: None,
        step_bps: vec![100],
        first_order_quote: vec![Decimal::new(25, 0)],
        multiplier: vec![Decimal::new(15, 1)],
        take_profit_bps: vec![80],
        leverage: vec![1],
        max_legs: vec![3],
        dynamic_allocation_enabled: false,
        short_stop_drawdown_pct_candidates: Vec::new(),
        short_atr_stop_multiplier_candidates: Vec::new(),
        allocation_cooldown_hours_candidates: Vec::new(),
    }
}

fn short_stop_search_space(
    direction_mode: MartingaleDirectionMode,
    directions: Vec<MartingaleDirection>,
    market: Option<MartingaleMarketKind>,
) -> SearchSpace {
    SearchSpace {
        symbols: vec!["BTCUSDT".to_string()],
        direction_mode,
        directions,
        market,
        margin_mode: None,
        step_bps: vec![100],
        first_order_quote: vec![Decimal::new(10, 0)],
        multiplier: vec![Decimal::new(2, 0)],
        take_profit_bps: vec![100],
        leverage: vec![3],
        max_legs: vec![4],
        dynamic_allocation_enabled: false,
        short_stop_drawdown_pct_candidates: Vec::new(),
        short_atr_stop_multiplier_candidates: Vec::new(),
        allocation_cooldown_hours_candidates: Vec::new(),
    }
}

fn trade(trade_time_ms: i64, price: f64) -> AggTrade {
    AggTrade {
        symbol: "BTCUSDT".to_string(),
        trade_time_ms,
        price,
        quantity: 1.0,
        is_buyer_maker: false,
    }
}
