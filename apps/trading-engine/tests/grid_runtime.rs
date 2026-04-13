use rust_decimal::Decimal;
use trading_engine::grid_builder::GridBuilder;
use trading_engine::runtime::{GridMode, GridRuntime, GridRuntimeConfig, RuntimeStatus};
use trading_engine::stop_loss::OverallStopLoss;
use trading_engine::take_profit::{MakerTakeProfit, OverallTakeProfit, TrailingTakeProfit};

fn decimal(value: i64, scale: u32) -> Decimal {
    Decimal::new(value, scale)
}

fn runtime_config() -> GridRuntimeConfig {
    let plan = GridBuilder::custom(
        GridMode::SpotClassic,
        vec![decimal(90, 0), decimal(100, 0), decimal(110, 0)],
    )
    .expect("custom grid should build");

    GridRuntimeConfig {
        mode: GridMode::SpotClassic,
        plan,
        quantity: decimal(1, 0),
        maker_take_profit: None,
        trailing_take_profit: None,
        overall_take_profit: None,
        overall_stop_loss: None,
    }
}

#[test]
fn arithmetic_grid_builds_evenly_spaced_levels() {
    let grid = GridBuilder::arithmetic(GridMode::SpotClassic, decimal(100, 0), decimal(130, 0), 4)
        .expect("arithmetic grid should build");

    assert_eq!(
        grid.levels,
        vec![
            decimal(100, 0),
            decimal(110, 0),
            decimal(120, 0),
            decimal(130, 0)
        ]
    );
}

#[test]
fn geometric_grid_builds_progressive_levels() {
    let grid = GridBuilder::geometric(GridMode::SpotClassic, decimal(100, 0), decimal(800, 0), 4)
        .expect("geometric grid should build");

    assert_eq!(
        grid.levels,
        vec![
            decimal(100, 0),
            decimal(200, 0),
            decimal(400, 0),
            decimal(800, 0)
        ]
    );
}

#[test]
fn geometric_grid_rejects_rounded_duplicate_levels() {
    let result = GridBuilder::geometric(
        GridMode::SpotClassic,
        decimal(1000000000, 8),
        decimal(1000000001, 8),
        3,
    );

    assert!(result.is_err());
}

#[test]
fn custom_grid_preserves_user_levels() {
    let levels = vec![decimal(95, 0), decimal(100, 0), decimal(1075, 1)];
    let grid = GridBuilder::custom(GridMode::SpotClassic, levels.clone())
        .expect("custom grid should build");

    assert_eq!(grid.levels, levels);
}

#[test]
fn maker_take_profit_closes_at_target_price() {
    let mut config = runtime_config();
    config.maker_take_profit = Some(MakerTakeProfit {
        target_percent: decimal(5, 2),
    });

    let mut runtime = GridRuntime::new(config).expect("spot long runtime should build");
    runtime
        .record_fill(decimal(100, 0), decimal(1, 0))
        .expect("positive quantity fill should succeed");

    let events = runtime.on_price(decimal(105, 0));

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].reason(), "maker_take_profit");
    assert_eq!(events[0].exit_price(), Some(decimal(105, 0)));
    assert!(runtime.position().is_none());
    assert_eq!(runtime.realized_pnl(), decimal(5, 0));
}

#[test]
fn trailing_take_profit_uses_post_activation_high() {
    let mut config = runtime_config();
    config.trailing_take_profit = Some(TrailingTakeProfit {
        trigger_price: decimal(110, 0),
        trailing_percent: decimal(10, 2),
    });

    let mut runtime = GridRuntime::new(config).expect("spot long runtime should build");
    runtime
        .record_fill(decimal(100, 0), decimal(1, 0))
        .expect("positive quantity fill should succeed");

    assert!(runtime.on_price(decimal(109, 0)).is_empty());
    assert!(runtime.on_price(decimal(111, 0)).is_empty());
    assert!(runtime.on_price(decimal(120, 0)).is_empty());
    assert!(runtime.on_price(decimal(109, 0)).is_empty());

    let events = runtime.on_price(decimal(107, 0));

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].reason(), "taker_trailing_take_profit");
    assert_eq!(events[0].exit_price(), Some(decimal(107, 0)));
    assert_eq!(runtime.realized_pnl(), decimal(7, 0));
}

#[test]
fn overall_take_profit_stops_runtime() {
    let mut config = runtime_config();
    config.overall_take_profit = Some(OverallTakeProfit {
        target_percent: decimal(10, 2),
    });

    let mut runtime = GridRuntime::new(config).expect("spot long runtime should build");
    runtime
        .record_fill(decimal(100, 0), decimal(1, 0))
        .expect("positive quantity fill should succeed");

    let events = runtime.on_price(decimal(110, 0));

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].reason(), "overall_take_profit");
    assert_eq!(runtime.status(), RuntimeStatus::Stopped);
    assert!(runtime.position().is_none());
}

#[test]
fn overall_stop_loss_stops_runtime() {
    let mut config = runtime_config();
    config.overall_stop_loss = Some(OverallStopLoss {
        max_drawdown_percent: decimal(5, 2),
    });

    let mut runtime = GridRuntime::new(config).expect("spot long runtime should build");
    runtime
        .record_fill(decimal(100, 0), decimal(1, 0))
        .expect("positive quantity fill should succeed");

    let events = runtime.on_price(decimal(95, 0));

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].reason(), "overall_stop_loss");
    assert_eq!(runtime.status(), RuntimeStatus::Stopped);
    assert_eq!(runtime.realized_pnl(), decimal(-5, 0));
}

#[test]
fn pause_resume_stop_and_rebuild_follow_runtime_lifecycle() {
    let mut config = runtime_config();
    config.maker_take_profit = Some(MakerTakeProfit {
        target_percent: decimal(5, 2),
    });

    let mut runtime = GridRuntime::new(config).expect("spot long runtime should build");
    runtime
        .record_fill(decimal(100, 0), decimal(1, 0))
        .expect("positive quantity fill should succeed");
    runtime.pause();

    assert!(runtime.on_price(decimal(105, 0)).is_empty());
    assert!(runtime.position().is_some());
    assert_eq!(runtime.status(), RuntimeStatus::Paused);

    runtime.resume();

    let resumed_events = runtime.on_price(decimal(105, 0));
    assert_eq!(resumed_events.len(), 1);
    assert_eq!(runtime.realized_pnl(), decimal(5, 0));

    runtime.stop();
    assert_eq!(runtime.status(), RuntimeStatus::Stopped);
    assert!(runtime.on_price(decimal(120, 0)).is_empty());

    let rebuilt =
        GridBuilder::arithmetic(GridMode::SpotClassic, decimal(80, 0), decimal(100, 0), 3)
            .expect("rebuilt grid should build");
    runtime
        .rebuild(rebuilt)
        .expect("supported rebuild should succeed");

    assert_eq!(runtime.status(), RuntimeStatus::Running);
    assert_eq!(
        runtime.grid().levels,
        vec![decimal(80, 0), decimal(90, 0), decimal(100, 0)]
    );
    assert_eq!(runtime.realized_pnl(), decimal(0, 0));
    assert!(runtime.position().is_none());
}

#[test]
fn runtime_rejects_plan_mode_mismatch() {
    let plan = GridBuilder::custom(
        GridMode::FuturesShort,
        vec![decimal(90, 0), decimal(100, 0), decimal(110, 0)],
    )
    .expect("custom grid should build");

    let config = GridRuntimeConfig {
        mode: GridMode::SpotClassic,
        plan,
        quantity: decimal(1, 0),
        maker_take_profit: None,
        trailing_take_profit: None,
        overall_take_profit: None,
        overall_stop_loss: None,
    };

    let result = GridRuntime::new(config);

    assert!(result.is_err());
}

#[test]
fn runtime_accepts_required_sell_short_and_neutral_modes() {
    for mode in [
        GridMode::SpotSellOnly,
        GridMode::FuturesShort,
        GridMode::FuturesNeutral,
    ] {
        let plan = GridBuilder::custom(
            mode,
            vec![decimal(90, 0), decimal(100, 0), decimal(110, 0)],
        )
        .expect("custom grid should build");
        let config = GridRuntimeConfig {
            mode,
            plan,
            quantity: decimal(1, 0),
            maker_take_profit: None,
            trailing_take_profit: None,
            overall_take_profit: None,
            overall_stop_loss: None,
        };

        let runtime = GridRuntime::new(config);
        assert!(
            runtime.is_ok(),
            "mode {mode:?} should be accepted by runtime"
        );
    }
}

#[test]
fn runtime_rejects_non_positive_default_quantity() {
    let mut config = runtime_config();
    config.quantity = Decimal::ZERO;

    let zero_result = GridRuntime::new(config.clone());
    assert!(zero_result.is_err());

    config.quantity = decimal(-1, 0);
    let negative_result = GridRuntime::new(config);
    assert!(negative_result.is_err());
}

#[test]
fn record_fill_rejects_non_positive_quantity() {
    let mut runtime = GridRuntime::new(runtime_config()).expect("spot long runtime should build");

    let zero_result = runtime.record_fill(decimal(100, 0), Decimal::ZERO);
    assert!(zero_result.is_err());
    assert!(runtime.position().is_none());

    let negative_result = runtime.record_fill(decimal(100, 0), decimal(-1, 0));
    assert!(negative_result.is_err());
    assert!(runtime.position().is_none());
}
