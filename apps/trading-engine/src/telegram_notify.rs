use serde_json::json;
use shared_db::{NotificationLogRecord, SharedDb};
use shared_domain::strategy::Strategy;
use std::sync::OnceLock;
use std::time::Duration as StdDuration;

pub(crate) fn telegram_bot_token() -> Option<String> {
    std::env::var("TELEGRAM_BOT_TOKEN")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub(crate) fn telegram_api_base_url() -> String {
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

pub(crate) fn send_telegram_message(
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

pub(crate) fn persist_telegram_notification(
    db: &SharedDb,
    strategy: &Strategy,
    template_key: &str,
    title: String,
    body: String,
    payload: serde_json::Value,
    traded_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), shared_db::SharedDbError> {
    let Some(binding) = db.find_telegram_binding(&strategy.owner_email)? else {
        return Ok(());
    };
    let delivered = if let Some(token) = telegram_bot_token() {
        send_telegram_message(&token, &binding.telegram_chat_id, &title, &body).is_ok()
    } else {
        false
    };
    db.insert_notification_log(&NotificationLogRecord {
        user_email: strategy.owner_email.clone(),
        channel: "telegram".to_string(),
        template_key: Some(template_key.to_string()),
        title,
        body,
        status: if delivered { "delivered" } else { "failed" }.to_string(),
        payload,
        created_at: traded_at,
        delivered_at: delivered.then_some(traded_at),
    })?;
    Ok(())
}
