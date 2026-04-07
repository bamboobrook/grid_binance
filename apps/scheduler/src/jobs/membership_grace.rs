use chrono::{DateTime, Utc};
use shared_db::{NotificationLogRecord, SharedDb, SharedDbError};
use shared_domain::membership::{MembershipSnapshot, MembershipStatus};
use shared_domain::strategy::{Strategy, StrategyRuntimeEvent, StrategyStatus};
use shared_events::{NotificationEvent, NotificationKind, NotificationRecord};
use std::{collections::BTreeMap, env, sync::OnceLock, time::Duration as StdDuration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GracePauseAction {
    pub email: String,
    pub paused_at: DateTime<Utc>,
}

pub fn due_grace_pauses(
    memberships: &[MembershipSnapshot],
    now: DateTime<Utc>,
) -> Vec<GracePauseAction> {
    memberships
        .iter()
        .filter(|membership| membership.status == MembershipStatus::Grace)
        .filter(|membership| membership.override_status.is_none())
        .filter(|membership| membership.grace_until.is_some_and(|until| now >= until))
        .map(|membership| GracePauseAction {
            email: membership.email.clone(),
            paused_at: now,
        })
        .collect()
}

pub fn run_membership_grace_once(
    db: &SharedDb,
    now: DateTime<Utc>,
) -> Result<usize, SharedDbError> {
    let actions = db
        .list_membership_records()?
        .into_iter()
        .filter(|(_, record)| record.override_status.is_none())
        .filter(|(_, record)| record.grace_until.is_some_and(|until| now >= until))
        .map(|(email, _)| GracePauseAction {
            email,
            paused_at: now,
        })
        .collect::<Vec<_>>();

    let mut paused = 0;
    for action in actions {
        for mut strategy in db.list_strategies(&action.email)? {
            if strategy.status != StrategyStatus::Running {
                continue;
            }
            pause_strategy_for_grace(&mut strategy, action.paused_at);
            db.update_strategy(&strategy)?;
            persist_grace_notification(db, &action.email, &strategy.id, action.paused_at)?;
            paused += 1;
        }
    }

    Ok(paused)
}

fn persist_grace_notification(
    db: &SharedDb,
    email: &str,
    strategy_id: &str,
    at: DateTime<Utc>,
) -> Result<(), SharedDbError> {
    let binding = db.find_telegram_binding(email)?;
    let record = NotificationRecord {
        event: NotificationEvent {
            email: email.to_string(),
            kind: NotificationKind::MembershipExpiring,
            title: "Membership grace expired".to_string(),
            message: "Strategy auto-paused after the 48-hour grace period expired.".to_string(),
            payload: BTreeMap::from([("strategy_id".to_string(), strategy_id.to_string())]),
        },
        telegram_delivered: binding.is_some(),
        in_app_delivered: true,
        show_expiry_popup: true,
    };
    let payload =
        serde_json::to_value(&record).map_err(|error| SharedDbError::new(error.to_string()))?;
    db.insert_notification_log(&NotificationLogRecord {
        user_email: email.to_string(),
        channel: "in_app".to_string(),
        template_key: Some("MembershipExpiring".to_string()),
        title: record.event.title.clone(),
        body: record.event.message.clone(),
        status: "delivered".to_string(),
        payload: payload.clone(),
        created_at: at,
        delivered_at: Some(at),
    })?;
    if let Some(binding) = binding {
        if let Some(token) = telegram_bot_token() {
            let delivered = send_telegram_message(
                &token,
                &binding.telegram_chat_id,
                &record.event.title,
                &record.event.message,
            )
            .is_ok();
            db.insert_notification_log(&NotificationLogRecord {
                user_email: email.to_string(),
                channel: "telegram".to_string(),
                template_key: Some("MembershipExpiring".to_string()),
                title: record.event.title,
                body: record.event.message,
                status: if delivered { "delivered" } else { "failed" }.to_string(),
                payload,
                created_at: at,
                delivered_at: delivered.then_some(at),
            })?;
        }
    }
    Ok(())
}

fn telegram_bot_token() -> Option<String> {
    env::var("TELEGRAM_BOT_TOKEN")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn telegram_api_base_url() -> String {
    env::var("TELEGRAM_API_BASE_URL")
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
) -> Result<(), SharedDbError> {
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
        .map_err(|error| SharedDbError::new(error.to_string()))?;
    Ok(())
}

fn pause_strategy_for_grace(strategy: &mut Strategy, paused_at: DateTime<Utc>) {
    strategy.status = StrategyStatus::Paused;
    for order in &mut strategy.runtime.orders {
        if order.status == "Working" {
            order.status = "Canceled".to_string();
        }
    }
    strategy.runtime.events.push(StrategyRuntimeEvent {
        event_type: "membership_grace_paused".to_string(),
        detail: "strategy auto-paused after membership grace expired".to_string(),
        price: None,
        created_at: paused_at,
    });
}

#[cfg(test)]
mod tests {
    use super::{due_grace_pauses, run_membership_grace_once};
    use crate::test_support::env_lock;
    use chrono::{DateTime, Utc};
    use rust_decimal::Decimal;
    use shared_db::{MembershipRecord, SharedDb, StoredStrategy, TelegramBindingRecord};
    use shared_domain::membership::{MembershipSnapshot, MembershipStatus};
    use shared_domain::strategy::{
        GridGeneration, GridLevel, PostTriggerAction, Strategy, StrategyAmountMode, StrategyMarket,
        StrategyMode, StrategyRevision, StrategyRuntime, StrategyRuntimeOrder, StrategyStatus,
    };
    use std::{
        collections::VecDeque,
        env,
        io::{Read, Write},
        net::TcpListener,
        sync::{Arc, Mutex},
        thread,
    };

    #[test]
    fn pauses_membership_when_grace_window_elapsed() {
        let now = parse_time("2026-05-05T00:00:00Z");
        let memberships = vec![
            snapshot(
                "due@example.com",
                MembershipStatus::Grace,
                None,
                Some("2026-05-04T23:59:59Z"),
                None,
            ),
            snapshot(
                "future@example.com",
                MembershipStatus::Grace,
                None,
                Some("2026-05-06T00:00:00Z"),
                None,
            ),
            snapshot(
                "active@example.com",
                MembershipStatus::Active,
                Some("2026-05-04T23:59:59Z"),
                Some("2026-05-06T00:00:00Z"),
                None,
            ),
            snapshot(
                "override@example.com",
                MembershipStatus::Grace,
                None,
                Some("2026-05-04T23:59:59Z"),
                Some(MembershipStatus::Frozen),
            ),
        ];

        let actions = due_grace_pauses(&memberships, now);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].email, "due@example.com");
        assert_eq!(actions[0].paused_at, now);
    }

    #[test]
    fn grace_job_pauses_running_strategies_and_cancels_working_orders() {
        let db = SharedDb::ephemeral().expect("db");
        let now = parse_time("2026-05-05T00:00:00Z");
        db.upsert_membership_record(
            "due@example.com",
            &MembershipRecord {
                activated_at: Some(parse_time("2026-04-01T00:00:00Z")),
                active_until: Some(parse_time("2026-05-03T00:00:00Z")),
                grace_until: Some(parse_time("2026-05-04T23:59:59Z")),
                override_status: None,
            },
        )
        .expect("membership");
        db.insert_strategy(&StoredStrategy {
            sequence_id: 1,
            strategy: strategy("due@example.com", "running-1", StrategyStatus::Running),
        })
        .expect("strategy");
        db.insert_strategy(&StoredStrategy {
            sequence_id: 2,
            strategy: strategy("due@example.com", "paused-1", StrategyStatus::Paused),
        })
        .expect("strategy");
        db.insert_strategy(&StoredStrategy {
            sequence_id: 3,
            strategy: strategy("other@example.com", "running-2", StrategyStatus::Running),
        })
        .expect("strategy");

        let paused = run_membership_grace_once(&db, now).expect("job result");

        assert_eq!(paused, 1);

        let updated = db
            .find_strategy("due@example.com", "running-1")
            .expect("strategy lookup")
            .expect("strategy exists");
        assert_eq!(updated.status, StrategyStatus::Paused);
        assert_eq!(updated.runtime.orders[0].status, "Canceled");
        assert_eq!(updated.runtime.orders[1].status, "Filled");
        assert_eq!(
            updated
                .runtime
                .events
                .last()
                .expect("runtime event")
                .event_type,
            "membership_grace_paused"
        );

        let already_paused = db
            .find_strategy("due@example.com", "paused-1")
            .expect("strategy lookup")
            .expect("strategy exists");
        assert_eq!(already_paused.status, StrategyStatus::Paused);
        assert_eq!(already_paused.runtime.orders[0].status, "Working");

        let other_user = db
            .find_strategy("other@example.com", "running-2")
            .expect("strategy lookup")
            .expect("strategy exists");
        assert_eq!(other_user.status, StrategyStatus::Running);
        assert_eq!(other_user.runtime.orders[0].status, "Working");

        let notifications = db
            .list_notification_logs("due@example.com", 10)
            .expect("notifications");
        assert_eq!(notifications.len(), 1);
        assert!(notifications[0].title.contains("Membership grace expired"));
    }

    #[test]
    fn grace_job_emits_telegram_log_when_bound_and_configured() {
        let _guard = env_lock().lock().unwrap();
        let _token = EnvGuard::set("TELEGRAM_BOT_TOKEN", "bot-test-token");
        let server = TestServer::start(vec![TestRoute {
            path_prefix: "/botbot-test-token/sendMessage",
            status_line: "HTTP/1.1 200 OK",
            body: r#"{"ok":true,"result":{"message_id":1}}"#,
        }]);
        let _base = EnvGuard::set("TELEGRAM_API_BASE_URL", &server.base_url);
        let db = SharedDb::ephemeral().expect("db");
        let now = parse_time("2026-05-05T00:00:00Z");
        db.upsert_membership_record(
            "due@example.com",
            &MembershipRecord {
                activated_at: Some(parse_time("2026-04-01T00:00:00Z")),
                active_until: Some(parse_time("2026-05-03T00:00:00Z")),
                grace_until: Some(parse_time("2026-05-04T23:59:59Z")),
                override_status: None,
            },
        )
        .unwrap();
        db.upsert_telegram_binding(&TelegramBindingRecord {
            user_email: "due@example.com".to_string(),
            telegram_user_id: "tg-1".to_string(),
            telegram_chat_id: "chat-1".to_string(),
            bound_at: now,
        })
        .unwrap();
        db.insert_strategy(&StoredStrategy {
            sequence_id: 1,
            strategy: strategy("due@example.com", "running-1", StrategyStatus::Running),
        })
        .unwrap();

        let paused = run_membership_grace_once(&db, now).unwrap();
        assert_eq!(paused, 1);
        let logs = db.list_notification_logs("due@example.com", 10).unwrap();
        assert!(logs
            .iter()
            .any(|record| record.channel == "telegram" && record.status == "delivered"));
    }

    fn strategy(email: &str, strategy_id: &str, status: StrategyStatus) -> Strategy {
        Strategy {
            id: strategy_id.to_string(),
            owner_email: email.to_string(),
            name: strategy_id.to_string(),
            symbol: "BTCUSDT".to_string(),
            budget: "1000".to_string(),
            grid_spacing_bps: 100,
            status,
            source_template_id: None,
            membership_ready: true,
            exchange_ready: true,
            permissions_ready: true,
            withdrawals_disabled: true,
            hedge_mode_ready: true,
            symbol_ready: true,
            filters_ready: true,
            margin_ready: true,
            conflict_ready: true,
            balance_ready: true,
            market: StrategyMarket::Spot,
            mode: StrategyMode::SpotClassic,
            draft_revision: revision(),
            active_revision: Some(revision()),
            runtime: StrategyRuntime {
                positions: Vec::new(),
                orders: vec![
                    StrategyRuntimeOrder {
                        order_id: format!("{strategy_id}-working"),
                        exchange_order_id: None,
                        level_index: Some(0),
                        side: "Buy".to_string(),
                        order_type: "Limit".to_string(),
                        price: Some(Decimal::new(100_000, 2)),
                        quantity: Decimal::new(1, 0),
                        status: "Working".to_string(),
                    },
                    StrategyRuntimeOrder {
                        order_id: format!("{strategy_id}-filled"),
                        exchange_order_id: None,
                        level_index: Some(1),
                        side: "Sell".to_string(),
                        order_type: "Limit".to_string(),
                        price: Some(Decimal::new(110_000, 2)),
                        quantity: Decimal::new(1, 0),
                        status: "Filled".to_string(),
                    },
                ],
                fills: Vec::new(),
                events: Vec::new(),
                last_preflight: None,
            },
            archived_at: None,
        }
    }

    fn revision() -> StrategyRevision {
        StrategyRevision {
            revision_id: "rev-1".to_string(),
            version: 1,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            levels: vec![GridLevel {
                level_index: 0,
                entry_price: Decimal::new(100_000, 2),
                quantity: Decimal::new(1, 0),
                take_profit_bps: 100,
                trailing_bps: None,
            }],
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        }
    }

    fn snapshot(
        email: &str,
        status: MembershipStatus,
        active_until: Option<&str>,
        grace_until: Option<&str>,
        override_status: Option<MembershipStatus>,
    ) -> MembershipSnapshot {
        MembershipSnapshot {
            email: email.to_string(),
            status,
            active_until: active_until.map(parse_time),
            grace_until: grace_until.map(parse_time),
            override_status,
        }
    }

    fn parse_time(value: &str) -> DateTime<Utc> {
        value.parse().expect("valid RFC3339 timestamp")
    }

    #[derive(Clone)]
    struct TestRoute {
        path_prefix: &'static str,
        status_line: &'static str,
        body: &'static str,
    }

    struct TestServer {
        base_url: String,
        join_handle: Option<thread::JoinHandle<()>>,
    }

    impl TestServer {
        fn start(routes: Vec<TestRoute>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
            let address = listener.local_addr().unwrap();
            let queue = Arc::new(Mutex::new(VecDeque::from(routes)));
            let queue_for_thread = queue.clone();
            let join_handle = thread::spawn(move || {
                while let Some(route) = queue_for_thread.lock().unwrap().pop_front() {
                    let (mut stream, _) = listener.accept().unwrap();
                    let mut buffer = [0u8; 4096];
                    let read = stream.read(&mut buffer).unwrap();
                    let request = String::from_utf8_lossy(&buffer[..read]);
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .unwrap();
                    assert!(
                        path.starts_with(route.path_prefix),
                        "expected path prefix {} but received {}",
                        route.path_prefix,
                        path
                    );
                    let response = format!(
                        "{}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                        route.status_line,
                        route.body.len(),
                        route.body
                    );
                    stream.write_all(response.as_bytes()).unwrap();
                }
            });
            Self {
                base_url: format!("http://{}", address),
                join_handle: Some(join_handle),
            }
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            if let Some(handle) = self.join_handle.take() {
                handle.join().unwrap();
            }
        }
    }

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: impl Into<String>) -> Self {
            let previous = env::var(key).ok();
            env::set_var(key, value.into());
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => env::set_var(self.key, value),
                None => env::remove_var(self.key),
            }
        }
    }
}
