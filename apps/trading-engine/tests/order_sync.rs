use rust_decimal::Decimal;
use shared_binance::{BinanceOrderRequest, BinanceOrderResponse};
use shared_domain::strategy::{
    GridGeneration, GridLevel, PostTriggerAction, ReferencePriceSource, RuntimeControls, Strategy,
    StrategyAmountMode, StrategyMarket, StrategyMode, StrategyRevision, StrategyRuntime,
    StrategyRuntimeOrder, StrategyRuntimePhase, StrategyRuntimePosition, StrategyStatus,
    StrategyType,
};
use trading_engine::order_sync::{
    sync_strategy_orders, BinanceOrderGateway, OrderQuantizationRules,
};

#[derive(Default)]
struct FakeGateway {
    placed: std::sync::Mutex<Vec<BinanceOrderRequest>>,
    canceled: std::sync::Mutex<Vec<(String, String, Option<String>, Option<String>)>>,
    filled_order_ids: std::sync::Mutex<std::collections::HashSet<String>>,
    remote_statuses: std::sync::Mutex<std::collections::HashMap<String, String>>,
    open_orders: std::sync::Mutex<Vec<BinanceOrderResponse>>,
    get_order_errors: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl BinanceOrderGateway for FakeGateway {
    fn place_order(&self, request: BinanceOrderRequest) -> Result<BinanceOrderResponse, String> {
        self.placed.lock().unwrap().push(request.clone());
        Ok(BinanceOrderResponse {
            market: request.market,
            symbol: request.symbol,
            order_id: "98765".to_string(),
            client_order_id: request.client_order_id,
            status: "NEW".to_string(),
            side: Some(request.side),
            order_type: Some(request.order_type),
            price: request.price,
            quantity: Some(request.quantity),
        })
    }

    fn cancel_order(
        &self,
        market: &str,
        symbol: &str,
        order_id: Option<&str>,
        client_order_id: Option<&str>,
    ) -> Result<BinanceOrderResponse, String> {
        self.canceled.lock().unwrap().push((
            market.to_string(),
            symbol.to_string(),
            order_id.map(ToOwned::to_owned),
            client_order_id.map(ToOwned::to_owned),
        ));
        let status = if client_order_id
            .is_some_and(|value| self.filled_order_ids.lock().unwrap().contains(value))
        {
            "FILLED"
        } else {
            "CANCELED"
        };
        Ok(BinanceOrderResponse {
            market: market.to_string(),
            symbol: symbol.to_string(),
            order_id: order_id.unwrap_or_default().to_string(),
            client_order_id: client_order_id.map(ToOwned::to_owned),
            status: status.to_string(),
            side: None,
            order_type: None,
            price: None,
            quantity: None,
        })
    }

    fn get_order(
        &self,
        market: &str,
        symbol: &str,
        order_id: Option<&str>,
        client_order_id: Option<&str>,
    ) -> Result<BinanceOrderResponse, String> {
        if let Some(error) = client_order_id
            .and_then(|value| self.get_order_errors.lock().unwrap().get(value).cloned())
            .or_else(|| {
                order_id.and_then(|value| self.get_order_errors.lock().unwrap().get(value).cloned())
            })
        {
            return Err(error);
        }
        let status = client_order_id
            .and_then(|value| self.remote_statuses.lock().unwrap().get(value).cloned())
            .or_else(|| {
                order_id.and_then(|value| self.remote_statuses.lock().unwrap().get(value).cloned())
            })
            .unwrap_or_else(|| {
                if client_order_id
                    .is_some_and(|value| self.filled_order_ids.lock().unwrap().contains(value))
                {
                    "FILLED".to_string()
                } else {
                    "CANCELED".to_string()
                }
            });
        Ok(BinanceOrderResponse {
            market: market.to_string(),
            symbol: symbol.to_string(),
            order_id: order_id.unwrap_or_default().to_string(),
            client_order_id: client_order_id.map(ToOwned::to_owned),
            status,
            side: None,
            order_type: None,
            price: None,
            quantity: None,
        })
    }

    fn open_orders(
        &self,
        _market: &str,
        _symbol: &str,
    ) -> Result<Vec<BinanceOrderResponse>, String> {
        Ok(self.open_orders.lock().unwrap().clone())
    }
}

#[test]
fn running_strategy_submits_missing_live_orders() {
    let gateway = FakeGateway::default();
    let mut strategy = sample_strategy(
        StrategyStatus::Running,
        StrategyMarket::Spot,
        StrategyMode::SpotClassic,
    );

    let result = sync_strategy_orders(&mut strategy, &gateway, None);

    assert_eq!(result.submitted, 1);
    assert_eq!(result.canceled, 0);
    assert_eq!(strategy.runtime.orders[0].status, "Placed");
    assert_eq!(
        strategy.runtime.orders[0].exchange_order_id.as_deref(),
        Some("98765")
    );
    let placed = gateway.placed.lock().unwrap();
    assert_eq!(placed.len(), 1);
    assert_eq!(placed[0].market, "spot");
    assert_eq!(
        placed[0].client_order_id.as_deref(),
        Some("strategy-1-order-0")
    );
}

#[test]
fn martingale_working_order_adopts_existing_live_order_by_client_id() {
    let gateway = FakeGateway::default();
    let mut strategy = sample_strategy(
        StrategyStatus::Running,
        StrategyMarket::FuturesUsdM,
        StrategyMode::FuturesLong,
    );
    strategy.strategy_type = StrategyType::MartingaleGrid;
    strategy.runtime.orders[0].order_id = "mg-portfolio-instance-cycle-1-long-leg-0".to_string();
    strategy.runtime.orders[0].exchange_order_id = None;
    strategy.runtime.orders[0].status = "Working".to_string();
    gateway
        .open_orders
        .lock()
        .unwrap()
        .push(BinanceOrderResponse {
            market: "usdm".to_string(),
            symbol: "BTCUSDT".to_string(),
            order_id: "remote-123".to_string(),
            client_order_id: Some("mg-portfolio-instance-cycle-1-long-leg-0".to_string()),
            status: "NEW".to_string(),
            side: Some("BUY".to_string()),
            order_type: Some("LIMIT".to_string()),
            price: Some("100".to_string()),
            quantity: Some("1".to_string()),
        });

    let result = sync_strategy_orders(&mut strategy, &gateway, None);

    assert_eq!(result.submitted, 0);
    assert_eq!(result.refreshed, 1);
    assert_eq!(gateway.placed.lock().unwrap().len(), 0);
    assert_eq!(strategy.runtime.orders[0].status, "Placed");
    assert_eq!(
        strategy.runtime.orders[0].exchange_order_id.as_deref(),
        Some("remote-123")
    );
}

#[test]
fn running_strategy_submits_market_close_intent_orders_without_local_close() {
    let gateway = FakeGateway::default();
    let mut strategy = sample_strategy(
        StrategyStatus::Running,
        StrategyMarket::Spot,
        StrategyMode::SpotClassic,
    );
    strategy.runtime.positions = vec![StrategyRuntimePosition {
        market: StrategyMarket::Spot,
        mode: StrategyMode::SpotClassic,
        quantity: Decimal::new(1, 0),
        average_entry_price: Decimal::new(100, 0),
    }];
    strategy.runtime.orders[0] = StrategyRuntimeOrder {
        order_id: "strategy-1-trail-0".to_string(),
        exchange_order_id: None,
        level_index: Some(0),
        side: "Sell".to_string(),
        order_type: "Market".to_string(),
        price: None,
        quantity: Decimal::new(1, 0),
        status: "ClosingRequested".to_string(),
    };

    let result = sync_strategy_orders(&mut strategy, &gateway, None);

    assert_eq!(result.submitted, 1);
    assert_eq!(strategy.runtime.positions.len(), 1);
    assert_eq!(strategy.runtime.orders[0].status, "Placed");
    let placed = gateway.placed.lock().unwrap();
    assert_eq!(placed.len(), 1);
    assert_eq!(placed[0].order_type, "MARKET");
    assert_eq!(
        placed[0].client_order_id.as_deref(),
        Some("strategy-1-trail-0")
    );
}

#[test]
fn paused_strategy_cancels_known_live_orders() {
    let gateway = FakeGateway::default();
    let mut strategy = sample_strategy(
        StrategyStatus::Paused,
        StrategyMarket::Spot,
        StrategyMode::SpotClassic,
    );
    strategy.runtime.orders[0].status = "Canceled".to_string();
    strategy.runtime.orders[0].exchange_order_id = Some("555".to_string());

    let result = sync_strategy_orders(&mut strategy, &gateway, None);

    assert_eq!(result.submitted, 0);
    assert_eq!(result.canceled, 1);
    assert_eq!(strategy.runtime.orders[0].exchange_order_id, None);
    let canceled = gateway.canceled.lock().unwrap();
    assert_eq!(canceled.len(), 1);
    assert_eq!(canceled[0].0, "spot");
    assert_eq!(canceled[0].2.as_deref(), Some("555"));
}

#[test]
fn placed_order_pulls_remote_status_back_into_runtime() {
    let gateway = FakeGateway::default();
    let mut strategy = sample_strategy(
        StrategyStatus::Running,
        StrategyMarket::Spot,
        StrategyMode::SpotClassic,
    );
    strategy.runtime.orders[0].status = "Placed".to_string();
    strategy.runtime.orders[0].exchange_order_id = Some("555".to_string());

    let result = sync_strategy_orders(&mut strategy, &gateway, None);

    assert_eq!(result.submitted, 0);
    assert_eq!(result.refreshed, 1);
    assert_eq!(strategy.runtime.orders[0].status, "Canceled");
}

#[test]
fn running_strategy_ignores_transient_refresh_errors_for_live_orders() {
    let gateway = FakeGateway::default();
    let mut strategy = sample_strategy(
        StrategyStatus::Running,
        StrategyMarket::Spot,
        StrategyMode::SpotClassic,
    );
    strategy.runtime.orders[0].status = "Placed".to_string();
    strategy.runtime.orders[0].exchange_order_id = Some("555".to_string());
    gateway.get_order_errors.lock().unwrap().insert(
        "strategy-1-order-0".to_string(),
        "binance signed request failed: error sending request for url (https://api.binance.com/api/v3/order): operation timed out".to_string(),
    );

    let result = sync_strategy_orders(&mut strategy, &gateway, None);

    assert_eq!(result.failed, 0);
    assert_eq!(result.refreshed, 0);
    assert_eq!(strategy.runtime.orders[0].status, "Placed");
    assert_eq!(
        strategy.runtime.orders[0].exchange_order_id.as_deref(),
        Some("555")
    );
}

#[test]
fn running_strategy_uses_open_order_snapshot_before_fallback_lookup() {
    let gateway = FakeGateway::default();
    let mut strategy = sample_strategy(
        StrategyStatus::Running,
        StrategyMarket::Spot,
        StrategyMode::SpotClassic,
    );
    strategy.runtime.orders[0].status = "Placed".to_string();
    strategy.runtime.orders[0].exchange_order_id = Some("555".to_string());
    strategy.runtime.orders.push(StrategyRuntimeOrder {
        order_id: "strategy-1-order-1".to_string(),
        exchange_order_id: Some("666".to_string()),
        level_index: Some(1),
        side: "Buy".to_string(),
        order_type: "Limit".to_string(),
        price: Some(Decimal::new(99, 0)),
        quantity: Decimal::new(1, 0),
        status: "Placed".to_string(),
    });
    gateway
        .open_orders
        .lock()
        .unwrap()
        .push(BinanceOrderResponse {
            market: "spot".to_string(),
            symbol: "BTCUSDT".to_string(),
            order_id: "555".to_string(),
            client_order_id: Some("strategy-1-order-0".to_string()),
            status: "NEW".to_string(),
            side: Some("BUY".to_string()),
            order_type: Some("LIMIT".to_string()),
            price: Some("100".to_string()),
            quantity: Some("1".to_string()),
        });
    gateway
        .remote_statuses
        .lock()
        .unwrap()
        .insert("strategy-1-order-1".to_string(), "CANCELED".to_string());
    gateway.get_order_errors.lock().unwrap().insert(
        "strategy-1-order-0".to_string(),
        "still-open order should not require point lookup".to_string(),
    );

    let result = sync_strategy_orders(&mut strategy, &gateway, None);

    assert_eq!(result.failed, 0);
    assert_eq!(result.refreshed, 1);
    assert_eq!(strategy.runtime.orders[0].status, "Placed");
    assert_eq!(strategy.runtime.orders[1].status, "Canceled");
    assert_eq!(strategy.runtime.orders[1].exchange_order_id, None);
}

#[test]
fn stopping_strategy_submits_reduce_only_market_close_orders() {
    let gateway = FakeGateway::default();
    let mut strategy = sample_strategy(
        StrategyStatus::Stopping,
        StrategyMarket::FuturesUsdM,
        StrategyMode::FuturesLong,
    );
    strategy.runtime.positions = vec![StrategyRuntimePosition {
        market: StrategyMarket::FuturesUsdM,
        mode: StrategyMode::FuturesLong,
        quantity: Decimal::new(2, 0),
        average_entry_price: Decimal::new(100, 0),
    }];
    strategy.runtime.orders[0].status = "Placed".to_string();
    strategy.runtime.orders[0].exchange_order_id = Some("grid-1".to_string());

    let result = sync_strategy_orders(&mut strategy, &gateway, None);

    assert_eq!(result.canceled, 1);
    assert_eq!(result.submitted, 1);
    let placed = gateway.placed.lock().unwrap();
    let close = placed
        .iter()
        .find(|order| {
            order
                .client_order_id
                .as_deref()
                .is_some_and(|value| value.contains("-stop-close-"))
        })
        .expect("close order");
    assert_eq!(close.order_type, "MARKET");
    assert_eq!(close.side, "SELL");
    assert_eq!(close.reduce_only, Some(true));
    assert_eq!(close.position_side.as_deref(), Some("LONG"));
    assert_eq!(strategy.status, StrategyStatus::Stopping);
}

#[test]
fn stopping_strategy_moves_to_stopped_after_close_fill_refresh() {
    let gateway = FakeGateway::default();
    let mut strategy = sample_strategy(
        StrategyStatus::Stopping,
        StrategyMarket::FuturesUsdM,
        StrategyMode::FuturesLong,
    );
    strategy.runtime.positions = vec![StrategyRuntimePosition {
        market: StrategyMarket::FuturesUsdM,
        mode: StrategyMode::FuturesLong,
        quantity: Decimal::new(2, 0),
        average_entry_price: Decimal::new(100, 0),
    }];
    strategy.runtime.orders.clear();
    strategy.runtime.orders.push(StrategyRuntimeOrder {
        order_id: "strategy-1-stop-close-0".to_string(),
        exchange_order_id: Some("close-1".to_string()),
        level_index: None,
        side: "Sell".to_string(),
        order_type: "Market".to_string(),
        price: None,
        quantity: Decimal::new(2, 0),
        status: "Placed".to_string(),
    });
    gateway
        .filled_order_ids
        .lock()
        .unwrap()
        .insert("strategy-1-stop-close-0".to_string());

    let result = sync_strategy_orders(&mut strategy, &gateway, None);

    assert_eq!(result.refreshed, 1);
    assert!(strategy.runtime.positions.is_empty());
    assert_eq!(strategy.status, StrategyStatus::Stopped);
}

#[test]
fn stopping_strategy_keeps_partially_filled_close_orders_pending() {
    let gateway = FakeGateway::default();
    let mut strategy = sample_strategy(
        StrategyStatus::Stopping,
        StrategyMarket::FuturesUsdM,
        StrategyMode::FuturesLong,
    );
    strategy.runtime.positions = vec![StrategyRuntimePosition {
        market: StrategyMarket::FuturesUsdM,
        mode: StrategyMode::FuturesLong,
        quantity: Decimal::new(2, 0),
        average_entry_price: Decimal::new(100, 0),
    }];
    strategy.runtime.orders.clear();
    strategy.runtime.orders.push(StrategyRuntimeOrder {
        order_id: "strategy-1-stop-close-0".to_string(),
        exchange_order_id: Some("close-1".to_string()),
        level_index: None,
        side: "Sell".to_string(),
        order_type: "Market".to_string(),
        price: None,
        quantity: Decimal::new(2, 0),
        status: "PartiallyFilled".to_string(),
    });
    gateway.remote_statuses.lock().unwrap().insert(
        "strategy-1-stop-close-0".to_string(),
        "PARTIALLY_FILLED".to_string(),
    );

    let result = sync_strategy_orders(&mut strategy, &gateway, None);

    assert_eq!(result.refreshed, 1);
    assert_eq!(strategy.runtime.positions.len(), 1);
    assert_eq!(strategy.runtime.orders[0].status, "PartiallyFilled");
    assert_eq!(strategy.status, StrategyStatus::Stopping);
}

#[test]
fn futures_neutral_maps_buy_and_sell_to_hedge_position_sides() {
    let gateway = FakeGateway::default();
    let mut strategy = sample_strategy(
        StrategyStatus::Running,
        StrategyMarket::FuturesUsdM,
        StrategyMode::FuturesNeutral,
    );
    strategy.runtime.orders.push(StrategyRuntimeOrder {
        order_id: "strategy-1-order-1".to_string(),
        exchange_order_id: None,
        level_index: Some(1),
        side: "Sell".to_string(),
        order_type: "Limit".to_string(),
        price: Some(Decimal::new(101, 0)),
        quantity: Decimal::new(1, 0),
        status: "Working".to_string(),
    });

    let result = sync_strategy_orders(&mut strategy, &gateway, None);

    assert_eq!(result.submitted, 2);
    let placed = gateway.placed.lock().unwrap();
    assert_eq!(placed[0].market, "usdm");
    assert_eq!(placed[0].position_side.as_deref(), Some("LONG"));
    assert_eq!(placed[1].position_side.as_deref(), Some("SHORT"));
}

#[test]
fn running_strategy_quantizes_price_and_quantity_before_submission() {
    let gateway = FakeGateway::default();
    let mut strategy = sample_strategy(
        StrategyStatus::Running,
        StrategyMarket::Spot,
        StrategyMode::SpotClassic,
    );
    strategy.runtime.orders[0].price = Some(Decimal::new(10013, 2));
    strategy.runtime.orders[0].quantity = Decimal::new(1237, 4);

    let rules = OrderQuantizationRules {
        price_tick_size: Some(Decimal::new(5, 2)),
        quantity_step_size: Some(Decimal::new(1, 3)),
    };

    let result = sync_strategy_orders(&mut strategy, &gateway, Some(&rules));

    assert_eq!(result.submitted, 1);
    let placed = gateway.placed.lock().unwrap();
    assert_eq!(placed[0].price.as_deref(), Some("100.1"));
    assert_eq!(placed[0].quantity, "0.123");
}

fn sample_strategy(status: StrategyStatus, market: StrategyMarket, mode: StrategyMode) -> Strategy {
    Strategy {
        id: "strategy-1".to_string(),
        owner_email: "trader@example.com".to_string(),
        name: "Grid".to_string(),
        symbol: "BTCUSDT".to_string(),
        budget: "1000".to_string(),
        grid_spacing_bps: 100,
        status,
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
        strategy_type: StrategyType::OrdinaryGrid,
        market,
        mode,
        runtime_phase: StrategyRuntimePhase::Draft,
        runtime_controls: RuntimeControls::default(),
        draft_revision: revision(),
        active_revision: Some(revision()),
        runtime: StrategyRuntime {
            positions: Vec::new(),
            orders: vec![StrategyRuntimeOrder {
                order_id: "strategy-1-order-0".to_string(),
                exchange_order_id: None,
                level_index: Some(0),
                side: "Buy".to_string(),
                order_type: "Limit".to_string(),
                price: Some(Decimal::new(100, 0)),
                quantity: Decimal::new(1, 0),
                status: "Working".to_string(),
            }],
            fills: Vec::new(),
            events: Vec::new(),
            last_preflight: None,
        },
        tags: Vec::new(),
        notes: String::new(),
        archived_at: None,
    }
}

fn revision() -> StrategyRevision {
    StrategyRevision {
        revision_id: "rev-1".to_string(),
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
            entry_price: Decimal::new(100, 0),
            quantity: Decimal::new(1, 0),
            take_profit_bps: 100,
            trailing_bps: None,
        }],
        overall_take_profit_bps: None,
        overall_stop_loss_bps: None,
        post_trigger_action: PostTriggerAction::Stop,
    }
}
