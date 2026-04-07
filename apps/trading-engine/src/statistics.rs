use std::collections::BTreeMap;

use rust_decimal::Decimal;
use shared_domain::analytics::{
    AccountSnapshotView, AnalyticsReport, CostAggregation, ExchangeTradeHistoryView,
    FillProfitView, StrategyProfitSummary, StrategySnapshotView, TradeFillInput,
    UserAggregate, WalletSnapshotView,
};

pub fn compute_fill_views(fills: &[TradeFillInput]) -> Vec<FillProfitView> {
    fills.iter().map(project_fill).collect()
}

pub fn compute_strategy_summaries(fills: &[FillProfitView]) -> Vec<StrategyProfitSummary> {
    let mut summaries: BTreeMap<String, StrategyProfitSummary> = BTreeMap::new();

    for fill in fills {
        let entry = summaries
            .entry(fill.strategy_id.clone())
            .or_insert_with(|| StrategyProfitSummary {
                strategy_id: fill.strategy_id.clone(),
                user_id: fill.user_id.clone(),
                symbol: fill.symbol.clone(),
                current_state: "Unknown".to_string(),
                fill_count: 0,
                order_count: 0,
                cost_basis: Decimal::ZERO,
                position_quantity: Decimal::ZERO,
                average_entry_price: Decimal::ZERO,
                realized_pnl: Decimal::ZERO,
                unrealized_pnl: Decimal::ZERO,
                fees_paid: Decimal::ZERO,
                funding_total: Decimal::ZERO,
                net_pnl: Decimal::ZERO,
            });

        entry.fill_count += 1;
        entry.realized_pnl += fill.realized_pnl;
        entry.fees_paid += fill.fee;
        entry.funding_total += fill.funding;
        entry.net_pnl += fill.net_pnl;
    }

    summaries.into_values().collect()
}

pub fn compute_user_aggregate(fills: &[FillProfitView]) -> UserAggregate {
    let user_id = match fills.first() {
        Some(fill)
            if fills
                .iter()
                .all(|candidate| candidate.user_id == fill.user_id) =>
        {
            fill.user_id.clone()
        }
        Some(_) => "all-users".to_string(),
        None => String::new(),
    };

    let mut aggregate = UserAggregate {
        user_id,
        realized_pnl: Decimal::ZERO,
        unrealized_pnl: Decimal::ZERO,
        fees_paid: Decimal::ZERO,
        funding_total: Decimal::ZERO,
        net_pnl: Decimal::ZERO,
        wallet_asset_count: 0,
        exchange_trade_count: 0,
    };

    for fill in fills {
        aggregate.realized_pnl += fill.realized_pnl;
        aggregate.fees_paid += fill.fee;
        aggregate.funding_total += fill.funding;
        aggregate.net_pnl += fill.net_pnl;
    }

    aggregate
}

pub fn compute_cost_aggregation(fills: &[FillProfitView]) -> CostAggregation {
    let mut costs = CostAggregation {
        fees_paid: Decimal::ZERO,
        funding_total: Decimal::ZERO,
    };

    for fill in fills {
        costs.fees_paid += fill.fee;
        costs.funding_total += fill.funding;
    }

    costs
}

pub fn compute_analytics_report(fills: &[TradeFillInput]) -> AnalyticsReport {
    let fill_views = compute_fill_views(fills);
    let strategies = compute_strategy_summaries(&fill_views);
    let user = compute_user_aggregate(&fill_views);
    let costs = compute_cost_aggregation(&fill_views);

    AnalyticsReport {
        fills: fill_views,
        strategies,
        user,
        costs,
        strategy_snapshots: Vec::<StrategySnapshotView>::new(),
        account_snapshots: Vec::<AccountSnapshotView>::new(),
        wallets: Vec::<WalletSnapshotView>::new(),
        exchange_trades: Vec::<ExchangeTradeHistoryView>::new(),
    }
}

pub fn export_fill_views_csv(fills: &[FillProfitView]) -> String {
    let mut csv = String::from(
        "strategy_id,user_id,symbol,quantity,entry_price,exit_price,realized_pnl,fee,funding,net_pnl\n",
    );

    for fill in fills {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{}\n",
            fill.strategy_id,
            fill.user_id,
            fill.symbol,
            fill.quantity.normalize(),
            fill.entry_price.normalize(),
            fill.exit_price.normalize(),
            fill.realized_pnl.normalize(),
            fill.fee.normalize(),
            fill.funding.normalize(),
            fill.net_pnl.normalize()
        ));
    }

    csv
}

fn project_fill(fill: &TradeFillInput) -> FillProfitView {
    let realized_pnl = (fill.exit_price - fill.entry_price) * fill.quantity;
    let net_pnl = realized_pnl - fill.fee + fill.funding;

    FillProfitView {
        strategy_id: fill.strategy_id.clone(),
        user_id: fill.user_id.clone(),
        symbol: fill.symbol.clone(),
        quantity: fill.quantity,
        entry_price: fill.entry_price,
        exit_price: fill.exit_price,
        realized_pnl,
        fee: fill.fee,
        funding: fill.funding,
        net_pnl,
    }
}
