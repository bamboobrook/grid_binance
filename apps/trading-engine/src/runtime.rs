use rust_decimal::Decimal;

use crate::grid_builder::GridPlan;
use crate::stop_loss::OverallStopLoss;
use crate::take_profit::{
    take_profit_price_from_bps, take_profit_price_from_percent, MakerTakeProfit, OverallTakeProfit,
    TrailingTakeProfit,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridMode {
    SpotGrid,
    FuturesLong,
    FuturesShort,
    ClassicBilateralSpot,
    ClassicBilateralFutures,
}

impl GridMode {
    pub fn is_ordinary(self) -> bool {
        matches!(
            self,
            Self::SpotGrid | Self::FuturesLong | Self::FuturesShort
        )
    }

    pub fn is_classic_bilateral(self) -> bool {
        matches!(
            self,
            Self::ClassicBilateralSpot | Self::ClassicBilateralFutures
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeStatus {
    Running,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridRuntimeConfig {
    pub mode: GridMode,
    pub plan: GridPlan,
    pub quantity: Decimal,
    pub ordinary_take_profit_bps: u32,
    pub maker_take_profit: Option<MakerTakeProfit>,
    pub trailing_take_profit: Option<TrailingTakeProfit>,
    pub overall_take_profit: Option<OverallTakeProfit>,
    pub overall_stop_loss: Option<OverallStopLoss>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    pub entry_price: Decimal,
    pub quantity: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeOrderKind {
    Entry,
    TakeProfit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeOrder {
    kind: RuntimeOrderKind,
    level_index: u32,
    price: Decimal,
    quantity: Decimal,
}

impl RuntimeOrder {
    pub fn kind(&self) -> RuntimeOrderKind {
        self.kind
    }

    pub fn level_index(&self) -> u32 {
        self.level_index
    }

    pub fn price(&self) -> Decimal {
        self.price
    }

    pub fn quantity(&self) -> Decimal {
        self.quantity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEvent {
    reason: &'static str,
    exit_price: Option<Decimal>,
}

impl RuntimeEvent {
    pub fn reason(&self) -> &'static str {
        self.reason
    }

    pub fn exit_price(&self) -> Option<Decimal> {
        self.exit_price
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridRuntimeError {
    message: String,
}

impl GridRuntimeError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for GridRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for GridRuntimeError {}

pub struct GridRuntime {
    mode: GridMode,
    plan: GridPlan,
    default_quantity: Decimal,
    ordinary_take_profit_bps: u32,
    maker_take_profit: Option<MakerTakeProfit>,
    trailing_take_profit: Option<TrailingTakeProfit>,
    overall_take_profit: Option<OverallTakeProfit>,
    overall_stop_loss: Option<OverallStopLoss>,
    status: RuntimeStatus,
    position: Option<Position>,
    realized_pnl: Decimal,
    last_price: Option<Decimal>,
    trailing_anchor: Option<Decimal>,
    ordinary_orders: Vec<RuntimeOrder>,
    started: bool,
}

impl GridRuntime {
    pub fn new(config: GridRuntimeConfig) -> Result<Self, GridRuntimeError> {
        validate_supported_mode(config.mode)?;
        validate_supported_mode(config.plan.mode)?;

        if config.mode != config.plan.mode {
            return Err(GridRuntimeError::new(
                "runtime mode must match grid plan mode",
            ));
        }

        validate_plan_shape(&config.plan)?;

        if config.quantity <= Decimal::ZERO {
            return Err(GridRuntimeError::new("default quantity must be positive"));
        }

        Ok(Self {
            mode: config.mode,
            plan: config.plan,
            default_quantity: config.quantity,
            ordinary_take_profit_bps: config.ordinary_take_profit_bps,
            maker_take_profit: config.maker_take_profit,
            trailing_take_profit: config.trailing_take_profit,
            overall_take_profit: config.overall_take_profit,
            overall_stop_loss: config.overall_stop_loss,
            status: RuntimeStatus::Running,
            position: None,
            realized_pnl: Decimal::ZERO,
            last_price: None,
            trailing_anchor: None,
            ordinary_orders: Vec::new(),
            started: false,
        })
    }

    pub fn start(&mut self) -> Result<(), GridRuntimeError> {
        if self.started {
            return Ok(());
        }
        self.started = true;

        if !self.mode.is_ordinary() {
            return Ok(());
        }

        let Some(anchor_price) = self.plan.levels.first().copied() else {
            return Err(GridRuntimeError::new(
                "ordinary grid plan requires at least one level",
            ));
        };

        self.record_fill(anchor_price, self.default_quantity)?;
        self.ordinary_orders.clear();
        self.upsert_ordinary_take_profit(0, anchor_price)?;
        for level_index in 1..self.plan.levels.len() {
            self.upsert_ordinary_entry(level_index as u32)?;
        }
        self.sort_ordinary_orders();
        Ok(())
    }

    pub fn ordinary_orders(&self) -> &[RuntimeOrder] {
        &self.ordinary_orders
    }

    pub fn fill_ordinary_entry(&mut self, level_index: u32) -> Result<(), GridRuntimeError> {
        if !self.mode.is_ordinary() {
            return Err(GridRuntimeError::new(
                "ordinary entry fills are only supported for ordinary grid modes",
            ));
        }
        let Some(level_price) = self.plan.levels.get(level_index as usize).copied() else {
            return Err(GridRuntimeError::new("ordinary grid level not found"));
        };
        let Some(order_index) = self.ordinary_orders.iter().position(|order| {
            order.level_index == level_index && order.kind == RuntimeOrderKind::Entry
        }) else {
            return Err(GridRuntimeError::new("ordinary entry order not found"));
        };

        self.ordinary_orders.remove(order_index);
        self.record_fill(level_price, self.default_quantity)?;
        self.upsert_ordinary_take_profit(level_index, level_price)?;
        self.sort_ordinary_orders();
        Ok(())
    }

    pub fn record_fill(
        &mut self,
        entry_price: Decimal,
        quantity: Decimal,
    ) -> Result<(), GridRuntimeError> {
        if quantity <= Decimal::ZERO {
            return Err(GridRuntimeError::new("fill quantity must be positive"));
        }

        if let Some(existing) = &mut self.position {
            let total_quantity = existing.quantity + quantity;
            let weighted_price = ((existing.entry_price * existing.quantity)
                + (entry_price * quantity))
                / total_quantity;

            existing.entry_price = weighted_price;
            existing.quantity = total_quantity;
        } else {
            self.position = Some(Position {
                entry_price,
                quantity,
            });
        }

        self.trailing_anchor = None;
        Ok(())
    }

    pub fn on_price(&mut self, price: Decimal) -> Vec<RuntimeEvent> {
        self.last_price = Some(price);

        if self.status != RuntimeStatus::Running {
            return Vec::new();
        }

        let Some(position) = self.position.clone() else {
            return Vec::new();
        };
        let is_short = is_short_mode(self.mode);

        if let Some(maker) = &self.maker_take_profit {
            let target_price = take_profit_price_from_percent(
                position.entry_price,
                maker.target_percent,
                is_short,
            );
            if price_reaches_target(price, target_price, is_short) {
                return vec![self.close_position(target_price, "maker_take_profit", false)];
            }
        }

        if let Some(trailing) = &self.trailing_take_profit {
            if is_short {
                match self.trailing_anchor {
                    Some(low) => {
                        let new_low = low.min(price);
                        self.trailing_anchor = Some(new_low);
                        let stop_price = new_low * (Decimal::ONE + trailing.trailing_percent);
                        if price >= stop_price {
                            return vec![self.close_position(
                                price,
                                "taker_trailing_take_profit",
                                false,
                            )];
                        }
                    }
                    None if price <= trailing.trigger_price => {
                        self.trailing_anchor = Some(price);
                    }
                    None => {}
                }
            } else {
                match self.trailing_anchor {
                    Some(high) => {
                        let new_high = high.max(price);
                        self.trailing_anchor = Some(new_high);
                        let stop_price = new_high * (Decimal::ONE - trailing.trailing_percent);
                        if price <= stop_price {
                            return vec![self.close_position(
                                price,
                                "taker_trailing_take_profit",
                                false,
                            )];
                        }
                    }
                    None if price >= trailing.trigger_price => {
                        self.trailing_anchor = Some(price);
                    }
                    None => {}
                }
            }
        }

        if let Some(overall) = &self.overall_take_profit {
            let target_price = take_profit_price_from_percent(
                position.entry_price,
                overall.target_percent,
                is_short,
            );
            if price_reaches_target(price, target_price, is_short) {
                return vec![self.close_position(price, "overall_take_profit", true)];
            }
        }

        if let Some(stop_loss) = &self.overall_stop_loss {
            let stop_price = if is_short {
                position.entry_price * (Decimal::ONE + stop_loss.max_drawdown_percent)
            } else {
                position.entry_price * (Decimal::ONE - stop_loss.max_drawdown_percent)
            };
            let triggered = if is_short {
                price >= stop_price
            } else {
                price <= stop_price
            };
            if triggered {
                return vec![self.close_position(price, "overall_stop_loss", true)];
            }
        }

        Vec::new()
    }

    pub fn pause(&mut self) {
        if self.status == RuntimeStatus::Running {
            self.status = RuntimeStatus::Paused;
        }
    }

    pub fn resume(&mut self) {
        if self.status == RuntimeStatus::Paused {
            self.status = RuntimeStatus::Running;
        }
    }

    pub fn stop(&mut self) {
        self.status = RuntimeStatus::Stopped;
    }

    pub fn rebuild(&mut self, plan: GridPlan) -> Result<(), GridRuntimeError> {
        validate_supported_mode(plan.mode)?;
        validate_plan_shape(&plan)?;
        self.mode = plan.mode;
        self.plan = plan;
        self.status = RuntimeStatus::Running;
        self.position = None;
        self.realized_pnl = Decimal::ZERO;
        self.last_price = None;
        self.trailing_anchor = None;
        self.ordinary_orders.clear();
        self.started = false;
        Ok(())
    }

    pub fn status(&self) -> RuntimeStatus {
        self.status
    }

    pub fn position(&self) -> Option<&Position> {
        self.position.as_ref()
    }

    pub fn realized_pnl(&self) -> Decimal {
        self.realized_pnl
    }

    pub fn grid(&self) -> &GridPlan {
        &self.plan
    }

    fn upsert_ordinary_entry(&mut self, level_index: u32) -> Result<(), GridRuntimeError> {
        let Some(price) = self.plan.levels.get(level_index as usize).copied() else {
            return Err(GridRuntimeError::new("ordinary grid level not found"));
        };
        if let Some(order) = self
            .ordinary_orders
            .iter_mut()
            .find(|order| order.level_index == level_index && order.kind == RuntimeOrderKind::Entry)
        {
            order.price = price;
            order.quantity = self.default_quantity;
            return Ok(());
        }
        self.ordinary_orders.push(RuntimeOrder {
            kind: RuntimeOrderKind::Entry,
            level_index,
            price,
            quantity: self.default_quantity,
        });
        Ok(())
    }

    fn upsert_ordinary_take_profit(
        &mut self,
        level_index: u32,
        fill_price: Decimal,
    ) -> Result<(), GridRuntimeError> {
        let price = take_profit_price_from_bps(
            fill_price,
            self.ordinary_take_profit_bps,
            is_short_mode(self.mode),
        );
        if let Some(order) = self.ordinary_orders.iter_mut().find(|order| {
            order.level_index == level_index && order.kind == RuntimeOrderKind::TakeProfit
        }) {
            order.price = price;
            order.quantity = self.default_quantity;
            return Ok(());
        }
        self.ordinary_orders.push(RuntimeOrder {
            kind: RuntimeOrderKind::TakeProfit,
            level_index,
            price,
            quantity: self.default_quantity,
        });
        Ok(())
    }

    fn sort_ordinary_orders(&mut self) {
        self.ordinary_orders.sort_by_key(|order| {
            (
                order.level_index,
                match order.kind {
                    RuntimeOrderKind::TakeProfit => 0u8,
                    RuntimeOrderKind::Entry => 1u8,
                },
            )
        });
    }

    fn close_position(
        &mut self,
        exit_price: Decimal,
        reason: &'static str,
        stop_after_close: bool,
    ) -> RuntimeEvent {
        if let Some(position) = self.position.take() {
            self.realized_pnl += if is_short_mode(self.mode) {
                (position.entry_price - exit_price) * position.quantity
            } else {
                (exit_price - position.entry_price) * position.quantity
            };
        }

        self.trailing_anchor = None;
        self.ordinary_orders.clear();

        if stop_after_close {
            self.status = RuntimeStatus::Stopped;
        }

        RuntimeEvent {
            reason,
            exit_price: Some(exit_price),
        }
    }
}

fn validate_supported_mode(_mode: GridMode) -> Result<(), GridRuntimeError> {
    Ok(())
}

fn validate_plan_shape(plan: &GridPlan) -> Result<(), GridRuntimeError> {
    plan.validate_shape()
        .map_err(|err| GridRuntimeError::new(format!("invalid grid plan shape: {err}")))
}

fn is_short_mode(mode: GridMode) -> bool {
    matches!(mode, GridMode::FuturesShort)
}

fn price_reaches_target(price: Decimal, target_price: Decimal, is_short: bool) -> bool {
    if is_short {
        price <= target_price
    } else {
        price >= target_price
    }
}
