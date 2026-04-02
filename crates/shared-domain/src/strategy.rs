use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StrategyStatus {
    Draft,
    Running,
    Paused,
    ErrorPaused,
    Completed,
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
    pub generation: GridGeneration,
    pub levels: Vec<GridLevel>,
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
    pub market: StrategyMarket,
    pub mode: StrategyMode,
    pub draft_revision: StrategyRevision,
    pub active_revision: Option<StrategyRevision>,
    pub runtime: StrategyRuntime,
    pub archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StrategyTemplate {
    pub id: String,
    pub name: String,
    pub symbol: String,
    pub budget: String,
    pub grid_spacing_bps: u32,
    pub membership_ready: bool,
    pub exchange_ready: bool,
    pub symbol_ready: bool,
}
