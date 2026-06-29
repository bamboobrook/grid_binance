use backtest_engine::martingale::indicator_runtime::IndicatorRuntimeContext;
use backtest_engine::martingale::kline_engine::{DEFAULT_FEE_BPS, DEFAULT_SLIPPAGE_BPS};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleDirection, MartingaleMarketKind, MartingaleStopLossModel, MartingaleStrategyConfig,
};

/// Margin-based net-PnL drawdown percent (core arithmetic), shared by all SL variants that
/// need a drawdown number. Matches backtest `strategy_net_pnl` / `capital_used_quote`.
///
/// `invested = qty * avg / leverage` == backtest `capital_used_quote` (sum of leg margins).
/// `net = realized + unrealized - entry_fees - exit_cost`, where `exit_cost` applies the shared
/// backtest fee+slippage rate to the current close notional. Returned as a positive percentage
/// of invested capital; the caller compares against the variant's threshold.
///
/// At SL-evaluation time the cycle is open and losing (no TP has fired), so current-cycle
/// realized PnL is typically ~0; the dominant term is the leverage-amplified unrealized loss.
fn martingale_net_drawdown_pct(
    config: &MartingaleStrategyConfig,
    quantity: Decimal,
    average_entry_price: Decimal,
    current_price: Decimal,
    realized_pnl: Decimal,
    entry_fees: Decimal,
) -> Option<f64> {
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
    Some((-net).max(0.0) / invested * 100.0)
}

/// Public drawdown-percent helper for the `StrategyDrawdownPct` variant. Returns `None` for any
/// other variant so the existing SL-drawdown branch in `martingale_exit_signal` skips cleanly.
/// Delegates to [`martingale_net_drawdown_pct`]; callers compare `>= pct_bps as f64 / 100.0`.
pub fn martingale_strategy_drawdown_pct(
    config: &MartingaleStrategyConfig,
    quantity: Decimal,
    average_entry_price: Decimal,
    current_price: Decimal,
    realized_pnl: Decimal,
    entry_fees: Decimal,
) -> Option<f64> {
    match &config.stop_loss {
        Some(MartingaleStopLossModel::StrategyDrawdownPct { .. }) => {
            martingale_net_drawdown_pct(
                config,
                quantity,
                average_entry_price,
                current_price,
                realized_pnl,
                entry_fees,
            )
        }
        _ => None,
    }
}

/// `RegimeBreakStop` evaluation for live parity with backtest `triggered_stop` (Task 3).
///
/// AND of two conditions (both must hold), matching backtest `kline_engine.rs:1448-1479`:
///   1. net-PnL drawdown `>= drawdown_pct_bps / 100`
///   2. close crossed the EMA against direction (long: close < ema; short: close > ema)
///
/// Returns `None` when the SL variant is not `RegimeBreakStop`. Returns `Some(false)` when
/// drawdown is below threshold or the EMA is still warming up (`latest_ema == None`) — this
/// mirrors backtest's "EMA warmup — do not trigger" and "drawdown below threshold" early
/// returns, so live never fires prematurely. Returns `Some(true)` only when both hold.
///
/// The EMA is read from the persisted `IndicatorRuntimeContext` (the same one the ATR loop
/// maintains), so it reflects the most recently completed 1m bar — bar-level parity with the
/// backtest (no look-ahead).
pub fn martingale_regime_break_triggered(
    config: &MartingaleStrategyConfig,
    quantity: Decimal,
    average_entry_price: Decimal,
    current_price: Decimal,
    realized_pnl: Decimal,
    entry_fees: Decimal,
    indicator_ctx: &mut IndicatorRuntimeContext,
) -> Option<bool> {
    let (ema_period, dd_bps) = match &config.stop_loss {
        Some(MartingaleStopLossModel::RegimeBreakStop {
            ema_period,
            drawdown_pct_bps,
        }) => (*ema_period, *drawdown_pct_bps),
        _ => return None,
    };
    let dd = martingale_net_drawdown_pct(
        config,
        quantity,
        average_entry_price,
        current_price,
        realized_pnl,
        entry_fees,
    )?;
    if dd < dd_bps as f64 / 100.0 {
        return Some(false);
    }
    let ema = indicator_ctx.latest_ema(&config.symbol, ema_period as usize)?;
    let price = current_price.to_f64()?;
    let broke = match config.direction {
        MartingaleDirection::Long => price < ema,
        MartingaleDirection::Short => price > ema,
    };
    Some(broke)
}
