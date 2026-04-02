use rust_decimal::Decimal;
use shared_domain::strategy::{
    GridGeneration, GridLevel, PostTriggerAction, StrategyMode, StrategyRevision,
};
use trading_engine::strategy_runtime::StrategyRuntimeEngine;

fn decimal(value: i64, scale: u32) -> Decimal {
    Decimal::new(value, scale)
}

fn rebuild_revision() -> StrategyRevision {
    StrategyRevision {
        revision_id: "revision-9".to_string(),
        version: 9,
        generation: GridGeneration::Arithmetic,
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
fn overall_take_profit_can_rebuild_and_continue_new_cycle() {
    let mut engine = StrategyRuntimeEngine::new(
        "strategy-10",
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
    assert_eq!(runtime.positions.len(), 0);
    assert_eq!(runtime.orders.len(), 2);
    assert!(runtime.orders.iter().all(|order| order.status == "Working"));
}

#[test]
fn pause_resume_rebuild_preserves_holdings_and_recreates_orders() {
    let mut engine = StrategyRuntimeEngine::new(
        "strategy-11",
        StrategyMode::SpotClassic,
        rebuild_revision(),
    )
    .expect("runtime should build");

    engine.start().expect("runtime should start");
    engine.fill_entry(0).expect("entry fill should succeed");
    engine.pause();

    let paused = engine.snapshot().clone();
    assert_eq!(paused.positions.len(), 1);
    assert_eq!(
        paused
            .orders
            .iter()
            .filter(|order| order.status == "Filled")
            .count(),
        1
    );
    assert_eq!(
        paused
            .orders
            .iter()
            .filter(|order| order.status == "Canceled")
            .count(),
        1
    );

    engine.resume().expect("resume should succeed");
    let resumed = engine.snapshot();

    assert_eq!(resumed.positions.len(), 1);
    assert_eq!(resumed.orders.len(), 2);
    assert!(resumed.orders.iter().all(|order| order.status == "Working"));
    assert_eq!(
        resumed
            .events
            .last()
            .expect("resume event")
            .event_type,
        "strategy_resumed"
    );
}
