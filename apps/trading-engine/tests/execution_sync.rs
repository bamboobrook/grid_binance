use rust_decimal::Decimal;
use shared_binance::BinanceExecutionUpdate;
use shared_domain::strategy::{
    GridGeneration, GridLevel, PostTriggerAction, Strategy, StrategyAmountMode, StrategyMarket,
    StrategyMode, StrategyRevision, StrategyRuntime, StrategyRuntimeOrder, StrategyStatus,
};
use trading_engine::execution_sync::apply_execution_update;

#[test]
fn execution_update_marks_order_canceled_and_records_event() {
    let mut strategy = sample_strategy();
    strategy.runtime.orders[0].status = "Placed".to_string();
    strategy.runtime.orders[0].exchange_order_id = Some("555".to_string());

    let changed = apply_execution_update(
        &mut strategy,
        &BinanceExecutionUpdate {
            market: "spot".to_string(),
            symbol: "BTCUSDT".to_string(),
            order_id: "555".to_string(),
            client_order_id: Some("strategy-1-order-0".to_string()),
            side: Some("BUY".to_string()),
            order_type: Some("LIMIT".to_string()),
            status: "CANCELED".to_string(),
            execution_type: Some("CANCELED".to_string()),
            order_price: Some("42000".to_string()),
            last_fill_price: None,
            last_fill_quantity: None,
            cumulative_fill_quantity: None,
            fee_amount: None,
            fee_asset: None,
            position_side: None,
            trade_id: None,
            realized_profit: None,
            event_time_ms: 1_710_000,
        },
    );

    assert!(changed);
    assert_eq!(strategy.runtime.orders[0].status, "Canceled");
    assert_eq!(
        strategy.runtime.events.last().unwrap().event_type,
        "execution_update_received"
    );
}

#[test]
fn trade_execution_update_appends_runtime_fill_with_fee() {
    let mut strategy = sample_strategy();

    let changed = apply_execution_update(
        &mut strategy,
        &BinanceExecutionUpdate {
            market: "spot".to_string(),
            symbol: "BTCUSDT".to_string(),
            order_id: "999".to_string(),
            client_order_id: Some("strategy-1-order-0".to_string()),
            side: Some("BUY".to_string()),
            order_type: Some("LIMIT".to_string()),
            status: "FILLED".to_string(),
            execution_type: Some("TRADE".to_string()),
            order_price: Some("42000".to_string()),
            last_fill_price: Some("42000".to_string()),
            last_fill_quantity: Some("0.001".to_string()),
            cumulative_fill_quantity: Some("0.001".to_string()),
            fee_amount: Some("0.05".to_string()),
            fee_asset: Some("USDT".to_string()),
            position_side: None,
            trade_id: None,
            realized_profit: None,
            event_time_ms: 1_710_001,
        },
    );

    assert!(changed);
    assert_eq!(strategy.runtime.fills.len(), 1);
    assert_eq!(strategy.runtime.fills[0].fill_type, "ExecutionUpdateFill");
    assert_eq!(strategy.runtime.fills[0].fee_asset.as_deref(), Some("USDT"));
}

#[test]
fn futures_trade_execution_uses_trade_id_and_realized_profit() {
    let mut strategy = sample_strategy();

    let changed = apply_execution_update(
        &mut strategy,
        &BinanceExecutionUpdate {
            market: "usdm".to_string(),
            symbol: "BTCUSDT".to_string(),
            order_id: "999".to_string(),
            client_order_id: Some("strategy-1-order-0".to_string()),
            side: Some("SELL".to_string()),
            order_type: Some("LIMIT".to_string()),
            status: "FILLED".to_string(),
            execution_type: Some("TRADE".to_string()),
            order_price: Some("43000".to_string()),
            last_fill_price: Some("43000".to_string()),
            last_fill_quantity: Some("0.001".to_string()),
            cumulative_fill_quantity: Some("0.001".to_string()),
            fee_amount: Some("0.04".to_string()),
            fee_asset: Some("USDT".to_string()),
            position_side: Some("SHORT".to_string()),
            trade_id: Some("321".to_string()),
            realized_profit: Some("1.25".to_string()),
            event_time_ms: 1_710_002,
        },
    );

    assert!(changed);
    assert_eq!(strategy.runtime.fills[0].fill_id, "exchange-trade-321");
    assert_eq!(
        strategy.runtime.fills[0].realized_pnl,
        Some(Decimal::new(125, 2))
    );
}

#[test]
fn execution_update_matches_client_order_id_when_exchange_id_missing() {
    let mut strategy = sample_strategy();

    let changed = apply_execution_update(
        &mut strategy,
        &BinanceExecutionUpdate {
            market: "spot".to_string(),
            symbol: "BTCUSDT".to_string(),
            order_id: "999".to_string(),
            client_order_id: Some("strategy-1-order-0".to_string()),
            side: Some("BUY".to_string()),
            order_type: Some("LIMIT".to_string()),
            status: "FILLED".to_string(),
            execution_type: Some("TRADE".to_string()),
            order_price: Some("42000".to_string()),
            last_fill_price: Some("42000".to_string()),
            last_fill_quantity: Some("0.001".to_string()),
            cumulative_fill_quantity: Some("0.001".to_string()),
            fee_amount: Some("0.05".to_string()),
            fee_asset: Some("USDT".to_string()),
            position_side: None,
            trade_id: None,
            realized_profit: None,
            event_time_ms: 1_710_001,
        },
    );

    assert!(changed);
    assert_eq!(strategy.runtime.orders[0].status, "Filled");
    assert_eq!(
        strategy.runtime.orders[0].exchange_order_id.as_deref(),
        Some("999")
    );
}

#[test]
fn entry_fill_creates_take_profit_order_and_runtime_position() {
    let mut strategy = sample_strategy();

    let changed = apply_execution_update(
        &mut strategy,
        &BinanceExecutionUpdate {
            market: "spot".to_string(),
            symbol: "BTCUSDT".to_string(),
            order_id: "999".to_string(),
            client_order_id: Some("strategy-1-order-0".to_string()),
            side: Some("BUY".to_string()),
            order_type: Some("LIMIT".to_string()),
            status: "FILLED".to_string(),
            execution_type: Some("TRADE".to_string()),
            order_price: Some("42000".to_string()),
            last_fill_price: Some("42000".to_string()),
            last_fill_quantity: Some("0.001".to_string()),
            cumulative_fill_quantity: Some("0.001".to_string()),
            fee_amount: Some("0.05".to_string()),
            fee_asset: Some("USDT".to_string()),
            position_side: None,
            trade_id: Some("entry-1".to_string()),
            realized_profit: None,
            event_time_ms: 1_710_010,
        },
    );

    assert!(changed);
    assert_eq!(strategy.runtime.positions.len(), 1);
    let take_profit = strategy
        .runtime
        .orders
        .iter()
        .find(|order| order.order_id.contains("-tp-"))
        .expect("take profit order should be created after entry fill");
    assert_eq!(take_profit.side, "Sell");
    assert_eq!(take_profit.status, "Working");
}

#[test]
fn partial_entry_fill_updates_runtime_position_and_exit_order_incrementally() {
    let mut strategy = sample_strategy();

    let changed = apply_execution_update(
        &mut strategy,
        &BinanceExecutionUpdate {
            market: "spot".to_string(),
            symbol: "BTCUSDT".to_string(),
            order_id: "999".to_string(),
            client_order_id: Some("strategy-1-order-0".to_string()),
            side: Some("BUY".to_string()),
            order_type: Some("LIMIT".to_string()),
            status: "PARTIALLY_FILLED".to_string(),
            execution_type: Some("TRADE".to_string()),
            order_price: Some("42000".to_string()),
            last_fill_price: Some("42000".to_string()),
            last_fill_quantity: Some("0.004".to_string()),
            cumulative_fill_quantity: Some("0.004".to_string()),
            fee_amount: Some("0.01".to_string()),
            fee_asset: Some("USDT".to_string()),
            position_side: None,
            trade_id: Some("partial-entry-1".to_string()),
            realized_profit: None,
            event_time_ms: 1_710_015,
        },
    );

    assert!(changed);
    assert_eq!(strategy.runtime.positions.len(), 1);
    assert_eq!(strategy.runtime.positions[0].quantity, Decimal::new(4, 3));
    let take_profit = strategy
        .runtime
        .orders
        .iter()
        .find(|order| order.order_id == "strategy-1-tp-0")
        .expect("take profit order should exist after partial entry");
    assert_eq!(take_profit.quantity, Decimal::new(4, 3));
    assert_eq!(take_profit.status, "Working");
}

#[test]
fn take_profit_fill_recreates_entry_order_for_next_grid_cycle() {
    let mut strategy = sample_strategy();
    strategy.runtime.positions.push(shared_domain::strategy::StrategyRuntimePosition {
        market: StrategyMarket::Spot,
        mode: StrategyMode::SpotClassic,
        quantity: Decimal::new(1, 3),
        average_entry_price: Decimal::new(42000, 0),
    });
    strategy.runtime.orders.push(StrategyRuntimeOrder {
        order_id: "strategy-1-tp-0".to_string(),
        exchange_order_id: Some("tp-999".to_string()),
        level_index: Some(0),
        side: "Sell".to_string(),
        order_type: "Limit".to_string(),
        price: Some(Decimal::new(42420, 0)),
        quantity: Decimal::new(1, 3),
        status: "Placed".to_string(),
    });

    let changed = apply_execution_update(
        &mut strategy,
        &BinanceExecutionUpdate {
            market: "spot".to_string(),
            symbol: "BTCUSDT".to_string(),
            order_id: "tp-999".to_string(),
            client_order_id: Some("strategy-1-tp-0".to_string()),
            side: Some("SELL".to_string()),
            order_type: Some("LIMIT".to_string()),
            status: "FILLED".to_string(),
            execution_type: Some("TRADE".to_string()),
            order_price: Some("42420".to_string()),
            last_fill_price: Some("42420".to_string()),
            last_fill_quantity: Some("0.001".to_string()),
            cumulative_fill_quantity: Some("0.001".to_string()),
            fee_amount: Some("0.05".to_string()),
            fee_asset: Some("USDT".to_string()),
            position_side: None,
            trade_id: Some("tp-1".to_string()),
            realized_profit: Some("0.42".to_string()),
            event_time_ms: 1_710_011,
        },
    );

    assert!(changed);
    assert!(strategy.runtime.positions.is_empty());
    assert!(strategy
        .runtime
        .orders
        .iter()
        .any(|order| order.order_id == "strategy-1-order-0" && order.side == "Buy" && order.status == "Working"));
}

#[test]
fn take_profit_fill_is_attributed_by_level_instead_of_quantity_only() {
    let mut strategy = Strategy {
        id: "strategy-1".to_string(),
        owner_email: "trader@example.com".to_string(),
        name: "Grid".to_string(),
        symbol: "BTCUSDT".to_string(),
        budget: "1000".to_string(),
        grid_spacing_bps: 100,
        status: StrategyStatus::Running,
        source_template_id: None,
        membership_ready: true,
        exchange_ready: true,
        permissions_ready: true,
        withdrawals_disabled: true,
        hedge_mode_ready: true,
        symbol_ready: true,
        filters_ready: true,
        margin_ready: true,
        conflict_ready: true,
        balance_ready: true,
        market: StrategyMarket::Spot,
        mode: StrategyMode::SpotClassic,
        draft_revision: StrategyRevision {
            revision_id: "rev-2".to_string(),
            version: 2,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            levels: vec![
                GridLevel {
                    level_index: 0,
                    entry_price: Decimal::new(100, 0),
                    quantity: Decimal::new(1, 0),
                    take_profit_bps: 100,
                    trailing_bps: None,
                },
                GridLevel {
                    level_index: 1,
                    entry_price: Decimal::new(120, 0),
                    quantity: Decimal::new(1, 0),
                    take_profit_bps: 100,
                    trailing_bps: None,
                },
            ],
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        },
        active_revision: None,
        runtime: StrategyRuntime {
            positions: vec![shared_domain::strategy::StrategyRuntimePosition {
                market: StrategyMarket::Spot,
                mode: StrategyMode::SpotClassic,
                quantity: Decimal::new(2, 0),
                average_entry_price: Decimal::new(110, 0),
            }],
            orders: vec![
                StrategyRuntimeOrder {
                    order_id: "strategy-1-order-0".to_string(),
                    exchange_order_id: Some("entry-0".to_string()),
                    level_index: Some(0),
                    side: "Buy".to_string(),
                    order_type: "Limit".to_string(),
                    price: Some(Decimal::new(100, 0)),
                    quantity: Decimal::new(1, 0),
                    status: "Filled".to_string(),
                },
                StrategyRuntimeOrder {
                    order_id: "strategy-1-order-1".to_string(),
                    exchange_order_id: Some("entry-1".to_string()),
                    level_index: Some(1),
                    side: "Buy".to_string(),
                    order_type: "Limit".to_string(),
                    price: Some(Decimal::new(120, 0)),
                    quantity: Decimal::new(1, 0),
                    status: "Filled".to_string(),
                },
                StrategyRuntimeOrder {
                    order_id: "strategy-1-tp-0".to_string(),
                    exchange_order_id: Some("tp-0".to_string()),
                    level_index: Some(0),
                    side: "Sell".to_string(),
                    order_type: "Limit".to_string(),
                    price: Some(Decimal::new(101, 0)),
                    quantity: Decimal::new(1, 0),
                    status: "Placed".to_string(),
                },
                StrategyRuntimeOrder {
                    order_id: "strategy-1-tp-1".to_string(),
                    exchange_order_id: Some("tp-1".to_string()),
                    level_index: Some(1),
                    side: "Sell".to_string(),
                    order_type: "Limit".to_string(),
                    price: Some(Decimal::new(121, 0)),
                    quantity: Decimal::new(1, 0),
                    status: "Placed".to_string(),
                },
            ],
            fills: vec![
                shared_domain::strategy::StrategyRuntimeFill {
                    fill_id: "entry-fill-0".to_string(),
                    order_id: Some("strategy-1-order-0".to_string()),
                    level_index: Some(0),
                    fill_type: "ExecutionUpdateFill".to_string(),
                    price: Decimal::new(100, 0),
                    quantity: Decimal::new(1, 0),
                    realized_pnl: None,
                    fee_amount: None,
                    fee_asset: None,
                },
                shared_domain::strategy::StrategyRuntimeFill {
                    fill_id: "entry-fill-1".to_string(),
                    order_id: Some("strategy-1-order-1".to_string()),
                    level_index: Some(1),
                    fill_type: "ExecutionUpdateFill".to_string(),
                    price: Decimal::new(120, 0),
                    quantity: Decimal::new(1, 0),
                    realized_pnl: None,
                    fee_amount: None,
                    fee_asset: None,
                },
            ],
            events: Vec::new(),
            last_preflight: None,
        },
        archived_at: None,
    };

    let changed = apply_execution_update(
        &mut strategy,
        &BinanceExecutionUpdate {
            market: "spot".to_string(),
            symbol: "BTCUSDT".to_string(),
            order_id: "tp-1".to_string(),
            client_order_id: Some("strategy-1-tp-1".to_string()),
            side: Some("SELL".to_string()),
            order_type: Some("LIMIT".to_string()),
            status: "FILLED".to_string(),
            execution_type: Some("TRADE".to_string()),
            order_price: Some("121".to_string()),
            last_fill_price: Some("121".to_string()),
            last_fill_quantity: Some("1".to_string()),
            cumulative_fill_quantity: Some("1".to_string()),
            fee_amount: Some("0.05".to_string()),
            fee_asset: Some("USDT".to_string()),
            position_side: None,
            trade_id: Some("tp-level-1".to_string()),
            realized_profit: Some("1".to_string()),
            event_time_ms: 1_710_030,
        },
    );

    assert!(changed);
    assert_eq!(strategy.runtime.positions.len(), 1);
    assert_eq!(strategy.runtime.positions[0].quantity, Decimal::new(1, 0));
    assert_eq!(
        strategy.runtime.positions[0].average_entry_price,
        Decimal::new(100, 0)
    );
    assert!(strategy.runtime.orders.iter().any(|order| {
        order.order_id == "strategy-1-order-1"
            && order.side == "Buy"
            && order.status == "Working"
    }));
}

#[test]
fn partial_stop_close_fill_reduces_position_without_leaving_stale_quantity() {
    let mut strategy = sample_strategy();
    strategy.status = StrategyStatus::Stopping;
    strategy.runtime.positions.push(shared_domain::strategy::StrategyRuntimePosition {
        market: StrategyMarket::Spot,
        mode: StrategyMode::SpotClassic,
        quantity: Decimal::new(10, 3),
        average_entry_price: Decimal::new(42000, 0),
    });
    strategy.runtime.orders.clear();
    strategy.runtime.orders.push(StrategyRuntimeOrder {
        order_id: "strategy-1-stop-close-0".to_string(),
        exchange_order_id: Some("close-999".to_string()),
        level_index: None,
        side: "Sell".to_string(),
        order_type: "Market".to_string(),
        price: None,
        quantity: Decimal::new(10, 3),
        status: "Placed".to_string(),
    });

    let changed = apply_execution_update(
        &mut strategy,
        &BinanceExecutionUpdate {
            market: "spot".to_string(),
            symbol: "BTCUSDT".to_string(),
            order_id: "close-999".to_string(),
            client_order_id: Some("strategy-1-stop-close-0".to_string()),
            side: Some("SELL".to_string()),
            order_type: Some("MARKET".to_string()),
            status: "PARTIALLY_FILLED".to_string(),
            execution_type: Some("TRADE".to_string()),
            order_price: None,
            last_fill_price: Some("42500".to_string()),
            last_fill_quantity: Some("0.004".to_string()),
            cumulative_fill_quantity: Some("0.004".to_string()),
            fee_amount: Some("0.01".to_string()),
            fee_asset: Some("USDT".to_string()),
            position_side: None,
            trade_id: Some("stop-close-partial".to_string()),
            realized_profit: None,
            event_time_ms: 1_710_040,
        },
    );

    assert!(changed);
    assert_eq!(strategy.runtime.orders[0].status, "PartiallyFilled");
    assert_eq!(strategy.runtime.positions.len(), 1);
    assert_eq!(strategy.runtime.positions[0].quantity, Decimal::new(6, 3));
    assert_eq!(strategy.status, StrategyStatus::Stopping);
}

#[test]
fn recycled_entry_order_ignores_closed_previous_cycle_cost_basis() {
    let mut strategy = sample_strategy();
    strategy.runtime.orders[0].status = "Working".to_string();
    strategy.runtime.orders[0].price = Some(Decimal::new(42000, 0));
    strategy.runtime.orders.push(StrategyRuntimeOrder {
        order_id: "strategy-1-tp-0".to_string(),
        exchange_order_id: None,
        level_index: Some(0),
        side: "Sell".to_string(),
        order_type: "Limit".to_string(),
        price: Some(Decimal::new(42420, 0)),
        quantity: Decimal::new(1, 3),
        status: "Filled".to_string(),
    });
    strategy.runtime.fills = vec![
        shared_domain::strategy::StrategyRuntimeFill {
            fill_id: "entry-cycle-1".to_string(),
            order_id: Some("strategy-1-order-0".to_string()),
            level_index: Some(0),
            fill_type: "ExecutionUpdateFill".to_string(),
            price: Decimal::new(42000, 0),
            quantity: Decimal::new(1, 3),
            realized_pnl: None,
            fee_amount: None,
            fee_asset: None,
        },
        shared_domain::strategy::StrategyRuntimeFill {
            fill_id: "exit-cycle-1".to_string(),
            order_id: Some("strategy-1-tp-0".to_string()),
            level_index: Some(0),
            fill_type: "ExecutionUpdateFill".to_string(),
            price: Decimal::new(42420, 0),
            quantity: Decimal::new(1, 3),
            realized_pnl: Some(Decimal::new(42, 2)),
            fee_amount: None,
            fee_asset: None,
        },
    ];
    strategy.runtime.positions.clear();

    let changed = apply_execution_update(
        &mut strategy,
        &BinanceExecutionUpdate {
            market: "spot".to_string(),
            symbol: "BTCUSDT".to_string(),
            order_id: "entry-recycle".to_string(),
            client_order_id: Some("strategy-1-order-0".to_string()),
            side: Some("BUY".to_string()),
            order_type: Some("LIMIT".to_string()),
            status: "PARTIALLY_FILLED".to_string(),
            execution_type: Some("TRADE".to_string()),
            order_price: Some("43000".to_string()),
            last_fill_price: Some("43000".to_string()),
            last_fill_quantity: Some("0.001".to_string()),
            cumulative_fill_quantity: Some("0.001".to_string()),
            fee_amount: Some("0.01".to_string()),
            fee_asset: Some("USDT".to_string()),
            position_side: None,
            trade_id: Some("entry-cycle-2".to_string()),
            realized_profit: None,
            event_time_ms: 1_710_050,
        },
    );

    assert!(changed);
    assert_eq!(strategy.runtime.positions.len(), 1);
    assert_eq!(
        strategy.runtime.positions[0].average_entry_price,
        Decimal::new(43000, 0)
    );
    let take_profit = strategy
        .runtime
        .orders
        .iter()
        .find(|order| order.order_id == "strategy-1-tp-0")
        .expect("take profit order");
    assert_eq!(take_profit.price, Some(Decimal::new(43430, 0)));
}

#[test]
fn final_fill_uses_cumulative_quantity_for_runtime_position_and_take_profit_order() {
    let mut strategy = sample_strategy();

    let partial_changed = apply_execution_update(
        &mut strategy,
        &BinanceExecutionUpdate {
            market: "spot".to_string(),
            symbol: "BTCUSDT".to_string(),
            order_id: "999".to_string(),
            client_order_id: Some("strategy-1-order-0".to_string()),
            side: Some("BUY".to_string()),
            order_type: Some("LIMIT".to_string()),
            status: "PARTIALLY_FILLED".to_string(),
            execution_type: Some("TRADE".to_string()),
            order_price: Some("42000".to_string()),
            last_fill_price: Some("42000".to_string()),
            last_fill_quantity: Some("0.004".to_string()),
            cumulative_fill_quantity: Some("0.004".to_string()),
            fee_amount: Some("0.01".to_string()),
            fee_asset: Some("USDT".to_string()),
            position_side: None,
            trade_id: Some("partial-1".to_string()),
            realized_profit: None,
            event_time_ms: 1_710_020,
        },
    );

    assert!(partial_changed);
    assert_eq!(strategy.runtime.positions.len(), 1);
    assert_eq!(strategy.runtime.positions[0].quantity, Decimal::new(4, 3));

    let final_changed = apply_execution_update(
        &mut strategy,
        &BinanceExecutionUpdate {
            market: "spot".to_string(),
            symbol: "BTCUSDT".to_string(),
            order_id: "999".to_string(),
            client_order_id: Some("strategy-1-order-0".to_string()),
            side: Some("BUY".to_string()),
            order_type: Some("LIMIT".to_string()),
            status: "FILLED".to_string(),
            execution_type: Some("TRADE".to_string()),
            order_price: Some("42000".to_string()),
            last_fill_price: Some("42000".to_string()),
            last_fill_quantity: Some("0.006".to_string()),
            cumulative_fill_quantity: Some("0.010".to_string()),
            fee_amount: Some("0.02".to_string()),
            fee_asset: Some("USDT".to_string()),
            position_side: None,
            trade_id: Some("partial-2".to_string()),
            realized_profit: None,
            event_time_ms: 1_710_021,
        },
    );

    assert!(final_changed);
    assert_eq!(strategy.runtime.positions.len(), 1);
    assert_eq!(strategy.runtime.positions[0].quantity, Decimal::new(10, 3));
    let take_profit = strategy
        .runtime
        .orders
        .iter()
        .find(|order| order.order_id.contains("-tp-"))
        .expect("take profit order should be created after full fill");
    assert_eq!(take_profit.quantity, Decimal::new(10, 3));
}


fn sample_strategy() -> Strategy {
    Strategy {
        id: "strategy-1".to_string(),
        owner_email: "trader@example.com".to_string(),
        name: "Grid".to_string(),
        symbol: "BTCUSDT".to_string(),
        budget: "1000".to_string(),
        grid_spacing_bps: 100,
        status: StrategyStatus::Running,
        source_template_id: None,
        membership_ready: true,
        exchange_ready: true,
        permissions_ready: true,
        withdrawals_disabled: true,
        hedge_mode_ready: true,
        symbol_ready: true,
        filters_ready: true,
        margin_ready: true,
        conflict_ready: true,
        balance_ready: true,
        market: StrategyMarket::Spot,
        mode: StrategyMode::SpotClassic,
        draft_revision: StrategyRevision {
            revision_id: "rev-1".to_string(),
            version: 1,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            levels: vec![GridLevel {
                level_index: 0,
                entry_price: Decimal::new(42000, 0),
                quantity: Decimal::new(1, 3),
                take_profit_bps: 100,
                trailing_bps: None,
            }],
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        },
        active_revision: None,
        runtime: StrategyRuntime {
            positions: Vec::new(),
            orders: vec![StrategyRuntimeOrder {
                order_id: "strategy-1-order-0".to_string(),
                exchange_order_id: None,
                level_index: Some(0),
                side: "Buy".to_string(),
                order_type: "Limit".to_string(),
                price: Some(Decimal::new(42000, 0)),
                quantity: Decimal::new(1, 3),
                status: "Working".to_string(),
            }],
            fills: Vec::new(),
            events: Vec::new(),
            last_preflight: None,
        },
        archived_at: None,
    }
}
