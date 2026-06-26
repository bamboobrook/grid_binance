//! Canonical Martingale capital model — the single source of truth for the
//! notional/margin split.
//!
//! Semantics (see
//! `docs/superpowers/plans/2026-06-25-martingale-margin-capital-parity-fix-for-glm.md`):
//!
//! - `first_order_quote`, `max_budget_quote` and `custom_sequence.notionals`
//!   are **leveraged order notional** — the position size sent to the
//!   exchange.
//! - Futures leg margin = notional / leverage.
//! - Spot leg margin = notional (unleveraged).
//! - Fees, slippage, funding, PnL, quantity, TP/SL and exchange orders use
//!   **notional**.
//! - Return, annualized return, drawdown, capital usage, portfolio weights and
//!   live budgets use **margin capital**.
//!
//! Concrete example (the plan's hard test): `first_order_quote = 10`,
//! `multiplier = 2`, `max_legs = 4`, futures `leverage = 2`
//! => leg notionals `[10, 20, 40, 80]`, planned notional `150`, leg margins
//! `[5, 10, 20, 40]`, planned margin `75`.

use shared_domain::martingale::{MartingaleMarketKind, MartingaleSizingModel};

use crate::martingale::rules::compute_leg_notionals;

/// Minimum notional accepted by the exchange when none is configured.
pub const DEFAULT_EXCHANGE_MIN_NOTIONAL: f64 = 0.0;

/// Effective leverage used for margin math. Futures uses the configured
/// leverage (clamped to `>= 1`); spot is unleveraged (`1.0`).
pub fn effective_leverage(market: MartingaleMarketKind, leverage: Option<u32>) -> f64 {
    match market {
        MartingaleMarketKind::Spot => 1.0,
        MartingaleMarketKind::UsdMFutures => leverage
            .map(|value| (value as f64).max(1.0))
            .unwrap_or(1.0),
    }
}

/// Leveraged order notional series (position size per leg) from sizing alone.
///
/// The `BudgetScaled` variant is still capped by its own `max_budget_quote`;
/// the external portfolio budget cap is enforced separately by the budget
/// rejection logic, so it is disabled here.
pub fn leg_notional_series(
    sizing: &MartingaleSizingModel,
    exchange_min_notional: f64,
) -> Result<Vec<f64>, String> {
    compute_leg_notionals(sizing, f64::MAX, exchange_min_notional)
}

/// Margin capital series: `notional / effective_leverage` per leg.
pub fn leg_margin_series(
    sizing: &MartingaleSizingModel,
    market: MartingaleMarketKind,
    leverage: Option<u32>,
    exchange_min_notional: f64,
) -> Result<Vec<f64>, String> {
    let leverage = effective_leverage(market, leverage);
    let notionals = leg_notional_series(sizing, exchange_min_notional)?;
    Ok(notionals.iter().map(|notional| notional / leverage).collect())
}

/// Planned total leveraged notional = sum of leg notionals.
pub fn planned_notional_quote(
    sizing: &MartingaleSizingModel,
    exchange_min_notional: f64,
) -> Result<f64, String> {
    let total: f64 = leg_notional_series(sizing, exchange_min_notional)?
        .iter()
        .sum();
    Ok(total)
}

/// Planned total margin capital = sum of leg margins.
pub fn planned_margin_quote(
    sizing: &MartingaleSizingModel,
    market: MartingaleMarketKind,
    leverage: Option<u32>,
    exchange_min_notional: f64,
) -> Result<f64, String> {
    let total: f64 = leg_margin_series(sizing, market, leverage, exchange_min_notional)?
        .iter()
        .sum();
    Ok(total)
}

/// Order quantity (base asset units) for a leveraged notional at a price.
pub fn order_quantity(notional_quote: f64, price: f64) -> f64 {
    if price > 0.0 && notional_quote.is_finite() {
        notional_quote / price
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use shared_domain::martingale::MartingaleSizingModel;

    fn multiplier_sizing(first: i64, mult: i64, legs: u32) -> MartingaleSizingModel {
        MartingaleSizingModel::Multiplier {
            first_order_quote: Decimal::from(first),
            multiplier: Decimal::from(mult),
            max_legs: legs,
        }
    }

    #[test]
    fn futures_planned_margin_uses_notional_divided_by_leverage() {
        // Plan hard test: first_order=10, mult=2, 4 legs, leverage=2
        // => planned notional 150, planned margin 75.
        let sizing = multiplier_sizing(10, 2, 4);

        let notionals = leg_notional_series(&sizing, DEFAULT_EXCHANGE_MIN_NOTIONAL).unwrap();
        assert_eq!(notionals, vec![10.0, 20.0, 40.0, 80.0]);

        let planned_notional = planned_notional_quote(&sizing, DEFAULT_EXCHANGE_MIN_NOTIONAL).unwrap();
        assert!((planned_notional - 150.0).abs() < 1e-9, "planned notional {planned_notional}");

        let margins = leg_margin_series(
            &sizing,
            MartingaleMarketKind::UsdMFutures,
            Some(2),
            DEFAULT_EXCHANGE_MIN_NOTIONAL,
        )
        .unwrap();
        assert_eq!(margins, vec![5.0, 10.0, 20.0, 40.0]);

        let planned_margin = planned_margin_quote(
            &sizing,
            MartingaleMarketKind::UsdMFutures,
            Some(2),
            DEFAULT_EXCHANGE_MIN_NOTIONAL,
        )
        .unwrap();
        assert!(
            (planned_margin - 75.0).abs() < 1e-9,
            "planned margin {planned_margin}"
        );
    }

    #[test]
    fn spot_margin_equals_notional() {
        let sizing = multiplier_sizing(10, 2, 4);
        let planned_margin = planned_margin_quote(
            &sizing,
            MartingaleMarketKind::Spot,
            None,
            DEFAULT_EXCHANGE_MIN_NOTIONAL,
        )
        .unwrap();
        // Spot is unleveraged: margin capital equals notional.
        assert!((planned_margin - 150.0).abs() < 1e-9);
    }

    #[test]
    fn higher_leverage_lowers_margin_for_same_notional() {
        let sizing = multiplier_sizing(10, 2, 4);
        let m2 = planned_margin_quote(
            &sizing,
            MartingaleMarketKind::UsdMFutures,
            Some(2),
            DEFAULT_EXCHANGE_MIN_NOTIONAL,
        )
        .unwrap();
        let m4 = planned_margin_quote(
            &sizing,
            MartingaleMarketKind::UsdMFutures,
            Some(4),
            DEFAULT_EXCHANGE_MIN_NOTIONAL,
        )
        .unwrap();
        assert!((m2 - 75.0).abs() < 1e-9);
        assert!((m4 - 37.5).abs() < 1e-9);
    }

    #[test]
    fn order_quantity_divides_notional_by_price() {
        // 150 USDT notional at 30000 USDT/BTC => 0.005 BTC.
        assert!((order_quantity(150.0, 30000.0) - 0.005).abs() < 1e-12);
        assert_eq!(order_quantity(150.0, 0.0), 0.0);
        assert_eq!(order_quantity(0.0, 30000.0), 0.0);
    }

    #[test]
    fn effective_leverage_clamps_spot_and_futures() {
        assert_eq!(effective_leverage(MartingaleMarketKind::Spot, Some(5)), 1.0);
        assert_eq!(
            effective_leverage(MartingaleMarketKind::UsdMFutures, Some(3)),
            3.0
        );
        assert_eq!(
            effective_leverage(MartingaleMarketKind::UsdMFutures, None),
            1.0
        );
    }
}
