use chrono::Utc;
use rust_decimal::Decimal;
use shared_binance::{BinanceClient, BinanceOrderRequest, BinanceOrderResponse};
use shared_domain::strategy::{
    Strategy, StrategyMarket, StrategyMode, StrategyRuntimeOrder, StrategyRuntimePosition,
    StrategyStatus, StrategyType,
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

    fn open_orders(&self, market: &str, symbol: &str) -> Result<Vec<BinanceOrderResponse>, String>;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderQuantizationRules {
    pub price_tick_size: Option<Decimal>,
    pub quantity_step_size: Option<Decimal>,
    pub min_quantity: Option<Decimal>,
    pub min_notional: Option<Decimal>,
    pub client_order_id_max_len: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderSyncResult {
    pub submitted: usize,
    pub canceled: usize,
    pub refreshed: usize,
    pub failed: usize,
    pub fatal: usize,
}

pub fn sync_strategy_orders(
    strategy: &mut Strategy,
    gateway: &impl BinanceOrderGateway,
    quantization: Option<&OrderQuantizationRules>,
) -> OrderSyncResult {
    if strategy.strategy_type == StrategyType::MartingaleGrid {
        return sync_martingale_strategy_orders(strategy, gateway, quantization);
    }

    let mut result = OrderSyncResult::default();
    match strategy.status {
        StrategyStatus::Running => {
            let live_open_orders =
                match gateway.open_orders(market_scope(strategy.market), &strategy.symbol) {
                    Ok(orders) => Some(orders),
                    Err(error) if is_transient_gateway_error(&error) => None,
                    Err(error) => {
                        record_order_error(&mut result, &error);
                        None
                    }
                };
            let live_order_ids = live_open_orders
                .as_ref()
                .map(|orders| {
                    orders
                        .iter()
                        .map(|order| order.order_id.clone())
                        .collect::<std::collections::HashSet<_>>()
                })
                .unwrap_or_default();
            let live_client_order_ids = live_open_orders
                .as_ref()
                .map(|orders| {
                    orders
                        .iter()
                        .filter_map(|order| order.client_order_id.clone())
                        .collect::<std::collections::HashSet<_>>()
                })
                .unwrap_or_default();
            for order in &mut strategy.runtime.orders {
                if order.status == "ClosingRequested" && order.exchange_order_id.is_none() {
                    let (_, quantity) = quantize_order(order, quantization);
                    // Derive positionSide from the *actual position being
                    // closed*, not from the order side. For FuturesNeutral a
                    // SELL close must still target the LONG position side (and
                    // vice versa) — deriving from `order.side` would instead
                    // open a new position in the opposite direction. Mirrors
                    // `submit_close_orders` (which also uses
                    // `close_position_side_for_order`).
                    let ps = close_position_side_for_order(
                        &strategy.runtime.positions,
                        strategy.market,
                        strategy.mode,
                        order,
                    );
                    let request = BinanceOrderRequest {
                        market: market_scope(strategy.market).to_string(),
                        symbol: strategy.symbol.clone(),
                        side: order.side.to_ascii_uppercase(),
                        order_type: "MARKET".to_string(),
                        quantity,
                        price: None,
                        time_in_force: None,
                        // Hedge Mode: never send reduceOnly when positionSide is set
                        reduce_only: if !matches!(strategy.market, StrategyMarket::Spot) && ps.is_none() {
                            Some(true)
                        } else {
                            None
                        },
                        position_side: ps,
                        client_order_id: Some(order.order_id.clone()),
                    };
                    if let Err(error) = validate_order_before_placement(&request, quantization) {
                        record_order_error(&mut result, &error);
                        continue;
                    }
                    match gateway.place_order(request) {
                        Ok(response) => {
                            order.exchange_order_id = Some(response.order_id);
                            order.status = "Placed".to_string();
                            result.submitted += 1;
                        }
                        Err(error) => {
                            record_order_error(&mut result, &error);
                        }
                    }
                    continue;
                }
                if order.status == "Placed" {
                    let Some(exchange_order_id) = order.exchange_order_id.clone() else {
                        continue;
                    };
                    if live_order_ids.contains(&exchange_order_id)
                        || live_client_order_ids.contains(&order.order_id)
                    {
                        continue;
                    }
                    if live_open_orders.is_none() {
                        continue;
                    }
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
                        Err(error) => {
                            if !is_transient_gateway_error(&error) {
                                record_order_error(&mut result, &error);
                            }
                        }
                    }
                    continue;
                }
                if order.status != "Working" || order.exchange_order_id.is_some() {
                    continue;
                }
                let (price, quantity) = quantize_order(order, quantization);
                let ps = position_side(strategy.mode, &order.side);
                let request = BinanceOrderRequest {
                    market: market_scope(strategy.market).to_string(),
                    symbol: strategy.symbol.clone(),
                    side: order.side.to_ascii_uppercase(),
                    order_type: order.order_type.to_ascii_uppercase(),
                    quantity,
                    price,
                    time_in_force: (order.order_type.eq_ignore_ascii_case("Limit"))
                        .then(|| "GTC".to_string()),
                    // Hedge Mode: never send reduceOnly when positionSide is set
                    reduce_only: if !matches!(strategy.market, StrategyMarket::Spot)
                        && is_exit_order(order)
                        && ps.is_none()
                    {
                        Some(true)
                    } else {
                        None
                    },
                    position_side: ps,
                    client_order_id: Some(order.order_id.clone()),
                };
                if let Err(error) = validate_order_before_placement(&request, quantization) {
                    record_order_error(&mut result, &error);
                    continue;
                }
                match gateway.place_order(request) {
                    Ok(response) => {
                        order.exchange_order_id = Some(response.order_id);
                        order.status = "Placed".to_string();
                        result.submitted += 1;
                    }
                    Err(error) => {
                        record_order_error(&mut result, &error);
                    }
                }
            }
        }
        StrategyStatus::Stopping => {
            cancel_open_strategy_orders(strategy, gateway, &mut result);
            ensure_close_orders(strategy);
            submit_close_orders(strategy, gateway, quantization, &mut result);
            refresh_close_orders(strategy, gateway, quantization, &mut result);
            finalize_stop_if_ready(strategy);
        }
        StrategyStatus::Paused | StrategyStatus::Stopped | StrategyStatus::ErrorPaused => {
            for order in &mut strategy.runtime.orders {
                if order.status == "Canceled" || order.exchange_order_id.is_none() {
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
                    Err(error) => {
                        record_order_error(&mut result, &error);
                    }
                }
            }
        }
        _ => {}
    }
    result
}

fn sync_martingale_strategy_orders(
    strategy: &mut Strategy,
    gateway: &impl BinanceOrderGateway,
    quantization: Option<&OrderQuantizationRules>,
) -> OrderSyncResult {
    let mut result = OrderSyncResult::default();
    if strategy.status != StrategyStatus::Running {
        return result;
    }

    let live_open_orders =
        match gateway.open_orders(market_scope(strategy.market), &strategy.symbol) {
            Ok(orders) => orders,
            Err(error) if is_transient_gateway_error(&error) => Vec::new(),
            Err(error) => {
                record_order_error(&mut result, &error);
                Vec::new()
            }
        };
    let live_client_order_ids = live_open_orders
        .iter()
        .filter_map(|order| order.client_order_id.clone())
        .collect::<std::collections::HashSet<_>>();

    for order in &mut strategy.runtime.orders {
        if !is_martingale_client_order(&order.order_id) {
            continue;
        }
        if order.status == "Working" && order.exchange_order_id.is_none() {
            if let Some(remote) = live_open_orders
                .iter()
                .find(|remote| remote.client_order_id.as_deref() == Some(order.order_id.as_str()))
            {
                order.exchange_order_id = Some(remote.order_id.clone());
                order.status = "Placed".to_string();
                result.refreshed += 1;
                continue;
            }
        }
        if order.status == "Placed" && live_client_order_ids.contains(&order.order_id) {
            continue;
        }
        if order.status != "Working" || order.exchange_order_id.is_some() {
            continue;
        }
        let quantity_normalized = normalize_to_step(
            order.quantity,
            quantization.and_then(|rules| rules.quantity_step_size),
        );
        if quantity_normalized <= Decimal::ZERO {
            result.failed += 1;
            continue;
        }
        let price = order.price.map(|price| {
            normalize_to_step(price, quantization.and_then(|rules| rules.price_tick_size))
                .normalize()
                .to_string()
        });
        let request = BinanceOrderRequest {
            market: market_scope(strategy.market).to_string(),
            symbol: strategy.symbol.clone(),
            side: order.side.to_ascii_uppercase(),
            order_type: order.order_type.to_ascii_uppercase(),
            quantity: quantity_normalized.normalize().to_string(),
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
            Err(error) => record_order_error(&mut result, &error),
        }
    }
    result
}

fn is_martingale_client_order(order_id: &str) -> bool {
    order_id.starts_with("mg-") && order_id.contains("-leg-")
}

fn is_transient_gateway_error(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    [
        "timed out",
        "timeout",
        "error sending request",
        "connection reset",
        "connection refused",
        "connection aborted",
        "peer closed connection",
        "tls",
        "unexpected eof",
        "temporarily unavailable",
        "network",
        "dns error",
    ]
    .iter()
    .any(|needle| error.contains(needle))
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
            Err(error) => record_order_error(result, &error),
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
    quantization: Option<&OrderQuantizationRules>,
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
        let (_, quantity) = quantize_order(order, quantization);
        let position_side = close_position_side_for_order(&positions, market, mode, order);
        let request = BinanceOrderRequest {
            market: market_scope(market).to_string(),
            symbol: symbol.clone(),
            side: order.side.to_ascii_uppercase(),
            order_type: "MARKET".to_string(),
            quantity,
            price: None,
            time_in_force: None,
            // Hedge Mode: never send reduceOnly (positionSide is used instead)
            reduce_only: if !matches!(market, StrategyMarket::Spot) && position_side.is_none() {
                Some(true)
            } else {
                None
            },
            position_side,
            client_order_id: Some(order.order_id.clone()),
        };
        match gateway.place_order(request) {
            Ok(response) => {
                order.exchange_order_id = Some(response.order_id);
                order.status = "Placed".to_string();
                result.submitted += 1;
            }
            Err(error) => record_order_error(result, &error),
        }
    }
}

fn refresh_close_orders(
    strategy: &mut Strategy,
    gateway: &impl BinanceOrderGateway,
    _quantization: Option<&OrderQuantizationRules>,
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
            Err(error) => record_order_error(result, &error),
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

/// Validate an order before submitting to Binance. Returns an error string
/// if the order would be rejected by the exchange due to:
/// - client_order_id exceeding the maximum length
/// - quantity below the minimum
/// - notional value below the minimum
fn validate_order_before_placement(
    request: &BinanceOrderRequest,
    rules: Option<&OrderQuantizationRules>,
) -> Result<(), String> {
    let Some(rules) = rules else {
        return Ok(());
    };
    // Client order id length check
    if let Some(ref client_order_id) = request.client_order_id {
        if rules.client_order_id_max_len > 0
            && client_order_id.len() > rules.client_order_id_max_len
        {
            return Err(format!(
                "clientOrderId '{}' ({} chars) exceeds Binance limit of {} chars",
                client_order_id,
                client_order_id.len(),
                rules.client_order_id_max_len
            ));
        }
    }
    // Minimum quantity check
    if let Some(min_qty) = rules.min_quantity.filter(|v| *v > Decimal::ZERO) {
        if let Ok(qty) = request.quantity.parse::<Decimal>() {
            if qty < min_qty {
                return Err(format!(
                    "order quantity {} is below minimum {} for {}",
                    qty.normalize(),
                    min_qty.normalize(),
                    request.symbol
                ));
            }
        }
    }
    // Minimum notional check
    if let Some(min_notional) = rules.min_notional.filter(|v| *v > Decimal::ZERO) {
        if let (Ok(qty), Some(ref price_str)) =
            (request.quantity.parse::<Decimal>(), request.price.as_ref())
        {
            if let Ok(price) = price_str.parse::<Decimal>() {
                let notional = (qty * price).normalize();
                if notional < min_notional {
                    return Err(format!(
                        "order notional {} is below minimum {} for {}",
                        notional,
                        min_notional.normalize(),
                        request.symbol
                    ));
                }
            }
        }
    }
    Ok(())
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

fn is_exit_order(order: &StrategyRuntimeOrder) -> bool {
    order.order_id.contains("-tp-") || order.order_id.contains("-trail-")
}

/// Classification of a Binance order error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderErrorClass {
    /// Error that should pause the strategy (e.g. insufficient balance,
    /// bad API key, leverage/nominal-value violations). Retrying on the
    /// next tick cannot fix these.
    Fatal,
    /// Transient error worth retrying on the next reconcile tick
    /// (rate limit, server timeout, disconnected). Counted as a soft
    /// failure so the strategy stays Running.
    Retryable,
    /// Informational / idempotent outcome (order already canceled, does
    /// not exist). Should be ignored without incrementing the failure
    /// counters so it does not block the strategy.
    Skip,
}

/// Parse the numeric Binance error code from a
/// `"binance error ({code}): {msg}"` message.
fn binance_error_code(error: &str) -> Option<i64> {
    let rest = error.strip_prefix("binance error (")?;
    let end = rest.find(')')?;
    rest[..end].parse::<i64>().ok()
}

/// Classify an order error string against Binance's documented error codes.
/// See https://developers.binance.com/docs/derivatives/usds-margined-futures/error-code
/// and the general error-code reference. Unknown errors fall back to
/// `Retryable` so an unrecognised transient issue is retried rather than
/// permanently pausing the strategy.
fn classify_binance_order_error(error: &str) -> OrderErrorClass {
    let Some(code) = binance_error_code(error) else {
        // Not a Binance-coded error (network/gateway). Defer to the
        // transient-substring check; if it isn't transient either, treat
        // it as a soft failure so the strategy keeps retrying.
        return if is_transient_gateway_error(error) {
            OrderErrorClass::Retryable
        } else {
            OrderErrorClass::Retryable
        };
    };
    match code {
        // --- Fatal: cannot be fixed by retrying, pause the strategy ---
        // -2010: insufficient balance / account has insufficient balance
        // -2019: margin is insufficient
        // -2018: the tick size / step size violates
        // -2022: leverage not valid for the configured bracket (parameter/config fix needed)
        // -4043: position is in liquidation state / notional exceeds bracket cap
        // -4164: order's notional value exceeds the allowed leverage bracket
        // -1100/1101/1102: illegal characters / parameter / too many parameters
        // -1015: too many orders (account-level rate-limit, needs operator action)
        // -2015: invalid API-key / permission (credential fix needed)
        // -1002/-1010: unauthorized / account restricted
        -2010 | -2019 | -2022 | -4043 | -4164 | -2015 | -1015 | -1010 | -1002 => {
            OrderErrorClass::Fatal
        }
        // --- Skip: idempotent outcomes, do not count as a failure ---
        // -2011: Unknown order sent (already filled/canceled/rejected)
        // -2013: Order does not exist
        // -4060/-4061: order has been triggered / in a state that rejects the op
        // -1099: order adjustments not allowed (noop)
        -2011 | -2013 | -4060 | -4061 | -1099 => OrderErrorClass::Skip,
        // --- Retryable: transient, retry on next tick ---
        // -1001: DISCONNECTED, -1003: rate limit (server), -1006/-1007: timeout,
        // -1021: timestamp out of recvWindow, -5022: server busy
        -1001 | -1003 | -1006 | -1007 | -1021 | -5022 => OrderErrorClass::Retryable,
        // Unknown Binance code: conservatively retry rather than pause.
        _ => OrderErrorClass::Retryable,
    }
}

fn record_order_error(result: &mut OrderSyncResult, error: &str) {
    match classify_binance_order_error(error) {
        OrderErrorClass::Fatal => result.fatal += 1,
        // Idempotent outcomes are not failures; ignore them so they don't
        // block or pause the strategy.
        OrderErrorClass::Skip => {}
        OrderErrorClass::Retryable => result.failed += 1,
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

    fn open_orders(&self, market: &str, symbol: &str) -> Result<Vec<BinanceOrderResponse>, String> {
        BinanceClient::open_orders(self, market, symbol).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fatal_codes_pause_strategy() {
        for code in [-2010, -2019, -2022, -4043, -4164, -2015, -1015, -1010, -1002] {
            let error = format!("binance error ({code}): some message");
            assert_eq!(
                classify_binance_order_error(&error),
                OrderErrorClass::Fatal,
                "code {code} should be Fatal"
            );
        }
    }

    #[test]
    fn idempotent_codes_are_skipped_without_counting_as_failure() {
        for code in [-2011, -2013, -4060, -4061, -1099] {
            let error = format!("binance error ({code}): some message");
            assert_eq!(
                classify_binance_order_error(&error),
                OrderErrorClass::Skip,
                "code {code} should be Skip"
            );
        }
        // record_order_error must NOT bump failed/fatal for Skip codes.
        let mut result = OrderSyncResult::default();
        record_order_error(&mut result, "binance error (-2013): Order does not exist");
        assert_eq!(result, OrderSyncResult::default());
    }

    #[test]
    fn transient_codes_are_retryable() {
        for code in [-1001, -1003, -1006, -1007, -1021, -5022] {
            let error = format!("binance error ({code}): some message");
            assert_eq!(
                classify_binance_order_error(&error),
                OrderErrorClass::Retryable,
                "code {code} should be Retryable"
            );
        }
        // Unknown Binance code falls back to Retryable rather than pausing.
        assert_eq!(
            classify_binance_order_error("binance error (-9999): unknown"),
            OrderErrorClass::Retryable
        );
    }

    #[test]
    fn non_binance_network_errors_are_retryable() {
        assert_eq!(
            classify_binance_order_error("error sending request: operation timed out"),
            OrderErrorClass::Retryable
        );
    }

    #[test]
    fn record_order_error_counts_fatal_and_failed_correctly() {
        let mut result = OrderSyncResult::default();
        record_order_error(&mut result, "binance error (-2010): insufficient balance");
        assert_eq!(result.fatal, 1);
        assert_eq!(result.failed, 0);

        record_order_error(&mut result, "binance error (-1001): disconnected");
        assert_eq!(result.fatal, 1);
        assert_eq!(result.failed, 1);
    }
}
