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

pub fn take_profit_price_from_percent(
    entry_price: Decimal,
    target_percent: Decimal,
    is_short: bool,
) -> Decimal {
    if is_short {
        entry_price * (Decimal::ONE - target_percent)
    } else {
        entry_price * (Decimal::ONE + target_percent)
    }
}

pub fn take_profit_price_from_bps(
    entry_price: Decimal,
    take_profit_bps: u32,
    is_short: bool,
) -> Decimal {
    take_profit_price_from_percent(
        entry_price,
        Decimal::from(take_profit_bps) / Decimal::from(10_000u32),
        is_short,
    )
}
