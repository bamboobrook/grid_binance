//! R4-M2 signal layer: Depth Replenishment + Aggressor Flow (plan §8 M2).
//!
//! Computes the depth imbalance and aggressor-flow exhaustion state used to
//! gate a Martin's FO/SO. This is the SIGNAL layer; combined with M1 via an
//! AND-gate it should increase trade frequency while staying conservative.
//!
//! M2 state (plan §8 M2):
//!   depth_imbalance(level) = (bid_notional - ask_notional) / (bid_notional + ask_notional)
//!     at 0.2% and 1.0% levels
//!   aggressor_flow = signed volume (buyer-initiated - seller-initiated) over a window
//!   replenishment = depth_imbalance moved favorably (bid replenished after a sell-off)
//!   flow_decay = aggressor pressure decelerating
//!
//! Long FO (M2): price downside + sell aggressor flow decay + bid depth replenishment.
//! Short FO is the mirror.

use serde::{Deserialize, Serialize};

const fn _v() {}

/// One bookDepth snapshot row (plan §7.2 schema: timestamp,percentage,depth,notional).
/// Binance uses integer percentages: negative = bid levels, positive = ask levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthRow {
    pub ts_ms: i64,
    pub percentage: i32, // -5..-1 (bid), +1..+5 (ask)
    pub depth: f64,
    pub notional: f64,
}

/// One aggTrades row (agg_id,price,quantity,first,last,transact_time,is_buyer_maker).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggTradeRow {
    pub ts_ms: i64,
    pub price: f64,
    pub quantity: f64,
    pub is_buyer_maker: bool,
}

/// The M2 state at a timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct M2State {
    pub ts_ms: i64,
    /// depth imbalance at 0.2% (-1..+1, positive = bid-heavy).
    pub imb_02: f64,
    /// depth imbalance at 1.0%.
    pub imb_10: f64,
    /// median of the two imbalances.
    pub imb_median: f64,
    /// signed aggressor flow over the window (positive = buy pressure).
    pub aggressor_flow: f64,
    /// true if bid depth replenished recently (imb rose in last few snapshots).
    pub bid_replenished: bool,
    /// true if sell aggressor flow decelerating (for long FO confirmation).
    pub sell_flow_decayed: bool,
    /// true if buy aggressor flow decelerating (for short FO confirmation).
    pub buy_flow_decayed: bool,
}

/// Build the M2 state series from depth rows + aggtrades, resampled to 5m snapshots.
/// `flow_window_bars` is the aggressor-flow lookback (15m=3, 60m=12 at 5m bars).
pub fn compute_m2_states(
    depth: &[DepthRow],
    trades: &[AggTradeRow],
    window_bars: usize,
) -> Vec<M2State> {
    use std::collections::BTreeMap;
    // index depth by (ts, percentage-key-string) to avoid f64 Ord issues.
    let mut depth_by_ts: BTreeMap<i64, BTreeMap<String, (f64, f64)>> = BTreeMap::new();
    for d in depth {
        depth_by_ts
            .entry(d.ts_ms)
            .or_insert_with(BTreeMap::new)
            .insert(pct_key(d.percentage), (d.depth, d.notional));
    }
    // aggregate trades into 5m buckets: signed flow = sum(qty if buyer-maker neg else pos)
    let bucket_ms = 300_000i64;
    let mut trade_flow_by_bucket: BTreeMap<i64, f64> = BTreeMap::new();
    for t in trades {
        let b = (t.ts_ms / bucket_ms) * bucket_ms;
        let signed = if t.is_buyer_maker { -t.quantity * t.price } else { t.quantity * t.price };
        *trade_flow_by_bucket.entry(b).or_insert(0.0) += signed;
    }
    let mut states = Vec::new();
    let mut imb_hist: Vec<f64> = Vec::new();
    let mut flow_hist: Vec<f64> = Vec::new();
    let mut sorted_ts: Vec<i64> = depth_by_ts.keys().copied().collect();
    sorted_ts.sort();
    for ts in sorted_ts {
        let dmap = &depth_by_ts[&ts];
        // Binance levels are 1%, 2% (integer). Plan's 0.2%/1.0% mapped to available 1%/2%.
        let (bid1, ask1) = split_bid_ask(dmap, 1);
        let (bid2, ask2) = split_bid_ask(dmap, 2);
        let imb02 = imbalance(bid1, ask1); // 1% level (closest to plan's 0.2%)
        let imb10 = imbalance(bid2, ask2); // 2% level (closest to plan's 1.0%)
        let imb_median = (imb02 + imb10) / 2.0;
        imb_hist.push(imb_median);
        if imb_hist.len() > window_bars { imb_hist.remove(0); }
        // aggressor flow at this bucket (and recent decay check)
        let cur_flow = trade_flow_by_bucket.get(&ts).copied().unwrap_or(0.0);
        flow_hist.push(cur_flow);
        if flow_hist.len() > window_bars { flow_hist.remove(0); }
        // replenishment: imb rose in last 3 snapshots
        let n = imb_hist.len();
        let bid_replenished = n >= 4 && imb_hist[n-1] > imb_hist[n-4];
        // flow decay: |flow| decreased recently
        let recent_sell: Vec<f64> = flow_hist.iter().rev().take(6).map(|f| f.min(0.0).abs()).collect();
        let recent_buy: Vec<f64> = flow_hist.iter().rev().take(6).map(|f| f.max(0.0)).collect();
        let sell_flow_decayed = recent_sell.len() >= 2 && recent_sell[0] < recent_sell[recent_sell.len()-1];
        let buy_flow_decayed = recent_buy.len() >= 2 && recent_buy[0] < recent_buy[recent_buy.len()-1];
        states.push(M2State {
            ts_ms: ts, imb_02: imb02, imb_10: imb10, imb_median,
            aggressor_flow: cur_flow, bid_replenished, sell_flow_decayed, buy_flow_decayed,
        });
    }
    states
}

fn pct_key(pct: i32) -> String {
    // Binance bookDepth uses integer percentages (1,2,3,4,5; negative = bid).
    format!("{}", pct)
}

fn split_bid_ask(dmap: &std::collections::BTreeMap<String, (f64, f64)>, pct: i32) -> (f64, f64) {
    // Binance bookDepth: bid side at -pct, ask side at +pct. Keyed by formatted string.
    let bid = dmap.get(&pct_key(-pct)).map(|(_, n)| *n).unwrap_or(0.0);
    let ask = dmap.get(&pct_key(pct)).map(|(_, n)| *n).unwrap_or(0.0);
    (bid, ask)
}

fn imbalance(bid: f64, ask: f64) -> f64 {
    let s = bid + ask;
    if s > 1e-9 { (bid - ask) / s } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imbalance_symmetric_is_zero() {
        assert!(imbalance(100.0, 100.0).abs() < 1e-9);
        assert!((imbalance(150.0, 50.0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn compute_handles_empty() {
        let s = compute_m2_states(&[], &[], 12);
        assert!(s.is_empty());
    }
}
