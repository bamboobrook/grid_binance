//! Per-strategy budget cap helpers.
//!
//! `max_strategy_budget_quote` is a MARGIN principal cap (the runtime's
//! `enforce_budget_for_next_leg` compares `strategy_margin_exposure +
//! next_margin` against it — both margin units). These helpers keep the cap in
//! margin units, flooring at the first leg's MARGIN (not the leveraged
//! NOTIONAL) so a strategy can always place leg 0 without inflating the cap by
//! up to leverage x.
use std::collections::HashMap;

use rust_decimal::Decimal;
use shared_domain::martingale::{
    MartingaleMarketKind, MartingalePortfolioConfig, MartingaleStrategyConfig, MartingaleSizingModel,
};

/// First leg's MARGIN capital.
///
/// Futures: `first_order_quote / leverage`. Spot: `first_order_quote`
/// (unleveraged). Returns `None` when the sizing has no legs or the first
/// notional is non-positive.
pub fn first_leg_margin_quote(strategy: &MartingaleStrategyConfig) -> Option<Decimal> {
    let first_notional = match &strategy.sizing {
        MartingaleSizingModel::Multiplier { first_order_quote, .. }
        | MartingaleSizingModel::BudgetScaled { first_order_quote, .. } => *first_order_quote,
        MartingaleSizingModel::CustomSequence { notionals } => notionals.first().copied()?,
    };
    if first_notional <= Decimal::ZERO {
        return None;
    }
    let leverage = match strategy.market {
        MartingaleMarketKind::Spot => Decimal::ONE,
        MartingaleMarketKind::UsdMFutures => strategy
            .leverage
            .map(|l| Decimal::from(l.max(1)))
            .unwrap_or(Decimal::ONE),
    };
    Some(first_notional / leverage)
}

/// Set the per-strategy MARGIN cap.
///
/// `budget_cap` is in MARGIN units. The effective cap is floored at the first
/// leg's MARGIN (so a strategy can always place leg 0) but never at NOTIONAL,
/// then narrowed into `max_strategy_budget_quote` via
/// [`cap_optional_budget_limit`].
pub fn cap_strategy_budget(strategy: &mut MartingaleStrategyConfig, budget_cap: Decimal) {
    if budget_cap <= Decimal::ZERO {
        return;
    }
    let effective_budget_cap = first_leg_margin_quote(strategy)
        .filter(|value| *value > Decimal::ZERO)
        .map(|first_leg_margin| budget_cap.max(first_leg_margin))
        .unwrap_or(budget_cap);
    cap_optional_budget_limit(
        &mut strategy.risk_limits.max_strategy_budget_quote,
        effective_budget_cap,
    );
}

/// Narrow an existing limit or set it.
///
/// If `limit` is `Some(positive)`, the result is `min(existing, budget_cap)`;
/// otherwise the limit is replaced by `budget_cap`.
pub fn cap_optional_budget_limit(limit: &mut Option<Decimal>, budget_cap: Decimal) {
    *limit = Some(match *limit {
        Some(current) if current > Decimal::ZERO => current.min(budget_cap),
        _ => budget_cap,
    });
}

/// Apply per-strategy MARGIN caps from the global budget times each strategy's
/// weight factor. Pure; relocated verbatim from `main.rs`.
pub fn apply_global_budget_allocations(
    config: &mut MartingalePortfolioConfig,
    weights: &HashMap<String, Decimal>,
) {
    let Some(global_budget) = config
        .risk_limits
        .max_global_budget_quote
        .filter(|value| *value > Decimal::ZERO)
    else {
        return;
    };
    let strategy_count = config.strategies.len();
    if strategy_count == 0 {
        return;
    }
    let equal_cap = global_budget / Decimal::from(strategy_count as u64);
    for strategy in &mut config.strategies {
        let budget_cap = weights
            .get(&strategy.strategy_id)
            .copied()
            .filter(|weight_factor| *weight_factor > Decimal::ZERO)
            .map(|weight_factor| global_budget * weight_factor)
            .unwrap_or(equal_cap);
        cap_strategy_budget(strategy, budget_cap);
    }
}
