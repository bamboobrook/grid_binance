use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shared_db::{
    DepositTransactionRecord, MembershipPlanRecord, MembershipRecord, NotificationLogRecord,
    SharedDb,
};
use shared_events::{NotificationEvent, NotificationKind, NotificationRecord};
use std::{collections::BTreeMap, sync::OnceLock, time::Duration as StdDuration};

use crate::order_matcher::canonicalize_amount;

#[derive(Debug)]
pub enum ProcessorError {
    InvalidRequest(&'static str),
    Storage(shared_db::SharedDbError),
}

impl std::fmt::Display for ProcessorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRequest(message) => f.write_str(message),
            Self::Storage(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ProcessorError {}

impl From<shared_db::SharedDbError> for ProcessorError {
    fn from(value: shared_db::SharedDbError) -> Self {
        Self::Storage(value)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ObservedChainTransfer {
    pub chain: String,
    pub asset: String,
    pub address: String,
    pub amount: String,
    pub tx_hash: String,
    pub confirmations: Option<u32>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListenerMatchResult {
    pub matched: bool,
    pub reason: Option<String>,
    pub deposit_status: String,
    pub order_id: Option<u64>,
}

pub fn process_observed_transfer(
    db: &SharedDb,
    transfer: ObservedChainTransfer,
) -> Result<ListenerMatchResult, ProcessorError> {
    let chain = transfer.chain.trim().to_uppercase();
    let asset = transfer.asset.trim().to_uppercase();
    let address = normalize_chain_address(&chain, transfer.address.trim());
    let tx_hash = normalize_chain_tx_hash(&chain, transfer.tx_hash.trim());
    if chain.is_empty() {
        return Err(ProcessorError::InvalidRequest("invalid chain"));
    }
    if asset.is_empty() {
        return Err(ProcessorError::InvalidRequest("invalid asset"));
    }
    if address.is_empty() {
        return Err(ProcessorError::InvalidRequest("invalid address"));
    }
    if tx_hash.is_empty() {
        return Err(ProcessorError::InvalidRequest("invalid tx_hash"));
    }
    let amount = canonicalize_amount(&transfer.amount)
        .map_err(|_| ProcessorError::InvalidRequest("invalid amount"))?;
    let confirmations = transfer.confirmations.unwrap_or_default();
    let existing = find_existing_deposit(db, &chain, &tx_hash)?;
    if existing
        .as_ref()
        .is_some_and(|record| record.status != "confirming")
    {
        return Ok(duplicate_result());
    }

    let orders = db.list_billing_orders()?;
    let address_candidates: Vec<_> = orders
        .iter()
        .filter(|order| {
            order.paid_at.is_none()
                && order.requested_at <= transfer.observed_at
                && order.chain == chain
                && order.assignment.as_ref().is_some_and(|assignment| {
                    normalize_chain_address(&chain, &assignment.address) == address
                })
        })
        .cloned()
        .collect();
    let valid_candidates: Vec<_> = address_candidates
        .iter()
        .filter(|order| order.asset == asset && amounts_match(&order.amount, &amount))
        .filter(|order| {
            order
                .assignment
                .as_ref()
                .is_some_and(|assignment| transfer.observed_at <= assignment.expires_at)
        })
        .cloned()
        .collect();

    if valid_candidates.len() == 1 {
        let order = valid_candidates.into_iter().next().expect("one candidate");
        let required_confirmations = read_required_confirmations(db, &chain)?;
        let deposit = DepositTransactionRecord {
            tx_hash: tx_hash.clone(),
            chain: chain.clone(),
            asset: asset.clone(),
            address: address.clone(),
            amount: amount.clone(),
            observed_at: transfer.observed_at,
            order_id: Some(order.order_id),
            status: "confirming".to_owned(),
            review_reason: Some("awaiting_confirmations".to_owned()),
            processed_at: None,
            matched_order_id: None,
        };
        db.upsert_deposit_transaction(&deposit)?;

        if confirmations < required_confirmations {
            return Ok(ListenerMatchResult {
                matched: false,
                reason: Some("awaiting_confirmations".to_owned()),
                deposit_status: "confirming".to_owned(),
                order_id: Some(order.order_id),
            });
        }

        let plan = db
            .list_membership_plans()?
            .into_iter()
            .find(|plan| plan.code == order.plan_code)
            .ok_or(ProcessorError::InvalidRequest("plan not configured"))?;
        let (active_until, grace_until) = entitlement_window(
            db.find_membership_record(&order.email)?.as_ref(),
            &plan,
            transfer.observed_at,
        );
        db.apply_membership_payment(
            order.order_id,
            &order.chain,
            &tx_hash,
            transfer.observed_at,
            &order.email,
            active_until,
            grace_until,
        )?;
        let binding = db.find_telegram_binding(&order.email)?;
        let telegram_delivered = match (binding.as_ref(), telegram_bot_token()) {
            (Some(binding), Some(token)) => send_telegram_message(
                &token,
                &binding.telegram_chat_id,
                "Deposit confirmed",
                &format!(
                    "{} {} deposit matched billing order {}.",
                    order.chain, order.asset, order.order_id
                ),
            )
            .is_ok(),
            _ => false,
        };
        let record = NotificationRecord {
            event: NotificationEvent {
                email: order.email.clone(),
                kind: NotificationKind::DepositConfirmed,
                title: "Deposit confirmed".to_string(),
                message: format!(
                    "{} {} deposit matched billing order {}.",
                    order.chain, order.asset, order.order_id
                ),
                payload: BTreeMap::from([
                    ("order_id".to_string(), order.order_id.to_string()),
                    ("tx_hash".to_string(), tx_hash.clone()),
                ]),
            },
            telegram_delivered,
            in_app_delivered: true,
            show_expiry_popup: false,
        };
        let payload = serde_json::to_value(&record).map_err(|error| {
            ProcessorError::Storage(shared_db::SharedDbError::new(error.to_string()))
        })?;
        db.insert_notification_log(&NotificationLogRecord {
            user_email: order.email.clone(),
            channel: "in_app".to_string(),
            template_key: Some("DepositConfirmed".to_string()),
            title: record.event.title.clone(),
            body: record.event.message.clone(),
            status: "delivered".to_string(),
            payload: payload.clone(),
            created_at: transfer.observed_at,
            delivered_at: Some(transfer.observed_at),
        })?;
        if binding.is_some() {
            db.insert_notification_log(&NotificationLogRecord {
                user_email: order.email.clone(),
                channel: "telegram".to_string(),
                template_key: Some("DepositConfirmed".to_string()),
                title: record.event.title.clone(),
                body: record.event.message.clone(),
                status: if telegram_delivered {
                    "delivered"
                } else {
                    "failed"
                }
                .to_string(),
                payload,
                created_at: transfer.observed_at,
                delivered_at: telegram_delivered.then_some(transfer.observed_at),
            })?;
        }
        return Ok(ListenerMatchResult {
            matched: true,
            reason: None,
            deposit_status: "matched".to_owned(),
            order_id: Some(order.order_id),
        });
    }

    if existing.is_some() {
        return Ok(duplicate_result());
    }

    let (reason, order_id) = if valid_candidates.len() > 1 {
        ("ambiguous_match".to_owned(), None)
    } else if address_candidates.len() > 1 && valid_candidates.is_empty() {
        let reason = if address_candidates.iter().any(|order| order.asset != asset) {
            "wrong_asset".to_owned()
        } else if address_candidates.iter().all(|order| {
            order
                .assignment
                .as_ref()
                .is_some_and(|assignment| transfer.observed_at > assignment.expires_at)
        }) {
            "order_expired".to_owned()
        } else {
            "exact_amount_required".to_owned()
        };
        let order_id = (reason == "order_expired")
            .then(|| address_candidates.first().map(|order| order.order_id))
            .flatten();
        (reason, order_id)
    } else if let Some(order) = address_candidates.first() {
        if order.asset != asset {
            ("wrong_asset".to_owned(), Some(order.order_id))
        } else if !amounts_match(&order.amount, &amount) {
            ("exact_amount_required".to_owned(), Some(order.order_id))
        } else {
            ("order_expired".to_owned(), Some(order.order_id))
        }
    } else {
        ("order_not_found".to_owned(), None)
    };

    db.upsert_deposit_transaction(&DepositTransactionRecord {
        tx_hash,
        chain,
        asset,
        address,
        amount,
        observed_at: transfer.observed_at,
        order_id,
        status: "manual_review_required".to_owned(),
        review_reason: Some(reason.clone()),
        processed_at: None,
        matched_order_id: None,
    })?;
    Ok(ListenerMatchResult {
        matched: false,
        reason: Some(reason),
        deposit_status: "manual_review_required".to_owned(),
        order_id,
    })
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
            "text": format!("{}
        {}", title, body),
        }))
        .map_err(|error| shared_db::SharedDbError::new(error.to_string()))?;
    Ok(())
}

fn duplicate_result() -> ListenerMatchResult {
    ListenerMatchResult {
        matched: false,
        reason: Some("duplicate_transaction".to_owned()),
        deposit_status: "duplicate_ignored".to_owned(),
        order_id: None,
    }
}

fn find_existing_deposit(
    db: &SharedDb,
    chain: &str,
    tx_hash: &str,
) -> Result<Option<DepositTransactionRecord>, ProcessorError> {
    Ok(db
        .list_deposit_transactions()?
        .into_iter()
        .find(|record| record.chain == chain && record.tx_hash == tx_hash))
}

fn normalize_chain_address(chain: &str, address: &str) -> String {
    match chain {
        "ETH" | "BSC" => address.trim().to_ascii_lowercase(),
        _ => address.trim().to_owned(),
    }
}

fn normalize_chain_tx_hash(chain: &str, tx_hash: &str) -> String {
    match chain {
        "ETH" | "BSC" => tx_hash.trim().to_ascii_lowercase(),
        _ => tx_hash.trim().to_owned(),
    }
}

fn amounts_match(left: &str, right: &str) -> bool {
    canonicalize_amount(left)
        .ok()
        .zip(canonicalize_amount(right).ok())
        .is_some_and(|(normalized_left, normalized_right)| normalized_left == normalized_right)
}

fn read_required_confirmations(db: &SharedDb, chain: &str) -> Result<u32, ProcessorError> {
    let key = match chain {
        "ETH" => "confirmations.eth",
        "BSC" => "confirmations.bsc",
        "SOL" => "confirmations.sol",
        _ => return Ok(12),
    };

    let Some(record) = db.get_system_config(key)? else {
        return Ok(12);
    };

    Ok(record
        .config_value
        .get("value")
        .and_then(|value| value.as_u64())
        .filter(|value| *value > 0 && *value <= u32::MAX as u64)
        .map(|value| value as u32)
        .unwrap_or(12))
}

pub fn promote_due_orders(db: &SharedDb, at: DateTime<Utc>) -> Result<usize, ProcessorError> {
    let orders = db.list_billing_orders()?;
    let mut chains = Vec::new();
    for order in &orders {
        if !chains.contains(&order.chain) {
            chains.push(order.chain.clone());
        }
    }

    let mut promoted = 0;
    for chain in chains {
        let mut queued = orders
            .iter()
            .filter(|order| {
                order.chain == chain && order.paid_at.is_none() && order.status == "queued"
            })
            .collect::<Vec<_>>();
        queued.sort_by_key(|order| order.enqueued_at.unwrap_or(order.requested_at));
        for order in queued {
            if db
                .allocate_or_queue_billing_order(order.order_id, &chain, at)?
                .is_some()
            {
                promoted += 1;
            }
        }
    }

    Ok(promoted)
}

fn entitlement_window(
    current: Option<&MembershipRecord>,
    plan: &MembershipPlanRecord,
    paid_at: DateTime<Utc>,
) -> (DateTime<Utc>, DateTime<Utc>) {
    let base = current
        .and_then(|record| record.active_until)
        .filter(|active_until| {
            current
                .and_then(|record| record.grace_until)
                .is_some_and(|grace_until| paid_at <= grace_until)
                || *active_until >= paid_at
        })
        .unwrap_or(paid_at);
    let active_until = base + Duration::days(i64::from(plan.duration_days));
    let grace_until = active_until + Duration::hours(48);
    (active_until, grace_until)
}

#[cfg(test)]
mod tests {
    use super::{process_observed_transfer, promote_due_orders, ObservedChainTransfer};
    use chrono::{DateTime, Utc};
    use serde_json::json;
    use shared_chain::assignment::AddressAssignment;
    use shared_db::{
        BillingOrderRecord, MembershipPlanPriceRecord, MembershipPlanRecord, SharedDb,
        SystemConfigRecord,
    };

    #[test]
    fn listener_waits_for_configured_confirmations_before_activating_membership() {
        let db = SharedDb::ephemeral().expect("db");
        seed_plan(&db, "monthly", 30, "BSC", "USDT", "20.00000000");
        db.upsert_system_config(&SystemConfigRecord {
            config_key: "confirmations.bsc".to_string(),
            config_value: json!({ "value": 3 }),
            updated_at: parse_time("2026-04-01T00:00:00Z"),
        })
        .expect("config");
        db.insert_billing_order(&BillingOrderRecord {
            order_id: 1,
            email: "listener@example.com".to_string(),
            chain: "BSC".to_string(),
            asset: "USDT".to_string(),
            plan_code: "monthly".to_string(),
            amount: "20.00000000".to_string(),
            requested_at: parse_time("2026-04-01T00:00:00Z"),
            assignment: Some(AddressAssignment {
                chain: "BSC".to_string(),
                address: "bsc-addr-1".to_string(),
                expires_at: parse_time("2026-04-01T01:00:00Z"),
            }),
            paid_at: None,
            tx_hash: None,
            status: "pending".to_string(),
            enqueued_at: None,
        })
        .expect("order");

        let first = process_observed_transfer(
            &db,
            ObservedChainTransfer {
                chain: "BSC".to_string(),
                asset: "USDT".to_string(),
                address: "bsc-addr-1".to_string(),
                amount: "20.00000000".to_string(),
                tx_hash: "tx-1".to_string(),
                confirmations: Some(1),
                observed_at: parse_time("2026-04-01T00:01:00Z"),
            },
        )
        .expect("process first observation");

        assert!(!first.matched);
        assert_eq!(first.reason.as_deref(), Some("awaiting_confirmations"));
        assert_eq!(first.deposit_status, "confirming");
        assert!(db
            .find_membership_record("listener@example.com")
            .expect("membership lookup")
            .is_none());

        let second = process_observed_transfer(
            &db,
            ObservedChainTransfer {
                chain: "BSC".to_string(),
                asset: "USDT".to_string(),
                address: "bsc-addr-1".to_string(),
                amount: "20.00000000".to_string(),
                tx_hash: "tx-1".to_string(),
                confirmations: Some(3),
                observed_at: parse_time("2026-04-01T00:02:00Z"),
            },
        )
        .expect("process confirmed observation");

        assert!(second.matched);
        assert_eq!(second.deposit_status, "matched");
        let membership = db
            .find_membership_record("listener@example.com")
            .expect("membership")
            .expect("membership exists");
        assert!(membership.active_until.is_some());
        let notifications = db
            .list_notification_logs("listener@example.com", 10)
            .expect("notifications");
        assert_eq!(notifications.len(), 1);
        assert!(notifications[0].title.contains("Deposit confirmed"));
    }

    #[test]
    fn listener_records_failed_telegram_delivery_when_binding_exists() {
        let _guard = env_lock().lock().unwrap();
        let db = SharedDb::ephemeral().expect("db");
        seed_plan(&db, "monthly", 30, "BSC", "USDT", "20.00000000");
        db.upsert_telegram_binding(&shared_db::TelegramBindingRecord {
            user_email: "listener@example.com".to_string(),
            telegram_user_id: "tg-user".to_string(),
            telegram_chat_id: "tg-chat".to_string(),
            bound_at: parse_time("2026-04-01T00:00:00Z"),
        })
        .expect("binding");
        std::env::set_var("TELEGRAM_BOT_TOKEN", "bot-test-token");
        std::env::set_var("TELEGRAM_API_BASE_URL", "http://127.0.0.1:1");
        db.insert_billing_order(&BillingOrderRecord {
            order_id: 1,
            email: "listener@example.com".to_string(),
            chain: "BSC".to_string(),
            asset: "USDT".to_string(),
            plan_code: "monthly".to_string(),
            amount: "20.00000000".to_string(),
            requested_at: parse_time("2026-04-01T00:00:00Z"),
            assignment: Some(AddressAssignment {
                chain: "BSC".to_string(),
                address: "bsc-addr-1".to_string(),
                expires_at: parse_time("2026-04-01T01:00:00Z"),
            }),
            paid_at: None,
            tx_hash: None,
            status: "pending".to_string(),
            enqueued_at: None,
        })
        .expect("order");

        let result = process_observed_transfer(
            &db,
            ObservedChainTransfer {
                chain: "BSC".to_string(),
                asset: "USDT".to_string(),
                address: "bsc-addr-1".to_string(),
                amount: "20.00000000".to_string(),
                tx_hash: "tx-1".to_string(),
                confirmations: Some(12),
                observed_at: parse_time("2026-04-01T00:01:00Z"),
            },
        )
        .expect("process");

        assert!(result.matched);
        let notifications = db
            .list_notification_logs("listener@example.com", 10)
            .expect("notifications");
        assert_eq!(notifications.len(), 2);
        assert_eq!(notifications[1].channel, "telegram");
        assert_eq!(notifications[1].status, "failed");
        std::env::remove_var("TELEGRAM_BOT_TOKEN");
        std::env::remove_var("TELEGRAM_API_BASE_URL");
    }

    #[test]
    fn listener_marks_in_app_payload_telegram_delivery_false_when_send_fails() {
        let _guard = env_lock().lock().unwrap();
        let db = SharedDb::ephemeral().expect("db");
        seed_plan(&db, "monthly", 30, "BSC", "USDT", "20.00000000");
        db.upsert_telegram_binding(&shared_db::TelegramBindingRecord {
            user_email: "listener@example.com".to_string(),
            telegram_user_id: "tg-user".to_string(),
            telegram_chat_id: "tg-chat".to_string(),
            bound_at: parse_time("2026-04-01T00:00:00Z"),
        })
        .expect("binding");
        std::env::set_var("TELEGRAM_BOT_TOKEN", "bot-test-token");
        std::env::set_var("TELEGRAM_API_BASE_URL", "http://127.0.0.1:1");
        db.insert_billing_order(&BillingOrderRecord {
            order_id: 1,
            email: "listener@example.com".to_string(),
            chain: "BSC".to_string(),
            asset: "USDT".to_string(),
            plan_code: "monthly".to_string(),
            amount: "20.00000000".to_string(),
            requested_at: parse_time("2026-04-01T00:00:00Z"),
            assignment: Some(AddressAssignment {
                chain: "BSC".to_string(),
                address: "bsc-addr-1".to_string(),
                expires_at: parse_time("2026-04-01T01:00:00Z"),
            }),
            paid_at: None,
            tx_hash: None,
            status: "pending".to_string(),
            enqueued_at: None,
        })
        .expect("order");

        process_observed_transfer(
            &db,
            ObservedChainTransfer {
                chain: "BSC".to_string(),
                asset: "USDT".to_string(),
                address: "bsc-addr-1".to_string(),
                amount: "20.00000000".to_string(),
                tx_hash: "tx-payload".to_string(),
                confirmations: Some(12),
                observed_at: parse_time("2026-04-01T00:01:00Z"),
            },
        )
        .expect("process");

        let notifications = db
            .list_notification_logs("listener@example.com", 10)
            .expect("notifications");
        let in_app = notifications
            .iter()
            .find(|record| record.channel == "in_app")
            .expect("in_app");
        assert_eq!(in_app.payload["telegram_delivered"], false);
        std::env::remove_var("TELEGRAM_BOT_TOKEN");
        std::env::remove_var("TELEGRAM_API_BASE_URL");
    }

    #[test]
    fn listener_records_failed_telegram_log_when_binding_exists_without_bot_token() {
        let _guard = env_lock().lock().unwrap();
        let db = SharedDb::ephemeral().expect("db");
        seed_plan(&db, "monthly", 30, "BSC", "USDT", "20.00000000");
        db.upsert_telegram_binding(&shared_db::TelegramBindingRecord {
            user_email: "listener@example.com".to_string(),
            telegram_user_id: "tg-user".to_string(),
            telegram_chat_id: "tg-chat".to_string(),
            bound_at: parse_time("2026-04-01T00:00:00Z"),
        })
        .expect("binding");
        std::env::remove_var("TELEGRAM_BOT_TOKEN");
        db.insert_billing_order(&BillingOrderRecord {
            order_id: 1,
            email: "listener@example.com".to_string(),
            chain: "BSC".to_string(),
            asset: "USDT".to_string(),
            plan_code: "monthly".to_string(),
            amount: "20.00000000".to_string(),
            requested_at: parse_time("2026-04-01T00:00:00Z"),
            assignment: Some(AddressAssignment {
                chain: "BSC".to_string(),
                address: "bsc-addr-1".to_string(),
                expires_at: parse_time("2026-04-01T01:00:00Z"),
            }),
            paid_at: None,
            tx_hash: None,
            status: "pending".to_string(),
            enqueued_at: None,
        })
        .expect("order");

        process_observed_transfer(
            &db,
            ObservedChainTransfer {
                chain: "BSC".to_string(),
                asset: "USDT".to_string(),
                address: "bsc-addr-1".to_string(),
                amount: "20.00000000".to_string(),
                tx_hash: "tx-no-token".to_string(),
                confirmations: Some(12),
                observed_at: parse_time("2026-04-01T00:01:00Z"),
            },
        )
        .expect("process");

        let notifications = db
            .list_notification_logs("listener@example.com", 10)
            .expect("notifications");
        assert!(notifications
            .iter()
            .any(|record| record.channel == "telegram" && record.status == "failed"));
    }

    #[test]
    fn listener_processes_exact_match_and_activates_membership() {
        let db = SharedDb::ephemeral().expect("db");
        seed_plan(&db, "monthly", 30, "BSC", "USDT", "20.00000000");
        db.insert_billing_order(&BillingOrderRecord {
            order_id: 1,
            email: "listener@example.com".to_string(),
            chain: "BSC".to_string(),
            asset: "USDT".to_string(),
            plan_code: "monthly".to_string(),
            amount: "20.00000000".to_string(),
            requested_at: parse_time("2026-04-01T00:00:00Z"),
            assignment: Some(AddressAssignment {
                chain: "BSC".to_string(),
                address: "bsc-addr-1".to_string(),
                expires_at: parse_time("2026-04-01T01:00:00Z"),
            }),
            paid_at: None,
            tx_hash: None,
            status: "pending".to_string(),
            enqueued_at: None,
        })
        .expect("order");

        let result = process_observed_transfer(
            &db,
            ObservedChainTransfer {
                chain: "BSC".to_string(),
                asset: "USDT".to_string(),
                address: "bsc-addr-1".to_string(),
                amount: "20.00000000".to_string(),
                tx_hash: "tx-1".to_string(),
                confirmations: Some(12),
                observed_at: parse_time("2026-04-01T00:01:00Z"),
            },
        )
        .expect("process");

        assert!(result.matched);
        assert_eq!(result.deposit_status, "matched");
        let membership = db
            .find_membership_record("listener@example.com")
            .expect("membership")
            .expect("membership exists");
        assert!(membership.active_until.is_some());
        let notifications = db
            .list_notification_logs("listener@example.com", 10)
            .expect("notifications");
        assert_eq!(notifications.len(), 1);
        assert!(notifications[0].title.contains("Deposit confirmed"));
    }

    #[test]
    fn listener_dedupes_by_chain_and_tx_hash() {
        let db = SharedDb::ephemeral().expect("db");
        seed_plan(&db, "monthly", 30, "BSC", "USDT", "20.00000000");
        seed_plan(&db, "monthly", 30, "ETH", "USDT", "20.00000000");
        for (order_id, chain, address, email) in [
            (1_u64, "BSC", "bsc-addr-1", "bsc@example.com"),
            (2_u64, "ETH", "eth-addr-1", "eth@example.com"),
        ] {
            db.insert_billing_order(&BillingOrderRecord {
                order_id,
                email: email.to_string(),
                chain: chain.to_string(),
                asset: "USDT".to_string(),
                plan_code: "monthly".to_string(),
                amount: "20.00000000".to_string(),
                requested_at: parse_time("2026-04-01T00:00:00Z"),
                assignment: Some(AddressAssignment {
                    chain: chain.to_string(),
                    address: address.to_string(),
                    expires_at: parse_time("2026-04-01T01:00:00Z"),
                }),
                paid_at: None,
                tx_hash: None,
                status: "pending".to_string(),
                enqueued_at: None,
            })
            .expect("order");
        }

        let first = process_observed_transfer(
            &db,
            ObservedChainTransfer {
                chain: "BSC".to_string(),
                asset: "USDT".to_string(),
                address: "bsc-addr-1".to_string(),
                amount: "20.00000000".to_string(),
                tx_hash: "same-hash".to_string(),
                confirmations: Some(12),
                observed_at: parse_time("2026-04-01T00:01:00Z"),
            },
        )
        .expect("process");
        let second = process_observed_transfer(
            &db,
            ObservedChainTransfer {
                chain: "ETH".to_string(),
                asset: "USDT".to_string(),
                address: "eth-addr-1".to_string(),
                amount: "20.00000000".to_string(),
                tx_hash: "same-hash".to_string(),
                confirmations: Some(12),
                observed_at: parse_time("2026-04-01T00:02:00Z"),
            },
        )
        .expect("process");

        assert!(first.matched);
        assert!(second.matched);
    }

    #[test]
    fn promote_due_orders_assigns_freed_addresses_without_api_calls() {
        let db = SharedDb::ephemeral().expect("db");
        seed_plan(&db, "monthly", 30, "BSC", "USDT", "20.00000000");
        db.upsert_deposit_address(&shared_db::DepositAddressPoolRecord {
            chain: "BSC".to_string(),
            address: "bsc-addr-1".to_string(),
            is_enabled: true,
        })
        .expect("address");
        db.insert_billing_order(&BillingOrderRecord {
            order_id: 1,
            email: "occupied@example.com".to_string(),
            chain: "BSC".to_string(),
            asset: "USDT".to_string(),
            plan_code: "monthly".to_string(),
            amount: "20.00000000".to_string(),
            requested_at: parse_time("2026-04-01T00:00:00Z"),
            assignment: Some(AddressAssignment {
                chain: "BSC".to_string(),
                address: "bsc-addr-1".to_string(),
                expires_at: parse_time("2026-04-01T01:00:00Z"),
            }),
            paid_at: None,
            tx_hash: None,
            status: "pending".to_string(),
            enqueued_at: None,
        })
        .expect("order");
        db.insert_billing_order(&BillingOrderRecord {
            order_id: 2,
            email: "queued@example.com".to_string(),
            chain: "BSC".to_string(),
            asset: "USDT".to_string(),
            plan_code: "monthly".to_string(),
            amount: "20.00000000".to_string(),
            requested_at: parse_time("2026-04-01T00:10:00Z"),
            assignment: None,
            paid_at: None,
            tx_hash: None,
            status: "queued".to_string(),
            enqueued_at: Some(parse_time("2026-04-01T00:10:00Z")),
        })
        .expect("order");

        let promoted =
            promote_due_orders(&db, parse_time("2026-04-01T01:00:01Z")).expect("promote");

        assert_eq!(promoted, 1);
        let orders = db.list_billing_orders().expect("orders");
        let queued = orders
            .iter()
            .find(|order| order.order_id == 2)
            .expect("queued order");
        assert_eq!(
            queued.assignment.as_ref().expect("assignment").address,
            "bsc-addr-1"
        );
    }

    #[test]
    fn listener_rejects_empty_required_fields() {
        let db = SharedDb::ephemeral().expect("db");
        for (chain, asset, address, tx_hash, expected) in [
            ("", "USDT", "addr", "tx", "invalid chain"),
            ("BSC", "", "addr", "tx", "invalid asset"),
            ("BSC", "USDT", "", "tx", "invalid address"),
            ("BSC", "USDT", "addr", "", "invalid tx_hash"),
        ] {
            let error = process_observed_transfer(
                &db,
                ObservedChainTransfer {
                    chain: chain.to_string(),
                    asset: asset.to_string(),
                    address: address.to_string(),
                    amount: "1.00000000".to_string(),
                    tx_hash: tx_hash.to_string(),
                    confirmations: None,
                    observed_at: parse_time("2026-04-01T00:00:00Z"),
                },
            )
            .expect_err("invalid request");
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn listener_matches_evm_addresses_case_insensitively() {
        let db = SharedDb::ephemeral().expect("db");
        seed_plan(&db, "monthly", 30, "ETH", "USDT", "20.000000");
        db.insert_billing_order(&BillingOrderRecord {
            order_id: 1,
            email: "listener@example.com".to_string(),
            chain: "ETH".to_string(),
            asset: "USDT".to_string(),
            plan_code: "monthly".to_string(),
            amount: "20.000000".to_string(),
            requested_at: parse_time("2026-04-01T00:00:00Z"),
            assignment: Some(AddressAssignment {
                chain: "ETH".to_string(),
                address: "0xAbCd000000000000000000000000000000000123".to_string(),
                expires_at: parse_time("2026-04-01T01:00:00Z"),
            }),
            paid_at: None,
            tx_hash: None,
            status: "pending".to_string(),
            enqueued_at: None,
        })
        .expect("order");

        let result = process_observed_transfer(
            &db,
            ObservedChainTransfer {
                chain: "ETH".to_string(),
                asset: "USDT".to_string(),
                address: "0xabcd000000000000000000000000000000000123".to_string(),
                amount: "20.000000".to_string(),
                tx_hash: "0xhash".to_string(),
                confirmations: Some(12),
                observed_at: parse_time("2026-04-01T00:10:00Z"),
            },
        )
        .expect("process");

        assert!(result.matched);
        assert_eq!(result.deposit_status, "matched");
    }

    fn seed_plan(
        db: &SharedDb,
        code: &str,
        duration_days: i32,
        chain: &str,
        asset: &str,
        amount: &str,
    ) {
        db.upsert_membership_plan(&MembershipPlanRecord {
            code: code.to_string(),
            name: code.to_string(),
            duration_days,
            is_active: true,
        })
        .expect("plan");
        db.upsert_plan_price(&MembershipPlanPriceRecord {
            plan_code: code.to_string(),
            chain: chain.to_string(),
            asset: asset.to_string(),
            amount: amount.to_string(),
        })
        .expect("price");
    }

    fn env_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    fn parse_time(value: &str) -> DateTime<Utc> {
        value.parse().expect("time")
    }
}
