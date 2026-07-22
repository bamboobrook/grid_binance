//! Cross-sectional momentum alpha signal (plan §3 — multi-factor ensemble).
//!
//! At each bar, ranks the N symbols by their recent-window return. Long the
//! top-ranked (winners), short the bottom-ranked (losers). This is an independent
//! alpha source from M1 (OI/crowding). Combined with M1, it may raise the
//! portfolio Sharpe via diversification of alpha streams.
//!
//! The signal produces a per-symbol direction (+1/0/-1) and strength (z-score
//! of the cross-sectional rank).

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

/// XS-momentum state at a timestamp: per-symbol direction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XsMomentumState {
    pub ts_ms: i64,
    /// symbol → (+1 long winners, -1 short losers, 0 neutral)
    pub directions: BTreeMap<String, i8>,
}

/// Compute XS-momentum directions from per-symbol close series.
/// `closes_by_sym`: symbol → Vec<(ts_ms, close)>, all same length/time-aligned.
/// `lookback_bars`: the momentum lookback window.
/// `long_frac`: fraction of symbols to long (top rank); same fraction shorted.
pub fn compute_xs_momentum(
    closes_by_sym: &BTreeMap<String, Vec<(i64, f64)>>,
    lookback_bars: usize,
    long_frac: f64,
) -> Vec<XsMomentumState> {
    let symbols: Vec<&String> = closes_by_sym.keys().collect();
    if symbols.is_empty() { return vec![]; }
    let n = symbols.len();
    let n_long = ((n as f64) * long_frac).round() as usize;
    let n_short = n_long;
    // Collect all timestamps from the first symbol (assume aligned).
    let ts_list: Vec<i64> = closes_by_sym[symbols[0]].iter().map(|(t, _)| *t).collect();
    let mut states = Vec::new();
    for (idx, &ts) in ts_list.iter().enumerate() {
        if idx < lookback_bars { continue; }
        // compute momentum = return over lookback for each symbol
        let mut mom: Vec<(String, f64)> = Vec::new();
        for sym in &symbols {
            let series = &closes_by_sym[*sym];
            if idx >= series.len() { continue; }
            let cur = series[idx].1;
            let past = series[idx - lookback_bars].1;
            if cur > 0.0 && past > 0.0 {
                mom.push((sym.to_string(), cur / past - 1.0));
            }
        }
        if mom.len() < n { continue; }
        // sort by momentum descending
        mom.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut dirs: BTreeMap<String, i8> = BTreeMap::new();
        for (i, (sym, _)) in mom.iter().enumerate() {
            if i < n_long {
                dirs.insert(sym.clone(), 1);
            } else if i >= mom.len() - n_short {
                dirs.insert(sym.clone(), -1);
            } else {
                dirs.insert(sym.clone(), 0);
            }
        }
        states.push(XsMomentumState { ts_ms: ts, directions: dirs });
    }
    states
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xs_momentum_assigns_directions() {
        let mut closes: BTreeMap<String, Vec<(i64, f64)>> = BTreeMap::new();
        // 4 symbols, 10 bars; A rises most, D falls most
        for (sym, drift) in &[("A", 0.01), ("B", 0.005), ("C", -0.005), ("D", -0.01)] {
            let series: Vec<(i64, f64)> = (0..10).map(|i| (i * 300_000, 100.0 * (1.0 + drift * i as f64))).collect();
            closes.insert(sym.to_string(), series);
        }
        let states = compute_xs_momentum(&closes, 5, 0.25);
        assert!(!states.is_empty());
        let last = states.last().unwrap();
        assert_eq!(last.directions["A"], 1); // top winner → long
        assert_eq!(last.directions["D"], -1); // bottom loser → short
    }
}
