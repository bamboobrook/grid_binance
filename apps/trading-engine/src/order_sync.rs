use chrono::Utc;
use rust_decimal::Decimal;
use shared_binance::{BinanceClient, BinanceOrderRequest, BinanceOrderResponse};
use shared_domain::strategy::{
    Strategy, StrategyMarket, StrategyMode, StrategyRuntimeOrder, StrategyRuntimePosition,
    StrategyStatus,
};

pub trait BinanceOrderGateway {
    fn place_order(&self, request: BinanceOrderRequest) -> Result<BinanceOrderResponse, String>;

    fn cancel_order(
        &self,
        market: &str,
        symbol: &str,
        order_id: Option<&str>,
        client_order_id: Option<&str>,
    ) -> Result<BinanceOrderResponse, String>;

    fn get_order(
        &self,
        market: &str,
        symbol: &str,
        order_id: Option<&str>,
        client_order_id: Option<&str>,
    ) -> Result<BinanceOrderResponse, String>;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderQuantizationRules {
    pub price_tick_size: Option<Decimal>,
    pub quantity_step_size: Option<Decimal>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderSyncResult {
    pub submitted: usize,
    pub canceled: usize,
    pub refreshed: usize,
    pub failed: usize,
}

pub fn sync_strategy_orders(
    strategy: &mut Strategy,
    gateway: &impl BinanceOrderGateway,
    quantization: Option<&OrderQuantizationRules>,
) -> OrderSyncResult {
    let mut result = OrderSyncResult::default();
    match strategy.status {
        StrategyStatus::Running => {
            for order in &mut strategy.runtime.orders {
                if order.status == "ClosingRequested" && order.exchange_order_id.is_none() {
                    let (_, quantity) = quantize_order(order, quantization);
                    let request = BinanceOrderRequest {
                        market: market_scope(strategy.market).to_string(),
                        symbol: strategy.symbol.clone(),
                        side: order.side.to_ascii_uppercase(),
                        order_type: "MARKET".to_string(),
                        quantity,
                        price: None,
                        time_in_force: None,
                        reduce_only: (!matches!(strategy.market, StrategyMarket::Spot))
                            .then_some(true),
                        position_side: position_side(strategy.mode, &order.side),
                        client_order_id: Some(order.order_id.clone()),
                    };
                    match gateway.place_order(request) {
                        Ok(response) => {
                            order.exchange_order_id = Some(response.order_id);
                            order.status = "Placed".to_string();
                            result.submitted += 1;
                        }
                        Err(_) => {
                            result.failed += 1;
                        }
                    }
                    continue;
                }
                if order.status == "Placed" {
                    if let Some(exchange_order_id) = order.exchange_order_id.clone() {
                        match gateway.get_order(
                            market_scope(strategy.market),
                            &strategy.symbol,
                            Some(&exchange_order_id),
                            Some(&order.order_id),
                        ) {
                            Ok(response) => {
                                if response.status.eq_ignore_ascii_case("CANCELED") {
                                    order.status = "Canceled".to_string();
                                    order.exchange_order_id = None;
                                    result.refreshed += 1;
                                }
                            }
                            Err(_) => result.failed += 1,
                        }
                    }
                    continue;
                }
                if order.status != "Working" || order.exchange_order_id.is_some() {
                    continue;
                }
                let (price, quantity) = quantize_order(order, quantization);
                let request = BinanceOrderRequest {
                    market: market_scope(strategy.market).to_string(),
                    symbol: strategy.symbol.clone(),
                    side: order.side.to_ascii_uppercase(),
                    order_type: order.order_type.to_ascii_uppercase(),
                    quantity,
                    price,
                    time_in_force: (order.order_type.eq_ignore_ascii_case("Limit"))
                        .then(|| "GTC".to_string()),
                    reduce_only: None,
                    position_side: position_side(strategy.mode, &order.side),
                    client_order_id: Some(order.order_id.clone()),
                };
                match gateway.place_order(request) {
                    Ok(response) => {
                        order.exchange_order_id = Some(response.order_id);
                        order.status = "Placed".to_string();
                        result.submitted += 1;
                    }
                    Err(_) => {
                        result.failed += 1;
                    }
                }
            }
        }
        StrategyStatus::Stopping => {
            cancel_open_strategy_orders(strategy, gateway, &mut result);
            ensure_close_orders(strategy);
            submit_close_orders(strategy, gateway, &mut result);
            refresh_close_orders(strategy, gateway, &mut result);
            finalize_stop_if_ready(strategy);
        }
        StrategyStatus::Paused | StrategyStatus::Stopped | StrategyStatus::ErrorPaused => {
            for order in &mut strategy.runtime.orders {
                if order.status != "Canceled" || order.exchange_order_id.is_none() {
                    continue;
                }
                let exchange_order_id = order.exchange_order_id.clone();
                match gateway.cancel_order(
                    market_scope(strategy.market),
                    &strategy.symbol,
                    exchange_order_id.as_deref(),
                    Some(&order.order_id),
                ) {
                    Ok(_) => {
                        order.exchange_order_id = None;
                        result.canceled += 1;
                    }
                    Err(_) => {
                        result.failed += 1;
                    }
                }
            }
        }
        _ => {}
    }
    result
}

fn cancel_open_strategy_orders(
    strategy: &mut Strategy,
    gateway: &impl BinanceOrderGateway,
    result: &mut OrderSyncResult,
) {
    for order in &mut strategy.runtime.orders {
        if is_close_order(order) {
            continue;
        }
        if matches!(order.status.as_str(), "Working" | "Placed")
            && order.exchange_order_id.is_none()
        {
            order.status = "Canceled".to_string();
            continue;
        }
        if !matches!(order.status.as_str(), "Working" | "Placed" | "Canceled")
            || order.exchange_order_id.is_none()
        {
            continue;
        }
        let exchange_order_id = order.exchange_order_id.clone();
        match gateway.cancel_order(
            market_scope(strategy.market),
            &strategy.symbol,
            exchange_order_id.as_deref(),
            Some(&order.order_id),
        ) {
            Ok(_) => {
                order.status = "Canceled".to_string();
                order.exchange_order_id = None;
                result.canceled += 1;
            }
            Err(_) => result.failed += 1,
        }
    }
}

fn ensure_close_orders(strategy: &mut Strategy) {
    for (index, position) in strategy.runtime.positions.iter().enumerate() {
        let close_order_id = close_order_id(&strategy.id, index);
        if strategy
            .runtime
            .orders
            .iter()
            .any(|order| order.order_id == close_order_id)
        {
            continue;
        }
        strategy.runtime.orders.push(StrategyRuntimeOrder {
            order_id: close_order_id,
            exchange_order_id: None,
            level_index: None,
            side: close_side_for_position(position).to_string(),
            order_type: "Market".to_string(),
            price: None,
            quantity: position.quantity,
            status: "ClosingRequested".to_string(),
        });
    }
}

fn submit_close_orders(
    strategy: &mut Strategy,
    gateway: &impl BinanceOrderGateway,
    result: &mut OrderSyncResult,
) {
    let positions = strategy.runtime.positions.clone();
    let market = strategy.market;
    let mode = strategy.mode;
    let symbol = strategy.symbol.clone();
    for order in &mut strategy.runtime.orders {
        if !is_close_order(order)
            || order.exchange_order_id.is_some()
            || order.status != "ClosingRequested"
        {
            continue;
        }
        let (_, quantity) = quantize_order(order, None);
        let request = BinanceOrderRequest {
            market: market_scope(market).to_string(),
            symbol: symbol.clone(),
            side: order.side.to_ascii_uppercase(),
            order_type: "MARKET".to_string(),
            quantity,
            price: None,
            time_in_force: None,
            reduce_only: (!matches!(market, StrategyMarket::Spot)).then_some(true),
            position_side: close_position_side_for_order(&positions, market, mode, order),
            client_order_id: Some(order.order_id.clone()),
        };
        match gateway.place_order(request) {
            Ok(response) => {
                order.exchange_order_id = Some(response.order_id);
                order.status = "Placed".to_string();
                result.submitted += 1;
            }
            Err(_) => result.failed += 1,
        }
    }
}

fn refresh_close_orders(
    strategy: &mut Strategy,
    gateway: &impl BinanceOrderGateway,
    result: &mut OrderSyncResult,
) {
    let mut filled_indices = Vec::new();
    for order in &mut strategy.runtime.orders {
        if !is_close_order(order)
            || order.exchange_order_id.is_none()
            || !matches!(order.status.as_str(), "Placed" | "PartiallyFilled")
        {
            continue;
        }
        let exchange_order_id = order.exchange_order_id.clone();
        match gateway.get_order(
            market_scope(strategy.market),
            &strategy.symbol,
            exchange_order_id.as_deref(),
            Some(&order.order_id),
        ) {
            Ok(response) => {
                if response.status.eq_ignore_ascii_case("FILLED") {
                    order.status = "Filled".to_string();
                    result.refreshed += 1;
                    if let Some(index) = close_order_index(&order.order_id) {
                        filled_indices.push(index);
                    }
                } else if response.status.eq_ignore_ascii_case("PARTIALLY_FILLED") {
                    order.status = "PartiallyFilled".to_string();
                    result.refreshed += 1;
                } else if response.status.eq_ignore_ascii_case("CANCELED") {
                    order.status = "ClosingRequested".to_string();
                    order.exchange_order_id = None;
                    result.refreshed += 1;
                }
            }
            Err(_) => result.failed += 1,
        }
    }
    filled_indices.sort_unstable();
    filled_indices.dedup();
    for index in filled_indices.into_iter().rev() {
        if index < strategy.runtime.positions.len() {
            strategy.runtime.positions.remove(index);
        }
    }
}

fn quantize_order(
    order: &StrategyRuntimeOrder,
    rules: Option<&OrderQuantizationRules>,
) -> (Option<String>, String) {
    let quantity = normalize_to_step(
        order.quantity,
        rules.and_then(|rules| rules.quantity_step_size),
    )
    .normalize()
    .to_string();
    let price = order.price.map(|price| {
        normalize_to_step(price, rules.and_then(|rules| rules.price_tick_size))
            .normalize()
            .to_string()
    });
    (price, quantity)
}

fn normalize_to_step(value: Decimal, step: Option<Decimal>) -> Decimal {
    let Some(step) = step.filter(|step| *step > Decimal::ZERO) else {
        return value;
    };
    ((value / step).floor() * step).normalize()
}

fn finalize_stop_if_ready(strategy: &mut Strategy) {
    let has_pending_close = strategy.runtime.orders.iter().any(|order| {
        is_close_order(order)
            && matches!(
                order.status.as_str(),
                "ClosingRequested" | "Placed" | "PartiallyFilled"
            )
    });
    if strategy.runtime.positions.is_empty() && !has_pending_close {
        if pending_rebuild_after_stop(strategy) {
            return;
        }
        strategy.status = StrategyStatus::Stopped;
        strategy
            .runtime
            .events
            .push(shared_domain::strategy::StrategyRuntimeEvent {
                event_type: "strategy_stopped".to_string(),
                detail: "strategy stopped after exchange close reconciliation".to_string(),
                price: None,
                created_at: Utc::now(),
            });
    }
}

fn is_close_order(order: &StrategyRuntimeOrder) -> bool {
    order.order_id.contains("-stop-close-")
}

fn pending_rebuild_after_stop(strategy: &Strategy) -> bool {
    let reset_index = strategy
        .runtime
        .events
        .iter()
        .rposition(|event| {
            matches!(
                event.event_type.as_str(),
                "strategy_started" | "strategy_resumed" | "strategy_rebuilt"
            )
        })
        .unwrap_or(0);
    strategy
        .runtime
        .events
        .iter()
        .skip(reset_index)
        .rev()
        .find(|event| {
            event.event_type.starts_with("overall_take_profit")
                || event.event_type.starts_with("overall_stop_loss")
        })
        .is_some_and(|event| event.event_type.ends_with("_rebuild"))
}

fn close_order_id(strategy_id: &str, index: usize) -> String {
    format!("{strategy_id}-stop-close-{index}")
}

fn close_order_index(order_id: &str) -> Option<usize> {
    order_id.rsplit('-').next()?.parse::<usize>().ok()
}

fn close_side_for_position(position: &StrategyRuntimePosition) -> &'static str {
    match position.mode {
        StrategyMode::SpotClassic | StrategyMode::SpotBuyOnly | StrategyMode::FuturesLong => "Sell",
        StrategyMode::SpotSellOnly | StrategyMode::FuturesShort => "Buy",
        StrategyMode::FuturesNeutral => "Sell",
    }
}

fn close_position_side_for_order(
    positions: &[StrategyRuntimePosition],
    market: StrategyMarket,
    mode: StrategyMode,
    order: &StrategyRuntimeOrder,
) -> Option<String> {
    if matches!(market, StrategyMarket::Spot) {
        return None;
    }
    let Some(index) = close_order_index(&order.order_id) else {
        return position_side(mode, &order.side);
    };
    positions
        .get(index)
        .and_then(|position| match position.mode {
            StrategyMode::FuturesLong => Some("LONG".to_string()),
            StrategyMode::FuturesShort => Some("SHORT".to_string()),
            _ => position_side(mode, &order.side),
        })
}

fn market_scope(market: StrategyMarket) -> &'static str {
    match market {
        StrategyMarket::Spot => "spot",
        StrategyMarket::FuturesUsdM => "usdm",
        StrategyMarket::FuturesCoinM => "coinm",
    }
}

fn position_side(mode: StrategyMode, side: &str) -> Option<String> {
    match mode {
        StrategyMode::FuturesLong => Some("LONG".to_string()),
        StrategyMode::FuturesShort => Some("SHORT".to_string()),
        StrategyMode::FuturesNeutral => {
            if side.eq_ignore_ascii_case("Buy") {
                Some("LONG".to_string())
            } else {
                Some("SHORT".to_string())
            }
        }
        _ => None,
    }
}

impl BinanceOrderGateway for BinanceClient {
    fn place_order(&self, request: BinanceOrderRequest) -> Result<BinanceOrderResponse, String> {
        BinanceClient::place_order(self, request).map_err(|error| error.to_string())
    }

    fn cancel_order(
        &self,
        market: &str,
        symbol: &str,
        order_id: Option<&str>,
        client_order_id: Option<&str>,
    ) -> Result<BinanceOrderResponse, String> {
        BinanceClient::cancel_order(self, market, symbol, order_id, client_order_id)
            .map_err(|error| error.to_string())
    }

    fn get_order(
        &self,
        market: &str,
        symbol: &str,
        order_id: Option<&str>,
        client_order_id: Option<&str>,
    ) -> Result<BinanceOrderResponse, String> {
        BinanceClient::get_order(self, market, symbol, order_id, client_order_id)
            .map_err(|error| error.to_string())
    }
}
