//! Per-strategy budget cap helpers.
//!
//! Thin delegating wrappers around the canonical runtime-parity implementation
//! in `backtest_engine::martingale::capital`. The public API is preserved so
//! `main.rs` and the existing tests keep working; the actual logic now lives in
//! exactly one place shared with the backtest replay binary (RT2).
//!
//! `max_strategy_budget_quote` is a MARGIN principal cap (the runtime's
//! `enforce_budget_for_next_leg` compares `strategy_margin_exposure +
//! next_margin` against it — both margin units). The cap is kept in margin
//! units, floored at the first leg's MARGIN (not the leveraged NOTIONAL) so a
//! strategy can always place leg 0 without inflating the cap by up to leverage x.

use std::collections::HashMap;

use rust_decimal::Decimal;
use shared_domain::martingale::{MartingalePortfolioConfig, MartingaleStrategyConfig};

use backtest_engine::martingale::capital as canonical;

/// First leg's MARGIN capital for a strategy.
///
/// Futures: `first_order_quote / leverage`. Spot: `first_order_quote`
/// (unleveraged). Returns `None` when the sizing has no legs or the first
/// notional is non-positive. Delegates to the canonical
/// [`canonical::first_leg_margin_for_strategy`].
pub fn first_leg_margin_quote(strategy: &MartingaleStrategyConfig) -> Option<Decimal> {
    canonical::first_leg_margin_for_strategy(strategy)
}

/// Set the per-strategy MARGIN cap. Delegates to the canonical
/// [`canonical::cap_strategy_budget_decimal`].
pub fn cap_strategy_budget(strategy: &mut MartingaleStrategyConfig, budget_cap: Decimal) {
    canonical::cap_strategy_budget_decimal(strategy, budget_cap);
}

/// Narrow an existing limit or set it. Delegates to the canonical
/// [`canonical::cap_optional_budget_limit_decimal`].
pub fn cap_optional_budget_limit(limit: &mut Option<Decimal>, budget_cap: Decimal) {
    canonical::cap_optional_budget_limit_decimal(limit, budget_cap);
}

/// Apply per-strategy MARGIN caps from the global budget times each strategy's
/// weight factor. Delegates to the canonical
/// [`canonical::apply_global_budget_allocations_decimal`].
pub fn apply_global_budget_allocations(
    config: &mut MartingalePortfolioConfig,
    weights: &HashMap<String, Decimal>,
) {
    canonical::apply_global_budget_allocations_decimal(config, weights);
}
