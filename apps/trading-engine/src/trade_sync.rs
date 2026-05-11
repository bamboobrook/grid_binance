use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use shared_binance::{BinanceClient, BinanceExecutionUpdate, BinanceUserTrade};
use shared_db::{ExchangeTradeHistoryRecord, NotificationLogRecord, SharedDb};
use shared_domain::strategy::{Strategy, StrategyRuntimeEvent, StrategyType};

use crate::execution_sync::{apply_execution_update, finalize_strategy_after_close};
use crate::telegram_notify::{persist_telegram_notification, telegram_bot_token};
use std::collections::{BTreeMap, HashSet};

pub trait BinanceTradeGateway {
    fn user_trades(
        &self,
        market: &str,
        symbol: &str,
        limit: usize,
    ) -> Result<Vec<BinanceUserTrade>, String>;

    fn get_order(
        &self,
        market: &str,
        symbol: &str,
        order_id: &str,
    ) -> Result<shared_binance::BinanceOrderResponse, String>;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TradeSyncResult {
    pub new_fills: usize,
}

pub fn sync_strategy_trades(
    db: &SharedDb,
    strategy: &mut Strategy,
    gateway: &impl BinanceTradeGateway,
) -> Result<TradeSyncResult, shared_db::SharedDbError> {
    let existing_fill_ids = strategy
        .runtime
        .fills
        .iter()
        .map(|fill| fill.fill_id.clone())
        .collect::<HashSet<_>>();
    let mut known_trade_ids = db
        .list_exchange_trade_history(&strategy.owner_email)?
        .into_iter()
        .map(|trade| trade.trade_id)
        .collect::<HashSet<_>>();
    let trades = gateway
        .user_trades(market_scope(strategy), &strategy.symbol, 100)
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    let mut grouped = BTreeMap::<String, Vec<BinanceUserTrade>>::new();
    for trade in trades {
        let Some(order_id) = trade.order_id.clone() else {
            continue;
        };
        grouped.entry(order_id).or_default().push(trade);
    }

    let mut new_fills = 0usize;
    for (order_id, order_trades) in grouped {
        let Some(order) = strategy
            .runtime
            .orders
            .iter()
            .find(|order| order.exchange_order_id.as_deref() == Some(order_id.as_str()))
            .cloned()
        else {
            continue;
        };
        if strategy.strategy_type == StrategyType::MartingaleGrid
            && !is_martingale_client_order(&order.order_id)
        {
            continue;
        }

        let new_trades = order_trades
            .iter()
            .filter(|trade| {
                let fill_id = format!("exchange-trade-{}", trade.trade_id);
                !existing_fill_ids.contains(&fill_id) && !known_trade_ids.contains(&trade.trade_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        if new_trades.is_empty() {
            continue;
        }

        let remote_order = gateway
            .get_order(market_scope(strategy), &strategy.symbol, &order_id)
            .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
        if !matches!(
            remote_order.status.to_ascii_uppercase().as_str(),
            "FILLED" | "PARTIALLY_FILLED"
        ) {
            continue;
        }

        let aggregate = aggregate_trade_group(&new_trades)?;
        let changed = apply_execution_update(
            strategy,
            &BinanceExecutionUpdate {
                market: aggregate.market.clone(),
                symbol: aggregate.symbol.clone(),
                order_id: order_id.clone(),
                client_order_id: Some(order.order_id.clone()),
                side: Some(aggregate.side.clone()),
                order_type: Some(order.order_type.clone()),
                status: remote_order.status,
                execution_type: Some("TRADE".to_string()),
                order_price: order.price.map(|value| value.normalize().to_string()),
                last_fill_price: Some(aggregate.price.clone()),
                last_fill_quantity: Some(aggregate.quantity.clone()),
                cumulative_fill_quantity: Some(aggregate.quantity.clone()),
                fee_amount: aggregate.fee_amount.clone(),
                fee_asset: aggregate.fee_asset.clone(),
                position_side: None,
                trade_id: Some(aggregate.trade_id.clone()),
                realized_profit: aggregate.realized_profit.clone(),
                event_time_ms: aggregate.traded_at_ms,
            },
        );
        if !changed {
            continue;
        }
        if strategy.strategy_type == StrategyType::MartingaleGrid {
            append_martingale_safety_leg(strategy, &order, &aggregate)?;
        }

        let traded_at = Utc
            .timestamp_millis_opt(aggregate.traded_at_ms)
            .single()
            .unwrap_or_else(Utc::now);
        let price = parse_decimal(&aggregate.price)?;
        let fee_amount = aggregate
            .fee_amount
            .as_deref()
            .map(parse_decimal)
            .transpose()?;
        let realized_pnl = strategy
            .runtime
            .fills
            .iter()
            .rev()
            .find(|fill| fill.fill_id == exchange_trade_fill_id(&aggregate.trade_id))
            .and_then(|fill| fill.realized_pnl)
            .or_else(|| {
                aggregate
                    .realized_profit
                    .as_deref()
                    .map(parse_decimal)
                    .transpose()
                    .ok()
                    .flatten()
            });

        strategy.runtime.events.push(StrategyRuntimeEvent {
            event_type: "grid_fill_executed".to_string(),
            detail: format!(
                "grid fill {} executed at {}",
                aggregate.trade_id, aggregate.price
            ),
            price: Some(price),
            created_at: traded_at,
        });

        for trade in &new_trades {
            let trade_at = Utc
                .timestamp_millis_opt(trade.traded_at_ms)
                .single()
                .unwrap_or_else(Utc::now);
            db.insert_exchange_trade_history(&ExchangeTradeHistoryRecord {
                trade_id: trade.trade_id.clone(),
                user_email: strategy.owner_email.clone(),
                exchange: "binance".to_string(),
                symbol: trade.symbol.clone(),
                side: trade.side.clone(),
                quantity: trade.quantity.clone(),
                price: trade.price.clone(),
                fee_amount: trade.fee_amount.clone(),
                fee_asset: trade.fee_asset.clone(),
                traded_at: trade_at,
            })?;
            known_trade_ids.insert(trade.trade_id.clone());
        }

        let title = format!("Grid fill {}", aggregate.symbol);
        let body = format!("{} grid filled at {}.", aggregate.symbol, aggregate.price);
        let payload = json!({
            "trade_id": aggregate.trade_id,
            "order_id": order.order_id,
            "symbol": aggregate.symbol,
            "price": aggregate.price,
            "quantity": aggregate.quantity,
            "fee_amount": aggregate.fee_amount,
            "fee_asset": aggregate.fee_asset,
        });
        db.insert_notification_log(&NotificationLogRecord {
            user_email: strategy.owner_email.clone(),
            channel: "in_app".to_string(),
            template_key: Some("GridFillExecuted".to_string()),
            title: title.clone(),
            body: body.clone(),
            status: "delivered".to_string(),
            payload: payload.clone(),
            created_at: traded_at,
            delivered_at: Some(traded_at),
        })?;
        let fill_profit_title = format!("Fill profit {}", aggregate.symbol);
        let realized_pnl = realized_pnl.unwrap_or(Decimal::ZERO);
        let net_pnl = realized_pnl - fee_amount.unwrap_or(Decimal::ZERO);
        let cumulative_net_pnl = strategy
            .runtime
            .fills
            .iter()
            .fold(Decimal::ZERO, |acc, fill| {
                acc + fill.realized_pnl.unwrap_or(Decimal::ZERO)
                    - fill.fee_amount.unwrap_or(Decimal::ZERO)
            });
        let fill_profit_body = format!("Grid fill realized {} net PnL.", net_pnl.normalize());
        let fill_profit_payload = json!({
            "trade_id": aggregate.trade_id,
            "symbol": aggregate.symbol,
            "realized_pnl": realized_pnl.normalize().to_string(),
            "net_pnl": net_pnl.normalize().to_string(),
            "cumulative_net_pnl": cumulative_net_pnl.normalize().to_string(),
        });
        db.insert_notification_log(&NotificationLogRecord {
            user_email: strategy.owner_email.clone(),
            channel: "in_app".to_string(),
            template_key: Some("FillProfitReported".to_string()),
            title: fill_profit_title.clone(),
            body: fill_profit_body.clone(),
            status: "delivered".to_string(),
            payload: fill_profit_payload.clone(),
            created_at: traded_at,
            delivered_at: Some(traded_at),
        })?;
        persist_telegram_trade_notification(
            db,
            strategy,
            &order,
            &aggregate,
            net_pnl,
            cumulative_net_pnl,
            traded_at,
        )?;
        let reference_price = parse_decimal(&aggregate.price).ok();
        finalize_strategy_after_close(strategy, reference_price);
        new_fills += 1;
    }

    Ok(TradeSyncResult { new_fills })
}

fn append_martingale_safety_leg(
    strategy: &mut Strategy,
    filled_order: &shared_domain::strategy::StrategyRuntimeOrder,
    aggregate: &AggregatedTradeGroup,
) -> Result<(), shared_db::SharedDbError> {
    let Some(leg_index) = filled_order.level_index else {
        return Ok(());
    };
    let next_leg_index = leg_index + 1;
    if strategy
        .runtime
        .orders
        .iter()
        .any(|order| order.level_index == Some(next_leg_index) && order.order_id.starts_with("mg-"))
    {
        return Ok(());
    }
    let fill_price = parse_decimal(&aggregate.price)?;
    let step = fill_price * Decimal::from(strategy.grid_spacing_bps) / Decimal::from(10_000u32);
    let is_short = filled_order.side.eq_ignore_ascii_case("Sell");
    let next_price = if is_short {
        fill_price + step
    } else {
        fill_price - step
    };
    if next_price <= Decimal::ZERO {
        return Ok(());
    }
    let next_order_id = next_martingale_order_id(&filled_order.order_id, next_leg_index);
    strategy
        .runtime
        .orders
        .push(shared_domain::strategy::StrategyRuntimeOrder {
            order_id: next_order_id,
            exchange_order_id: None,
            level_index: Some(next_leg_index),
            side: filled_order.side.clone(),
            order_type: "Limit".to_string(),
            price: Some(next_price.normalize()),
            quantity: filled_order.quantity,
            status: "Working".to_string(),
        });
    Ok(())
}

fn next_martingale_order_id(order_id: &str, next_leg_index: u32) -> String {
    if let Some(prefix) = order_id.rsplit_once("-leg-").map(|(prefix, _)| prefix) {
        format!("{prefix}-leg-{next_leg_index}")
    } else {
        format!("{order_id}-leg-{next_leg_index}")
    }
}

fn is_martingale_client_order(order_id: &str) -> bool {
    order_id.starts_with("mg-") && order_id.contains("-leg-")
}

fn exchange_trade_fill_id(trade_id: &str) -> String {
    format!("exchange-trade-{trade_id}")
}

fn persist_telegram_trade_notification(
    db: &SharedDb,
    strategy: &Strategy,
    order: &shared_domain::strategy::StrategyRuntimeOrder,
    aggregate: &AggregatedTradeGroup,
    net_pnl: Decimal,
    cumulative_net_pnl: Decimal,
    traded_at: chrono::DateTime<Utc>,
) -> Result<(), shared_db::SharedDbError> {
    let (template_key, title, body) =
        telegram_trade_message(strategy, order, aggregate, net_pnl, cumulative_net_pnl);
    let payload = json!({
        "trade_id": aggregate.trade_id,
        "order_id": order.order_id,
        "symbol": aggregate.symbol,
        "price": aggregate.price,
        "quantity": aggregate.quantity,
        "net_pnl": net_pnl.normalize().to_string(),
        "cumulative_net_pnl": cumulative_net_pnl.normalize().to_string(),
    });
    persist_telegram_notification(db, strategy, template_key, title, body, payload, traded_at)
}

fn telegram_trade_message(
    strategy: &Strategy,
    order: &shared_domain::strategy::StrategyRuntimeOrder,
    aggregate: &AggregatedTradeGroup,
    net_pnl: Decimal,
    cumulative_net_pnl: Decimal,
) -> (&'static str, String, String) {
    if is_close_trade_order(order) {
        (
            "FillProfitReported",
            format!("平仓成交 {}", aggregate.symbol),
            format!(
                "策略：{}
交易对：{}
方向：{}
成交价：{}
成交数量：{}
本格净收益：{}
策略累计净收益：{}",
                strategy.name,
                aggregate.symbol,
                describe_trade_direction(strategy, order, true),
                aggregate.price,
                aggregate.quantity,
                net_pnl.normalize(),
                cumulative_net_pnl.normalize(),
            ),
        )
    } else {
        (
            "GridFillExecuted",
            format!("开仓成交 {}", aggregate.symbol),
            format!(
                "策略：{}
交易对：{}
方向：{}
成交价：{}
成交数量：{}",
                strategy.name,
                aggregate.symbol,
                describe_trade_direction(strategy, order, false),
                aggregate.price,
                aggregate.quantity,
            ),
        )
    }
}

fn is_close_trade_order(order: &shared_domain::strategy::StrategyRuntimeOrder) -> bool {
    order.order_id.contains("-tp-") || order.order_id.contains("-stop-close-")
}

fn describe_trade_direction(
    strategy: &Strategy,
    order: &shared_domain::strategy::StrategyRuntimeOrder,
    is_close: bool,
) -> &'static str {
    match strategy.market {
        shared_domain::strategy::StrategyMarket::Spot => {
            if is_close {
                "现货卖出"
            } else {
                "现货买入"
            }
        }
        shared_domain::strategy::StrategyMarket::FuturesUsdM
        | shared_domain::strategy::StrategyMarket::FuturesCoinM => match strategy.mode {
            shared_domain::strategy::StrategyMode::FuturesShort => {
                if is_close {
                    "合约空单平仓"
                } else {
                    "合约空单开仓"
                }
            }
            shared_domain::strategy::StrategyMode::FuturesNeutral => {
                if order.side.eq_ignore_ascii_case("Sell") {
                    if is_close {
                        "合约空单平仓"
                    } else {
                        "合约空单开仓"
                    }
                } else if is_close {
                    "合约多单平仓"
                } else {
                    "合约多单开仓"
                }
            }
            _ => {
                if is_close {
                    "合约多单平仓"
                } else {
                    "合约多单开仓"
                }
            }
        },
    }
}

struct AggregatedTradeGroup {
    market: String,
    trade_id: String,
    symbol: String,
    side: String,
    price: String,
    quantity: String,
    fee_amount: Option<String>,
    fee_asset: Option<String>,
    realized_profit: Option<String>,
    traded_at_ms: i64,
}

fn aggregate_trade_group(
    trades: &[BinanceUserTrade],
) -> Result<AggregatedTradeGroup, shared_db::SharedDbError> {
    let first = trades
        .first()
        .ok_or_else(|| shared_db::SharedDbError::new("trade group cannot be empty"))?;
    let mut total_quantity = Decimal::ZERO;
    let mut weighted_price = Decimal::ZERO;
    let mut total_fee = Decimal::ZERO;
    let mut has_fee = false;
    let mut total_realized = Decimal::ZERO;
    let mut has_realized = false;
    let mut latest_trade_id = first.trade_id.clone();
    let mut latest_trade_at = first.traded_at_ms;
    let mut fee_asset = first.fee_asset.clone();

    for trade in trades {
        let quantity = parse_decimal(&trade.quantity)?;
        let price = parse_decimal(&trade.price)?;
        total_quantity += quantity;
        weighted_price += price * quantity;
        if let Some(fee) = trade.fee_amount.as_deref() {
            total_fee += parse_decimal(fee)?;
            has_fee = true;
        }
        if let Some(realized) = trade.realized_profit.as_deref() {
            total_realized += parse_decimal(realized)?;
            has_realized = true;
        }
        if trade.traded_at_ms >= latest_trade_at {
            latest_trade_at = trade.traded_at_ms;
            latest_trade_id = trade.trade_id.clone();
        }
        if fee_asset.is_none() {
            fee_asset = trade.fee_asset.clone();
        }
    }

    if total_quantity <= Decimal::ZERO {
        return Err(shared_db::SharedDbError::new(
            "trade group quantity must be positive",
        ));
    }

    Ok(AggregatedTradeGroup {
        market: first.market.clone(),
        trade_id: latest_trade_id,
        symbol: first.symbol.clone(),
        side: first.side.clone(),
        price: (weighted_price / total_quantity).normalize().to_string(),
        quantity: total_quantity.normalize().to_string(),
        fee_amount: has_fee.then(|| total_fee.normalize().to_string()),
        fee_asset,
        realized_profit: has_realized.then(|| total_realized.normalize().to_string()),
        traded_at_ms: latest_trade_at,
    })
}

fn market_scope(strategy: &Strategy) -> &'static str {
    match strategy.market {
        shared_domain::strategy::StrategyMarket::Spot => "spot",
        shared_domain::strategy::StrategyMarket::FuturesUsdM => "usdm",
        shared_domain::strategy::StrategyMarket::FuturesCoinM => "coinm",
    }
}

fn parse_decimal(value: &str) -> Result<Decimal, shared_db::SharedDbError> {
    value
        .parse::<Decimal>()
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))
}

impl BinanceTradeGateway for BinanceClient {
    fn user_trades(
        &self,
        market: &str,
        symbol: &str,
        limit: usize,
    ) -> Result<Vec<BinanceUserTrade>, String> {
        BinanceClient::user_trades(self, market, symbol, limit).map_err(|error| error.to_string())
    }

    fn get_order(
        &self,
        market: &str,
        symbol: &str,
        order_id: &str,
    ) -> Result<shared_binance::BinanceOrderResponse, String> {
        BinanceClient::get_order(self, market, symbol, Some(order_id), None)
            .map_err(|error| error.to_string())
    }
}
