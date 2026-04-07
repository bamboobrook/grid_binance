use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use shared_binance::{BinanceClient, BinanceUserTrade};
use shared_db::{ExchangeTradeHistoryRecord, NotificationLogRecord, SharedDb};
use shared_domain::strategy::{Strategy, StrategyRuntimeEvent};
use std::{
    collections::HashSet,
    sync::OnceLock,
    time::Duration as StdDuration,
};

pub trait BinanceTradeGateway {
    fn user_trades(
        &self,
        market: &str,
        symbol: &str,
        limit: usize,
    ) -> Result<Vec<BinanceUserTrade>, String>;
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
    let known_trade_ids = db
        .list_exchange_trade_history(&strategy.owner_email)?
        .into_iter()
        .map(|trade| trade.trade_id)
        .collect::<HashSet<_>>();
    let trades = gateway
        .user_trades(market_scope(strategy), &strategy.symbol, 100)
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    let mut new_fills = 0usize;

    for trade in trades {
        let Some(order_id) = trade.order_id.clone() else {
            continue;
        };
        let Some(order) = strategy
            .runtime
            .orders
            .iter_mut()
            .find(|order| order.exchange_order_id.as_deref() == Some(order_id.as_str()))
        else {
            continue;
        };
        let fill_id = format!("exchange-trade-{}", trade.trade_id);
        if existing_fill_ids.contains(&fill_id) || known_trade_ids.contains(&trade.trade_id) {
            continue;
        }
        let traded_at = Utc
            .timestamp_millis_opt(trade.traded_at_ms)
            .single()
            .unwrap_or_else(Utc::now);
        let price = parse_decimal(&trade.price)?;
        let quantity = parse_decimal(&trade.quantity)?;
        let fee_amount = match trade.fee_amount.as_deref() {
            Some(value) => Some(parse_decimal(value)?),
            None => None,
        };
        order.status = "Filled".to_string();
        if strategy.status == shared_domain::strategy::StrategyStatus::Stopping
            && order.order_id.contains("-stop-close-")
        {
            if let Some(index) = close_order_index(&order.order_id) {
                if index < strategy.runtime.positions.len() {
                    strategy.runtime.positions.remove(index);
                }
            }
        }
        strategy.runtime.fills.push(shared_domain::strategy::StrategyRuntimeFill {
            fill_id: fill_id.clone(),
            order_id: Some(order.order_id.clone()),
            level_index: order.level_index,
            fill_type: "ExchangeFill".to_string(),
            price,
            quantity,
            realized_pnl: None,
            fee_amount,
            fee_asset: trade.fee_asset.clone(),
        });
        strategy.runtime.events.push(StrategyRuntimeEvent {
            event_type: "grid_fill_executed".to_string(),
            detail: format!("grid fill {} executed at {}", trade.trade_id, trade.price),
            price: Some(price),
            created_at: traded_at,
        });
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
            traded_at,
        })?;
        let title = format!("Grid fill {}", trade.symbol);
        let body = format!("{} grid filled at {}.", trade.symbol, trade.price);
        let payload = json!({
            "trade_id": trade.trade_id,
            "order_id": order.order_id,
            "symbol": trade.symbol,
            "price": trade.price,
            "quantity": trade.quantity,
            "fee_amount": trade.fee_amount,
            "fee_asset": trade.fee_asset,
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
        if let Some(binding) = db.find_telegram_binding(&strategy.owner_email)? {
            let delivered = if let Some(token) = telegram_bot_token() {
                send_telegram_message(&token, &binding.telegram_chat_id, &title, &body).is_ok()
            } else {
                false
            };
            db.insert_notification_log(&NotificationLogRecord {
                user_email: strategy.owner_email.clone(),
                channel: "telegram".to_string(),
                template_key: Some("GridFillExecuted".to_string()),
                title,
                body,
                status: if delivered { "delivered" } else { "failed" }.to_string(),
                payload: payload.clone(),
                created_at: traded_at,
                delivered_at: delivered.then_some(traded_at),
            })?;
        }
        let fill_profit_title = format!("Fill profit {}", trade.symbol);
        let realized_pnl = Decimal::ZERO;
        let net_pnl = realized_pnl - fee_amount.unwrap_or(Decimal::ZERO);
        let cumulative_net_pnl = strategy
            .runtime
            .fills
            .iter()
            .fold(Decimal::ZERO, |acc, fill| {
                acc + fill.realized_pnl.unwrap_or(Decimal::ZERO) - fill.fee_amount.unwrap_or(Decimal::ZERO)
            });
        let fill_profit_body = format!(
            "Grid fill realized {} net PnL.",
            net_pnl.normalize()
        );
        let fill_profit_payload = json!({
            "trade_id": trade.trade_id,
            "symbol": trade.symbol,
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
        if let Some(binding) = db.find_telegram_binding(&strategy.owner_email)? {
            let delivered = if let Some(token) = telegram_bot_token() {
                send_telegram_message(&token, &binding.telegram_chat_id, &fill_profit_title, &fill_profit_body).is_ok()
            } else {
                false
            };
            db.insert_notification_log(&NotificationLogRecord {
                user_email: strategy.owner_email.clone(),
                channel: "telegram".to_string(),
                template_key: Some("FillProfitReported".to_string()),
                title: fill_profit_title,
                body: fill_profit_body,
                status: if delivered { "delivered" } else { "failed" }.to_string(),
                payload: fill_profit_payload,
                created_at: traded_at,
                delivered_at: delivered.then_some(traded_at),
            })?;
        }
        finalize_stopping_status(strategy);
        new_fills += 1;
    }

    Ok(TradeSyncResult { new_fills })
}

fn close_order_index(order_id: &str) -> Option<usize> {
    order_id.rsplit('-').next()?.parse::<usize>().ok()
}

fn finalize_stopping_status(strategy: &mut Strategy) {
    let has_pending_close = strategy.runtime.orders.iter().any(|order| {
        order.order_id.contains("-stop-close-") && matches!(order.status.as_str(), "ClosingRequested" | "Placed")
    });
    if strategy.status == shared_domain::strategy::StrategyStatus::Stopping
        && strategy.runtime.positions.is_empty()
        && !has_pending_close
    {
        strategy.status = shared_domain::strategy::StrategyStatus::Stopped;
    }
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
}

fn telegram_bot_token() -> Option<String> {
    std::env::var("TELEGRAM_BOT_TOKEN")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn telegram_api_base_url() -> String {
    std::env::var("TELEGRAM_API_BASE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "https://api.telegram.org".to_string())
}

fn telegram_http_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| ureq::AgentBuilder::new().timeout(StdDuration::from_secs(5)).build())
}

fn send_telegram_message(
    bot_token: &str,
    chat_id: &str,
    title: &str,
    body: &str,
) -> Result<(), shared_db::SharedDbError> {
    telegram_http_agent()
        .post(&format!("{}/bot{}/sendMessage", telegram_api_base_url(), bot_token))
        .send_json(ureq::json!({
            "chat_id": chat_id,
            "text": format!("{}\n{}", title, body),
        }))
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    Ok(())
}
