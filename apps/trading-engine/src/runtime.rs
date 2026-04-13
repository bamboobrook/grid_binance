use rust_decimal::Decimal;

use crate::grid_builder::GridPlan;
use crate::stop_loss::OverallStopLoss;
use crate::take_profit::{MakerTakeProfit, OverallTakeProfit, TrailingTakeProfit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridMode {
    SpotClassic,
    SpotBuyOnly,
    SpotSellOnly,
    FuturesLong,
    FuturesShort,
    FuturesNeutral,
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
    maker_take_profit: Option<MakerTakeProfit>,
    trailing_take_profit: Option<TrailingTakeProfit>,
    overall_take_profit: Option<OverallTakeProfit>,
    overall_stop_loss: Option<OverallStopLoss>,
    status: RuntimeStatus,
    position: Option<Position>,
    realized_pnl: Decimal,
    last_price: Option<Decimal>,
    trailing_high: Option<Decimal>,
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

        if config.quantity <= Decimal::ZERO {
            return Err(GridRuntimeError::new("default quantity must be positive"));
        }

        Ok(Self {
            mode: config.mode,
            plan: config.plan,
            maker_take_profit: config.maker_take_profit,
            trailing_take_profit: config.trailing_take_profit,
            overall_take_profit: config.overall_take_profit,
            overall_stop_loss: config.overall_stop_loss,
            status: RuntimeStatus::Running,
            position: None,
            realized_pnl: Decimal::ZERO,
            last_price: None,
            trailing_high: None,
        })
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

        self.trailing_high = None;
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

        if let Some(maker) = &self.maker_take_profit {
            let target_price = position.entry_price * (Decimal::ONE + maker.target_percent);
            if price >= target_price {
                return vec![self.close_position(target_price, "maker_take_profit", false)];
            }
        }

        if let Some(trailing) = &self.trailing_take_profit {
            match self.trailing_high {
                Some(high) => {
                    let new_high = high.max(price);
                    self.trailing_high = Some(new_high);
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
                    self.trailing_high = Some(price);
                }
                None => {}
            }
        }

        if let Some(overall) = &self.overall_take_profit {
            let target_price = position.entry_price * (Decimal::ONE + overall.target_percent);
            if price >= target_price {
                return vec![self.close_position(price, "overall_take_profit", true)];
            }
        }

        if let Some(stop_loss) = &self.overall_stop_loss {
            let stop_price = position.entry_price * (Decimal::ONE - stop_loss.max_drawdown_percent);
            if price <= stop_price {
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
        self.mode = plan.mode;
        self.plan = plan;
        self.status = RuntimeStatus::Running;
        self.position = None;
        self.realized_pnl = Decimal::ZERO;
        self.last_price = None;
        self.trailing_high = None;
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

    fn close_position(
        &mut self,
        exit_price: Decimal,
        reason: &'static str,
        stop_after_close: bool,
    ) -> RuntimeEvent {
        if let Some(position) = self.position.take() {
            self.realized_pnl += (exit_price - position.entry_price) * position.quantity;
        }

        self.trailing_high = None;

        if stop_after_close {
            self.status = RuntimeStatus::Stopped;
        }

        RuntimeEvent {
            reason,
            exit_price: Some(exit_price),
        }
    }
}

fn validate_supported_mode(mode: GridMode) -> Result<(), GridRuntimeError> {
    let _ = mode;
    Ok(())
}
