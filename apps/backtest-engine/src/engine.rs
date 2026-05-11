use crate::model::{BacktestConfig, BacktestResult, EquityPoint, TradeRecord};
use serde::{Deserialize, Serialize};

pub struct BacktestEngine;

impl BacktestEngine {
    pub fn run(config: BacktestConfig, klines: Vec<KlineRecord>) -> anyhow::Result<BacktestResult> {
        if klines.is_empty() {
            anyhow::bail!("no kline data provided");
        }

        let grid_step = (config.upper_price - config.lower_price) / config.grid_count as f64;
        let mut trades: Vec<TradeRecord> = Vec::new();
        let mut equity_curve: Vec<EquityPoint> = Vec::new();
        let mut cash = config.investment;
        let mut position = 0.0_f64;
        let mut peak_equity = config.investment;
        let mut max_drawdown = 0.0_f64;

        let per_grid_investment = config.investment / config.grid_count as f64;

        for kline in &klines {
            let mut grid_idx = ((kline.close - config.lower_price) / grid_step).floor() as i32;
            grid_idx = grid_idx.clamp(0, config.grid_count as i32 - 1);

            let buy_price = config.lower_price + grid_idx as f64 * grid_step;
            let sell_price = buy_price + grid_step;

            if kline.close <= buy_price && cash >= per_grid_investment {
                let qty = per_grid_investment / buy_price;
                cash -= per_grid_investment;
                position += qty;
                trades.push(TradeRecord {
                    timestamp: kline.time.clone(),
                    side: "Buy".into(),
                    price: buy_price,
                    quantity: qty,
                    grid_index: grid_idx as u32,
                });
            } else if kline.close >= sell_price && position * sell_price > 0.0 {
                let qty = per_grid_investment / sell_price;
                let sell_qty = qty.min(position);
                cash += sell_qty * sell_price;
                position -= sell_qty;
                trades.push(TradeRecord {
                    timestamp: kline.time.clone(),
                    side: "Sell".into(),
                    price: sell_price,
                    quantity: sell_qty,
                    grid_index: grid_idx as u32,
                });
            }

            let equity = cash + position * kline.close;
            peak_equity = peak_equity.max(equity);
            let drawdown = (peak_equity - equity) / peak_equity;
            max_drawdown = max_drawdown.max(drawdown);

            equity_curve.push(EquityPoint {
                date: kline.time.clone(),
                equity: (equity * 100.0).round() / 100.0,
            });
        }

        let final_equity = cash + position * klines.last().map(|k| k.close).unwrap_or(0.0);
        let total_pnl = final_equity - config.investment;
        let wins = trades
            .chunks(2)
            .filter(|pair| {
                if pair.len() == 2 && pair[0].side == "Buy" && pair[1].side == "Sell" {
                    pair[1].price > pair[0].price
                } else {
                    false
                }
            })
            .count();
        let total_pairs = trades.len() / 2;
        let win_rate = if total_pairs > 0 {
            wins as f64 / total_pairs as f64
        } else {
            0.0
        };

        let days = klines.len() as f64;
        let annualized_return = if days > 0.0 && config.investment > 0.0 {
            ((final_equity / config.investment).powf(365.0 / days) - 1.0) * 100.0
        } else {
            0.0
        };

        Ok(BacktestResult {
            config,
            total_pnl: (total_pnl * 100.0).round() / 100.0,
            max_drawdown: (max_drawdown * 10000.0).round() / 100.0,
            trade_count: trades.len() as u32,
            win_rate: (win_rate * 100.0).round() / 100.0,
            annualized_return: (annualized_return * 100.0).round() / 100.0,
            trades,
            equity_curve,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineRecord {
    pub time: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}
