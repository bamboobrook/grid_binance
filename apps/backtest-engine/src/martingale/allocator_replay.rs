//! Round 9 Task P2: forward-only martingale sleeve curve allocator.
//!
//! Pure allocator logic that selects among martingale sleeves (each sleeve is a
//! full portfolio config) using only data strictly before the interval it
//! affects. This is the Rust port of the repaired Python allocator in
//! `scripts/glm_r8_regime_rescue_allocator.py` (Round 9 repaired version).
//!
//! ## Why this exists
//!
//! Round 8 P2 allocator was Python-only and had a current-interval timing
//! leak. Round 9 P1 repaired the timing (forward-only), and this module
//! provides a Rust implementation of the same curve-selection rule. Curve
//! parity alone does not establish event-level or production parity: inactive
//! sleeve positions and shared capital are outside this module.
//!
//! ## Core invariants
//!
//! 1. **Forward-only timing**: a rebalance decision at timestamp `ts` uses
//!    equity observations up to and including `ts`, but the decision only
//!    affects the interval AFTER `ts`. The merged PnL for the step ending at
//!    `ts` is attributed to the previously active sleeve.
//! 2. **Legacy eligibility switches**: despite their field names,
//!    `max_high_ann_weight` and `min_low_dd_weight` do not implement
//!    fractional weights. Only zero/non-zero eligibility behavior is modeled.
//! 3. **No forced cycle close on switch**: existing martingale cycles are not
//!    force-closed when the active sleeve changes; new cycles open only for
//!    the active sleeve.
//!
//! ## Usage
//!
//! - `AllocatorConfig` / `AllocatorState` carry the parameters and the
//!   runtime state.
//! - `run_allocator_replay` is the pure backtest entry point that takes a
//!   pre-computed map of sleeve equity curves.
//! - The `trading-engine` consumes the same `AllocatorConfig` and uses
//!   `AllocatorState` to decide which sleeve's strategies may open new cycles
//!   at each rebalance boundary.

use serde::{Deserialize, Serialize};

use crate::martingale::metrics::EquityPoint;

const MS_PER_DAY: i64 = 86_400_000;

/// Available score functions for the allocator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllocatorScoreFunction {
    /// `rolling_return - 1.0 * rolling_max_dd`
    AnnMinus1Dd,
    /// `rolling_return - 2.0 * rolling_max_dd`
    AnnMinus2Dd,
    /// `rolling_return - 0.5 * rolling_max_dd`
    AnnMinusCost,
    /// `rolling_return / (rolling_max_dd + 1.0)` if return > 0 else
    /// `rolling_return - rolling_max_dd`
    CalmarLike,
}

impl AllocatorScoreFunction {
    pub fn evaluate(&self, rolling_return_quote: f64, rolling_max_dd_pct: f64) -> f64 {
        match self {
            Self::AnnMinus1Dd => rolling_return_quote - 1.0 * rolling_max_dd_pct,
            Self::AnnMinus2Dd => rolling_return_quote - 2.0 * rolling_max_dd_pct,
            Self::AnnMinusCost => rolling_return_quote - 0.5 * rolling_max_dd_pct,
            Self::CalmarLike => {
                if rolling_return_quote > 0.0 {
                    rolling_return_quote / (rolling_max_dd_pct + 1.0)
                } else {
                    rolling_return_quote - rolling_max_dd_pct
                }
            }
        }
    }
}

/// Configuration for the portfolio allocator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AllocatorConfig {
    /// Lookback window in days for the rolling return/DD score.
    pub lookback_days: i32,
    /// Rebalance cadence in days.
    pub rebalance_days: i32,
    /// Score function family.
    pub score_function: AllocatorScoreFunction,
    /// Legacy binary eligibility switch for high-ann sleeves. `0.0` excludes
    /// them; every positive value allows them to compete on score.
    pub max_high_ann_weight: f64,
    /// Legacy binary eligibility switch for low-DD sleeves. Every positive
    /// value keeps them eligible; no fractional allocation is created.
    pub min_low_dd_weight: f64,
    /// If `Some(pct)`, switch to cash when merged DD exceeds this percent.
    pub cash_trigger_rolling_dd_pct: Option<f64>,
    /// Hysteresis: only switch sleeves if the new best score exceeds the
    /// current active sleeve's score by this gap.
    pub switch_hysteresis_score_gap: f64,
    /// Sleeve IDs classified as high-ann (excluded when `max_high_ann_weight == 0`).
    pub high_ann_sleeve_ids: Vec<String>,
    /// Sleeve IDs classified as low-DD (forced eligible when
    /// `min_low_dd_weight > 0`).
    pub low_dd_sleeve_ids: Vec<String>,
}

impl Default for AllocatorConfig {
    fn default() -> Self {
        Self {
            lookback_days: 60,
            rebalance_days: 7,
            score_function: AllocatorScoreFunction::AnnMinus2Dd,
            max_high_ann_weight: 1.0,
            min_low_dd_weight: 0.0,
            cash_trigger_rolling_dd_pct: None,
            switch_hysteresis_score_gap: 0.0,
            high_ann_sleeve_ids: Vec::new(),
            low_dd_sleeve_ids: Vec::new(),
        }
    }
}

impl AllocatorConfig {
    fn lookback_ms(&self) -> i64 {
        (self.lookback_days as i64) * MS_PER_DAY
    }
    fn rebalance_ms(&self) -> i64 {
        (self.rebalance_days as i64) * MS_PER_DAY
    }

    /// Returns the eligible sleeve set after applying legacy binary switches.
    fn eligible_sleeves(&self, all_sleeves: &[&str]) -> Vec<String> {
        let high: std::collections::HashSet<&str> = self
            .high_ann_sleeve_ids
            .iter()
            .map(String::as_str)
            .collect();
        let low: std::collections::HashSet<&str> = self
            .low_dd_sleeve_ids
            .iter()
            .map(String::as_str)
            .collect();
        let mut eligible: Vec<String> = Vec::new();
        for s in all_sleeves {
            // If max_hi == 0, exclude high-ann sleeves entirely
            if self.max_high_ann_weight <= 0.0 && high.contains(*s) {
                continue;
            }
            eligible.push((*s).to_string());
        }
        // If min_lo > 0, ensure low-DD sleeves are eligible
        if self.min_low_dd_weight > 0.0 {
            for s in all_sleeves {
                if low.contains(*s) && !eligible.iter().any(|e| e == *s) {
                    eligible.push((*s).to_string());
                }
            }
        }
        eligible
    }
}

/// Per-sleeve equity curve keyed by sleeve ID.
pub type SleeveCurves = std::collections::HashMap<String, Vec<EquityPoint>>;

/// Per-sleeve PnL series (equity minus first equity), keyed by sleeve ID.
fn build_pnl_by(curves: &SleeveCurves) -> std::collections::HashMap<String, Vec<(i64, f64)>> {
    curves
        .iter()
        .map(|(name, ec)| {
            if ec.is_empty() {
                (name.clone(), Vec::new())
            } else {
                let first = ec[0].equity_quote;
                let pnl: Vec<(i64, f64)> = ec
                    .iter()
                    .map(|p| (p.timestamp_ms, p.equity_quote - first))
                    .collect();
                (name.clone(), pnl)
            }
        })
        .collect()
}

/// Lookup PnL at the most recent observation <= cutoff. Returns 0.0 if none.
fn pnl_at_or_before(pnl_series: &[(i64, f64)], cutoff: i64) -> f64 {
    // Series is sorted ascending by timestamp. Linear/branch search.
    let mut result = 0.0;
    for (t, v) in pnl_series {
        if *t <= cutoff {
            result = *v;
        } else {
            break;
        }
    }
    result
}

/// Compute scores for each eligible sleeve using only data <= ts.
fn compute_scores(
    curves: &SleeveCurves,
    pnl_by: &std::collections::HashMap<String, Vec<(i64, f64)>>,
    ts: i64,
    cfg: &AllocatorConfig,
    eligible: &[String],
    budget: f64,
) -> std::collections::HashMap<String, f64> {
    let lookback_start_ts = ts - cfg.lookback_ms();
    let mut scores = std::collections::HashMap::new();
    for name in eligible {
        if name == "cash" {
            continue;
        }
        let pnl_series = match pnl_by.get(name) {
            Some(s) if !s.is_empty() => s,
            _ => continue,
        };
        let pnl_now = pnl_at_or_before(pnl_series, ts);
        let pnl_past = pnl_at_or_before(pnl_series, lookback_start_ts);
        let ret = pnl_now - pnl_past;
        // DD inside lookback window (data <= ts only)
        let lb_pnls: Vec<f64> = pnl_series
            .iter()
            .filter(|(t, _)| *t >= lookback_start_ts && *t <= ts)
            .map(|(_, v)| *v)
            .collect();
        let lb_dd = if lb_pnls.is_empty() {
            0.0
        } else {
            let lb_peak = lb_pnls.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let lb_trough = lb_pnls.iter().cloned().fold(f64::INFINITY, f64::min);
            let denom = budget + lb_peak;
            if denom > 0.0 {
                ((lb_peak - lb_trough) / denom) * 100.0
            } else {
                0.0
            }
        };
        scores.insert(name.clone(), cfg.score_function.evaluate(ret, lb_dd));
    }
    scores
}

/// Output metrics from an allocator replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AllocatorMetrics {
    pub annualized_return_pct: f64,
    pub max_drawdown_pct: f64,
    pub total_return_pct: f64,
}

/// Pure forward-only curve recombination. Returns diagnostic metrics.
///
/// `budget` is the initial capital in quote units. Each sleeve's curve is
/// assumed to start at `budget` (the caller is responsible for renormalizing
/// sleeve curves if they were generated with a different starting equity).
/// The function does not model shared capital or reconcile open positions when
/// switching sleeves, so its output is not an executable portfolio backtest.
pub fn run_allocator_replay(
    curves: &SleeveCurves,
    cfg: &AllocatorConfig,
    budget: f64,
) -> Option<AllocatorMetrics> {
    let all_ts_set: std::collections::BTreeSet<i64> = curves
        .values()
        .flat_map(|ec| ec.iter().map(|p| p.timestamp_ms))
        .collect();
    let all_ts: Vec<i64> = all_ts_set.into_iter().collect();
    if all_ts.is_empty() {
        return None;
    }

    let pnl_by = build_pnl_by(curves);
    let all_sleeve_names_owned: Vec<String> = curves.keys().cloned().collect();
    let all_sleeve_names: Vec<&str> = all_sleeve_names_owned.iter().map(String::as_str).collect();
    let eligible = cfg.eligible_sleeves(&all_sleeve_names);

    // Initial active sleeve: lowest-DD eligible default (no lookahead at t0).
    let low_dd: std::collections::HashSet<&str> = cfg
        .low_dd_sleeve_ids
        .iter()
        .map(String::as_str)
        .collect();
    let mut active: Option<String> = if eligible.is_empty() {
        None
    } else {
        // Prefer a low-DD sleeve alphabetically; else first eligible.
        let init_candidates: Vec<String> = eligible
            .iter()
            .filter(|s| low_dd.contains(s.as_str()))
            .cloned()
            .collect();
        if !init_candidates.is_empty() {
            let mut sorted = init_candidates.clone();
            sorted.sort();
            Some(sorted[0].clone())
        } else {
            // fallback: alphabetically first eligible
            let mut sorted = eligible.clone();
            sorted.sort();
            sorted.first().cloned()
        }
    };

    let mut active_score: f64 = f64::NEG_INFINITY;
    let mut last_rebal_ts = all_ts[0];
    let mut merged_pnl: f64 = 0.0;
    let mut peak = budget;
    let mut max_dd: f64 = 0.0;
    let mut prev_ts = all_ts[0];

    for (i, &ts) in all_ts.iter().enumerate() {
        // STEP 1: apply the interval [prev_ts, ts] to the CURRENTLY active sleeve.
        let step: f64 = match &active {
            None => 0.0,
            Some(name) if name == "cash" => 0.0,
            Some(name) => {
                let pnl_now = pnl_at_or_before(pnl_by.get(name).unwrap_or(&Vec::new()), ts);
                let pnl_prev = pnl_at_or_before(pnl_by.get(name).unwrap_or(&Vec::new()), prev_ts);
                pnl_now - pnl_prev
            }
        };
        merged_pnl += step;
        let eq = budget + merged_pnl;
        if eq > peak {
            peak = eq;
        }
        let dd = if peak > 0.0 { ((peak - eq) / peak) * 100.0 } else { 0.0 };
        if dd > max_dd {
            max_dd = dd;
        }

        // STEP 2: maybe rebalance — uses data <= ts, applies to next interval.
        if i > 0 && (ts - last_rebal_ts) >= cfg.rebalance_ms() {
            let eligible_now = cfg.eligible_sleeves(&all_sleeve_names);
            let scores = compute_scores(curves, &pnl_by, ts, cfg, &eligible_now, budget);
            if !scores.is_empty() {
                // Find best
                let mut best: Option<(String, f64)> = None;
                for (name, &score) in &scores {
                    match &best {
                        None => best = Some((name.clone(), score)),
                        Some((_, bs)) if score > *bs => best = Some((name.clone(), score)),
                        _ => {}
                    }
                }
                if let Some((best_name, best_score)) = best {
                    // Cash DD trigger
                    if let Some(cash_dd) = cfg.cash_trigger_rolling_dd_pct {
                        if dd > cash_dd {
                            active = Some("cash".to_string());
                        }
                    } else if active.is_none()
                        || !scores.contains_key(active.as_ref().unwrap())
                        || (best_score - active_score > cfg.switch_hysteresis_score_gap)
                    {
                        active = Some(best_name);
                        active_score = best_score;
                    } else {
                        active_score = *scores
                            .get(active.as_ref().unwrap())
                            .unwrap_or(&active_score);
                    }
                }
            }
            last_rebal_ts = ts;
        }

        prev_ts = ts;
    }

    let days = ((all_ts[all_ts.len() - 1] - all_ts[0]) as f64) / (MS_PER_DAY as f64);
    let total_ret = merged_pnl / budget;
    let ann = if days > 0.0 && (1.0 + total_ret) > 0.0 {
        ((1.0 + total_ret).powf(365.0 / days) - 1.0) * 100.0
    } else {
        -999.0
    };
    Some(AllocatorMetrics {
        annualized_return_pct: ann,
        max_drawdown_pct: max_dd,
        total_return_pct: total_ret * 100.0,
    })
}

/// Live runtime state for the allocator. The `trading-engine` keeps one of
/// these and consults it at each rebalance boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AllocatorState {
    /// Currently active sleeve ID (or "cash").
    pub active_sleeve_id: String,
    /// Next timestamp (ms) at which a rebalance decision should be made.
    pub next_rebalance_ms: i64,
    /// Timestamp (ms) of the last completed interval used for scoring.
    pub last_completed_interval_ms: i64,
    /// Per-sleeve rolling metrics snapshot from the most recent rebalance.
    pub rolling_metrics_by_sleeve: std::collections::HashMap<String, RollingMetrics>,
}

/// Rolling window metrics for a single sleeve at a rebalance boundary.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RollingMetrics {
    pub rolling_return_quote: f64,
    pub rolling_max_dd_pct: f64,
    pub score: f64,
}

impl AllocatorState {
    /// Initialize a new allocator state with a given starting sleeve and the
    /// first rebalance boundary.
    pub fn new(initial_sleeve_id: String, first_rebalance_ms: i64) -> Self {
        Self {
            active_sleeve_id: initial_sleeve_id,
            next_rebalance_ms: first_rebalance_ms,
            last_completed_interval_ms: 0,
            rolling_metrics_by_sleeve: std::collections::HashMap::new(),
        }
    }

    /// Returns true if a new cycle for the given sleeve may open at `now_ms`.
    ///
    /// Existing cycles are never force-closed by the allocator; only new-cycle
    /// gating is controlled here.
    pub fn may_open_new_cycle_for(&self, sleeve_id: &str, now_ms: i64) -> bool {
        // If we are past the next rebalance boundary, the caller is expected
        // to have called `rebalance` first. We do not auto-rebalance here to
        // keep this method pure; we simply check whether the sleeve matches
        // the active one.
        if now_ms < self.next_rebalance_ms {
            // We are inside an interval — the active sleeve is whatever was
            // chosen at the most recent rebalance.
            return sleeve_id == self.active_sleeve_id;
        }
        // Past rebalance boundary without an explicit rebalance call: be
        // conservative and only allow the previously active sleeve. This
        // forces the caller to invoke rebalance() to switch.
        sleeve_id == self.active_sleeve_id
    }

    /// Apply a rebalance decision at `now_ms`. Uses the supplied per-sleeve
    /// rolling metrics (computed from data <= now_ms) to choose the new active
    /// sleeve for the interval AFTER now_ms.
    ///
    /// Returns the new active sleeve id.
    pub fn rebalance(
        &mut self,
        now_ms: i64,
        cfg: &AllocatorConfig,
        sleeve_metrics: &std::collections::HashMap<String, RollingMetrics>,
    ) -> String {
        let all_sleeve_names: Vec<&str> =
            sleeve_metrics.keys().map(String::as_str).collect();
        let eligible = cfg.eligible_sleeves(&all_sleeve_names);
        // Pick best eligible score
        let mut best: Option<(String, f64)> = None;
        for name in &eligible {
            if name == "cash" {
                continue;
            }
            if let Some(m) = sleeve_metrics.get(name) {
                match &best {
                    None => best = Some((name.clone(), m.score)),
                    Some((_, bs)) if m.score > *bs => best = Some((name.clone(), m.score)),
                    _ => {}
                }
            }
        }
        let new_active = match best {
            Some((name, score)) => {
                // Hysteresis
                let current_score = self
                    .rolling_metrics_by_sleeve
                    .get(&self.active_sleeve_id)
                    .map(|m| m.score)
                    .unwrap_or(f64::NEG_INFINITY);
                let active_still_eligible = eligible.iter().any(|e| e == &self.active_sleeve_id);
                if score - current_score > cfg.switch_hysteresis_score_gap
                    || !active_still_eligible
                {
                    name
                } else {
                    self.active_sleeve_id.clone()
                }
            }
            None => self.active_sleeve_id.clone(),
        };
        self.active_sleeve_id = new_active.clone();
        self.last_completed_interval_ms = now_ms;
        self.next_rebalance_ms = now_ms + cfg.rebalance_ms();
        self.rolling_metrics_by_sleeve = sleeve_metrics.clone();
        new_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve(points: &[(i64, f64)]) -> Vec<EquityPoint> {
        points
            .iter()
            .map(|(t, e)| EquityPoint {
                timestamp_ms: *t,
                equity_quote: *e,
            })
            .collect()
    }

    #[test]
    fn allocator_switch_uses_completed_interval_only() {
        // Two sleeves. A has a single huge gain on day 100 only. B drifts up
        // slowly throughout. With forward-only timing, the allocator cannot
        // switch to A ON the day-100 interval; the switch (if any) happens at
        // the next rebalance AFTER day 100 and applies forward.
        let base_ts = 1_700_000_000_000;
        let day_ms = MS_PER_DAY;
        let budget = 5000.0_f64;
        let mut pts_a = Vec::new();
        let mut pts_b = Vec::new();
        let mut eq_a = budget;
        let mut eq_b = budget;
        for d in 0..200 {
            let ts = base_ts + d * day_ms;
            eq_b *= 1.001;
            if d == 100 {
                eq_a *= 1.50;
            }
            pts_a.push((ts, eq_a));
            pts_b.push((ts, eq_b));
        }
        let mut curves = SleeveCurves::new();
        curves.insert("A".to_string(), curve(&pts_a));
        curves.insert("B".to_string(), curve(&pts_b));

        let cfg = AllocatorConfig {
            lookback_days: 60,
            rebalance_days: 30,
            score_function: AllocatorScoreFunction::AnnMinus1Dd,
            max_high_ann_weight: 1.0,
            min_low_dd_weight: 0.0,
            cash_trigger_rolling_dd_pct: None,
            switch_hysteresis_score_gap: 0.0,
            high_ann_sleeve_ids: Vec::new(),
            low_dd_sleeve_ids: Vec::new(),
        };
        let res = run_allocator_replay(&curves, &cfg, budget).expect("non-None");

        // The forward-only property: total return cannot systematically exceed
        // what is achievable without future info. Construct a synthetic "leaky
        // upper bound" where we pretend the allocator could have captured A's
        // full +50% spike on day 100.
        // Forward-only result should be well below the leaky +50%-capturing
        // upper bound. Specifically, since A is flat except for one day, and
        // B is steady upward, the allocator's return must be bounded by the
        // best steady-state outcome (~ B's 1.001^200 ≈ +22%).
        // Assert: forward total return is bounded by ~ +30% (well below +50%
        // leaky capture).
        assert!(
            res.total_return_pct < 30.0,
            "forward-only ret {} should be < 30% (no current-interval leak), got {}",
            res.total_return_pct,
            res.total_return_pct
        );
    }

    #[test]
    fn allocator_respects_max_high_ann_and_min_low_dd_weights() {
        // A: high return, volatile. B: low return, stable.
        let base_ts = 1_700_000_000_000;
        let day_ms = MS_PER_DAY;
        let budget = 5000.0_f64;
        let mut pts_a = Vec::new();
        let mut pts_b = Vec::new();
        let mut eq_a = budget;
        let mut eq_b = budget;
        for d in 0..300 {
            let ts = base_ts + d * day_ms;
            if d % 10 < 5 {
                eq_a *= 1.05;
            } else {
                eq_a *= 0.97;
            }
            eq_b *= 1.0005;
            pts_a.push((ts, eq_a));
            pts_b.push((ts, eq_b));
        }
        let mut curves = SleeveCurves::new();
        curves.insert("A".to_string(), curve(&pts_a));
        curves.insert("B".to_string(), curve(&pts_b));

        // Capped: max_hi=0 excludes A entirely, B forced eligible
        let cfg_capped = AllocatorConfig {
            lookback_days: 60,
            rebalance_days: 30,
            score_function: AllocatorScoreFunction::AnnMinus1Dd,
            max_high_ann_weight: 0.0,
            min_low_dd_weight: 1.0,
            cash_trigger_rolling_dd_pct: None,
            switch_hysteresis_score_gap: 0.0,
            high_ann_sleeve_ids: vec!["A".to_string()],
            low_dd_sleeve_ids: vec!["B".to_string()],
        };
        // Uncapped: allow A fully
        let cfg_uncapped = AllocatorConfig {
            max_high_ann_weight: 1.0,
            min_low_dd_weight: 0.0,
            ..cfg_capped.clone()
        };

        let res_capped = run_allocator_replay(&curves, &cfg_capped, budget).expect("capped");
        let res_uncapped = run_allocator_replay(&curves, &cfg_uncapped, budget).expect("uncapped");

        // Capped must track B (~ +16.2% over 300 days at 1.0005^300)
        let b_total = (((1.0005f64).powf(300.0)) - 1.0) * 100.0;
        assert!(
            (res_capped.total_return_pct - b_total).abs() < 5.0,
            "capped ret {} should track B-only {}, diff {}",
            res_capped.total_return_pct,
            b_total,
            (res_capped.total_return_pct - b_total).abs()
        );
        // Capped and uncapped MUST differ (params bind)
        assert!(
            (res_capped.total_return_pct - res_uncapped.total_return_pct).abs() > 0.1,
            "capped {} == uncapped {} — weight params do not bind",
            res_capped.total_return_pct,
            res_uncapped.total_return_pct
        );
    }

    #[test]
    fn live_runtime_persists_allocator_active_sleeve_until_next_rebalance() {
        // Initialize with R4 active, rebalance at day 7, verify that within
        // the [day0, day7) interval only R4 may open new cycles, and that
        // after the rebalance the new active sleeve is used.
        let mut state = AllocatorState::new("R4".to_string(), 7 * MS_PER_DAY);
        // Before rebalance: only R4 may open
        assert!(state.may_open_new_cycle_for("R4", 3 * MS_PER_DAY));
        assert!(!state.may_open_new_cycle_for("ANKR", 3 * MS_PER_DAY));

        // Simulate rebalance at day 7 with synthetic metrics favoring ANKR
        let cfg = AllocatorConfig {
            lookback_days: 60,
            rebalance_days: 7,
            score_function: AllocatorScoreFunction::AnnMinus1Dd,
            max_high_ann_weight: 1.0,
            min_low_dd_weight: 0.0,
            cash_trigger_rolling_dd_pct: None,
            switch_hysteresis_score_gap: 0.0,
            high_ann_sleeve_ids: Vec::new(),
            low_dd_sleeve_ids: Vec::new(),
        };
        let mut metrics = std::collections::HashMap::new();
        metrics.insert(
            "R4".to_string(),
            RollingMetrics {
                rolling_return_quote: 10.0,
                rolling_max_dd_pct: 5.0,
                score: cfg.score_function.evaluate(10.0, 5.0),
            },
        );
        metrics.insert(
            "ANKR".to_string(),
            RollingMetrics {
                rolling_return_quote: 50.0,
                rolling_max_dd_pct: 10.0,
                score: cfg.score_function.evaluate(50.0, 10.0),
            },
        );
        let new_active = state.rebalance(7 * MS_PER_DAY, &cfg, &metrics);
        assert_eq!(new_active, "ANKR", "rebalance should switch to ANKR");
        assert_eq!(state.active_sleeve_id, "ANKR");
        assert_eq!(state.next_rebalance_ms, 7 * MS_PER_DAY + 7 * MS_PER_DAY);

        // After rebalance: only ANKR may open new cycles
        assert!(state.may_open_new_cycle_for("ANKR", 10 * MS_PER_DAY));
        assert!(!state.may_open_new_cycle_for("R4", 10 * MS_PER_DAY));

        // Existing R4 cycles are NOT force-closed by the allocator — the
        // caller is responsible for cycle lifecycle. We only assert that NEW
        // cycles are gated by the active sleeve.
    }
}
