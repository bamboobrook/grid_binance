use rust_decimal::Decimal;
use shared_domain::strategy::{
    GridGeneration, GridLevel, PostTriggerAction, StrategyMode, StrategyRevision,
};
use trading_engine::strategy_runtime::StrategyRuntimeEngine;

fn decimal(value: i64, scale: u32) -> Decimal {
    Decimal::new(value, scale)
}

fn revision_with_trailing(trailing_bps: Option<u32>) -> StrategyRevision {
    StrategyRevision {
        revision_id: "revision-1".to_string(),
        version: 1,
        generation: GridGeneration::Custom,
        levels: vec![GridLevel {
            level_index: 0,
            entry_price: decimal(100, 0),
            quantity: decimal(1, 0),
            take_profit_bps: 1000,
            trailing_bps,
        }],
        overall_take_profit_bps: None,
        overall_stop_loss_bps: None,
        post_trigger_action: PostTriggerAction::Stop,
    }
}

#[test]
fn trailing_take_profit_rejects_retracement_wider_than_grid_take_profit() {
    let result = StrategyRuntimeEngine::new(
        "strategy-1",
        StrategyMode::SpotClassic,
        revision_with_trailing(Some(1200)),
    );

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "level 0 trailing_bps must be less than or equal to take_profit_bps"
    );
}

#[test]
fn trailing_take_profit_uses_post_activation_high_and_taker_close() {
    let mut engine = StrategyRuntimeEngine::new(
        "strategy-2",
        StrategyMode::SpotClassic,
        revision_with_trailing(Some(500)),
    )
    .expect("runtime should build");

    engine.start().expect("runtime should start");
    engine.fill_entry(0).expect("level fill should succeed");

    assert!(engine.on_price(decimal(109, 0)).expect("price update").is_empty());
    assert!(engine.on_price(decimal(110, 0)).expect("price update").is_empty());
    assert!(engine.on_price(decimal(120, 0)).expect("price update").is_empty());

    let events = engine.on_price(decimal(114, 0)).expect("price update");
    let runtime = engine.snapshot();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "taker_trailing_take_profit");
    assert_eq!(events[0].price, Some(decimal(114, 0)));
    assert_eq!(runtime.fills.len(), 2);
    assert_eq!(runtime.positions.len(), 0);
    assert_eq!(
        runtime
            .events
            .last()
            .expect("final event")
            .event_type,
        "taker_trailing_take_profit"
    );
}
