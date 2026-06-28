//! Pure logic for the `portfolio_budget_replay` binary.
//!
//! The replay binary's `main` is not unit-testable, so the pure pieces live here:
//!
//! - [`prepare_replay_config`] — the runtime-parity fix: sets the global margin
//!   cap then applies the per-strategy `portfolio_weight_pct` →
//!   `max_strategy_budget_quote` caps via the canonical applier
//!   `capital::apply_portfolio_weight_margin_caps` (the SAME function the live
//!   `trading-engine` uses). A replay that does NOT call this is not a valid
//!   prediction of live behavior, because live enforces per-strategy caps.
//! - [`OnBudgetMetrics`] / [`on_budget_metrics`] — annualized return and max DD
//!   rebased to the budget principal, plus the min-equity / `principal_breached`
//!   (equity ≤ 0) hardening.
//! - [`RejectionBreakdown`] / [`classify_rejections`] — counts of the sim's
//!   budget-rejection reason strings by their EXACT prefixes.
//! - [`RiskProfile`] / [`GateThreshold`] — parameterized gate thresholds.
//! - [`MinimumCapitalView`] / [`minimum_capital_view`] — the Step 4A
//!   minimum-capital feasibility projection.
//!
//! All functions are pure. `main` stays thin: parse args, load data, run the
//! sim, call these, print JSON.

use std::collections::HashMap;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::Value;
use shared_domain::martingale::{
    MartingaleMarketKind, MartingalePortfolioConfig, MartingaleSizingModel,
    MartingaleStrategyConfig,
};

use crate::martingale::capital::{
    apply_portfolio_weight_margin_caps, first_leg_margin_for_strategy,
    DEFAULT_EXCHANGE_MIN_NOTIONAL,
};

// ===========================================================================
// Risk-profile gate.
// ===========================================================================

/// Risk profile selecting the gate threshold table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskProfile {
    Conservative,
    Balanced,
    Aggressive,
}

impl RiskProfile {
    /// Parse a `--profile` argument value.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.to_ascii_lowercase().as_str() {
            "conservative" => Ok(Self::Conservative),
            "balanced" => Ok(Self::Balanced),
            "aggressive" => Ok(Self::Aggressive),
            other => Err(format!(
                "unknown profile {other:?}; expected conservative|balanced|aggressive"
            )),
        }
    }

    /// Auto-detect from a portfolio_id substring (e.g.
    /// `mp_margin_v2_lp_conservative_20260626`), defaulting to conservative.
    pub fn detect_from_portfolio_id(portfolio_id: &str) -> Self {
        let lower = portfolio_id.to_ascii_lowercase();
        if lower.contains("aggressive") {
            Self::Aggressive
        } else if lower.contains("balanced") {
            Self::Balanced
        } else {
            Self::Conservative
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Conservative => "conservative",
            Self::Balanced => "balanced",
            Self::Aggressive => "aggressive",
        }
    }

    /// The gate threshold for this profile.
    pub fn threshold(self) -> GateThreshold {
        match self {
            Self::Conservative => GateThreshold {
                annualized_return_pct: 50.0,
                max_drawdown_pct: 10.0,
            },
            Self::Balanced => GateThreshold {
                annualized_return_pct: 90.0,
                max_drawdown_pct: 20.0,
            },
            Self::Aggressive => GateThreshold {
                annualized_return_pct: 110.0,
                max_drawdown_pct: 30.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GateThreshold {
    pub annualized_return_pct: f64,
    pub max_drawdown_pct: f64,
}

/// Outcome of the gate evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct GateOutcome {
    pub profile: RiskProfile,
    pub annualized_threshold: f64,
    pub drawdown_threshold: f64,
    pub passed: bool,
}

/// Evaluate the gate. `passed = ann > ann_thr && max_dd <= dd_thr &&
/// !principal_breached && max_capital_used <= budget + epsilon`.
pub fn evaluate_gate(
    profile: RiskProfile,
    annualized_return_pct: f64,
    max_drawdown_pct: f64,
    principal_breached: bool,
    max_capital_used_quote: f64,
    budget_quote: f64,
) -> GateOutcome {
    let threshold = profile.threshold();
    let within_budget = max_capital_used_quote <= budget_quote + CAPITAL_EPS;
    let passed = annualized_return_pct > threshold.annualized_return_pct
        && max_drawdown_pct <= threshold.max_drawdown_pct
        && !principal_breached
        && within_budget;
    GateOutcome {
        profile,
        annualized_threshold: threshold.annualized_return_pct,
        drawdown_threshold: threshold.max_drawdown_pct,
        passed,
    }
}

/// Epsilon for capital comparisons (compensates f64 drift).
const CAPITAL_EPS: f64 = 1e-6;

// ===========================================================================
// Config prep — the runtime-parity fix.
// ===========================================================================

/// Result of preparing the portfolio config for a runtime-parity replay.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedReplayConfig {
    /// Whether the per-strategy weight caps were actually applied. Always true
    /// for a positive budget; false (no-op) when the budget is ≤ 0.
    pub runtime_weight_caps_applied: bool,
}

/// Prepare a parsed portfolio config for a budget-capped replay:
/// 1. set `risk_limits.max_global_budget_quote = Some(budget)`;
/// 2. apply per-strategy `max_strategy_budget_quote` caps from the weight
///    factors in `raw_portfolio_config_value` via the canonical applier.
///
/// `raw_portfolio_config_value` is the JSON value of the portfolio config
/// (the `portfolio_config` sub-object if the file is a search-result wrapper,
/// else the file root) — the same shape the live runtime reads.
pub fn prepare_replay_config(
    config: &mut MartingalePortfolioConfig,
    raw_portfolio_config_value: &Value,
    budget: Decimal,
) -> Result<PreparedReplayConfig, String> {
    config.risk_limits.max_global_budget_quote = Some(budget);
    let runtime_weight_caps_applied = budget > Decimal::ZERO;
    if runtime_weight_caps_applied {
        apply_portfolio_weight_margin_caps(config, raw_portfolio_config_value)?;
    }
    Ok(PreparedReplayConfig {
        runtime_weight_caps_applied,
    })
}

// ===========================================================================
// On-budget metrics (annualized / max DD rebased to budget + min-equity hardening).
// ===========================================================================

/// On-budget equity-curve metrics rebased to the budget principal.
#[derive(Debug, Clone, PartialEq)]
pub struct OnBudgetMetrics {
    pub total_return_pct: f64,
    pub annualized_return_pct: f64,
    pub max_drawdown_pct: f64,
    /// Minimum `equity_on_budget` over the curve.
    pub min_equity_quote: f64,
    /// True iff `equity_on_budget <= 0.0` at any point (principal wiped).
    pub principal_breached: bool,
}

/// Compute on-budget metrics from a synthetic/real cumulative-PnL series.
///
/// `budget_quote` is the principal the curve is rebased onto.
/// `cum_pnl_series` is the cumulative PnL at each timestamp (cum_pnl(t0) need
/// not be zero — the rebase is `equity_on_budget(t) = budget + cum_pnl(t)`).
/// `days` is the elapsed span of the series.
///
/// Annualized return uses `((1+total_return_on_budget)^(365/days)-1)*100`;
/// max DD is peak-to-trough on `equity_on_budget`; `principal_breached` is set
/// when any `equity_on_budget <= 0.0`.
pub fn on_budget_metrics(budget_quote: f64, cum_pnl_series: &[f64], days: f64) -> OnBudgetMetrics {
    let budget = budget_quote.max(1e-9);
    let mut peak = f64::NEG_INFINITY;
    let mut max_dd_pct = 0.0_f64;
    let mut min_equity = f64::INFINITY;
    let mut principal_breached = false;

    for &cum_pnl in cum_pnl_series {
        let equity = budget + cum_pnl;
        if equity > peak {
            peak = equity;
        }
        if equity < min_equity {
            min_equity = equity;
        }
        if equity <= 0.0 {
            principal_breached = true;
        }
        if peak > 0.0 {
            let dd = (peak - equity) / peak * 100.0;
            if dd > max_dd_pct {
                max_dd_pct = dd;
            }
        }
    }

    // If the series is empty, fall back to the budget principal itself.
    if cum_pnl_series.is_empty() {
        min_equity = budget;
    }

    let first = cum_pnl_series.first().copied().unwrap_or(0.0);
    let last = cum_pnl_series.last().copied().unwrap_or(0.0);
    let total_return_fraction = (last - first) / budget;
    let annualized = if days > 0.0 && (1.0 + total_return_fraction) > 0.0 {
        ((1.0 + total_return_fraction).powf(365.0 / days) - 1.0) * 100.0
    } else {
        f64::NEG_INFINITY
    };

    OnBudgetMetrics {
        total_return_pct: total_return_fraction * 100.0,
        annualized_return_pct: annualized,
        max_drawdown_pct: max_dd_pct,
        min_equity_quote: min_equity,
        principal_breached,
    }
}

// ===========================================================================
// Rejection classification.
// ===========================================================================

/// Counts of budget-rejection reasons by their EXACT sim prefixes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RejectionBreakdown {
    pub global: usize,
    pub symbol: usize,
    pub direction: usize,
    pub strategy: usize,
    pub total: usize,
}

/// Classify a list of sim rejection-reason strings by their prefixes.
///
/// Prefixes (from `kline_engine::budget_rejection_reason`):
/// - `"global budget exceeded for strategy ..."`
/// - `"symbol budget exceeded for strategy ..."`
/// - `"direction budget exceeded for strategy ..."`
/// - `"strategy budget exceeded for strategy ..."`
///
/// Strings that match none of these are counted only in `total`.
pub fn classify_rejections(reasons: &[String]) -> RejectionBreakdown {
    let mut bd = RejectionBreakdown {
        total: reasons.len(),
        ..Default::default()
    };
    for reason in reasons {
        if reason.starts_with("global budget exceeded") {
            bd.global += 1;
        } else if reason.starts_with("symbol budget exceeded") {
            bd.symbol += 1;
        } else if reason.starts_with("direction budget exceeded") {
            bd.direction += 1;
        } else if reason.starts_with("strategy budget exceeded") {
            bd.strategy += 1;
        }
    }
    bd
}

// ===========================================================================
// Minimum-capital feasibility view (Step 4A).
// ===========================================================================

/// Minimum-capital feasibility projection over a portfolio config + weights.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimumCapitalView {
    /// Σ over strategies of full-ladder planned margin with the ORIGINAL
    /// `first_order_quote` (unweighted, unscaled).
    pub natural_unscaled_planned_margin_quote: f64,
    /// Σ over strategies of `full_ladder_margin × weight_factor`.
    pub lp_weighted_planned_margin_quote: f64,
    /// max over strategies of
    /// `exchange_min_notional × full_ladder_margin / (weight_factor × first_order_quote)`;
    /// the smallest principal at which every strategy's first leg clears the
    /// exchange minimum under exact (uniform) scaling.
    pub min_exact_scaled_executable_principal_quote: f64,
    pub min_exact_scaled_bottleneck_symbol: Option<String>,
    pub min_exact_scaled_bottleneck_strategy_id: Option<String>,
    pub min_exact_scaled_bottleneck_first_order_quote: Option<f64>,
    /// Smallest scaled first order if principal were 1000:
    /// `first_order_quote × 1000 / min_exact_scaled_executable_principal_quote`
    /// (at the bottleneck strategy).
    pub scale_to_1000_min_first_order_quote: Option<f64>,
    /// The scaling model the current replay gate uses.
    pub scale_model_used_for_gate: &'static str,
}

/// Compute the minimum-capital feasibility view.
///
/// `weights` maps strategy_id → weight factor (0..1). Strategies missing from
/// the map fall back to an equal share (`1/strategies.len()`), matching the
/// applier's fallback. `exchange_min_notional` is the exchange minimum order
/// notional.
pub fn minimum_capital_view(
    strategies: &[MartingaleStrategyConfig],
    weights: &HashMap<String, Decimal>,
    exchange_min_notional: f64,
) -> Result<MinimumCapitalView, String> {
    if strategies.is_empty() {
        return Ok(MinimumCapitalView {
            natural_unscaled_planned_margin_quote: 0.0,
            lp_weighted_planned_margin_quote: 0.0,
            min_exact_scaled_executable_principal_quote: 0.0,
            min_exact_scaled_bottleneck_symbol: None,
            min_exact_scaled_bottleneck_strategy_id: None,
            min_exact_scaled_bottleneck_first_order_quote: None,
            scale_to_1000_min_first_order_quote: None,
            scale_model_used_for_gate: "cap_truncated",
        });
    }

    let equal = Decimal::ONE / Decimal::from(strategies.len() as u64);

    let mut natural_unscaled = 0.0_f64;
    let mut lp_weighted = 0.0_f64;
    let mut min_exact_principal = 0.0_f64;
    let mut bottleneck_symbol: Option<String> = None;
    let mut bottleneck_strategy_id: Option<String> = None;
    let mut bottleneck_first_order: Option<f64> = None;

    for strategy in strategies {
        let wf = weights
            .get(&strategy.strategy_id)
            .copied()
            .filter(|w| *w > Decimal::ZERO)
            .unwrap_or(equal);

        // Full-ladder planned margin with the ORIGINAL first_order_quote.
        let full_ladder_margin = full_ladder_margin_for_strategy(strategy)?;
        natural_unscaled += full_ladder_margin;
        lp_weighted += full_ladder_margin * wf.to_f64().unwrap_or(0.0);

        // The first_order_quote (leveraged notional) of this strategy.
        let first_order = first_order_notional_for_strategy(strategy);
        if let Some(first_order_f) = first_order {
            let wf_f = wf.to_f64().unwrap_or(0.0);
            // exchange_min_notional × full_ladder_margin / (weight_factor × first_order_quote)
            if wf_f > 0.0 && first_order_f > 0.0 {
                let principal = exchange_min_notional * full_ladder_margin / (wf_f * first_order_f);
                if principal > min_exact_principal {
                    min_exact_principal = principal;
                    bottleneck_symbol = Some(strategy.symbol.clone());
                    bottleneck_strategy_id = Some(strategy.strategy_id.clone());
                    bottleneck_first_order = Some(first_order_f);
                }
            }
        }
    }

    // scale_to_1000_min_first_order_quote at the bottleneck strategy.
    let scale_to_1000 = match (bottleneck_first_order, min_exact_principal > 0.0) {
        (Some(first_order_f), true) => Some(first_order_f * 1000.0 / min_exact_principal),
        _ => None,
    };

    Ok(MinimumCapitalView {
        natural_unscaled_planned_margin_quote: natural_unscaled,
        lp_weighted_planned_margin_quote: lp_weighted,
        min_exact_scaled_executable_principal_quote: min_exact_principal,
        min_exact_scaled_bottleneck_symbol: bottleneck_symbol,
        min_exact_scaled_bottleneck_strategy_id: bottleneck_strategy_id,
        min_exact_scaled_bottleneck_first_order_quote: bottleneck_first_order,
        scale_to_1000_min_first_order_quote: scale_to_1000,
        scale_model_used_for_gate: "cap_truncated",
    })
}

/// Full-ladder planned margin (sum of leg margins) for one strategy with its
/// ORIGINAL sizing (no cap), f64.
fn full_ladder_margin_for_strategy(strategy: &MartingaleStrategyConfig) -> Result<f64, String> {
    let leverage = match strategy.market {
        MartingaleMarketKind::Spot => 1.0,
        MartingaleMarketKind::UsdMFutures => strategy.leverage.unwrap_or(1).max(1) as f64,
    };
    let notionals = leg_notional_decimals(strategy)?;
    let margin: f64 = notionals
        .iter()
        .map(|n| n.to_f64().unwrap_or(0.0) / leverage)
        .sum();
    Ok(margin)
}

/// First leg's leveraged notional (Decimal) for a strategy, or None if no legs.
fn first_order_notional_for_strategy(strategy: &MartingaleStrategyConfig) -> Option<f64> {
    let first = match &strategy.sizing {
        MartingaleSizingModel::Multiplier {
            first_order_quote, ..
        }
        | MartingaleSizingModel::BudgetScaled {
            first_order_quote, ..
        } => *first_order_quote,
        MartingaleSizingModel::CustomSequence { notionals } => notionals.first().copied()?,
    };
    Some(first.to_f64().unwrap_or(0.0))
}

/// Leg notional series in Decimal (reuses the canonical computation but
/// returns Decimal via the f64 helper — kept local to avoid widening the
/// capital.rs API for the replay's diagnostic-only needs).
fn leg_notional_decimals(strategy: &MartingaleStrategyConfig) -> Result<Vec<Decimal>, String> {
    let f = crate::martingale::capital::leg_notional_series(
        &strategy.sizing,
        DEFAULT_EXCHANGE_MIN_NOTIONAL,
    )?;
    Ok(f.into_iter()
        .map(|v| Decimal::try_from(v).unwrap_or_else(|_| Decimal::ZERO))
        .collect())
}

// ===========================================================================
// Diagnostics assembly helpers (pure; consume the canonical projection).
// ===========================================================================

/// Per-strategy diagnostic row for the JSON `per_strategy[]` block.
#[derive(Debug, Clone)]
pub struct PerStrategyDiagnostic<'a> {
    pub strategy: &'a MartingaleStrategyConfig,
    /// weight factor (0..1) used for the cap, or the equal-share fallback.
    pub weight_factor: f64,
    /// `max_strategy_budget_quote` after apply (effective cap), if set.
    pub effective_cap_quote: Option<f64>,
    pub first_leg_margin_quote: f64,
    /// Best-effort static projection of accepted legs (NOT a gate).
    pub accepted_static_legs: u32,
}

/// Build the per-strategy diagnostics from the post-apply config and the
/// canonical portfolio projection. Pure.
pub fn build_per_strategy_diagnostics<'a>(
    config: &'a MartingalePortfolioConfig,
    weights: &HashMap<String, Decimal>,
    projection: &crate::martingale::capital::PortfolioCapitalProjection,
) -> Vec<PerStrategyDiagnostic<'a>> {
    let equal = if config.strategies.is_empty() {
        0.0
    } else {
        1.0 / config.strategies.len() as f64
    };
    let mut out = Vec::with_capacity(config.strategies.len());
    for strategy in &config.strategies {
        let wf = weights
            .get(&strategy.strategy_id)
            .copied()
            .filter(|w| *w > Decimal::ZERO)
            .map(|w| w.to_f64().unwrap_or(0.0))
            .unwrap_or(equal);
        let effective_cap = strategy
            .risk_limits
            .max_strategy_budget_quote
            .and_then(|d| d.to_f64());
        let first_leg_margin = first_leg_margin_for_strategy(strategy)
            .map(|d| d.to_f64().unwrap_or(0.0))
            .unwrap_or(0.0);
        // accepted_static_legs from the projection (matched by strategy_id).
        let accepted_static_legs = projection
            .strategies
            .iter()
            .find(|p| p.strategy_id == strategy.strategy_id)
            .map(|p| p.legs.iter().take_while(|l| l.accepted).count() as u32)
            .unwrap_or(0);
        out.push(PerStrategyDiagnostic {
            strategy,
            weight_factor: wf,
            effective_cap_quote: effective_cap,
            first_leg_margin_quote: first_leg_margin,
            accepted_static_legs,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::martingale::capital::project_portfolio_capital;
    use rust_decimal::Decimal;
    use shared_domain::martingale::{
        MartingaleDirection, MartingaleDirectionMode, MartingaleEntryTrigger, MartingaleMarginMode,
        MartingaleRiskLimits, MartingaleSpacingModel, MartingaleStrategyConfig,
        MartingaleTakeProfitModel,
    };
    use std::collections::HashMap;

    // ----- helpers (mirror capital.rs test fixtures) -----

    fn multiplier_sizing(first: i64, mult: i64, legs: u32) -> MartingaleSizingModel {
        MartingaleSizingModel::Multiplier {
            first_order_quote: Decimal::from(first),
            multiplier: Decimal::from(mult),
            max_legs: legs,
        }
    }

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
            serde_json::Value::String(id.to_string().to_uppercase()),
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

    // ----- prepare_replay_config parity tests -----

    #[test]
    fn prepare_replay_config_single_strategy_cap_floored_at_first_leg_margin() {
        // 1 strategy, foq=250, lev=5, weight 100%, global budget 50.
        // first-leg margin 250/5 = 50 floors the cap at 50 (NOT notional 250).
        let strat_json = strategy_json("s1", "usd_m_futures", Some(5), "250", Some("100"));
        let config_value = portfolio_json(&[strat_json], None);
        let mut config: MartingalePortfolioConfig =
            serde_json::from_value(config_value.clone()).unwrap();

        let prep = prepare_replay_config(&mut config, &config_value, Decimal::from(50)).unwrap();
        assert!(prep.runtime_weight_caps_applied);
        assert_eq!(
            config.risk_limits.max_global_budget_quote,
            Some(Decimal::from(50))
        );
        assert_eq!(
            config.strategies[0].risk_limits.max_strategy_budget_quote,
            Some(Decimal::from(50))
        );
    }

    #[test]
    fn prepare_replay_config_two_strategy_weighted() {
        // 2 strategies, global 1000, A 60% (foq 100 lev 10 -> margin 10),
        // B 40% (foq 200 lev 5 -> margin 40). Caps max(600,10)=600 / max(400,40)=400.
        let a = strategy_json("a", "usd_m_futures", Some(10), "100", Some("60"));
        let b = strategy_json("b", "usd_m_futures", Some(5), "200", Some("40"));
        let config_value = portfolio_json(&[a, b], None);
        let mut config: MartingalePortfolioConfig =
            serde_json::from_value(config_value.clone()).unwrap();

        let prep = prepare_replay_config(&mut config, &config_value, Decimal::from(1000)).unwrap();
        assert!(prep.runtime_weight_caps_applied);
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
    fn prepare_replay_config_zero_budget_is_noop_flag() {
        let strat_json = strategy_json("s1", "usd_m_futures", Some(5), "100", Some("100"));
        let config_value = portfolio_json(&[strat_json], None);
        let mut config: MartingalePortfolioConfig =
            serde_json::from_value(config_value.clone()).unwrap();
        let prep = prepare_replay_config(&mut config, &config_value, Decimal::ZERO).unwrap();
        assert!(!prep.runtime_weight_caps_applied);
        assert_eq!(
            config.strategies[0].risk_limits.max_strategy_budget_quote,
            None
        );
    }

    // ----- on_budget_metrics tests -----

    #[test]
    fn on_budget_metrics_basic_upside() {
        // budget 100, cum_pnl goes 0 -> 10 over 365 days => +10% total, +10% ann.
        let m = on_budget_metrics(100.0, &[0.0, 10.0], 365.0);
        assert!(
            (m.total_return_pct - 10.0).abs() < 1e-9,
            "{}",
            m.total_return_pct
        );
        assert!(
            (m.annualized_return_pct - 10.0).abs() < 1e-6,
            "{}",
            m.annualized_return_pct
        );
        assert!((m.max_drawdown_pct - 0.0).abs() < 1e-9);
        assert!((m.min_equity_quote - 100.0).abs() < 1e-9);
        assert!(!m.principal_breached);
    }

    #[test]
    fn on_budget_metrics_drawdown_and_breach() {
        // budget 100, cum_pnl: 0, -60 (equity 40), -120 (equity -20 <= 0 -> breach), -10.
        let m = on_budget_metrics(100.0, &[0.0, -60.0, -120.0, -10.0], 365.0);
        assert!(m.principal_breached, "equity dipped to -20 <= 0");
        assert!(m.min_equity_quote <= 0.0);
        // Peak 100, trough -20 => DD = (100-(-20))/100*100 = 120%.
        assert!(
            (m.max_drawdown_pct - 120.0).abs() < 1e-6,
            "{}",
            m.max_drawdown_pct
        );
    }

    #[test]
    fn on_budget_metrics_compounding_annualization() {
        // budget 100, +20 over 182.5 days (half year) => total 20%, ann ~ ((1.2)^2-1)*100 = 44%.
        let m = on_budget_metrics(100.0, &[0.0, 20.0], 182.5);
        let expected = ((1.2_f64).powf(2.0) - 1.0) * 100.0;
        assert!((m.annualized_return_pct - expected).abs() < 1e-6);
    }

    // ----- classify_rejections tests -----

    #[test]
    fn classify_rejections_all_four_prefixes() {
        let reasons = vec![
            "global budget exceeded for strategy s1; current_capital_quote=10".to_string(),
            "symbol budget exceeded for strategy s2; symbol=BTC".to_string(),
            "direction budget exceeded for strategy s3; direction=Long".to_string(),
            "strategy budget exceeded for strategy s4; budget_quote=50".to_string(),
            "global budget exceeded for strategy s1".to_string(),
            "unrelated reason".to_string(),
        ];
        let bd = classify_rejections(&reasons);
        assert_eq!(bd.global, 2);
        assert_eq!(bd.symbol, 1);
        assert_eq!(bd.direction, 1);
        assert_eq!(bd.strategy, 1);
        assert_eq!(bd.total, 6);
    }

    #[test]
    fn classify_rejections_empty() {
        let bd = classify_rejections(&[]);
        assert_eq!(bd, RejectionBreakdown::default());
        assert_eq!(bd.total, 0);
    }

    // ----- risk-profile gate tests -----

    #[test]
    fn risk_profile_threshold_table() {
        assert_eq!(
            RiskProfile::Conservative.threshold(),
            GateThreshold {
                annualized_return_pct: 50.0,
                max_drawdown_pct: 10.0
            }
        );
        assert_eq!(
            RiskProfile::Balanced.threshold(),
            GateThreshold {
                annualized_return_pct: 90.0,
                max_drawdown_pct: 20.0
            }
        );
        assert_eq!(
            RiskProfile::Aggressive.threshold(),
            GateThreshold {
                annualized_return_pct: 110.0,
                max_drawdown_pct: 30.0
            }
        );
    }

    #[test]
    fn risk_profile_detect_from_portfolio_id() {
        assert_eq!(
            RiskProfile::detect_from_portfolio_id("mp_margin_v2_lp_conservative_20260626"),
            RiskProfile::Conservative
        );
        assert_eq!(
            RiskProfile::detect_from_portfolio_id("mp_margin_v2_lp_balanced_x"),
            RiskProfile::Balanced
        );
        assert_eq!(
            RiskProfile::detect_from_portfolio_id("mp_margin_v2_lp_aggressive_x"),
            RiskProfile::Aggressive
        );
        assert_eq!(
            RiskProfile::detect_from_portfolio_id("mp_margin_v2_lp_unknown"),
            RiskProfile::Conservative
        );
    }

    #[test]
    fn evaluate_gate_passes_and_fails() {
        // Conservative: ann>50, dd<=10, no breach, within budget.
        let g = evaluate_gate(RiskProfile::Conservative, 60.0, 8.0, false, 950.0, 1000.0);
        assert!(g.passed);
        // Fails on DD.
        let g = evaluate_gate(RiskProfile::Conservative, 60.0, 12.0, false, 950.0, 1000.0);
        assert!(!g.passed);
        // Fails on principal breach.
        let g = evaluate_gate(RiskProfile::Conservative, 60.0, 8.0, true, 950.0, 1000.0);
        assert!(!g.passed);
        // Fails when max_capital_used exceeds budget.
        let g = evaluate_gate(RiskProfile::Conservative, 60.0, 8.0, false, 1001.0, 1000.0);
        assert!(!g.passed);
        // Balanced threshold.
        let g = evaluate_gate(RiskProfile::Balanced, 95.0, 18.0, false, 1000.0, 1000.0);
        assert!(g.passed);
    }

    // ----- minimum_capital_view tests (hand-computed) -----

    #[test]
    fn minimum_capital_view_two_strategy_hand_computed() {
        // Strategy a: foq=10, mult=2, 4 legs, futures lev=2.
        //   notionals [10,20,40,80], margins [5,10,20,40], full_ladder_margin=75.
        // Strategy b: foq=20, mult=2, 4 legs, futures lev=2.
        //   notionals [20,40,80,160], margins [10,20,40,80], full_ladder_margin=150.
        // weights a=0.6, b=0.4. exchange_min_notional = 5.
        let a = strat(
            "a",
            multiplier_sizing(10, 2, 4),
            MartingaleMarketKind::UsdMFutures,
            Some(2),
        );
        let b = strat(
            "b",
            multiplier_sizing(20, 2, 4),
            MartingaleMarketKind::UsdMFutures,
            Some(2),
        );
        let strategies = vec![a, b];
        let mut weights = HashMap::new();
        weights.insert("a".to_string(), Decimal::new(6, 1)); // 0.6
        weights.insert("b".to_string(), Decimal::new(4, 1)); // 0.4

        let view = minimum_capital_view(&strategies, &weights, 5.0).unwrap();

        // natural_unscaled = 75 + 150 = 225.
        assert!(
            (view.natural_unscaled_planned_margin_quote - 225.0).abs() < 1e-9,
            "{}",
            view.natural_unscaled_planned_margin_quote
        );
        // lp_weighted = 75*0.6 + 150*0.4 = 45 + 60 = 105.
        assert!(
            (view.lp_weighted_planned_margin_quote - 105.0).abs() < 1e-9,
            "{}",
            view.lp_weighted_planned_margin_quote
        );

        // min_exact_principal:
        //   a: 5 * 75 / (0.6 * 10) = 375 / 6 = 62.5
        //   b: 5 * 150 / (0.4 * 20) = 750 / 8 = 93.75
        //   max = 93.75, bottleneck = b.
        assert!(
            (view.min_exact_scaled_executable_principal_quote - 93.75).abs() < 1e-6,
            "{}",
            view.min_exact_scaled_executable_principal_quote
        );
        assert_eq!(
            view.min_exact_scaled_bottleneck_strategy_id.as_deref(),
            Some("b")
        );
        // scale_to_1000 at b: first_order 20 * 1000 / 93.75 = 213.333...
        let expected_scale = 20.0 * 1000.0 / 93.75;
        let actual = view.scale_to_1000_min_first_order_quote.unwrap();
        assert!((actual - expected_scale).abs() < 1e-6, "{}", actual);
        assert_eq!(view.scale_model_used_for_gate, "cap_truncated");
    }

    #[test]
    fn minimum_capital_view_empty() {
        let view = minimum_capital_view(&[], &HashMap::new(), 5.0).unwrap();
        assert_eq!(view.natural_unscaled_planned_margin_quote, 0.0);
        assert_eq!(view.min_exact_scaled_executable_principal_quote, 0.0);
        assert_eq!(view.scale_to_1000_min_first_order_quote, None);
    }

    #[test]
    fn minimum_capital_view_equal_weight_fallback() {
        // Single strategy, NO weight entry -> fallback equal = 1.0.
        // foq=10, lev=2, 4 legs => full_ladder_margin 75, first_order 10.
        // min_exact = 5 * 75 / (1.0 * 10) = 37.5.
        let a = strat(
            "a",
            multiplier_sizing(10, 2, 4),
            MartingaleMarketKind::UsdMFutures,
            Some(2),
        );
        let view = minimum_capital_view(&[a], &HashMap::new(), 5.0).unwrap();
        assert!(
            (view.min_exact_scaled_executable_principal_quote - 37.5).abs() < 1e-6,
            "{}",
            view.min_exact_scaled_executable_principal_quote
        );
        // lp_weighted = 75 * 1.0 = 75.
        assert!((view.lp_weighted_planned_margin_quote - 75.0).abs() < 1e-9);
    }

    // ----- build_per_strategy_diagnostics smoke test -----

    #[test]
    fn per_strategy_diagnostics_after_apply() {
        // 2 strategies, global 1000, A 60% B 40% as above; caps 600/400.
        let a = strat(
            "a",
            multiplier_sizing(100, 2, 4),
            MartingaleMarketKind::UsdMFutures,
            Some(10),
        );
        let b = strat(
            "b",
            multiplier_sizing(200, 2, 4),
            MartingaleMarketKind::UsdMFutures,
            Some(5),
        );
        let mut config = MartingalePortfolioConfig {
            direction_mode: MartingaleDirectionMode::LongOnly,
            strategies: vec![a, b],
            risk_limits: MartingaleRiskLimits::default(),
        };
        let mut weights_dec = HashMap::new();
        weights_dec.insert("a".to_string(), Decimal::new(6, 1));
        weights_dec.insert("b".to_string(), Decimal::new(4, 1));
        // Apply caps manually via the canonical applier path.
        let raw = serde_json::json!({
            "strategies": [
                {"strategy_id": "a", "portfolio_weight_pct": "60"},
                {"strategy_id": "b", "portfolio_weight_pct": "40"},
            ]
        });
        config.risk_limits.max_global_budget_quote = Some(Decimal::from(1000));
        apply_portfolio_weight_margin_caps(&mut config, &raw).unwrap();

        let weights_f64: HashMap<String, f64> = weights_dec
            .iter()
            .map(|(k, v)| (k.clone(), v.to_f64().unwrap_or(0.0)))
            .collect();
        let proj = project_portfolio_capital(
            &config.strategies,
            &weights_f64,
            1000.0,
            DEFAULT_EXCHANGE_MIN_NOTIONAL,
            0.0,
            0.0,
        )
        .unwrap();

        let diags = build_per_strategy_diagnostics(&config, &weights_dec, &proj);
        assert_eq!(diags.len(), 2);
        // a: cap 600, weight 0.6.
        assert!((diags[0].weight_factor - 0.6).abs() < 1e-9);
        assert!((diags[0].effective_cap_quote.unwrap() - 600.0).abs() < 1e-6);
        // b: cap 400.
        assert!((diags[1].effective_cap_quote.unwrap() - 400.0).abs() < 1e-6);
        // accepted_static_legs for a: notionals [100,200,400,800], margins (lev10) [10,20,40,80],
        //   cap 600 -> all 4 accepted (cum 150 <= 600).
        assert_eq!(diags[0].accepted_static_legs, 4);
    }
}
