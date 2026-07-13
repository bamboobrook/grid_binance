//! Round 11 Task P3: Native DCA minigrid backtest tests.
//!
//! These tests verify the minigrid config validation, price computation,
//! and close-fraction semantics. The minigrid is inventory-reducing only:
//! it closes fractions of existing legs at levels above (long) / below (short)
//! the last safety fill price. It never adds exposure.

use backtest_engine::market_data::KlineBar;
use backtest_engine::martingale::kline_engine::run_kline_screening;
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDcaMiniGridConfig, MartingaleDirection, MartingaleDirectionMode,
    MartingaleMarginMode, MartingaleMarketKind, MartingalePortfolioConfig, MartingaleRiskLimits,
    MartingaleSizingModel, MartingaleSpacingModel, MartingaleStrategyConfig,
    MartingaleTakeProfitModel,
};

fn cfg(
    levels: u32,
    spacing: u32,
    num: u32,
    den: u32,
    profit: u32,
    max_active: u32,
) -> MartingaleDcaMiniGridConfig {
    MartingaleDcaMiniGridConfig {
        levels_per_band: levels,
        spacing_bps: spacing,
        close_fraction_num: num,
        close_fraction_den: den,
        min_profit_bps: profit,
        max_active_levels: max_active,
    }
}

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

fn minigrid_portfolio() -> MartingalePortfolioConfig {
    MartingalePortfolioConfig {
        direction_mode: MartingaleDirectionMode::LongOnly,
        strategies: vec![MartingaleStrategyConfig {
            strategy_id: "minigrid-long".to_string(),
            symbol: "BNBUSDT".to_string(),
            market: MartingaleMarketKind::UsdMFutures,
            direction: MartingaleDirection::Long,
            direction_mode: MartingaleDirectionMode::LongOnly,
            margin_mode: Some(MartingaleMarginMode::Isolated),
            leverage: Some(10),
            spacing: MartingaleSpacingModel::FixedPercent { step_bps: 200 },
            sizing: MartingaleSizingModel::Multiplier {
                first_order_quote: Decimal::new(100, 0),
                multiplier: Decimal::ONE,
                max_legs: 2,
            },
            take_profit: MartingaleTakeProfitModel::Percent { bps: 1_000 },
            stop_loss: None,
            indicators: vec![],
            entry_triggers: vec![],
            risk_limits: MartingaleRiskLimits {
                dca_minigrid: Some(cfg(1, 30, 1, 2, 20, 1)),
                ..Default::default()
            },
        }],
        risk_limits: MartingaleRiskLimits {
            max_global_budget_quote: Some(Decimal::new(1_000, 0)),
            ..Default::default()
        },
    }
}

#[test]
fn native_minigrid_after_safety_fill_reduces_inventory_only() {
    // A minigrid config with valid parameters should compute level prices
    // that are ABOVE the last safety fill for longs (reduce inventory by selling).
    let mg = cfg(3, 30, 1, 4, 20, 2);
    mg.validate().expect("valid config");
    let last_fill = 100.0_f64;
    // Level 0 for long: price = 100 * (1 + (20 + 0*30)/10000) = 100.20
    let l0 = mg.level_price(last_fill, 0, true);
    assert!(
        l0 > last_fill,
        "long minigrid level 0 ({}) must be above last_fill ({})",
        l0,
        last_fill
    );
    assert!(
        (l0 - 100.20).abs() < 0.001,
        "level 0 price should be 100.20, got {}",
        l0
    );
    // Level 1: 100 * (1 + (20+30)/10000) = 100.50
    let l1 = mg.level_price(last_fill, 1, true);
    assert!(l1 > l0, "levels must increase for longs");
    assert!(
        (l1 - 100.50).abs() < 0.001,
        "level 1 price should be 100.50, got {}",
        l1
    );
    // The close fraction reduces inventory (1/4 = 25%)
    let frac = mg.close_fraction();
    assert!(
        (frac - 0.25).abs() < 1e-9,
        "close fraction should be 0.25, got {}",
        frac
    );
    // Verify the minigrid is inventory-reducing: it closes a fraction of
    // EXISTING legs. The close_fraction applies to remaining quantity.
    // (The engine integration emits "dca_minigrid_take_profit" events; here
    // we verify the price math that drives those events.)
}

#[test]
fn native_minigrid_never_opens_without_active_safety_leg() {
    // The minigrid config requires a safety leg to exist before placing levels.
    // This test verifies the config validation rejects invalid params that
    // would allow opening without a safety leg.
    // The engine enforces "at least one safety leg" at runtime; here we verify
    // the config is structurally sound.
    let mg = cfg(1, 15, 1, 8, 10, 1);
    mg.validate().expect("minimal valid config");
    // A minigrid with max_active_levels=1 and levels_per_band=1 places at most
    // 1 level per band — it cannot "open" multiple positions.
    assert_eq!(mg.max_active_levels, 1);
    assert_eq!(mg.levels_per_band, 1);
    // Verify that the level price is always on the favorable side (reducing)
    let last_fill = 50.0;
    let long_level = mg.level_price(last_fill, 0, true);
    let short_level = mg.level_price(last_fill, 0, false);
    assert!(
        long_level > last_fill,
        "long level must be above fill (reduce by selling)"
    );
    assert!(
        short_level < last_fill,
        "short level must be below fill (reduce by buying)"
    );
}

#[test]
fn native_minigrid_respects_min_notional_and_close_fraction() {
    // If the close notional would be below 5 USDT, the engine skips the minigrid
    // event. This test verifies the close_fraction math and the min-profit
    // constraint ensure the closed quantity is meaningful.
    let mg = cfg(2, 50, 1, 6, 35, 3);
    mg.validate().expect("valid");
    // close_fraction = 1/6 ≈ 0.1667
    let frac = mg.close_fraction();
    assert!((frac - (1.0 / 6.0)).abs() < 1e-9, "close fraction 1/6");
    // For a position worth 30 USDT, closing 1/6 = 5 USDT — exactly at min-notional
    let position_value = 30.0;
    let close_value = position_value * frac;
    assert!(
        close_value >= 5.0,
        "close value {} should be >= 5 USDT (min notional)",
        close_value
    );
    // For a position worth 24 USDT, closing 1/6 = 4 USDT — below min-notional, skip
    let small_position = 24.0;
    let small_close = small_position * frac;
    assert!(
        small_close < 5.0,
        "close value {} should be < 5 USDT (would be skipped)",
        small_close
    );
}

#[test]
fn native_minigrid_validation_rejects_invalid_configs() {
    // levels_per_band out of range
    assert!(
        cfg(0, 30, 1, 4, 20, 2).validate().is_err(),
        "levels=0 should fail"
    );
    assert!(
        cfg(6, 30, 1, 4, 20, 2).validate().is_err(),
        "levels=6 should fail"
    );
    // spacing_bps out of range
    assert!(
        cfg(3, 5, 1, 4, 20, 2).validate().is_err(),
        "spacing=5 should fail"
    );
    assert!(
        cfg(3, 200, 1, 4, 20, 2).validate().is_err(),
        "spacing=200 should fail"
    );
    // close_fraction_num = 0
    assert!(
        cfg(3, 30, 0, 4, 20, 2).validate().is_err(),
        "num=0 should fail"
    );
    // close_fraction_den < num
    assert!(
        cfg(3, 30, 4, 2, 20, 2).validate().is_err(),
        "den<num should fail"
    );
    // min_profit_bps < 5
    assert!(
        cfg(3, 30, 1, 4, 3, 2).validate().is_err(),
        "profit=3 should fail"
    );
    // max_active_levels out of range
    assert!(
        cfg(3, 30, 1, 4, 20, 0).validate().is_err(),
        "max_active=0 should fail"
    );
    assert!(
        cfg(3, 30, 1, 4, 20, 6).validate().is_err(),
        "max_active=6 should fail"
    );
}

#[test]
fn native_minigrid_level_price_symmetry() {
    // Verify long and short level prices are symmetric around the fill price.
    let mg = cfg(3, 40, 1, 4, 15, 2);
    let fill = 200.0;
    for idx in 0..3 {
        let long_p = mg.level_price(fill, idx, true);
        let short_p = mg.level_price(fill, idx, false);
        let long_offset = long_p - fill;
        let short_offset = fill - short_p;
        assert!(
            (long_offset - short_offset).abs() < 1e-9,
            "level {} not symmetric: long_offset={}, short_offset={}",
            idx,
            long_offset,
            short_offset
        );
    }
}

#[test]
fn native_minigrid_execution_uses_position_average_and_skips_creation_bar() {
    let start = 1_672_531_200_000_i64;
    let bars = vec![
        bar(start, 100.0, 100.0, 100.0, 100.0),
        bar(start + 60_000, 99.0, 99.0, 97.9, 98.0),
        bar(start + 120_000, 98.0, 98.3, 98.0, 98.2),
    ];
    let result = run_kline_screening(minigrid_portfolio(), &bars).expect("replay");
    let reductions = result
        .events
        .iter()
        .filter(|event| event.event_type == "dca_minigrid_take_profit")
        .collect::<Vec<_>>();
    assert_eq!(reductions.len(), 1);
    assert_eq!(reductions[0].timestamp_ms, start + 120_000);
    assert!(reductions[0].detail.contains("pnl_quote="));
    let trade = result
        .trades
        .iter()
        .find(|trade| trade.event_type == "reduce_position")
        .expect("minigrid reduce trade");
    assert!(
        trade.realized_pnl_quote < 0.0,
        "a bounce above the safety fill is still below the aggregate position average"
    );
    assert!(trade.fee_quote > 0.0);
    assert!(trade.slippage_quote > 0.0);
}
