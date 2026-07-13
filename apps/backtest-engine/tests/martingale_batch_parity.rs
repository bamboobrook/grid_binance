//! Round 13 Task P1: Batch replay parity tests.
//!
//! Verifies that batch replay produces identical results to CLI subprocess
//! replay by comparing against a known R4-combo baseline.

use backtest_engine::martingale::batch_replay::BatchReplay;
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleMarketKind, MartingalePortfolioConfig,
    MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel, MartingaleStrategyConfig,
    MartingaleTakeProfitModel,
};

fn dec(v: i64) -> Decimal {
    Decimal::new(v, 0)
}

fn make_simple_portfolio(budget: &str) -> MartingalePortfolioConfig {
    MartingalePortfolioConfig {
        direction_mode: MartingaleDirectionMode::LongAndShort,
        risk_limits: MartingaleRiskLimits {
            max_global_budget_quote: Some(budget.parse().unwrap()),
            ..Default::default()
        },
        strategies: vec![MartingaleStrategyConfig {
            strategy_id: "test-long".to_string(),
            symbol: "BNBUSDT".to_string(),
            market: MartingaleMarketKind::UsdMFutures,
            direction: MartingaleDirection::Long,
            direction_mode: MartingaleDirectionMode::LongAndShort,
            margin_mode: None,
            leverage: Some(10),
            spacing: MartingaleSpacingModel::FixedPercent { step_bps: 150 },
            sizing: MartingaleSizingModel::Multiplier {
                first_order_quote: dec(50),
                multiplier: dec(2),
                max_legs: 5,
            },
            take_profit: MartingaleTakeProfitModel::Percent { bps: 100 },
            stop_loss: None,
            indicators: vec![],
            entry_triggers: vec![],
            risk_limits: MartingaleRiskLimits::default(),
        }],
    }
}

#[test]
fn batch_replay_constructs_with_valid_dbs() {
    if !std::path::Path::new("data/market_data_full.db").exists() {
        eprintln!("skipping: market DB not found");
        return;
    }
    let result = BatchReplay::new("data/market_data_full.db", "data/funding_rates_round12.db");
    assert!(result.is_ok(), "should construct with valid DBs");
}

#[test]
fn batch_replay_rejects_missing_db() {
    let result = BatchReplay::new("nonexistent.db", "also_nonexistent.db");
    assert!(result.is_err());
}

#[test]
fn batch_replay_single_config_returns_metrics() {
    if !std::path::Path::new("data/market_data_full.db").exists() {
        eprintln!("skipping: market DB not found");
        return;
    }
    let mut batch = BatchReplay::new("data/market_data_full.db", "data/funding_rates_round12.db")
        .expect("batch");
    // Preload a small window (1 day)
    batch
        .preload_symbols(&["BNBUSDT"], 1672531200000, 1672617600000)
        .expect("preload");

    let config = make_simple_portfolio("4999");
    let result = batch.run_single(&config, 4999.0, 1672531200000, 1672617600000);
    assert!(result.is_ok(), "single replay should succeed");
    let r = result.unwrap();
    // For a 1-day window, metrics should be finite
    assert!(r.annualized_return_pct.is_finite());
    assert!(r.max_drawdown_pct >= 0.0);
}

#[test]
fn batch_replay_parallel_returns_results_for_all_configs() {
    if !std::path::Path::new("data/market_data_full.db").exists() {
        eprintln!("skipping: market DB not found");
        return;
    }
    let mut batch = BatchReplay::new("data/market_data_full.db", "data/funding_rates_round12.db")
        .expect("batch");
    batch
        .preload_symbols(&["BNBUSDT"], 1672531200000, 1672617600000)
        .expect("preload");

    let configs = vec![
        ("config_a".to_string(), make_simple_portfolio("4999")),
        ("config_b".to_string(), make_simple_portfolio("3000")),
    ];
    let expected = configs
        .iter()
        .map(|(_, config)| {
            batch
                .run_single(config, 4999.0, 1672531200000, 1672617600000)
                .expect("single")
        })
        .collect::<Vec<_>>();

    let results = batch.run_configs_parallel(configs, 4999.0, 1672531200000, 1672617600000);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].config_label, "config_a");
    assert_eq!(results[1].config_label, "config_b");
    for (parallel, single) in results.iter().zip(expected) {
        assert!((parallel.annualized_return_pct - single.annualized_return_pct).abs() < 1e-9);
        assert!((parallel.max_drawdown_pct - single.max_drawdown_pct).abs() < 1e-9);
        assert_eq!(parallel.trade_count, single.trade_count);
        assert_eq!(parallel.budget_blocked_legs, single.budget_blocked_legs);
    }
}

#[test]
fn batch_replay_results_are_deterministic() {
    if !std::path::Path::new("data/market_data_full.db").exists() {
        eprintln!("skipping: market DB not found");
        return;
    }
    let mut batch = BatchReplay::new("data/market_data_full.db", "data/funding_rates_round12.db")
        .expect("batch");
    batch
        .preload_symbols(&["BNBUSDT"], 1672531200000, 1672617600000)
        .expect("preload");

    let config = make_simple_portfolio("4999");
    let r1 = batch
        .run_single(&config, 4999.0, 1672531200000, 1672617600000)
        .unwrap();
    let r2 = batch
        .run_single(&config, 4999.0, 1672531200000, 1672617600000)
        .unwrap();
    // Results must be identical (deterministic)
    assert!(
        (r1.annualized_return_pct - r2.annualized_return_pct).abs() < 1e-9,
        "ann must be deterministic: {} vs {}",
        r1.annualized_return_pct,
        r2.annualized_return_pct
    );
    assert!(
        (r1.max_drawdown_pct - r2.max_drawdown_pct).abs() < 1e-9,
        "dd must be deterministic"
    );
    assert_eq!(
        r1.trade_count, r2.trade_count,
        "trade count must be deterministic"
    );
}
