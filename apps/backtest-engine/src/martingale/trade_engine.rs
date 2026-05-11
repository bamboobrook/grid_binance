use shared_domain::martingale::MartingalePortfolioConfig;

use crate::market_data::{AggTrade, KlineBar};
use crate::martingale::kline_engine::run_kline_screening;
use crate::martingale::metrics::MartingaleBacktestResult;

pub fn run_trade_refinement(
    portfolio: MartingalePortfolioConfig,
    trades: &[AggTrade],
) -> Result<MartingaleBacktestResult, String> {
    portfolio.validate()?;
    let bars = trades_to_ordered_price_bars(trades)?;
    let mut result = run_kline_screening(portfolio, &bars)?;
    result.events.insert(
        0,
        crate::martingale::metrics::MartingaleBacktestEvent {
            timestamp_ms: bars.first().map(|bar| bar.open_time_ms).unwrap_or_default(),
            event_type: "trade_refinement_started".to_string(),
            symbol: bars
                .first()
                .map(|bar| bar.symbol.clone())
                .unwrap_or_default(),
            strategy_instance_id: String::new(),
            cycle_id: None,
            detail: format!("agg_trade_count={}", trades.len()),
        },
    );
    Ok(result)
}

pub fn trades_to_ordered_price_bars(trades: &[AggTrade]) -> Result<Vec<KlineBar>, String> {
    let mut ordered: Vec<(usize, AggTrade)> = trades.iter().cloned().enumerate().collect();
    ordered.sort_by(|left, right| {
        left.1
            .trade_time_ms
            .cmp(&right.1.trade_time_ms)
            .then_with(|| left.0.cmp(&right.0))
    });

    let mut bars = Vec::with_capacity(ordered.len());
    let mut last_timestamp_ms = None;
    for (_, trade) in ordered {
        validate_trade(&trade)?;
        let timestamp_ms = match last_timestamp_ms {
            Some(previous) if trade.trade_time_ms <= previous => previous
                .checked_add(1)
                .ok_or_else(|| "trade timestamp overflow while preserving order".to_string())?,
            _ => trade.trade_time_ms,
        };
        last_timestamp_ms = Some(timestamp_ms);
        bars.push(KlineBar {
            symbol: trade.symbol,
            open_time_ms: timestamp_ms,
            open: trade.price,
            high: trade.price,
            low: trade.price,
            close: trade.price,
            volume: trade.quantity,
        });
    }
    Ok(bars)
}

fn validate_trade(trade: &AggTrade) -> Result<(), String> {
    if trade.symbol.trim().is_empty() {
        return Err("trade symbol cannot be empty".to_string());
    }
    if !trade.price.is_finite() || trade.price <= 0.0 {
        return Err(format!(
            "trade price must be finite and positive, got {}",
            trade.price
        ));
    }
    if !trade.quantity.is_finite() || trade.quantity < 0.0 {
        return Err(format!(
            "trade quantity must be finite and non-negative, got {}",
            trade.quantity
        ));
    }
    Ok(())
}
