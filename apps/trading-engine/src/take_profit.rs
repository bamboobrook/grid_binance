use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MakerTakeProfit {
    pub target_percent: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrailingTakeProfit {
    pub trigger_price: Decimal,
    pub trailing_percent: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverallTakeProfit {
    pub target_percent: Decimal,
}
