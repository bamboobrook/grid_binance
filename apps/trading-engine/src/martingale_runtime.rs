use backtest_engine::martingale::rules::{compute_leg_notionals, compute_leg_trigger_prices};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleMarketKind, MartingalePortfolioConfig,
    MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel, MartingaleStrategyConfig,
};
use shared_domain::strategy::StrategyStatus;
use std::collections::{HashMap, HashSet};

const BPS_DENOMINATOR: Decimal = Decimal::from_parts(10_000, 0, 0, false, 0);

#[derive(Debug, Clone, PartialEq)]
pub struct MartingaleRuntimeConfig {
    pub portfolio_id: String,
    pub strategy_instance_id: String,
    pub portfolio: MartingalePortfolioConfig,
    pub portfolio_budget_quote: Decimal,
    pub exchange_min_notional: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MartingaleRuntimeContext {
    pub pause_new_entries: bool,
    pub strategy_status: StrategyStatus,
    pub global_drawdown_quote: Decimal,
}

impl Default for MartingaleRuntimeContext {
    fn default() -> Self {
        Self {
            pause_new_entries: false,
            strategy_status: StrategyStatus::Running,
            global_drawdown_quote: Decimal::ZERO,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MartingaleRuntimeOrderStatus {
    Working,
    Placed,
    Filled,
    Canceled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MartingaleRuntimeOrder {
    pub client_order_id: String,
    pub exchange_order_id: Option<String>,
    pub strategy_id: String,
    pub symbol: String,
    pub cycle_id: String,
    pub direction: MartingaleDirection,
    pub leg_index: u32,
    pub side: String,
    pub price: Decimal,
    pub quantity: Decimal,
    pub notional_quote: Decimal,
    pub status: MartingaleRuntimeOrderStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MartingaleRecoveredPosition {
    pub symbol: String,
    pub quantity: Decimal,
    pub entry_price: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MartingaleRuntimeError {
    message: String,
}

impl MartingaleRuntimeError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for MartingaleRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for MartingaleRuntimeError {}

#[derive(Debug, Clone)]
struct RuntimeStrategy {
    config: MartingaleStrategyConfig,
    cycle: Option<CycleState>,
    paused_by_recovery: bool,
    stopped: bool,
}

#[derive(Debug, Clone)]
struct CycleState {
    cycle_id: String,
    anchor_price: Decimal,
    next_leg_index: u32,
}

#[derive(Debug, Clone)]
pub struct MartingaleRuntime {
    portfolio_id: String,
    strategy_instance_id: String,
    portfolio_risk_limits: MartingaleRiskLimits,
    portfolio_budget_quote: Decimal,
    exchange_min_notional: Decimal,
    strategies: HashMap<String, RuntimeStrategy>,
    orders: Vec<MartingaleRuntimeOrder>,
    recovered_positions: Vec<MartingaleRecoveredPosition>,
    futures_preflight_passed: bool,
    cycle_sequence: u64,
}

impl MartingaleRuntime {
    pub fn new(config: MartingaleRuntimeConfig) -> Result<Self, MartingaleRuntimeError> {
        config
            .portfolio
            .validate()
            .map_err(MartingaleRuntimeError::new)?;
        if config.portfolio_budget_quote < Decimal::ZERO {
            return Err(MartingaleRuntimeError::new(
                "portfolio budget quote cannot be negative",
            ));
        }
        if config.exchange_min_notional < Decimal::ZERO {
            return Err(MartingaleRuntimeError::new(
                "exchange minimum notional cannot be negative",
            ));
        }

        let mut strategies = HashMap::new();
        for strategy in config.portfolio.strategies {
            strategies.insert(
                strategy.strategy_id.clone(),
                RuntimeStrategy {
                    config: strategy,
                    cycle: None,
                    paused_by_recovery: false,
                    stopped: false,
                },
            );
        }

        Ok(Self {
            portfolio_id: config.portfolio_id,
            strategy_instance_id: config.strategy_instance_id,
            portfolio_risk_limits: config.portfolio.risk_limits,
            portfolio_budget_quote: config.portfolio_budget_quote,
            exchange_min_notional: config.exchange_min_notional,
            strategies,
            orders: Vec::new(),
            recovered_positions: Vec::new(),
            futures_preflight_passed: false,
            cycle_sequence: 0,
        })
    }

    pub fn preflight_start(
        &mut self,
        exchange: &FuturesExchangeSettings,
    ) -> Result<(), MartingaleRuntimeError> {
        validate_futures_settings_before_start(&self.portfolio_config(), exchange)?;
        self.futures_preflight_passed = true;
        Ok(())
    }

    pub fn start_cycle_with_futures_preflight(
        &mut self,
        exchange: &FuturesExchangeSettings,
        strategy_id: &str,
        anchor_price: Decimal,
        context: MartingaleRuntimeContext,
    ) -> Result<(), MartingaleRuntimeError> {
        self.preflight_start(exchange)?;
        self.start_cycle(strategy_id, anchor_price, context)
    }

    pub fn start_cycle(
        &mut self,
        strategy_id: &str,
        anchor_price: Decimal,
        context: MartingaleRuntimeContext,
    ) -> Result<(), MartingaleRuntimeError> {
        self.enforce_new_entry_controls(strategy_id, context)?;
        self.enforce_preflight_before_start(strategy_id)?;
        if anchor_price <= Decimal::ZERO {
            return Err(MartingaleRuntimeError::new("anchor price must be positive"));
        }
        self.enforce_budget_for_next_leg(strategy_id, 0)?;

        if self.strategy(strategy_id)?.cycle.is_some() {
            return Ok(());
        }
        self.cycle_sequence += 1;
        let cycle_id = format!("cycle-{}", self.cycle_sequence);
        let direction = self.strategy(strategy_id)?.config.direction;
        self.strategy_mut(strategy_id)?.cycle = Some(CycleState {
            cycle_id: cycle_id.clone(),
            anchor_price,
            next_leg_index: 0,
        });

        self.place_leg(strategy_id, direction, &cycle_id, 0, anchor_price)
    }

    pub fn mark_leg_filled(
        &mut self,
        strategy_id: &str,
        direction: MartingaleDirection,
        leg_index: u32,
    ) -> Result<(), MartingaleRuntimeError> {
        self.mark_leg_filled_with_context(
            strategy_id,
            direction,
            leg_index,
            MartingaleRuntimeContext::default(),
        )
    }

    pub fn mark_leg_filled_with_context(
        &mut self,
        strategy_id: &str,
        direction: MartingaleDirection,
        leg_index: u32,
        context: MartingaleRuntimeContext,
    ) -> Result<(), MartingaleRuntimeError> {
        self.enforce_new_entry_controls(strategy_id, context)?;
        let strategy = self.strategy(strategy_id)?;
        if strategy.config.direction != direction {
            return Err(MartingaleRuntimeError::new(
                "direction does not match strategy",
            ));
        }
        let cycle = strategy
            .cycle
            .as_ref()
            .ok_or_else(|| MartingaleRuntimeError::new("cycle is not active"))?;
        if leg_index + 1 < cycle.next_leg_index {
            return Ok(());
        }
        let cycle_id = cycle.cycle_id.clone();
        let anchor_price = cycle.anchor_price;
        let next_leg_index = leg_index + 1;
        let max_legs = max_legs(&strategy.config.sizing);
        let spacing = strategy.config.spacing.clone();

        self.strategy_mut(strategy_id)?
            .cycle
            .as_mut()
            .expect("cycle checked above")
            .next_leg_index = next_leg_index;

        if let Some(order) = self.orders.iter_mut().find(|order| {
            order.strategy_id == strategy_id
                && order.direction == direction
                && order.leg_index == leg_index
                && order.cycle_id == cycle_id
        }) {
            order.status = MartingaleRuntimeOrderStatus::Filled;
        }

        if next_leg_index >= max_legs {
            return Ok(());
        }
        self.enforce_budget_for_next_leg(strategy_id, next_leg_index)?;
        let price = leg_trigger_price(anchor_price, direction, &spacing, next_leg_index)?;
        self.place_leg(strategy_id, direction, &cycle_id, next_leg_index, price)
    }

    pub fn replace_recovered_positions(&mut self, positions: Vec<MartingaleRecoveredPosition>) {
        self.recovered_positions = positions;
    }

    pub fn recovered_positions(&self) -> &[MartingaleRecoveredPosition] {
        &self.recovered_positions
    }

    pub fn orders(&self) -> &[MartingaleRuntimeOrder] {
        &self.orders
    }

    pub fn orders_for(
        &self,
        strategy_id: &str,
        direction: MartingaleDirection,
    ) -> Vec<&MartingaleRuntimeOrder> {
        self.orders
            .iter()
            .filter(|order| order.strategy_id == strategy_id && order.direction == direction)
            .collect()
    }

    pub fn known_client_order_ids(&self) -> HashSet<String> {
        self.orders
            .iter()
            .map(|order| order.client_order_id.clone())
            .collect()
    }

    pub fn mark_order_placed(&mut self, client_order_id: &str, exchange_order_id: String) -> bool {
        let Some(order) = self
            .orders
            .iter_mut()
            .find(|order| order.client_order_id == client_order_id)
        else {
            return false;
        };
        order.exchange_order_id = Some(exchange_order_id);
        order.status = MartingaleRuntimeOrderStatus::Placed;
        true
    }

    pub fn pause_strategy_for_recovery(&mut self, strategy_id: &str) {
        if let Some(strategy) = self.strategies.get_mut(strategy_id) {
            strategy.paused_by_recovery = true;
        }
    }

    pub fn pause_all_for_recovery(&mut self) {
        for strategy in self.strategies.values_mut() {
            strategy.paused_by_recovery = true;
        }
    }

    pub fn is_strategy_paused(&self, strategy_id: &str) -> bool {
        self.strategies
            .get(strategy_id)
            .map(|strategy| strategy.paused_by_recovery)
            .unwrap_or(false)
    }

    pub fn stop_strategy(&mut self, strategy_id: &str) {
        if let Some(strategy) = self.strategies.get_mut(strategy_id) {
            strategy.stopped = true;
        }
    }

    pub fn strategy_id_for_client_order(&self, client_order_id: &str) -> Option<&str> {
        self.orders
            .iter()
            .find(|order| order.client_order_id == client_order_id)
            .map(|order| order.strategy_id.as_str())
    }

    fn portfolio_config(&self) -> MartingalePortfolioConfig {
        MartingalePortfolioConfig {
            direction_mode: portfolio_direction_mode(self.strategies.values().map(|s| &s.config)),
            strategies: self
                .strategies
                .values()
                .map(|strategy| strategy.config.clone())
                .collect(),
            risk_limits: self.portfolio_risk_limits.clone(),
        }
    }

    fn enforce_new_entry_controls(
        &self,
        strategy_id: &str,
        context: MartingaleRuntimeContext,
    ) -> Result<(), MartingaleRuntimeError> {
        let strategy = self.strategy(strategy_id)?;
        if strategy.stopped || matches!(context.strategy_status, StrategyStatus::Stopped) {
            return Err(MartingaleRuntimeError::new("strategy is stopped"));
        }
        if strategy.paused_by_recovery {
            return Err(MartingaleRuntimeError::new(
                "recovery incomplete blocks new legs",
            ));
        }
        if context.pause_new_entries {
            return Err(MartingaleRuntimeError::new("portfolio paused new entries"));
        }
        if matches!(
            context.strategy_status,
            StrategyStatus::Paused | StrategyStatus::ErrorPaused | StrategyStatus::Stopping
        ) {
            return Err(MartingaleRuntimeError::new("strategy is paused"));
        }
        if let Some(limit) = self.portfolio_risk_limits.max_global_drawdown_quote {
            if context.global_drawdown_quote >= limit {
                return Err(MartingaleRuntimeError::new(
                    "global drawdown pauses new entries",
                ));
            }
        }
        Ok(())
    }

    fn enforce_preflight_before_start(
        &self,
        strategy_id: &str,
    ) -> Result<(), MartingaleRuntimeError> {
        let strategy = self.strategy(strategy_id)?;
        if strategy.config.market == MartingaleMarketKind::UsdMFutures
            && !self.futures_preflight_passed
        {
            return Err(MartingaleRuntimeError::new(
                "futures preflight must pass before start",
            ));
        }
        Ok(())
    }

    fn enforce_budget_for_next_leg(
        &self,
        strategy_id: &str,
        leg_index: u32,
    ) -> Result<(), MartingaleRuntimeError> {
        let strategy = self.strategy(strategy_id)?;
        let next_notional = leg_notional(
            &strategy.config.sizing,
            self.portfolio_budget_quote,
            self.exchange_min_notional,
            leg_index,
        )?;
        enforce_limit(
            "strategy budget",
            strategy.config.risk_limits.max_strategy_budget_quote,
            self.strategy_exposure(strategy_id) + next_notional,
        )?;
        enforce_limit(
            "symbol budget",
            strategy.config.risk_limits.max_symbol_budget_quote,
            self.symbol_exposure(&strategy.config.symbol) + next_notional,
        )?;
        enforce_limit(
            "direction budget",
            strategy.config.risk_limits.max_direction_budget_quote,
            self.direction_exposure(strategy.config.direction) + next_notional,
        )?;
        enforce_limit(
            "global budget",
            self.portfolio_risk_limits.max_global_budget_quote,
            self.global_exposure() + next_notional,
        )?;
        Ok(())
    }

    fn place_leg(
        &mut self,
        strategy_id: &str,
        direction: MartingaleDirection,
        cycle_id: &str,
        leg_index: u32,
        price: Decimal,
    ) -> Result<(), MartingaleRuntimeError> {
        let strategy = self.strategy(strategy_id)?.config.clone();
        let notional_quote = leg_notional(
            &strategy.sizing,
            self.portfolio_budget_quote,
            self.exchange_min_notional,
            leg_index,
        )?;
        let quantity = notional_quote / price;
        let direction_label = direction_label(direction);
        let client_order_id = format!(
            "mg-{}-{}-{}-{}-leg-{}",
            self.portfolio_id, self.strategy_instance_id, cycle_id, direction_label, leg_index
        );
        self.orders.push(MartingaleRuntimeOrder {
            client_order_id,
            exchange_order_id: None,
            strategy_id: strategy_id.to_string(),
            symbol: strategy.symbol,
            cycle_id: cycle_id.to_string(),
            direction,
            leg_index,
            side: entry_side(direction).to_string(),
            price,
            quantity,
            notional_quote,
            status: MartingaleRuntimeOrderStatus::Working,
        });
        Ok(())
    }

    fn strategy(&self, strategy_id: &str) -> Result<&RuntimeStrategy, MartingaleRuntimeError> {
        self.strategies
            .get(strategy_id)
            .ok_or_else(|| MartingaleRuntimeError::new("strategy not found"))
    }

    fn strategy_mut(
        &mut self,
        strategy_id: &str,
    ) -> Result<&mut RuntimeStrategy, MartingaleRuntimeError> {
        self.strategies
            .get_mut(strategy_id)
            .ok_or_else(|| MartingaleRuntimeError::new("strategy not found"))
    }

    fn strategy_exposure(&self, strategy_id: &str) -> Decimal {
        self.orders
            .iter()
            .filter(|order| order.strategy_id == strategy_id)
            .map(|order| order.notional_quote)
            .sum()
    }

    fn symbol_exposure(&self, symbol: &str) -> Decimal {
        self.orders
            .iter()
            .filter(|order| order.symbol == symbol)
            .map(|order| order.notional_quote)
            .sum()
    }

    fn direction_exposure(&self, direction: MartingaleDirection) -> Decimal {
        self.orders
            .iter()
            .filter(|order| order.direction == direction)
            .map(|order| order.notional_quote)
            .sum()
    }

    fn global_exposure(&self) -> Decimal {
        self.orders.iter().map(|order| order.notional_quote).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuturesExchangeSettings {
    pub hedge_mode: bool,
    pub symbols: HashMap<String, FuturesSymbolSettings>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuturesSymbolSettings {
    pub margin_mode: shared_domain::martingale::MartingaleMarginMode,
    pub leverage: u32,
}

pub fn validate_futures_settings_before_start(
    portfolio: &MartingalePortfolioConfig,
    exchange: &FuturesExchangeSettings,
) -> Result<(), MartingaleRuntimeError> {
    let has_futures_long = portfolio.strategies.iter().any(|strategy| {
        strategy.market == MartingaleMarketKind::UsdMFutures
            && strategy.direction == MartingaleDirection::Long
    });
    let has_futures_short = portfolio.strategies.iter().any(|strategy| {
        strategy.market == MartingaleMarketKind::UsdMFutures
            && strategy.direction == MartingaleDirection::Short
    });
    if has_futures_long && has_futures_short && !exchange.hedge_mode {
        return Err(MartingaleRuntimeError::new(
            "long+short futures portfolio requires Hedge Mode",
        ));
    }

    for strategy in portfolio
        .strategies
        .iter()
        .filter(|strategy| strategy.market == MartingaleMarketKind::UsdMFutures)
    {
        let Some(symbol_settings) = exchange.symbols.get(&strategy.symbol) else {
            return Err(MartingaleRuntimeError::new(format!(
                "missing futures settings for {}",
                strategy.symbol
            )));
        };
        if Some(symbol_settings.margin_mode) != strategy.margin_mode {
            return Err(MartingaleRuntimeError::new(format!(
                "{} margin mode conflicts with exchange settings",
                strategy.symbol
            )));
        }
        if Some(symbol_settings.leverage) != strategy.leverage {
            return Err(MartingaleRuntimeError::new(format!(
                "{} leverage conflicts with exchange settings",
                strategy.symbol
            )));
        }
    }

    if matches!(
        portfolio.direction_mode,
        MartingaleDirectionMode::LongAndShort
    ) && !exchange.hedge_mode
    {
        return Err(MartingaleRuntimeError::new(
            "long+short futures portfolio requires Hedge Mode",
        ));
    }
    Ok(())
}

fn leg_trigger_price(
    anchor_price: Decimal,
    direction: MartingaleDirection,
    spacing: &MartingaleSpacingModel,
    leg_index: u32,
) -> Result<Decimal, MartingaleRuntimeError> {
    let max_legs = leg_index.max(1);
    if let (Some(anchor), Some(_)) = (anchor_price.to_f64(), Decimal::ONE.to_f64()) {
        if let Ok(prices) = compute_leg_trigger_prices(anchor, direction, spacing, None, max_legs) {
            if let Some(price) = prices.get(leg_index.saturating_sub(1) as usize) {
                return Decimal::try_from(*price)
                    .map(|price| price.normalize())
                    .map_err(|_| MartingaleRuntimeError::new("computed price overflow"));
            }
        }
    }
    let distance_bps = spacing_distance_bps(spacing, leg_index)?;
    let offset = anchor_price * distance_bps / BPS_DENOMINATOR;
    let price = match direction {
        MartingaleDirection::Long => anchor_price - offset,
        MartingaleDirection::Short => anchor_price + offset,
    };
    if price <= Decimal::ZERO {
        return Err(MartingaleRuntimeError::new(
            "computed leg trigger price must be positive",
        ));
    }
    Ok(price.normalize())
}

fn spacing_distance_bps(
    spacing: &MartingaleSpacingModel,
    leg_index: u32,
) -> Result<Decimal, MartingaleRuntimeError> {
    match spacing {
        MartingaleSpacingModel::FixedPercent { step_bps } => {
            Ok(Decimal::from(*step_bps) * Decimal::from(leg_index))
        }
        MartingaleSpacingModel::Multiplier {
            first_step_bps,
            multiplier,
        } => decimal_pow(*multiplier, leg_index)
            .map(|factor| Decimal::from(*first_step_bps) * factor),
        MartingaleSpacingModel::CustomSequence { steps_bps } => steps_bps
            .get(leg_index.saturating_sub(1) as usize)
            .copied()
            .map(Decimal::from)
            .ok_or_else(|| MartingaleRuntimeError::new("custom spacing leg not found")),
        MartingaleSpacingModel::Atr { .. } => Err(MartingaleRuntimeError::new(
            "unsupported spacing model for live runtime without indicators",
        )),
        MartingaleSpacingModel::Mixed { phases } => phases
            .first()
            .ok_or_else(|| MartingaleRuntimeError::new("mixed spacing requires phases"))
            .and_then(|phase| spacing_distance_bps(phase, leg_index)),
    }
}

fn leg_notional(
    sizing: &MartingaleSizingModel,
    portfolio_budget_quote: Decimal,
    exchange_min_notional: Decimal,
    leg_index: u32,
) -> Result<Decimal, MartingaleRuntimeError> {
    if let (Some(budget), Some(min_notional)) = (
        portfolio_budget_quote.to_f64(),
        exchange_min_notional.to_f64(),
    ) {
        if let Ok(notionals) = compute_leg_notionals(sizing, budget, min_notional) {
            if let Some(notional) = notionals.get(leg_index as usize) {
                return Decimal::try_from(*notional)
                    .map(|notional| notional.normalize())
                    .map_err(|_| MartingaleRuntimeError::new("computed notional overflow"));
            }
        }
    }
    let notional = match sizing {
        MartingaleSizingModel::Multiplier {
            first_order_quote,
            multiplier,
            max_legs,
        }
        | MartingaleSizingModel::BudgetScaled {
            first_order_quote,
            multiplier,
            max_legs,
            ..
        } => {
            if leg_index >= *max_legs {
                return Err(MartingaleRuntimeError::new("leg index exceeds max legs"));
            }
            *first_order_quote * decimal_pow(*multiplier, leg_index)?
        }
        MartingaleSizingModel::CustomSequence { notionals } => notionals
            .get(leg_index as usize)
            .copied()
            .ok_or_else(|| MartingaleRuntimeError::new("custom notional leg not found"))?,
    };
    if notional < exchange_min_notional {
        return Err(MartingaleRuntimeError::new(
            "leg notional is below exchange minimum",
        ));
    }
    if notional > portfolio_budget_quote {
        return Err(MartingaleRuntimeError::new(
            "leg notional exceeds portfolio budget",
        ));
    }
    Ok(notional.normalize())
}

fn max_legs(sizing: &MartingaleSizingModel) -> u32 {
    match sizing {
        MartingaleSizingModel::Multiplier { max_legs, .. }
        | MartingaleSizingModel::BudgetScaled { max_legs, .. } => *max_legs,
        MartingaleSizingModel::CustomSequence { notionals } => notionals.len() as u32,
    }
}

fn decimal_pow(value: Decimal, exponent: u32) -> Result<Decimal, MartingaleRuntimeError> {
    let base = value
        .to_f64()
        .ok_or_else(|| MartingaleRuntimeError::new("decimal cannot be converted"))?;
    Decimal::try_from(base.powi(exponent as i32))
        .map_err(|_| MartingaleRuntimeError::new("decimal power overflow"))
}

fn enforce_limit(
    name: &str,
    limit: Option<Decimal>,
    value: Decimal,
) -> Result<(), MartingaleRuntimeError> {
    if let Some(limit) = limit {
        if value > limit {
            return Err(MartingaleRuntimeError::new(format!(
                "{name} exceeded before placing leg",
            )));
        }
    }
    Ok(())
}

fn entry_side(direction: MartingaleDirection) -> &'static str {
    match direction {
        MartingaleDirection::Long => "BUY",
        MartingaleDirection::Short => "SELL",
    }
}

fn direction_label(direction: MartingaleDirection) -> &'static str {
    match direction {
        MartingaleDirection::Long => "long",
        MartingaleDirection::Short => "short",
    }
}

fn portfolio_direction_mode<'a>(
    strategies: impl Iterator<Item = &'a MartingaleStrategyConfig>,
) -> MartingaleDirectionMode {
    let mut has_long = false;
    let mut has_short = false;
    for strategy in strategies {
        has_long |= strategy.direction == MartingaleDirection::Long;
        has_short |= strategy.direction == MartingaleDirection::Short;
    }
    match (has_long, has_short) {
        (true, true) => MartingaleDirectionMode::LongAndShort,
        (false, true) => MartingaleDirectionMode::ShortOnly,
        _ => MartingaleDirectionMode::LongOnly,
    }
}
