use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TradeFillInput {
    pub strategy_id: String,
    pub user_id: String,
    pub symbol: String,
    pub level_index: Option<u32>,
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
    pub is_short: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FillProfitView {
    pub strategy_id: String,
    pub user_id: String,
    pub symbol: String,
    pub level_index: Option<u32>,
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
    pub current_state: String,
    pub fill_count: usize,
    pub order_count: usize,
    #[serde(with = "rust_decimal::serde::str")]
    pub cost_basis: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub position_quantity: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub average_entry_price: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub long_position_quantity: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub long_average_entry_price: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub short_position_quantity: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub short_average_entry_price: Decimal,
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
    pub wallet_asset_count: usize,
    pub exchange_trade_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CostAggregation {
    #[serde(with = "rust_decimal::serde::str")]
    pub fees_paid: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub funding_total: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StrategySnapshotView {
    pub strategy_id: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub realized_pnl: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub unrealized_pnl: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub fees_paid: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub funding_total: Decimal,
    pub captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountSnapshotView {
    pub exchange: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub realized_pnl: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub unrealized_pnl: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub fees_paid: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub funding_total: Decimal,
    pub captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletSnapshotView {
    pub exchange: String,
    pub wallet_type: String,
    pub balances: BTreeMap<String, String>,
    pub captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExchangeTradeHistoryView {
    pub trade_id: String,
    pub exchange: String,
    pub symbol: String,
    pub side: String,
    pub quantity: String,
    pub price: String,
    pub fee_amount: Option<String>,
    pub fee_asset: Option<String>,
    pub traded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnalyticsReport {
    pub fills: Vec<FillProfitView>,
    pub strategies: Vec<StrategyProfitSummary>,
    pub user: UserAggregate,
    pub costs: CostAggregation,
    pub strategy_snapshots: Vec<StrategySnapshotView>,
    pub account_snapshots: Vec<AccountSnapshotView>,
    pub wallets: Vec<WalletSnapshotView>,
    pub exchange_trades: Vec<ExchangeTradeHistoryView>,
}
