use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquityPoint {
    pub timestamp_ms: i64,
    pub equity_quote: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MartingaleMetrics {
    pub total_return_pct: f64,
    pub max_drawdown_pct: f64,
    pub global_drawdown_pct: Option<f64>,
    pub max_strategy_drawdown_pct: Option<f64>,
    pub data_quality_score: Option<f64>,
    pub trade_count: u64,
    pub stop_count: u64,
    pub max_capital_used_quote: f64,
    pub survival_passed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MartingaleBacktestEvent {
    pub timestamp_ms: i64,
    pub event_type: String,
    pub symbol: String,
    pub strategy_instance_id: String,
    pub cycle_id: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketRegimeLabel {
    StrongUptrend,
    Uptrend,
    Range,
    Downtrend,
    StrongDowntrend,
    HighVolatility,
    ExtremeRisk,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllocationAction {
    None,
    Rebalance,
    DirectionPaused,
    DirectionOrdersCancelled,
    DirectionForcedExit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AllocationCurvePoint {
    pub timestamp_ms: i64,
    pub symbol: String,
    pub long_weight_pct: f64,
    pub short_weight_pct: f64,
    pub action: AllocationAction,
    pub reason: String,
    pub in_cooldown: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegimeTimelinePoint {
    pub timestamp_ms: i64,
    pub symbol: String,
    pub btc_regime: MarketRegimeLabel,
    pub symbol_regime: MarketRegimeLabel,
    pub extreme_risk: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CostSummary {
    pub fee_quote: f64,
    pub slippage_quote: f64,
    pub stop_loss_quote: f64,
    pub forced_exit_quote: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MartingaleBacktestResult {
    pub metrics: MartingaleMetrics,
    pub events: Vec<MartingaleBacktestEvent>,
    pub equity_curve: Vec<EquityPoint>,
    pub rejection_reasons: Vec<String>,
    #[serde(default)]
    pub allocation_curve: Vec<AllocationCurvePoint>,
    #[serde(default)]
    pub regime_timeline: Vec<RegimeTimelinePoint>,
    #[serde(default)]
    pub cost_summary: CostSummary,
    #[serde(default)]
    pub rebalance_count: u64,
    #[serde(default)]
    pub forced_exit_count: u64,
    #[serde(default)]
    pub average_allocation_hold_hours: Option<f64>,
}

impl MartingaleBacktestResult {
    pub fn with_core(
        metrics: MartingaleMetrics,
        events: Vec<MartingaleBacktestEvent>,
        equity_curve: Vec<EquityPoint>,
        rejection_reasons: Vec<String>,
    ) -> Self {
        Self {
            metrics,
            events,
            equity_curve,
            rejection_reasons,
            allocation_curve: Vec::new(),
            regime_timeline: Vec::new(),
            cost_summary: CostSummary::default(),
            rebalance_count: 0,
            forced_exit_count: 0,
            average_allocation_hold_hours: None,
        }
    }
}
