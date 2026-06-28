//! Pure 1m candle aggregation for live martingale indicator feeds.
//!
//! Extracted verbatim from `main.rs`'s former `completed_martingale_indicator_bars`
//! (which used `HOUR_MS = 3_600_000`). The backtest runs ATR(21)/ADX(14) on 1m bars,
//! so live aggregation must use the same interval. `complete_bars` is parameterized
//! by `bucket_ms` so callers (and tests) can choose the interval; production wiring
//! passes `MINUTE_MS`.

use std::collections::HashMap;

use backtest_engine::market_data::KlineBar;
use rust_decimal::prelude::ToPrimitive;
use shared_events::MarketTick;

/// One minute in milliseconds. Matches the backtest bar interval so that
/// per-bar indicators (ATR(21), ADX(14)) are evaluated on identical bars live.
pub const MINUTE_MS: i64 = 60_000;

#[derive(Debug, Clone)]
pub struct LiveCandleBucket {
    pub open_time_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

impl LiveCandleBucket {
    pub fn new(open_time_ms: i64, price: f64) -> Self {
        Self {
            open_time_ms,
            open: price,
            high: price,
            low: price,
            close: price,
        }
    }

    pub fn update(&mut self, price: f64) {
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
    }

    pub fn into_bar(self, symbol: String) -> KlineBar {
        KlineBar {
            symbol,
            open_time_ms: self.open_time_ms,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: 0.0,
        }
    }
}

/// Pure bucketing: folds `ticks` into `buckets` and returns the bars that
/// completed during this call (i.e. bars whose next tick landed in a strictly
/// later bucket).
///
/// Semantics (moved verbatim from the former `main.rs` loop, only the bucket
/// divisor is parameterized):
/// - same bucket  -> `bucket.update(price)`
/// - strictly-later bucket -> emit previous bar, start new bucket
/// - strictly-earlier bucket (out-of-order) -> dropped
/// - no existing bucket for symbol -> insert new bucket (no emission)
/// - non-futures markets and non-finite/non-positive prices are skipped
pub fn complete_bars(
    buckets: &mut HashMap<String, LiveCandleBucket>,
    ticks: &[MarketTick],
    bucket_ms: i64,
) -> Vec<KlineBar> {
    let mut completed = Vec::new();

    for tick in ticks {
        if tick.market != "usdm" && tick.market != "futures" && tick.market != "usd_m_futures" {
            continue;
        }
        let Some(price) = tick
            .price
            .to_f64()
            .filter(|price| price.is_finite() && *price > 0.0)
        else {
            continue;
        };
        let bucket_open_ms = tick.event_time_ms.div_euclid(bucket_ms) * bucket_ms;
        match buckets.get_mut(&tick.symbol) {
            Some(bucket) if bucket.open_time_ms == bucket_open_ms => bucket.update(price),
            Some(bucket) if bucket.open_time_ms < bucket_open_ms => {
                let previous = bucket.clone().into_bar(tick.symbol.clone());
                *bucket = LiveCandleBucket::new(bucket_open_ms, price);
                completed.push(previous);
            }
            Some(_) => {}
            None => {
                buckets.insert(
                    tick.symbol.clone(),
                    LiveCandleBucket::new(bucket_open_ms, price),
                );
            }
        }
    }

    completed
}
