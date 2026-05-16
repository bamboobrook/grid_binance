use rust_decimal::Decimal;
use shared_domain::strategy::{
    GridGeneration, GridLevel, PostTriggerAction, ReferencePriceSource, StrategyAmountMode,
    StrategyMarket, StrategyMode, StrategyRevision, StrategyType,
};
use trading_engine::strategy_runtime::StrategyRuntimeEngine;

fn decimal(value: i64, scale: u32) -> Decimal {
    Decimal::new(value, scale)
}

fn rebuild_revision() -> StrategyRevision {
    StrategyRevision {
        revision_id: "revision-9".to_string(),
        version: 9,
        strategy_type: StrategyType::OrdinaryGrid,
        generation: GridGeneration::Arithmetic,
        amount_mode: StrategyAmountMode::Quote,
        futures_margin_mode: None,
        leverage: None,
        reference_price_source: ReferencePriceSource::Manual,
        reference_price: None,
        levels: vec![
            GridLevel {
                level_index: 0,
                entry_price: decimal(100, 0),
                quantity: decimal(1, 0),
                take_profit_bps: 500,
                trailing_bps: None,
            },
            GridLevel {
                level_index: 1,
                entry_price: decimal(105, 0),
                quantity: decimal(1, 0),
                take_profit_bps: 500,
                trailing_bps: None,
            },
        ],
        overall_take_profit_bps: Some(1000),
        overall_stop_loss_bps: Some(300),
        post_trigger_action: PostTriggerAction::Rebuild,
    }
}

#[test]
fn overall_take_profit_emits_rebuild_signal_without_local_flattening() {
    let mut engine = StrategyRuntimeEngine::new(
        "strategy-10",
        StrategyMarket::Spot,
        StrategyMode::SpotClassic,
        rebuild_revision(),
    )
    .expect("runtime should build");

    engine.start().expect("runtime should start");
    engine.fill_entry(0).expect("entry fill should succeed");

    let events = engine.on_price(decimal(110, 0)).expect("price update");
    let runtime = engine.snapshot();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "overall_take_profit_rebuild");
    assert_eq!(runtime.positions.len(), 1);
    assert_eq!(runtime.fills.len(), 1);
    assert!(runtime.orders.iter().any(|order| order.status == "Working"));
}

#[test]
fn pause_resume_rebuild_preserves_holdings_and_recreates_orders() {
    let mut engine = StrategyRuntimeEngine::new(
        "strategy-11",
        StrategyMarket::Spot,
        StrategyMode::SpotClassic,
        rebuild_revision(),
    )
    .expect("runtime should build");

    engine.start().expect("runtime should start");
    engine.fill_entry(0).expect("entry fill should succeed");
    engine.pause();

    let paused = engine.snapshot().clone();
    assert_eq!(paused.positions.len(), 1);
    assert!(paused
        .orders
        .iter()
        .any(|order| order.status == "Filled" && order.order_id == "strategy-11-order-0"));
    assert!(
        paused
            .orders
            .iter()
            .filter(|order| order.status == "Canceled")
            .count()
            >= 1
    );

    engine.resume().expect("resume should succeed");
    let resumed = engine.snapshot();

    assert_eq!(resumed.positions.len(), 1);
    assert_eq!(resumed.orders.len(), 2);
    assert!(resumed
        .orders
        .iter()
        .all(|order| matches!(order.status.as_str(), "Working" | "Monitoring" | "Armed")));
    assert_eq!(
        resumed.events.last().expect("resume event").event_type,
        "strategy_resumed"
    );
}

#[test]
fn spot_classic_start_splits_orders_above_and_below_midpoint() {
    let mut engine = StrategyRuntimeEngine::new(
        "strategy-spot-classic",
        StrategyMarket::Spot,
        StrategyMode::SpotClassic,
        StrategyRevision {
            revision_id: "revision-spot-classic".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
            levels: vec![
                GridLevel {
                    level_index: 0,
                    entry_price: decimal(90, 0),
                    quantity: decimal(1, 0),
                    take_profit_bps: 500,
                    trailing_bps: None,
                },
                GridLevel {
                    level_index: 1,
                    entry_price: decimal(100, 0),
                    quantity: decimal(1, 0),
                    take_profit_bps: 500,
                    trailing_bps: None,
                },
                GridLevel {
                    level_index: 2,
                    entry_price: decimal(110, 0),
                    quantity: decimal(1, 0),
                    take_profit_bps: 500,
                    trailing_bps: None,
                },
            ],
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        },
    )
    .expect("runtime should build");

    engine.start().expect("runtime should start");
    let runtime = engine.snapshot();

    assert_eq!(runtime.orders.len(), 2);
    assert_eq!(runtime.orders[0].side, "Buy");
    assert_eq!(runtime.orders[0].price, Some(decimal(90, 0)));
    assert_eq!(runtime.orders[1].side, "Sell");
    assert_eq!(runtime.orders[1].price, Some(decimal(110, 0)));
}

#[test]
fn entry_fill_creates_maker_take_profit_order_for_non_trailing_level() {
    let mut engine = StrategyRuntimeEngine::new(
        "strategy-maker-tp",
        StrategyMarket::Spot,
        StrategyMode::SpotClassic,
        StrategyRevision {
            revision_id: "revision-maker-tp".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
            levels: vec![
                GridLevel {
                    level_index: 0,
                    entry_price: decimal(95, 0),
                    quantity: decimal(1, 0),
                    take_profit_bps: 500,
                    trailing_bps: None,
                },
                GridLevel {
                    level_index: 1,
                    entry_price: decimal(105, 0),
                    quantity: decimal(1, 0),
                    take_profit_bps: 500,
                    trailing_bps: None,
                },
            ],
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        },
    )
    .expect("runtime should build");

    engine.start().expect("runtime should start");
    engine.fill_entry(0).expect("entry fill should succeed");
    let runtime = engine.snapshot();

    assert!(runtime.orders.iter().any(|order| {
        order.order_id == "strategy-maker-tp-tp-0"
            && order.side == "Sell"
            && order.status == "Working"
            && order.price == Some(decimal(9975, 2))
    }));
}

#[test]
fn futures_short_runtime_uses_short_side_and_short_profit_formula() {
    let mut engine = StrategyRuntimeEngine::new(
        "strategy-12",
        StrategyMarket::FuturesUsdM,
        StrategyMode::FuturesShort,
        StrategyRevision {
            revision_id: "revision-short".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
            levels: vec![GridLevel {
                level_index: 0,
                entry_price: decimal(100, 0),
                quantity: decimal(1, 0),
                take_profit_bps: 500,
                trailing_bps: None,
            }],
            overall_take_profit_bps: None,
            overall_stop_loss_bps: Some(500),
            post_trigger_action: PostTriggerAction::Stop,
        },
    )
    .expect("runtime should build");

    engine.start().expect("runtime should start");
    assert_eq!(engine.snapshot().orders[0].side, "Sell");
    engine.fill_entry(0).expect("entry fill should succeed");
    assert_eq!(
        engine.snapshot().positions[0].market,
        shared_domain::strategy::StrategyMarket::FuturesUsdM
    );

    let runtime = engine.snapshot();

    assert!(runtime.orders.iter().any(|order| {
        order.order_id == "strategy-12-tp-0"
            && order.side == "Buy"
            && order.status == "Working"
            && order.price == Some(decimal(95, 0))
    }));
    assert_eq!(runtime.positions.len(), 1);
    // Note: fill_entry creates 2 fills - one for the entry and one for fee accounting
    assert_eq!(runtime.fills.len(), 2);
}

#[test]
fn futures_coinm_runtime_preserves_market_type() {
    let mut engine = StrategyRuntimeEngine::new(
        "strategy-13",
        StrategyMarket::FuturesCoinM,
        StrategyMode::FuturesLong,
        StrategyRevision {
            revision_id: "revision-coinm".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
            levels: vec![GridLevel {
                level_index: 0,
                entry_price: decimal(100, 0),
                quantity: decimal(1, 0),
                take_profit_bps: 500,
                trailing_bps: None,
            }],
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        },
    )
    .expect("runtime should build");

    engine.start().expect("runtime should start");
    engine.fill_entry(0).expect("entry fill should succeed");

    assert_eq!(
        engine.snapshot().positions[0].market,
        StrategyMarket::FuturesCoinM
    );
}

#[test]
fn futures_neutral_runtime_keeps_both_sides_and_skips_overall_tp_when_hedged() {
    let mut engine = StrategyRuntimeEngine::new(
        "strategy-14",
        StrategyMarket::FuturesUsdM,
        StrategyMode::FuturesNeutral,
        StrategyRevision {
            revision_id: "revision-neutral".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
            levels: vec![
                GridLevel {
                    level_index: 0,
                    entry_price: decimal(100, 0),
                    quantity: decimal(1, 0),
                    take_profit_bps: 500,
                    trailing_bps: None,
                },
                GridLevel {
                    level_index: 1,
                    entry_price: decimal(100, 0),
                    quantity: decimal(1, 0),
                    take_profit_bps: 500,
                    trailing_bps: None,
                },
            ],
            overall_take_profit_bps: Some(200),
            overall_stop_loss_bps: Some(200),
            post_trigger_action: PostTriggerAction::Stop,
        },
    )
    .expect("runtime should build");

    engine.start().expect("runtime should start");
    assert_eq!(engine.snapshot().orders[0].side, "Buy");
    assert_eq!(engine.snapshot().orders[1].side, "Sell");

    engine.fill_entry(0).expect("long leg");
    engine.fill_entry(1).expect("short leg");

    assert_eq!(engine.snapshot().positions.len(), 2);
    assert!(engine
        .on_price(decimal(102, 0))
        .expect("price update")
        .is_empty());
    assert!(engine
        .on_price(decimal(98, 0))
        .expect("price update")
        .is_empty());
}

#[test]
fn futures_neutral_overall_take_profit_uses_combined_unrealized_pnl() {
    let mut engine = StrategyRuntimeEngine::new(
        "strategy-15",
        StrategyMarket::FuturesUsdM,
        StrategyMode::FuturesNeutral,
        StrategyRevision {
            revision_id: "revision-neutral-overall".to_string(),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
            levels: vec![
                GridLevel {
                    level_index: 0,
                    entry_price: decimal(95, 0),
                    quantity: decimal(1, 0),
                    take_profit_bps: 500,
                    trailing_bps: None,
                },
                GridLevel {
                    level_index: 1,
                    entry_price: decimal(105, 0),
                    quantity: decimal(1, 0),
                    take_profit_bps: 500,
                    trailing_bps: None,
                },
            ],
            overall_take_profit_bps: Some(300),
            overall_stop_loss_bps: Some(300),
            post_trigger_action: PostTriggerAction::Stop,
        },
    )
    .expect("runtime should build");

    engine.start().expect("runtime should start");
    engine.fill_entry(0).expect("long leg");
    engine.fill_entry(1).expect("short leg");

    let events = engine.on_price(decimal(100, 0)).expect("price update");

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "overall_take_profit_stop");
    assert!(!engine.is_running());
}
