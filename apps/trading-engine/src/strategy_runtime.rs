use chrono::Utc;
use rust_decimal::Decimal;
use shared_domain::strategy::{
    PostTriggerAction, StrategyMarket, StrategyMode, StrategyRevision, StrategyRuntime,
    StrategyRuntimeEvent, StrategyRuntimeFill, StrategyRuntimeOrder, StrategyRuntimePosition,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategyRuntimeError {
    message: String,
}

impl StrategyRuntimeError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for StrategyRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for StrategyRuntimeError {}

#[derive(Debug, Clone)]
struct OpenLevelState {
    level_index: u32,
    entry_price: Decimal,
    quantity: Decimal,
    take_profit_bps: u32,
    trailing_bps: Option<u32>,
    trailing_extreme: Option<Decimal>,
    is_short: bool,
    closing_requested: bool,
}

#[derive(Debug)]
pub struct StrategyRuntimeEngine {
    strategy_id: String,
    market: StrategyMarket,
    mode: StrategyMode,
    revision: StrategyRevision,
    runtime: StrategyRuntime,
    open_levels: Vec<OpenLevelState>,
    running: bool,
    fill_sequence: u64,
}

impl StrategyRuntimeEngine {
    pub fn new(
        strategy_id: &str,
        market: StrategyMarket,
        mode: StrategyMode,
        revision: StrategyRevision,
    ) -> Result<Self, StrategyRuntimeError> {
        for level in &revision.levels {
            if level.trailing_bps.unwrap_or(level.take_profit_bps) > level.take_profit_bps {
                return Err(StrategyRuntimeError::new(format!(
                    "level {} trailing_bps must be less than or equal to take_profit_bps",
                    level.level_index
                )));
            }
        }

        Ok(Self {
            strategy_id: strategy_id.to_string(),
            market,
            mode,
            revision,
            runtime: StrategyRuntime::default(),
            open_levels: Vec::new(),
            running: false,
            fill_sequence: 0,
        })
    }

    pub fn from_runtime_snapshot(
        strategy_id: &str,
        market: StrategyMarket,
        mode: StrategyMode,
        revision: StrategyRevision,
        runtime: StrategyRuntime,
        running: bool,
    ) -> Result<Self, StrategyRuntimeError> {
        let mut engine = Self::new(strategy_id, market, mode, revision.clone())?;
        engine.runtime = runtime.clone();
        engine.running = running;
        engine.fill_sequence = runtime.fills.len() as u64;
        engine.open_levels = derive_open_levels(&runtime, &revision)?;
        if engine.open_levels.is_empty() && !runtime.positions.is_empty() {
            engine.open_levels = runtime
                .positions
                .iter()
                .enumerate()
                .map(|(index, position)| {
                    let template_level = revision
                        .levels
                        .get(index)
                        .or_else(|| revision.levels.first())
                        .ok_or_else(|| StrategyRuntimeError::new("revision level must exist"))?;
                    Ok(OpenLevelState {
                        level_index: template_level.level_index,
                        entry_price: position.average_entry_price,
                        quantity: position.quantity,
                        take_profit_bps: template_level.take_profit_bps,
                        trailing_bps: template_level.trailing_bps,
                        trailing_extreme: latest_trailing_anchor(&runtime, template_level.level_index),
                        is_short: matches!(position.mode, StrategyMode::SpotSellOnly | StrategyMode::FuturesShort),
                        closing_requested: false,
                    })
                })
                .collect::<Result<Vec<_>, StrategyRuntimeError>>()?;
        }
        engine.recompute_position();
        Ok(engine)
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn start(&mut self) -> Result<(), StrategyRuntimeError> {
        self.running = true;
        self.runtime.orders = self.build_active_orders(None);
        push_event(
            &mut self.runtime.events,
            "strategy_started",
            "strategy started",
            None,
        );
        Ok(())
    }

    pub fn fill_entry(&mut self, level_index: u32) -> Result<(), StrategyRuntimeError> {
        let level = self.level_for(level_index)?.clone();
        let default_side = initial_entry_side(
            self.mode,
            level.level_index,
            level.entry_price,
            self.reference_price(None),
        )
        .unwrap_or("Buy")
        .to_string();
        let entry_order = self.entry_order_mut(level_index);
        let side = entry_order
            .as_ref()
            .map(|order| order.side.clone())
            .unwrap_or(default_side);
        if let Some(order) = entry_order {
            order.status = "Filled".to_string();
        }

        self.fill_sequence += 1;
        self.runtime.fills.push(StrategyRuntimeFill {
            fill_id: format!("{}-fill-{}", self.strategy_id, self.fill_sequence),
            order_id: Some(entry_order_id(&self.strategy_id, level_index)),
            level_index: Some(level_index),
            fill_type: "Entry".to_string(),
            price: level.entry_price,
            quantity: level.quantity,
            realized_pnl: None,
            fee_amount: None,
            fee_asset: None,
        });
        self.open_levels.retain(|state| state.level_index != level_index);
        self.open_levels.push(OpenLevelState {
            level_index,
            entry_price: level.entry_price,
            quantity: level.quantity,
            take_profit_bps: level.take_profit_bps,
            trailing_bps: level.trailing_bps,
            trailing_extreme: None,
            is_short: side.eq_ignore_ascii_case("Sell"),
            closing_requested: false,
        });
        self.upsert_exit_order(level_index, level.entry_price, level.quantity, level.take_profit_bps, level.trailing_bps, side.eq_ignore_ascii_case("Sell"));
        self.recompute_position();
        push_event(
            &mut self.runtime.events,
            "entry_fill",
            "entry fill recorded",
            Some(level.entry_price),
        );
        Ok(())
    }

    pub fn fill_take_profit(
        &mut self,
        level_index: u32,
        exit_price: Decimal,
    ) -> Result<(), StrategyRuntimeError> {
        if !self.open_levels.iter().any(|state| state.level_index == level_index) {
            return Err(StrategyRuntimeError::new("open level not found"));
        }
        let _ = self.close_level(level_index, exit_price, "maker_take_profit", true);
        self.open_levels.retain(|state| state.level_index != level_index);
        self.recompute_position();
        Ok(())
    }

    pub fn on_price(
        &mut self,
        price: Decimal,
    ) -> Result<Vec<StrategyRuntimeEvent>, StrategyRuntimeError> {
        if !self.running {
            return Ok(Vec::new());
        }

        if self.should_trigger_overall_take_profit(price) {
            return Ok(vec![self.trigger_post_take_profit(price)]);
        }
        if self.should_trigger_overall_stop_loss(price) {
            return Ok(vec![self.trigger_post_stop_loss(price)]);
        }

        let mut emitted = Vec::new();
        let mut pending_closures = Vec::new();

        for state in &mut self.open_levels {
            if state.closing_requested {
                continue;
            }
            let tp_price = take_profit_price(state.entry_price, state.take_profit_bps, state.is_short);
            let Some(trailing_bps) = state.trailing_bps else {
                continue;
            };
            if let Some(extreme) = state.trailing_extreme {
                if state.is_short {
                    let new_low = extreme.min(price);
                    state.trailing_extreme = Some(new_low);
                    push_event(
                        &mut self.runtime.events,
                        &format!("trailing_anchor_{}", state.level_index),
                        "trailing anchor updated",
                        Some(new_low),
                    );
                    let retrace_price = new_low * short_trailing_factor(trailing_bps);
                    if price >= retrace_price {
                        pending_closures.push((state.level_index, price, "taker_trailing_take_profit"));
                    }
                } else {
                    let new_high = extreme.max(price);
                    state.trailing_extreme = Some(new_high);
                    push_event(
                        &mut self.runtime.events,
                        &format!("trailing_anchor_{}", state.level_index),
                        "trailing anchor updated",
                        Some(new_high),
                    );
                    let retrace_price = new_high * trailing_factor(trailing_bps);
                    if price <= retrace_price {
                        pending_closures.push((state.level_index, price, "taker_trailing_take_profit"));
                    }
                }
            } else if price_reaches_take_profit(price, tp_price, state.is_short) {
                state.trailing_extreme = Some(price);
                push_event(
                    &mut self.runtime.events,
                    &format!("trailing_anchor_{}", state.level_index),
                    "trailing anchor armed",
                    Some(price),
                );
            }
        }

        if !pending_closures.is_empty() {
            for (level_index, exit_price, event_type) in &pending_closures {
                emitted.push(self.request_level_close(*level_index, *exit_price, event_type));
            }
        }

        Ok(emitted)
    }

    pub fn pause(&mut self) {
        self.running = false;
        for order in &mut self.runtime.orders {
            if matches!(order.status.as_str(), "Working" | "Placed" | "Monitoring" | "Armed") {
                order.status = "Canceled".to_string();
            }
        }
        push_event(
            &mut self.runtime.events,
            "strategy_paused",
            "strategy paused",
            None,
        );
    }

    pub fn resume(&mut self) -> Result<(), StrategyRuntimeError> {
        self.resume_with_reference_price(None)
    }

    pub fn resume_with_reference_price(
        &mut self,
        reference_price: Option<Decimal>,
    ) -> Result<(), StrategyRuntimeError> {
        self.running = true;
        self.runtime.orders = self.build_active_orders(reference_price);
        push_event(
            &mut self.runtime.events,
            "strategy_resumed",
            "strategy resumed",
            None,
        );
        Ok(())
    }

    pub fn snapshot(&self) -> &StrategyRuntime {
        &self.runtime
    }

    fn should_trigger_overall_take_profit(&self, price: Decimal) -> bool {
        let Some(overall_bps) = self.revision.overall_take_profit_bps else {
            return false;
        };
        let Some((cost_basis, unrealized_pnl)) = self.overall_profit_state(price) else {
            return false;
        };
        unrealized_pnl >= cost_basis * Decimal::from(overall_bps) / Decimal::from(10_000u32)
    }

    fn should_trigger_overall_stop_loss(&self, price: Decimal) -> bool {
        let Some(overall_bps) = self.revision.overall_stop_loss_bps else {
            return false;
        };
        let Some((cost_basis, unrealized_pnl)) = self.overall_profit_state(price) else {
            return false;
        };
        unrealized_pnl <= -(cost_basis * Decimal::from(overall_bps) / Decimal::from(10_000u32))
    }

    fn trigger_post_take_profit(&mut self, price: Decimal) -> StrategyRuntimeEvent {
        self.running = false;
        match self.revision.post_trigger_action {
            PostTriggerAction::Stop => push_event(
                &mut self.runtime.events,
                "overall_take_profit_stop",
                "overall_take_profit_stop",
                Some(price),
            ),
            PostTriggerAction::Rebuild => push_event(
                &mut self.runtime.events,
                "overall_take_profit_rebuild",
                "overall_take_profit_rebuild",
                Some(price),
            ),
        }
    }

    fn trigger_post_stop_loss(&mut self, price: Decimal) -> StrategyRuntimeEvent {
        self.running = false;
        match self.revision.post_trigger_action {
            PostTriggerAction::Stop => push_event(
                &mut self.runtime.events,
                "overall_stop_loss_stop",
                "overall_stop_loss_stop",
                Some(price),
            ),
            PostTriggerAction::Rebuild => push_event(
                &mut self.runtime.events,
                "overall_stop_loss_rebuild",
                "overall_stop_loss_rebuild",
                Some(price),
            ),
        }
    }

    fn overall_profit_state(&self, price: Decimal) -> Option<(Decimal, Decimal)> {
        if self.open_levels.is_empty() {
            return None;
        }

        let cost_basis = self.open_levels.iter().fold(Decimal::ZERO, |acc, level| {
            acc + (level.entry_price * level.quantity)
        });
        if cost_basis <= Decimal::ZERO {
            return None;
        }

        let unrealized_pnl = self.open_levels.iter().fold(Decimal::ZERO, |acc, level| {
            let pnl = if level.is_short {
                (level.entry_price - price) * level.quantity
            } else {
                (price - level.entry_price) * level.quantity
            };
            acc + pnl
        });
        Some((cost_basis, unrealized_pnl))
    }

    fn close_level(
        &mut self,
        level_index: u32,
        price: Decimal,
        event_type: &str,
        reopen_entry: bool,
    ) -> StrategyRuntimeEvent {
        let state = self
            .open_levels
            .iter()
            .find(|state| state.level_index == level_index)
            .cloned()
            .expect("open level must exist");
        self.fill_sequence += 1;
        self.runtime.fills.push(StrategyRuntimeFill {
            fill_id: format!("{}-fill-{}", self.strategy_id, self.fill_sequence),
            order_id: Some(active_exit_order_id(&self.strategy_id, level_index, state.trailing_bps)),
            level_index: Some(level_index),
            fill_type: "Exit".to_string(),
            price,
            quantity: state.quantity,
            realized_pnl: Some(realized_pnl(state.entry_price, price, state.quantity, state.is_short)),
            fee_amount: None,
            fee_asset: None,
        });
        if let Some(order) = self.runtime.orders.iter_mut().find(|order| {
            order.level_index == Some(level_index)
                && (is_take_profit_order(order) || is_trailing_monitor_order(order))
        }) {
            order.status = "Filled".to_string();
        }
        if reopen_entry {
            self.activate_entry_order(level_index);
        }
        self.recompute_position();
        push_event(&mut self.runtime.events, event_type, event_type, Some(price))
    }

    fn request_level_close(
        &mut self,
        level_index: u32,
        price: Decimal,
        event_type: &str,
    ) -> StrategyRuntimeEvent {
        let state = self
            .open_levels
            .iter_mut()
            .find(|state| state.level_index == level_index)
            .expect("open level must exist");
        state.closing_requested = true;
        let order_id = active_exit_order_id(&self.strategy_id, level_index, state.trailing_bps);
        if let Some(order) = self.runtime.orders.iter_mut().find(|order| order.order_id == order_id) {
            order.exchange_order_id = None;
            order.side = exit_side(state.is_short).to_string();
            order.order_type = "Market".to_string();
            order.price = None;
            order.quantity = state.quantity;
            order.status = "ClosingRequested".to_string();
        }
        push_event(&mut self.runtime.events, event_type, event_type, Some(price))
    }

    fn recompute_position(&mut self) {
        if self.open_levels.is_empty() {
            self.runtime.positions.clear();
            return;
        }

        let mut long_quantity = Decimal::ZERO;
        let mut long_cost = Decimal::ZERO;
        let mut short_quantity = Decimal::ZERO;
        let mut short_cost = Decimal::ZERO;

        for level in &self.open_levels {
            if level.is_short {
                short_quantity += level.quantity;
                short_cost += level.entry_price * level.quantity;
            } else {
                long_quantity += level.quantity;
                long_cost += level.entry_price * level.quantity;
            }
        }

        let mut positions = Vec::new();
        if long_quantity > Decimal::ZERO {
            positions.push(StrategyRuntimePosition {
                market: self.market,
                mode: if matches!(self.market, StrategyMarket::Spot) {
                    StrategyMode::SpotBuyOnly
                } else {
                    StrategyMode::FuturesLong
                },
                quantity: long_quantity,
                average_entry_price: long_cost / long_quantity,
            });
        }
        if short_quantity > Decimal::ZERO {
            positions.push(StrategyRuntimePosition {
                market: self.market,
                mode: if matches!(self.market, StrategyMarket::Spot) {
                    StrategyMode::SpotSellOnly
                } else {
                    StrategyMode::FuturesShort
                },
                quantity: short_quantity,
                average_entry_price: short_cost / short_quantity,
            });
        }
        self.runtime.positions = positions;
    }

    fn build_active_orders(&self, reference_price: Option<Decimal>) -> Vec<StrategyRuntimeOrder> {
        let mut orders = Vec::new();
        let reference_price = self.reference_price(reference_price);
        for level in &self.revision.levels {
            if self.open_levels.iter().any(|state| state.level_index == level.level_index) {
                continue;
            }
            let Some(side) = initial_entry_side(
                self.mode,
                level.level_index,
                level.entry_price,
                reference_price,
            ) else {
                continue;
            };
            orders.push(StrategyRuntimeOrder {
                order_id: entry_order_id(&self.strategy_id, level.level_index),
                exchange_order_id: None,
                level_index: Some(level.level_index),
                side: side.to_string(),
                order_type: "Limit".to_string(),
                price: Some(level.entry_price),
                quantity: level.quantity,
                status: "Working".to_string(),
            });
        }
        for state in &self.open_levels {
            if let Some(trailing_bps) = state.trailing_bps {
                orders.push(StrategyRuntimeOrder {
                    order_id: trailing_order_id(&self.strategy_id, state.level_index),
                    exchange_order_id: None,
                    level_index: Some(state.level_index),
                    side: exit_side(state.is_short).to_string(),
                    order_type: "TrailMonitor".to_string(),
                    price: Some(take_profit_price(state.entry_price, state.take_profit_bps, state.is_short)),
                    quantity: state.quantity,
                    status: if state.trailing_extreme.is_some() { "Armed".to_string() } else { "Monitoring".to_string() },
                });
                let _ = trailing_bps;
            } else {
                orders.push(StrategyRuntimeOrder {
                    order_id: take_profit_order_id(&self.strategy_id, state.level_index),
                    exchange_order_id: None,
                    level_index: Some(state.level_index),
                    side: exit_side(state.is_short).to_string(),
                    order_type: "Limit".to_string(),
                    price: Some(take_profit_price(state.entry_price, state.take_profit_bps, state.is_short)),
                    quantity: state.quantity,
                    status: "Working".to_string(),
                });
            }
        }
        orders.sort_by_key(|order| order.level_index.unwrap_or(u32::MAX));
        orders
    }

    fn level_for(&self, level_index: u32) -> Result<&shared_domain::strategy::GridLevel, StrategyRuntimeError> {
        self.revision
            .levels
            .iter()
            .find(|level| level.level_index == level_index)
            .ok_or_else(|| StrategyRuntimeError::new("grid level not found"))
    }

    fn entry_order_mut(&mut self, level_index: u32) -> Option<&mut StrategyRuntimeOrder> {
        self.runtime.orders.iter_mut().find(|order| {
            order.level_index == Some(level_index) && is_entry_order(order)
        })
    }

    fn upsert_exit_order(
        &mut self,
        level_index: u32,
        entry_price: Decimal,
        quantity: Decimal,
        take_profit_bps: u32,
        trailing_bps: Option<u32>,
        is_short: bool,
    ) {
        let order_id = active_exit_order_id(&self.strategy_id, level_index, trailing_bps);
        let order_type = if trailing_bps.is_some() { "TrailMonitor" } else { "Limit" };
        let price = Some(take_profit_price(entry_price, take_profit_bps, is_short));
        let status = if trailing_bps.is_some() { "Monitoring" } else { "Working" };
        if let Some(order) = self.runtime.orders.iter_mut().find(|order| order.order_id == order_id) {
            order.exchange_order_id = None;
            order.level_index = Some(level_index);
            order.side = exit_side(is_short).to_string();
            order.order_type = order_type.to_string();
            order.price = price;
            order.quantity = quantity;
            order.status = status.to_string();
            return;
        }
        self.runtime.orders.push(StrategyRuntimeOrder {
            order_id,
            exchange_order_id: None,
            level_index: Some(level_index),
            side: exit_side(is_short).to_string(),
            order_type: order_type.to_string(),
            price,
            quantity,
            status: status.to_string(),
        });
    }

    fn activate_entry_order(&mut self, level_index: u32) {
        let Ok(level) = self.level_for(level_index).cloned() else {
            return;
        };
        let Some(side) = initial_entry_side(
            self.mode,
            level.level_index,
            level.entry_price,
            self.reference_price(None),
        ) else {
            return;
        };
        if let Some(order) = self.entry_order_mut(level_index) {
            order.exchange_order_id = None;
            order.side = side.to_string();
            order.order_type = "Limit".to_string();
            order.price = Some(level.entry_price);
            order.quantity = level.quantity;
            order.status = "Working".to_string();
            return;
        }
        self.runtime.orders.push(StrategyRuntimeOrder {
            order_id: entry_order_id(&self.strategy_id, level_index),
            exchange_order_id: None,
            level_index: Some(level_index),
            side: side.to_string(),
            order_type: "Limit".to_string(),
            price: Some(level.entry_price),
            quantity: level.quantity,
            status: "Working".to_string(),
        });
    }

    fn reference_price(&self, override_price: Option<Decimal>) -> Decimal {
        override_price
            .or_else(|| {
                self.runtime
                    .events
                    .iter()
                    .rev()
                    .find_map(|event| event.price)
            })
            .or_else(|| {
                let total_quantity = self
                    .runtime
                    .positions
                    .iter()
                    .fold(Decimal::ZERO, |acc, position| acc + position.quantity);
                (total_quantity > Decimal::ZERO).then(|| {
                    self.runtime.positions.iter().fold(Decimal::ZERO, |acc, position| {
                        acc + (position.average_entry_price * position.quantity)
                    }) / total_quantity
                })
            })
            .or_else(|| self.runtime.fills.iter().rev().find_map(|fill| (fill.price > Decimal::ZERO).then_some(fill.price)))
            .unwrap_or_else(|| effective_reference_price(&self.revision))
    }
}

fn derive_open_levels(
    runtime: &StrategyRuntime,
    revision: &StrategyRevision,
) -> Result<Vec<OpenLevelState>, StrategyRuntimeError> {
    let mut items = Vec::new();
    for order in &runtime.orders {
        if !(is_take_profit_order(order) || is_trailing_monitor_order(order)) {
            continue;
        }
        if !matches!(order.status.as_str(), "Working" | "Placed" | "Monitoring" | "Armed") {
            continue;
        }
        let Some(level_index) = order.level_index else {
            continue;
        };
        let level = revision
            .levels
            .iter()
            .find(|level| level.level_index == level_index)
            .ok_or_else(|| StrategyRuntimeError::new("revision level must exist"))?;
        items.push(OpenLevelState {
            level_index,
            entry_price: level.entry_price,
            quantity: order.quantity,
            take_profit_bps: level.take_profit_bps,
            trailing_bps: level.trailing_bps,
            trailing_extreme: latest_trailing_anchor(runtime, level_index),
            is_short: order.side.eq_ignore_ascii_case("Buy"),
            closing_requested: matches!(order.status.as_str(), "ClosingRequested" | "PartiallyFilled")
                || order.order_type.eq_ignore_ascii_case("Market"),
        });
    }
    Ok(items)
}

fn latest_trailing_anchor(runtime: &StrategyRuntime, level_index: u32) -> Option<Decimal> {
    runtime
        .events
        .iter()
        .rev()
        .find(|event| event.event_type == format!("trailing_anchor_{level_index}"))
        .and_then(|event| event.price)
}

fn effective_reference_price(revision: &StrategyRevision) -> Decimal {
    match revision.levels.as_slice() {
        [] => Decimal::ZERO,
        [single] => single.entry_price,
        levels => {
            let first = levels.first().expect("first level").entry_price;
            let last = levels.last().expect("last level").entry_price;
            (first + last) / Decimal::from(2u32)
        }
    }
}

fn initial_entry_side(
    mode: StrategyMode,
    level_index: u32,
    level_price: Decimal,
    reference_price: Decimal,
) -> Option<&'static str> {
    match mode {
        StrategyMode::SpotClassic => {
            if level_price < reference_price {
                Some("Buy")
            } else if level_price > reference_price {
                Some("Sell")
            } else {
                None
            }
        }
        StrategyMode::FuturesNeutral => {
            if level_price < reference_price {
                Some("Buy")
            } else if level_price > reference_price {
                Some("Sell")
            } else if level_index % 2 == 0 {
                Some("Buy")
            } else {
                Some("Sell")
            }
        }
        StrategyMode::SpotBuyOnly | StrategyMode::FuturesLong => {
            if level_price <= reference_price { Some("Buy") } else { None }
        }
        StrategyMode::SpotSellOnly | StrategyMode::FuturesShort => {
            if level_price >= reference_price { Some("Sell") } else { None }
        }
    }
}

fn take_profit_price(entry_price: Decimal, take_profit_bps: u32, is_short: bool) -> Decimal {
    if is_short {
        entry_price * (Decimal::ONE - Decimal::from(take_profit_bps) / Decimal::from(10_000u32))
    } else {
        entry_price * (Decimal::ONE + Decimal::from(take_profit_bps) / Decimal::from(10_000u32))
    }
}

fn trailing_factor(bps: u32) -> Decimal {
    Decimal::ONE - Decimal::from(bps) / Decimal::from(10_000u32)
}

fn short_trailing_factor(bps: u32) -> Decimal {
    Decimal::ONE + Decimal::from(bps) / Decimal::from(10_000u32)
}

fn price_reaches_take_profit(price: Decimal, threshold: Decimal, is_short: bool) -> bool {
    if is_short {
        price <= threshold
    } else {
        price >= threshold
    }
}

fn realized_pnl(entry_price: Decimal, exit_price: Decimal, quantity: Decimal, is_short: bool) -> Decimal {
    if is_short {
        (entry_price - exit_price) * quantity
    } else {
        (exit_price - entry_price) * quantity
    }
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

fn active_exit_order_id(strategy_id: &str, level_index: u32, trailing_bps: Option<u32>) -> String {
    if trailing_bps.is_some() {
        trailing_order_id(strategy_id, level_index)
    } else {
        take_profit_order_id(strategy_id, level_index)
    }
}

fn is_entry_order(order: &StrategyRuntimeOrder) -> bool {
    order.order_id.contains("-order-") && !order.order_id.contains("-stop-close-")
}

fn is_take_profit_order(order: &StrategyRuntimeOrder) -> bool {
    order.order_id.contains("-tp-")
}

fn is_trailing_monitor_order(order: &StrategyRuntimeOrder) -> bool {
    order.order_id.contains("-trail-")
}

fn exit_side(is_short: bool) -> &'static str {
    if is_short { "Buy" } else { "Sell" }
}

fn push_event(
    events: &mut Vec<StrategyRuntimeEvent>,
    event_type: &str,
    detail: &str,
    price: Option<Decimal>,
) -> StrategyRuntimeEvent {
    let event = StrategyRuntimeEvent {
        event_type: event_type.to_string(),
        detail: detail.to_string(),
        price,
        created_at: Utc::now(),
    };
    events.push(event.clone());
    event
}
