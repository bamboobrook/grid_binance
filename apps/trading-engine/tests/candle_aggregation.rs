use std::collections::HashMap;

use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use shared_events::MarketTick;
use trading_engine::martingale_candle::{complete_bars, LiveCandleBucket, MINUTE_MS};

fn tick(sym: &str, ms: i64, price: f64) -> MarketTick {
    MarketTick {
        symbol: sym.to_string(),
        market: "usdm".to_string(),
        price: Decimal::from_f64(price).unwrap(),
        event_time_ms: ms,
    }
}

#[test]
fn completed_bars_emit_on_minute_boundary_not_hour() {
    let mut buckets: HashMap<String, LiveCandleBucket> = HashMap::new();
    let ticks = vec![
        tick("BTCUSDT", 0, 100.0),
        tick("BTCUSDT", 30_000, 102.0), // same 1m bucket
        tick("BTCUSDT", 60_000, 101.0), // next 1m bucket -> minute-0 bar completes
    ];
    let bars = complete_bars(&mut buckets, &ticks, MINUTE_MS);
    assert_eq!(bars.len(), 1, "exactly the minute-0 bar should complete");
    assert_eq!(bars[0].open_time_ms, 0);
    assert_eq!(bars[0].open, 100.0);
    assert_eq!(bars[0].high, 102.0);
    assert_eq!(bars[0].low, 100.0);
    assert_eq!(bars[0].close, 102.0);
}

#[test]
fn out_of_order_tick_is_dropped_and_does_not_emit() {
    let mut buckets: HashMap<String, LiveCandleBucket> = HashMap::new();
    let ticks = vec![
        tick("ETHUSDT", 120_000, 50.0), // opens bucket @ 120s
        tick("ETHUSDT", 60_000, 49.0),  // out-of-order (< bucket.open) -> dropped
        tick("ETHUSDT", 180_000, 51.0), // next bucket -> emit bar @ 120s
    ];
    let bars = complete_bars(&mut buckets, &ticks, MINUTE_MS);
    assert_eq!(bars.len(), 1);
    assert_eq!(bars[0].open_time_ms, 120_000);
    assert_eq!(bars[0].open, 50.0);
    assert_eq!(bars[0].close, 50.0);
}

#[test]
fn non_usdm_market_is_skipped() {
    let mut buckets: HashMap<String, LiveCandleBucket> = HashMap::new();
    let ticks = vec![
        MarketTick {
            symbol: "BTCUSDT".to_string(),
            market: "spot".to_string(),
            price: Decimal::from_f64(100.0).unwrap(),
            event_time_ms: 0,
        },
        // spot tick is skipped; usdm opens bucket @ 60s and is completed by the 120s tick
        tick("BTCUSDT", 60_000, 101.0),
        tick("BTCUSDT", 120_000, 103.0),
    ];
    let bars = complete_bars(&mut buckets, &ticks, MINUTE_MS);
    assert_eq!(bars.len(), 1, "spot tick is skipped; only the usdm minute-1 bar completes");
    assert_eq!(bars[0].open_time_ms, 60_000);
    assert_eq!(bars[0].open, 101.0);
}
