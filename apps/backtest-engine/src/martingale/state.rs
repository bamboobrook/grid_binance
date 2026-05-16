use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use shared_domain::martingale::MartingaleDirection;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MartingalePortfolioState {
    pub cash_quote: f64,
    pub reserved_margin_quote: f64,
    pub realized_pnl_quote: f64,
    pub equity_peak_quote: f64,
    pub symbols: BTreeMap<String, MartingaleSymbolState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MartingaleSymbolState {
    pub gross_exposure_quote: f64,
    pub net_exposure_quote: f64,
    pub long_exposure_quote: f64,
    pub short_exposure_quote: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MartingaleCycleState {
    pub cycle_id: String,
    pub direction: MartingaleDirection,
    pub anchor_price: f64,
    pub legs: Vec<MartingaleLegState>,
    pub trailing_high_watermark: Option<f64>,
    pub trailing_low_watermark: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MartingaleLegState {
    pub leg_index: u32,
    pub price: f64,
    pub quantity: f64,
    pub notional_quote: f64,
    pub fee_quote: f64,
    pub slippage_quote: f64,
}
