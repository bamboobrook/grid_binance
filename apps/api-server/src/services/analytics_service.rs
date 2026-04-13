use std::collections::BTreeMap;

use rust_decimal::Decimal;
use shared_db::{
    AccountProfitSnapshotRecord, BillingOrderRecord, ExchangeTradeHistoryRecord,
    ExchangeWalletSnapshotRecord, SharedDb, SharedDbError, StrategyProfitSnapshotRecord,
};
use shared_domain::analytics::{
    AccountSnapshotView, AnalyticsReport, CostAggregation, ExchangeTradeHistoryView,
    FillProfitView, StrategyProfitSummary, StrategySnapshotView, TradeFillInput, UserAggregate,
    WalletSnapshotView,
};
use shared_domain::strategy::{Strategy, StrategyRuntimePosition, StrategyType};
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

        let fill_inputs = self.trade_fill_inputs(&strategies);
        let fills = compute_fill_views(&fill_inputs);
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
            .map(|strategy| {
                let projected_fills = fills_for_strategy(&fills, &strategy.id);
                strategy_summary(
                    strategy,
                    latest_strategy_snapshots.get(&strategy.id),
                    &projected_fills,
                )
            })
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
        let fill_inputs = self.trade_fill_inputs(&strategies);
        let fills = compute_fill_views(&fill_inputs);
        let summaries = strategies
            .iter()
            .map(|strategy| {
                let projected_fills = fills_for_strategy(&fills, &strategy.id);
                strategy_summary(
                    strategy,
                    latest_strategy_snapshots.get(&strategy.id),
                    &projected_fills,
                )
            })
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
            let order_sides = strategy
                .runtime
                .orders
                .iter()
                .map(|order| (order.order_id.as_str(), order.side.as_str()))
                .collect::<BTreeMap<_, _>>();
            let order_levels = strategy
                .runtime
                .orders
                .iter()
                .filter_map(|order| {
                    order
                        .level_index
                        .map(|level_index| (order.order_id.as_str(), level_index))
                })
                .collect::<BTreeMap<_, _>>();

            if strategy.strategy_type == StrategyType::OrdinaryGrid {
                fills.extend(project_ordinary_level_fill_inputs(
                    strategy,
                    &order_sides,
                    &order_levels,
                ));
                continue;
            }

            fills.extend(
                strategy
                    .runtime
                    .fills
                    .iter()
                    .map(|fill| trade_fill_input(strategy, fill, &order_sides, &order_levels)),
            );
        }
        fills
    }
}

#[derive(Default)]
struct OrdinaryLevelFillProjection {
    level_index: u32,
    entry_quantity: Decimal,
    entry_notional: Decimal,
    exit_quantity: Decimal,
    exit_notional: Decimal,
    realized_pnl: Decimal,
    fee: Decimal,
    is_short: Option<bool>,
}

fn strategy_summary(
    strategy: &Strategy,
    snapshot: Option<&StrategySnapshotNumbers>,
    fills: &[FillProfitView],
) -> Result<StrategyProfitSummary, SharedDbError> {
    let realized_from_fills = fills
        .iter()
        .fold(Decimal::ZERO, |acc, fill| acc + fill.realized_pnl);
    let fees_from_fills = fills.iter().fold(Decimal::ZERO, |acc, fill| acc + fill.fee);

    let position = aggregate_position(&strategy.runtime.positions);
    let cost_basis = position.long_quantity * position.long_average_entry_price
        + position.short_quantity * position.short_average_entry_price;
    let (position_quantity, average_entry_price) = if position.has_single_side() {
        if position.long_quantity.is_zero() {
            (position.short_quantity, position.short_average_entry_price)
        } else {
            (position.long_quantity, position.long_average_entry_price)
        }
    } else {
        (Decimal::ZERO, Decimal::ZERO)
    };
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
        long_position_quantity: position.long_quantity.normalize(),
        long_average_entry_price: position.long_average_entry_price.normalize(),
        short_position_quantity: position.short_quantity.normalize(),
        short_average_entry_price: position.short_average_entry_price.normalize(),
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
            if value != Decimal::ZERO
                || (trade_history_total == Decimal::ZERO && fill_total == Decimal::ZERO) =>
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

fn fills_for_strategy(fills: &[FillProfitView], strategy_id: &str) -> Vec<FillProfitView> {
    fills
        .iter()
        .filter(|fill| fill.strategy_id == strategy_id)
        .cloned()
        .collect()
}

fn project_ordinary_level_fill_inputs(
    strategy: &Strategy,
    order_sides: &BTreeMap<&str, &str>,
    order_levels: &BTreeMap<&str, u32>,
) -> Vec<TradeFillInput> {
    let mut levels = BTreeMap::<u32, OrdinaryLevelFillProjection>::new();
    let mut unscoped_fills = Vec::new();
    let mut startup_level_fallback = first_level_index(strategy);

    for fill in &strategy.runtime.fills {
        let level_index = resolve_fill_level_index(fill, order_levels)
            .or_else(|| fallback_first_ordinary_level(fill, &mut startup_level_fallback));
        let Some(level_index) = level_index else {
            unscoped_fills.push(trade_fill_input(strategy, fill, order_sides, order_levels));
            continue;
        };

        let level = levels.entry(level_index).or_insert_with(|| OrdinaryLevelFillProjection {
            level_index,
            ..OrdinaryLevelFillProjection::default()
        });
        level
            .is_short
            .get_or_insert_with(|| resolve_fill_direction(strategy, fill, order_sides));
        level.fee += fill.fee_amount.unwrap_or(Decimal::ZERO);

        if is_exit_fill(fill) {
            level.exit_quantity += fill.quantity;
            level.exit_notional += fill.price * fill.quantity;
            level.realized_pnl += fill.realized_pnl.unwrap_or(Decimal::ZERO);
        } else {
            level.entry_quantity += fill.quantity;
            level.entry_notional += fill.price * fill.quantity;
        }
    }

    let mut projected = levels
        .into_values()
        .filter_map(|level| ordinary_level_fill_input(strategy, level))
        .collect::<Vec<_>>();
    projected.extend(unscoped_fills);
    projected
}

fn ordinary_level_fill_input(
    strategy: &Strategy,
    level: OrdinaryLevelFillProjection,
) -> Option<TradeFillInput> {
    let quantity = if level.entry_quantity.is_zero() {
        level.exit_quantity
    } else {
        level.entry_quantity
    };
    if quantity.is_zero() {
        return None;
    }

    let is_short = level.is_short.unwrap_or(matches!(
        strategy.mode,
        shared_domain::strategy::StrategyMode::SpotSellOnly
            | shared_domain::strategy::StrategyMode::FuturesShort
    ));
    let entry_price = if level.entry_quantity.is_zero() {
        let exit_price = if level.exit_quantity.is_zero() {
            Decimal::ZERO
        } else {
            level.exit_notional / level.exit_quantity
        };
        derive_entry_price(exit_price, quantity, Some(level.realized_pnl), is_short)
    } else {
        level.entry_notional / level.entry_quantity
    };
    let exit_price = if level.exit_quantity.is_zero() {
        entry_price
    } else {
        level.exit_notional / level.exit_quantity
    };

    Some(TradeFillInput {
        strategy_id: strategy.id.clone(),
        user_id: strategy.owner_email.clone(),
        symbol: strategy.symbol.clone(),
        level_index: Some(level.level_index),
        quantity,
        entry_price,
        exit_price,
        fee: level.fee,
        funding: Decimal::ZERO,
        is_short,
    })
}

fn trade_fill_input(
    strategy: &Strategy,
    fill: &shared_domain::strategy::StrategyRuntimeFill,
    order_sides: &BTreeMap<&str, &str>,
    order_levels: &BTreeMap<&str, u32>,
) -> TradeFillInput {
    let is_short = resolve_fill_direction(strategy, fill, order_sides);

    TradeFillInput {
        strategy_id: strategy.id.clone(),
        user_id: strategy.owner_email.clone(),
        symbol: strategy.symbol.clone(),
        level_index: resolve_fill_level_index(fill, order_levels),
        quantity: fill.quantity,
        entry_price: derive_entry_price(fill.price, fill.quantity, fill.realized_pnl, is_short),
        exit_price: fill.price,
        fee: fill.fee_amount.unwrap_or(Decimal::ZERO),
        funding: Decimal::ZERO,
        is_short,
    }
}

fn is_exit_fill(fill: &shared_domain::strategy::StrategyRuntimeFill) -> bool {
    fill.realized_pnl.is_some() || fill.fill_type.eq_ignore_ascii_case("exit")
}

fn resolve_fill_level_index(
    fill: &shared_domain::strategy::StrategyRuntimeFill,
    order_levels: &BTreeMap<&str, u32>,
) -> Option<u32> {
    fill.level_index.or_else(|| {
        fill.order_id
            .as_deref()
            .and_then(|order_id| order_levels.get(order_id).copied())
    })
}

fn fallback_first_ordinary_level(
    fill: &shared_domain::strategy::StrategyRuntimeFill,
    startup_level_fallback: &mut Option<u32>,
) -> Option<u32> {
    if fill.order_id.is_some() || is_exit_fill(fill) {
        return None;
    }

    startup_level_fallback.take()
}

fn first_level_index(strategy: &Strategy) -> Option<u32> {
    strategy
        .active_revision
        .as_ref()
        .unwrap_or(&strategy.draft_revision)
        .levels
        .first()
        .map(|level| level.level_index)
}

fn aggregate_position(positions: &[StrategyRuntimePosition]) -> PositionAggregation {
    let mut aggregation = PositionAggregation::default();

    for position in positions {
        let weighted_cost = position.quantity * position.average_entry_price;
        if matches!(
            position.mode,
            shared_domain::strategy::StrategyMode::SpotSellOnly
                | shared_domain::strategy::StrategyMode::FuturesShort
        ) {
            aggregation.short_quantity += position.quantity;
            aggregation.short_weighted_cost += weighted_cost;
        } else {
            aggregation.long_quantity += position.quantity;
            aggregation.long_weighted_cost += weighted_cost;
        }
    }

    if !aggregation.long_quantity.is_zero() {
        aggregation.long_average_entry_price =
            aggregation.long_weighted_cost / aggregation.long_quantity;
    }
    if !aggregation.short_quantity.is_zero() {
        aggregation.short_average_entry_price =
            aggregation.short_weighted_cost / aggregation.short_quantity;
    }

    aggregation
}

fn resolve_fill_direction(
    strategy: &Strategy,
    fill: &shared_domain::strategy::StrategyRuntimeFill,
    order_sides: &BTreeMap<&str, &str>,
) -> bool {
    if let Some(order_side) = fill
        .order_id
        .as_deref()
        .and_then(|order_id| order_sides.get(order_id).copied())
    {
        return if fill.realized_pnl.is_some() {
            order_side.eq_ignore_ascii_case("Buy")
        } else {
            order_side.eq_ignore_ascii_case("Sell")
        };
    }

    matches!(
        strategy.mode,
        shared_domain::strategy::StrategyMode::SpotSellOnly
            | shared_domain::strategy::StrategyMode::FuturesShort
    )
}

fn derive_entry_price(
    exit_price: Decimal,
    quantity: Decimal,
    realized_pnl: Option<Decimal>,
    is_short: bool,
) -> Decimal {
    if quantity.is_zero() {
        return exit_price;
    }

    match realized_pnl {
        Some(realized_pnl) if is_short => exit_price + (realized_pnl / quantity),
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

#[derive(Default)]
struct PositionAggregation {
    long_quantity: Decimal,
    long_average_entry_price: Decimal,
    long_weighted_cost: Decimal,
    short_quantity: Decimal,
    short_average_entry_price: Decimal,
    short_weighted_cost: Decimal,
}

impl PositionAggregation {
    fn has_single_side(&self) -> bool {
        self.long_quantity.is_zero() ^ self.short_quantity.is_zero()
    }
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
