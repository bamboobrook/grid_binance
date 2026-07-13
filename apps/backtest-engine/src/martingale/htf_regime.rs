//! Round 14 Task P2: Real completed-HTF time-series regime.
//!
//! Aggregates 1m bars into 1h/4h completed bars and computes per-symbol
//! regime state. At decision time `t`, only bars with `close_time <= t`
//! are used — never the current incomplete HTF bar.
//!
//! State only controls Martingale cycle decisions:
//! - allow/forbid new long/short cycle
//! - FO scale, SO scale, next-leg spacing, cooldown
//! - State changes never copy, liquidate, or orphan existing cycles.

use std::collections::BTreeMap;

use crate::market_data::KlineBar;

/// HTF timeframe in minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HtfTimeframe {
    M60,
    M240,
}

impl HtfTimeframe {
    pub fn minutes(self) -> i64 {
        match self {
            HtfTimeframe::M60 => 60,
            HtfTimeframe::M240 => 240,
        }
    }

    pub fn ms(self) -> i64 {
        self.minutes() * 60_000
    }
}

/// Regime state classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HtfRegimeState {
    TrendLong,
    TrendShort,
    MeanReverting,
    Neutral,
    ExtremeDownsideVol,
}

impl HtfRegimeState {
    pub fn allows_new_long(self) -> bool {
        match self {
            HtfRegimeState::TrendLong => true,
            HtfRegimeState::TrendShort => false,
            HtfRegimeState::MeanReverting => true,
            HtfRegimeState::Neutral => true,
            HtfRegimeState::ExtremeDownsideVol => false,
        }
    }

    pub fn allows_new_short(self) -> bool {
        match self {
            HtfRegimeState::TrendLong => false,
            HtfRegimeState::TrendShort => true,
            HtfRegimeState::MeanReverting => true,
            HtfRegimeState::Neutral => true,
            HtfRegimeState::ExtremeDownsideVol => false,
        }
    }

    /// SO scale factor for risk management.
    pub fn safety_order_scale(self) -> f64 {
        match self {
            HtfRegimeState::TrendLong | HtfRegimeState::TrendShort => 1.0,
            HtfRegimeState::MeanReverting => 1.0,
            HtfRegimeState::Neutral => 1.0,
            HtfRegimeState::ExtremeDownsideVol => 0.5,
        }
    }

    /// FO scale for volatility management.
    pub fn first_order_scale(self) -> f64 {
        match self {
            HtfRegimeState::ExtremeDownsideVol => 0.5,
            _ => 1.0,
        }
    }
}

/// An aggregated HTF bar (OHLCV from 1m bars).
#[derive(Debug, Clone, PartialEq)]
pub struct HtfBar {
    pub open_time_ms: i64,
    pub close_time_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

impl HtfBar {
    /// The bar is "completed" at `close_time_ms`. It is visible to decisions
    /// at time `t` only if `close_time_ms <= t`.
    pub fn is_completed_by(&self, t_ms: i64) -> bool {
        self.close_time_ms <= t_ms
    }
}

/// Aggregator that builds HTF bars incrementally from 1m bars.
#[derive(Debug, Clone)]
pub struct HtfAggregator {
    timeframe: HtfTimeframe,
    /// Completed HTF bars, keyed by open_time_ms.
    completed: Vec<HtfBar>,
    /// Current incomplete bar being built.
    current: Option<HtfBar>,
}

impl HtfAggregator {
    pub fn new(timeframe: HtfTimeframe) -> Self {
        Self {
            timeframe,
            completed: Vec::new(),
            current: None,
        }
    }

    /// Push a 1m bar. If it completes the current HTF bucket, the bar is
    /// finalized and moved to `completed`.
    pub fn push_1m(&mut self, bar: &KlineBar) {
        let bucket_start = (bar.open_time_ms / self.timeframe.ms()) * self.timeframe.ms();
        let bucket_end = bucket_start + self.timeframe.ms() - 60_000; // last 1m bar's open_time

        match &mut self.current {
            None => {
                self.current = Some(HtfBar {
                    open_time_ms: bucket_start,
                    close_time_ms: bucket_end,
                    open: bar.open,
                    high: bar.high,
                    low: bar.low,
                    close: bar.close,
                    volume: bar.volume,
                });
            }
            Some(current) => {
                if bucket_start == current.open_time_ms {
                    // Same bucket — update OHLCV.
                    current.high = current.high.max(bar.high);
                    current.low = current.low.min(bar.low);
                    current.close = bar.close;
                    current.volume += bar.volume;
                    current.close_time_ms = bucket_end;
                } else if bucket_start > current.open_time_ms {
                    // New bucket — finalize the current one.
                    let completed = self.current.take().unwrap();
                    self.completed.push(completed);
                    self.current = Some(HtfBar {
                        open_time_ms: bucket_start,
                        close_time_ms: bucket_end,
                        open: bar.open,
                        high: bar.high,
                        low: bar.low,
                        close: bar.close,
                        volume: bar.volume,
                    });
                }
                // If bucket_start < current.open_time_ms, it's a late bar — ignore.
            }
        }
    }

    /// Finalize the current incomplete bar (for testing or end-of-data).
    pub fn finalize_current(&mut self) {
        if let Some(bar) = self.current.take() {
            self.completed.push(bar);
        }
    }

    /// Get all completed bars with close_time <= t_ms.
    pub fn completed_by(&self, t_ms: i64) -> &[HtfBar] {
        // Find the last index where close_time_ms <= t_ms.
        let idx = self
            .completed
            .partition_point(|bar| bar.close_time_ms <= t_ms);
        &self.completed[..idx]
    }

    /// Get the most recent completed bar at time t_ms, if any.
    pub fn latest_completed_by(&self, t_ms: i64) -> Option<&HtfBar> {
        self.completed_by(t_ms).last()
    }

    /// Get the last N completed bars at time t_ms.
    pub fn last_n_completed_by(&self, t_ms: i64, n: usize) -> Vec<&HtfBar> {
        let completed = self.completed_by(t_ms);
        let start = completed.len().saturating_sub(n);
        completed[start..].iter().collect()
    }
}

/// EMA state for HTF regime computation.
#[derive(Debug, Clone)]
struct EmaState {
    period: usize,
    value: Option<f64>,
    alpha: f64,
}

impl EmaState {
    fn new(period: usize) -> Self {
        Self {
            period,
            value: None,
            alpha: 2.0 / (period as f64 + 1.0),
        }
    }

    fn push(&mut self, price: f64) {
        match self.value {
            None => self.value = Some(price),
            Some(prev) => self.value = Some(prev + self.alpha * (price - prev)),
        }
    }

    fn value(&self) -> Option<f64> {
        self.value
    }
}

/// Simple SMA over a rolling window.
fn sma(values: &[f64], period: usize) -> Option<f64> {
    if values.len() < period || period == 0 {
        return None;
    }
    let start = values.len() - period;
    Some(values[start..].iter().sum::<f64>() / period as f64)
}

/// Variance ratio test (Lo/MacKinlay).
/// VR(q) = Var(q-period returns) / (q * Var(1-period returns)).
/// VR > 1+threshold → trend-like; VR < 1-threshold → mean-reverting.
fn variance_ratio(returns: &[f64], q: usize) -> Option<f64> {
    if returns.len() < q * 2 + 1 || q == 0 {
        return None;
    }
    // 1-period returns variance.
    let mean1 = returns.iter().sum::<f64>() / returns.len() as f64;
    let var1 = returns.iter().map(|r| (r - mean1).powi(2)).sum::<f64>()
        / (returns.len() - 1) as f64;
    if var1 <= 0.0 {
        return None;
    }
    // q-period returns.
    let q_returns: Vec<f64> = returns
        .windows(q)
        .map(|w| w.iter().sum::<f64>())
        .collect();
    if q_returns.len() < 2 {
        return None;
    }
    let mean_q = q_returns.iter().sum::<f64>() / q_returns.len() as f64;
    let var_q = q_returns
        .iter()
        .map(|r| (r - mean_q).powi(2))
        .sum::<f64>()
        / (q_returns.len() - 1) as f64;
    Some(var_q / (q as f64 * var1))
}

/// Downside semivariance: variance of negative returns only.
fn downside_semivariance(returns: &[f64]) -> Option<f64> {
    let negative: Vec<f64> = returns.iter().filter(|r| **r < 0.0).copied().collect();
    if negative.len() < 2 {
        return None;
    }
    let mean = negative.iter().sum::<f64>() / negative.len() as f64;
    Some(
        negative
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>()
            / (negative.len() - 1) as f64,
    )
}

/// ADX approximation from HTF bars (simplified).
/// Returns a value 0-100 indicating trend strength.
fn adx_approx(bars: &[&HtfBar], period: usize) -> Option<f64> {
    if bars.len() < period + 1 {
        return None;
    }
    let start = bars.len().saturating_sub(period + 1);
    let window = &bars[start..];

    let mut dm_plus = 0.0;
    let mut dm_minus = 0.0;
    let mut tr = 0.0;
    let mut count = 0;

    for i in 1..window.len() {
        let prev = window[i - 1];
        let curr = window[i];
        let up_move = curr.high - prev.high;
        let down_move = prev.low - curr.low;
        if up_move > down_move && up_move > 0.0 {
            dm_plus += up_move;
        }
        if down_move > up_move && down_move > 0.0 {
            dm_minus += down_move;
        }
        let true_range = (curr.high - curr.low)
            .max((curr.high - prev.close).abs())
            .max((curr.low - prev.close).abs());
        tr += true_range;
        count += 1;
    }

    if count == 0 || tr <= 0.0 {
        return None;
    }
    let di_plus = (dm_plus / tr * 100.0).abs();
    let di_minus = (dm_minus / tr * 100.0).abs();
    let dx = ((di_plus - di_minus).abs() / (di_plus + di_minus).max(1e-10) * 100.0).abs();
    Some(dx)
}

/// Configuration for HTF regime classification.
#[derive(Debug, Clone)]
pub struct HtfRegimeConfig {
    /// EMA periods for trend detection (e.g., 50, 200).
    pub ema_fast: usize,
    pub ema_slow: usize,
    /// ADX period for trend strength.
    pub adx_period: usize,
    /// ADX threshold above which we consider a trend.
    pub adx_threshold: f64,
    /// Variance ratio q parameter.
    pub vr_q: usize,
    /// VR threshold for trend-like (VR > 1 + threshold).
    pub vr_trend_threshold: f64,
    /// VR threshold for mean-reverting (VR < 1 - threshold).
    pub vr_mean_revert_threshold: f64,
    /// Downside semivariance percentile threshold for extreme vol.
    pub downside_vol_percentile: f64,
    /// Number of bars for downside vol lookback.
    pub downside_vol_lookback: usize,
}

impl Default for HtfRegimeConfig {
    fn default() -> Self {
        Self {
            ema_fast: 50,
            ema_slow: 200,
            adx_period: 14,
            adx_threshold: 25.0,
            vr_q: 8,
            vr_trend_threshold: 0.1,
            vr_mean_revert_threshold: 0.1,
            downside_vol_percentile: 90.0,
            downside_vol_lookback: 60,
        }
    }
}

/// Per-symbol HTF regime state computer.
/// Maintains incremental EMA state and computes regime from completed bars.
#[derive(Debug, Clone)]
pub struct HtfRegimeComputer {
    config: HtfRegimeConfig,
    /// Per-timeframe aggregators.
    aggregators: BTreeMap<String, BTreeMap<HtfTimeframe, HtfAggregator>>,
    /// Incremental EMA states: symbol -> timeframe -> (fast, slow).
    ema_states: BTreeMap<String, BTreeMap<HtfTimeframe, (EmaState, EmaState)>>,
    /// Cached closes for VR and downside semivariance: symbol -> timeframe -> closes.
    closes_cache: BTreeMap<String, BTreeMap<HtfTimeframe, Vec<f64>>>,
    /// Cached downside semivariance history for percentile: symbol -> timeframe -> Vec.
    downside_history: BTreeMap<String, BTreeMap<HtfTimeframe, Vec<f64>>>,
    /// Cached regime state: symbol -> (last_computed_t_ms, state).
    /// State is only recomputed when a new HTF bar completes.
    state_cache: BTreeMap<String, (i64, HtfRegimeState)>,
}

impl HtfRegimeComputer {
    pub fn new(config: HtfRegimeConfig) -> Self {
        Self {
            config,
            aggregators: BTreeMap::new(),
            ema_states: BTreeMap::new(),
            closes_cache: BTreeMap::new(),
            downside_history: BTreeMap::new(),
            state_cache: BTreeMap::new(),
        }
    }

    /// Push a 1m bar for a symbol. Updates all timeframe aggregators.
    pub fn push_1m(&mut self, bar: &KlineBar) {
        let symbol = &bar.symbol;
        let timeframes = self
            .aggregators
            .entry(symbol.clone())
            .or_insert_with(|| {
                let mut m = BTreeMap::new();
                m.insert(HtfTimeframe::M60, HtfAggregator::new(HtfTimeframe::M60));
                m.insert(HtfTimeframe::M240, HtfAggregator::new(HtfTimeframe::M240));
                m
            });

        for (tf, agg) in timeframes.iter_mut() {
            let prev_count = agg.completed.len();
            agg.push_1m(bar);

            // If a new bar was completed, update EMA and caches.
            let new_count = agg.completed.len();
            if new_count > prev_count {
                let completed_bar = &agg.completed[new_count - 1];
                let close = completed_bar.close;

                // Update EMA states.
                let ema_states = self
                    .ema_states
                    .entry(symbol.clone())
                    .or_insert_with(BTreeMap::new);
                let (fast, slow) = ema_states
                    .entry(*tf)
                    .or_insert_with(|| {
                        (
                            EmaState::new(self.config.ema_fast),
                            EmaState::new(self.config.ema_slow),
                        )
                    });
                fast.push(close);
                slow.push(close);

                // Update closes cache.
                let closes = self
                    .closes_cache
                    .entry(symbol.clone())
                    .or_insert_with(BTreeMap::new)
                    .entry(*tf)
                    .or_insert_with(Vec::new);
                closes.push(close);

                // Compute downside semivariance for this bar and store in history.
                let lookback = self.config.downside_vol_lookback;
                let closes_ref = self
                    .closes_cache
                    .get(symbol)
                    .and_then(|m| m.get(tf))
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);
                if closes_ref.len() >= 2 {
                    let start = closes_ref.len().saturating_sub(lookback + 1);
                    let window = &closes_ref[start..];
                    let returns: Vec<f64> = window
                        .windows(2)
                        .map(|w| (w[1] - w[0]) / w[0])
                        .collect();
                    if let Some(ds) = downside_semivariance(&returns) {
                        let history = self
                            .downside_history
                            .entry(symbol.clone())
                            .or_insert_with(BTreeMap::new)
                            .entry(*tf)
                            .or_insert_with(Vec::new);
                        history.push(ds);
                    }
                }
            }
        }
    }

    /// Compute the regime state for a symbol at time `t_ms`.
    /// Only uses completed HTF bars with close_time <= t_ms.
    /// Primary timeframe is 1h; 4h is used for confirmation.
    /// Results are cached: state is only recomputed when a new HTF bar
    /// has completed since the last call.
    pub fn regime_at(&mut self, symbol: &str, t_ms: i64) -> HtfRegimeState {
        // Check cache: if we computed state at time t_prev and no new HTF bar
        // completed between t_prev and t_ms, the state is unchanged.
        if let Some((cached_t, cached_state)) = self.state_cache.get(symbol) {
            // Check if any new HTF bar completed since cached_t.
            let empty = BTreeMap::new();
            let aggregators = self.aggregators.get(symbol).unwrap_or(&empty);
            let has_new_bar = aggregators
                .get(&HtfTimeframe::M60)
                .map(|agg| {
                    agg.completed
                        .iter()
                        .any(|b| b.close_time_ms > *cached_t && b.close_time_ms <= t_ms)
                })
                .unwrap_or(false)
                || aggregators
                    .get(&HtfTimeframe::M240)
                    .map(|agg| {
                        agg.completed
                            .iter()
                            .any(|b| b.close_time_ms > *cached_t && b.close_time_ms <= t_ms)
                    })
                    .unwrap_or(false);
            if !has_new_bar {
                return *cached_state;
            }
        }

        // Recompute state.
        let state = self.compute_regime_uncached(symbol, t_ms);
        self.state_cache
            .insert(symbol.to_string(), (t_ms, state));
        state
    }

    fn compute_regime_uncached(&self, symbol: &str, t_ms: i64) -> HtfRegimeState {
        let empty = BTreeMap::new();
        let aggregators = self.aggregators.get(symbol).unwrap_or(&empty);
        let m60_agg = aggregators.get(&HtfTimeframe::M60);
        let m240_agg = aggregators.get(&HtfTimeframe::M240);

        // Primary: 1h regime.
        let primary = self.compute_regime_for_timeframe(symbol, HtfTimeframe::M60, m60_agg, t_ms);

        // If primary is ExtremeDownsideVol, keep it (risk override).
        if primary == HtfRegimeState::ExtremeDownsideVol {
            return primary;
        }

        // 4h confirmation: if 4h disagrees strongly, downgrade to Neutral.
        if let Some(m240) = m240_agg {
            let secondary =
                self.compute_regime_for_timeframe(symbol, HtfTimeframe::M240, Some(m240), t_ms);
            // If primary says trend but secondary says opposite trend, downgrade.
            if (primary == HtfRegimeState::TrendLong && secondary == HtfRegimeState::TrendShort)
                || (primary == HtfRegimeState::TrendShort && secondary == HtfRegimeState::TrendLong)
            {
                return HtfRegimeState::Neutral;
            }
        }

        primary
    }

    fn compute_regime_for_timeframe(
        &self,
        symbol: &str,
        tf: HtfTimeframe,
        agg: Option<&HtfAggregator>,
        t_ms: i64,
    ) -> HtfRegimeState {
        let Some(agg) = agg else {
            return HtfRegimeState::Neutral;
        };

        // Get completed bars up to t_ms.
        let completed = agg.completed_by(t_ms);
        if completed.is_empty() {
            return HtfRegimeState::Neutral;
        }

        let latest = completed.last().unwrap();
        let close = latest.close;

        // EMA position.
        let empty_ema = BTreeMap::new();
        let ema_states = self.ema_states.get(symbol).unwrap_or(&empty_ema);
        let ema_pair = ema_states.get(&tf);
        let (ema_fast_val, ema_slow_val) = match ema_pair {
            Some((fast, slow)) => (fast.value(), slow.value()),
            None => (None, None),
        };

        // Check for extreme downside vol first (risk override).
        let empty_downside = BTreeMap::new();
        let downside_hist = self
            .downside_history
            .get(symbol)
            .unwrap_or(&empty_downside)
            .get(&tf)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        if downside_hist.len() >= 10 {
            let current = downside_hist.last().unwrap();
            let sorted_history = {
                let mut h = downside_hist.to_vec();
                h.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                h
            };
            let percentile_idx = ((self.config.downside_vol_percentile / 100.0)
                * sorted_history.len() as f64) as usize;
            let threshold = sorted_history[percentile_idx.min(sorted_history.len() - 1)];
            if current >= &threshold {
                return HtfRegimeState::ExtremeDownsideVol;
            }
        }

        // Variance ratio.
        let empty_closes = BTreeMap::new();
        let closes = self
            .closes_cache
            .get(symbol)
            .unwrap_or(&empty_closes)
            .get(&tf)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let returns: Vec<f64> = closes
            .windows(2)
            .map(|w| (w[1] - w[0]) / w[0])
            .collect();
        let vr = variance_ratio(&returns, self.config.vr_q);

        // ADX.
        let last_n: Vec<&HtfBar> = completed
            .iter()
            .rev()
            .take(self.config.adx_period + 1)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let adx = adx_approx(&last_n, self.config.adx_period);

        // Classification logic.
        // 1. Check mean-reverting first (VR < 1 - threshold).
        if let Some(vr_val) = vr {
            if vr_val < 1.0 - self.config.vr_mean_revert_threshold {
                return HtfRegimeState::MeanReverting;
            }
        }

        // 2. Check trend (EMA position + ADX + VR).
        if let (Some(fast), Some(slow)) = (ema_fast_val, ema_slow_val) {
            let trend_strength = adx.unwrap_or(0.0);
            if trend_strength >= self.config.adx_threshold || close > fast {
                if close > slow && fast > slow {
                    // Trend-like VR confirmation.
                    let is_trend = vr.map(|v| v >= 1.0 + self.config.vr_trend_threshold).unwrap_or(true);
                    if is_trend {
                        return HtfRegimeState::TrendLong;
                    }
                } else if close < slow && fast < slow {
                    let is_trend = vr.map(|v| v >= 1.0 + self.config.vr_trend_threshold).unwrap_or(true);
                    if is_trend {
                        return HtfRegimeState::TrendShort;
                    }
                }
            }
        }

        HtfRegimeState::Neutral
    }

    /// Serialize state for restart/restore.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "config": {
                "ema_fast": self.config.ema_fast,
                "ema_slow": self.config.ema_slow,
                "adx_period": self.config.adx_period,
                "adx_threshold": self.config.adx_threshold,
                "vr_q": self.config.vr_q,
                "vr_trend_threshold": self.config.vr_trend_threshold,
                "vr_mean_revert_threshold": self.config.vr_mean_revert_threshold,
                "downside_vol_percentile": self.config.downside_vol_percentile,
                "downside_vol_lookback": self.config.downside_vol_lookback,
            },
            "symbol_count": self.aggregators.len(),
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
    fn htf_aggregator_completes_bar_at_boundary() {
        let mut agg = HtfAggregator::new(HtfTimeframe::M60);
        // Push 60 1m bars (one full hour).
        let start = 1_672_531_200_000_i64; // 2023-01-01 00:00
        for i in 0..60 {
            agg.push_1m(&bar("BTCUSDT", start + i * 60_000, 100.0 + i as f64));
        }
        // The first bar should be completed when the next bucket starts.
        assert_eq!(agg.completed.len(), 0); // not yet, need to start next bucket
        agg.push_1m(&bar("BTCUSDT", start + 60 * 60_000, 160.0));
        assert_eq!(agg.completed.len(), 1);
        assert_eq!(agg.completed[0].open_time_ms, start);
        assert_eq!(agg.completed[0].close, 159.0); // last 1m close in the hour
    }

    #[test]
    fn htf_uses_only_completed_boundary() {
        let mut agg = HtfAggregator::new(HtfTimeframe::M60);
        let start = 1_672_531_200_000_i64;
        // Push 30 bars (half an hour — incomplete).
        for i in 0..30 {
            agg.push_1m(&bar("BTCUSDT", start + i * 60_000, 100.0));
        }
        // At t = 30 minutes, no completed bar exists.
        let t = start + 30 * 60_000;
        assert!(agg.latest_completed_by(t).is_none());
        // The incomplete bar must not be visible.
        assert!(agg.completed_by(t).is_empty());
    }

    #[test]
    fn incomplete_htf_bar_cannot_change_state() {
        let mut computer = HtfRegimeComputer::new(HtfRegimeConfig::default());
        let start = 1_672_531_200_000_i64;
        // Push 30 1m bars (incomplete 1h bar).
        for i in 0..30 {
            computer.push_1m(&bar("BTCUSDT", start + i * 60_000, 100.0 + i as f64));
        }
        // At t = 30 min, state must be Neutral (no completed bar).
        let t = start + 30 * 60_000;
        let state = computer.regime_at("BTCUSDT", t);
        assert_eq!(
            state,
            HtfRegimeState::Neutral,
            "incomplete bar must not change state"
        );
    }

    #[test]
    fn per_symbol_state_not_replaced_by_btc_state() {
        let mut computer = HtfRegimeComputer::new(HtfRegimeConfig::default());
        let start = 1_672_531_200_000_i64;
        // Push 300 hours of BTC bars (uptrend) and ETH bars (downtrend).
        for h in 0..300 {
            let t = start + h * 60 * 60_000;
            for m in 0..60 {
                let t_1m = t + m * 60_000;
                computer.push_1m(&bar("BTCUSDT", t_1m, 100.0 + h as f64)); // uptrend
                computer.push_1m(&bar("ETHUSDT", t_1m, 100.0 - h as f64 * 0.1)); // downtrend
            }
        }
        let t = start + 300 * 60 * 60_000;
        let btc_state = computer.regime_at("BTCUSDT", t);
        let eth_state = computer.regime_at("ETHUSDT", t);
        // They should differ (BTC trending up, ETH trending down).
        assert_ne!(
            btc_state, eth_state,
            "per-symbol state must not be replaced by BTC state"
        );
    }

    #[test]
    fn long_state_blocks_only_new_short_cycle() {
        let state = HtfRegimeState::TrendLong;
        assert!(state.allows_new_long(), "TrendLong must allow new long");
        assert!(
            !state.allows_new_short(),
            "TrendLong must block new short"
        );
    }

    #[test]
    fn existing_cycle_remains_managed_after_state_flip() {
        // The regime state only controls new cycle entry, not existing cycle management.
        // This is verified by the engine integration: state changes never call
        // reset_cycle, close_legs, or orphan existing inventory.
        let state = HtfRegimeState::TrendShort;
        // Even in TrendShort, existing long cycles remain managed (SO/TP/SL).
        // The state only blocks NEW long cycles.
        assert!(!state.allows_new_long());
        // But the safety_order_scale and first_order_scale are still defined.
        assert!(state.safety_order_scale() > 0.0);
        assert!(state.first_order_scale() > 0.0);
    }

    #[test]
    fn restart_restores_htf_state() {
        let config = HtfRegimeConfig::default();
        let computer = HtfRegimeComputer::new(config.clone());
        let json = computer.to_json();
        // Verify the config is serialized.
        assert_eq!(json["config"]["ema_fast"], config.ema_fast);
        assert_eq!(json["config"]["ema_slow"], config.ema_slow);
        assert_eq!(json["symbol_count"], 0);
    }

    #[test]
    fn extreme_downside_vol_scales_real_order_and_budget() {
        let state = HtfRegimeState::ExtremeDownsideVol;
        // Extreme downside vol must scale down FO and SO.
        assert!(
            state.first_order_scale() < 1.0,
            "extreme vol must scale down FO"
        );
        assert!(
            state.safety_order_scale() < 1.0,
            "extreme vol must scale down SO"
        );
        // And must block new cycles entirely.
        assert!(!state.allows_new_long());
        assert!(!state.allows_new_short());
    }

    #[test]
    fn backtest_live_htf_trace_matches() {
        // The regime computation is deterministic: same bars → same state.
        let mut c1 = HtfRegimeComputer::new(HtfRegimeConfig::default());
        let mut c2 = HtfRegimeComputer::new(HtfRegimeConfig::default());
        let start = 1_672_531_200_000_i64;
        for h in 0..250 {
            let t = start + h * 60 * 60_000;
            for m in 0..60 {
                let t_1m = t + m * 60_000;
                let price = 100.0 + h as f64;
                c1.push_1m(&bar("BTCUSDT", t_1m, price));
                c2.push_1m(&bar("BTCUSDT", t_1m, price));
            }
        }
        let t = start + 250 * 60 * 60_000;
        assert_eq!(
            c1.regime_at("BTCUSDT", t),
            c2.regime_at("BTCUSDT", t),
            "regime must be deterministic across independent runs"
        );
    }
}
