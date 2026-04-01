use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketTick {
    pub symbol: String,
    pub price: Decimal,
    pub event_time_ms: i64,
}
