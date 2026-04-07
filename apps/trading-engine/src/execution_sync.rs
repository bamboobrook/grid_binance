use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use shared_binance::BinanceExecutionUpdate;
use shared_domain::strategy::Strategy;

pub fn apply_execution_update(strategy: &mut Strategy, update: &BinanceExecutionUpdate) -> bool {
    if !strategy.symbol.eq_ignore_ascii_case(&update.symbol) {
        return false;
    }

    let Some(order_index) = strategy.runtime.orders.iter().position(|order| {
        order
            .exchange_order_id
            .as_deref()
            .is_some_and(|value| value == update.order_id)
            || update
                .client_order_id
                .as_deref()
                .is_some_and(|client_order_id| order.order_id == client_order_id)
    }) else {
        return false;
    };

    let (order_id, level_index, close_filled) = {
        let order = &mut strategy.runtime.orders[order_index];
        let order_id = order.order_id.clone();
        let level_index = order.level_index;
        order.exchange_order_id = Some(update.order_id.clone());
        order.status = normalize_order_status(&update.status).to_string();
        if let Some(price) = update.order_price.as_deref() {
            if let Ok(value) = price.parse() {
                order.price = Some(value);
            }
        }
        let close_filled = strategy.status == shared_domain::strategy::StrategyStatus::Stopping
            && order.order_id.contains("-stop-close-")
            && order.status == "Filled";
        (order_id, level_index, close_filled)
    };

    if update
        .execution_type
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("TRADE"))
    {
        append_execution_fill(strategy, update, &order_id, level_index);
    }
    if close_filled {
        if let Some(index) = close_order_index(&order_id) {
            if index < strategy.runtime.positions.len() {
                strategy.runtime.positions.remove(index);
            }
        }
        finalize_stopping_status(strategy);
    }

    strategy
        .runtime
        .events
        .push(shared_domain::strategy::StrategyRuntimeEvent {
            event_type: "execution_update_received".to_string(),
            detail: format!(
                "{} {} {}",
                update.symbol,
                update.execution_type.as_deref().unwrap_or("UPDATE"),
                update.status
            ),
            price: update
                .last_fill_price
                .as_deref()
                .and_then(|value| value.parse().ok()),
            created_at: Utc
                .timestamp_millis_opt(update.event_time_ms)
                .single()
                .unwrap_or_else(Utc::now),
        });
    true
}

fn normalize_order_status(status: &str) -> &'static str {
    match status.trim().to_ascii_uppercase().as_str() {
        "NEW" => "Placed",
        "PARTIALLY_FILLED" => "PartiallyFilled",
        "FILLED" => "Filled",
        "CANCELED" | "EXPIRED" | "REJECTED" => "Canceled",
        _ => "Placed",
    }
}

fn append_execution_fill(
    strategy: &mut Strategy,
    update: &BinanceExecutionUpdate,
    order_id: &str,
    level_index: Option<u32>,
) {
    let Some(quantity) = update
        .last_fill_quantity
        .as_deref()
        .and_then(|value| value.parse::<Decimal>().ok())
        .filter(|value| *value > Decimal::ZERO)
    else {
        return;
    };
    let fill_id = update
        .trade_id
        .as_deref()
        .map(|trade_id| format!("exchange-trade-{trade_id}"))
        .unwrap_or_else(|| {
            format!(
                "execution-update-{}-{}",
                update.order_id, update.event_time_ms
            )
        });
    if strategy
        .runtime
        .fills
        .iter()
        .any(|fill| fill.fill_id == fill_id)
    {
        return;
    }
    let price = update
        .last_fill_price
        .as_deref()
        .or(update.order_price.as_deref())
        .and_then(|value| value.parse::<Decimal>().ok())
        .unwrap_or(Decimal::ZERO);
    let fee_amount = update
        .fee_amount
        .as_deref()
        .and_then(|value| value.parse::<Decimal>().ok());
    strategy
        .runtime
        .fills
        .push(shared_domain::strategy::StrategyRuntimeFill {
            fill_id,
            order_id: Some(order_id.to_string()),
            level_index,
            fill_type: "ExecutionUpdateFill".to_string(),
            price,
            quantity,
            realized_pnl: update
                .realized_profit
                .as_deref()
                .and_then(|value| value.parse::<Decimal>().ok()),
            fee_amount,
            fee_asset: update.fee_asset.clone(),
        });
}

fn close_order_index(order_id: &str) -> Option<usize> {
    order_id.rsplit('-').next()?.parse::<usize>().ok()
}

fn finalize_stopping_status(strategy: &mut Strategy) {
    let has_pending_close = strategy.runtime.orders.iter().any(|order| {
        order.order_id.contains("-stop-close-")
            && matches!(order.status.as_str(), "ClosingRequested" | "Placed")
    });
    if strategy.runtime.positions.is_empty() && !has_pending_close {
        strategy.status = shared_domain::strategy::StrategyStatus::Stopped;
    }
}
