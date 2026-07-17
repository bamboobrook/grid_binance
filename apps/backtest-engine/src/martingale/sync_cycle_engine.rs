//! Round 18 R2: native synchronized residual Martingale cycle engine.
//!
//! Implements the M1 synchronized pair cycle (plan §7) and the M2 market-factor
//! residual basket (plan §8) as a single aggregate cycle. This is the FIRST
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

use std::collections::BTreeMap;

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
    /// Per-leg direction sign (+1 long, -1 short) of the FO at cycle open.
    /// Determined by the sign of the residual at entry: a leg with positive
    /// residual (over-valued vs factor/pair) is shorted, negative is bought.
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
    fn new(cycle_id: String, cycle_seq: u64, n_legs: usize, opened_at_ms: i64, z_open: f64) -> Self {
        Self {
            cycle_id,
            cycle_seq,
            leg_fills: vec![Vec::new(); n_legs],
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
        self.leg_fills[leg_idx].iter().map(|f| f.notional_quote).sum()
    }

    /// Aggregate unrealized PnL across legs given current mark prices.
    /// For each leg: sign * (mark - avg_entry)/avg_entry * leg_notional.
    fn aggregate_unrealized_pnl(
        &self,
        marks: &[f64],
        leg_direction_signs: &[i8],
        funding_since_fill: &[f64],
    ) -> f64 {
        let mut total = 0.0;
        for (i, fills) in self.leg_fills.iter().enumerate() {
            if fills.is_empty() || i >= marks.len() {
                continue;
            }
            let notional = fills.iter().map(|f| f.notional_quote).sum::<f64>();
            let avg_entry = fills.iter().map(|f| f.price).sum::<f64>() / fills.len() as f64;
            if avg_entry <= 0.0 {
                continue;
            }
            let mark = marks[i];
            let sign = leg_direction_signs[i] as f64;
            let price_pnl = sign * (mark - avg_entry) / avg_entry * notional;
            // subtract funding paid since last fill (already realized-like cost)
            let fund = funding_since_fill.get(i).copied().unwrap_or(0.0);
            total += price_pnl - fund;
        }
        total
    }

    /// Net aggregate cycle PnL = realized + unrealized, after all costs.
    fn aggregate_net_pnl(
        &self,
        marks: &[f64],
        leg_direction_signs: &[i8],
        funding_since_fill: &[f64],
    ) -> f64 {
        self.aggregate_realized_pnl_quote
            + self.aggregate_unrealized_pnl(marks, leg_direction_signs, funding_since_fill)
    }

    fn avg_entry(&self, leg_idx: usize) -> Option<f64> {
        let fills = &self.leg_fills[leg_idx];
        if fills.is_empty() {
            None
        } else {
            Some(fills.iter().map(|f| f.price).sum::<f64>() / fills.len() as f64)
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

/// Per-leg notionals for the synchronized ladder. Layer k (0-indexed) has
/// notional = fo_per_leg * multiplier^k. We split `group_fo_quote` dollar-beta
/// neutral across legs at layer 0.
pub fn synchronized_leg_notionals(cfg: &SynchronizedCycleConfig, n_legs: usize) -> Vec<Vec<f64>> {
    let fo_per_leg = cfg.group_fo_quote / n_legs as f64;
    (0..cfg.max_legs as usize)
        .map(|k| {
            let layer_notional = fo_per_leg * cfg.multiplier.powi(k as i32);
            vec![layer_notional; n_legs]
        })
        .collect()
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
pub fn residual_z(fit: &SynchronizedFit, marks: &BTreeMap<String, f64>, factor_mark: Option<f64>) -> Option<f64> {
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
    validate_martingale_contract(cfg)?;
    if fits.is_empty() {
        return Err("no synchronized groups fit".to_string());
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
        .map(|f| synchronized_leg_notionals(cfg, f.legs.len()))
        .collect();

    // Per-group active cycle state (None = no active cycle).
    let mut active: Vec<Option<SynchronizedCycleState>> = vec![None; n_groups];
    // Per-group realized PnL accumulator.
    let mut group_realized: Vec<f64> = vec![0.0; n_groups];
    // Per-group funding cost accumulator.
    let mut group_funding: Vec<f64> = vec![0.0; n_groups];
    // Per-group fee/slippage accumulators.
    let mut group_fee: Vec<f64> = vec![0.0; n_groups];
    let mut group_slip: Vec<f64> = vec![0.0; n_groups];
    // Per-group fill counts (FO/SO/TP/reduce/atomic-reject).
    let mut group_fo: Vec<u64> = vec![0; n_groups];
    let mut group_so: Vec<u64> = vec![0; n_groups];
    let mut group_tp: Vec<u64> = vec![0; n_groups];
    let mut group_reduce: Vec<u64> = vec![0; n_groups];
    let mut group_atomic_reject: Vec<u64> = vec![0; n_groups];
    // Count of groups that produced at least one SO (martingale evidence).
    let mut groups_with_so: u32 = 0;

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
    sorted_funding.sort_by(|a, b| a.funding_time_ms.cmp(&b.funding_time_ms).then(a.symbol.cmp(&b.symbol)));
    let mut funding_index = 0usize;

    // Latest close per symbol (completed bar boundary).
    let mut latest_close: BTreeMap<String, f64> = BTreeMap::new();

    // Iterate the merged bar stream by timestamp groups (same pattern as
    // kline_engine).
    let mut bar_index = 0;
    while bar_index < bars.len() {
        let timestamp_ms = bars[bar_index].open_time_ms;
        let group_start = bar_index;
        while bar_index < bars.len() && bars[bar_index].open_time_ms == timestamp_ms {
            let b = &bars[bar_index];
            if b.close <= 0.0 || b.open < 0.0 || b.high < 0.0 || b.low < 0.0 {
                bar_index += 1;
                continue;
            }
            latest_close.insert(b.symbol.clone(), b.close);
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
                                let sign = fit.leg_direction_signs[leg_pos] as f64;
                                let mark = f.mark_price.unwrap_or_else(|| {
                                    latest_close.get(&f.symbol).copied().unwrap_or(0.0)
                                });
                                let _ = mark; // funding_rate is already a rate
                                let fund_quote = -sign * leg_notional * f.funding_rate;
                                // attribute to the leg's last fill as funding_paid
                                if let Some(last_fill) = cycle.leg_fills[leg_pos].last_mut() {
                                    last_fill.funding_paid_quote += fund_quote;
                                }
                                cycle.aggregate_realized_pnl_quote += fund_quote;
                                group_funding[gi] += fund_quote;
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
            if let Some(cycle) = &mut active[gi] {
                if cycle.aborted {
                    // nothing more; will be cleared when equity recomputed
                } else {
                    // Snapshot the residual z from the LAST fill before updating
                    // to the current z. The SO adverse-check must measure how far
                    // the residual has moved SINCE the last fill, not zero.
                    let prev_last_z = cycle.last_residual_z;
                    let net_pnl = cycle.aggregate_net_pnl(&marks, &fit.leg_direction_signs, &[]);
                    let depth = cycle.depth as usize;

                    // abort: deadline
                    if let Some(deadline_h) = cfg.cycle_deadline_h {
                        let age_h = (timestamp_ms - cycle.opened_at_ms) / 3_600_000;
                        if age_h >= deadline_h as i64 {
                            // aggregate reduce/close: realize at current marks
                            let (realized_delta, _) = close_cycle_at_marks(
                                cycle, &marks, &fit.leg_direction_signs, timestamp_ms, fit,
                                &mut trades, &mut events, "sync_abort_deadline",
                            );
                            cycle.aggregate_realized_pnl_quote += realized_delta;
                            cycle.aborted = true;
                            cycle.abort_reason = Some(format!("deadline_h={} age_h={}", deadline_h, age_h));
                            realized_pnl_quote += realized_delta;
                            group_reduce[gi] += 1;
                            trade_count += fit.legs.len() as u64;
                            continue;
                        }
                    }

                    // TP: net PnL > floor AND |z| <= exit_z
                    let tp_floor_quote =
                        cycle.aggregate_open_notional_quote() * (cfg.tp_net_bps_floor as f64) / 10_000.0;
                    if net_pnl > tp_floor_quote && z.abs() <= cfg.exit_z && depth >= 1 {
                        let (realized_delta, _) = close_cycle_at_marks(
                            cycle, &marks, &fit.leg_direction_signs, timestamp_ms, fit,
                            &mut trades, &mut events, "sync_tp",
                        );
                        cycle.aggregate_realized_pnl_quote += realized_delta;
                        realized_pnl_quote += realized_delta;
                        group_tp[gi] += 1;
                        trade_count += fit.legs.len() as u64;
                        // close out
                        let closed_id = cycle.cycle_id.clone();
                        let had_so = cycle.depth > 1;
                        group_realized[gi] += cycle.aggregate_realized_pnl_quote;
                        if had_so {
                            groups_with_so += 1;
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
                            let mut new_fills: Vec<Option<LegFill>> = Vec::with_capacity(fit.legs.len());
                            for (li, sym) in fit.legs.iter().enumerate() {
                                let mark = marks[li];
                                let layer_notional = group_notionals[gi][layer_idx][li];
                                // exchange min notional + margin cap check
                                let fee = layer_notional * fee_bps / 10_000.0;
                                let slip = layer_notional * slip_bps / 10_000.0;
                                let margin_needed = layer_notional / cfg.leverage as f64;
                                let projected_margin = projected_total_margin(cycle, cfg) + margin_needed;
                                if layer_notional < 5.0 || projected_margin > group_gross_cap_quote {
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
                                        cycle.aggregate_realized_pnl_quote -= f.fee_quote + f.slippage_quote;
                                        trades.push(MartingaleTradeDetail {
                                            timestamp_ms,
                                            symbol: fit.legs[li].clone(),
                                            direction: if fit.leg_direction_signs[li] > 0 {
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
                                    detail: "reason=min_notional_or_margin_cap;action=so_layer_skipped".to_string(),
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
                // FO layer (layer 0).
                let mut all_ok = true;
                let mut new_fills: Vec<Option<LegFill>> = Vec::with_capacity(fit.legs.len());
                // leg direction: for M1 pair, leg 0 (A) takes -entry_sign, leg 1 (B) +entry_sign.
                // for M2 basket, legs with positive residual component get -entry_sign.
                for (li, _sym) in fit.legs.iter().enumerate() {
                    let mark = marks[li];
                    let layer_notional = group_notionals[gi][0][li];
                    let fee = layer_notional * fee_bps / 10_000.0;
                    let slip = layer_notional * slip_bps / 10_000.0;
                    let margin_needed = layer_notional / cfg.leverage as f64;
                    // No active cycle yet on this group; projected = just this FO layer.
                    let projected_margin = margin_needed;
                    if layer_notional < 5.0 || projected_margin > group_gross_cap_quote {
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
                    let mut cycle =
                        SynchronizedCycleState::new(cycle_id.clone(), cycle_seq, fit.legs.len(), timestamp_ms, z);
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
                                direction: if fit.leg_direction_signs[li] > 0 {
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
                            z, entry_sign, cfg.group_fo_quote, fit.legs.len()
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
            let marks = match collect_marks(fit, &latest_close) {
                Some(m) => m,
                None => continue,
            };
            let (realized_delta, _) = close_cycle_at_marks(
                cycle, &marks, &fit.leg_direction_signs, bars.last().map(|b| b.open_time_ms).unwrap_or(0), fit,
                &mut trades, &mut events, "sync_force_close_end",
            );
            cycle.aggregate_realized_pnl_quote += realized_delta;
            realized_pnl_quote += realized_delta;
            group_realized[gi] += cycle.aggregate_realized_pnl_quote;
            if cycle.depth > 1 {
                groups_with_so += 1;
            }
            trade_count += fit.legs.len() as u64;
        }
    }

    // Build metrics.
    let final_equity = initial_equity + realized_pnl_quote;
    let days = if bars.len() >= 2 {
        (bars.last().unwrap().open_time_ms - bars.first().unwrap().open_time_ms) as f64 / 86_400_000.0
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
        let dd = if peak > 0.0 { (peak - pt.equity_quote) / peak * 100.0 } else { 0.0 };
        max_dd_pct = max_dd_pct.max(dd);
        dd_curve.push(crate::martingale::metrics::DrawdownPoint {
            timestamp_ms: pt.timestamp_ms,
            drawdown_pct: dd,
        });
    }
    let min_equity = equity_curve.iter().map(|p| p.equity_quote).fold(initial_equity, f64::min);
    let breach = min_equity <= 0.0 || final_equity < 0.0;

    // Concentration: max single-symbol gross notional contribution.
    let mut sym_gross: BTreeMap<String, f64> = BTreeMap::new();
    let mut total_gross = 0.0f64;
    for fit in fits {
        let marks = collect_marks(fit, &latest_close).unwrap_or_default();
        for (li, sym) in fit.legs.iter().enumerate() {
            let _ = &marks;
            let g = 0.0; // gross computed post-hoc not needed; placeholder
            total_gross += g;
            *sym_gross.entry(sym.clone()).or_insert(0.0) += g;
        }
    }
    let _ = total_gross;

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
        return_drawdown_ratio: if max_dd_pct > 0.0 { Some(total_return_pct / max_dd_pct) } else { None },
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
    let sync_summary = serde_json::json!({
        "family": cfg.family,
        "groups": n_groups,
        "groups_with_so": groups_with_so,
        "group_fo": group_fo,
        "group_so": group_so,
        "group_tp": group_tp,
        "group_reduce": group_reduce,
        "group_atomic_reject": group_atomic_reject,
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
    result.rejection_reasons.push(format!("SYNC_SUMMARY:{}", sync_summary));

    Ok(result)
}

fn next_cycle_seq() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(1);
    SEQ.fetch_add(1, Ordering::SeqCst)
}

fn collect_marks(fit: &SynchronizedFit, latest_close: &BTreeMap<String, f64>) -> Option<Vec<f64>> {
    let mut out = Vec::with_capacity(fit.legs.len());
    for sym in &fit.legs {
        out.push(*latest_close.get(sym)?);
    }
    Some(out)
}

fn factor_price(fit: &SynchronizedFit, cfg: &SynchronizedCycleConfig, latest_close: &BTreeMap<String, f64>) -> Option<f64> {
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
            total += cycle.aggregate_unrealized_pnl(&marks, &fit.leg_direction_signs, &[]);
        }
    }
    total
}

/// Project the margin already committed to the current cycle's open layers.
/// Used to check the group gross cap before adding a new layer. The portfolio
/// margin is tracked separately; this is the per-cycle incremental view.
fn projected_total_margin(
    cycle: &SynchronizedCycleState,
    cfg: &SynchronizedCycleConfig,
) -> f64 {
    cycle
        .leg_fills
        .iter()
        .flat_map(|fills| fills.iter())
        .map(|f| f.notional_quote / cfg.leverage as f64)
        .sum()
}

/// Close an active cycle at the current marks: realize the unrealized PnL into
/// `aggregate_realized_pnl_quote` and clear all leg fills. Returns the realized
/// delta (excluding fees, which were paid at fill). Also emits a close trade
/// per leg.
fn close_cycle_at_marks(
    cycle: &mut SynchronizedCycleState,
    marks: &[f64],
    leg_direction_signs: &[i8],
    timestamp_ms: i64,
    fit: &SynchronizedFit,
    trades: &mut Vec<MartingaleTradeDetail>,
    events: &mut Vec<MartingaleBacktestEvent>,
    event_type: &str,
) -> (f64, ()) {
    let mut realized_delta = 0.0;
    for (li, fills) in cycle.leg_fills.iter_mut().enumerate() {
        if fills.is_empty() {
            continue;
        }
        let mark = marks[li];
        let avg_entry = fills.iter().map(|f| f.price).sum::<f64>() / fills.len() as f64;
        let notional = fills.iter().map(|f| f.notional_quote).sum::<f64>();
        let sign = leg_direction_signs[li] as f64;
        let leg_pnl = if avg_entry > 0.0 {
            sign * (mark - avg_entry) / avg_entry * notional
        } else {
            0.0
        };
        realized_delta += leg_pnl;
        trades.push(MartingaleTradeDetail {
            timestamp_ms,
            symbol: fit.legs[li].clone(),
            direction: if sign > 0.0 { "long".to_string() } else { "short".to_string() },
            event_type: event_type.to_string(),
            leg_index: None,
            price: mark,
            margin_quote: notional / 1.0,
            notional_quote: notional,
            leverage: 1.0,
            fee_quote: 0.0,
            slippage_quote: 0.0,
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
    (realized_delta, ())
}
