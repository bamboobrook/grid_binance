use crate::strategy_runtime::StrategyRuntimeEngine;
use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use shared_binance::BinanceExecutionUpdate;
use shared_domain::strategy::{
    Strategy, StrategyMarket, StrategyMode, StrategyRuntimeFill, StrategyRuntimeOrder,
    StrategyRuntimePosition,
};

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

    let (order_id, level_index, order_status, order_side, is_close_order, close_filled) = {
        let order = &mut strategy.runtime.orders[order_index];
        let order_id = order.order_id.clone();
        let level_index = order.level_index;
        let order_side = order.side.clone();
        order.exchange_order_id = Some(update.order_id.clone());
        order.status = normalize_order_status(&update.status).to_string();
        if let Some(price) = update.order_price.as_deref() {
            if let Ok(value) = price.parse() {
                order.price = Some(value);
            }
        }
        let is_close_order = order.order_id.contains("-stop-close-");
        let close_filled = strategy.status == shared_domain::strategy::StrategyStatus::Stopping
            && is_close_order
            && order.status == "Filled";
        (
            order_id,
            level_index,
            order.status.clone(),
            order_side,
            is_close_order,
            close_filled,
        )
    };

    let trade_applied = update
        .execution_type
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("TRADE"));
    if trade_applied {
        append_execution_fill(strategy, update, &order_id, level_index);
    }

    if is_close_order && trade_applied {
        apply_close_fill(strategy, &order_id, &order_side, update);
        if close_filled {
            finalize_strategy_after_close(strategy, update_price(update));
        }
    } else if trade_applied && order_status != "Canceled" {
        advance_grid_cycle_after_fill(strategy, order_index);
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
    let realized_pnl = derive_realized_pnl_for_fill(
        strategy,
        order_id,
        level_index,
        price,
        quantity,
        update.realized_profit.as_deref(),
    );
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
            realized_pnl,
            fee_amount,
            fee_asset: update.fee_asset.clone(),
        });
}

fn advance_grid_cycle_after_fill(strategy: &mut Strategy, order_index: usize) {
    let order = strategy.runtime.orders[order_index].clone();
    if order.order_id.contains("-stop-close-") {
        return;
    }
    let Some(level_index) = order.level_index else {
        return;
    };

    if is_take_profit_order(&order.order_id) || is_trailing_monitor_order(&order.order_id) {
        handle_take_profit_fill(strategy, level_index, &order);
        return;
    }

    handle_entry_fill(strategy, level_index, &order);
}

fn handle_entry_fill(strategy: &mut Strategy, level_index: u32, order: &StrategyRuntimeOrder) {
    let Some(level_state) = level_state(strategy, level_index) else {
        return;
    };
    recompute_positions(strategy);
    sync_exit_order(strategy, level_index, &level_state, order);
    if should_activate_ordinary_replenishment_orders(strategy, level_index, order) {
        activate_ordinary_replenishment_orders(strategy, level_index, &order.side);
    }
}

fn handle_take_profit_fill(
    strategy: &mut Strategy,
    level_index: u32,
    order: &StrategyRuntimeOrder,
) {
    recompute_positions(strategy);
    let remaining = level_state(strategy, level_index);
    if strategy.status == shared_domain::strategy::StrategyStatus::Stopping {
        finalize_strategy_after_close(strategy, None);
        return;
    }
    if remaining.is_some() {
        return;
    }

    let Some(level) = level_for(strategy, level_index).cloned() else {
        return;
    };
    if let Some(entry_order) = strategy.runtime.orders.iter_mut().find(|candidate| {
        candidate.level_index == Some(level_index) && is_entry_order(&candidate.order_id)
    }) {
        entry_order.exchange_order_id = None;
        entry_order.side = opposite_side(&order.side).to_string();
        entry_order.order_type = "Limit".to_string();
        entry_order.price = Some(level.entry_price);
        entry_order.quantity = level.quantity;
        entry_order.status = "Working".to_string();
        return;
    }
    strategy.runtime.orders.push(StrategyRuntimeOrder {
        order_id: entry_order_id(&strategy.id, level_index),
        exchange_order_id: None,
        level_index: Some(level_index),
        side: opposite_side(&order.side).to_string(),
        order_type: "Limit".to_string(),
        price: Some(level.entry_price),
        quantity: level.quantity,
        status: "Working".to_string(),
    });
}

#[derive(Debug, Clone, Copy)]
struct LevelState {
    quantity: Decimal,
    average_entry_price: Decimal,
    is_short: bool,
}

fn effective_entry_fill_quantity(strategy: &Strategy, fill: &StrategyRuntimeFill) -> Decimal {
    if strategy.market != StrategyMarket::Spot {
        return fill.quantity;
    }
    let Some(order_id) = fill.order_id.as_deref() else {
        return fill.quantity;
    };
    let Some(order) = strategy
        .runtime
        .orders
        .iter()
        .find(|candidate| candidate.order_id == order_id)
    else {
        return fill.quantity;
    };
    if !order.side.eq_ignore_ascii_case("Buy") {
        return fill.quantity;
    }
    let Some(fee_asset) = fill.fee_asset.as_deref() else {
        return fill.quantity;
    };
    if fee_asset.is_empty()
        || !strategy
            .symbol
            .to_ascii_uppercase()
            .starts_with(&fee_asset.to_ascii_uppercase())
    {
        return fill.quantity;
    }
    let fee_amount = fill.fee_amount.unwrap_or(Decimal::ZERO);
    let adjusted = fill.quantity - fee_amount;
    if adjusted > Decimal::ZERO {
        adjusted
    } else {
        Decimal::ZERO
    }
}

fn level_state(strategy: &Strategy, level_index: u32) -> Option<LevelState> {
    let entry_order_id = entry_order_id(&strategy.id, level_index);
    let mut remaining_quantity = Decimal::ZERO;
    let mut remaining_cost = Decimal::ZERO;

    for fill in strategy
        .runtime
        .fills
        .iter()
        .filter(|fill| fill.level_index == Some(level_index))
    {
        if fill.order_id.as_deref() == Some(entry_order_id.as_str()) {
            remaining_quantity += effective_entry_fill_quantity(strategy, fill);
            remaining_cost += fill.price * fill.quantity;
            continue;
        }

        if remaining_quantity <= Decimal::ZERO {
            remaining_quantity = Decimal::ZERO;
            remaining_cost = Decimal::ZERO;
            continue;
        }

        let closed_quantity = fill.quantity.min(remaining_quantity);
        let average_entry_price = remaining_cost / remaining_quantity;
        remaining_quantity -= closed_quantity;
        remaining_cost -= average_entry_price * closed_quantity;
        if remaining_quantity <= Decimal::ZERO {
            remaining_quantity = Decimal::ZERO;
            remaining_cost = Decimal::ZERO;
        }
    }

    if remaining_quantity <= Decimal::ZERO {
        return None;
    }

    let is_short = strategy
        .runtime
        .orders
        .iter()
        .find(|order| order.order_id == entry_order_id)
        .is_some_and(|order| order.side.eq_ignore_ascii_case("Sell"));

    Some(LevelState {
        quantity: remaining_quantity,
        average_entry_price: remaining_cost / remaining_quantity,
        is_short,
    })
}

fn recompute_positions(strategy: &mut Strategy) {
    let mut long_quantity = Decimal::ZERO;
    let mut long_cost = Decimal::ZERO;
    let mut short_quantity = Decimal::ZERO;
    let mut short_cost = Decimal::ZERO;
    let levels = strategy
        .active_revision
        .as_ref()
        .unwrap_or(&strategy.draft_revision)
        .levels
        .iter()
        .map(|level| level.level_index)
        .collect::<Vec<_>>();

    for level_index in levels {
        let Some(state) = level_state(strategy, level_index) else {
            continue;
        };
        if state.is_short {
            short_quantity += state.quantity;
            short_cost += state.average_entry_price * state.quantity;
        } else {
            long_quantity += state.quantity;
            long_cost += state.average_entry_price * state.quantity;
        }
    }

    let mut positions = Vec::new();
    if long_quantity > Decimal::ZERO {
        positions.push(StrategyRuntimePosition {
            market: strategy.market,
            mode: position_mode_for_entry(strategy.market, strategy.mode, "Buy"),
            quantity: long_quantity,
            average_entry_price: long_cost / long_quantity,
        });
    }
    if short_quantity > Decimal::ZERO {
        positions.push(StrategyRuntimePosition {
            market: strategy.market,
            mode: position_mode_for_entry(strategy.market, strategy.mode, "Sell"),
            quantity: short_quantity,
            average_entry_price: short_cost / short_quantity,
        });
    }
    strategy.runtime.positions = positions;
}

fn sync_exit_order(
    strategy: &mut Strategy,
    level_index: u32,
    state: &LevelState,
    order: &StrategyRuntimeOrder,
) {
    let Some(level) = level_for(strategy, level_index).cloned() else {
        return;
    };
    let order_id = if level.trailing_bps.is_some() {
        trailing_order_id(&strategy.id, level_index)
    } else {
        take_profit_order_id(&strategy.id, level_index)
    };
    let order_type = if level.trailing_bps.is_some() {
        "TrailMonitor"
    } else {
        "Limit"
    };
    let status = if level.trailing_bps.is_some() {
        "Monitoring"
    } else {
        "Working"
    };
    let price = Some(take_profit_price(
        state.average_entry_price,
        level.take_profit_bps,
        is_short_side(&order.side),
    ));
    if let Some(existing) = strategy
        .runtime
        .orders
        .iter_mut()
        .find(|candidate| candidate.order_id == order_id)
    {
        if matches!(existing.status.as_str(), "Filled" | "Canceled") {
            existing.exchange_order_id = None;
            existing.status = status.to_string();
        }
        existing.side = opposite_side(&order.side).to_string();
        existing.order_type = order_type.to_string();
        existing.price = price;
        existing.quantity = state.quantity;
        return;
    }
    strategy.runtime.orders.push(StrategyRuntimeOrder {
        order_id,
        exchange_order_id: None,
        level_index: Some(level_index),
        side: opposite_side(&order.side).to_string(),
        order_type: order_type.to_string(),
        price,
        quantity: state.quantity,
        status: status.to_string(),
    });
}

fn level_for(strategy: &Strategy, level_index: u32) -> Option<&shared_domain::strategy::GridLevel> {
    strategy
        .active_revision
        .as_ref()
        .unwrap_or(&strategy.draft_revision)
        .levels
        .iter()
        .find(|level| level.level_index == level_index)
}

fn should_activate_ordinary_replenishment_orders(
    strategy: &Strategy,
    level_index: u32,
    order: &StrategyRuntimeOrder,
) -> bool {
    if strategy.strategy_type != shared_domain::strategy::StrategyType::OrdinaryGrid
        || !order.status.eq_ignore_ascii_case("Filled")
    {
        return false;
    }
    let Some(first_level) = strategy
        .active_revision
        .as_ref()
        .unwrap_or(&strategy.draft_revision)
        .levels
        .first()
    else {
        return false;
    };
    if first_level.level_index != level_index {
        return false;
    }
    matches!(
        strategy.mode,
        StrategyMode::SpotBuyOnly
            | StrategyMode::SpotSellOnly
            | StrategyMode::FuturesLong
            | StrategyMode::FuturesShort
    )
}

fn activate_ordinary_replenishment_orders(
    strategy: &mut Strategy,
    anchor_level_index: u32,
    side: &str,
) {
    let revision = strategy
        .active_revision
        .as_ref()
        .unwrap_or(&strategy.draft_revision);
    let Some(first_level) = revision.levels.first() else {
        return;
    };
    if first_level.level_index != anchor_level_index {
        return;
    }

    for level in revision.levels.iter().skip(1) {
        let order_id = entry_order_id(&strategy.id, level.level_index);
        if strategy
            .runtime
            .orders
            .iter()
            .any(|order| order.order_id == order_id)
        {
            continue;
        }
        strategy.runtime.orders.push(StrategyRuntimeOrder {
            order_id,
            exchange_order_id: None,
            level_index: Some(level.level_index),
            side: side.to_string(),
            order_type: "Limit".to_string(),
            price: Some(level.entry_price),
            quantity: level.quantity,
            status: "Working".to_string(),
        });
    }
    strategy
        .runtime
        .orders
        .sort_by_key(|candidate| candidate.level_index.unwrap_or(u32::MAX));
}

fn position_mode_for_entry(market: StrategyMarket, mode: StrategyMode, side: &str) -> StrategyMode {
    match market {
        StrategyMarket::Spot => {
            if side.eq_ignore_ascii_case("Sell") {
                StrategyMode::SpotSellOnly
            } else {
                match mode {
                    StrategyMode::SpotBuyOnly => StrategyMode::SpotBuyOnly,
                    _ => StrategyMode::SpotClassic,
                }
            }
        }
        StrategyMarket::FuturesUsdM | StrategyMarket::FuturesCoinM => {
            if side.eq_ignore_ascii_case("Sell") {
                StrategyMode::FuturesShort
            } else {
                StrategyMode::FuturesLong
            }
        }
    }
}

fn is_entry_order(order_id: &str) -> bool {
    order_id.contains("-order-") && !order_id.contains("-stop-close-")
}

fn is_take_profit_order(order_id: &str) -> bool {
    order_id.contains("-tp-")
}

fn is_trailing_monitor_order(order_id: &str) -> bool {
    order_id.contains("-trail-")
}

fn entry_order_id(strategy_id: &str, level_index: u32) -> String {
    format!("{strategy_id}-order-{level_index}")
}

fn take_profit_order_id(strategy_id: &str, level_index: u32) -> String {
    format!("{strategy_id}-tp-{level_index}")
}

fn trailing_order_id(strategy_id: &str, level_index: u32) -> String {
    format!("{strategy_id}-trail-{level_index}")
}

fn take_profit_price(entry_price: Decimal, take_profit_bps: u32, is_short: bool) -> Decimal {
    if is_short {
        entry_price * (Decimal::ONE - Decimal::from(take_profit_bps) / Decimal::from(10_000u32))
    } else {
        entry_price * (Decimal::ONE + Decimal::from(take_profit_bps) / Decimal::from(10_000u32))
    }
}

fn is_short_side(side: &str) -> bool {
    side.eq_ignore_ascii_case("Sell")
}

fn opposite_side(side: &str) -> &'static str {
    if side.eq_ignore_ascii_case("Sell") {
        "Buy"
    } else {
        "Sell"
    }
}

fn close_order_index(order_id: &str) -> Option<usize> {
    order_id.rsplit('-').next()?.parse::<usize>().ok()
}

pub(crate) fn finalize_strategy_after_close(
    strategy: &mut Strategy,
    reference_price: Option<Decimal>,
) {
    let has_pending_close = strategy.runtime.orders.iter().any(|order| {
        order.order_id.contains("-stop-close-")
            && matches!(
                order.status.as_str(),
                "ClosingRequested" | "Placed" | "PartiallyFilled"
            )
    });
    if strategy.status != shared_domain::strategy::StrategyStatus::Stopping
        || !strategy.runtime.positions.is_empty()
        || has_pending_close
    {
        return;
    }

    if pending_rebuild_after_stop(strategy) {
        let revision = strategy
            .active_revision
            .clone()
            .unwrap_or_else(|| strategy.draft_revision.clone());
        if let Ok(mut engine) = StrategyRuntimeEngine::from_runtime_snapshot(
            &strategy.id,
            strategy.market,
            strategy.mode,
            revision,
            strategy.runtime.clone(),
            false,
        ) {
            if engine.resume_with_reference_price(reference_price).is_ok() {
                strategy.runtime = engine.snapshot().clone();
                strategy.status = shared_domain::strategy::StrategyStatus::Running;
                strategy
                    .runtime
                    .events
                    .push(shared_domain::strategy::StrategyRuntimeEvent {
                        event_type: "strategy_rebuilt".to_string(),
                        detail: "strategy rebuilt from current market reference".to_string(),
                        price: reference_price,
                        created_at: Utc::now(),
                    });
                return;
            }
        }
        return;
    }

    strategy.status = shared_domain::strategy::StrategyStatus::Stopped;
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

fn update_price(update: &BinanceExecutionUpdate) -> Option<Decimal> {
    update
        .last_fill_price
        .as_deref()
        .or(update.order_price.as_deref())
        .and_then(|value| value.parse::<Decimal>().ok())
}

fn realized_pnl_local(
    entry_price: Decimal,
    exit_price: Decimal,
    quantity: Decimal,
    is_short: bool,
) -> Decimal {
    if is_short {
        (entry_price - exit_price) * quantity
    } else {
        (exit_price - entry_price) * quantity
    }
}

fn derive_realized_pnl_for_fill(
    strategy: &Strategy,
    order_id: &str,
    level_index: Option<u32>,
    price: Decimal,
    quantity: Decimal,
    exchange_realized_profit: Option<&str>,
) -> Option<Decimal> {
    if let Some(realized) = exchange_realized_profit.and_then(|value| value.parse::<Decimal>().ok())
    {
        return Some(realized);
    }
    if is_entry_order(order_id) {
        return None;
    }

    if order_id.contains("-stop-close-") {
        let state = close_level_state(strategy, order_id)?;
        let closed_quantity = quantity.min(state.quantity);
        return Some(realized_pnl_local(
            state.average_entry_price,
            price,
            closed_quantity,
            state.is_short,
        ));
    }

    let state = level_state(strategy, level_index?)?;
    let closed_quantity = quantity.min(state.quantity);
    Some(realized_pnl_local(
        state.average_entry_price,
        price,
        closed_quantity,
        state.is_short,
    ))
}

fn close_level_state(strategy: &Strategy, order_id: &str) -> Option<LevelState> {
    let target_index = close_order_index(order_id);
    let close_side = strategy
        .runtime
        .orders
        .iter()
        .find(|order| order.order_id == order_id)
        .map(|order| order.side.as_str())?;

    let position = target_index
        .and_then(|index| strategy.runtime.positions.get(index))
        .or_else(|| {
            strategy.runtime.positions.iter().find(|position| {
                matches!(
                    (close_side.eq_ignore_ascii_case("Sell"), position.mode),
                    (
                        true,
                        StrategyMode::SpotClassic
                            | StrategyMode::SpotBuyOnly
                            | StrategyMode::FuturesLong
                    ) | (
                        false,
                        StrategyMode::SpotSellOnly | StrategyMode::FuturesShort
                    )
                )
            })
        })?;

    Some(LevelState {
        quantity: position.quantity,
        average_entry_price: position.average_entry_price,
        is_short: matches!(
            position.mode,
            StrategyMode::SpotSellOnly | StrategyMode::FuturesShort
        ),
    })
}

fn apply_close_fill(
    strategy: &mut Strategy,
    order_id: &str,
    order_side: &str,
    update: &BinanceExecutionUpdate,
) {
    let Some(fill_quantity) = update
        .last_fill_quantity
        .as_deref()
        .and_then(|value| value.parse::<Decimal>().ok())
        .filter(|value| *value > Decimal::ZERO)
    else {
        return;
    };

    if let Some(order) = strategy
        .runtime
        .orders
        .iter_mut()
        .find(|order| order.order_id == order_id)
    {
        order.quantity = (order.quantity - fill_quantity).max(Decimal::ZERO);
    }

    let matched_index = close_order_index(order_id)
        .filter(|index| *index < strategy.runtime.positions.len())
        .or_else(|| {
            strategy.runtime.positions.iter().position(|position| {
                matches!(
                    (order_side.eq_ignore_ascii_case("Sell"), position.mode),
                    (
                        true,
                        StrategyMode::SpotClassic
                            | StrategyMode::SpotBuyOnly
                            | StrategyMode::FuturesLong
                    ) | (
                        false,
                        StrategyMode::SpotSellOnly | StrategyMode::FuturesShort
                    )
                )
            })
        });

    let Some(index) = matched_index else {
        return;
    };
    if strategy.runtime.positions[index].quantity <= fill_quantity {
        strategy.runtime.positions.remove(index);
    } else {
        strategy.runtime.positions[index].quantity -= fill_quantity;
    }
}
