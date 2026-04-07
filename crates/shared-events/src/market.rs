use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketTick {
    pub symbol: String,
    pub market: String,
    pub price: Decimal,
    pub event_time_ms: i64,
}
