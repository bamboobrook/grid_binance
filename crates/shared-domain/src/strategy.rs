use chrono::{DateTime, Utc};
pub use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::{Mutex, OnceLock}};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StrategyStatus {
    Draft,
    Running,
    Paused,
    ErrorPaused,
    Completed,
    Stopping,
    Stopped,
    Archived,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PreflightStepStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StrategyMarket {
    Spot,
    FuturesUsdM,
    FuturesCoinM,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StrategyMode {
    SpotClassic,
    SpotBuyOnly,
    SpotSellOnly,
    FuturesLong,
    FuturesShort,
    FuturesNeutral,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GridGeneration {
    Arithmetic,
    Geometric,
    Custom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PostTriggerAction {
    Stop,
    Rebuild,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StrategyAmountMode {
    Quote,
    Base,
}

impl Default for StrategyAmountMode {
    fn default() -> Self {
        Self::Quote
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FuturesMarginMode {
    Isolated,
    Cross,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StrategyType {
    OrdinaryGrid,
    ClassicBilateralGrid,
}

impl Default for StrategyType {
    fn default() -> Self {
        Self::OrdinaryGrid
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReferencePriceSource {
    Manual,
    Market,
}

impl Default for ReferencePriceSource {
    fn default() -> Self {
        Self::Manual
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StrategyRuntimePhase {
    Draft,
}

impl Default for StrategyRuntimePhase {
    fn default() -> Self {
        Self::Draft
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeControls {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreflightFailure {
    pub step: String,
    pub reason: String,
    pub guidance: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreflightStepResult {
    pub step: String,
    pub status: PreflightStepStatus,
    pub reason: Option<String>,
    pub guidance: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreflightReport {
    pub ok: bool,
    pub steps: Vec<PreflightStepResult>,
    pub failures: Vec<PreflightFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GridLevel {
    pub level_index: u32,
    pub entry_price: Decimal,
    pub quantity: Decimal,
    pub take_profit_bps: u32,
    pub trailing_bps: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StrategyRevision {
    pub revision_id: String,
    pub version: u32,
    pub strategy_type: StrategyType,
    pub generation: GridGeneration,
    pub levels: Vec<GridLevel>,
    pub amount_mode: StrategyAmountMode,
    pub futures_margin_mode: Option<FuturesMarginMode>,
    pub leverage: Option<u32>,
    #[serde(default)]
    pub reference_price_source: ReferencePriceSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_price: Option<Decimal>,
    pub overall_take_profit_bps: Option<u32>,
    pub overall_stop_loss_bps: Option<u32>,
    pub post_trigger_action: PostTriggerAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StrategyRuntimePosition {
    pub market: StrategyMarket,
    pub mode: StrategyMode,
    pub quantity: Decimal,
    pub average_entry_price: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StrategyRuntimeOrder {
    pub order_id: String,
    #[serde(default)]
    pub exchange_order_id: Option<String>,
    pub level_index: Option<u32>,
    pub side: String,
    pub order_type: String,
    pub price: Option<Decimal>,
    pub quantity: Decimal,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StrategyRuntimeFill {
    pub fill_id: String,
    pub order_id: Option<String>,
    pub level_index: Option<u32>,
    pub fill_type: String,
    pub price: Decimal,
    pub quantity: Decimal,
    pub realized_pnl: Option<Decimal>,
    pub fee_amount: Option<Decimal>,
    pub fee_asset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StrategyRuntimeEvent {
    pub event_type: String,
    pub detail: String,
    pub price: Option<Decimal>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StrategyRuntime {
    pub positions: Vec<StrategyRuntimePosition>,
    pub orders: Vec<StrategyRuntimeOrder>,
    pub fills: Vec<StrategyRuntimeFill>,
    pub events: Vec<StrategyRuntimeEvent>,
    pub last_preflight: Option<PreflightReport>,
}

impl Default for StrategyRuntime {
    fn default() -> Self {
        Self {
            positions: Vec::new(),
            orders: Vec::new(),
            fills: Vec::new(),
            events: Vec::new(),
            last_preflight: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Strategy {
    pub id: String,
    #[serde(skip)]
    pub owner_email: String,
    pub name: String,
    pub symbol: String,
    pub budget: String,
    pub grid_spacing_bps: u32,
    pub status: StrategyStatus,
    pub source_template_id: Option<String>,
    pub membership_ready: bool,
    pub exchange_ready: bool,
    pub permissions_ready: bool,
    pub withdrawals_disabled: bool,
    pub hedge_mode_ready: bool,
    pub symbol_ready: bool,
    pub filters_ready: bool,
    pub margin_ready: bool,
    pub conflict_ready: bool,
    pub balance_ready: bool,
    pub strategy_type: StrategyType,
    pub market: StrategyMarket,
    pub mode: StrategyMode,
    #[serde(default)]
    pub runtime_phase: StrategyRuntimePhase,
    #[serde(default)]
    pub runtime_controls: RuntimeControls,
    pub draft_revision: StrategyRevision,
    pub active_revision: Option<StrategyRevision>,
    pub runtime: StrategyRuntime,
    pub archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategyTemplate {
    pub id: String,
    pub name: String,
    pub symbol: String,
    pub market: StrategyMarket,
    pub mode: StrategyMode,
    pub strategy_type: StrategyType,
    pub generation: GridGeneration,
    pub levels: Vec<GridLevel>,
    pub amount_mode: StrategyAmountMode,
    pub futures_margin_mode: Option<FuturesMarginMode>,
    pub leverage: Option<u32>,
    pub budget: String,
    pub grid_spacing_bps: u32,
    pub membership_ready: bool,
    pub exchange_ready: bool,
    pub permissions_ready: bool,
    pub withdrawals_disabled: bool,
    pub hedge_mode_ready: bool,
    pub symbol_ready: bool,
    pub filters_ready: bool,
    pub margin_ready: bool,
    pub conflict_ready: bool,
    pub balance_ready: bool,
    pub overall_take_profit_bps: Option<u32>,
    pub overall_stop_loss_bps: Option<u32>,
    pub reference_price_source: ReferencePriceSource,
    pub post_trigger_action: PostTriggerAction,
}

#[derive(Serialize, Deserialize)]
struct StrategyTemplateSerde {
    id: String,
    name: String,
    symbol: String,
    market: StrategyMarket,
    mode: StrategyMode,
    #[serde(default)]
    strategy_type: StrategyType,
    generation: GridGeneration,
    levels: Vec<GridLevel>,
    #[serde(default)]
    amount_mode: StrategyAmountMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    futures_margin_mode: Option<FuturesMarginMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    leverage: Option<u32>,
    budget: String,
    grid_spacing_bps: u32,
    membership_ready: bool,
    exchange_ready: bool,
    permissions_ready: bool,
    withdrawals_disabled: bool,
    hedge_mode_ready: bool,
    symbol_ready: bool,
    filters_ready: bool,
    margin_ready: bool,
    conflict_ready: bool,
    balance_ready: bool,
    overall_take_profit_bps: Option<u32>,
    overall_stop_loss_bps: Option<u32>,
    #[serde(default)]
    reference_price_source: ReferencePriceSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reference_price: Option<Decimal>,
    post_trigger_action: PostTriggerAction,
}

static STRATEGY_TEMPLATE_REFERENCE_PRICES: OnceLock<Mutex<HashMap<String, Option<Decimal>>>> =
    OnceLock::new();

fn strategy_template_reference_prices() -> &'static Mutex<HashMap<String, Option<Decimal>>> {
    STRATEGY_TEMPLATE_REFERENCE_PRICES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn set_strategy_template_reference_price(template_id: &str, reference_price: Option<Decimal>) {
    let mut prices = strategy_template_reference_prices()
        .lock()
        .expect("strategy template reference prices lock");
    prices.insert(template_id.to_owned(), reference_price);
}

pub fn strategy_template_reference_price(template: &StrategyTemplate) -> Option<Decimal> {
    strategy_template_reference_prices()
        .lock()
        .expect("strategy template reference prices lock")
        .get(&template.id)
        .copied()
        .flatten()
}

impl Serialize for StrategyTemplate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        StrategyTemplateSerde {
            id: self.id.clone(),
            name: self.name.clone(),
            symbol: self.symbol.clone(),
            market: self.market,
            mode: self.mode,
            strategy_type: self.strategy_type,
            generation: self.generation,
            levels: self.levels.clone(),
            amount_mode: self.amount_mode,
            futures_margin_mode: self.futures_margin_mode,
            leverage: self.leverage,
            budget: self.budget.clone(),
            grid_spacing_bps: self.grid_spacing_bps,
            membership_ready: self.membership_ready,
            exchange_ready: self.exchange_ready,
            permissions_ready: self.permissions_ready,
            withdrawals_disabled: self.withdrawals_disabled,
            hedge_mode_ready: self.hedge_mode_ready,
            symbol_ready: self.symbol_ready,
            filters_ready: self.filters_ready,
            margin_ready: self.margin_ready,
            conflict_ready: self.conflict_ready,
            balance_ready: self.balance_ready,
            overall_take_profit_bps: self.overall_take_profit_bps,
            overall_stop_loss_bps: self.overall_stop_loss_bps,
            reference_price_source: self.reference_price_source,
            reference_price: strategy_template_reference_price(self),
            post_trigger_action: self.post_trigger_action,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StrategyTemplate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = StrategyTemplateSerde::deserialize(deserializer)?;
        set_strategy_template_reference_price(&value.id, value.reference_price);
        Ok(StrategyTemplate {
            id: value.id,
            name: value.name,
            symbol: value.symbol,
            market: value.market,
            mode: value.mode,
            strategy_type: value.strategy_type,
            generation: value.generation,
            levels: value.levels,
            amount_mode: value.amount_mode,
            futures_margin_mode: value.futures_margin_mode,
            leverage: value.leverage,
            budget: value.budget,
            grid_spacing_bps: value.grid_spacing_bps,
            membership_ready: value.membership_ready,
            exchange_ready: value.exchange_ready,
            permissions_ready: value.permissions_ready,
            withdrawals_disabled: value.withdrawals_disabled,
            hedge_mode_ready: value.hedge_mode_ready,
            symbol_ready: value.symbol_ready,
            filters_ready: value.filters_ready,
            margin_ready: value.margin_ready,
            conflict_ready: value.conflict_ready,
            balance_ready: value.balance_ready,
            overall_take_profit_bps: value.overall_take_profit_bps,
            overall_stop_loss_bps: value.overall_stop_loss_bps,
            reference_price_source: value.reference_price_source,
            post_trigger_action: value.post_trigger_action,
        })
    }
}
