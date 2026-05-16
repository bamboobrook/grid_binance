use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub symbol: String,
    pub strategy_type: String,
    pub lower_price: f64,
    pub upper_price: f64,
    pub grid_count: u32,
    pub equal_mode: String,
    pub investment: f64,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub timestamp: String,
    pub side: String,
    pub price: f64,
    pub quantity: f64,
    pub grid_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub config: BacktestConfig,
    pub total_pnl: f64,
    pub max_drawdown: f64,
    pub trade_count: u32,
    pub win_rate: f64,
    pub annualized_return: f64,
    pub trades: Vec<TradeRecord>,
    pub equity_curve: Vec<EquityPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    pub date: String,
    pub equity: f64,
}
