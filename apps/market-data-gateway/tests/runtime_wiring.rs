use market_data_gateway::{
    binance_ws::{BinanceTradeEvent, GatewayRuntime},
    subscriptions::SymbolActivity,
};
use rust_decimal::Decimal;
use shared_events::MarketTick;

#[tokio::test]
async fn subscribe_only_active_symbols_and_emit_ticks() {
    let symbols = vec![
        SymbolActivity::new("BTCUSDT", true),
        SymbolActivity::new("ETHUSDT", false),
        SymbolActivity::new("BNBUSDT", true),
    ];
    let mut runtime = GatewayRuntime::new(&symbols);

    let subscriptions = runtime.subscriptions();
    assert_eq!(subscriptions.len(), 2);
    assert_eq!(subscriptions[0].stream_name, "btcusdt@trade");
    assert_eq!(subscriptions[1].stream_name, "bnbusdt@trade");

    let tick = runtime.emit_tick(BinanceTradeEvent::new(
        "BTCUSDT",
        Decimal::new(4312578, 2),
        1_710_000,
    ));
    assert_eq!(
        tick,
        Some(MarketTick {
            symbol: "BTCUSDT".to_string(),
            price: Decimal::new(4312578, 2),
            event_time_ms: 1_710_000,
        })
    );

    let ignored = runtime.emit_tick(BinanceTradeEvent::new(
        "ETHUSDT",
        Decimal::new(312499, 2),
        1_710_050,
    ));
    assert_eq!(ignored, None);
}

#[tokio::test]
async fn reconnect_refreshes_subscription_plan() {
    let mut runtime = GatewayRuntime::new(&[SymbolActivity::new("BTCUSDT", true)]);
    runtime.disconnect();

    let refreshed = runtime
        .reconnect(&[
            SymbolActivity::new("BTCUSDT", false),
            SymbolActivity::new("ETHUSDT", true),
        ])
        .to_vec();

    assert_eq!(runtime.reconnect_count(), 1);
    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].stream_name, "ethusdt@trade");

    let stale_symbol = runtime.emit_tick(BinanceTradeEvent::new("BTCUSDT", Decimal::new(1, 0), 10));
    assert_eq!(stale_symbol, None);

    let active_symbol =
        runtime.emit_tick(BinanceTradeEvent::new("ETHUSDT", Decimal::new(2, 0), 20));
    assert_eq!(
        active_symbol,
        Some(MarketTick {
            symbol: "ETHUSDT".to_string(),
            price: Decimal::new(2, 0),
            event_time_ms: 20,
        })
    );
}

#[tokio::test]
async fn health_reflects_connection_and_tick_freshness() {
    let mut runtime = GatewayRuntime::new(&[SymbolActivity::new("BTCUSDT", true)]);

    let cold_health = runtime.health(1_000, 500);
    assert_eq!(cold_health.connected, true);
    assert_eq!(cold_health.ready, false);
    assert_eq!(cold_health.last_tick_age_ms, None);

    runtime.emit_tick(BinanceTradeEvent::new(
        "BTCUSDT",
        Decimal::new(123, 0),
        1_200,
    ));
    let warm_health = runtime.health(1_500, 500);
    assert_eq!(warm_health.connected, true);
    assert_eq!(warm_health.ready, true);
    assert_eq!(warm_health.last_tick_age_ms, Some(300));

    let stale_health = runtime.health(1_801, 500);
    assert_eq!(stale_health.connected, true);
    assert_eq!(stale_health.ready, false);
    assert_eq!(stale_health.last_tick_age_ms, Some(601));

    runtime.disconnect();
    let disconnected_health = runtime.health(1_801, 500);
    assert_eq!(disconnected_health.connected, false);
    assert_eq!(disconnected_health.ready, false);
    assert_eq!(disconnected_health.last_tick_age_ms, Some(601));
}

#[tokio::test]
async fn reconnect_clears_stale_freshness_until_new_tick_arrives() {
    let mut runtime = GatewayRuntime::new(&[SymbolActivity::new("BTCUSDT", true)]);

    runtime.emit_tick(BinanceTradeEvent::new(
        "BTCUSDT",
        Decimal::new(456, 0),
        2_000,
    ));
    let pre_reconnect = runtime.health(2_100, 500);
    assert_eq!(pre_reconnect.ready, true);
    assert_eq!(pre_reconnect.last_tick_age_ms, Some(100));

    runtime.disconnect();
    runtime.reconnect(&[SymbolActivity::new("BTCUSDT", true)]);

    let post_reconnect = runtime.health(2_100, 500);
    assert_eq!(post_reconnect.connected, true);
    assert_eq!(post_reconnect.ready, false);
    assert_eq!(post_reconnect.last_tick_age_ms, None);
}
