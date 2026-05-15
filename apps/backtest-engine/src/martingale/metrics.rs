#[derive(Debug, Clone, PartialEq)]
pub struct EquityPoint {
    pub timestamp_ms: i64,
    pub equity_quote: f64,
}

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub struct MartingaleBacktestEvent {
    pub timestamp_ms: i64,
    pub event_type: String,
    pub symbol: String,
    pub strategy_instance_id: String,
    pub cycle_id: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MartingaleBacktestResult {
    pub metrics: MartingaleMetrics,
    pub events: Vec<MartingaleBacktestEvent>,
    pub equity_curve: Vec<EquityPoint>,
    pub rejection_reasons: Vec<String>,
}
