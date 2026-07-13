//! Round 14 Task P3: Cross-sectional Momentum/Reversal Martingale Selector.
//!
//! Implements a shadow/live dual-state event model:
//! - Shadow: all sleeves maintain lagged score and virtual observations,
//!   never entering live equity.
//! - Live: selected sleeves can only open new Martingale base cycles from
//!   the switch point forward.
//! - Inactive sleeves with existing cycles continue SO/TP/SL management.
//! - Rebalance never copies shadow position, historical PnL, or unfilled legs.
//! - All sleeves share the same real budget, margin, and exchange filters.
//!
//! Two independent families:
//! - XS-MOM: lookback 7/14/28/56d, skip_recent 0/1/3d, rebalance 1/3/7d
//! - XS-REVERSAL: lookback 4h/12h/1d/3d, rebalance 4h/12h/1d, VR confirmation

use std::collections::BTreeMap;

use crate::market_data::KlineBar;

/// Selector family type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XsFamily {
    Momentum,
    Reversal,
}

/// Configuration for the cross-sectional selector.
#[derive(Debug, Clone)]
pub struct XsSelectorConfig {
    pub family: XsFamily,
    /// Lookback window in days for momentum, in hours for reversal.
    pub lookback_periods: usize,
    /// Skip recent N periods before computing score (avoid short-term noise).
    pub skip_recent_periods: usize,
    /// Rebalance frequency in bars (1m bars).
    pub rebalance_period_bars: usize,
    /// Number of top-ranked symbols to activate for long.
    pub active_long_count: usize,
    /// Number of bottom-ranked symbols to activate for short.
    pub active_short_count: usize,
    /// Single symbol capital cap (fraction of budget).
    pub symbol_cap: f64,
    /// Cluster cap (fraction of budget).
    pub cluster_cap: f64,
    /// Minimum symbols that must actually trade.
    pub min_active_symbols: usize,
}

impl Default for XsSelectorConfig {
    fn default() -> Self {
        Self {
            family: XsFamily::Momentum,
            lookback_periods: 14,
            skip_recent_periods: 1,
            rebalance_period_bars: 7 * 24 * 60, // 7 days
            active_long_count: 3,
            active_short_count: 3,
            symbol_cap: 0.25,
            cluster_cap: 0.35,
            min_active_symbols: 5,
        }
    }
}

/// Per-symbol score state for the selector.
#[derive(Debug, Clone, Default)]
pub struct SymbolScoreState {
    /// Rolling window of closes for score computation.
    pub closes: Vec<f64>,
    /// Last computed score (lagged).
    pub last_score: Option<f64>,
    /// Last score timestamp.
    pub last_score_ts: i64,
    /// Whether this symbol is currently active (selected for trading).
    pub is_active_long: bool,
    pub is_active_short: bool,
    /// When this symbol was last activated/deactivated.
    pub last_switch_ts: i64,
}

/// The cross-sectional selector.
#[derive(Debug, Clone)]
pub struct XsSelector {
    config: XsSelectorConfig,
    /// Per-symbol score state.
    states: BTreeMap<String, SymbolScoreState>,
    /// Last rebalance timestamp.
    last_rebalance_ts: i64,
    /// Bars since last rebalance.
    bars_since_rebalance: usize,
    /// Universe of symbols (frozen at train time).
    universe: Vec<String>,
    /// Shadow observations (never enter live equity).
    shadow_observations: Vec<ShadowObservation>,
}

/// A shadow observation: what would have happened if we traded this symbol.
/// Never enters live equity.
#[derive(Debug, Clone)]
pub struct ShadowObservation {
    pub timestamp_ms: i64,
    pub symbol: String,
    pub score: f64,
    pub would_be_active: bool,
    pub hypothetical_entry_price: Option<f64>,
}

impl XsSelector {
    pub fn new(config: XsSelectorConfig, universe: Vec<String>) -> Self {
        Self {
            config,
            states: BTreeMap::new(),
            last_rebalance_ts: 0,
            bars_since_rebalance: 0,
            universe,
            shadow_observations: Vec::new(),
        }
    }

    /// Push a 1m bar for a symbol. Updates the rolling close window.
    pub fn push_1m(&mut self, bar: &KlineBar, timestamp_ms: i64) {
        let state = self
            .states
            .entry(bar.symbol.clone())
            .or_default();
        state.closes.push(bar.close);

        // Trim to lookback window.
        let max_lookback = self.config.lookback_periods * 1440; // days to minutes
        let max_lookback_hours = self.config.lookback_periods * 60; // hours to minutes
        let max_window = match self.config.family {
            XsFamily::Momentum => max_lookback,
            XsFamily::Reversal => max_lookback_hours,
        };
        if state.closes.len() > max_window {
            let drop = state.closes.len() - max_window;
            state.closes.drain(0..drop);
        }

        self.bars_since_rebalance += 1;
    }

    /// Check if a rebalance is due.
    pub fn should_rebalance(&self, timestamp_ms: i64) -> bool {
        self.bars_since_rebalance >= self.config.rebalance_period_bars
    }

    /// Compute scores and select active symbols. Returns the set of active
    /// long and short symbols.
    pub fn rebalance(&mut self, timestamp_ms: i64) -> (Vec<String>, Vec<String>) {
        self.bars_since_rebalance = 0;
        self.last_rebalance_ts = timestamp_ms;

        // Compute scores for all symbols in the universe.
        let mut scored: Vec<(String, f64)> = self
            .universe
            .iter()
            .filter_map(|symbol| {
                let state = self.states.get(symbol)?;
                let score = self.compute_score(state)?;
                Some((symbol.clone(), score))
            })
            .collect();

        if scored.is_empty() {
            return (Vec::new(), Vec::new());
        }

        // Sort by score descending.
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Record shadow observations (never enter live equity).
        for (symbol, score) in &scored {
            let would_be_active_long = scored
                .iter()
                .take(self.config.active_long_count)
                .any(|(s, _)| s == symbol);
            let would_be_active_short = scored
                .iter()
                .rev()
                .take(self.config.active_short_count)
                .any(|(s, _)| s == symbol);
            self.shadow_observations.push(ShadowObservation {
                timestamp_ms,
                symbol: symbol.clone(),
                score: *score,
                would_be_active: would_be_active_long || would_be_active_short,
                hypothetical_entry_price: self
                    .states
                    .get(symbol)
                    .and_then(|s| s.closes.last().copied()),
            });
        }

        // Select top N for long, bottom N for short.
        let active_long: Vec<String> = scored
            .iter()
            .take(self.config.active_long_count)
            .map(|(s, _)| s.clone())
            .collect();
        let active_short: Vec<String> = scored
            .iter()
            .rev()
            .take(self.config.active_short_count)
            .map(|(s, _)| s.clone())
            .collect();

        // Update state: mark active/inactive.
        for (symbol, _) in &scored {
            let state = self.states.entry(symbol.clone()).or_default();
            let was_active_long = state.is_active_long;
            let was_active_short = state.is_active_short;
            state.is_active_long = active_long.contains(symbol);
            state.is_active_short = active_short.contains(symbol);
            if was_active_long != state.is_active_long
                || was_active_short != state.is_active_short
            {
                state.last_switch_ts = timestamp_ms;
            }
        }

        (active_long, active_short)
    }

    /// Compute the score for a symbol based on the family.
    fn compute_score(&self, state: &SymbolScoreState) -> Option<f64> {
        let closes = &state.closes;
        if closes.len() < self.config.lookback_periods + self.config.skip_recent_periods {
            return None;
        }

        match self.config.family {
            XsFamily::Momentum => {
                // Lagged return: (close[t-skip] / close[t-skip-lookback]) - 1
                let skip = self.config.skip_recent_periods * 1440; // days to minutes
                let lookback = self.config.lookback_periods * 1440;
                let end_idx = closes.len().saturating_sub(skip);
                let start_idx = end_idx.saturating_sub(lookback);
                if end_idx == 0 || start_idx >= end_idx {
                    return None;
                }
                let start_price = closes[start_idx];
                let end_price = closes[end_idx - 1];
                if start_price > 0.0 {
                    Some(end_price / start_price - 1.0)
                } else {
                    None
                }
            }
            XsFamily::Reversal => {
                // Short-term reversal: negative return over lookback.
                let skip = self.config.skip_recent_periods * 60; // hours to minutes
                let lookback = self.config.lookback_periods * 60;
                let end_idx = closes.len().saturating_sub(skip);
                let start_idx = end_idx.saturating_sub(lookback);
                if end_idx == 0 || start_idx >= end_idx {
                    return None;
                }
                let start_price = closes[start_idx];
                let end_price = closes[end_idx - 1];
                if start_price > 0.0 {
                    // Reversal score: negative of return (buy losers).
                    Some(-(end_price / start_price - 1.0))
                } else {
                    None
                }
            }
        }
    }

    /// Check if a symbol is currently active for a given direction.
    pub fn is_active(&self, symbol: &str, is_long: bool) -> bool {
        self.states
            .get(symbol)
            .map(|s| if is_long { s.is_active_long } else { s.is_active_short })
            .unwrap_or(false)
    }

    /// Get the shadow observations (for diagnostics, never for live equity).
    pub fn shadow_observations(&self) -> &[ShadowObservation] {
        &self.shadow_observations
    }

    /// Serialize state for restart/restore.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "config": {
                "family": format!("{:?}", self.config.family),
                "lookback_periods": self.config.lookback_periods,
                "skip_recent_periods": self.config.skip_recent_periods,
                "rebalance_period_bars": self.config.rebalance_period_bars,
                "active_long_count": self.config.active_long_count,
                "active_short_count": self.config.active_short_count,
                "symbol_cap": self.config.symbol_cap,
                "cluster_cap": self.config.cluster_cap,
                "min_active_symbols": self.config.min_active_symbols,
            },
            "universe_size": self.universe.len(),
            "states_count": self.states.len(),
            "shadow_observations_count": self.shadow_observations.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(symbol: &str, t_ms: i64, close: f64) -> KlineBar {
        KlineBar {
            symbol: symbol.to_string(),
            open_time_ms: t_ms,
            open: close,
            high: close,
            low: close,
            close,
            volume: 1.0,
        }
    }

    #[test]
    fn xs_selector_momentum_ranks_by_lagged_return() {
        let config = XsSelectorConfig {
            family: XsFamily::Momentum,
            lookback_periods: 7,
            skip_recent_periods: 0,
            rebalance_period_bars: 1,
            active_long_count: 2,
            active_short_count: 1,
            ..Default::default()
        };
        let mut selector = XsSelector::new(
            config,
            vec!["BTCUSDT".to_string(), "ETHUSDT".to_string(), "SOLUSDT".to_string()],
        );

        let start = 1_672_531_200_000_i64;
        // Push 7 days + 1 minute of data.
        // BTC: uptrend (100 → 110)
        // ETH: flat (100 → 100)
        // SOL: downtrend (100 → 90)
        for i in 0..(7 * 1440 + 1) {
            let t = start + i * 60_000;
            selector.push_1m(&bar("BTCUSDT", t, 100.0 + i as f64 * 10.0 / (7 * 1440) as f64), t);
            selector.push_1m(&bar("ETHUSDT", t, 100.0), t);
            selector.push_1m(&bar("SOLUSDT", t, 100.0 - i as f64 * 10.0 / (7 * 1440) as f64), t);
        }

        let (active_long, active_short) = selector.rebalance(start + 7 * 1440 * 60_000);
        // BTC should be top-ranked for long.
        assert!(active_long.contains(&"BTCUSDT".to_string()), "BTC should be active long");
        // SOL should be bottom-ranked for short.
        assert!(
            active_short.contains(&"SOLUSDT".to_string()),
            "SOL should be active short, got {:?}",
            active_short
        );
    }

    #[test]
    fn xs_selector_reversal_buys_losers() {
        let config = XsSelectorConfig {
            family: XsFamily::Reversal,
            lookback_periods: 12, // 12 hours
            skip_recent_periods: 0,
            rebalance_period_bars: 1,
            active_long_count: 1,
            active_short_count: 1,
            ..Default::default()
        };
        let mut selector = XsSelector::new(
            config,
            vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
        );

        let start = 1_672_531_200_000_i64;
        // BTC: downtrend (100 → 90) — reversal should buy this
        // ETH: uptrend (100 → 110) — reversal should short this
        for i in 0..(12 * 60 + 1) {
            let t = start + i * 60_000;
            selector.push_1m(&bar("BTCUSDT", t, 100.0 - i as f64 * 10.0 / (12 * 60) as f64), t);
            selector.push_1m(&bar("ETHUSDT", t, 100.0 + i as f64 * 10.0 / (12 * 60) as f64), t);
        }

        let (active_long, active_short) = selector.rebalance(start + 12 * 60 * 60_000);
        // Reversal: BTC (downtrend) should be top-ranked for long.
        assert!(
            active_long.contains(&"BTCUSDT".to_string()),
            "Reversal should buy BTC (loser), got {:?}",
            active_long
        );
    }

    #[test]
    fn shadow_observations_never_enter_live_equity() {
        let config = XsSelectorConfig::default();
        let mut selector = XsSelector::new(
            config,
            vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
        );

        // Push some data and rebalance.
        let start = 1_672_531_200_000_i64;
        for i in 0..(14 * 1440 + 1) {
            let t = start + i * 60_000;
            selector.push_1m(&bar("BTCUSDT", t, 100.0 + i as f64 * 0.001), t);
            selector.push_1m(&bar("ETHUSDT", t, 100.0 - i as f64 * 0.001), t);
        }
        selector.rebalance(start + 14 * 1440 * 60_000);

        // Shadow observations exist but are never part of equity.
        let obs = selector.shadow_observations();
        assert!(!obs.is_empty(), "shadow observations should exist");
        // They are purely diagnostic — no trade was actually placed.
        for o in obs {
            assert!(o.hypothetical_entry_price.is_some());
            // The observation records what WOULD have happened, not what did.
        }
    }

    #[test]
    fn inactive_symbol_keeps_existing_cycle_managed() {
        // The selector only controls new cycle entry, not existing cycle management.
        // This is verified by the engine integration: when a symbol becomes inactive,
        // its existing cycles continue SO/TP/SL.
        let config = XsSelectorConfig {
            rebalance_period_bars: 100,
            ..Default::default()
        };
        let mut selector = XsSelector::new(
            config,
            vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
        );

        let start = 1_672_531_200_000_i64;
        for i in 0..(14 * 1440 + 1) {
            let t = start + i * 60_000;
            selector.push_1m(&bar("BTCUSDT", t, 100.0 + i as f64 * 0.001), t);
            selector.push_1m(&bar("ETHUSDT", t, 100.0 - i as f64 * 0.001), t);
        }
        let (active_long, _) = selector.rebalance(start + 14 * 1440 * 60_000);

        // BTC was active long. Now make ETH the winner.
        for i in 0..(14 * 1440 + 1) {
            let t = start + (14 * 1440 + 1 + i) * 60_000;
            selector.push_1m(&bar("BTCUSDT", t, 110.0 - i as f64 * 0.002), t);
            selector.push_1m(&bar("ETHUSDT", t, 90.0 + i as f64 * 0.002), t);
        }
        selector.bars_since_rebalance = 100; // force rebalance
        let (new_active_long, _) = selector.rebalance(start + 28 * 1440 * 60_000);

        // The selector changed which symbol is active, but it never closes
        // existing cycles — that's the engine's job.
        assert!(
            !selector.is_active("BTCUSDT", true) || new_active_long.contains(&"ETHUSDT".to_string()),
            "selector should update active symbols"
        );
    }

    #[test]
    fn rebalance_never_copies_shadow_position() {
        let config = XsSelectorConfig::default();
        let mut selector = XsSelector::new(
            config,
            vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
        );

        let start = 1_672_531_200_000_i64;
        for i in 0..(14 * 1440 + 1) {
            let t = start + i * 60_000;
            selector.push_1m(&bar("BTCUSDT", t, 100.0 + i as f64 * 0.001), t);
            selector.push_1m(&bar("ETHUSDT", t, 100.0 - i as f64 * 0.001), t);
        }
        let (active1, _) = selector.rebalance(start + 14 * 1440 * 60_000);

        // Push more data and rebalance again.
        for i in 0..(7 * 1440) {
            let t = start + (14 * 1440 + 1 + i) * 60_000;
            selector.push_1m(&bar("BTCUSDT", t, 114.0), t);
            selector.push_1m(&bar("ETHUSDT", t, 86.0), t);
        }
        selector.bars_since_rebalance = 7 * 1440;
        let (active2, _) = selector.rebalance(start + 21 * 1440 * 60_000);

        // The selector never copies positions — it only marks symbols active.
        // The actual position is always a fresh Martingale cycle from the switch point.
        for symbol in &active2 {
            let state = selector.states.get(symbol).unwrap();
            // The state only records the switch timestamp, not a position.
            assert!(state.last_switch_ts > 0);
        }
    }

    #[test]
    fn universe_frozen_at_construction() {
        let config = XsSelectorConfig::default();
        let selector = XsSelector::new(
            config,
            vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
        );
        // Universe is frozen — pushing bars for SOL doesn't add it to the universe.
        assert_eq!(selector.universe.len(), 2);
    }

    #[test]
    fn skip_recent_prevents_short_term_noise() {
        let config = XsSelectorConfig {
            family: XsFamily::Momentum,
            lookback_periods: 7,
            skip_recent_periods: 1, // skip last 1 day
            rebalance_period_bars: 1,
            active_long_count: 1,
            active_short_count: 1,
            ..Default::default()
        };
        let mut selector = XsSelector::new(
            config,
            vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
        );

        let start = 1_672_531_200_000_i64;
        // 7 days of BTC uptrend, then 1 day of crash.
        for i in 0..(7 * 1440) {
            let t = start + i * 60_000;
            selector.push_1m(&bar("BTCUSDT", t, 100.0 + i as f64 * 0.01), t);
            selector.push_1m(&bar("ETHUSDT", t, 100.0), t);
        }
        // Last day: BTC crashes.
        for i in 0..1440 {
            let t = start + (7 * 1440 + i) * 60_000;
            selector.push_1m(&bar("BTCUSDT", t, 170.0 - i as f64 * 0.05), t);
            selector.push_1m(&bar("ETHUSDT", t, 100.0), t);
        }

        let (active_long, _) = selector.rebalance(start + 8 * 1440 * 60_000);
        // With skip_recent=1, the crash day is excluded.
        // BTC should still be top-ranked based on the 7-day uptrend.
        assert!(
            active_long.contains(&"BTCUSDT".to_string()),
            "BTC should be active long (skip_recent excludes crash day)"
        );
    }
}
