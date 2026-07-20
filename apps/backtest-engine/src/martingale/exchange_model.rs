//! Round 20 P2: production-conservative Binance exchange model.
//!
//! Implements the exchange-side realism the plan §1.3 requires from G0 onward:
//! PRICE_FILTER / LOT_SIZE / MARKET_LOT_SIZE / MIN_NOTIONAL/NOTIONAL rounding,
//! maintenance-margin tiers, conservative liquidation, and the quantity/price
//! rounding that turns an ideal notional into a real exchange order.
//!
//! This module is deliberately separate from `sync_cycle_engine.rs` so the
//! conservative path is testable in isolation and the ideal-atomic path remains
//! available only as an upper-bound control (plan §4 P2 last paragraph).

use serde::{Deserialize, Serialize};

/// Per-symbol Binance filter snapshot (plan §3.2). Values reflect current
/// Binance exchangeInfo for USD-M perps; a more conservative stress pair is
/// applied alongside in `stress_filters()`. Historical rules are unavailable,
/// so we never assume a zero threshold.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExchangeFilters {
    pub symbol: String,
    /// PRICE_FILTER: tick_size. Prices round to the nearest multiple.
    pub tick_size: f64,
    /// LOT_SIZE / MARKET_LOT_SIZE: step_size. Quantities round down to a multiple.
    pub step_size: f64,
    /// LOT_SIZE min quantity.
    pub min_qty: f64,
    /// MIN_NOTIONAL / NOTIONAL: min notional in quote USDT.
    pub min_notional: f64,
    /// maintenance-margin fraction at the current tier (e.g. 0.0065 = 0.65%).
    pub maint_margin_rate: f64,
    /// maintenance amount (quote USDT) for the current tier.
    pub maint_amount: f64,
}

impl ExchangeFilters {
    /// Conservative defaults for a typical USD-M perp (BTCUSDT-like).
    /// These are deliberately NOT zero — the plan forbids zero-threshold
    /// assumptions (§3.2).
    pub fn default_for(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            tick_size: 0.10,
            step_size: 0.001,
            min_qty: 0.001,
            min_notional: 5.0,
            maint_margin_rate: 0.005,
            maint_amount: 0.0,
        }
    }

    /// A more conservative stress pair (tighter min_notional, larger tick/step,
    /// higher maintenance). Used alongside the default to bound exchange-rule
    /// uncertainty (plan §3.2 "current rules + more conservative stress").
    pub fn stress_for(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            tick_size: 0.50,
            step_size: 0.005,
            min_qty: 0.005,
            min_notional: 10.0,
            maint_margin_rate: 0.01,
            maint_amount: 0.0,
        }
    }

    /// Round a price to the nearest valid tick (PRICE_FILTER).
    pub fn round_price(&self, price: f64) -> f64 {
        if self.tick_size <= 0.0 {
            return price;
        }
        (price / self.tick_size).round() * self.tick_size
    }

    /// Round a quantity DOWN to the nearest valid step (LOT_SIZE). Rounded down
    /// so the order never exceeds the intended notional.
    pub fn round_qty_down(&self, qty: f64) -> f64 {
        if self.step_size <= 0.0 {
            return qty;
        }
        (qty / self.step_size).floor() * self.step_size
    }

    /// Apply all four filters to an intended (price, notional_quote) order.
    /// Returns Some((rounded_price, rounded_qty, rounded_notional)) if the
    /// order passes all filters, else None with a reason.
    pub fn filter_order(
        &self,
        price: f64,
        notional_quote: f64,
    ) -> Result<(f64, f64, f64), FilterReject> {
        if price <= 0.0 || notional_quote <= 0.0 {
            return Err(FilterReject::InvalidInput);
        }
        let rounded_price = self.round_price(price);
        if rounded_price <= 0.0 {
            return Err(FilterReject::PriceFilter);
        }
        let raw_qty = notional_quote / rounded_price;
        let rounded_qty = self.round_qty_down(raw_qty);
        if rounded_qty < self.min_qty {
            return Err(FilterReject::LotSizeMinQty);
        }
        let rounded_notional = rounded_qty * rounded_price;
        if rounded_notional < self.min_notional {
            return Err(FilterReject::MinNotional);
        }
        Ok((rounded_price, rounded_qty, rounded_notional))
    }

    /// Maintenance margin required for a position of `notional_quote` at the
    /// current tier: max(maint_amount, notional * maint_margin_rate).
    pub fn maintenance_margin(&self, notional_quote: f64) -> f64 {
        (self.maint_amount).max(notional_quote * self.maint_margin_rate)
    }

    /// Conservative liquidation check: a position is liquidated if the
    /// available margin (account equity - maintenance margin of all positions)
    /// falls below zero. Returns true if this position's loss would trigger
    /// liquidation given `position_notional`, `unrealized_loss_quote`, and
    /// `available_margin_quote` (equity minus other positions' maintenance).
    pub fn liquidation_triggered(
        &self,
        position_notional: f64,
        unrealized_loss_quote: f64,
        available_margin_quote: f64,
    ) -> bool {
        // After the loss, available margin must still cover this position's
        // maintenance margin. If not, conservative liquidation fires.
        let maint = self.maintenance_margin(position_notional);
        let margin_after_loss = available_margin_quote - unrealized_loss_quote;
        margin_after_loss < maint
    }
}

/// Reason a filter rejected an order. Used for cooldown classification
/// (plan §4.1: permanent_config vs temporary_margin vs market_data).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilterReject {
    PriceFilter,
    LotSizeMinQty,
    MinNotional,
    InvalidInput,
}

impl FilterReject {
    /// Classify the rejection for cooldown purposes (plan §4.1).
    /// min_notional impossible at this budget/cap => permanent_config (freeze).
    pub fn classification(&self) -> &'static str {
        match self {
            FilterReject::MinNotional | FilterReject::LotSizeMinQty => "permanent_config",
            FilterReject::PriceFilter => "permanent_config",
            FilterReject::InvalidInput => "permanent_config",
        }
    }
}

/// Round 20 P2 §4.1: rejection cooldown state. Tracks (group, depth, reason,
/// state_hash) and prevents re-emitting the same permanent rejection every bar.
#[derive(Debug, Clone, Default)]
pub struct RejectionCooldown {
    // key: (group_id, depth, reason_class, state_hash) -> last_emit_ms
    seen: std::collections::HashMap<String, i64>,
}

impl RejectionCooldown {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if this (group, depth, reason, state_hash) was already
    /// emitted and should be suppressed (cooldown active).
    pub fn is_suppressed(&self, group: &str, depth: u32, reason: &str, state_hash: &str) -> bool {
        let key = format!("{}|{}|{}|{}", group, depth, reason, state_hash);
        self.seen.contains_key(&key)
    }

    /// Record an emission. Returns the previous emission time if any.
    pub fn record(&mut self, group: &str, depth: u32, reason: &str, state_hash: &str, ts_ms: i64) -> Option<i64> {
        let key = format!("{}|{}|{}|{}", group, depth, reason, state_hash);
        self.seen.insert(key, ts_ms)
    }

    /// Permanent rejections (permanent_config) are frozen: once recorded they
    /// are always suppressed. Temporary rejections could expire, but for the
    /// research engine we treat all filter rejections as permanent within a
    /// cycle (plan §4.1: permanent_config => cycle/family freeze).
    pub fn len(&self) -> usize {
        self.seen.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn btc_filters() -> ExchangeFilters {
        ExchangeFilters { symbol: "BTCUSDT".to_string(), tick_size: 0.10, step_size: 0.001,
                          min_qty: 0.001, min_notional: 5.0, maint_margin_rate: 0.005, maint_amount: 0.0 }
    }

    #[test]
    fn price_filter_rounds_to_tick() {
        let f = btc_filters();
        assert!((f.round_price(100.04) - 100.0).abs() < 1e-9);
        assert!((f.round_price(100.06) - 100.1).abs() < 1e-9);
        assert!((f.round_price(99.97) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn lot_size_rounds_qty_down() {
        let f = btc_filters();
        // notional 31.0 at price 100 => qty 0.31 => rounds to 0.31 (step 0.001)
        let (_, q, _) = f.filter_order(100.0, 31.0).unwrap();
        assert!((q - 0.31).abs() < 1e-9);
        // notional 30.0001 => qty 0.300001 => rounds DOWN to 0.300
        let (_, q2, _) = f.filter_order(100.0, 30.0001).unwrap();
        assert!((q2 - 0.300).abs() < 1e-9);
    }

    #[test]
    fn min_notional_rejects_small_order() {
        let f = btc_filters();
        // notional 4.0 < min_notional 5.0 => reject
        assert_eq!(f.filter_order(100.0, 4.0).unwrap_err(), FilterReject::MinNotional);
        // notional 5.0 exactly => qty 0.05 >= min_qty 0.001, notional 5.0 >= 5.0 => pass
        let (_, _, n) = f.filter_order(100.0, 5.0).unwrap();
        assert!((n - 5.0).abs() < 1e-9 || n >= 4.99);
    }

    #[test]
    fn lot_size_min_qty_rejects_tiny_qty() {
        let f = ExchangeFilters { symbol: "X".to_string(), tick_size: 0.01, step_size: 0.1,
                                   min_qty: 0.2, min_notional: 5.0, maint_margin_rate: 0.005, maint_amount: 0.0 };
        // notional 10 at price 1000 => qty 0.01 < min_qty 0.2 => reject
        assert_eq!(f.filter_order(1000.0, 10.0).unwrap_err(), FilterReject::LotSizeMinQty);
    }

    #[test]
    fn maintenance_margin_uses_max_of_amount_and_rate() {
        let f = ExchangeFilters { symbol: "X".to_string(), tick_size: 0.1, step_size: 0.001,
                                   min_qty: 0.001, min_notional: 5.0, maint_margin_rate: 0.01, maint_amount: 50.0 };
        // notional 1000 * 0.01 = 10 < 50 => maint = 50
        assert!((f.maintenance_margin(1000.0) - 50.0).abs() < 1e-9);
        // notional 10000 * 0.01 = 100 > 50 => maint = 100
        assert!((f.maintenance_margin(10000.0) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn liquidation_triggered_when_margin_below_maintenance() {
        let f = btc_filters();
        // position notional 1000, maint = 1000*0.005 = 5. available 10, loss 8 => margin_after = 2 < 5 => liquidate
        assert!(f.liquidation_triggered(1000.0, 8.0, 10.0));
        // loss 3 => margin_after = 7 >= 5 => no liquidation
        assert!(!f.liquidation_triggered(1000.0, 3.0, 10.0));
    }

    #[test]
    fn stress_filters_are_more_conservative_than_default() {
        let d = ExchangeFilters::default_for("BTCUSDT");
        let s = ExchangeFilters::stress_for("BTCUSDT");
        assert!(s.tick_size > d.tick_size);
        assert!(s.step_size > d.step_size);
        assert!(s.min_notional > d.min_notional);
        assert!(s.maint_margin_rate > d.maint_margin_rate);
    }

    #[test]
    fn rejection_cooldown_suppresses_duplicate_permanent_reject() {
        let mut cd = RejectionCooldown::new();
        let g = "G1"; let depth = 1u32; let reason = "permanent_config"; let sh = "abc";
        assert!(!cd.is_suppressed(g, depth, reason, sh));
        cd.record(g, depth, reason, sh, 1000);
        // same (group,depth,reason,state_hash) is suppressed
        assert!(cd.is_suppressed(g, depth, reason, sh));
        // different state_hash is not suppressed
        assert!(!cd.is_suppressed(g, depth, reason, "def"));
        // different depth is not suppressed
        assert!(!cd.is_suppressed(g, 2, reason, sh));
    }

    #[test]
    fn filter_reject_classification_is_permanent_for_min_notional() {
        assert_eq!(FilterReject::MinNotional.classification(), "permanent_config");
        assert_eq!(FilterReject::LotSizeMinQty.classification(), "permanent_config");
        assert_eq!(FilterReject::PriceFilter.classification(), "permanent_config");
    }
}
