use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquityPoint {
    pub timestamp_ms: i64,
    pub equity_quote: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawdownPoint {
    pub timestamp_ms: i64,
    pub drawdown_pct: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MartingaleTradeDetail {
    pub timestamp_ms: i64,
    pub symbol: String,
    pub direction: String,
    pub event_type: String,
    pub leg_index: Option<u32>,
    pub price: f64,
    pub margin_quote: f64,
    pub notional_quote: f64,
    pub leverage: f64,
    pub fee_quote: f64,
    pub slippage_quote: f64,
    pub realized_pnl_quote: f64,
    pub equity_after_quote: f64,
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
    #[serde(default)]
    pub return_drawdown_ratio: Option<f64>,
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
    #[serde(default)]
    pub drawdown_curve: Vec<DrawdownPoint>,
    #[serde(default)]
    pub trades: Vec<MartingaleTradeDetail>,
    pub rejection_reasons: Vec<String>,
}

pub fn calculate_annualized_return_pct(
    initial_equity_quote: f64,
    ending_equity_quote: f64,
    backtest_days: f64,
) -> Option<f64> {
    if !initial_equity_quote.is_finite()
        || !ending_equity_quote.is_finite()
        || !backtest_days.is_finite()
        || initial_equity_quote <= 0.0
        || backtest_days <= 0.0
    {
        return None;
    }
    if ending_equity_quote <= 0.0 {
        return Some(-100.0);
    }
    let period_return = ending_equity_quote / initial_equity_quote - 1.0;
    Some(((1.0 + period_return).powf(365.0 / backtest_days) - 1.0) * 100.0)
}

pub fn build_drawdown_curve(equity_curve: &[EquityPoint]) -> Vec<DrawdownPoint> {
    let mut peak = f64::NEG_INFINITY;
    equity_curve
        .iter()
        .filter_map(|point| {
            if !point.equity_quote.is_finite() {
                return None;
            }
            peak = peak.max(point.equity_quote);
            let drawdown_pct = if peak <= 0.0 { 0.0 } else { ((peak - point.equity_quote) / peak) * 100.0 };
            Some(DrawdownPoint { timestamp_ms: point.timestamp_ms, drawdown_pct })
        })
        .collect()
}

pub fn planned_margin_quote(first_margin_quote: f64, order_multiplier: f64, max_legs: u32) -> f64 {
    (0..max_legs)
        .map(|leg| first_margin_quote * order_multiplier.powi(leg as i32))
        .sum()
}

pub fn leveraged_position_pnl_quote(margin_quote: f64, leverage: f64, price_move_pct: f64) -> f64 {
    margin_quote * leverage * price_move_pct
}

pub fn notional_quote(margin_quote: f64, leverage: f64) -> f64 {
    margin_quote * leverage.max(1.0)
}

#[cfg(test)]
mod margin_tests {
    use super::*;

    #[test]
    fn leveraged_margin_return_uses_planned_total_margin_not_first_order_only() {
        let plan = planned_margin_quote(10.0, 2.0, 4);
        assert_eq!(plan, 150.0);

        let pnl = leveraged_position_pnl_quote(10.0, 2.0, 0.01);
        assert_eq!(pnl, 0.2);

        let return_pct = pnl / plan * 100.0;
        assert!((return_pct - 0.13333333333333333).abs() < 0.000001);
    }

    #[test]
    fn annualized_return_uses_backtest_days() {
        let annualized = calculate_annualized_return_pct(1000.0, 1100.0, 365.0).unwrap();
        assert!((annualized - 10.0).abs() < 0.000001);

        let half_year = calculate_annualized_return_pct(1000.0, 1100.0, 182.5).unwrap();
        assert!(half_year > 20.0);

        assert!(calculate_annualized_return_pct(1000.0, 1100.0, 0.0).is_none());
    }

    #[test]
    fn drawdown_curve_tracks_peak_and_decline() {
        let equity_curve = vec![
            EquityPoint { timestamp_ms: 1, equity_quote: 100.0 },
            EquityPoint { timestamp_ms: 2, equity_quote: 110.0 },
            EquityPoint { timestamp_ms: 3, equity_quote: 105.0 },
            EquityPoint { timestamp_ms: 4, equity_quote: 95.0 },
        ];
        let dd = build_drawdown_curve(&equity_curve);
        assert_eq!(dd.len(), 4);
        assert!((dd[0].drawdown_pct - 0.0).abs() < 0.000001);
        assert!((dd[1].drawdown_pct - 0.0).abs() < 0.000001);
        assert!((dd[2].drawdown_pct - 4.5454545).abs() < 0.01);
        assert!((dd[3].drawdown_pct - 13.6363636).abs() < 0.01);
    }

    #[test]
    fn notional_quote_multiplies_margin_by_leverage() {
        assert_eq!(notional_quote(10.0, 3.0), 30.0);
        assert_eq!(notional_quote(10.0, 1.0), 10.0);
        assert_eq!(notional_quote(10.0, 0.5), 10.0);
    }
}