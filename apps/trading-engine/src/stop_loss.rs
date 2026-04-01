use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverallStopLoss {
    pub max_drawdown_percent: Decimal,
}
