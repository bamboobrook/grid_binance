use crate::subscriptions::{
    active_symbol_subscriptions, MarketStreamPlan, SymbolActivity, SymbolSubscription,
};
use futures_util::StreamExt;
use rust_decimal::Decimal;
use serde::Deserialize;
use shared_db::SharedDb;
use shared_events::MarketTick;
use std::{
    io::{Error as IoError, ErrorKind},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const INITIAL_RECONNECT_DELAY_MS: u64 = 1000;
const MAX_RECONNECT_DELAY_MS: u64 = 60000;
const MAX_RECONNECT_ATTEMPTS: u32 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceTradeEvent {
    pub symbol: String,
    pub market: String,
    pub price: Decimal,
    pub event_time_ms: i64,
}

impl BinanceTradeEvent {
    pub fn new(
        symbol: impl Into<String>,
        market: impl Into<String>,
        price: Decimal,
        event_time_ms: i64,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            market: market.into(),
            price,
            event_time_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayHealth {
    pub connected: bool,
    pub ready: bool,
    pub last_tick_age_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct GatewayRuntime {
    subscriptions: Vec<SymbolSubscription>,
    connected: bool,
    last_tick_time_ms: Option<i64>,
    reconnect_count: u32,
}

impl GatewayRuntime {
    pub fn new(symbols: &[SymbolActivity]) -> Self {
        Self {
            subscriptions: active_symbol_subscriptions(symbols),
            connected: true,
            last_tick_time_ms: None,
            reconnect_count: 0,
        }
    }

    pub fn subscriptions(&self) -> &[SymbolSubscription] {
        &self.subscriptions
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    pub fn reconnect(&mut self, symbols: &[SymbolActivity]) -> &[SymbolSubscription] {
        self.subscriptions = active_symbol_subscriptions(symbols);
        self.connected = true;
        self.last_tick_time_ms = None;
        self.reconnect_count += 1;
        &self.subscriptions
    }

    pub fn reconnect_count(&self) -> u32 {
        self.reconnect_count
    }

    pub fn emit_tick(&mut self, event: BinanceTradeEvent) -> Option<MarketTick> {
        let subscribed = self.subscriptions.iter().any(|subscription| {
            subscription.symbol == event.symbol && subscription.market == event.market
        });

        if !self.connected || !subscribed {
            return None;
        }

        self.last_tick_time_ms = Some(event.event_time_ms);

        Some(MarketTick {
            symbol: event.symbol,
            market: event.market,
            price: event.price,
            event_time_ms: event.event_time_ms,
        })
    }

    pub fn health(&self, now_ms: i64, stale_after_ms: i64) -> GatewayHealth {
        let last_tick_age_ms = self.last_tick_time_ms.map(|tick_time| now_ms - tick_time);
        let ready =
            self.connected && matches!(last_tick_age_ms, Some(age_ms) if age_ms <= stale_after_ms);

        GatewayHealth {
            connected: self.connected,
            ready,
            last_tick_age_ms,
        }
    }
}

pub async fn run_market_stream(
    plan: &MarketStreamPlan,
    runtime: Arc<Mutex<GatewayRuntime>>,
    db: SharedDb,
) -> Result<(), IoError> {
    let mut reconnect_attempt: u32 = 0;

    loop {
        match connect_async(&plan.url).await {
            Ok((stream, _)) => {
                reconnect_attempt = 0; // Reset on successful connection
                let (_, mut read) = stream.split();

                while let Some(message) = read.next().await {
                    let message = match message {
                        Ok(msg) => msg,
                        Err(error) => {
                            eprintln!("WebSocket error for {}: {}", plan.market, error);
                            break;
                        }
                    };

                    let payload = match message {
                        Message::Text(text) => text.to_string(),
                        Message::Binary(bytes) => match String::from_utf8(bytes.to_vec()) {
                            Ok(text) => text,
                            Err(error) => {
                                eprintln!("Invalid UTF-8 from {}: {}", plan.market, error);
                                continue;
                            }
                        },
                        Message::Close(frame) => {
                            eprintln!("WebSocket closed for {}: {:?}", plan.market, frame);
                            break;
                        }
                        _ => continue,
                    };

                    if let Some(event) = parse_trade_message(&plan.market, &payload) {
                        if let Ok(mut guard) = runtime.lock() {
                            if let Some(tick) = guard.emit_tick(event) {
                                if let Err(error) = db.enqueue_market_tick(&tick) {
                                    eprintln!("Failed to enqueue tick: {}", error);
                                }
                            }
                        }
                    }
                }

                if let Ok(mut guard) = runtime.lock() {
                    guard.disconnect();
                }
            }
            Err(error) => {
                eprintln!("Failed to connect to {}: {}", plan.market, error);
            }
        }

        // Calculate exponential backoff delay
        reconnect_attempt += 1;
        if reconnect_attempt >= MAX_RECONNECT_ATTEMPTS {
            return Err(IoError::new(
                ErrorKind::Other,
                format!(
                    "Max reconnect attempts ({}) exceeded for {}",
                    MAX_RECONNECT_ATTEMPTS, plan.market
                ),
            ));
        }

        let delay_ms = INITIAL_RECONNECT_DELAY_MS * 2_u64.pow(reconnect_attempt - 1);
        let delay_ms = delay_ms.min(MAX_RECONNECT_DELAY_MS);
        eprintln!(
            "Reconnecting to {} in {}ms (attempt {}/{})...",
            plan.market, delay_ms, reconnect_attempt, MAX_RECONNECT_ATTEMPTS
        );
        sleep(Duration::from_millis(delay_ms)).await;
    }
}

pub fn parse_trade_message(default_market: &str, payload: &str) -> Option<BinanceTradeEvent> {
    let combined = serde_json::from_str::<CombinedTradeEnvelope>(payload).ok();
    let raw = match combined {
        Some(envelope) => envelope.data,
        None => serde_json::from_str::<TradePayload>(payload).ok()?,
    };

    let price = raw.price.parse::<Decimal>().ok()?;
    Some(BinanceTradeEvent::new(
        raw.symbol,
        default_market.to_ascii_lowercase(),
        price,
        raw.event_time_ms,
    ))
}

#[derive(Debug, Deserialize)]
struct CombinedTradeEnvelope {
    data: TradePayload,
}

#[derive(Debug, Deserialize)]
struct TradePayload {
    #[serde(rename = "E")]
    event_time_ms: i64,
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "p")]
    price: String,
}

#[cfg(test)]
mod tests {
    use super::parse_trade_message;
    use rust_decimal::Decimal;

    #[test]
    fn parse_trade_message_preserves_market_and_price() {
        let event = parse_trade_message(
            "usdm",
            r#"{"stream":"btcusdt@trade","data":{"e":"trade","E":1710000,"s":"BTCUSDT","p":"43125.78"}}"#,
        )
        .expect("trade message should parse");

        assert_eq!(event.symbol, "BTCUSDT");
        assert_eq!(event.market, "usdm");
        assert_eq!(event.price, Decimal::new(4312578, 2));
        assert_eq!(event.event_time_ms, 1_710_000);
    }

    #[test]
    fn parse_trade_message_rejects_invalid_payloads() {
        assert!(parse_trade_message("spot", "{}").is_none());
        assert!(parse_trade_message(
            "spot",
            r#"{"stream":"btcusdt@trade","data":{"e":"trade","E":1710000,"s":"BTCUSDT","p":"nope"}}"#,
        )
        .is_none());
    }
}
