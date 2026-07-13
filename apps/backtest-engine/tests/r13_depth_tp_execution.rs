use backtest_engine::market_data::KlineBar;
use backtest_engine::martingale::kline_engine::run_kline_screening;
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDepthTpConfig, MartingaleDirection, MartingaleDirectionMode, MartingaleMarginMode,
    MartingaleMarketKind, MartingalePortfolioConfig, MartingaleRiskLimits, MartingaleSizingModel,
    MartingaleSpacingModel, MartingaleStrategyConfig, MartingaleTakeProfitModel,
};

fn bar(timestamp_ms: i64, open: f64, high: f64, low: f64, close: f64) -> KlineBar {
    KlineBar {
        symbol: "BNBUSDT".to_string(),
        open_time_ms: timestamp_ms,
        open,
        high,
        low,
        close,
        volume: 1.0,
    }
}

fn portfolio(reduce_pct: u32) -> MartingalePortfolioConfig {
    MartingalePortfolioConfig {
        direction_mode: MartingaleDirectionMode::LongOnly,
        strategies: vec![MartingaleStrategyConfig {
            strategy_id: "depth-long".to_string(),
            symbol: "BNBUSDT".to_string(),
            market: MartingaleMarketKind::UsdMFutures,
            direction: MartingaleDirection::Long,
            direction_mode: MartingaleDirectionMode::LongOnly,
            margin_mode: Some(MartingaleMarginMode::Isolated),
            leverage: Some(10),
            spacing: MartingaleSpacingModel::FixedPercent { step_bps: 100 },
            sizing: MartingaleSizingModel::Multiplier {
                first_order_quote: Decimal::new(100, 0),
                multiplier: Decimal::ONE,
                max_legs: 5,
            },
            take_profit: MartingaleTakeProfitModel::Partial {
                stages: vec![(300, 1_000, 1_000), (1, 1, 2_000)],
                breakeven_after_stage: 1,
                breakeven_buffer_bps: 100,
            },
            stop_loss: None,
            indicators: vec![],
            entry_triggers: vec![],
            risk_limits: MartingaleRiskLimits {
                depth_tp: Some(MartingaleDepthTpConfig {
                    depth_01_tp_bps: 5_000,
                    depth_23_tp_bps: 40,
                    depth_23_reduce_pct: reduce_pct,
                    depth_4plus_tp_bps: 20,
                    depth_4plus_reduce_pct: 50,
                }),
                ..Default::default()
            },
        }],
        risk_limits: MartingaleRiskLimits {
            max_global_budget_quote: Some(Decimal::new(1_000, 0)),
            ..Default::default()
        },
    }
}

fn depth_bars() -> Vec<KlineBar> {
    let start = 1_672_531_200_000_i64;
    vec![
        bar(start, 100.0, 100.0, 100.0, 100.0),
        bar(start + 60_000, 99.5, 99.6, 98.9, 99.0),
        bar(start + 120_000, 98.5, 98.6, 97.9, 98.0),
        bar(start + 180_000, 99.0, 99.5, 98.9, 99.2),
        bar(start + 240_000, 99.1, 99.5, 99.0, 99.2),
    ]
}

#[test]
fn depth_tp_uses_configured_reduce_fraction_once_per_depth() {
    let result = run_kline_screening(portfolio(25), &depth_bars()).expect("replay");
    let reductions = result
        .events
        .iter()
        .filter(|event| event.event_type == "depth_take_profit_reduce")
        .collect::<Vec<_>>();
    assert_eq!(reductions.len(), 1);
    assert!(reductions[0].detail.contains("filled_legs=3"));
    assert!(reductions[0].detail.contains("close_frac=0.25"));
    let trade = result
        .trades
        .iter()
        .find(|trade| trade.event_type == "reduce_position")
        .expect("depth reduce trade");
    assert!(trade.realized_pnl_quote.is_finite());
    assert!(trade.fee_quote > 0.0);
}

#[test]
fn depth_tp_zero_reduce_closes_cycle_instead_of_using_original_partial_model() {
    let result = run_kline_screening(portfolio(0), &depth_bars()).expect("replay");
    assert!(result
        .events
        .iter()
        .any(|event| event.event_type == "take_profit"));
    assert!(!result
        .events
        .iter()
        .any(|event| event.event_type == "depth_take_profit_reduce"));
    assert!(!result
        .events
        .iter()
        .any(|event| event.event_type == "partial_take_profit"));
}
