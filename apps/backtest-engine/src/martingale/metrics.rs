use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquityPoint {
    pub timestamp_ms: i64,
    pub equity_quote: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MartingaleMetrics {
    pub total_return_pct: f64,
    #[serde(default)]
    pub annualized_return_pct: Option<f64>,
    pub max_drawdown_pct: f64,
    pub global_drawdown_pct: Option<f64>,
    pub max_strategy_drawdown_pct: Option<f64>,
    #[serde(default)]
    pub monthly_win_rate_pct: Option<f64>,
    #[serde(default)]
    pub max_leverage_used: Option<f64>,
    #[serde(default)]
    pub min_liquidation_buffer_pct: Option<f64>,
    #[serde(default)]
    pub total_fee_quote: Option<f64>,
    #[serde(default)]
    pub total_slippage_quote: Option<f64>,
    #[serde(default)]
    pub planned_margin_quote: Option<f64>,
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
pub struct MartingaleBacktestResult {
    pub metrics: MartingaleMetrics,
    pub events: Vec<MartingaleBacktestEvent>,
    pub equity_curve: Vec<EquityPoint>,
    pub rejection_reasons: Vec<String>,
}
