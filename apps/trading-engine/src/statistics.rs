use chrono::Utc;
use std::collections::BTreeMap;
use std::collections::HashSet;
use rust_decimal::Decimal;
use shared_db::{
    AccountProfitSnapshotRecord, ExchangeTradeHistoryRecord, ExchangeWalletSnapshotRecord,
    SharedDb, SharedDbError,
};
use shared_domain::strategy::StrategyMode;
use shared_domain::analytics::{
    AccountSnapshotView, AnalyticsReport, CostAggregation, ExchangeTradeHistoryView,
    FillProfitView, StrategyProfitSummary, StrategySnapshotView, TradeFillInput, UserAggregate,
    WalletSnapshotView,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LiveStatisticsSnapshot {
    pub open_order_count: usize,
    pub position_count: usize,
    pub total_position_notional: String,
    pub realized_pnl: String,
    pub unrealized_pnl: String,
    pub fees_paid: String,
    pub funding_total: String,
    pub wallet_balance: String,
    pub last_user_stream_event_at: Option<String>,
    pub last_rest_reconcile_at: Option<String>,
    pub stats_stale: bool,
    pub computed_at: String,
}

impl Default for LiveStatisticsSnapshot {
    fn default() -> Self {
        Self {
            open_order_count: 0,
            position_count: 0,
            total_position_notional: "0".to_string(),
            realized_pnl: "0".to_string(),
            unrealized_pnl: "0".to_string(),
            fees_paid: "0".to_string(),
            funding_total: "0".to_string(),
            wallet_balance: "0".to_string(),
            last_user_stream_event_at: None,
            last_rest_reconcile_at: None,
            stats_stale: true,
            computed_at: Utc::now().to_rfc3339(),
        }
    }
}

pub fn compute_live_statistics(
    summaries: &[StrategyProfitSummary],
    wallet_balance: Decimal,
    last_stream_event: Option<chrono::DateTime<Utc>>,
    last_rest_reconcile: Option<chrono::DateTime<Utc>>,
    stale_threshold_secs: i64,
) -> LiveStatisticsSnapshot {
    let now = Utc::now();
    let mut snapshot = LiveStatisticsSnapshot::default();

    for summary in summaries {
        snapshot.realized_pnl = (parse_decimal_or_zero(&snapshot.realized_pnl)
            + summary.realized_pnl)
            .normalize()
            .to_string();
        snapshot.unrealized_pnl = (parse_decimal_or_zero(&snapshot.unrealized_pnl)
            + summary.unrealized_pnl)
            .normalize()
            .to_string();
        snapshot.fees_paid = (parse_decimal_or_zero(&snapshot.fees_paid) + summary.fees_paid)
            .normalize()
            .to_string();
        snapshot.funding_total = (parse_decimal_or_zero(&snapshot.funding_total)
            + summary.funding_total)
            .normalize()
            .to_string();
        snapshot.open_order_count += summary.order_count;
    }

    snapshot.wallet_balance = wallet_balance.normalize().to_string();
    snapshot.last_user_stream_event_at = last_stream_event.map(|dt| dt.to_rfc3339());
    snapshot.last_rest_reconcile_at = last_rest_reconcile.map(|dt| dt.to_rfc3339());

    let freshest = last_stream_event
        .into_iter()
        .chain(last_rest_reconcile)
        .max();
    snapshot.stats_stale = match freshest {
        Some(fresh) => (now - fresh).num_seconds() > stale_threshold_secs,
        None => true,
    };
    snapshot.computed_at = now.to_rfc3339();

    snapshot
}

pub fn compute_live_statistics_from_db(
    db: &SharedDb,
    email: &str,
    strategy_ids: Option<&[String]>,
    stale_threshold_secs: i64,
) -> Result<LiveStatisticsSnapshot, SharedDbError> {
    let all_strategies = db.list_strategies(email)?;

    let strategies: Vec<&shared_domain::strategy::Strategy> = match strategy_ids {
        Some(ids) => {
            let id_set: HashSet<&str> = ids.iter().map(|s| s.as_str()).collect();
            all_strategies.iter().filter(|s| id_set.contains(s.id.as_str())).collect()
        }
        None => all_strategies.iter().collect(),
    };

    let trade_history = db.list_exchange_trade_history(email)?;
    let account_snapshots = db.list_account_profit_snapshots(email)?;
    let wallet_snapshots = db.list_exchange_wallet_snapshots(email)?;

    let trade_fees = sum_trade_history_fees(&trade_history);
    let (account_realized, account_unrealized, mut account_fees, account_funding) =
        merge_latest_account_fields(&account_snapshots);
    if account_fees == Decimal::ZERO && trade_fees != Decimal::ZERO {
        account_fees = trade_fees;
    }
    let wallet_balance = latest_wallet_balance_usdt(&wallet_snapshots);

    let (last_stream_event_at, last_rest_reconcile_at) =
        extract_sync_timestamps(&strategies);

    let summaries: Vec<StrategyProfitSummary> = strategies
        .iter()
        .map(|strategy| {
            StrategyProfitSummary {
                strategy_id: strategy.id.clone(),
                user_id: email.to_string(),
                symbol: strategy.symbol.clone(),
                current_state: format!("{:?}", strategy.status),
                fill_count: strategy.runtime.fills.len(),
                order_count: strategy.runtime.orders.len(),
                cost_basis: strategy
                    .runtime
                    .positions
                    .iter()
                    .map(|p| p.average_entry_price * p.quantity.abs())
                    .sum(),
                position_quantity: strategy
                    .runtime
                    .positions
                    .iter()
                    .map(|p| p.quantity)
                    .sum(),
                average_entry_price: strategy
                    .runtime
                    .positions
                    .first()
                    .map(|p| p.average_entry_price)
                    .unwrap_or(Decimal::ZERO),
                long_position_quantity: strategy
                    .runtime
                    .positions
                    .iter()
                    .filter(|p| p.mode == StrategyMode::FuturesLong)
                    .map(|p| p.quantity)
                    .sum(),
                long_average_entry_price: strategy
                    .runtime
                    .positions
                    .iter()
                    .filter(|p| p.mode == StrategyMode::FuturesLong)
                    .map(|p| p.average_entry_price)
                    .next()
                    .unwrap_or(Decimal::ZERO),
                short_position_quantity: strategy
                    .runtime
                    .positions
                    .iter()
                    .filter(|p| p.mode == StrategyMode::FuturesShort)
                    .map(|p| p.quantity)
                    .sum(),
                short_average_entry_price: strategy
                    .runtime
                    .positions
                    .iter()
                    .filter(|p| p.mode == StrategyMode::FuturesShort)
                    .map(|p| p.average_entry_price)
                    .next()
                    .unwrap_or(Decimal::ZERO),
                realized_pnl: Decimal::ZERO,
                unrealized_pnl: Decimal::ZERO,
                fees_paid: Decimal::ZERO,
                funding_total: Decimal::ZERO,
                net_pnl: Decimal::ZERO,
            }
        })
        .collect();

    let mut snapshot = compute_live_statistics(
        &summaries,
        wallet_balance,
        last_stream_event_at,
        last_rest_reconcile_at,
        stale_threshold_secs,
    );

    snapshot.realized_pnl = account_realized.normalize().to_string();
    snapshot.unrealized_pnl = account_unrealized.normalize().to_string();
    snapshot.fees_paid = account_fees.normalize().to_string();
    snapshot.funding_total = account_funding.normalize().to_string();

    Ok(snapshot)
}

pub fn compute_position_count_for_strategies(
    db: &SharedDb,
    email: &str,
    strategy_ids: Option<&[String]>,
) -> Result<usize, SharedDbError> {
    let all_strategies = db.list_strategies(email)?;
    let filtered: Vec<&shared_domain::strategy::Strategy> = match strategy_ids {
        Some(ids) => {
            let id_set: HashSet<&str> = ids.iter().map(|s| s.as_str()).collect();
            all_strategies.iter().filter(|s| id_set.contains(s.id.as_str())).collect()
        }
        None => all_strategies.iter().collect(),
    };
    Ok(filtered.iter().filter(|s| !s.runtime.positions.is_empty()).count())
}

fn sum_trade_history_fees(trades: &[ExchangeTradeHistoryRecord]) -> Decimal {
    trades
        .iter()
        .filter_map(|t| {
            t.fee_amount
                .as_deref()
                .and_then(|f| f.parse::<Decimal>().ok())
        })
        .sum()
}

fn latest_wallet_balance_usdt(
    snapshots: &[ExchangeWalletSnapshotRecord],
) -> Decimal {
    snapshots
        .iter()
        .max_by_key(|s| s.captured_at)
        .and_then(|s| s.balances.get("USDT"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Decimal>().ok())
        .unwrap_or(Decimal::ZERO)
}

fn merge_latest_account_fields(
    snapshots: &[AccountProfitSnapshotRecord],
) -> (Decimal, Decimal, Decimal, Decimal) {
    let mut realized = Decimal::ZERO;
    let mut unrealized = Decimal::ZERO;
    let mut fees = Decimal::ZERO;
    let mut funding = Decimal::ZERO;
    let mut realized_ts: Option<chrono::DateTime<Utc>> = None;
    let mut unrealized_ts: Option<chrono::DateTime<Utc>> = None;
    let mut fees_ts: Option<chrono::DateTime<Utc>> = None;
    let mut funding_ts: Option<chrono::DateTime<Utc>> = None;

    for snapshot in snapshots {
        let captured = snapshot.captured_at;
        let field_realized = parse_decimal_or_zero(&snapshot.realized_pnl);
        let field_unrealized = parse_decimal_or_zero(&snapshot.unrealized_pnl);
        let field_fees = parse_decimal_or_zero(&snapshot.fees);
        let field_funding = snapshot
            .funding
            .as_deref()
            .map(|f| parse_decimal_or_zero(f))
            .unwrap_or(Decimal::ZERO);

        if field_realized != Decimal::ZERO
            && realized_ts.map_or(true, |ts| captured > ts)
        {
            realized = field_realized;
            realized_ts = Some(captured);
        }
        if field_unrealized != Decimal::ZERO
            && unrealized_ts.map_or(true, |ts| captured > ts)
        {
            unrealized = field_unrealized;
            unrealized_ts = Some(captured);
        }
        if field_fees != Decimal::ZERO
            && fees_ts.map_or(true, |ts| captured > ts)
        {
            fees = field_fees;
            fees_ts = Some(captured);
        }
        if field_funding != Decimal::ZERO
            && funding_ts.map_or(true, |ts| captured > ts)
        {
            funding = field_funding;
            funding_ts = Some(captured);
        }
    }

    (realized, unrealized, fees, funding)
}

fn extract_sync_timestamps(
    strategies: &[&shared_domain::strategy::Strategy],
) -> (Option<chrono::DateTime<Utc>>, Option<chrono::DateTime<Utc>>) {
    let mut last_stream: Option<chrono::DateTime<Utc>> = None;
    let mut last_rest: Option<chrono::DateTime<Utc>> = None;

    for strategy in strategies {
        for event in &strategy.runtime.events {
            match event.event_type.as_str() {
                "last_stream_event_at" => {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&event.detail) {
                        let dt = dt.with_timezone(&Utc);
                        last_stream = Some(match last_stream {
                            Some(existing) if dt > existing => dt,
                            Some(existing) => existing,
                            None => dt,
                        });
                    }
                }
                "last_rest_reconcile_at" => {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&event.detail) {
                        let dt = dt.with_timezone(&Utc);
                        last_rest = Some(match last_rest {
                            Some(existing) if dt > existing => dt,
                            Some(existing) => existing,
                            None => dt,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    (last_stream, last_rest)
}

fn parse_decimal_or_zero(value: &str) -> Decimal {
    value.parse::<Decimal>().unwrap_or(Decimal::ZERO)
}

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
                long_position_quantity: Decimal::ZERO,
                long_average_entry_price: Decimal::ZERO,
                short_position_quantity: Decimal::ZERO,
                short_average_entry_price: Decimal::ZERO,
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
    let realized_pnl = if fill.is_short {
        (fill.entry_price - fill.exit_price) * fill.quantity
    } else {
        (fill.exit_price - fill.entry_price) * fill.quantity
    };
    let net_pnl = realized_pnl - fill.fee + fill.funding;

    FillProfitView {
        strategy_id: fill.strategy_id.clone(),
        user_id: fill.user_id.clone(),
        symbol: fill.symbol.clone(),
        level_index: fill.level_index,
        quantity: fill.quantity,
        entry_price: fill.entry_price,
        exit_price: fill.exit_price,
        realized_pnl,
        fee: fill.fee,
        funding: fill.funding,
        net_pnl,
    }
}

#[cfg(test)]
mod tests {
    use super::{compute_fill_views, compute_live_statistics, compute_live_statistics_from_db};
    use chrono::Utc;
    use rust_decimal::Decimal;
    use shared_db::{
        AccountProfitSnapshotRecord, ExchangeWalletSnapshotRecord, SharedDb,
    };
    use shared_domain::analytics::{StrategyProfitSummary, TradeFillInput};
    use shared_domain::strategy::{
        Strategy, StrategyRuntime, StrategyRuntimeEvent, StrategyRuntimeOrder,
        StrategyRuntimePosition, StrategyStatus, StrategyType, StrategyMarket,
        StrategyMode, StrategyRevision, GridGeneration, StrategyAmountMode,
        GridLevel, ReferencePriceSource, PostTriggerAction,
    };

    #[test]
    fn compute_fill_views_projects_short_realized_pnl_with_short_formula() {
        let fills = compute_fill_views(&[TradeFillInput {
            strategy_id: "strategy-short".to_string(),
            user_id: "trader@example.com".to_string(),
            symbol: "BTCUSDT".to_string(),
            level_index: Some(3),
            quantity: Decimal::new(2, 0),
            entry_price: Decimal::new(100, 0),
            exit_price: Decimal::new(90, 0),
            fee: Decimal::new(4, 1),
            funding: Decimal::ZERO,
            is_short: true,
        }]);

        assert_eq!(fills[0].level_index, Some(3));
        assert_eq!(fills[0].realized_pnl, Decimal::new(20, 0));
        assert_eq!(fills[0].net_pnl, Decimal::new(196, 1));
    }

    #[test]
    fn live_statistics_sums_realized_and_unrealized_pnl() {
        let summaries = vec![
            StrategyProfitSummary {
                strategy_id: "s1".to_string(),
                user_id: "u1".to_string(),
                symbol: "BTCUSDT".to_string(),
                current_state: "Running".to_string(),
                fill_count: 1,
                order_count: 3,
                cost_basis: Decimal::ZERO,
                position_quantity: Decimal::ZERO,
                average_entry_price: Decimal::ZERO,
                long_position_quantity: Decimal::ZERO,
                long_average_entry_price: Decimal::ZERO,
                short_position_quantity: Decimal::ZERO,
                short_average_entry_price: Decimal::ZERO,
                realized_pnl: Decimal::new(100, 0),
                unrealized_pnl: Decimal::new(50, 0),
                fees_paid: Decimal::new(5, 0),
                funding_total: Decimal::new(2, 0),
                net_pnl: Decimal::new(147, 0),
            },
            StrategyProfitSummary {
                strategy_id: "s2".to_string(),
                user_id: "u1".to_string(),
                symbol: "ETHUSDT".to_string(),
                current_state: "Running".to_string(),
                fill_count: 2,
                order_count: 5,
                cost_basis: Decimal::ZERO,
                position_quantity: Decimal::ZERO,
                average_entry_price: Decimal::ZERO,
                long_position_quantity: Decimal::ZERO,
                long_average_entry_price: Decimal::ZERO,
                short_position_quantity: Decimal::ZERO,
                short_average_entry_price: Decimal::ZERO,
                realized_pnl: Decimal::new(-20, 0),
                unrealized_pnl: Decimal::new(10, 0),
                fees_paid: Decimal::new(3, 0),
                funding_total: Decimal::new(1, 0),
                net_pnl: Decimal::new(-12, 0),
            },
        ];

        let snapshot = compute_live_statistics(
            &summaries,
            Decimal::new(1000, 0),
            None,
            None,
            600,
        );

        assert_eq!(snapshot.realized_pnl, "80");
        assert_eq!(snapshot.unrealized_pnl, "60");
        assert_eq!(snapshot.fees_paid, "8");
        assert_eq!(snapshot.funding_total, "3");
        assert_eq!(snapshot.open_order_count, 8);
        assert_eq!(snapshot.wallet_balance, "1000");
        assert!(snapshot.stats_stale, "no sync timestamps");
    }

    #[test]
    fn live_statistics_mark_fresh_when_sync_is_recent() {
        let summaries: Vec<StrategyProfitSummary> = vec![];
        let now = Utc::now();
        let snapshot = compute_live_statistics(
            &summaries,
            Decimal::ZERO,
            Some(now),
            None,
            600,
        );

        assert!(!snapshot.stats_stale, "sync just happened");
        assert_eq!(snapshot.last_user_stream_event_at, Some(now.to_rfc3339()));
        assert_eq!(snapshot.last_rest_reconcile_at, None);
    }

    #[test]
    fn live_statistics_mark_stale_when_exceeds_threshold() {
        let summaries: Vec<StrategyProfitSummary> = vec![];
        let old = Utc::now() - chrono::Duration::seconds(601);
        let snapshot = compute_live_statistics(
            &summaries,
            Decimal::ZERO,
            Some(old),
            None,
            600,
        );

        assert!(snapshot.stats_stale, "sync older than 600s threshold");
    }

    fn make_test_strategy(id: &str, email: &str, symbol: &str, order_count: usize) -> Strategy {
        let revision = StrategyRevision {
            revision_id: format!("rev-{}", id),
            version: 1,
            strategy_type: StrategyType::OrdinaryGrid,
            generation: GridGeneration::Arithmetic,
            levels: vec![GridLevel {
                level_index: 0,
                entry_price: Decimal::new(50000, 0),
                quantity: Decimal::new(1, 2),
                take_profit_bps: 200,
                trailing_bps: None,
            }],
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            reference_price_source: ReferencePriceSource::Manual,
            reference_price: None,
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        };
        let mut orders = Vec::new();
        for i in 0..order_count {
            orders.push(StrategyRuntimeOrder {
                order_id: format!("order-{}-{}", id, i),
                exchange_order_id: None,
                level_index: Some(i as u32),
                side: "Buy".to_string(),
                order_type: "Limit".to_string(),
                price: Some(Decimal::new(500, 2)),
                quantity: Decimal::new(1, 1),
                status: "Placed".to_string(),
            });
        }
        Strategy {
            id: id.to_string(),
            owner_email: email.to_string(),
            name: format!("Strategy {}", id),
            symbol: symbol.to_string(),
            budget: "10000".to_string(),
            grid_spacing_bps: 50,
            status: StrategyStatus::Running,
            source_template_id: None,
            membership_ready: true,
            exchange_ready: true,
            permissions_ready: true,
            withdrawals_disabled: false,
            hedge_mode_ready: true,
            symbol_ready: true,
            filters_ready: true,
            margin_ready: true,
            conflict_ready: true,
            balance_ready: true,
            strategy_type: StrategyType::OrdinaryGrid,
            market: StrategyMarket::FuturesUsdM,
            mode: StrategyMode::FuturesLong,
            runtime_phase: Default::default(),
            runtime_controls: Default::default(),
            draft_revision: revision,
            tags: vec![],
            notes: String::new(),
            active_revision: None,
            runtime: StrategyRuntime {
                positions: vec![StrategyRuntimePosition {
                    market: StrategyMarket::FuturesUsdM,
                    mode: StrategyMode::FuturesLong,
                    quantity: Decimal::new(1, 1),
                    average_entry_price: Decimal::new(500, 2),
                }],
                orders,
                fills: vec![],
                events: vec![StrategyRuntimeEvent {
                    event_type: "last_stream_event_at".to_string(),
                    detail: Utc::now().to_rfc3339(),
                    price: None,
                    created_at: Utc::now(),
                }],
                last_preflight: None,
            },
            archived_at: None,
        }
    }

    #[test]
    fn portfolio_scoped_stats_only_include_given_strategy_ids() {
        let db = SharedDb::ephemeral().expect("ephemeral db");
        let email = "owner@test.com";
        let now = Utc::now();

        let strategy_a = make_test_strategy("strat-a", email, "BTCUSDT", 3);
        let strategy_b = make_test_strategy("strat-b", email, "ETHUSDT", 7);

        db.insert_strategy(&shared_db::StoredStrategy { sequence_id: 1, strategy: strategy_a })
            .expect("insert A");
        db.insert_strategy(&shared_db::StoredStrategy { sequence_id: 2, strategy: strategy_b })
            .expect("insert B");

        let _ = db.insert_account_profit_snapshot(&AccountProfitSnapshotRecord {
            user_email: email.to_string(),
            exchange: "binance".to_string(),
            realized_pnl: "200".to_string(),
            unrealized_pnl: "50".to_string(),
            fees: "10".to_string(),
            funding: Some("-2".to_string()),
            captured_at: now,
        });

        let mut balances = serde_json::Map::new();
        balances.insert("USDT".to_string(), serde_json::json!("5000"));
        let _ = db.insert_exchange_wallet_snapshot(&ExchangeWalletSnapshotRecord {
            user_email: email.to_string(),
            exchange: "binance".to_string(),
            wallet_type: "futures".to_string(),
            balances: serde_json::Value::Object(balances),
            captured_at: now,
        });

        let stats = compute_live_statistics_from_db(
            &db, email, Some(&["strat-a".to_string()]), 600,
        )
        .expect("compute");

        assert_eq!(stats.open_order_count, 3, "only strategy A orders (3), not B (7)");
        assert_eq!(stats.realized_pnl, "200", "account-level realized_pnl injected once");
        assert_eq!(stats.unrealized_pnl, "50");
        assert_eq!(stats.fees_paid, "10");
        assert_eq!(stats.funding_total, "-2");
        assert_eq!(stats.wallet_balance, "5000");
        assert!(!stats.stats_stale, "sync timestamp from strategy A");
    }

    #[test]
    fn multi_strategy_portfolio_does_not_multiply_account_level_pnl() {
        let db = SharedDb::ephemeral().expect("ephemeral db");
        let email = "multi@test.com";
        let now = Utc::now();

        db.insert_strategy(&shared_db::StoredStrategy {
            sequence_id: 1,
            strategy: make_test_strategy("s1", email, "BTCUSDT", 2),
        }).expect("insert");
        db.insert_strategy(&shared_db::StoredStrategy {
            sequence_id: 2,
            strategy: make_test_strategy("s2", email, "ETHUSDT", 4),
        }).expect("insert");
        db.insert_strategy(&shared_db::StoredStrategy {
            sequence_id: 3,
            strategy: make_test_strategy("s3", email, "SOLUSDT", 1),
        }).expect("insert");

        let _ = db.insert_account_profit_snapshot(&AccountProfitSnapshotRecord {
            user_email: email.to_string(),
            exchange: "binance".to_string(),
            realized_pnl: "300".to_string(),
            unrealized_pnl: "100".to_string(),
            fees: "15".to_string(),
            funding: Some("-5".to_string()),
            captured_at: now,
        });

        let mut balances = serde_json::Map::new();
        balances.insert("USDT".to_string(), serde_json::json!("8000"));
        let _ = db.insert_exchange_wallet_snapshot(&ExchangeWalletSnapshotRecord {
            user_email: email.to_string(),
            exchange: "binance".to_string(),
            wallet_type: "futures".to_string(),
            balances: serde_json::Value::Object(balances),
            captured_at: now,
        });

        let stats = compute_live_statistics_from_db(
            &db,
            email,
            Some(&["s1".to_string(), "s2".to_string(), "s3".to_string()]),
            600,
        )
        .expect("compute");

        assert_eq!(stats.open_order_count, 7, "2+4+1 = 7 strategy-level orders (correctly summed)");
        assert_eq!(stats.realized_pnl, "300", "account-level PnL should be 300, NOT 3x300=900");
        assert_eq!(stats.unrealized_pnl, "100");
        assert_eq!(stats.fees_paid, "15");
        assert_eq!(stats.funding_total, "-5");
        assert_eq!(stats.wallet_balance, "8000");
    }
}
