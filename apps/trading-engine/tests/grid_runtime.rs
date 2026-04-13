use rust_decimal::Decimal;
use trading_engine::grid_builder::GridBuilder;
use trading_engine::runtime::{
    GridMode, GridRuntime, GridRuntimeConfig, RuntimeOrderKind, RuntimeStatus,
};
use trading_engine::stop_loss::OverallStopLoss;
use trading_engine::take_profit::{MakerTakeProfit, OverallTakeProfit, TrailingTakeProfit};

fn decimal(value: i64, scale: u32) -> Decimal {
    Decimal::new(value, scale)
}

fn runtime_config() -> GridRuntimeConfig {
    let plan = GridBuilder::custom(
        GridMode::SpotGrid,
        vec![decimal(110, 0), decimal(100, 0), decimal(90, 0)],
    )
    .expect("custom grid should build");

    GridRuntimeConfig {
        mode: GridMode::SpotGrid,
        plan,
        quantity: decimal(1, 0),
        ordinary_take_profit_bps: 100,
        maker_take_profit: None,
        trailing_take_profit: None,
        overall_take_profit: None,
        overall_stop_loss: None,
    }
}

fn short_runtime_config() -> GridRuntimeConfig {
    let plan = GridBuilder::custom(
        GridMode::FuturesShort,
        vec![decimal(90, 0), decimal(100, 0), decimal(110, 0)],
    )
    .expect("custom short grid should build");

    GridRuntimeConfig {
        mode: GridMode::FuturesShort,
        plan,
        quantity: decimal(1, 0),
        ordinary_take_profit_bps: 100,
        maker_take_profit: None,
        trailing_take_profit: None,
        overall_take_profit: None,
        overall_stop_loss: None,
    }
}

#[test]
fn ordinary_spot_grid_uses_anchor_fixed_step_without_bilateral_levels() {
    let grid = GridBuilder::ordinary_fixed_step(GridMode::SpotGrid, decimal(70000, 0), 100, 4)
        .expect("ordinary grid should build");

    assert_eq!(
        grid.levels,
        vec![
            decimal(70000, 0),
            decimal(69300, 0),
            decimal(68600, 0),
            decimal(67900, 0)
        ]
    );
    assert!(grid.lower_levels.is_empty());
    assert!(grid.upper_levels.is_empty());
}

#[test]
fn classic_bilateral_grid_supports_fixed_and_geometric_spacing() {
    let fixed = GridBuilder::classic_bilateral_fixed(
        GridMode::ClassicBilateralSpot,
        decimal(70000, 0),
        100,
        2,
    )
    .expect("fixed bilateral grid should build");
    assert!(fixed.levels.is_empty());
    assert_eq!(
        fixed.lower_levels,
        vec![decimal(69300, 0), decimal(68600, 0)]
    );
    assert_eq!(
        fixed.upper_levels,
        vec![decimal(70700, 0), decimal(71400, 0)]
    );

    let geometric = GridBuilder::classic_bilateral_geometric(
        GridMode::ClassicBilateralSpot,
        decimal(70000, 0),
        100,
        2,
    )
    .expect("geometric bilateral grid should build");
    assert!(geometric.levels.is_empty());
    assert_eq!(
        geometric.lower_levels,
        vec![decimal(69300, 0), decimal(68607, 0)]
    );
    assert_eq!(
        geometric.upper_levels,
        vec![decimal(70700, 0), decimal(71407, 0)]
    );
}

#[test]
fn custom_grid_preserves_spot_levels_in_execution_order() {
    let levels = vec![decimal(1075, 1), decimal(100, 0), decimal(95, 0)];
    let grid =
        GridBuilder::custom(GridMode::SpotGrid, levels.clone()).expect("custom grid should build");

    assert_eq!(grid.levels, levels);
    assert!(grid.lower_levels.is_empty());
    assert!(grid.upper_levels.is_empty());
}

#[test]
fn custom_grid_preserves_futures_short_levels_in_execution_order() {
    let levels = vec![decimal(95, 0), decimal(100, 0), decimal(1075, 1)];
    let grid = GridBuilder::custom(GridMode::FuturesShort, levels.clone())
        .expect("custom short grid should build");

    assert_eq!(grid.levels, levels);
    assert!(grid.lower_levels.is_empty());
    assert!(grid.upper_levels.is_empty());
}

#[test]
fn custom_grid_rejects_levels_with_wrong_mode_order() {
    let spot_result = GridBuilder::custom(
        GridMode::SpotGrid,
        vec![decimal(95, 0), decimal(100, 0), decimal(1075, 1)],
    );
    assert!(spot_result.is_err());

    let short_result = GridBuilder::custom(
        GridMode::FuturesShort,
        vec![decimal(1075, 1), decimal(100, 0), decimal(95, 0)],
    );
    assert!(short_result.is_err());
}

#[test]
fn ordinary_grid_start_executes_level_one_and_places_only_one_take_profit_plus_lower_entries() {
    let mut runtime = GridRuntime::new(runtime_config()).expect("spot long runtime should build");

    runtime.start().expect("ordinary runtime should start");

    let position = runtime
        .position()
        .expect("anchor level should fill on start");
    assert_eq!(position.entry_price, decimal(110, 0));
    assert_eq!(position.quantity, decimal(1, 0));

    let orders = runtime.ordinary_orders();
    assert_eq!(
        orders
            .iter()
            .filter(|order| order.kind() == RuntimeOrderKind::TakeProfit)
            .count(),
        1
    );
    let take_profit = orders
        .iter()
        .find(|order| order.level_index() == 0 && order.kind() == RuntimeOrderKind::TakeProfit)
        .expect("level zero take profit should exist");
    assert_eq!(take_profit.price(), decimal(11110, 2));
    assert_eq!(
        orders
            .iter()
            .filter(|order| order.kind() == RuntimeOrderKind::Entry)
            .map(|order| order.level_index())
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}

#[test]
fn ordinary_grid_creates_take_profit_only_after_the_corresponding_level_fills() {
    let mut runtime = GridRuntime::new(runtime_config()).expect("spot long runtime should build");

    runtime.start().expect("ordinary runtime should start");
    assert!(!runtime
        .ordinary_orders()
        .iter()
        .any(|order| { order.level_index() == 1 && order.kind() == RuntimeOrderKind::TakeProfit }));

    runtime
        .fill_ordinary_entry(1)
        .expect("replenishment entry should fill");

    let orders = runtime.ordinary_orders();
    let take_profit = orders
        .iter()
        .find(|order| order.level_index() == 1 && order.kind() == RuntimeOrderKind::TakeProfit)
        .expect("filled replenishment level should create take profit");
    assert_eq!(take_profit.price(), decimal(101, 0));
    assert!(orders
        .iter()
        .any(|order| order.level_index() == 2 && order.kind() == RuntimeOrderKind::Entry));
}

#[test]
fn ordinary_short_grid_take_profit_uses_fill_price_and_points_down() {
    let mut runtime = GridRuntime::new(short_runtime_config()).expect("short runtime should build");

    runtime.start().expect("short runtime should start");

    let take_profit = runtime
        .ordinary_orders()
        .iter()
        .find(|order| order.level_index() == 0 && order.kind() == RuntimeOrderKind::TakeProfit)
        .expect("short anchor take profit should exist");
    assert_eq!(take_profit.price(), decimal(891, 1));
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

    let rebuilt = GridBuilder::ordinary_fixed_step(GridMode::SpotGrid, decimal(100, 0), 1000, 3)
        .expect("rebuilt grid should build");
    runtime
        .rebuild(rebuilt)
        .expect("supported rebuild should succeed");

    assert_eq!(runtime.status(), RuntimeStatus::Running);
    assert_eq!(
        runtime.grid().levels,
        vec![decimal(100, 0), decimal(90, 0), decimal(80, 0)]
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
        mode: GridMode::SpotGrid,
        plan,
        quantity: decimal(1, 0),
        ordinary_take_profit_bps: 100,
        maker_take_profit: None,
        trailing_take_profit: None,
        overall_take_profit: None,
        overall_stop_loss: None,
    };

    let result = GridRuntime::new(config);

    assert!(result.is_err());
}

#[test]
fn runtime_accepts_built_plans_for_all_task_two_modes() {
    let plans = vec![
        GridBuilder::ordinary_fixed_step(GridMode::SpotGrid, decimal(70000, 0), 100, 3)
            .expect("spot grid should build"),
        GridBuilder::ordinary_fixed_step(GridMode::FuturesLong, decimal(70000, 0), 100, 3)
            .expect("futures long grid should build"),
        GridBuilder::ordinary_fixed_step(GridMode::FuturesShort, decimal(70000, 0), 100, 3)
            .expect("futures short grid should build"),
        GridBuilder::classic_bilateral_fixed(
            GridMode::ClassicBilateralSpot,
            decimal(70000, 0),
            100,
            2,
        )
        .expect("spot bilateral grid should build"),
        GridBuilder::classic_bilateral_fixed(
            GridMode::ClassicBilateralFutures,
            decimal(70000, 0),
            100,
            2,
        )
        .expect("futures bilateral grid should build"),
    ];

    for plan in plans {
        let config = GridRuntimeConfig {
            mode: plan.mode,
            plan,
            quantity: decimal(1, 0),
            ordinary_take_profit_bps: 100,
            maker_take_profit: None,
            trailing_take_profit: None,
            overall_take_profit: None,
            overall_stop_loss: None,
        };

        let runtime = GridRuntime::new(config);
        assert!(runtime.is_ok(), "built plan should be accepted by runtime");
    }
}

#[test]
fn runtime_rejects_mode_matched_plan_with_invalid_shape() {
    let config = GridRuntimeConfig {
        mode: GridMode::ClassicBilateralSpot,
        plan: trading_engine::grid_builder::GridPlan {
            mode: GridMode::ClassicBilateralSpot,
            levels: vec![decimal(90, 0), decimal(100, 0), decimal(110, 0)],
            lower_levels: Vec::new(),
            upper_levels: Vec::new(),
        },
        quantity: decimal(1, 0),
        ordinary_take_profit_bps: 100,
        maker_take_profit: None,
        trailing_take_profit: None,
        overall_take_profit: None,
        overall_stop_loss: None,
    };

    let result = GridRuntime::new(config);

    assert!(result.is_err());
}

#[test]
fn runtime_rejects_directly_constructed_ordinary_plan_with_non_positive_level() {
    let config = GridRuntimeConfig {
        mode: GridMode::SpotGrid,
        plan: trading_engine::grid_builder::GridPlan {
            mode: GridMode::SpotGrid,
            levels: vec![decimal(110, 0), decimal(100, 0), Decimal::ZERO],
            lower_levels: Vec::new(),
            upper_levels: Vec::new(),
        },
        quantity: decimal(1, 0),
        ordinary_take_profit_bps: 100,
        maker_take_profit: None,
        trailing_take_profit: None,
        overall_take_profit: None,
        overall_stop_loss: None,
    };

    let result = GridRuntime::new(config);

    assert!(result.is_err());
}

#[test]
fn bilateral_mode_cannot_use_ordinary_shaped_custom_plan() {
    let result = GridBuilder::custom(
        GridMode::ClassicBilateralSpot,
        vec![decimal(90, 0), decimal(100, 0), decimal(110, 0)],
    );

    assert!(result.is_err());
}

#[test]
fn ordinary_fixed_step_rejects_non_positive_generated_levels() {
    let result = GridBuilder::ordinary_fixed_step(GridMode::SpotGrid, decimal(100, 0), 6000, 3);

    assert!(result.is_err());
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
