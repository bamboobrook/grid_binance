use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use shared_binance::BinanceExecutionUpdate;
use shared_db::{ExchangeTradeHistoryRecord, NotificationLogRecord, SharedDb};
use shared_domain::strategy::Strategy;
use std::{sync::OnceLock, time::Duration as StdDuration};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionEffectsResult {
    pub new_trades: usize,
}

pub fn persist_execution_effects(
    db: &SharedDb,
    strategy: &Strategy,
    update: &BinanceExecutionUpdate,
) -> Result<ExecutionEffectsResult, shared_db::SharedDbError> {
    let Some(trade_id) = update.trade_id.as_deref() else {
        return Ok(ExecutionEffectsResult::default());
    };
    let known = db
        .list_exchange_trade_history(&strategy.owner_email)?
        .into_iter()
        .any(|trade| trade.trade_id == trade_id);
    if known {
        return Ok(ExecutionEffectsResult::default());
    }

    let traded_at = Utc
        .timestamp_millis_opt(update.event_time_ms)
        .single()
        .unwrap_or_else(Utc::now);
    let side = update.side.clone().unwrap_or_default();
    let quantity = update.last_fill_quantity.clone().unwrap_or_default();
    let price = update
        .last_fill_price
        .clone()
        .or(update.order_price.clone())
        .unwrap_or_default();
    db.insert_exchange_trade_history(&ExchangeTradeHistoryRecord {
        trade_id: trade_id.to_string(),
        user_email: strategy.owner_email.clone(),
        exchange: "binance".to_string(),
        symbol: update.symbol.clone(),
        side,
        quantity: quantity.clone(),
        price: price.clone(),
        fee_amount: update.fee_amount.clone(),
        fee_asset: update.fee_asset.clone(),
        traded_at,
    })?;

    let fill_title = format!("Grid fill {}", update.symbol);
    let fill_body = format!("{} grid filled at {}.", update.symbol, price);
    let fill_payload = json!({
        "trade_id": trade_id,
        "order_id": update.order_id,
        "symbol": update.symbol,
        "price": price,
        "quantity": quantity,
        "fee_amount": update.fee_amount,
        "fee_asset": update.fee_asset,
    });
    db.insert_notification_log(&NotificationLogRecord {
        user_email: strategy.owner_email.clone(),
        channel: "in_app".to_string(),
        template_key: Some("GridFillExecuted".to_string()),
        title: fill_title.clone(),
        body: fill_body.clone(),
        status: "delivered".to_string(),
        payload: fill_payload.clone(),
        created_at: traded_at,
        delivered_at: Some(traded_at),
    })?;
    if let Some(binding) = db.find_telegram_binding(&strategy.owner_email)? {
        let delivered = if let Some(token) = telegram_bot_token() {
            send_telegram_message(&token, &binding.telegram_chat_id, &fill_title, &fill_body)
                .is_ok()
        } else {
            false
        };
        db.insert_notification_log(&NotificationLogRecord {
            user_email: strategy.owner_email.clone(),
            channel: "telegram".to_string(),
            template_key: Some("GridFillExecuted".to_string()),
            title: fill_title,
            body: fill_body,
            status: if delivered { "delivered" } else { "failed" }.to_string(),
            payload: fill_payload,
            created_at: traded_at,
            delivered_at: delivered.then_some(traded_at),
        })?;
    }

    let realized_pnl = update
        .realized_profit
        .as_deref()
        .and_then(|value| value.parse::<Decimal>().ok())
        .unwrap_or(Decimal::ZERO);
    let fee = update
        .fee_amount
        .as_deref()
        .and_then(|value| value.parse::<Decimal>().ok())
        .unwrap_or(Decimal::ZERO);
    let net_pnl = realized_pnl - fee;
    let cumulative_net_pnl = strategy
        .runtime
        .fills
        .iter()
        .fold(Decimal::ZERO, |acc, fill| {
            acc + fill.realized_pnl.unwrap_or(Decimal::ZERO)
                - fill.fee_amount.unwrap_or(Decimal::ZERO)
        });
    let profit_title = format!("Fill profit {}", update.symbol);
    let profit_body = format!("Grid fill realized {} net PnL.", net_pnl.normalize());
    let profit_payload = json!({
        "trade_id": trade_id,
        "symbol": update.symbol,
        "realized_pnl": realized_pnl.normalize().to_string(),
        "net_pnl": net_pnl.normalize().to_string(),
        "cumulative_net_pnl": cumulative_net_pnl.normalize().to_string(),
    });
    db.insert_notification_log(&NotificationLogRecord {
        user_email: strategy.owner_email.clone(),
        channel: "in_app".to_string(),
        template_key: Some("FillProfitReported".to_string()),
        title: profit_title.clone(),
        body: profit_body.clone(),
        status: "delivered".to_string(),
        payload: profit_payload.clone(),
        created_at: traded_at,
        delivered_at: Some(traded_at),
    })?;
    if let Some(binding) = db.find_telegram_binding(&strategy.owner_email)? {
        let delivered = if let Some(token) = telegram_bot_token() {
            send_telegram_message(
                &token,
                &binding.telegram_chat_id,
                &profit_title,
                &profit_body,
            )
            .is_ok()
        } else {
            false
        };
        db.insert_notification_log(&NotificationLogRecord {
            user_email: strategy.owner_email.clone(),
            channel: "telegram".to_string(),
            template_key: Some("FillProfitReported".to_string()),
            title: profit_title,
            body: profit_body,
            status: if delivered { "delivered" } else { "failed" }.to_string(),
            payload: profit_payload,
            created_at: traded_at,
            delivered_at: delivered.then_some(traded_at),
        })?;
    }

    Ok(ExecutionEffectsResult { new_trades: 1 })
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
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout(StdDuration::from_secs(5))
            .build()
    })
}

fn send_telegram_message(
    bot_token: &str,
    chat_id: &str,
    title: &str,
    body: &str,
) -> Result<(), shared_db::SharedDbError> {
    telegram_http_agent()
        .post(&format!(
            "{}/bot{}/sendMessage",
            telegram_api_base_url(),
            bot_token
        ))
        .send_json(ureq::json!({
            "chat_id": chat_id,
            "text": format!("{}\n{}", title, body),
        }))
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    Ok(())
}
