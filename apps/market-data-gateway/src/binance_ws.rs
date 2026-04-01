use crate::subscriptions::{active_symbol_subscriptions, SymbolActivity, SymbolSubscription};
use rust_decimal::Decimal;
use shared_events::MarketTick;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceTradeEvent {
    pub symbol: String,
    pub price: Decimal,
    pub event_time_ms: i64,
}

impl BinanceTradeEvent {
    pub fn new(symbol: impl Into<String>, price: Decimal, event_time_ms: i64) -> Self {
        Self {
            symbol: symbol.into(),
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
        self.reconnect_count += 1;
        &self.subscriptions
    }

    pub fn reconnect_count(&self) -> u32 {
        self.reconnect_count
    }

    pub fn emit_tick(&mut self, event: BinanceTradeEvent) -> Option<MarketTick> {
        let subscribed = self
            .subscriptions
            .iter()
            .any(|subscription| subscription.symbol == event.symbol);

        if !self.connected || !subscribed {
            return None;
        }

        self.last_tick_time_ms = Some(event.event_time_ms);

        Some(MarketTick {
            symbol: event.symbol,
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
