//! R4-M1 signal layer: OI / Crowding Exhaustion state (plan §8 M1).
//!
//! Computes the fit-frozen rolling robust state used to gate a single-symbol
//! Martin's FO/SO/TP/abort. This is the SIGNAL layer; it produces boolean
//! activation flags + a state hash. The engine integration (wiring these flags
//! into the FO/SO decision path) is the remaining M1 work.
//!
//! M1 state (plan §8 M1, only completed 5m snapshots):
//!   oi_change  = delta(log(sum_open_interest_value))
//!   taker_flow = log(sum_taker_long_short_vol_ratio)
//!   top_crowd  = log(sum_toptrader_long_short_ratio)
//!   all_crowd  = log(count_long_short_ratio)
//!   price_ext  = (log_price - rolling_median(log_price)) scaled by MAD
//!
//! Long FO must simultaneously satisfy:
//!   1. price downside extension
//!   2. OI expansion
//!   3. taker sell imbalance extreme
//!   4. top/all crowd leaning short
//!   5. taker sell pressure decelerating (not still accelerating)
//! Short FO is the mirror.

use serde::{Deserialize, Serialize};

/// One row of parsed metrics at a 5-minute snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricRow {
    pub ts_ms: i64,
    pub symbol: String,
    pub open_interest_value: f64,
    pub top_trader_ls_ratio: f64,
    pub all_trader_ls_ratio: f64,
    pub taker_ls_vol_ratio: f64,
}

/// The robust M1 state at a timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct M1State {
    pub ts_ms: i64,
    pub oi_change: f64,
    pub taker_flow: f64,
    pub top_crowd: f64,
    pub all_crowd: f64,
    pub price_ext: f64,
}

/// Activation flags produced by the M1 logic at a timestamp.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct M1Activation {
    pub long_fo: bool,
    pub short_fo: bool,
    pub long_so_exhaustion: bool,  // SO allowed only when exhaustion appears
    pub short_so_exhaustion: bool,
}

/// Compute the M1 state series from metrics + aligned 5m closes.
///
/// `window` is the rolling window size (in snapshots) for the robust median/MAD.
/// All z/quantiles use ONLY past data (anti-overfit).
pub fn compute_m1_states(
    metrics: &[MetricRow],
    closes_5m: &[(i64, f64)], // (ts_ms, close) aligned to 5m
    window: usize,
) -> Vec<M1State> {
    // merge metrics and closes by timestamp
    use std::collections::BTreeMap;
    let close_map: BTreeMap<i64, f64> = closes_5m.iter().cloned().collect();
    let metric_map: BTreeMap<i64, &MetricRow> = metrics.iter().map(|m| (m.ts_ms, m)).collect();
    let mut states = Vec::new();
    let mut log_oi_hist: Vec<f64> = Vec::new(); // rolling log OI value
    let mut log_price_hist: Vec<f64> = Vec::new();
    for (ts, close) in closes_5m {
        if *close <= 0.0 { continue; }
        let lp = close.ln();
        log_price_hist.push(lp);
        if log_price_hist.len() > window {
            log_price_hist.remove(0);
        }
        let m = match metric_map.get(ts) {
            Some(m) => *m,
            None => continue,
        };
        let oi_val = m.open_interest_value;
        if oi_val <= 0.0 { continue; }
        let log_oi = oi_val.ln();
        // oi_change = delta(log OI)
        let oi_change = log_oi_hist.last().map(|&prev| log_oi - prev).unwrap_or(0.0);
        log_oi_hist.push(log_oi);
        if log_oi_hist.len() > window {
            log_oi_hist.remove(0);
        }
        // robust price extension: (lp - rolling_median) / rolling_mad
        let (med, mad) = robust_median_mad(&log_price_hist);
        let price_ext = if mad > 1e-12 { (lp - med) / mad } else { 0.0 };
        // taker_flow, crowds (guarded logs)
        let taker_flow = safe_log(m.taker_ls_vol_ratio);
        let top_crowd = safe_log(m.top_trader_ls_ratio);
        let all_crowd = safe_log(m.all_trader_ls_ratio);
        states.push(M1State {
            ts_ms: *ts,
            oi_change,
            taker_flow,
            top_crowd,
            all_crowd,
            price_ext,
        });
    }
    states
}

/// Evaluate the M1 activation flags at each state using past-window quantiles.
///
/// `tail_q` is the tail quantile (e.g. 0.95). All thresholds are computed from
/// the past `window` states only (anti-overfit / causal).
pub fn evaluate_activations(states: &[M1State], window: usize, tail_q: f64) -> Vec<M1Activation> {
    let mut out = Vec::with_capacity(states.len());
    for i in 0..states.len() {
        let start = if i > window { i - window } else { 0 };
        let past = &states[start..i];
        let act = if past.len() < 30 {
            M1Activation::default()
        } else {
            let q_price_down = -quantile(&past.iter().map(|s| s.price_ext).collect::<Vec<_>>(), tail_q);
            let q_oi_up = quantile(&past.iter().map(|s| s.oi_change).collect::<Vec<_>>(), tail_q);
            let q_taker_sell = -quantile(&past.iter().map(|s| s.taker_flow).collect::<Vec<_>>(), tail_q);
            let q_taker_buy = quantile(&past.iter().map(|s| s.taker_flow).collect::<Vec<_>>(), tail_q);
            let q_top_short = -quantile(&past.iter().map(|s| s.top_crowd).collect::<Vec<_>>(), tail_q);
            let q_top_long = quantile(&past.iter().map(|s| s.top_crowd).collect::<Vec<_>>(), tail_q);
            // deceleration: taker_flow rose in the prior window then fell in the last few
            let recent: Vec<f64> = past.iter().rev().take(6).map(|s| s.taker_flow).collect();
            let taker_decel_sell = recent.first().copied().unwrap_or(0.0) > recent.last().copied().unwrap_or(0.0);
            let taker_decel_buy = recent.first().copied().unwrap_or(0.0) < recent.last().copied().unwrap_or(0.0);
            let s = &states[i];
            let long_fo = s.price_ext <= q_price_down
                && s.oi_change >= q_oi_up
                && s.taker_flow <= q_taker_sell
                && s.top_crowd <= q_top_short
                && taker_decel_sell;
            let short_fo = s.price_ext >= -q_price_down
                && s.oi_change >= q_oi_up
                && s.taker_flow >= q_taker_buy
                && s.top_crowd >= q_top_long
                && taker_decel_buy;
            M1Activation {
                long_fo,
                short_fo,
                long_so_exhaustion: taker_decel_sell,
                short_so_exhaustion: taker_decel_buy,
            }
        };
        out.push(act);
    }
    out
}

fn robust_median_mad(xs: &[f64]) -> (f64, f64) {
    if xs.is_empty() { return (0.0, 0.0); }
    let mut s = xs.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let med = s[s.len() / 2];
    let mut dev: Vec<f64> = xs.iter().map(|x| (x - med).abs()).collect();
    dev.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mad = dev[dev.len() / 2];
    (med, mad)
}

fn quantile(xs: &[f64], q: f64) -> f64 {
    if xs.is_empty() { return 0.0; }
    let mut s = xs.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((s.len() as f64 - 1.0) * q).round() as usize;
    s[idx.min(s.len() - 1)]
}

fn safe_log(x: f64) -> f64 {
    if x > 0.0 { x.ln() } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_states_handles_empty() {
        let s = compute_m1_states(&[], &[], 100);
        assert!(s.is_empty());
    }

    #[test]
    fn evaluate_activations_causal_no_activation_with_short_history() {
        let states: Vec<M1State> = (0..20)
            .map(|i| M1State {
                ts_ms: i * 300_000,
                oi_change: 0.0,
                taker_flow: 0.0,
                top_crowd: 0.0,
                all_crowd: 0.0,
                price_ext: 0.0,
            })
            .collect();
        let acts = evaluate_activations(&states, 100, 0.95);
        // < 30 past -> all default (no activation). Anti-overfit.
        assert!(acts.iter().all(|a| !a.long_fo && !a.short_fo));
    }
}
