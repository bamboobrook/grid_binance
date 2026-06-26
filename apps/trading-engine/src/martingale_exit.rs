use backtest_engine::martingale::kline_engine::{DEFAULT_FEE_BPS, DEFAULT_SLIPPAGE_BPS};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleMarketKind, MartingaleStopLossModel, MartingaleStrategyConfig,
};

/// Margin-based net-PnL drawdown percent, matching backtest `strategy_net_pnl` / `capital_used_quote`.
///
/// `invested = qty * avg / leverage` == backtest `capital_used_quote` (sum of leg margins).
/// `net = realized + unrealized - entry_fees - exit_cost`, where `exit_cost` applies the shared
/// backtest fee+slippage rate to the current close notional. The caller triggers the stop when
/// the returned value `>= pct_bps as f64 / 100.0`.
///
/// At SL-evaluation time the cycle is open and losing (no TP has fired), so current-cycle
/// realized PnL is typically ~0; the dominant term is the leverage-amplified unrealized loss.
pub fn martingale_strategy_drawdown_pct(
    config: &MartingaleStrategyConfig,
    quantity: Decimal,
    average_entry_price: Decimal,
    current_price: Decimal,
    realized_pnl: Decimal,
    entry_fees: Decimal,
) -> Option<f64> {
    let pct_bps = match &config.stop_loss {
        Some(MartingaleStopLossModel::StrategyDrawdownPct { pct_bps }) => *pct_bps,
        _ => return None,
    };
    // Respect market: Spot is always 1.0x (matches backtest `effective_leverage`); futures
    // honor the configured leverage.
    let leverage = if config.market == MartingaleMarketKind::Spot {
        1.0
    } else {
        config.leverage.unwrap_or(1).max(1) as f64
    };
    let qty = quantity.abs().to_f64()?;
    let avg = average_entry_price.to_f64()?;
    let price = current_price.to_f64()?;
    if leverage <= 0.0 || qty <= 0.0 || avg <= 0.0 {
        return None;
    }
    let invested = qty * avg / leverage; // == backtest capital_used_quote
    let dir_sign = if config.direction == MartingaleDirection::Long {
        1.0
    } else {
        -1.0
    };
    let unrealized = (price - avg) * qty * dir_sign;
    let realized = realized_pnl.to_f64().unwrap_or(0.0);
    let fees = entry_fees.to_f64().unwrap_or(0.0);
    let exit_cost = (qty * price) * (DEFAULT_FEE_BPS + DEFAULT_SLIPPAGE_BPS) / 10_000.0;
    let net = realized + unrealized - fees - exit_cost;
    let _ = pct_bps; // pct_bps consumed by caller for the threshold compare
    Some((-net).max(0.0) / invested * 100.0)
}
