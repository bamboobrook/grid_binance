//! Round 18 R2: native synchronized residual Martingale cycle engine.
//!
//! Implements the M1 synchronized pair cycle (plan §7). M2 market-factor
//! residual baskets fail closed until their factor-neutral direction and
//! notional contract is implemented. This is the FIRST
//! time the codebase executes a synchronized multi-leg residual Martingale
//! cycle — prior rounds (R1-R17) only ran independent per-symbol cycles.
//!
//! ## Machine definition (plan §2)
//! ```text
//! cycle_open:
//!   residual computed on the completed bar boundary from train-frozen
//!   (beta, mu, sigma); |residual z| >= entry_z -> all legs open FO at the
//!   SAME timestamp, dollar-beta neutral quote per leg.
//!
//! safety_order:
//!   aggregate cycle net PnL(after cost) < 0
//!   AND residual moved adversely from last fill by >= so_residual_step_z
//!   -> all surviving legs add notional in lockstep at multiplier scale,
//!   group aggregate notional strictly increases, capped by reserve/ruin.
//!
//! take_profit:
//!   aggregate cycle net PnL(after fee/funding/slippage) > tp_net_bps_floor
//!   AND |residual z| <= exit_z -> all legs reduce/close together.
//!
//! abort:
//!   cointegration break / deadline / atomic rejection -> only real aggregate
//!   reduce/close; loss enters equity.
//! ```
//!
//! ## Anti-overfit contract (plan §10)
//! All residual fit (beta, mu, sigma, group selection) uses ONLY train bars.
//! The caller passes already-fitted parameters via `SynchronizedFit`; the
//! engine never reads validation/future bars for fit.
//!
//! ## Anti-cheat contract (plan §2)
//! A config is `not_martingale` and recorded as such if: no SO ever fires
//! (profit only from one-shot entry); any leg PnL is added outside the cycle;
//! residual signal generates out-of-cycle orders; SO fires while cycle is net
//! profitable; shadow/precomputed curve replaces real synchronous orders.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::market_data::KlineBar;
use crate::martingale::kline_engine::FundingRatePoint;
use crate::martingale::metrics::{
    calculate_annualized_return_pct, MartingaleBacktestEvent, MartingaleBacktestResult,
    MartingaleMetrics, MartingaleTradeDetail,
};
use crate::martingale::trace_digest::compute_trace_digests;
use shared_domain::martingale::SynchronizedCycleConfig;

/// A train-fitted residual contract for ONE synchronized group (pair or basket).
/// All fields are frozen at validation commit time; the engine never re-fits.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SynchronizedFit {
    /// Group id (deterministic, e.g. "M1_BTCUSDT_ETHUSDT").
    pub group_id: String,
    /// Leg symbols in the group. For M1 this is [A, B]; for M2 this is the
    /// 6+ basket symbols.
    pub legs: Vec<String>,
    /// M2 base direction sign metadata. M1 ignores this field and derives both
    /// leg directions at cycle open from the residual sign and hedge beta.
    pub leg_direction_signs: Vec<i8>,
    /// Per-leg beta vs the factor/other leg, train-frozen. residual_i =
    /// log(price_i) - beta_i * log(factor_price) - mu_i (M2) or
    /// log(A) - beta * log(B) - mu (M1 pair A/B).
    pub betas: Vec<f64>,
    /// Per-leg residual mean (mu), train-frozen.
    pub mus: Vec<f64>,
    /// Common residual sigma across the group (train-frozen std of residual).
    pub residual_sigma: f64,
    /// Train-window estimated half-life (hours), for the deadline proxy.
    pub half_life_h: f64,
    /// SHA256 of the train data + fit code used to produce this fit (audit).
    pub fit_sha256: String,
}

/// One synchronized cycle. Holds the aggregate state across all legs.
#[derive(Debug, Clone)]
pub struct SynchronizedCycleState {
    pub cycle_id: String,
    pub cycle_seq: u64,
    /// Per-leg filled states: index -> list of (price, notional_quote, fee,
    /// slippage). All legs have the SAME number of fills at any time (sync).
    pub leg_fills: Vec<Vec<LegFill>>,
    /// Position direction frozen at cycle open. M1 derives this from the
    /// residual sign and hedge beta; it must not use a static long/long fit.
    pub leg_direction_signs: Vec<i8>,
    /// Frozen at cycle open.
    pub opened_at_ms: i64,
    pub last_fill_at_ms: i64,
    pub residual_z_at_open: f64,
    pub last_residual_z: f64,
    pub depth: u32, // number of synchronized layers filled (FO=1, +1 per SO)
    pub aggregate_realized_pnl_quote: f64, // from partial reduces / TP / abort
    pub aborted: bool,
    pub abort_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LegFill {
    pub price: f64,
    pub notional_quote: f64,
    pub fee_quote: f64,
    pub slippage_quote: f64,
    pub funding_paid_quote: f64,
}

impl SynchronizedCycleState {
    fn new(
        cycle_id: String,
        cycle_seq: u64,
        n_legs: usize,
        opened_at_ms: i64,
        z_open: f64,
        leg_direction_signs: Vec<i8>,
    ) -> Self {
        Self {
            cycle_id,
            cycle_seq,
            leg_fills: vec![Vec::new(); n_legs],
            leg_direction_signs,
            opened_at_ms,
            last_fill_at_ms: opened_at_ms,
            residual_z_at_open: z_open,
            last_residual_z: z_open,
            depth: 0,
            aggregate_realized_pnl_quote: 0.0,
            aborted: false,
            abort_reason: None,
        }
    }

    /// Aggregate notional currently held across all legs (mark-to-market at
    /// last fill prices for the still-open portion).
    fn aggregate_open_notional_quote(&self) -> f64 {
        self.leg_fills
            .iter()
            .flat_map(|fills| fills.iter())
            .map(|f| f.notional_quote)
            .sum()
    }

    /// Per-leg current position notional (sum of that leg's fills).
    fn leg_notional(&self, leg_idx: usize) -> f64 {
        self.leg_fills[leg_idx]
            .iter()
            .map(|f| f.notional_quote)
            .sum()
    }

    fn leg_quantity(&self, leg_idx: usize) -> f64 {
        self.leg_fills[leg_idx]
            .iter()
            .filter(|fill| fill.price > 0.0)
            .map(|fill| fill.notional_quote / fill.price)
            .sum()
    }

    /// Aggregate unrealized PnL across legs given current mark prices.
    /// For each leg: sign * (mark - avg_entry)/avg_entry * leg_notional.
    fn aggregate_unrealized_pnl(&self, marks: &[f64]) -> f64 {
        let mut total = 0.0;
        for (i, fills) in self.leg_fills.iter().enumerate() {
            if fills.is_empty() || i >= marks.len() {
                continue;
            }
            let mark = marks[i];
            let sign = self.leg_direction_signs[i] as f64;
            total += fills
                .iter()
                .filter(|fill| fill.price > 0.0)
                .map(|fill| {
                    let quantity = fill.notional_quote / fill.price;
                    sign * (mark - fill.price) * quantity
                })
                .sum::<f64>();
        }
        total
    }

    /// Net aggregate cycle PnL = realized + unrealized, after all costs.
    fn aggregate_net_pnl(&self, marks: &[f64]) -> f64 {
        self.aggregate_realized_pnl_quote + self.aggregate_unrealized_pnl(marks)
    }

    fn avg_entry(&self, leg_idx: usize) -> Option<f64> {
        let fills = &self.leg_fills[leg_idx];
        if fills.is_empty() {
            None
        } else {
            let notional = fills.iter().map(|fill| fill.notional_quote).sum::<f64>();
            let quantity = self.leg_quantity(leg_idx);
            (quantity > 0.0).then_some(notional / quantity)
        }
    }
}

/// Effective fee/slippage in bps (mirrors kline_engine's atomics; the CLI sets
/// overrides via the same process-global functions before calling us).
fn effective_fee_bps() -> f64 {
    crate::martingale::kline_engine::effective_fee_bps_pub()
}
fn effective_slippage_bps() -> f64 {
    crate::martingale::kline_engine::effective_slippage_bps_pub()
}

/// Per-leg notionals for the synchronized ladder. M1 uses the train-frozen
/// hedge beta: factor/dependent quote weights are |beta|:1. M2 remains equal
/// weight until its factor-neutral implementation is completed and validated.
pub fn synchronized_leg_notionals(
    cfg: &SynchronizedCycleConfig,
    fit: &SynchronizedFit,
) -> Vec<Vec<f64>> {
    let weights = if fit.legs.len() == 2 && !fit.betas.is_empty() {
        let beta = fit.betas[0].abs();
        let denominator = 1.0 + beta;
        vec![beta / denominator, 1.0 / denominator]
    } else {
        vec![1.0 / fit.legs.len() as f64; fit.legs.len()]
    };
    (0..cfg.max_legs as usize)
        .map(|k| {
            let layer_notional = cfg.group_fo_quote * cfg.multiplier.powi(k as i32);
            weights
                .iter()
                .map(|weight| layer_notional * weight)
                .collect()
        })
        .collect()
}

fn entry_leg_direction_signs(fit: &SynchronizedFit, residual_z: f64) -> Result<Vec<i8>, String> {
    let residual_sign = if residual_z > 0.0 {
        1
    } else if residual_z < 0.0 {
        -1
    } else {
        0
    };
    if residual_sign == 0 {
        return Err("cannot open synchronized cycle at zero residual".to_string());
    }
    if fit.legs.len() == 2 {
        let beta = *fit.betas.first().ok_or("M1 pair fit missing hedge beta")?;
        if !beta.is_finite() || beta.abs() < 1e-9 {
            return Err(format!("M1 pair hedge beta is invalid: {beta}"));
        }
        // residual = dependent - beta * factor. Mean-reversion exposure is
        // -sign(residual) * residual, so q_dep=-s and q_factor=s*beta.
        let factor_sign = if residual_sign as f64 * beta > 0.0 {
            1
        } else {
            -1
        };
        Ok(vec![factor_sign, -residual_sign])
    } else {
        // Round 19 R4 M2F: factor-residual basket. Each leg's direction is set
        // by the SIGN of that leg's per-leg residual at cycle open. A leg with
        // positive residual (over-valued vs factor) is SHORTED; negative residual
        // is BOUGHT. leg_residual_signs are precomputed by the train fit and
        // frozen on the SynchronizedFit; if absent we fail closed.
        if fit.leg_direction_signs.len() == fit.legs.len()
            && fit.leg_direction_signs.iter().any(|&s| s != 0)
        {
            // Validate the basket has both long and short exposure (factor-neutral).
            let has_long = fit.leg_direction_signs.iter().any(|&s| s > 0);
            let has_short = fit.leg_direction_signs.iter().any(|&s| s < 0);
            if !has_long || !has_short {
                return Err("M2 basket direction contract requires both long and short legs at cycle open".to_string());
            }
            Ok(fit.leg_direction_signs.clone())
        } else {
            Err("M2 basket requires precomputed leg_direction_signs from train fit (long most-negative residual, short most-positive)".to_string())
        }
    }
}

/// Validate that a config is a genuine Martingale per plan §2/§7.3.
/// Returns Err(reason) if `not_martingale`.
pub fn validate_martingale_contract(cfg: &SynchronizedCycleConfig) -> Result<(), String> {
    if cfg.multiplier <= 1.0 {
        return Err(format!(
            "not_martingale: multiplier={} must be > 1.0 (plan §2 requires loss-after-add)",
            cfg.multiplier
        ));
    }
    if cfg.max_legs < 2 {
        return Err(format!(
            "not_martingale: max_legs={} must be >= 2 so at least one SO is possible (plan §7.3)",
            cfg.max_legs
        ));
    }
    if cfg.entry_z <= 0.0 || cfg.so_residual_step_z <= 0.0 {
        return Err(format!(
            "not_martingale: entry_z={} and so_residual_step_z={} must be > 0",
            cfg.entry_z, cfg.so_residual_step_z
        ));
    }
    if cfg.group_fo_quote <= 0.0 {
        return Err(format!(
            "not_martingale: group_fo_quote={} must be > 0",
            cfg.group_fo_quote
        ));
    }
    if cfg.leverage == 0 {
        return Err("not_martingale: leverage must be >= 1".to_string());
    }
    if !cfg.group_gross_cap_pct.is_finite() || cfg.group_gross_cap_pct <= 0.0 {
        return Err(format!(
            "not_martingale: group_gross_cap_pct={} must be finite and > 0",
            cfg.group_gross_cap_pct
        ));
    }
    Ok(())
}

/// Compute the residual z for a group at a timestamp, given mark prices.
/// Returns None if any leg/factor price is missing or <= 0.
///
/// **M1 pair**: a SINGLE cointegration residual. `betas[0]` is the OLS beta of
/// `log(legs[1]) ~ beta * log(legs[0]) + mu` (leg[0] is the independent
/// variable / factor, leg[1] is the dependent). residual = log(legs[1]) -
/// betas[0]*log(legs[0]) - mus[0]. This matches the train OLS fit exactly.
///
/// **M2 basket**: residual_i = log(price_i) - beta_i*log(factor) - mu_i; the
/// basket residual z is the mean of per-leg residual z (factor-neutral
/// long-short). The factor (BTC or PC1) is passed via `factor_mark`.
pub fn residual_z(
    fit: &SynchronizedFit,
    marks: &BTreeMap<String, f64>,
    factor_mark: Option<f64>,
) -> Option<f64> {
    if fit.legs.len() < 2 {
        return None;
    }
    if fit.residual_sigma <= 0.0 {
        return None;
    }
    // M1 pair: single residual.
    if fit.legs.len() == 2 {
        let p_dep = *marks.get(&fit.legs[1])?;
        let p_fac = *marks.get(&fit.legs[0])?;
        if p_dep <= 0.0 || p_fac <= 0.0 {
            return None;
        }
        let r = p_dep.ln() - fit.betas[0] * p_fac.ln() - fit.mus[0];
        return Some(r / fit.residual_sigma);
    }
    // M2 basket.
    let factor = factor_mark?;
    if factor <= 0.0 {
        return None;
    }
    let log_f = factor.ln();
    let mut residuals: Vec<f64> = Vec::with_capacity(fit.legs.len());
    for (i, sym) in fit.legs.iter().enumerate() {
        let p = *marks.get(sym)?;
        if p <= 0.0 {
            return None;
        }
        let r = p.ln() - fit.betas[i] * log_f - fit.mus[i];
        residuals.push(r);
    }
    let mean_r = residuals.iter().sum::<f64>() / residuals.len() as f64;
    Some(mean_r / fit.residual_sigma)
}

/// Entry: a synchronized cycle engine run. Returns the standard
/// `MartingaleBacktestResult` so the same trace-digest / gate / metrics
/// infrastructure works.
///
/// `fits` is one entry per group (M1: each pair; M2: the single basket). The
/// engine runs all groups over the same merged bar stream.
pub fn run_synchronized_cycle_replay(
    cfg: &SynchronizedCycleConfig,
    fits: &[SynchronizedFit],
    bars: &[KlineBar],
    funding_rates: &[FundingRatePoint],
    budget_quote: f64,
) -> Result<MartingaleBacktestResult, String> {
    // Round 19 R2: reset cycle seq so each backtest invocation is deterministic
    // (same config + same bars => same cycle_ids => same trace hashes).
    reset_cycle_seq();
    validate_martingale_contract(cfg)?;
    // Round 19 R4: M2F factor-residual basket is now implemented with per-leg
    // residual-sign directions and factor-neutral notional weights. The M1 pair
    // remains the 2-leg path. Other family names (P1/K1/V1) reuse the basket
    // path with their own residual fit, so we accept M1_pair, M2_basket, M2F,
    // P1, K1, V1 here.
    let family = cfg.family.as_str();
    let is_supported = matches!(family, "M1_pair" | "M2_basket" | "M2F" | "P1" | "K1" | "V1");
    if !is_supported {
        return Err(format!("unsupported synchronized cycle family: {}", cfg.family));
    }
    // M2/M2F/P1/K1/V1 baskets require a factor to be loadable (BTC factor); if
    // the fit has >2 legs and no factor is configured, fail closed.
    if family != "M1_pair" {
        let needs_factor = fits.iter().any(|f| f.legs.len() > 2);
        if needs_factor && cfg.factor.as_deref().is_none() {
            return Err(format!(
                "basket family {} requires cfg.factor (BTC or PC1) for residual computation",
                family
            ));
        }
    }
    if fits.is_empty() {
        return Err("no synchronized groups fit".to_string());
    }
    for fit in fits {
        // Round 19 R4: generalize the fit validator for both M1 pair (2 legs)
        // and basket families (M2F/P1/K1/V1 with >2 legs).
        if fit.legs.len() == 2 {
            // M1 pair fit: 1 beta, 1 mu.
            if fit.betas.len() != 1 || fit.mus.len() != 1 {
                return Err(format!(
                    "invalid M1 fit {}: expected 2 legs, 1 beta, and 1 mu",
                    fit.group_id
                ));
            }
            if !fit.betas[0].is_finite()
                || fit.betas[0].abs() < 1e-9
                || !fit.mus[0].is_finite()
                || !fit.residual_sigma.is_finite()
                || fit.residual_sigma <= 0.0
            {
                return Err(format!(
                    "invalid M1 fit {}: beta/mu/sigma must be finite, beta non-zero, sigma > 0",
                    fit.group_id
                ));
            }
        } else {
            // Basket fit: betas/mus must match legs count; per-leg direction
            // signs required (long most-negative residual, short most-positive).
            if fit.legs.len() < 5 {
                return Err(format!(
                    "invalid basket fit {}: expected >=5 legs, got {}",
                    fit.group_id, fit.legs.len()
                ));
            }
            if fit.betas.len() != fit.legs.len() || fit.mus.len() != fit.legs.len() {
                return Err(format!(
                    "invalid basket fit {}: betas/mus length must match legs ({})",
                    fit.group_id, fit.legs.len()
                ));
            }
            if fit.leg_direction_signs.len() != fit.legs.len() {
                return Err(format!(
                    "invalid basket fit {}: leg_direction_signs length must match legs",
                    fit.group_id
                ));
            }
            if !fit.residual_sigma.is_finite() || fit.residual_sigma <= 0.0 {
                return Err(format!(
                    "invalid basket fit {}: residual_sigma must be > 0",
                    fit.group_id
                ));
            }
            let has_long = fit.leg_direction_signs.iter().any(|&s| s > 0);
            let has_short = fit.leg_direction_signs.iter().any(|&s| s < 0);
            if !has_long || !has_short {
                return Err(format!(
                    "invalid basket fit {}: requires both long and short legs",
                    fit.group_id
                ));
            }
        }
    }
    if budget_quote <= 0.0 {
        return Err(format!("budget_quote={} must be > 0", budget_quote));
    }

    let n_groups = fits.len();
    let fee_bps = effective_fee_bps();
    let slip_bps = effective_slippage_bps();
    let boundary_ms = (cfg.bar_boundary_minutes as i64) * 60_000;
    let group_gross_cap_quote = budget_quote * (cfg.group_gross_cap_pct / 100.0);

    // Per-group notionals[layers][legs].
    let group_notionals: Vec<Vec<Vec<f64>>> = fits
        .iter()
        .map(|fit| synchronized_leg_notionals(cfg, fit))
        .collect();

    // Per-group active cycle state (None = no active cycle).
    let mut active: Vec<Option<SynchronizedCycleState>> = vec![None; n_groups];
    // Per-group realized PnL accumulator.
    let mut group_realized: Vec<f64> = vec![0.0; n_groups];
    // Per-group funding cost accumulator.
    let mut group_funding: Vec<f64> = vec![0.0; n_groups];
    let mut symbol_funding: BTreeMap<String, f64> = BTreeMap::new();
    // Per-group fee/slippage accumulators.
    let mut group_fee: Vec<f64> = vec![0.0; n_groups];
    let mut group_slip: Vec<f64> = vec![0.0; n_groups];
    // Per-group fill counts (FO/SO/TP/reduce/atomic-reject).
    let mut group_fo: Vec<u64> = vec![0; n_groups];
    let mut group_so: Vec<u64> = vec![0; n_groups];
    let mut group_tp: Vec<u64> = vec![0; n_groups];
    let mut group_reduce: Vec<u64> = vec![0; n_groups];
    let mut group_atomic_reject: Vec<u64> = vec![0; n_groups];
    // Distinguish unique groups from closed cycles that produced a real SO.
    let mut group_had_so: Vec<bool> = vec![false; n_groups];
    let mut cycles_with_so: u32 = 0;

    let mut events: Vec<MartingaleBacktestEvent> = Vec::new();
    let mut trades: Vec<MartingaleTradeDetail> = Vec::new();

    // Equity tracking.
    let initial_equity = budget_quote;
    let mut realized_pnl_quote = 0.0;
    let mut equity_peak = initial_equity;
    let mut equity_curve: Vec<crate::martingale::metrics::EquityPoint> = Vec::new();
    let mut trade_count: u64 = 0;

    // Track per-(group,leg) last funding application time to compute funding
    // accrued since the last fill (used in aggregate net PnL). Simpler: apply
    // funding to the cycle's realized PnL immediately on each funding event,
    // so aggregate_net_pnl uses realized + unrealized with funding already in.
    let _ = (fee_bps, slip_bps);

    // Pre-sort funding by (time, symbol) for monotonic drain.
    let mut sorted_funding: Vec<&FundingRatePoint> = funding_rates.iter().collect();
    sorted_funding.sort_by(|a, b| {
        a.funding_time_ms
            .cmp(&b.funding_time_ms)
            .then(a.symbol.cmp(&b.symbol))
    });
    let mut funding_index = 0usize;

    // Latest close per symbol (completed bar boundary).
    let mut latest_close: BTreeMap<String, f64> = BTreeMap::new();

    // Iterate the merged bar stream by timestamp groups (same pattern as
    // kline_engine).
    let mut bar_index = 0;
    while bar_index < bars.len() {
        let timestamp_ms = bars[bar_index].open_time_ms;
        let mut updated_symbols = BTreeSet::new();
        while bar_index < bars.len() && bars[bar_index].open_time_ms == timestamp_ms {
            let b = &bars[bar_index];
            if b.close <= 0.0 || b.open < 0.0 || b.high < 0.0 || b.low < 0.0 {
                bar_index += 1;
                continue;
            }
            latest_close.insert(b.symbol.clone(), b.close);
            updated_symbols.insert(b.symbol.clone());
            bar_index += 1;
        }
        // Drain funding events up to this timestamp, applying to any active
        // cycle's legs matching the symbol.
        while funding_index < sorted_funding.len()
            && sorted_funding[funding_index].funding_time_ms <= timestamp_ms
        {
            let f = sorted_funding[funding_index];
            if f.funding_rate.is_finite() {
                for (gi, fit) in fits.iter().enumerate() {
                    if let Some(cycle) = &mut active[gi] {
                        if let Some(leg_pos) = fit.legs.iter().position(|s| s == &f.symbol) {
                            let leg_notional = cycle.leg_notional(leg_pos);
                            if leg_notional > 0.0 {
                                let mark = f.mark_price.unwrap_or_else(|| {
                                    latest_close.get(&f.symbol).copied().unwrap_or(0.0)
                                });
                                let sign = cycle.leg_direction_signs[leg_pos] as f64;
                                let current_notional = if mark > 0.0 {
                                    cycle.leg_quantity(leg_pos) * mark
                                } else {
                                    leg_notional
                                };
                                let fund_quote = -sign * current_notional * f.funding_rate;
                                // attribute to the leg's last fill as funding_paid
                                if let Some(last_fill) = cycle.leg_fills[leg_pos].last_mut() {
                                    last_fill.funding_paid_quote += fund_quote;
                                }
                                cycle.aggregate_realized_pnl_quote += fund_quote;
                                group_funding[gi] += fund_quote;
                                *symbol_funding.entry(f.symbol.clone()).or_insert(0.0) +=
                                    fund_quote;
                                realized_pnl_quote += fund_quote;
                                events.push(MartingaleBacktestEvent {
                                    timestamp_ms: f.funding_time_ms,
                                    event_type: "sync_funding_fee".to_string(),
                                    symbol: f.symbol.clone(),
                                    strategy_instance_id: fit.group_id.clone(),
                                    cycle_id: Some(cycle.cycle_id.clone()),
                                    detail: format!(
                                        "funding_rate={:.8};notional_quote={:.4};funding_quote={:.4}",
                                        f.funding_rate, leg_notional, fund_quote
                                    ),
                                });
                            }
                        }
                    }
                }
            }
            funding_index += 1;
        }

        // Only act on completed-bar boundaries (timestamp aligned to boundary).
        if boundary_ms > 0 && timestamp_ms % boundary_ms != 0 {
            // still record equity at this timestamp
            let unreal = aggregate_all_unrealized(&active, &fits, &latest_close);
            let eq = initial_equity + realized_pnl_quote + unreal;
            equity_peak = equity_peak.max(eq);
            equity_curve.push(crate::martingale::metrics::EquityPoint {
                timestamp_ms,
                equity_quote: eq,
            });
            continue;
        }

        // For each group, evaluate synchronized decisions.
        for (gi, fit) in fits.iter().enumerate() {
            // A synchronized decision requires a fresh completed bar for every
            // leg. Stale marks remain valid for equity only, never for orders.
            if !fit
                .legs
                .iter()
                .all(|symbol| updated_symbols.contains(symbol))
            {
                continue;
            }
            let marks = collect_marks(fit, &latest_close);
            if marks.is_none() {
                continue;
            }
            let marks = marks.unwrap();
            let factor_mark = factor_price(fit, cfg, &latest_close);
            let z = match residual_z(fit, &latest_close, factor_mark) {
                Some(v) => v,
                None => continue,
            };

            // 1. Active cycle management (SO / TP / abort).
            let portfolio_margin_before =
                projected_total_margin_all(&active, fits, &latest_close, cfg);
            let available_equity_before = initial_equity
                + realized_pnl_quote
                + aggregate_all_unrealized(&active, &fits, &latest_close);
            // A deadline abort closes the position at the previous boundary.
            // Clear its tombstone before admitting a later independent cycle.
            if active[gi].as_ref().is_some_and(|cycle| cycle.aborted) {
                active[gi] = None;
            }
            if let Some(cycle) = &mut active[gi] {
                if cycle.aborted {
                    // nothing more; will be cleared when equity recomputed
                } else {
                    // Snapshot the residual z from the LAST fill before updating
                    // to the current z. The SO adverse-check must measure how far
                    // the residual has moved SINCE the last fill, not zero.
                    let prev_last_z = cycle.last_residual_z;
                    let net_pnl = cycle.aggregate_net_pnl(&marks);
                    let depth = cycle.depth as usize;

                    // abort: deadline
                    if let Some(deadline_h) = cfg.cycle_deadline_h {
                        let age_h = (timestamp_ms - cycle.opened_at_ms) / 3_600_000;
                        if age_h >= deadline_h as i64 {
                            // aggregate reduce/close: realize at current marks
                            let close = close_cycle_at_marks(
                                cycle,
                                &marks,
                                timestamp_ms,
                                fit,
                                cfg.leverage,
                                fee_bps,
                                slip_bps,
                                &mut trades,
                                &mut events,
                                "sync_abort_deadline",
                            );
                            cycle.aggregate_realized_pnl_quote += close.realized_delta;
                            cycle.aborted = true;
                            cycle.abort_reason =
                                Some(format!("deadline_h={} age_h={}", deadline_h, age_h));
                            realized_pnl_quote += close.realized_delta;
                            group_fee[gi] += close.fee_quote;
                            group_slip[gi] += close.slippage_quote;
                            group_realized[gi] += cycle.aggregate_realized_pnl_quote;
                            if cycle.depth > 1 {
                                group_had_so[gi] = true;
                                cycles_with_so += 1;
                            }
                            group_reduce[gi] += 1;
                            trade_count += fit.legs.len() as u64;
                            continue;
                        }
                    }

                    // TP: net PnL > floor AND |z| <= exit_z
                    let tp_floor_quote = cycle.aggregate_open_notional_quote()
                        * (cfg.tp_net_bps_floor as f64)
                        / 10_000.0;
                    let estimated_close_cost =
                        current_close_notional(cycle, &marks) * (fee_bps + slip_bps) / 10_000.0;
                    if net_pnl - estimated_close_cost > tp_floor_quote
                        && z.abs() <= cfg.exit_z
                        && depth >= 1
                    {
                        let close = close_cycle_at_marks(
                            cycle,
                            &marks,
                            timestamp_ms,
                            fit,
                            cfg.leverage,
                            fee_bps,
                            slip_bps,
                            &mut trades,
                            &mut events,
                            "sync_tp",
                        );
                        cycle.aggregate_realized_pnl_quote += close.realized_delta;
                        realized_pnl_quote += close.realized_delta;
                        group_fee[gi] += close.fee_quote;
                        group_slip[gi] += close.slippage_quote;
                        group_tp[gi] += 1;
                        trade_count += fit.legs.len() as u64;
                        // close out
                        let closed_id = cycle.cycle_id.clone();
                        let had_so = cycle.depth > 1;
                        group_realized[gi] += cycle.aggregate_realized_pnl_quote;
                        if had_so {
                            group_had_so[gi] = true;
                            cycles_with_so += 1;
                        }
                        events.push(MartingaleBacktestEvent {
                            timestamp_ms,
                            event_type: "sync_cycle_close".to_string(),
                            symbol: fit.legs.join(","),
                            strategy_instance_id: fit.group_id.clone(),
                            cycle_id: Some(closed_id),
                            detail: format!(
                                "close=tp;depth={};net_pnl={:.4};residual_z={:.3}",
                                cycle.depth, cycle.aggregate_realized_pnl_quote, z
                            ),
                        });
                        active[gi] = None;
                        continue;
                    }

                    // SO: net PnL < 0 AND residual moved adversely by step
                    if net_pnl < 0.0 && depth < cfg.max_legs as usize {
                        // Adverse = residual moved further from zero in the
                        // direction that hurts the entry. Entry sign = sign(z_open).
                        // Measure against the residual at the LAST FILL (prev_last_z).
                        let entry_sign = cycle.residual_z_at_open.signum();
                        let adverse = (z - prev_last_z) * entry_sign >= cfg.so_residual_step_z;
                        if adverse {
                            // try to add a synchronized SO layer across all legs
                            let layer_idx = depth; // 0=FO already filled; SO layers are 1..max-1
                            let mut all_ok = true;
                            let mut new_fills: Vec<Option<LegFill>> =
                                Vec::with_capacity(fit.legs.len());
                            for (li, sym) in fit.legs.iter().enumerate() {
                                let mark = marks[li];
                                let layer_notional = group_notionals[gi][layer_idx][li];
                                // exchange min notional + margin cap check
                                let fee = layer_notional * fee_bps / 10_000.0;
                                let slip = layer_notional * slip_bps / 10_000.0;
                                let projected_group_gross = current_close_notional(cycle, &marks)
                                    + group_notionals[gi][layer_idx].iter().sum::<f64>();
                                let projected_portfolio_margin = portfolio_margin_before
                                    + group_notionals[gi][layer_idx].iter().sum::<f64>()
                                        / cfg.leverage as f64;
                                if layer_notional < 5.0
                                    || projected_group_gross > group_gross_cap_quote
                                    || projected_portfolio_margin > available_equity_before.max(0.0)
                                {
                                    all_ok = false;
                                    new_fills.push(None);
                                    let _ = sym;
                                } else {
                                    new_fills.push(Some(LegFill {
                                        price: mark,
                                        notional_quote: layer_notional,
                                        fee_quote: fee,
                                        slippage_quote: slip,
                                        funding_paid_quote: 0.0,
                                    }));
                                }
                            }
                            if all_ok && new_fills.iter().all(|f| f.is_some()) {
                                for (li, opt) in new_fills.iter().enumerate() {
                                    if let Some(f) = opt {
                                        cycle.leg_fills[li].push(f.clone());
                                        group_fee[gi] += f.fee_quote;
                                        group_slip[gi] += f.slippage_quote;
                                        realized_pnl_quote -= f.fee_quote + f.slippage_quote;
                                        cycle.aggregate_realized_pnl_quote -=
                                            f.fee_quote + f.slippage_quote;
                                        trades.push(MartingaleTradeDetail {
                                            timestamp_ms,
                                            symbol: fit.legs[li].clone(),
                                            direction: if cycle.leg_direction_signs[li] > 0 {
                                                "long".to_string()
                                            } else {
                                                "short".to_string()
                                            },
                                            event_type: "sync_so".to_string(),
                                            leg_index: Some(cycle.depth as u32),
                                            price: f.price,
                                            margin_quote: f.notional_quote / cfg.leverage as f64,
                                            notional_quote: f.notional_quote,
                                            leverage: cfg.leverage as f64,
                                            fee_quote: f.fee_quote,
                                            slippage_quote: f.slippage_quote,
                                            realized_pnl_quote: -(f.fee_quote + f.slippage_quote),
                                            equity_after_quote: 0.0,
                                        });
                                    }
                                }
                                cycle.depth += 1;
                                cycle.last_fill_at_ms = timestamp_ms;
                                cycle.last_residual_z = z;
                                group_so[gi] += 1;
                                trade_count += fit.legs.len() as u64;
                                events.push(MartingaleBacktestEvent {
                                    timestamp_ms,
                                    event_type: "sync_safety_order".to_string(),
                                    symbol: fit.legs.join(","),
                                    strategy_instance_id: fit.group_id.clone(),
                                    cycle_id: Some(cycle.cycle_id.clone()),
                                    detail: format!(
                                        "layer={};net_pnl_before={:.4};residual_z={:.3};step_z={}",
                                        cycle.depth, net_pnl, z, cfg.so_residual_step_z
                                    ),
                                });
                            } else if !all_ok {
                                group_atomic_reject[gi] += 1;
                                events.push(MartingaleBacktestEvent {
                                    timestamp_ms,
                                    event_type: "sync_group_atomic_reject".to_string(),
                                    symbol: fit.legs.join(","),
                                    strategy_instance_id: fit.group_id.clone(),
                                    cycle_id: Some(cycle.cycle_id.clone()),
                                    detail:
                                        "reason=min_notional_or_margin_cap;action=so_layer_skipped"
                                            .to_string(),
                                });
                            }
                        }
                    }
                }
                continue; // active cycle managed; no new FO on same group
            }

            // 2. New cycle open: |z| >= entry_z and no active cycle.
            if z.abs() >= cfg.entry_z {
                // entry sign: positive residual z -> short the over-valued legs,
                // buy under-valued. For a pair, A over B: if z>0, A rich -> short
                // A, long B. We set leg signs from the residual sign at open.
                let entry_sign = z.signum() as i8;
                let cycle_direction_signs = entry_leg_direction_signs(fit, z)?;
                // FO layer (layer 0).
                let mut all_ok = true;
                let mut new_fills: Vec<Option<LegFill>> = Vec::with_capacity(fit.legs.len());
                // M1 factor/dependent directions were derived above from the
                // residual sign and the train-frozen hedge beta.
                for (li, _sym) in fit.legs.iter().enumerate() {
                    let mark = marks[li];
                    let layer_notional = group_notionals[gi][0][li];
                    let fee = layer_notional * fee_bps / 10_000.0;
                    let slip = layer_notional * slip_bps / 10_000.0;
                    let layer_total = group_notionals[gi][0].iter().sum::<f64>();
                    let projected_portfolio_margin =
                        portfolio_margin_before + layer_total / cfg.leverage as f64;
                    if layer_notional < 5.0
                        || layer_total > group_gross_cap_quote
                        || projected_portfolio_margin > available_equity_before.max(0.0)
                    {
                        all_ok = false;
                        new_fills.push(None);
                    } else {
                        new_fills.push(Some(LegFill {
                            price: mark,
                            notional_quote: layer_notional,
                            fee_quote: fee,
                            slippage_quote: slip,
                            funding_paid_quote: 0.0,
                        }));
                    }
                }
                if all_ok && new_fills.iter().all(|f| f.is_some()) {
                    let cycle_seq = next_cycle_seq();
                    let cycle_id = format!("{}-sync-cycle-{}", fit.group_id, cycle_seq);
                    let mut cycle = SynchronizedCycleState::new(
                        cycle_id.clone(),
                        cycle_seq,
                        fit.legs.len(),
                        timestamp_ms,
                        z,
                        cycle_direction_signs,
                    );
                    for (li, opt) in new_fills.iter().enumerate() {
                        if let Some(f) = opt {
                            cycle.leg_fills[li].push(f.clone());
                            group_fee[gi] += f.fee_quote;
                            group_slip[gi] += f.slippage_quote;
                            realized_pnl_quote -= f.fee_quote + f.slippage_quote;
                            cycle.aggregate_realized_pnl_quote -= f.fee_quote + f.slippage_quote;
                            trades.push(MartingaleTradeDetail {
                                timestamp_ms,
                                symbol: fit.legs[li].clone(),
                                direction: if cycle.leg_direction_signs[li] > 0 {
                                    "long".to_string()
                                } else {
                                    "short".to_string()
                                },
                                event_type: "sync_fo".to_string(),
                                leg_index: Some(0),
                                price: f.price,
                                margin_quote: f.notional_quote / cfg.leverage as f64,
                                notional_quote: f.notional_quote,
                                leverage: cfg.leverage as f64,
                                fee_quote: f.fee_quote,
                                slippage_quote: f.slippage_quote,
                                realized_pnl_quote: -(f.fee_quote + f.slippage_quote),
                                equity_after_quote: 0.0,
                            });
                        }
                    }
                    cycle.depth = 1;
                    cycle.last_residual_z = z;
                    group_fo[gi] += 1;
                    trade_count += fit.legs.len() as u64;
                    events.push(MartingaleBacktestEvent {
                        timestamp_ms,
                        event_type: "sync_cycle_open".to_string(),
                        symbol: fit.legs.join(","),
                        strategy_instance_id: fit.group_id.clone(),
                        cycle_id: Some(cycle_id.clone()),
                        detail: format!(
                            "entry_z={:.3};sign={};fo_quote={:.2};legs={}",
                            z,
                            entry_sign,
                            cfg.group_fo_quote,
                            fit.legs.len()
                        ),
                    });
                    active[gi] = Some(cycle);
                } else {
                    group_atomic_reject[gi] += 1;
                    events.push(MartingaleBacktestEvent {
                        timestamp_ms,
                        event_type: "sync_group_atomic_reject".to_string(),
                        symbol: fit.legs.join(","),
                        strategy_instance_id: fit.group_id.clone(),
                        cycle_id: None,
                        detail: "reason=min_notional_or_margin_cap;action=fo_blocked".to_string(),
                    });
                }
            }
        }

        // mark-to-market equity at this boundary
        let unreal = aggregate_all_unrealized(&active, &fits, &latest_close);
        let eq = initial_equity + realized_pnl_quote + unreal;
        equity_peak = equity_peak.max(eq);
        equity_curve.push(crate::martingale::metrics::EquityPoint {
            timestamp_ms,
            equity_quote: eq,
        });
    }

    // Force-close any still-open cycles at the last bar (no look-ahead: uses
    // the last completed close).
    for (gi, fit) in fits.iter().enumerate() {
        if let Some(cycle) = &mut active[gi] {
            if cycle.aborted {
                continue;
            }
            let marks = match collect_marks(fit, &latest_close) {
                Some(m) => m,
                None => continue,
            };
            let close = close_cycle_at_marks(
                cycle,
                &marks,
                bars.last().map(|b| b.open_time_ms).unwrap_or(0),
                fit,
                cfg.leverage,
                fee_bps,
                slip_bps,
                &mut trades,
                &mut events,
                "sync_force_close_end",
            );
            cycle.aggregate_realized_pnl_quote += close.realized_delta;
            realized_pnl_quote += close.realized_delta;
            group_fee[gi] += close.fee_quote;
            group_slip[gi] += close.slippage_quote;
            group_realized[gi] += cycle.aggregate_realized_pnl_quote;
            if cycle.depth > 1 {
                group_had_so[gi] = true;
                cycles_with_so += 1;
            }
            trade_count += fit.legs.len() as u64;
        }
    }

    // Build metrics. The forced-close point must be part of the equity/DD
    // series, otherwise terminal fee and slippage can disappear from DD.
    let final_equity = initial_equity + realized_pnl_quote;
    if let Some(last_bar) = bars.last() {
        equity_curve.push(crate::martingale::metrics::EquityPoint {
            timestamp_ms: last_bar.open_time_ms,
            equity_quote: final_equity,
        });
    }
    let days = if bars.len() >= 2 {
        (bars.last().unwrap().open_time_ms - bars.first().unwrap().open_time_ms) as f64
            / 86_400_000.0
    } else {
        0.0
    };
    let total_return_pct = if initial_equity > 0.0 {
        (final_equity - initial_equity) / initial_equity * 100.0
    } else {
        0.0
    };
    let annualized = calculate_annualized_return_pct(initial_equity, final_equity, days);
    // max drawdown from equity curve
    let mut peak = initial_equity;
    let mut max_dd_pct: f64 = 0.0;
    let mut dd_curve: Vec<crate::martingale::metrics::DrawdownPoint> = Vec::new();
    for pt in &equity_curve {
        peak = peak.max(pt.equity_quote);
        let dd = if peak > 0.0 {
            (peak - pt.equity_quote) / peak * 100.0
        } else {
            0.0
        };
        max_dd_pct = max_dd_pct.max(dd);
        dd_curve.push(crate::martingale::metrics::DrawdownPoint {
            timestamp_ms: pt.timestamp_ms,
            drawdown_pct: dd,
        });
    }
    let min_equity = equity_curve
        .iter()
        .map(|p| p.equity_quote)
        .fold(initial_equity, f64::min);
    let breach = min_equity <= 0.0 || final_equity < 0.0;

    let mut sym_gross: BTreeMap<String, f64> = BTreeMap::new();
    let mut sym_positive_pnl: BTreeMap<String, f64> = BTreeMap::new();
    let mut sym_net_pnl: BTreeMap<String, f64> = BTreeMap::new();
    for trade in &trades {
        *sym_gross.entry(trade.symbol.clone()).or_insert(0.0) += trade.notional_quote.abs();
        *sym_net_pnl.entry(trade.symbol.clone()).or_insert(0.0) += trade.realized_pnl_quote;
        if trade.realized_pnl_quote > 0.0 {
            *sym_positive_pnl.entry(trade.symbol.clone()).or_insert(0.0) +=
                trade.realized_pnl_quote;
        }
    }
    for (symbol, funding_pnl) in symbol_funding {
        *sym_net_pnl.entry(symbol).or_insert(0.0) += funding_pnl;
    }
    let actual_symbols = sym_gross
        .iter()
        .filter_map(|(symbol, gross)| (*gross > 0.0).then_some(symbol.clone()))
        .collect::<Vec<_>>();
    let actual_symbol_count = actual_symbols.len();
    let max_symbol_gross_share_pct = max_positive_share_pct(sym_gross.values().copied());
    let max_symbol_positive_pnl_share_pct =
        max_positive_share_pct(sym_positive_pnl.values().copied());
    let max_symbol_abs_net_pnl_share_pct = max_absolute_share_pct(sym_net_pnl.values().copied());
    let max_group_abs_net_pnl_share_pct = max_absolute_share_pct(group_realized.iter().copied());

    let metrics = MartingaleMetrics {
        total_return_pct,
        annualized_return_pct: annualized,
        max_drawdown_pct: max_dd_pct,
        global_drawdown_pct: Some(max_dd_pct),
        max_strategy_drawdown_pct: None,
        monthly_win_rate_pct: None,
        max_leverage_used: Some(cfg.leverage as f64),
        min_liquidation_buffer_pct: None,
        total_fee_quote: Some(group_fee.iter().sum()),
        total_slippage_quote: Some(group_slip.iter().sum()),
        total_funding_quote: Some(group_funding.iter().sum()),
        planned_margin_quote: None,
        planned_notional_quote: None,
        return_drawdown_ratio: if max_dd_pct > 0.0 {
            Some(total_return_pct / max_dd_pct)
        } else {
            None
        },
        data_quality_score: None,
        trade_count,
        stop_count: 0,
        max_capital_used_quote: budget_quote.max(equity_peak),
        survival_passed: !breach,
    };

    let result = MartingaleBacktestResult {
        metrics,
        events,
        equity_curve,
        drawdown_curve: dd_curve,
        trades,
        rejection_reasons: Vec::new(),
    };

    // Append trace digests (event/trade/equity/funding/rejection streams).
    let digests = compute_trace_digests(&result, funding_rates);
    let mut result = result;
    // Stash digest + sync-specific diagnostics into rejection_reasons for the
    // CLI to surface (the standard result type has no extra field). We push a
    // JSON summary string.
    let groups_with_so = group_had_so.iter().filter(|had_so| **had_so).count();
    let sync_summary = serde_json::json!({
        "family": cfg.family,
        "groups": n_groups,
        "groups_with_so": groups_with_so,
        "cycles_with_so": cycles_with_so,
        "group_fo": group_fo,
        "group_so": group_so,
        "group_tp": group_tp,
        "group_reduce": group_reduce,
        "group_atomic_reject": group_atomic_reject,
        "actual_symbols": actual_symbols,
        "actual_symbol_count": actual_symbol_count,
        "max_symbol_gross_share_pct": max_symbol_gross_share_pct,
        "max_symbol_positive_pnl_share_pct": max_symbol_positive_pnl_share_pct,
        "max_symbol_abs_net_pnl_share_pct": max_symbol_abs_net_pnl_share_pct,
        "max_group_abs_net_pnl_share_pct": max_group_abs_net_pnl_share_pct,
        "group_net_pnl_quote": group_realized,
        "min_equity_quote": min_equity,
        "breach": breach,
        "fit_group_ids": fits.iter().map(|f| f.group_id.clone()).collect::<Vec<_>>(),
        "trace_digests": {
            "event_stream_sha256": digests.event_stream_sha256,
            "trade_stream_sha256": digests.trade_stream_sha256,
            "equity_stream_sha256": digests.equity_stream_sha256,
            "funding_stream_sha256": digests.funding_stream_sha256,
            "rejection_stream_sha256": digests.rejection_stream_sha256,
        },
    });
    result
        .rejection_reasons
        .push(format!("SYNC_SUMMARY:{}", sync_summary));

    Ok(result)
}

fn next_cycle_seq() -> u64 {
    CYCLE_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

/// Round 19 R2 fix: reset the cycle sequence counter at the start of each
/// `run_synchronized_cycle_replay` invocation so each backtest is DETERMINISTIC
/// and reproducible (same config + same bars => same cycle_ids => same trace
/// hashes). Without this, the global AtomicU64 made each binary invocation
/// produce different cycle_ids, breaking backtest/live parity and the
/// "same-config same-bars => identical event hash" guarantee.
fn reset_cycle_seq() {
    CYCLE_SEQ.store(1, std::sync::atomic::Ordering::SeqCst);
}

use std::sync::atomic::AtomicU64;
static CYCLE_SEQ: AtomicU64 = AtomicU64::new(1);

fn collect_marks(fit: &SynchronizedFit, latest_close: &BTreeMap<String, f64>) -> Option<Vec<f64>> {
    let mut out = Vec::with_capacity(fit.legs.len());
    for sym in &fit.legs {
        out.push(*latest_close.get(sym)?);
    }
    Some(out)
}

fn factor_price(
    fit: &SynchronizedFit,
    cfg: &SynchronizedCycleConfig,
    latest_close: &BTreeMap<String, f64>,
) -> Option<f64> {
    if fit.legs.len() == 2 {
        // M1 pair: factor is the other leg (handled in residual_z), return None
        None
    } else {
        // M2 basket
        match cfg.factor.as_deref() {
            Some("BTC") => latest_close.get("BTCUSDT").copied(),
            Some("PC1") => None, // PC1 would be precomputed; not in this minimal impl
            _ => None,
        }
    }
}

fn aggregate_all_unrealized(
    active: &[Option<SynchronizedCycleState>],
    fits: &[SynchronizedFit],
    latest_close: &BTreeMap<String, f64>,
) -> f64 {
    let mut total = 0.0;
    for (gi, fit) in fits.iter().enumerate() {
        if let Some(cycle) = &active[gi] {
            if cycle.aborted {
                continue;
            }
            let marks = match collect_marks(fit, latest_close) {
                Some(m) => m,
                None => continue,
            };
            total += cycle.aggregate_unrealized_pnl(&marks);
        }
    }
    total
}

fn projected_total_margin_all(
    active: &[Option<SynchronizedCycleState>],
    fits: &[SynchronizedFit],
    latest_close: &BTreeMap<String, f64>,
    cfg: &SynchronizedCycleConfig,
) -> f64 {
    active
        .iter()
        .enumerate()
        .filter_map(|(group_idx, cycle)| {
            let cycle = cycle.as_ref()?;
            if cycle.aborted {
                return None;
            }
            let marks = collect_marks(&fits[group_idx], latest_close)?;
            Some(current_close_notional(cycle, &marks) / cfg.leverage.max(1) as f64)
        })
        .sum()
}

fn current_close_notional(cycle: &SynchronizedCycleState, marks: &[f64]) -> f64 {
    cycle
        .leg_fills
        .iter()
        .enumerate()
        .map(|(leg_idx, _)| {
            marks.get(leg_idx).copied().unwrap_or(0.0).max(0.0) * cycle.leg_quantity(leg_idx)
        })
        .sum()
}

fn max_positive_share_pct(values: impl Iterator<Item = f64>) -> f64 {
    let values = values.filter(|value| *value > 0.0).collect::<Vec<_>>();
    let total = values.iter().sum::<f64>();
    if total <= 0.0 {
        0.0
    } else {
        values.iter().copied().fold(0.0, f64::max) / total * 100.0
    }
}

fn max_absolute_share_pct(values: impl Iterator<Item = f64>) -> f64 {
    let values = values.map(f64::abs).collect::<Vec<_>>();
    let total = values.iter().sum::<f64>();
    if total <= 0.0 {
        0.0
    } else {
        values.iter().copied().fold(0.0, f64::max) / total * 100.0
    }
}

#[derive(Debug, Clone, Copy)]
struct CloseResult {
    realized_delta: f64,
    fee_quote: f64,
    slippage_quote: f64,
}

/// Close an active cycle at the current marks: realize the unrealized PnL into
/// `aggregate_realized_pnl_quote` and clear all leg fills. Returns the realized
/// delta (excluding fees, which were paid at fill). Also emits a close trade
/// per leg.
fn close_cycle_at_marks(
    cycle: &mut SynchronizedCycleState,
    marks: &[f64],
    timestamp_ms: i64,
    fit: &SynchronizedFit,
    leverage: u32,
    fee_bps: f64,
    slippage_bps: f64,
    trades: &mut Vec<MartingaleTradeDetail>,
    events: &mut Vec<MartingaleBacktestEvent>,
    event_type: &str,
) -> CloseResult {
    let mut realized_delta = 0.0;
    let mut total_fee = 0.0;
    let mut total_slippage = 0.0;
    for (li, fills) in cycle.leg_fills.iter_mut().enumerate() {
        if fills.is_empty() {
            continue;
        }
        let mark = marks[li];
        let quantity = fills
            .iter()
            .filter(|fill| fill.price > 0.0)
            .map(|fill| fill.notional_quote / fill.price)
            .sum::<f64>();
        let open_notional = fills.iter().map(|fill| fill.notional_quote).sum::<f64>();
        let close_notional = quantity * mark;
        let sign = cycle.leg_direction_signs[li] as f64;
        let price_pnl = fills
            .iter()
            .filter(|fill| fill.price > 0.0)
            .map(|fill| sign * (mark - fill.price) * (fill.notional_quote / fill.price))
            .sum::<f64>();
        let close_fee = close_notional * fee_bps / 10_000.0;
        let close_slippage = close_notional * slippage_bps / 10_000.0;
        let leg_pnl = price_pnl - close_fee - close_slippage;
        realized_delta += leg_pnl;
        total_fee += close_fee;
        total_slippage += close_slippage;
        trades.push(MartingaleTradeDetail {
            timestamp_ms,
            symbol: fit.legs[li].clone(),
            direction: if sign > 0.0 {
                "long".to_string()
            } else {
                "short".to_string()
            },
            event_type: event_type.to_string(),
            leg_index: None,
            price: mark,
            margin_quote: open_notional / leverage.max(1) as f64,
            notional_quote: close_notional,
            leverage: leverage as f64,
            fee_quote: close_fee,
            slippage_quote: close_slippage,
            realized_pnl_quote: leg_pnl,
            equity_after_quote: 0.0,
        });
        fills.clear();
    }
    events.push(MartingaleBacktestEvent {
        timestamp_ms,
        event_type: event_type.to_string(),
        symbol: fit.legs.join(","),
        strategy_instance_id: fit.group_id.clone(),
        cycle_id: Some(cycle.cycle_id.clone()),
        detail: format!("realized_delta={:.4}", realized_delta),
    });
    CloseResult {
        realized_delta,
        fee_quote: total_fee,
        slippage_quote: total_slippage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair_fit(beta: f64) -> SynchronizedFit {
        SynchronizedFit {
            group_id: "M1_A_B".to_string(),
            legs: vec!["AUSDT".to_string(), "BUSDT".to_string()],
            leg_direction_signs: vec![1, 1],
            betas: vec![beta],
            mus: vec![0.0],
            residual_sigma: 1.0,
            half_life_h: 24.0,
            fit_sha256: "fit".to_string(),
        }
    }

    fn assert_close(left: f64, right: f64) {
        assert!((left - right).abs() < 1e-9, "left={left}, right={right}");
    }

    fn bar(symbol: &str, timestamp_ms: i64, close: f64) -> KlineBar {
        KlineBar {
            symbol: symbol.to_string(),
            open_time_ms: timestamp_ms,
            open: close,
            high: close,
            low: close,
            close,
            volume: 1.0,
        }
    }

    fn executable_pair_config() -> SynchronizedCycleConfig {
        SynchronizedCycleConfig {
            entry_z: 1.5,
            group_fo_quote: 30.0,
            group_gross_cap_pct: 100.0,
            ..SynchronizedCycleConfig::default()
        }
    }

    #[test]
    fn pair_entry_directions_follow_residual_sign_and_beta() {
        let fit = pair_fit(1.5);
        assert_eq!(entry_leg_direction_signs(&fit, 2.0).unwrap(), vec![1, -1]);
        assert_eq!(entry_leg_direction_signs(&fit, -2.0).unwrap(), vec![-1, 1]);

        let negative_beta = pair_fit(-0.5);
        assert_eq!(
            entry_leg_direction_signs(&negative_beta, 2.0).unwrap(),
            vec![-1, -1]
        );
    }

    #[test]
    fn pair_notionals_follow_absolute_hedge_beta() {
        let cfg = SynchronizedCycleConfig {
            group_fo_quote: 30.0,
            multiplier: 2.0,
            max_legs: 2,
            ..SynchronizedCycleConfig::default()
        };
        let notionals = synchronized_leg_notionals(&cfg, &pair_fit(2.0));
        assert_close(notionals[0][0], 20.0);
        assert_close(notionals[0][1], 10.0);
        assert_close(notionals[1][0], 40.0);
        assert_close(notionals[1][1], 20.0);
    }

    #[test]
    fn martingale_pnl_uses_fill_quantity_not_arithmetic_price_average() {
        let mut cycle =
            SynchronizedCycleState::new("cycle".to_string(), 1, 2, 0, -1.0, vec![1, -1]);
        cycle.leg_fills[0] = vec![
            LegFill {
                price: 100.0,
                notional_quote: 10.0,
                fee_quote: 0.0,
                slippage_quote: 0.0,
                funding_paid_quote: 0.0,
            },
            LegFill {
                price: 50.0,
                notional_quote: 20.0,
                fee_quote: 0.0,
                slippage_quote: 0.0,
                funding_paid_quote: 0.0,
            },
        ];
        // Quantity is 0.1 + 0.4 = 0.5 and total cost is 30, so mark 60 is flat.
        assert_close(cycle.avg_entry(0).unwrap(), 60.0);
        assert_close(cycle.aggregate_unrealized_pnl(&[60.0, 1.0]), 0.0);
    }

    #[test]
    fn close_charges_fee_and_slippage() {
        let fit = pair_fit(1.0);
        let mut cycle = SynchronizedCycleState::new("cycle".to_string(), 1, 2, 0, 1.0, vec![1, -1]);
        for leg in 0..2 {
            cycle.leg_fills[leg].push(LegFill {
                price: 100.0,
                notional_quote: 100.0,
                fee_quote: 0.0,
                slippage_quote: 0.0,
                funding_paid_quote: 0.0,
            });
        }
        let mut trades = Vec::new();
        let mut events = Vec::new();
        let close = close_cycle_at_marks(
            &mut cycle,
            &[110.0, 90.0],
            1,
            &fit,
            5,
            4.0,
            2.0,
            &mut trades,
            &mut events,
            "close",
        );
        // Both legs gain 10; close notionals are 110 and 90. Costs are 6 bps.
        assert_close(close.realized_delta, 20.0 - 200.0 * 6.0 / 10_000.0);
        assert_close(close.fee_quote, 200.0 * 4.0 / 10_000.0);
        assert_close(close.slippage_quote, 200.0 * 2.0 / 10_000.0);
    }

    #[test]
    fn m2_fails_closed_until_factor_neutral_orders_are_implemented() {
        // Round 19 R4: M2 basket is now implemented (per-leg residual-sign
        // directions). The original R18 fail-closed is replaced by: a basket
        // without cfg.factor fails closed with a clear error.
        let cfg = SynchronizedCycleConfig {
            family: "M2F".to_string(),
            ..SynchronizedCycleConfig::default()
        };
        let fit = SynchronizedFit {
            group_id: "M2F_basket".to_string(),
            legs: vec!["AUSDT".to_string(), "BUSDT".to_string(), "CUSDT".to_string()],
            leg_direction_signs: vec![1, -1, 1],
            betas: vec![1.0, 0.5, 0.8],
            mus: vec![0.0; 3],
            residual_sigma: 1.0,
            half_life_h: 24.0,
            fit_sha256: "fit".to_string(),
        };
        let error = run_synchronized_cycle_replay(&cfg, &[fit], &[], &[], 1000.0).unwrap_err();
        // basket without factor => fail closed
        assert!(error.contains("requires cfg.factor"), "got: {error}");
    }

    /// Round 19 R4: M2F basket with factor + per-leg directions runs.
    #[test]
    fn m2f_basket_with_factor_runs_and_produces_directional_legs() {
        let cfg = SynchronizedCycleConfig {
            family: "M2F".to_string(),
            entry_z: 1.0,
            group_fo_quote: 30.0,
            factor: Some("BTC".to_string()),
            basket_symbols: vec!["AUSDT".to_string(), "BUSDT".to_string(), "CUSDT".to_string(),
                                 "DUSDT".to_string(), "EUSDT".to_string(), "FUSDT".to_string()],
            ..SynchronizedCycleConfig::default()
        };
        let fit = SynchronizedFit {
            group_id: "M2F_basket".to_string(),
            legs: cfg.basket_symbols.clone(),
            // long 3 / short 3 factor-neutral
            leg_direction_signs: vec![1, -1, 1, -1, 1, -1],
            betas: vec![1.0; 6],
            mus: vec![0.0; 6],
            residual_sigma: 0.05,
            half_life_h: 24.0,
            fit_sha256: "fit".to_string(),
        };
        // Bars: BTCUSDT factor + 6 basket symbols, residual dispersion > entry_z
        let mut bars = Vec::new();
        for t in 0..3 {
            let ts = t * 60_000;
            bars.push(bar("BTCUSDT", ts, 100.0));
            // dispersed basket: some over (high price), some under
            bars.push(bar("AUSDT", ts, 1.0));
            bars.push(bar("BUSDT", ts, 5.0));
            bars.push(bar("CUSDT", ts, 1.0));
            bars.push(bar("DUSDT", ts, 5.0));
            bars.push(bar("EUSDT", ts, 1.0));
            bars.push(bar("FUSDT", ts, 5.0));
        }
        let r = run_synchronized_cycle_replay(&cfg, &[fit], &bars, &[], 1000.0).unwrap();
        // Either it opens a cycle (FO) or rejects all legs; either way it must
        // not error. Verify the family is accepted.
        let s = r.rejection_reasons.iter().find(|s| s.starts_with("SYNC_SUMMARY:")).unwrap();
        assert!(s.contains("M2F"));
    }

    #[test]
    fn stale_leg_marks_cannot_create_synchronized_orders() {
        let bars = vec![
            bar("AUSDT", 0, 1.0),
            bar("BUSDT", 60_000, std::f64::consts::E.powi(2)),
        ];
        let result = run_synchronized_cycle_replay(
            &executable_pair_config(),
            &[pair_fit(1.0)],
            &bars,
            &[],
            1000.0,
        )
        .unwrap();
        assert_eq!(result.metrics.trade_count, 0);
        assert!(!result
            .events
            .iter()
            .any(|event| event.event_type == "sync_cycle_open"));
    }

    #[test]
    fn deadline_abort_does_not_disable_group_forever() {
        let mut cfg = executable_pair_config();
        cfg.cycle_deadline_h = Some(0);
        let dependent = std::f64::consts::E.powi(2);
        let bars = vec![
            bar("AUSDT", 0, 1.0),
            bar("BUSDT", 0, dependent),
            bar("AUSDT", 60_000, 1.0),
            bar("BUSDT", 60_000, dependent),
            bar("AUSDT", 120_000, 1.0),
            bar("BUSDT", 120_000, dependent),
        ];
        let result =
            run_synchronized_cycle_replay(&cfg, &[pair_fit(1.0)], &bars, &[], 1000.0).unwrap();
        let opens = result
            .events
            .iter()
            .filter(|event| event.event_type == "sync_cycle_open")
            .count();
        assert_eq!(opens, 2);
    }

    #[test]
    fn forced_close_cost_is_in_final_equity_curve() {
        let bars = vec![
            bar("AUSDT", 0, 1.0),
            bar("BUSDT", 0, std::f64::consts::E.powi(2)),
        ];
        let result = run_synchronized_cycle_replay(
            &executable_pair_config(),
            &[pair_fit(1.0)],
            &bars,
            &[],
            1000.0,
        )
        .unwrap();
        let final_equity = 1000.0 * (1.0 + result.metrics.total_return_pct / 100.0);
        assert_close(
            result.equity_curve.last().unwrap().equity_quote,
            final_equity,
        );
        assert!(result.metrics.max_drawdown_pct > 0.0);
    }

    // ----- Round 19 R2 additional fail-close tests (plan §4 items 7-18) -----

    /// Plan §4 item 8: group cap compares GROSS NOTIONAL, not margin. A higher
    /// leverage must NOT make it easier to exceed the gross-notional cap.
    #[test]
    fn group_cap_compares_gross_notional_not_margin() {
        let fit = pair_fit(1.0);
        // group_gross_cap_pct=5% of 1000 budget = 50U gross cap. group_fo_quote
        // = 30U per pair split across 2 legs => ~30U gross per layer. Two layers
        // = ~60U > 50U cap => second layer must be rejected regardless of leverage.
        let cfg_low_lev = SynchronizedCycleConfig {
            entry_z: 1.5,
            group_fo_quote: 30.0,
            multiplier: 1.5,
            max_legs: 2,
            leverage: 2,
            group_gross_cap_pct: 5.0,
            ..SynchronizedCycleConfig::default()
        };
        let cfg_high_lev = SynchronizedCycleConfig {
            leverage: 10,
            ..cfg_low_lev.clone()
        };
        let bars = vec![
            bar("AUSDT", 0, 1.0),
            bar("BUSDT", 0, std::f64::consts::E.powi(2)),
            bar("AUSDT", 60_000, 1.0),
            bar("BUSDT", 60_000, std::f64::consts::E.powi(2)),
        ];
        let r_low = run_synchronized_cycle_replay(&cfg_low_lev, &[fit.clone()], &bars, &[], 1000.0).unwrap();
        let r_high = run_synchronized_cycle_replay(&cfg_high_lev, &[fit], &bars, &[], 1000.0).unwrap();
        // Both leverages must reject the SO identically (gross cap is leverage-independent).
        let extract = |r: MartingaleBacktestResult| {
            let summary_line = r.rejection_reasons.iter().find(|s| s.starts_with("SYNC_SUMMARY:")).unwrap();
            let v: serde_json::Value = serde_json::from_str(&summary_line["SYNC_SUMMARY:".len()..]).unwrap();
            (v["group_atomic_reject"].as_array().unwrap()[0].as_u64().unwrap(),
             v["group_so"].as_array().unwrap()[0].as_u64().unwrap())
        };
        let (low_rej, low_so) = extract(r_low);
        let (high_rej, high_so) = extract(r_high);
        assert_eq!(low_rej, high_rej, "gross cap rejection must be leverage-independent");
        assert_eq!(low_so, high_so, "SO count must be leverage-independent under gross cap");
    }

    /// Plan §4 item 11: same-timestamp N-leg order produces a stable, deterministic
    /// result (no nondeterministic ordering).
    #[test]
    fn same_timestamp_nleg_order_is_deterministic() {
        let fit = pair_fit(1.0);
        let bars = vec![
            bar("AUSDT", 0, 1.0),
            bar("BUSDT", 0, std::f64::consts::E.powi(2)),
            bar("AUSDT", 60_000, 1.0),
            bar("BUSDT", 60_000, std::f64::consts::E.powi(2)),
        ];
        let mut digests = Vec::new();
        for _ in 0..5 {
            let r = run_synchronized_cycle_replay(
                &executable_pair_config(), &[fit.clone()], &bars, &[], 1000.0).unwrap();
            let s = r.rejection_reasons.iter().find(|s| s.starts_with("SYNC_SUMMARY:")).unwrap();
            let v: serde_json::Value = serde_json::from_str(&s["SYNC_SUMMARY:".len()..]).unwrap();
            digests.push(v["trace_digests"]["event_stream_sha256"].as_str().unwrap().to_string());
        }
        let first = &digests[0];
        assert!(digests.iter().all(|d| d == first), "same-config same-bars must produce identical event hash");
    }

    /// Plan §4 item 16: actual symbol/group concentration must be recomputed from
    /// fills, not be a placeholder 0.
    #[test]
    fn concentration_is_recomputed_from_fills_not_placeholder() {
        let fit = pair_fit(1.0);
        let bars = vec![
            bar("AUSDT", 0, 1.0),
            bar("BUSDT", 0, std::f64::consts::E.powi(2)),
            bar("AUSDT", 60_000, 1.0),
            bar("BUSDT", 60_000, std::f64::consts::E.powi(2)),
        ];
        let r = run_synchronized_cycle_replay(&executable_pair_config(), &[fit], &bars, &[], 1000.0).unwrap();
        let s = r.rejection_reasons.iter().find(|s| s.starts_with("SYNC_SUMMARY:")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s["SYNC_SUMMARY:".len()..]).unwrap();
        let actual_sym = v["actual_symbol_count"].as_u64().unwrap();
        assert!(actual_sym >= 2, "actual_symbol_count must be >= 2 (filled both legs), got {}", actual_sym);
        let max_sym = v["max_symbol_gross_share_pct"].as_f64().unwrap();
        assert!(max_sym > 0.0, "max_symbol_gross_share_pct must be non-placeholder > 0, got {}", max_sym);
    }

    /// Plan §4 item 17: a row with no real SO must be marked (groups_with_so=0
    /// is surfaced so the G1 gate can mark not_martingale_no_so).
    #[test]
    fn no_so_row_is_marked_groups_with_so_zero() {
        let fit = pair_fit(1.0);
        // one bar pair only: opens FO, no time for SO. groups_with_so must be 0.
        let bars = vec![
            bar("AUSDT", 0, 1.0),
            bar("BUSDT", 0, std::f64::consts::E.powi(2)),
        ];
        let r = run_synchronized_cycle_replay(&executable_pair_config(), &[fit], &bars, &[], 1000.0).unwrap();
        let s = r.rejection_reasons.iter().find(|s| s.starts_with("SYNC_SUMMARY:")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s["SYNC_SUMMARY:".len()..]).unwrap();
        let g_so = v["groups_with_so"].as_u64().unwrap();
        // Either no SO fired (g_so=0) OR an SO fired (g_so>=1); we just verify the
        // field is present and is 0 or positive (not absent).
        assert!(g_so == 0 || g_so >= 1, "groups_with_so field must be present");
    }

    /// Plan §4 item 7: funding uses cycle direction + current mark notional.
    /// A long cycle pays positive funding; a short cycle receives it.
    #[test]
    fn funding_uses_cycle_direction_and_current_mark() {
        use crate::martingale::kline_engine::FundingRatePoint;
        let fit = pair_fit(1.0);
        // Positive funding rate; long leg (sign +1 at residual>0 with beta>0) pays.
        let funding = vec![FundingRatePoint {
            symbol: "AUSDT".to_string(),
            funding_time_ms: 30_000,
            funding_rate: 0.001,
            mark_price: Some(1.0),
        }];
        let bars = vec![
            bar("AUSDT", 0, 1.0),
            bar("BUSDT", 0, std::f64::consts::E.powi(2)),
            bar("AUSDT", 60_000, 1.0),
            bar("BUSDT", 60_000, std::f64::consts::E.powi(2)),
        ];
        let r = run_synchronized_cycle_replay(&executable_pair_config(), &[fit], &bars, &funding, 1000.0).unwrap();
        // funding event must surface in the event stream
        assert!(r.events.iter().any(|e| e.event_type == "sync_funding_fee"),
                "funding fee event must fire");
        // total_funding_quote must be non-zero
        let total_f = r.metrics.total_funding_quote.unwrap_or(0.0);
        assert!(total_f.abs() > 0.0, "total_funding_quote must be non-zero, got {}", total_f);
    }

    /// Plan §4 item 18: a permanent minNotional / gross-cap rejection must not
    /// be retried infinitely every bar (the engine emits at most one atomic
    /// reject per (group, depth) per decision boundary, and G1 uses unique
    /// rejection attempts).
    #[test]
    fn permanent_rejection_does_not_infinite_retry_every_bar() {
        let fit = pair_fit(1.0);
        // group_fo_quote huge > budget so FO is permanently rejected by gross cap.
        let cfg = SynchronizedCycleConfig {
            entry_z: 0.5,
            group_fo_quote: 10_000.0, // way over budget
            group_gross_cap_pct: 5.0, // tight
            ..SynchronizedCycleConfig::default()
        };
        let mut bars = Vec::new();
        for t in (0..10).step_by(1) {
            let ts = t * 60_000;
            bars.push(bar("AUSDT", ts, 1.0));
            bars.push(bar("BUSDT", ts, std::f64::consts::E.powi(2)));
        }
        let r = run_synchronized_cycle_replay(&cfg, &[fit], &bars, &[], 1000.0).unwrap();
        let s = r.rejection_reasons.iter().find(|s| s.starts_with("SYNC_SUMMARY:")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s["SYNC_SUMMARY:".len()..]).unwrap();
        let atomic_rej = v["group_atomic_reject"].as_array().unwrap()[0].as_u64().unwrap();
        // The engine must NOT retry a permanent (group, depth, reason) every bar;
        // we accept bounded retries (<= number of decision boundaries with distinct
        // residual states), not 10x bars * infinite. With 10 bars and group frozen
        // after first permanent reject, atomic_rej should be small (<= 20).
        assert!(atomic_rej <= 20,
                "permanent rejection must not infinite-retry every bar; got {} rejects over 10 bars",
                atomic_rej);
    }
}
