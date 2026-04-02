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

    pub fn start(&mut self) -> Result<(), StrategyRuntimeError> {
        self.running = true;
        self.runtime.orders = self.build_working_orders();
        push_event(&mut self.runtime.events, "strategy_started", "strategy started", None);
        Ok(())
    }

    pub fn fill_entry(&mut self, level_index: u32) -> Result<(), StrategyRuntimeError> {
        let level = self
            .revision
            .levels
            .iter()
            .find(|level| level.level_index == level_index)
            .ok_or_else(|| StrategyRuntimeError::new("grid level not found"))?
            .clone();

        if let Some(order) = self
            .runtime
            .orders
            .iter_mut()
            .find(|order| order.level_index == Some(level_index))
        {
            order.status = "Filled".to_string();
        }

        self.fill_sequence += 1;
        self.runtime.fills.push(StrategyRuntimeFill {
            fill_id: format!("{}-fill-{}", self.strategy_id, self.fill_sequence),
            order_id: Some(format!("{}-order-{}", self.strategy_id, level_index)),
            level_index: Some(level_index),
            fill_type: "Entry".to_string(),
            price: level.entry_price,
            quantity: level.quantity,
            realized_pnl: None,
            fee_amount: None,
            fee_asset: None,
        });
        self.open_levels.push(OpenLevelState {
            level_index,
            entry_price: level.entry_price,
            quantity: level.quantity,
            take_profit_bps: level.take_profit_bps,
            trailing_bps: level.trailing_bps,
            trailing_extreme: None,
            is_short: level_is_short(self.mode, level.level_index),
        });
        self.recompute_position();
        push_event(&mut self.runtime.events, "entry_fill", "entry fill recorded", Some(level.entry_price));
        Ok(())
    }

    pub fn on_price(&mut self, price: Decimal) -> Result<Vec<StrategyRuntimeEvent>, StrategyRuntimeError> {
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
            let tp_price = take_profit_price(state.entry_price, state.take_profit_bps, state.is_short);
            match state.trailing_bps {
                Some(trailing_bps) => {
                    if let Some(extreme) = state.trailing_extreme {
                        if state.is_short {
                            let new_low = extreme.min(price);
                            state.trailing_extreme = Some(new_low);
                            let retrace_price = new_low * short_trailing_factor(trailing_bps);
                            if price >= retrace_price {
                                pending_closures.push((state.level_index, price, "taker_trailing_take_profit"));
                            }
                        } else {
                            let new_high = extreme.max(price);
                            state.trailing_extreme = Some(new_high);
                            let retrace_price = new_high * trailing_factor(trailing_bps);
                            if price <= retrace_price {
                                pending_closures.push((state.level_index, price, "taker_trailing_take_profit"));
                            }
                        }
                    } else if price_reaches_take_profit(price, tp_price, state.is_short) {
                        state.trailing_extreme = Some(price);
                    }
                }
                None => {
                    if price_reaches_take_profit(price, tp_price, state.is_short) {
                        pending_closures.push((state.level_index, tp_price, "maker_take_profit"));
                    }
                }
            }
        }

        if !pending_closures.is_empty() {
            for (level_index, exit_price, event_type) in &pending_closures {
                emitted.push(self.close_level(*level_index, *exit_price, event_type));
            }
            self.open_levels.retain(|state| {
                !pending_closures
                    .iter()
                    .any(|(index, _, _)| index == &state.level_index)
            });
            self.recompute_position();
        }

        Ok(emitted)
    }

    pub fn pause(&mut self) {
        self.running = false;
        for order in &mut self.runtime.orders {
            if order.status == "Working" {
                order.status = "Canceled".to_string();
            }
        }
        push_event(&mut self.runtime.events, "strategy_paused", "strategy paused", None);
    }

    pub fn resume(&mut self) -> Result<(), StrategyRuntimeError> {
        self.running = true;
        self.runtime.orders = self.build_working_orders();
        push_event(&mut self.runtime.events, "strategy_resumed", "strategy resumed", None);
        Ok(())
    }

    pub fn snapshot(&self) -> &StrategyRuntime {
        &self.runtime
    }

    fn should_trigger_overall_take_profit(&self, price: Decimal) -> bool {
        let Some(overall_bps) = self.revision.overall_take_profit_bps else {
            return false;
        };
        let Some(position) = self.runtime.positions.first() else {
            return false;
        };
        let Some(is_short) = overall_exposure_is_short(self.mode, &self.open_levels) else {
            return false;
        };
        price_reaches_take_profit(
            price,
            take_profit_price(position.average_entry_price, overall_bps, is_short),
            is_short,
        )
    }

    fn should_trigger_overall_stop_loss(&self, price: Decimal) -> bool {
        let Some(overall_bps) = self.revision.overall_stop_loss_bps else {
            return false;
        };
        let Some(position) = self.runtime.positions.first() else {
            return false;
        };
        let Some(is_short) = overall_exposure_is_short(self.mode, &self.open_levels) else {
            return false;
        };
        price_hits_stop_loss(
            price,
            stop_loss_price(position.average_entry_price, overall_bps, is_short),
            is_short,
        )
    }

    fn trigger_post_take_profit(&mut self, price: Decimal) -> StrategyRuntimeEvent {
        match self.revision.post_trigger_action {
            PostTriggerAction::Stop => self.close_all(price, "overall_take_profit_stop", false),
            PostTriggerAction::Rebuild => self.close_all(price, "overall_take_profit_rebuild", true),
        }
    }

    fn trigger_post_stop_loss(&mut self, price: Decimal) -> StrategyRuntimeEvent {
        match self.revision.post_trigger_action {
            PostTriggerAction::Stop => self.close_all(price, "overall_stop_loss_stop", false),
            PostTriggerAction::Rebuild => self.close_all(price, "overall_stop_loss_rebuild", true),
        }
    }

    fn close_all(&mut self, price: Decimal, event_type: &str, rebuild: bool) -> StrategyRuntimeEvent {
        let closing_levels = self.open_levels.clone();
        for state in closing_levels {
            let _ = self.close_level(state.level_index, price, event_type);
        }
        self.open_levels.clear();
        self.runtime.positions.clear();
        for order in &mut self.runtime.orders {
            if order.status == "Working" {
                order.status = "Canceled".to_string();
            }
        }
        if rebuild {
            self.runtime.orders = self.build_working_orders();
            self.running = true;
        } else {
            self.running = false;
        }

        push_event(&mut self.runtime.events, event_type, event_type, Some(price))
    }

    fn close_level(&mut self, level_index: u32, price: Decimal, event_type: &str) -> StrategyRuntimeEvent {
        let state = self
            .open_levels
            .iter()
            .find(|state| state.level_index == level_index)
            .cloned()
            .expect("open level must exist");
        self.fill_sequence += 1;
        self.runtime.fills.push(StrategyRuntimeFill {
            fill_id: format!("{}-fill-{}", self.strategy_id, self.fill_sequence),
            order_id: Some(format!("{}-order-{}", self.strategy_id, level_index)),
            level_index: Some(level_index),
            fill_type: "Exit".to_string(),
            price,
            quantity: state.quantity,
            realized_pnl: Some(realized_pnl(state.entry_price, price, state.quantity, state.is_short)),
            fee_amount: None,
            fee_asset: None,
        });
        if let Some(order) = self
            .runtime
            .orders
            .iter_mut()
            .find(|order| order.level_index == Some(level_index))
        {
            order.status = "Filled".to_string();
        }

        push_event(&mut self.runtime.events, event_type, event_type, Some(price))
    }

    fn recompute_position(&mut self) {
        if self.open_levels.is_empty() {
            self.runtime.positions.clear();
            return;
        }

        if self.mode == StrategyMode::FuturesNeutral {
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
                    mode: StrategyMode::FuturesLong,
                    quantity: long_quantity,
                    average_entry_price: long_cost / long_quantity,
                });
            }
            if short_quantity > Decimal::ZERO {
                positions.push(StrategyRuntimePosition {
                    market: self.market,
                    mode: StrategyMode::FuturesShort,
                    quantity: short_quantity,
                    average_entry_price: short_cost / short_quantity,
                });
            }
            self.runtime.positions = positions;
            return;
        }

        let total_quantity = self
            .open_levels
            .iter()
            .fold(Decimal::ZERO, |acc, level| acc + level.quantity);
        let weighted_cost = self
            .open_levels
            .iter()
            .fold(Decimal::ZERO, |acc, level| acc + level.entry_price * level.quantity);

        self.runtime.positions = vec![StrategyRuntimePosition {
            market: self.market,
            mode: if overall_exposure_is_short(self.mode, &self.open_levels) == Some(true) {
                StrategyMode::FuturesShort
            } else {
                self.mode
            },
            quantity: total_quantity,
            average_entry_price: weighted_cost / total_quantity,
        }];
    }

    fn build_working_orders(&self) -> Vec<StrategyRuntimeOrder> {
        self.revision
            .levels
            .iter()
            .map(|level| StrategyRuntimeOrder {
                order_id: format!("{}-order-{}", self.strategy_id, level.level_index),
                level_index: Some(level.level_index),
                side: entry_side(self.mode, level.level_index).to_string(),
                order_type: "Limit".to_string(),
                price: Some(level.entry_price),
                quantity: level.quantity,
                status: "Working".to_string(),
            })
            .collect()
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

fn stop_loss_price(entry_price: Decimal, stop_loss_bps: u32, is_short: bool) -> Decimal {
    if is_short {
        entry_price * (Decimal::ONE + Decimal::from(stop_loss_bps) / Decimal::from(10_000u32))
    } else {
        entry_price * (Decimal::ONE - Decimal::from(stop_loss_bps) / Decimal::from(10_000u32))
    }
}

fn price_reaches_take_profit(price: Decimal, threshold: Decimal, is_short: bool) -> bool {
    if is_short {
        price <= threshold
    } else {
        price >= threshold
    }
}

fn price_hits_stop_loss(price: Decimal, threshold: Decimal, is_short: bool) -> bool {
    if is_short {
        price >= threshold
    } else {
        price <= threshold
    }
}

fn realized_pnl(entry_price: Decimal, exit_price: Decimal, quantity: Decimal, is_short: bool) -> Decimal {
    if is_short {
        (entry_price - exit_price) * quantity
    } else {
        (exit_price - entry_price) * quantity
    }
}

fn entry_side(mode: StrategyMode, level_index: u32) -> &'static str {
    if level_is_short(mode, level_index) {
        "Sell"
    } else {
        "Buy"
    }
}

fn level_is_short(mode: StrategyMode, level_index: u32) -> bool {
    match mode {
        StrategyMode::SpotClassic | StrategyMode::SpotBuyOnly | StrategyMode::FuturesLong => false,
        StrategyMode::SpotSellOnly | StrategyMode::FuturesShort => true,
        StrategyMode::FuturesNeutral => level_index % 2 == 1,
    }
}

fn overall_exposure_is_short(mode: StrategyMode, open_levels: &[OpenLevelState]) -> Option<bool> {
    let mut has_short = false;
    let mut has_long = false;
    for level in open_levels {
        if level.is_short {
            has_short = true;
        } else {
            has_long = true;
        }
    }
    if has_short && has_long {
        return None;
    }
    if has_short {
        Some(true)
    } else if has_long {
        Some(false)
    } else {
        Some(matches!(mode, StrategyMode::SpotSellOnly | StrategyMode::FuturesShort))
    }
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
