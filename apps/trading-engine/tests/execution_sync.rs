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
