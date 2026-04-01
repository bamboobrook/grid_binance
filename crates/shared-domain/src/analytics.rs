use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TradeFillInput {
    pub strategy_id: String,
    pub user_id: String,
    pub symbol: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub entry_price: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub exit_price: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub fee: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub funding: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FillProfitView {
    pub strategy_id: String,
    pub user_id: String,
    pub symbol: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub entry_price: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub exit_price: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub realized_pnl: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub fee: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub funding: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub net_pnl: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StrategyProfitSummary {
    pub strategy_id: String,
    pub user_id: String,
    pub symbol: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub realized_pnl: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub unrealized_pnl: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub fees_paid: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub funding_total: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub net_pnl: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserAggregate {
    pub user_id: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub realized_pnl: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub unrealized_pnl: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub fees_paid: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub funding_total: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub net_pnl: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CostAggregation {
    #[serde(with = "rust_decimal::serde::str")]
    pub fees_paid: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub funding_total: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnalyticsReport {
    pub fills: Vec<FillProfitView>,
    pub strategies: Vec<StrategyProfitSummary>,
    pub user: UserAggregate,
    pub costs: CostAggregation,
}
