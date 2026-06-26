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

use shared_domain::martingale::{MartingaleMarketKind, MartingaleSizingModel, MartingaleStrategyConfig};

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

/// Epsilon for margin cap comparisons (compensates f64 drift).
const CAP_EPS: f64 = 1e-9;

/// One leg's projected capital under capping.
#[derive(Debug, Clone, PartialEq)]
pub struct LegCapitalProjection {
    pub leg_index: u32,
    pub notional_quote: f64,
    pub margin_quote: f64,
    pub accepted: bool,
    pub skip_reason: Option<String>,
}

/// Per-strategy projection: full theoretical series vs budget-capped feasible legs.
#[derive(Debug, Clone, PartialEq)]
pub struct StrategyCapitalProjection {
    pub strategy_id: String,
    pub leverage: f64,
    /// The margin cap actually applied to this strategy's walk
    /// (= min(weight_cap floored at first-leg margin, remaining global margin)).
    pub strategy_margin_cap_quote: f64,
    pub full_series_margin_quote: f64,
    pub full_series_notional_quote: f64,
    pub budget_capped_margin_quote: f64,
    pub budget_capped_notional_quote: f64,
    pub first_leg_margin_quote: f64,
    pub first_leg_notional_quote: f64,
    pub first_leg_accepted: bool,
    pub legs: Vec<LegCapitalProjection>,
}

/// Portfolio projection under a global margin cap.
#[derive(Debug, Clone, PartialEq)]
pub struct PortfolioCapitalProjection {
    pub global_margin_cap_quote: f64,
    pub strategies: Vec<StrategyCapitalProjection>,
    pub full_series_margin_quote: f64,
    pub full_series_notional_quote: f64,
    pub budget_capped_margin_quote: f64,
    pub budget_capped_notional_quote: f64,
    /// Sum of each strategy's ACCEPTED first-leg margin (the realistic
    /// first-cycle committed margin).
    pub first_leg_margin_quote: f64,
    pub first_leg_notional_quote: f64,
    /// Entry fee on the budget-capped (accepted) notional.
    pub projected_fee_quote: f64,
    /// budget_capped_margin * (1 + fee_buffer_pct/100) + projected_fee.
    pub required_with_buffer_quote: f64,
    /// True iff every strategy accepted its first leg.
    pub all_strategies_can_start: bool,
}

/// First-leg margin = first leg notional / effective leverage (0.0 if no legs).
///
/// Note: the leg series is computed WITHOUT the exchange min-notional filter
/// (we pass 0.0 to `leg_notional_series`) so the first leg's theoretical margin
/// is always returned; the caller decides separately whether the first leg is
/// acceptable under the exchange minimum.
pub fn first_leg_margin_quote(
    sizing: &MartingaleSizingModel,
    market: MartingaleMarketKind,
    leverage: Option<u32>,
    _exchange_min_notional: f64,
) -> Result<f64, String> {
    let lev = effective_leverage(market, leverage);
    let notionals = leg_notional_series(sizing, DEFAULT_EXCHANGE_MIN_NOTIONAL)?;
    if notionals.is_empty() {
        Ok(0.0)
    } else {
        Ok(notionals[0] / lev)
    }
}

/// Project one strategy's legs under a margin cap. `strategy_margin_cap` is the
/// per-strategy margin cap (already floored at first-leg margin by the caller);
/// `available_global_margin` is the remaining global margin pool this strategy
/// may draw from. A leg is accepted iff its notional >= exchange_min_notional
/// AND cumulative margin (this strategy) <= strategy_margin_cap AND cumulative
/// margin <= available_global_margin. Legs are walked in series order.
pub fn project_strategy_capital(
    strategy_id: &str,
    sizing: &MartingaleSizingModel,
    market: MartingaleMarketKind,
    leverage: Option<u32>,
    strategy_margin_cap: f64,
    available_global_margin: f64,
    exchange_min_notional: f64,
) -> Result<StrategyCapitalProjection, String> {
    let lev = effective_leverage(market, leverage);
    // Fetch the FULL theoretical series without the exchange min-notional filter
    // (`leg_notional_series` otherwise rejects the whole series when any leg is
    // below the min). The per-leg min-notional skip is applied in the walk below.
    let notionals = leg_notional_series(sizing, DEFAULT_EXCHANGE_MIN_NOTIONAL)?;

    if notionals.is_empty() {
        return Ok(StrategyCapitalProjection {
            strategy_id: strategy_id.to_string(),
            leverage: lev,
            strategy_margin_cap_quote: strategy_margin_cap,
            full_series_margin_quote: 0.0,
            full_series_notional_quote: 0.0,
            budget_capped_margin_quote: 0.0,
            budget_capped_notional_quote: 0.0,
            first_leg_margin_quote: 0.0,
            first_leg_notional_quote: 0.0,
            first_leg_accepted: false,
            legs: Vec::new(),
        });
    }

    let margins: Vec<f64> = notionals.iter().map(|n| n / lev).collect();
    let full_series_margin: f64 = margins.iter().sum();
    let full_series_notional: f64 = notionals.iter().sum();
    let first_leg_margin = margins[0];
    let first_leg_notional = notionals[0];

    // Walk the full series for diagnostics, recording each leg's accept/skip
    // status. A leg is accepted iff (a) its notional meets the exchange minimum,
    // (b) cumulative accepted margin stays within the per-strategy cap, and
    // (c) cumulative accepted margin stays within the remaining global pool.
    // Once a leg is rejected, no later leg is accepted (martingale legs are
    // sequential: leg N+1 cannot trigger without leg N active), but we keep
    // walking to record the full diagnostic picture of which legs would be
    // capped. `budget_capped_*` therefore sums the contiguous accepted prefix.
    let mut legs: Vec<LegCapitalProjection> = Vec::with_capacity(notionals.len());
    let mut cum_strategy = 0.0f64;
    let mut series_broken = false;

    for (i, (&notional, &margin)) in notionals.iter().zip(margins.iter()).enumerate() {
        let (accepted, skip_reason) = if series_broken {
            // A prior leg already broke the chain; this leg cannot be placed.
            (false, Some("prior leg not accepted".to_string()))
        } else if notional < exchange_min_notional {
            series_broken = true;
            (false, Some("below exchange min notional".to_string()))
        } else if cum_strategy + margin <= strategy_margin_cap + CAP_EPS
            && cum_strategy + margin <= available_global_margin + CAP_EPS
        {
            cum_strategy += margin;
            (true, None)
        } else {
            series_broken = true;
            (false, Some("exceeds margin cap".to_string()))
        };
        legs.push(LegCapitalProjection {
            leg_index: i as u32,
            notional_quote: notional,
            margin_quote: margin,
            accepted,
            skip_reason,
        });
    }

    // budget_capped sums only the contiguous accepted prefix.
    let budget_capped_margin: f64 = legs
        .iter()
        .take_while(|l| l.accepted)
        .map(|l| l.margin_quote)
        .sum();
    let budget_capped_notional: f64 = legs
        .iter()
        .take_while(|l| l.accepted)
        .map(|l| l.notional_quote)
        .sum();
    let first_leg_accepted = legs[0].accepted;

    Ok(StrategyCapitalProjection {
        strategy_id: strategy_id.to_string(),
        leverage: lev,
        strategy_margin_cap_quote: strategy_margin_cap,
        full_series_margin_quote: full_series_margin,
        full_series_notional_quote: full_series_notional,
        budget_capped_margin_quote: budget_capped_margin,
        budget_capped_notional_quote: budget_capped_notional,
        first_leg_margin_quote: first_leg_margin,
        first_leg_notional_quote: first_leg_notional,
        first_leg_accepted,
        legs,
    })
}

/// Project a whole portfolio under a global margin cap. Strategies are walked
/// in `strategies` order, drawing from a shared global margin pool (remaining
/// decreases by each strategy's accepted margin). Per-strategy cap =
/// `(global_margin_cap * weight_factor).max(first_leg_margin)`, where
/// weight_factor = weights[strategy_id] (fallback 1/strategies.len()).
pub fn project_portfolio_capital(
    strategies: &[MartingaleStrategyConfig],
    weights: &std::collections::HashMap<String, f64>,
    global_margin_cap: f64,
    exchange_min_notional: f64,
    entry_fee_bps: f64,
    fee_buffer_pct: f64,
) -> Result<PortfolioCapitalProjection, String> {
    if strategies.is_empty() {
        return Ok(PortfolioCapitalProjection {
            global_margin_cap_quote: global_margin_cap,
            strategies: Vec::new(),
            full_series_margin_quote: 0.0,
            full_series_notional_quote: 0.0,
            budget_capped_margin_quote: 0.0,
            budget_capped_notional_quote: 0.0,
            first_leg_margin_quote: 0.0,
            first_leg_notional_quote: 0.0,
            projected_fee_quote: 0.0,
            required_with_buffer_quote: 0.0,
            all_strategies_can_start: true,
        });
    }

    let equal = 1.0 / strategies.len() as f64;
    let mut remaining_global = global_margin_cap;
    let mut projections: Vec<StrategyCapitalProjection> = Vec::with_capacity(strategies.len());

    for strategy in strategies {
        let wf = weights
            .get(&strategy.strategy_id)
            .copied()
            .filter(|w| *w > 0.0)
            .unwrap_or(equal);
        let weight_cap = global_margin_cap * wf;
        let flm = first_leg_margin_quote(
            &strategy.sizing,
            strategy.market,
            strategy.leverage,
            exchange_min_notional,
        )?;
        let strat_cap = weight_cap.max(flm);
        let proj = project_strategy_capital(
            &strategy.strategy_id,
            &strategy.sizing,
            strategy.market,
            strategy.leverage,
            strat_cap,
            remaining_global,
            exchange_min_notional,
        )?;
        remaining_global = (remaining_global - proj.budget_capped_margin_quote).max(0.0);
        projections.push(proj);
    }

    let full_series_margin: f64 = projections.iter().map(|p| p.full_series_margin_quote).sum();
    let full_series_notional: f64 =
        projections.iter().map(|p| p.full_series_notional_quote).sum();
    let budget_capped_margin: f64 =
        projections.iter().map(|p| p.budget_capped_margin_quote).sum();
    let budget_capped_notional: f64 =
        projections.iter().map(|p| p.budget_capped_notional_quote).sum();
    let first_leg_margin: f64 = projections
        .iter()
        .filter(|p| p.first_leg_accepted)
        .map(|p| p.first_leg_margin_quote)
        .sum();
    let first_leg_notional: f64 = projections
        .iter()
        .filter(|p| p.first_leg_accepted)
        .map(|p| p.first_leg_notional_quote)
        .sum();
    let projected_fee = budget_capped_notional * entry_fee_bps / 10_000.0;
    let required_with_buffer =
        budget_capped_margin * (1.0 + fee_buffer_pct / 100.0) + projected_fee;
    let all_strategies_can_start = projections.iter().all(|p| p.first_leg_accepted);

    Ok(PortfolioCapitalProjection {
        global_margin_cap_quote: global_margin_cap,
        strategies: projections,
        full_series_margin_quote: full_series_margin,
        full_series_notional_quote: full_series_notional,
        budget_capped_margin_quote: budget_capped_margin,
        budget_capped_notional_quote: budget_capped_notional,
        first_leg_margin_quote: first_leg_margin,
        first_leg_notional_quote: first_leg_notional,
        projected_fee_quote: projected_fee,
        required_with_buffer_quote: required_with_buffer,
        all_strategies_can_start,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use shared_domain::martingale::{
        MartingaleDirection, MartingaleDirectionMode, MartingaleEntryTrigger,
        MartingaleMarginMode, MartingaleRiskLimits, MartingaleSpacingModel,
        MartingaleStrategyConfig, MartingaleTakeProfitModel,
    };
    use std::collections::HashMap;

    fn multiplier_sizing(first: i64, mult: i64, legs: u32) -> MartingaleSizingModel {
        MartingaleSizingModel::Multiplier {
            first_order_quote: Decimal::from(first),
            multiplier: Decimal::from(mult),
            max_legs: legs,
        }
    }

    /// Minimal `MartingaleStrategyConfig` for projection tests. The struct does
    /// not derive `Default`, so we fill the non-under-test fields explicitly.
    fn strat(
        id: &str,
        sizing: MartingaleSizingModel,
        market: MartingaleMarketKind,
        leverage: Option<u32>,
    ) -> MartingaleStrategyConfig {
        let margin_mode = match market {
            MartingaleMarketKind::Spot => None,
            MartingaleMarketKind::UsdMFutures => Some(MartingaleMarginMode::Isolated),
        };
        MartingaleStrategyConfig {
            strategy_id: id.to_string(),
            symbol: id.to_string(),
            market,
            direction: MartingaleDirection::Long,
            direction_mode: MartingaleDirectionMode::LongOnly,
            margin_mode,
            leverage,
            spacing: MartingaleSpacingModel::FixedPercent { step_bps: 100 },
            sizing,
            take_profit: MartingaleTakeProfitModel::Percent { bps: 100 },
            stop_loss: None,
            indicators: Vec::new(),
            entry_triggers: vec![MartingaleEntryTrigger::Immediate],
            risk_limits: MartingaleRiskLimits::default(),
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

    #[test]
    fn first_leg_margin_divides_first_notional_by_leverage() {
        // Multiplier foq=10, futures lev=2 => first notional 10, margin 5.
        let sizing = multiplier_sizing(10, 2, 4);
        let futures_margin = first_leg_margin_quote(
            &sizing,
            MartingaleMarketKind::UsdMFutures,
            Some(2),
            DEFAULT_EXCHANGE_MIN_NOTIONAL,
        )
        .unwrap();
        assert!(
            (futures_margin - 5.0).abs() < 1e-9,
            "futures first leg margin {futures_margin}"
        );
        // Spot: margin equals notional.
        let spot_margin = first_leg_margin_quote(
            &sizing,
            MartingaleMarketKind::Spot,
            None,
            DEFAULT_EXCHANGE_MIN_NOTIONAL,
        )
        .unwrap();
        assert!(
            (spot_margin - 10.0).abs() < 1e-9,
            "spot first leg margin {spot_margin}"
        );
    }

    #[test]
    fn project_strategy_caps_legs_at_margin_cap() {
        // notionals [10,20,40,80], margins [5,10,20,40], full_series_margin=75.
        let sizing = multiplier_sizing(10, 2, 4);
        let proj = project_strategy_capital(
            "a",
            &sizing,
            MartingaleMarketKind::UsdMFutures,
            Some(2),
            15.0, // strategy_margin_cap
            1000.0, // available_global_margin
            DEFAULT_EXCHANGE_MIN_NOTIONAL,
        )
        .unwrap();

        assert!((proj.full_series_margin_quote - 75.0).abs() < 1e-9);
        assert!((proj.full_series_notional_quote - 150.0).abs() < 1e-9);
        assert!((proj.first_leg_margin_quote - 5.0).abs() < 1e-9);
        assert!((proj.first_leg_notional_quote - 10.0).abs() < 1e-9);
        assert!(proj.first_leg_accepted);
        // Legs 0,1 accepted (cum 15), legs 2,3 skipped.
        assert_eq!(proj.legs.len(), 4);
        assert!(proj.legs[0].accepted);
        assert!(proj.legs[1].accepted);
        assert!(!proj.legs[2].accepted);
        assert!(!proj.legs[3].accepted);
        assert_eq!(
            proj.legs[2].skip_reason.as_deref(),
            Some("exceeds margin cap")
        );
        assert!((proj.budget_capped_margin_quote - 15.0).abs() < 1e-9);
        assert!((proj.budget_capped_notional_quote - 30.0).abs() < 1e-9);
    }

    #[test]
    fn project_strategy_global_pool_blocks_after_exhausted() {
        let sizing = multiplier_sizing(10, 2, 4);
        let proj = project_strategy_capital(
            "a",
            &sizing,
            MartingaleMarketKind::UsdMFutures,
            Some(2),
            1000.0, // strategy_margin_cap (generous)
            15.0, // available_global_margin (tight)
            DEFAULT_EXCHANGE_MIN_NOTIONAL,
        )
        .unwrap();

        // Legs 0,1 accepted (cum 15 == available_global), leg 2 skipped.
        assert!(proj.legs[0].accepted);
        assert!(proj.legs[1].accepted);
        assert!(!proj.legs[2].accepted);
        assert!(proj.legs[2].skip_reason.is_some());
        assert!((proj.budget_capped_margin_quote - 15.0).abs() < 1e-9);
    }

    #[test]
    fn project_strategy_rejects_below_min_notional() {
        let sizing = multiplier_sizing(10, 2, 4);
        let proj = project_strategy_capital(
            "a",
            &sizing,
            MartingaleMarketKind::UsdMFutures,
            Some(2),
            1000.0,
            1000.0,
            25.0, // exchange_min_notional: leg 0 notional 10 < 25
        )
        .unwrap();

        assert!(!proj.first_leg_accepted);
        assert_eq!(
            proj.legs[0].skip_reason.as_deref(),
            Some("below exchange min notional")
        );
        assert!((proj.budget_capped_margin_quote - 0.0).abs() < 1e-9);
    }

    #[test]
    fn project_portfolio_applies_weights_and_global_pool() {
        let sizing = multiplier_sizing(10, 2, 4);
        let strategies = vec![
            strat("a", sizing.clone(), MartingaleMarketKind::UsdMFutures, Some(2)),
            strat("b", sizing, MartingaleMarketKind::UsdMFutures, Some(2)),
        ];
        let mut weights = HashMap::new();
        weights.insert("a".to_string(), 0.5);
        weights.insert("b".to_string(), 0.5);

        let proj = project_portfolio_capital(
            &strategies,
            &weights,
            20.0, // global_margin_cap
            DEFAULT_EXCHANGE_MIN_NOTIONAL,
            4.5,  // entry_fee_bps
            5.0,  // fee_buffer_pct
        )
        .unwrap();

        // Each weight_cap=10, flm=5, strat_cap=10.
        // Strategy a: leg0 (5) accepted, leg1 (cum15>10) skipped. consumed 5.
        // Strategy b: available=15, leg0 (5) accepted, leg1 (cum15>10) skipped. consumed 5.
        assert!((proj.budget_capped_margin_quote - 10.0).abs() < 1e-9);
        assert!((proj.budget_capped_notional_quote - 20.0).abs() < 1e-9);
        assert!((proj.first_leg_margin_quote - 10.0).abs() < 1e-9);
        assert!(proj.all_strategies_can_start);
        // projected_fee = 20 * 4.5 / 10000 = 0.009
        assert!((proj.projected_fee_quote - 0.009).abs() < 1e-9);
        // required_with_buffer = 10 * 1.05 + 0.009 = 10.509
        assert!((proj.required_with_buffer_quote - 10.509).abs() < 1e-9);
    }

    #[test]
    fn project_portfolio_all_can_start_false_when_global_exhausted() {
        let sizing = multiplier_sizing(10, 2, 1);
        let strategies = vec![
            strat("a", sizing.clone(), MartingaleMarketKind::UsdMFutures, Some(1)),
            strat("b", sizing, MartingaleMarketKind::UsdMFutures, Some(1)),
        ];
        let mut weights = HashMap::new();
        weights.insert("a".to_string(), 0.5);
        weights.insert("b".to_string(), 0.5);

        let proj = project_portfolio_capital(
            &strategies,
            &weights,
            8.0, // global_margin_cap (< leg margin 10 each)
            DEFAULT_EXCHANGE_MIN_NOTIONAL,
            0.0,
            0.0,
        )
        .unwrap();

        // flm each=10, weight_cap each=4, strat_cap=max(4,10)=10.
        // Strategy a: available=8, leg0 margin 10 > 8 -> not accepted.
        // Strategy b: available=8, leg0 margin 10 > 8 -> not accepted.
        assert!(!proj.all_strategies_can_start);
        assert!((proj.budget_capped_margin_quote - 0.0).abs() < 1e-9);
    }

    #[test]
    fn project_portfolio_full_series_diagnostic_unbounded_by_cap() {
        let sizing = multiplier_sizing(10, 2, 4);
        let strategies =
            vec![strat("a", sizing, MartingaleMarketKind::UsdMFutures, Some(2))];
        let mut weights = HashMap::new();
        weights.insert("a".to_string(), 1.0);

        let proj = project_portfolio_capital(
            &strategies,
            &weights,
            5.0, // global_margin_cap
            DEFAULT_EXCHANGE_MIN_NOTIONAL,
            0.0,
            0.0,
        )
        .unwrap();

        // full_series_margin must still be 75 (diagnostic, NOT capped).
        assert!((proj.full_series_margin_quote - 75.0).abs() < 1e-9);
        // budget_capped_margin <= 5.
        assert!(proj.budget_capped_margin_quote <= 5.0);
        // first_leg_margin = 5 (== cap, accepted since cum5<=5).
        assert!((proj.first_leg_margin_quote - 5.0).abs() < 1e-9);
    }
}
