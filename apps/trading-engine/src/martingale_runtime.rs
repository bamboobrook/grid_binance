use backtest_engine::market_data::KlineBar;
use backtest_engine::martingale::exit_rules::{take_profit_price, ExitDecision};
use backtest_engine::martingale::indicator_runtime::{
    latest_atr_for_strategy, IndicatorRuntimeContext,
};
use backtest_engine::martingale::rules::{compute_leg_notionals, compute_leg_trigger_prices};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleDirectionMode, MartingaleEntryTrigger,
    MartingaleIndicatorConfig, MartingaleMarketKind, MartingalePortfolioConfig,
    MartingaleRiskLimits, MartingaleSizingModel, MartingaleSpacingModel, MartingaleStrategyConfig,
};
use shared_domain::strategy::StrategyStatus;
use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::hash::{Hash, Hasher};

const BPS_DENOMINATOR: Decimal = Decimal::from_parts(10_000, 0, 0, false, 0);

#[derive(Debug, Clone, PartialEq)]
pub struct MartingaleRuntimeConfig {
    pub portfolio_id: String,
    pub strategy_instance_id: String,
    pub portfolio: MartingalePortfolioConfig,
    pub portfolio_budget_quote: Decimal,
    pub exchange_min_notional: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MartingaleRuntimeContext {
    pub pause_new_entries: bool,
    pub strategy_status: StrategyStatus,
    pub global_drawdown_quote: Decimal,
    pub now_ms: Option<i64>,
    pub last_cycle_closed_at_ms: Option<i64>,
    /// Portfolio drawdown percent (peak→current equity). When `Some(dd)` with
    /// `dd > 6.0`, new cycles are paused (parity port of backtest guard
    /// `kline_engine.rs:142-146`). `None` only when the live equity sum is
    /// genuinely unavailable; in production `main.rs` threads a real `Some`.
    pub portfolio_drawdown_pct: Option<f64>,
}

impl Default for MartingaleRuntimeContext {
    fn default() -> Self {
        Self {
            pause_new_entries: false,
            strategy_status: StrategyStatus::Running,
            global_drawdown_quote: Decimal::ZERO,
            now_ms: None,
            last_cycle_closed_at_ms: None,
            portfolio_drawdown_pct: None,
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
    /// Margin capital reserved by this order = `notional_quote / leverage`.
    /// Persisted on the order so budget checks never recompute from a strategy
    /// leverage that may change after order creation.
    pub margin_quote: Decimal,
    /// Planned leverage captured at order creation (`margin = notional / leverage`).
    pub leverage: u32,
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
    started_at_ms: Option<i64>,
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
    indicator_context: IndicatorRuntimeContext,
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
            indicator_context: IndicatorRuntimeContext::default(),
        })
    }

    pub fn warmup_indicators_from_bars(&mut self, bars: Vec<KlineBar>) {
        for bar in &bars {
            self.indicator_context.push_bar(bar);
        }
    }

    pub fn evaluate_entry_triggers(
        &mut self,
        strategy: &MartingaleStrategyConfig,
        context: MartingaleRuntimeContext,
    ) -> Result<bool, MartingaleRuntimeError> {
        if strategy.entry_triggers.is_empty() {
            return Ok(true);
        }
        for trigger in &strategy.entry_triggers {
            match trigger {
                MartingaleEntryTrigger::IndicatorExpression { expression } => {
                    let result = match self
                        .indicator_context
                        .evaluate_expression(&strategy.symbol, expression)
                    {
                        Ok(result) => result,
                        Err(error) if error.starts_with("no indicator bars") => return Ok(false),
                        Err(error) => return Err(MartingaleRuntimeError::new(error)),
                    };
                    if !result.unwrap_or(false) {
                        return Ok(false);
                    }
                }
                MartingaleEntryTrigger::Cooldown { seconds } => {
                    if let (Some(now_ms), Some(closed_at_ms)) =
                        (context.now_ms, context.last_cycle_closed_at_ms)
                    {
                        let elapsed_ms = now_ms.saturating_sub(closed_at_ms);
                        if elapsed_ms < (*seconds as i64).saturating_mul(1_000) {
                            return Ok(false);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(true)
    }

    pub fn indicator_latest_atr(&mut self, strategy: &MartingaleStrategyConfig) -> Option<f64> {
        latest_atr_for_strategy(&mut self.indicator_context, strategy)
    }

    pub fn has_indicator_warmup_for(&self, symbol: &str, period: usize) -> bool {
        self.indicator_context
            .bars_by_symbol
            .get(symbol)
            .map(|bars| bars.len() >= period)
            .unwrap_or(false)
    }

    /// 实盘 TP 评估：用回测相同的 exit_rules 纯函数计算止盈价。
    /// 返回 (tp_price, ExitDecision) — 当 ExitDecision != None 时应平仓止盈。
    pub fn evaluate_strategy_take_profit(
        &mut self,
        strategy_id: &str,
        average_entry: f64,
    ) -> Result<Option<(f64, ExitDecision)>, MartingaleRuntimeError> {
        let (direction, take_profit_model, strategy_config) = {
            let strategy = self
                .strategies
                .get(strategy_id)
                .ok_or_else(|| MartingaleRuntimeError::new("strategy not found"))?;
            (
                strategy.config.direction,
                strategy.config.take_profit.clone(),
                strategy.config.clone(),
            )
        };
        let latest_atr = if matches!(
            take_profit_model,
            shared_domain::martingale::MartingaleTakeProfitModel::Atr { .. }
        ) {
            self.indicator_latest_atr(&strategy_config)
        } else {
            None
        };
        let tp_price = take_profit_price(average_entry, direction, &take_profit_model, latest_atr)
            .map_err(MartingaleRuntimeError::new)?;
        let triggered = tp_price.is_finite() && tp_price > 0.0;
        if triggered {
            Ok(Some((tp_price, ExitDecision::TakeProfit)))
        } else {
            Ok(None)
        }
    }

    /// 注入持久化的 indicator_context（跨 reconcile tick 保持指标状态）
    pub fn set_indicator_context(&mut self, ctx: IndicatorRuntimeContext) {
        self.indicator_context = ctx;
    }

    /// 导出当前 indicator_context（用于跨 tick 持久化）
    pub fn indicator_context_clone(&self) -> IndicatorRuntimeContext {
        self.indicator_context.clone()
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
        self.enforce_new_entry_controls(strategy_id, context, true)?;
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
            started_at_ms: context.now_ms,
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
        self.enforce_new_entry_controls(strategy_id, context, false)?;
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
        let strategy_config = strategy.config.clone();

        if let Some(order) = self.orders.iter_mut().find(|order| {
            order.strategy_id == strategy_id
                && order.direction == direction
                && order.leg_index == leg_index
                && order.cycle_id == cycle_id
        }) {
            order.status = MartingaleRuntimeOrderStatus::Filled;
        }

        // ADX skip guard: skip the safety (averaging-down) leg in extreme trends
        // (backtest parity — kline_engine.rs safety-skip block). The threshold is
        // now parity-structured: read from the strategy's risk_limits, defaulting
        // to 45.0 to match the backtest `DEFAULT_SAFETY_SKIP_ADX_THRESHOLD`.
        // Only fires for strategies that configure an ADX indicator; strategies
        // without ADX are unaffected. Must run BEFORE advancing `next_leg_index`
        // so a skipped leg retries on a later fill once ADX drops below the
        // threshold (matches backtest `continue`).
        let adx_threshold = strategy_config
            .risk_limits
            .safety_skip_adx_threshold
            .unwrap_or(45.0);
        let adx_period = strategy_config.indicators.iter().find_map(|i| match i {
            MartingaleIndicatorConfig::Adx { period } => Some(*period as usize),
            _ => None,
        });
        if let Some(period) = adx_period {
            if let Some(adx) = self
                .indicator_context
                .latest_adx(&strategy_config.symbol, period)
            {
                if adx > adx_threshold {
                    // Do NOT advance next_leg_index — the just-filled leg is marked
                    // Filled above, and the safety leg retries on the next fill.
                    return Ok(());
                }
            }
        }

        self.strategy_mut(strategy_id)?
            .cycle
            .as_mut()
            .expect("cycle checked above")
            .next_leg_index = next_leg_index;

        if next_leg_index >= max_legs {
            return Ok(());
        }
        self.enforce_budget_for_next_leg(strategy_id, next_leg_index)?;
        let latest_atr = self.indicator_latest_atr(&strategy_config);
        let price = leg_trigger_price(
            anchor_price,
            direction,
            &spacing,
            next_leg_index,
            latest_atr,
        )?;
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
        &mut self,
        strategy_id: &str,
        context: MartingaleRuntimeContext,
        new_cycle: bool,
    ) -> Result<(), MartingaleRuntimeError> {
        let strategy = self.strategy(strategy_id)?;
        let strategy_config = strategy.config.clone();
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
        // The portfolio-DD>6% and ATR>2% guards are NEW-CYCLE-ONLY in the
        // backtest (kline_engine.rs:142-160 — inside the new-cycle block). The
        // safety-leg path (mark_leg_filled_with_context, new_cycle=false) must
        // skip them so high-ATR / high-DD conditions do not block averaging
        // that the backtest would have done. All guards above apply to both.
        if new_cycle {
            // Parity port of backtest guard (kline_engine.rs): portfolio
            // drawdown above the configured percent pauses new cycles. `Some`
            // carries a real equity-based percent from `main.rs`; `None` (tests
            // / pre-wiring) skips the guard. The threshold is parity-structured:
            // read from portfolio risk_limits, defaulting to 6.0 to match the
            // backtest `DEFAULT_NEW_CYCLE_DRAWDOWN_PAUSE_PCT`.
            let dd_threshold = self
                .portfolio_risk_limits
                .new_cycle_drawdown_pause_pct
                .unwrap_or(6.0);
            if let Some(dd) = context.portfolio_drawdown_pct {
                if dd > dd_threshold {
                    return Err(MartingaleRuntimeError::new(
                        "portfolio drawdown above threshold pauses new entries",
                    ));
                }
            }

            // 方向3: 波动率过滤 — ATR/close*100 超过阈值时暂停新 cycle（高波动期风险大）
            // Parity port of backtest guard (kline_engine.rs). The threshold is
            // parity-structured: read from portfolio risk_limits, defaulting to
            // 2.0 to match the backtest `DEFAULT_NEW_CYCLE_ATR_PAUSE_PCT`.
            // `&mut self` ATR read completes before `evaluate_entry_triggers`
            // borrows `&mut self`.
            let atr_threshold = self
                .portfolio_risk_limits
                .new_cycle_atr_pause_pct
                .unwrap_or(2.0);
            if let Some(atr) = self.indicator_latest_atr(&strategy_config) {
                let close = self
                    .indicator_context
                    .bars_by_symbol
                    .get(&strategy_config.symbol)
                    .and_then(|bars| bars.last())
                    .map(|bar| bar.close);
                if let Some(close) = close {
                    if close > 0.0 && atr / close * 100.0 > atr_threshold {
                        return Err(MartingaleRuntimeError::new(
                            "atr volatility above threshold pauses new cycle",
                        ));
                    }
                }
            }
        }
        if !self.evaluate_entry_triggers(&strategy_config, context)? {
            return Err(MartingaleRuntimeError::new(
                "entry triggers are not satisfied",
            ));
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
        // Budget checks use MARGIN capital (= notional / leverage), matching
        // the backtest capital model.
        let next_notional = leg_notional(
            &strategy.config.sizing,
            self.exchange_min_notional,
            leg_index,
        )?;
        let leverage = strategy.config.leverage.unwrap_or(1).max(1);
        let next_margin = next_notional / Decimal::from(leverage);
        enforce_limit(
            "strategy budget",
            strategy.config.risk_limits.max_strategy_budget_quote,
            self.strategy_margin_exposure(strategy_id) + next_margin,
        )?;
        enforce_limit(
            "symbol budget",
            strategy.config.risk_limits.max_symbol_budget_quote,
            self.symbol_margin_exposure(&strategy.config.symbol) + next_margin,
        )?;
        enforce_limit(
            "direction budget",
            strategy.config.risk_limits.max_direction_budget_quote,
            self.direction_margin_exposure(strategy.config.direction) + next_margin,
        )?;
        // Global margin cap = portfolio_budget_quote, the portfolio's margin
        // capital (equals max_global_budget when set, or the sum of planned
        // margins as a fallback so the cap always exists).
        enforce_limit(
            "global budget",
            Some(self.portfolio_budget_quote),
            self.global_margin_exposure() + next_margin,
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
        // Canonical capital model (see backtest_engine::martingale::capital):
        // the sizing geometric series is the LEVERAGED ORDER NOTIONAL (position
        // size); margin = notional / leverage; quantity = notional / price.
        let notional_quote = leg_notional(&strategy.sizing, self.exchange_min_notional, leg_index)?;
        let leverage = strategy.leverage.unwrap_or(1).max(1);
        let margin_quote = notional_quote / Decimal::from(leverage);
        let quantity = notional_quote / price;
        let direction_label = direction_label(direction);
        let client_order_id = martingale_client_order_id(
            &self.portfolio_id,
            &self.strategy_instance_id,
            strategy_id,
            cycle_id,
            direction_label,
            leg_index,
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
            margin_quote,
            leverage,
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

    fn strategy_margin_exposure(&self, strategy_id: &str) -> Decimal {
        self.orders
            .iter()
            .filter(|order| order.strategy_id == strategy_id)
            .map(|order| self.order_margin_quote(order))
            .sum()
    }

    fn symbol_margin_exposure(&self, symbol: &str) -> Decimal {
        self.orders
            .iter()
            .filter(|order| order.symbol == symbol)
            .map(|order| self.order_margin_quote(order))
            .sum()
    }

    fn direction_margin_exposure(&self, direction: MartingaleDirection) -> Decimal {
        self.orders
            .iter()
            .filter(|order| order.direction == direction)
            .map(|order| self.order_margin_quote(order))
            .sum()
    }

    fn global_margin_exposure(&self) -> Decimal {
        self.orders
            .iter()
            .map(|order| self.order_margin_quote(order))
            .sum()
    }

    fn order_margin_quote(&self, order: &MartingaleRuntimeOrder) -> Decimal {
        // Margin is persisted on the order at creation time (margin =
        // notional / planned leverage), so exposure accounting never depends
        // on a strategy leverage that may change later.
        order.margin_quote
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
        if let Some(strategy_leverage) = strategy.leverage {
            if symbol_settings.leverage < strategy_leverage {
                return Err(MartingaleRuntimeError::new(format!(
                    "{} leverage conflicts with exchange settings",
                    strategy.symbol
                )));
            }
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

fn martingale_client_order_id(
    portfolio_id: &str,
    strategy_instance_id: &str,
    strategy_id: &str,
    cycle_id: &str,
    direction_label: &str,
    leg_index: u32,
) -> String {
    let mut hasher = DefaultHasher::new();
    portfolio_id.hash(&mut hasher);
    strategy_instance_id.hash(&mut hasher);
    strategy_id.hash(&mut hasher);
    cycle_id.hash(&mut hasher);
    leg_index.hash(&mut hasher);
    format!(
        "mg-{hash:016x}-{direction_label}-leg-{leg_index}",
        hash = hasher.finish()
    )
}

fn leg_trigger_price(
    anchor_price: Decimal,
    direction: MartingaleDirection,
    spacing: &MartingaleSpacingModel,
    leg_index: u32,
    latest_atr: Option<f64>,
) -> Result<Decimal, MartingaleRuntimeError> {
    let max_legs = leg_index.max(1);
    if let (Some(anchor), Some(_)) = (anchor_price.to_f64(), Decimal::ONE.to_f64()) {
        if let Ok(prices) =
            compute_leg_trigger_prices(anchor, direction, spacing, latest_atr, max_legs)
        {
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
        MartingaleSpacingModel::Atr { .. } => {
            Err(MartingaleRuntimeError::new(
                "ATR spacing requires latest_atr to compute trigger prices; use compute_leg_trigger_prices path instead",
            ))
        }
        MartingaleSpacingModel::Mixed { phases } => phases
            .first()
            .ok_or_else(|| MartingaleRuntimeError::new("mixed spacing requires phases"))
            .and_then(|phase| spacing_distance_bps(phase, leg_index)),
    }
}

fn leg_notional(
    sizing: &MartingaleSizingModel,
    exchange_min_notional: Decimal,
    leg_index: u32,
) -> Result<Decimal, MartingaleRuntimeError> {
    // Returns the leveraged order NOTIONAL (position size) for a leg. The
    // margin budget is enforced separately in enforce_budget_for_next_leg
    // (margin = notional / leverage vs the portfolio margin budget), matching
    // the backtest capital model in backtest_engine::martingale::capital.
    if let Some(min_notional) = exchange_min_notional.to_f64() {
        if let Ok(notionals) = compute_leg_notionals(sizing, f64::MAX, min_notional) {
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
