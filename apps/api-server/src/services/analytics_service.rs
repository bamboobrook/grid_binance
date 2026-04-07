use std::collections::BTreeMap;

use rust_decimal::Decimal;
use shared_db::{
    AccountProfitSnapshotRecord, BillingOrderRecord, ExchangeTradeHistoryRecord,
    ExchangeWalletSnapshotRecord, SharedDb, SharedDbError, StrategyProfitSnapshotRecord,
};
use shared_domain::analytics::{
    AccountSnapshotView, AnalyticsReport, CostAggregation, ExchangeTradeHistoryView,
    StrategyProfitSummary, StrategySnapshotView, TradeFillInput, UserAggregate,
    WalletSnapshotView,
};
use shared_domain::strategy::{Strategy, StrategyRuntimePosition};
use trading_engine::statistics::compute_fill_views;

#[derive(Clone)]
pub struct AnalyticsService {
    db: SharedDb,
}

impl Default for AnalyticsService {
    fn default() -> Self {
        Self::new(SharedDb::ephemeral().expect("ephemeral analytics db should initialize"))
    }
}

impl AnalyticsService {
    pub fn new(db: SharedDb) -> Self {
        Self { db }
    }

    pub fn report_for_user(&self, user_email: &str) -> Result<AnalyticsReport, SharedDbError> {
        let strategies = self.db.list_strategies(user_email)?;
        let strategy_snapshot_records = self.db.list_strategy_profit_snapshots(user_email)?;
        let account_snapshot_records = self.db.list_account_profit_snapshots(user_email)?;
        let wallet_snapshot_records = self.db.list_exchange_wallet_snapshots(user_email)?;
        let trade_history_records = self.db.list_exchange_trade_history(user_email)?;

        let fills = compute_fill_views(&self.trade_fill_inputs(&strategies));
        let strategy_snapshots = to_strategy_snapshot_views(&strategy_snapshot_records)?;
        let account_snapshots = to_account_snapshot_views(&account_snapshot_records)?;
        let wallets = to_wallet_snapshot_views(&wallet_snapshot_records);
        let exchange_trades = to_exchange_trade_history_views(&trade_history_records);
        let latest_strategy_snapshots = latest_strategy_snapshot_map(&strategy_snapshot_records)?;
        let latest_account_snapshots =
            latest_account_snapshots_by_exchange(&account_snapshot_records)?;
        let latest_wallets = latest_wallets_by_exchange(&wallet_snapshot_records);

        let strategies = strategies
            .iter()
            .map(|strategy| strategy_summary(strategy, latest_strategy_snapshots.get(&strategy.id)))
            .collect::<Result<Vec<_>, SharedDbError>>()?;

        let user = user_aggregate(
            user_email,
            &fills,
            &trade_history_records,
            &latest_account_snapshots,
            &latest_wallets,
            trade_history_records.len(),
        )?;
        let costs = cost_aggregation(&fills, &trade_history_records, &latest_account_snapshots)?;

        Ok(AnalyticsReport {
            fills,
            strategies,
            user,
            costs,
            strategy_snapshots,
            account_snapshots,
            wallets,
            exchange_trades,
        })
    }

    pub fn export_orders_csv(&self, user_email: &str) -> Result<String, SharedDbError> {
        let mut csv =
            String::from("order_id,strategy_id,symbol,side,order_type,price,quantity,status\n");
        for strategy in self.db.list_strategies(user_email)? {
            for order in strategy.runtime.orders {
                csv.push_str(&format!(
                    "{},{},{},{},{},{},{},{}\n",
                    order.order_id,
                    strategy.id,
                    strategy.symbol,
                    order.side,
                    order.order_type,
                    order.price.map(format_decimal).unwrap_or_default(),
                    format_decimal(order.quantity),
                    order.status,
                ));
            }
        }
        Ok(csv)
    }

    pub fn export_fills_csv(&self, user_email: &str) -> Result<String, SharedDbError> {
        let mut csv = String::from(
            "fill_id,strategy_id,order_id,symbol,price,quantity,realized_pnl,fee_amount,fee_asset,fill_type\n",
        );
        for strategy in self.db.list_strategies(user_email)? {
            for fill in strategy.runtime.fills {
                csv.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{}\n",
                    fill.fill_id,
                    strategy.id,
                    fill.order_id.unwrap_or_default(),
                    strategy.symbol,
                    format_decimal(fill.price),
                    format_decimal(fill.quantity),
                    fill.realized_pnl.map(format_decimal).unwrap_or_default(),
                    fill.fee_amount.map(format_decimal).unwrap_or_default(),
                    fill.fee_asset.unwrap_or_default(),
                    fill.fill_type,
                ));
            }
        }
        Ok(csv)
    }

    pub fn export_strategy_stats_csv(&self, user_email: &str) -> Result<String, SharedDbError> {
        let strategies = self.db.list_strategies(user_email)?;
        let latest_strategy_snapshots =
            latest_strategy_snapshot_map(&self.db.list_strategy_profit_snapshots(user_email)?)?;
        let summaries = strategies
            .iter()
            .map(|strategy| strategy_summary(strategy, latest_strategy_snapshots.get(&strategy.id)))
            .collect::<Result<Vec<_>, SharedDbError>>()?;

        let mut csv = String::from(
            "strategy_id,user_id,symbol,current_state,fill_count,order_count,cost_basis,position_quantity,average_entry_price,realized_pnl,unrealized_pnl,fees_paid,funding_total,net_pnl
",
        );
        for summary in &summaries {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{}
",
                summary.strategy_id,
                summary.user_id,
                summary.symbol,
                summary.current_state,
                summary.fill_count,
                summary.order_count,
                format_decimal(summary.cost_basis),
                format_decimal(summary.position_quantity),
                format_decimal(summary.average_entry_price),
                format_decimal(summary.realized_pnl),
                format_decimal(summary.unrealized_pnl),
                format_decimal(summary.fees_paid),
                format_decimal(summary.funding_total),
                format_decimal(summary.net_pnl),
            ));
        }
        Ok(csv)
    }

    pub fn export_payments_csv(&self, user_email: &str) -> Result<String, SharedDbError> {
        let mut csv = String::from(
            "order_id,email,chain,asset,plan_code,amount,status,address,requested_at,paid_at,tx_hash\n",
        );
        let mut orders = self.db.list_billing_orders()?;
        orders.retain(|order| order.email.eq_ignore_ascii_case(user_email));
        for order in orders {
            csv.push_str(&payment_row(&order));
        }
        Ok(csv)
    }

    fn trade_fill_inputs(&self, strategies: &[Strategy]) -> Vec<TradeFillInput> {
        let mut fills = Vec::new();
        for strategy in strategies {
            for fill in &strategy.runtime.fills {
                fills.push(TradeFillInput {
                    strategy_id: strategy.id.clone(),
                    user_id: strategy.owner_email.clone(),
                    symbol: strategy.symbol.clone(),
                    quantity: fill.quantity,
                    entry_price: derive_entry_price(fill.price, fill.quantity, fill.realized_pnl),
                    exit_price: fill.price,
                    fee: fill.fee_amount.unwrap_or(Decimal::ZERO),
                    funding: Decimal::ZERO,
                });
            }
        }
        fills
    }
}

fn strategy_summary(
    strategy: &Strategy,
    snapshot: Option<&StrategySnapshotNumbers>,
) -> Result<StrategyProfitSummary, SharedDbError> {
    let realized_from_fills = strategy
        .runtime
        .fills
        .iter()
        .filter_map(|fill| fill.realized_pnl)
        .fold(Decimal::ZERO, |acc, value| acc + value);
    let fees_from_fills = strategy
        .runtime
        .fills
        .iter()
        .filter_map(|fill| fill.fee_amount)
        .fold(Decimal::ZERO, |acc, value| acc + value);

    let position = aggregate_position(&strategy.runtime.positions)?;
    let cost_basis = position
        .map(|(quantity, average_entry_price)| quantity * average_entry_price)
        .unwrap_or(Decimal::ZERO);
    let position_quantity = position
        .map(|(quantity, _)| quantity)
        .unwrap_or(Decimal::ZERO);
    let average_entry_price = position
        .map(|(_, average_entry_price)| average_entry_price)
        .unwrap_or(Decimal::ZERO);
    let realized_pnl = snapshot
        .map(|snapshot| snapshot.realized_pnl)
        .unwrap_or(realized_from_fills);
    let unrealized_pnl = snapshot
        .map(|snapshot| snapshot.unrealized_pnl)
        .unwrap_or(Decimal::ZERO);
    let fees_paid = snapshot
        .map(|snapshot| snapshot.fees_paid)
        .unwrap_or(fees_from_fills);
    let funding_total = snapshot
        .map(|snapshot| snapshot.funding_total)
        .unwrap_or(Decimal::ZERO);
    let net_pnl = realized_pnl + unrealized_pnl - fees_paid + funding_total;

    Ok(StrategyProfitSummary {
        strategy_id: strategy.id.clone(),
        user_id: strategy.owner_email.clone(),
        symbol: strategy.symbol.clone(),
        current_state: format!("{:?}", strategy.status),
        fill_count: strategy.runtime.fills.len(),
        order_count: strategy.runtime.orders.len(),
        cost_basis: cost_basis.normalize(),
        position_quantity: position_quantity.normalize(),
        average_entry_price: average_entry_price.normalize(),
        realized_pnl: realized_pnl.normalize(),
        unrealized_pnl: unrealized_pnl.normalize(),
        fees_paid: fees_paid.normalize(),
        funding_total: funding_total.normalize(),
        net_pnl: net_pnl.normalize(),
    })
}

fn user_aggregate(
    user_email: &str,
    fills: &[shared_domain::analytics::FillProfitView],
    trade_history_records: &[ExchangeTradeHistoryRecord],
    latest_account_snapshots: &BTreeMap<String, AccountSnapshotNumbers>,
    latest_wallets: &BTreeMap<String, WalletSnapshotView>,
    exchange_trade_count: usize,
) -> Result<UserAggregate, SharedDbError> {
    let realized_from_fills = fills
        .iter()
        .fold(Decimal::ZERO, |acc, fill| acc + fill.realized_pnl);
    let fees_from_fills = fills.iter().fold(Decimal::ZERO, |acc, fill| acc + fill.fee);
    let funding_from_fills = fills
        .iter()
        .fold(Decimal::ZERO, |acc, fill| acc + fill.funding);

    let snapshot_realized = latest_account_snapshots
        .values()
        .fold(Decimal::ZERO, |acc, snapshot| acc + snapshot.realized_pnl);
    let realized_pnl = resolve_snapshot_total(
        (!latest_account_snapshots.is_empty()).then_some(snapshot_realized),
        realized_from_fills,
    );
    let unrealized_pnl = latest_account_snapshots
        .values()
        .fold(Decimal::ZERO, |acc, snapshot| acc + snapshot.unrealized_pnl);
    let trade_history_fees = trade_history_fee_total(trade_history_records)?;
    let snapshot_fees = latest_account_snapshots
        .values()
        .fold(Decimal::ZERO, |acc, snapshot| acc + snapshot.fees_paid);
    let fees_paid = resolve_fee_total(
        (!latest_account_snapshots.is_empty()).then_some(snapshot_fees),
        trade_history_fees,
        fees_from_fills,
    );
    let snapshot_funding = latest_account_snapshots
        .values()
        .fold(Decimal::ZERO, |acc, snapshot| acc + snapshot.funding_total);
    let funding_total = resolve_snapshot_total(
        (!latest_account_snapshots.is_empty()).then_some(snapshot_funding),
        funding_from_fills,
    );
    let wallet_asset_count = latest_wallets
        .values()
        .fold(0_usize, |acc, wallet| acc + wallet.balances.len());

    Ok(UserAggregate {
        user_id: user_email.to_string(),
        realized_pnl: realized_pnl.normalize(),
        unrealized_pnl: unrealized_pnl.normalize(),
        fees_paid: fees_paid.normalize(),
        funding_total: funding_total.normalize(),
        net_pnl: (realized_pnl + unrealized_pnl - fees_paid + funding_total).normalize(),
        wallet_asset_count,
        exchange_trade_count,
    })
}

fn cost_aggregation(
    fills: &[shared_domain::analytics::FillProfitView],
    trade_history_records: &[ExchangeTradeHistoryRecord],
    latest_account_snapshots: &BTreeMap<String, AccountSnapshotNumbers>,
) -> Result<CostAggregation, SharedDbError> {
    let fill_fees = fills.iter().fold(Decimal::ZERO, |acc, fill| acc + fill.fee);
    let trade_history_fees = trade_history_fee_total(trade_history_records)?;
    let snapshot_fees = latest_account_snapshots
        .values()
        .fold(Decimal::ZERO, |acc, snapshot| acc + snapshot.fees_paid);
    let fees_paid = resolve_fee_total(
        (!latest_account_snapshots.is_empty()).then_some(snapshot_fees),
        trade_history_fees,
        fill_fees,
    );
    let fill_funding = fills
        .iter()
        .fold(Decimal::ZERO, |acc, fill| acc + fill.funding);
    let snapshot_funding = latest_account_snapshots
        .values()
        .fold(Decimal::ZERO, |acc, snapshot| acc + snapshot.funding_total);
    let funding_total = resolve_snapshot_total(
        (!latest_account_snapshots.is_empty()).then_some(snapshot_funding),
        fill_funding,
    );

    Ok(CostAggregation {
        fees_paid: fees_paid.normalize(),
        funding_total: funding_total.normalize(),
    })
}

fn trade_history_fee_total(
    records: &[ExchangeTradeHistoryRecord],
) -> Result<Decimal, SharedDbError> {
    records.iter().try_fold(Decimal::ZERO, |acc, record| {
        let fee = parse_optional_decimal(record.fee_amount.as_deref())?;
        Ok(acc + fee)
    })
}

fn resolve_fee_total(
    snapshot_total: Option<Decimal>,
    trade_history_total: Decimal,
    fill_total: Decimal,
) -> Decimal {
    match snapshot_total {
        Some(value)
            if value != Decimal::ZERO || (trade_history_total == Decimal::ZERO && fill_total == Decimal::ZERO) =>
        {
            value
        }
        _ if trade_history_total != Decimal::ZERO => trade_history_total,
        _ => fill_total,
    }
}

fn resolve_snapshot_total(snapshot_total: Option<Decimal>, fallback_total: Decimal) -> Decimal {
    match snapshot_total {
        Some(value) if value != Decimal::ZERO || fallback_total == Decimal::ZERO => value,
        _ => fallback_total,
    }
}

fn to_strategy_snapshot_views(
    snapshots: &[StrategyProfitSnapshotRecord],
) -> Result<Vec<StrategySnapshotView>, SharedDbError> {
    snapshots
        .iter()
        .map(|snapshot| {
            Ok(StrategySnapshotView {
                strategy_id: snapshot.strategy_id.clone(),
                realized_pnl: parse_decimal(&snapshot.realized_pnl)?.normalize(),
                unrealized_pnl: parse_decimal(&snapshot.unrealized_pnl)?.normalize(),
                fees_paid: parse_decimal(&snapshot.fees)?.normalize(),
                funding_total: parse_optional_decimal(snapshot.funding.as_deref())?.normalize(),
                captured_at: snapshot.captured_at.to_rfc3339(),
            })
        })
        .collect()
}

fn to_account_snapshot_views(
    snapshots: &[AccountProfitSnapshotRecord],
) -> Result<Vec<AccountSnapshotView>, SharedDbError> {
    snapshots
        .iter()
        .map(|snapshot| {
            Ok(AccountSnapshotView {
                exchange: snapshot.exchange.clone(),
                realized_pnl: parse_decimal(&snapshot.realized_pnl)?.normalize(),
                unrealized_pnl: parse_decimal(&snapshot.unrealized_pnl)?.normalize(),
                fees_paid: parse_decimal(&snapshot.fees)?.normalize(),
                funding_total: parse_optional_decimal(snapshot.funding.as_deref())?.normalize(),
                captured_at: snapshot.captured_at.to_rfc3339(),
            })
        })
        .collect()
}

fn to_wallet_snapshot_views(snapshots: &[ExchangeWalletSnapshotRecord]) -> Vec<WalletSnapshotView> {
    snapshots
        .iter()
        .map(|snapshot| WalletSnapshotView {
            exchange: snapshot.exchange.clone(),
            wallet_type: snapshot.wallet_type.clone(),
            balances: json_value_to_string_map(&snapshot.balances),
            captured_at: snapshot.captured_at.to_rfc3339(),
        })
        .collect()
}

fn to_exchange_trade_history_views(
    records: &[ExchangeTradeHistoryRecord],
) -> Vec<ExchangeTradeHistoryView> {
    records
        .iter()
        .map(|record| ExchangeTradeHistoryView {
            trade_id: record.trade_id.clone(),
            exchange: record.exchange.clone(),
            symbol: record.symbol.clone(),
            side: record.side.clone(),
            quantity: record.quantity.clone(),
            price: record.price.clone(),
            fee_amount: record.fee_amount.clone(),
            fee_asset: record.fee_asset.clone(),
            traded_at: record.traded_at.to_rfc3339(),
        })
        .collect()
}

fn latest_strategy_snapshot_map(
    snapshots: &[StrategyProfitSnapshotRecord],
) -> Result<BTreeMap<String, StrategySnapshotNumbers>, SharedDbError> {
    let mut map = BTreeMap::new();
    for snapshot in snapshots {
        map.insert(
            snapshot.strategy_id.clone(),
            StrategySnapshotNumbers {
                realized_pnl: parse_decimal(&snapshot.realized_pnl)?.normalize(),
                unrealized_pnl: parse_decimal(&snapshot.unrealized_pnl)?.normalize(),
                fees_paid: parse_decimal(&snapshot.fees)?.normalize(),
                funding_total: parse_optional_decimal(snapshot.funding.as_deref())?.normalize(),
            },
        );
    }
    Ok(map)
}

fn latest_account_snapshots_by_exchange(
    snapshots: &[AccountProfitSnapshotRecord],
) -> Result<BTreeMap<String, AccountSnapshotNumbers>, SharedDbError> {
    let mut latest = BTreeMap::new();
    for snapshot in snapshots {
        latest.insert(
            snapshot.exchange.clone(),
            AccountSnapshotNumbers {
                realized_pnl: parse_decimal(&snapshot.realized_pnl)?.normalize(),
                unrealized_pnl: parse_decimal(&snapshot.unrealized_pnl)?.normalize(),
                fees_paid: parse_decimal(&snapshot.fees)?.normalize(),
                funding_total: parse_optional_decimal(snapshot.funding.as_deref())?.normalize(),
            },
        );
    }
    Ok(latest)
}

fn latest_wallets_by_exchange(
    snapshots: &[ExchangeWalletSnapshotRecord],
) -> BTreeMap<String, WalletSnapshotView> {
    let mut latest = BTreeMap::new();
    for snapshot in snapshots {
        latest.insert(
            snapshot.exchange.clone(),
            WalletSnapshotView {
                exchange: snapshot.exchange.clone(),
                wallet_type: snapshot.wallet_type.clone(),
                balances: json_value_to_string_map(&snapshot.balances),
                captured_at: snapshot.captured_at.to_rfc3339(),
            },
        );
    }
    latest
}

fn aggregate_position(
    positions: &[StrategyRuntimePosition],
) -> Result<Option<(Decimal, Decimal)>, SharedDbError> {
    let total_quantity = positions
        .iter()
        .fold(Decimal::ZERO, |acc, position| acc + position.quantity);
    if total_quantity.is_zero() {
        return Ok(None);
    }

    let weighted_cost = positions.iter().fold(Decimal::ZERO, |acc, position| {
        acc + (position.quantity * position.average_entry_price)
    });
    Ok(Some((total_quantity, weighted_cost / total_quantity)))
}

fn derive_entry_price(
    exit_price: Decimal,
    quantity: Decimal,
    realized_pnl: Option<Decimal>,
) -> Decimal {
    if quantity.is_zero() {
        return exit_price;
    }

    match realized_pnl {
        Some(realized_pnl) => exit_price - (realized_pnl / quantity),
        None => exit_price,
    }
}

fn payment_row(order: &BillingOrderRecord) -> String {
    format!(
        "{},{},{},{},{},{},{},{},{},{},{}\n",
        order.order_id,
        order.email,
        order.chain,
        order.asset,
        order.plan_code,
        order.amount,
        order.status,
        order
            .assignment
            .as_ref()
            .map(|assignment| assignment.address.clone())
            .unwrap_or_default(),
        order.requested_at.to_rfc3339(),
        order
            .paid_at
            .map(|value| value.to_rfc3339())
            .unwrap_or_default(),
        order.tx_hash.clone().unwrap_or_default(),
    )
}

fn parse_decimal(value: &str) -> Result<Decimal, SharedDbError> {
    value
        .parse()
        .map_err(|error| SharedDbError::new(format!("invalid decimal '{value}': {error}")))
}

fn parse_optional_decimal(value: Option<&str>) -> Result<Decimal, SharedDbError> {
    match value {
        Some(value) => parse_decimal(value),
        None => Ok(Decimal::ZERO),
    }
}

fn format_decimal(value: Decimal) -> String {
    value.normalize().to_string()
}

struct StrategySnapshotNumbers {
    realized_pnl: Decimal,
    unrealized_pnl: Decimal,
    fees_paid: Decimal,
    funding_total: Decimal,
}

struct AccountSnapshotNumbers {
    realized_pnl: Decimal,
    unrealized_pnl: Decimal,
    fees_paid: Decimal,
    funding_total: Decimal,
}

fn json_value_to_string_map(value: &serde_json::Value) -> BTreeMap<String, String> {
    match value {
        serde_json::Value::Object(map) => map
            .iter()
            .map(|(key, value)| (key.clone(), json_value_to_string(value)))
            .collect(),
        _ => BTreeMap::new(),
    }
}

fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Array(values) => values
            .iter()
            .map(json_value_to_string)
            .collect::<Vec<_>>()
            .join(","),
        serde_json::Value::Object(_) => value.to_string(),
    }
}
