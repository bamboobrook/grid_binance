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

use shared_domain::martingale::{
    MartingaleMarketKind, MartingalePortfolioConfig, MartingaleSizingModel,
    MartingaleStrategyConfig,
};
use std::collections::HashMap;

use crate::martingale::rules::compute_leg_notionals;

/// Minimum notional accepted by the exchange when none is configured.
pub const DEFAULT_EXCHANGE_MIN_NOTIONAL: f64 = 0.0;

/// Effective leverage used for margin math. Futures uses the configured
/// leverage (clamped to `>= 1`); spot is unleveraged (`1.0`).
pub fn effective_leverage(market: MartingaleMarketKind, leverage: Option<u32>) -> f64 {
    match market {
        MartingaleMarketKind::Spot => 1.0,
        MartingaleMarketKind::UsdMFutures => {
            leverage.map(|value| (value as f64).max(1.0)).unwrap_or(1.0)
        }
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
    Ok(notionals
        .iter()
        .map(|notional| notional / leverage)
        .collect())
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
    let full_series_notional: f64 = projections
        .iter()
        .map(|p| p.full_series_notional_quote)
        .sum();
    let budget_capped_margin: f64 = projections
        .iter()
        .map(|p| p.budget_capped_margin_quote)
        .sum();
    let budget_capped_notional: f64 = projections
        .iter()
        .map(|p| p.budget_capped_notional_quote)
        .sum();
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

// ===========================================================================
// Canonical runtime-parity budget-cap applier (Decimal, runtime-parity).
//
// The live `trading-engine` and the backtest replay binary must apply the
// IDENTICAL per-strategy margin-cap logic, otherwise a replay measurement is
// not a valid prediction of live behavior. The functions below are the ONE
// source of truth; `trading-engine::martingale_budget` delegates to them and
// the replay binary (RT2) will too.
//
// MARGIN units throughout (never NOTIONAL as a cap floor).
// ===========================================================================

use rust_decimal::Decimal as RtDecimal;

/// Decimal first-leg margin for a strategy (canonical, runtime-parity).
///
/// Futures: `first_order_quote / leverage` (leverage floored at 1). Spot:
/// `first_order_quote` (unleveraged). Returns `None` when there are no legs or
/// the first notional is non-positive. This is the strategy-level companion of
/// the low-level `first_leg_margin_quote(sizing, market, leverage, min_notional)`
/// f64 helper above — distinct name to avoid clashing with that contract.
pub fn first_leg_margin_for_strategy(strategy: &MartingaleStrategyConfig) -> Option<RtDecimal> {
    let first_notional = match &strategy.sizing {
        MartingaleSizingModel::Multiplier {
            first_order_quote, ..
        }
        | MartingaleSizingModel::BudgetScaled {
            first_order_quote, ..
        } => *first_order_quote,
        MartingaleSizingModel::CustomSequence { notionals } => notionals.first().copied()?,
    };
    if first_notional <= RtDecimal::ZERO {
        return None;
    }
    let leverage = match strategy.market {
        MartingaleMarketKind::Spot => RtDecimal::ONE,
        MartingaleMarketKind::UsdMFutures => RtDecimal::from(strategy.leverage.unwrap_or(1).max(1)),
    };
    Some(first_notional / leverage)
}

/// Narrow an existing limit or set it (Decimal). If `limit` is `Some(positive)`,
/// the result is `min(existing, budget_cap)`; otherwise the limit is replaced.
pub fn cap_strategy_budget_decimal(strategy: &mut MartingaleStrategyConfig, budget_cap: RtDecimal) {
    if budget_cap <= RtDecimal::ZERO {
        return;
    }
    let effective_budget_cap = first_leg_margin_for_strategy(strategy)
        .filter(|value| *value > RtDecimal::ZERO)
        .map(|first_leg_margin| budget_cap.max(first_leg_margin))
        .unwrap_or(budget_cap);
    cap_optional_budget_limit_decimal(
        &mut strategy.risk_limits.max_strategy_budget_quote,
        effective_budget_cap,
    );
}

/// Decimal version of [`cap_optional_budget_limit`].
pub fn cap_optional_budget_limit_decimal(limit: &mut Option<RtDecimal>, budget_cap: RtDecimal) {
    *limit = Some(match *limit {
        Some(current) if current > RtDecimal::ZERO => current.min(budget_cap),
        _ => budget_cap,
    });
}

/// Apply per-strategy MARGIN caps from the global budget times each strategy's
/// weight factor (Decimal, canonical runtime-parity version). Pure. If
/// `max_global_budget_quote` is absent or `<= 0` this is a no-op.
pub fn apply_global_budget_allocations_decimal(
    config: &mut MartingalePortfolioConfig,
    weights: &HashMap<String, RtDecimal>,
) {
    let Some(global_budget) = config
        .risk_limits
        .max_global_budget_quote
        .filter(|value| *value > RtDecimal::ZERO)
    else {
        return;
    };
    let strategy_count = config.strategies.len();
    if strategy_count == 0 {
        return;
    }
    let equal_cap = global_budget / RtDecimal::from(strategy_count as u64);
    for strategy in &mut config.strategies {
        let budget_cap = weights
            .get(&strategy.strategy_id)
            .copied()
            .filter(|weight_factor| *weight_factor > RtDecimal::ZERO)
            .map(|weight_factor| global_budget * weight_factor)
            .unwrap_or(equal_cap);
        cap_strategy_budget_decimal(strategy, budget_cap);
    }
}

/// Parse a `serde_json::Value` into a `Decimal`, mirroring the
/// `decimal_from_json` helper in `trading-engine::main`:
/// - JSON string: parsed via `Decimal::from_str`.
/// - JSON integer: parsed as a Decimal.
/// - JSON float: parsed via `Decimal::try_from(f64)`.
/// Returns `None` on any failure or empty string.
fn decimal_from_json(value: &serde_json::Value) -> Option<RtDecimal> {
    value
        .as_str()
        .or_else(|| value.as_i64().map(|_| ""))
        .and_then(|text| {
            if text.is_empty() {
                None
            } else {
                text.parse::<RtDecimal>().ok()
            }
        })
        .or_else(|| {
            value
                .as_f64()
                .and_then(|number| RtDecimal::try_from(number).ok())
        })
}

/// Per-strategy weight factors extracted from the RAW portfolio config JSON.
///
/// Reads `strategies[].portfolio_weight_pct` (also accepts `weight_pct`),
/// divided by 100. A strategy with NO weight field is OMITTED from the map
/// (falls back to the equal share at apply time); a strategy whose weight is
/// present but `<= 0` is an ERROR. This is the canonical extractor shared by
/// the live runtime and the replay binary.
pub fn extract_portfolio_weight_factors(
    raw_portfolio_config: &serde_json::Value,
) -> Result<HashMap<String, RtDecimal>, String> {
    let mut weights = HashMap::new();
    let Some(strategies) = raw_portfolio_config
        .get("strategies")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(weights);
    };
    for strategy in strategies {
        let Some(strategy_id) = strategy
            .get("strategy_id")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let Some(weight_pct) = strategy
            .get("portfolio_weight_pct")
            .or_else(|| strategy.get("weight_pct"))
            .and_then(decimal_from_json)
        else {
            continue;
        };
        if weight_pct <= RtDecimal::ZERO {
            return Err(format!(
                "portfolio weight for {strategy_id} must be positive"
            ));
        }
        weights.insert(strategy_id.to_owned(), weight_pct / RtDecimal::from(100));
    }
    Ok(weights)
}

/// Apply per-strategy MARGIN caps from `global budget * weight`, floored at
/// each strategy's first-leg margin. The ONE runtime-parity applier used by
/// both the live `trading-engine` and the backtest replay binary. No-op when
/// `max_global_budget_quote` is absent or `<= 0`. Mutates each strategy's
/// `risk_limits.max_strategy_budget_quote`.
pub fn apply_portfolio_weight_margin_caps(
    config: &mut MartingalePortfolioConfig,
    raw_portfolio_config: &serde_json::Value,
) -> Result<(), String> {
    let weights = extract_portfolio_weight_factors(raw_portfolio_config)?;
    apply_global_budget_allocations_decimal(config, &weights);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use shared_domain::martingale::{
        MartingaleDirection, MartingaleDirectionMode, MartingaleEntryTrigger, MartingaleMarginMode,
        MartingaleRiskLimits, MartingaleSpacingModel, MartingaleStrategyConfig,
        MartingaleTakeProfitModel,
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

        let planned_notional =
            planned_notional_quote(&sizing, DEFAULT_EXCHANGE_MIN_NOTIONAL).unwrap();
        assert!(
            (planned_notional - 150.0).abs() < 1e-9,
            "planned notional {planned_notional}"
        );

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
            15.0,   // strategy_margin_cap
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
            15.0,   // available_global_margin (tight)
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
            strat(
                "a",
                sizing.clone(),
                MartingaleMarketKind::UsdMFutures,
                Some(2),
            ),
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
            4.5, // entry_fee_bps
            5.0, // fee_buffer_pct
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
            strat(
                "a",
                sizing.clone(),
                MartingaleMarketKind::UsdMFutures,
                Some(1),
            ),
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
        let strategies = vec![strat(
            "a",
            sizing,
            MartingaleMarketKind::UsdMFutures,
            Some(2),
        )];
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

    // ----- Canonical runtime-parity cap applier (Decimal) tests. -----

    fn portfolio_json(
        strategies: &[serde_json::Value],
        global_budget: Option<&str>,
    ) -> serde_json::Value {
        let mut risk = serde_json::Map::new();
        if let Some(budget) = global_budget {
            risk.insert(
                "max_global_budget_quote".to_string(),
                serde_json::Value::String(budget.to_string()),
            );
        }
        serde_json::json!({
            "direction_mode": "long_and_short",
            "strategies": strategies,
            "risk_limits": serde_json::Value::Object(risk),
        })
    }

    fn strategy_json(
        id: &str,
        market: &str,
        leverage: Option<u32>,
        first_order_quote: &str,
        weight_pct: Option<&str>,
    ) -> serde_json::Value {
        let mut s = serde_json::Map::new();
        s.insert(
            "strategy_id".to_string(),
            serde_json::Value::String(id.to_string()),
        );
        s.insert(
            "symbol".to_string(),
            serde_json::Value::String("BTCUSDT".to_string()),
        );
        s.insert(
            "market".to_string(),
            serde_json::Value::String(market.to_string()),
        );
        s.insert(
            "direction".to_string(),
            serde_json::Value::String("long".to_string()),
        );
        s.insert(
            "direction_mode".to_string(),
            serde_json::Value::String("long_and_short".to_string()),
        );
        if let Some(lev) = leverage {
            s.insert("leverage".to_string(), serde_json::json!(lev));
        }
        s.insert(
            "spacing".to_string(),
            serde_json::json!({"fixed_percent": {"step_bps": 100}}),
        );
        s.insert(
            "sizing".to_string(),
            serde_json::json!({
                "multiplier": {"first_order_quote": first_order_quote, "multiplier": "2", "max_legs": 4}
            }),
        );
        s.insert(
            "take_profit".to_string(),
            serde_json::json!({"percent": {"bps": 100}}),
        );
        if let Some(weight) = weight_pct {
            s.insert(
                "portfolio_weight_pct".to_string(),
                serde_json::Value::String(weight.to_string()),
            );
        }
        s.insert(
            "indicators".to_string(),
            serde_json::Value::Array(Vec::new()),
        );
        s.insert(
            "entry_triggers".to_string(),
            serde_json::Value::Array(Vec::new()),
        );
        s.insert(
            "risk_limits".to_string(),
            serde_json::Value::Object(serde_json::Map::new()),
        );
        serde_json::Value::Object(s)
    }

    #[test]
    fn hard_example_first_order_250_lev5_cap50() {
        // 1 strategy, foq=250, lev=5, weight 100%, global budget 50.
        // first-leg margin 250/5 = 50 floors the cap at 50; notional 250 is NOT the cap.
        let strat_json = strategy_json("s1", "usd_m_futures", Some(5), "250", Some("100"));
        let config_value = portfolio_json(&[strat_json], Some("50"));
        let mut config: MartingalePortfolioConfig =
            serde_json::from_value(config_value.clone()).unwrap();
        apply_portfolio_weight_margin_caps(&mut config, &config_value).unwrap();
        assert_eq!(
            config.strategies[0].risk_limits.max_strategy_budget_quote,
            Some(Decimal::from(50))
        );
    }

    #[test]
    fn weighted_two_strategy() {
        // global 1000; A weight 60% foq 100 lev 10 (margin 10), B weight 40% foq 200 lev 5 (margin 40).
        // A cap max(600,10)=600; B cap max(400,40)=400.
        let a = strategy_json("a", "usd_m_futures", Some(10), "100", Some("60"));
        let b = strategy_json("b", "usd_m_futures", Some(5), "200", Some("40"));
        let config_value = portfolio_json(&[a, b], Some("1000"));
        let mut config: MartingalePortfolioConfig =
            serde_json::from_value(config_value.clone()).unwrap();
        apply_portfolio_weight_margin_caps(&mut config, &config_value).unwrap();
        assert_eq!(
            config.strategies[0].risk_limits.max_strategy_budget_quote,
            Some(Decimal::from(600))
        );
        assert_eq!(
            config.strategies[1].risk_limits.max_strategy_budget_quote,
            Some(Decimal::from(400))
        );
    }

    #[test]
    fn missing_weight_falls_back_to_equal_cap() {
        // 2 strategies, global 1000, neither has a weight field -> each cap max(500, first_leg_margin).
        // foq=100 lev=10 -> first-leg margin=10 <= 500 -> cap=500.
        let a = strategy_json("a", "usd_m_futures", Some(10), "100", None);
        let b = strategy_json("b", "usd_m_futures", Some(10), "100", None);
        let config_value = portfolio_json(&[a, b], Some("1000"));
        let mut config: MartingalePortfolioConfig =
            serde_json::from_value(config_value.clone()).unwrap();
        apply_portfolio_weight_margin_caps(&mut config, &config_value).unwrap();
        assert_eq!(
            config.strategies[0].risk_limits.max_strategy_budget_quote,
            Some(Decimal::from(500))
        );
        assert_eq!(
            config.strategies[1].risk_limits.max_strategy_budget_quote,
            Some(Decimal::from(500))
        );
    }

    #[test]
    fn zero_global_budget_is_noop() {
        // global 0 or None -> no max_strategy_budget_quote written.
        let a = strategy_json("a", "usd_m_futures", Some(5), "100", Some("50"));
        // Case 1: budget = "0".
        let config_value = portfolio_json(&[a.clone()], Some("0"));
        let mut config: MartingalePortfolioConfig =
            serde_json::from_value(config_value.clone()).unwrap();
        apply_portfolio_weight_margin_caps(&mut config, &config_value).unwrap();
        assert_eq!(
            config.strategies[0].risk_limits.max_strategy_budget_quote,
            None
        );

        // Case 2: budget field absent.
        let config_value = portfolio_json(&[a], None);
        let mut config: MartingalePortfolioConfig =
            serde_json::from_value(config_value.clone()).unwrap();
        apply_portfolio_weight_margin_caps(&mut config, &config_value).unwrap();
        assert_eq!(
            config.strategies[0].risk_limits.max_strategy_budget_quote,
            None
        );
    }

    #[test]
    fn negative_weight_is_error() {
        // a strategy with portfolio_weight_pct = 0 (or negative) -> Err.
        let a = strategy_json("a", "usd_m_futures", Some(5), "100", Some("0"));
        let config_value = portfolio_json(&[a], Some("1000"));
        let mut config: MartingalePortfolioConfig =
            serde_json::from_value(config_value.clone()).unwrap();
        let result = apply_portfolio_weight_margin_caps(&mut config, &config_value);
        assert!(result.is_err(), "zero weight must be an error");

        // Negative.
        let a = strategy_json("a", "usd_m_futures", Some(5), "100", Some("-10"));
        let config_value = portfolio_json(&[a], Some("1000"));
        let mut config: MartingalePortfolioConfig =
            serde_json::from_value(config_value.clone()).unwrap();
        let result = apply_portfolio_weight_margin_caps(&mut config, &config_value);
        assert!(result.is_err(), "negative weight must be an error");
    }
}
