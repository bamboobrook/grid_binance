use backtest_engine::martingale::metrics::{MartingaleBacktestResult, MartingaleMetrics};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};

use backtest_engine::intelligent_search::{intelligent_search, IntelligentSearchConfig};
use backtest_engine::market_data::AggTrade;
use backtest_engine::martingale::scoring::{score_candidate, ScoringConfig};
use backtest_engine::martingale::trade_engine::{
    run_trade_refinement, trades_to_ordered_price_bars,
};
use backtest_engine::search::{random_search, SearchSpace};
use backtest_engine::time_splits::{named_stress_windows, walk_forward_windows, WalkForwardConfig};
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleMarketKind, MartingalePortfolioConfig,
    MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel, MartingaleStrategyConfig,
    MartingaleTakeProfitModel,
};

#[test]
fn random_search_is_reproducible() {
    let space = SearchSpace {
        symbols: vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
        directions: vec![MartingaleDirection::Long, MartingaleDirection::Short],
        step_bps: vec![50, 100, 150],
        first_order_quote: vec![Decimal::new(25, 0), Decimal::new(50, 0)],
        take_profit_bps: vec![60, 90],
        leverage: vec![1, 3],
        max_legs: vec![3, 4],
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
fn survival_failure_never_outranks_valid_candidate() {
    let valid = result(true, 5.0, 4.0, 20, 1, 500.0, vec![]);
    let failed = result(
        false,
        50.0,
        1.0,
        20,
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
        directions: vec![MartingaleDirection::Long],
        step_bps: vec![100],
        first_order_quote: vec![Decimal::new(25, 0)],
        take_profit_bps: vec![80],
        leverage: vec![1],
        max_legs: vec![3],
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
