use chrono::{DateTime, Duration, Utc};
use shared_db::{NotificationLogRecord, SharedDb, SharedDbError};
use shared_domain::membership::{MembershipSnapshot, MembershipStatus};
use shared_events::{NotificationEvent, NotificationKind, NotificationRecord};
use std::{collections::BTreeMap, env, sync::OnceLock, time::Duration as StdDuration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReminderKind {
    Renewal,
    GraceEnding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MembershipReminder {
    pub email: String,
    pub kind: ReminderKind,
    pub due_at: DateTime<Utc>,
}

pub fn due_membership_reminders(
    memberships: &[MembershipSnapshot],
    now: DateTime<Utc>,
    lookahead: Duration,
) -> Vec<MembershipReminder> {
    let reminder_until = now + lookahead;

    memberships
        .iter()
        .filter(|membership| membership.override_status.is_none())
        .filter_map(|membership| {
            let active_due = matches!(membership.status, MembershipStatus::Active)
                .then_some(membership.active_until)
                .flatten()
                .filter(|due_at| *due_at >= now && *due_at <= reminder_until)
                .map(|due_at| MembershipReminder {
                    email: membership.email.clone(),
                    kind: ReminderKind::Renewal,
                    due_at,
                });

            active_due.or_else(|| {
                matches!(membership.status, MembershipStatus::Grace)
                    .then_some(membership.grace_until)
                    .flatten()
                    .filter(|due_at| *due_at >= now && *due_at <= reminder_until)
                    .map(|due_at| MembershipReminder {
                        email: membership.email.clone(),
                        kind: ReminderKind::GraceEnding,
                        due_at,
                    })
            })
        })
        .collect()
}

pub fn run_membership_reminders_once(
    db: &SharedDb,
    now: DateTime<Utc>,
    lookahead: Duration,
) -> Result<usize, SharedDbError> {
    let memberships = db
        .list_membership_records()?
        .into_iter()
        .map(|(email, record)| MembershipSnapshot {
            email,
            status: resolve_status(&record, now),
            active_until: record.active_until,
            grace_until: record.grace_until,
            override_status: record.override_status,
        })
        .collect::<Vec<_>>();
    let reminders = due_membership_reminders(&memberships, now, lookahead);
    let mut inserted = 0usize;

    for reminder in reminders {
        if reminder_already_logged(db, &reminder)? {
            continue;
        }
        let (title, message) = reminder_message(&reminder);
        let payload_map = BTreeMap::from([
            ("due_at".to_string(), reminder.due_at.to_rfc3339()),
            (
                "kind".to_string(),
                match reminder.kind {
                    ReminderKind::Renewal => "renewal",
                    ReminderKind::GraceEnding => "grace-ending",
                }
                .to_string(),
            ),
        ]);
        let record = NotificationRecord {
            event: NotificationEvent {
                email: reminder.email.clone(),
                kind: NotificationKind::MembershipExpiring,
                title: title.to_string(),
                message: message.to_string(),
                payload: payload_map,
            },
            telegram_delivered: db.find_telegram_binding(&reminder.email)?.is_some(),
            in_app_delivered: true,
            show_expiry_popup: true,
        };
        let payload =
            serde_json::to_value(&record).map_err(|error| SharedDbError::new(error.to_string()))?;
        db.insert_notification_log(&NotificationLogRecord {
            user_email: reminder.email.clone(),
            channel: "in_app".to_string(),
            template_key: Some("MembershipExpiring".to_string()),
            title: title.to_string(),
            body: message.to_string(),
            status: "delivered".to_string(),
            payload: payload.clone(),
            created_at: now,
            delivered_at: Some(now),
        })?;
        if let Some(binding) = db.find_telegram_binding(&reminder.email)? {
            if let Some(token) = telegram_bot_token() {
                let delivered =
                    send_telegram_message(&token, &binding.telegram_chat_id, title, message)
                        .is_ok();
                db.insert_notification_log(&NotificationLogRecord {
                    user_email: reminder.email.clone(),
                    channel: "telegram".to_string(),
                    template_key: Some("MembershipExpiring".to_string()),
                    title: title.to_string(),
                    body: message.to_string(),
                    status: if delivered { "delivered" } else { "failed" }.to_string(),
                    payload,
                    created_at: now,
                    delivered_at: delivered.then_some(now),
                })?;
            }
        }
        inserted += 1;
    }

    Ok(inserted)
}

fn resolve_status(record: &shared_db::MembershipRecord, now: DateTime<Utc>) -> MembershipStatus {
    if let Some(status) = record.override_status.clone() {
        return status;
    }
    if record.active_until.is_some_and(|until| now <= until) {
        MembershipStatus::Active
    } else if record.grace_until.is_some_and(|until| now <= until) {
        MembershipStatus::Grace
    } else {
        MembershipStatus::Expired
    }
}

fn reminder_already_logged(
    db: &SharedDb,
    reminder: &MembershipReminder,
) -> Result<bool, SharedDbError> {
    let due_at = reminder.due_at.to_rfc3339();
    Ok(db
        .list_notification_logs(&reminder.email, 50)?
        .into_iter()
        .any(|record| {
            record.template_key.as_deref() == Some("MembershipExpiring")
                && record
                    .payload
                    .get("event")
                    .and_then(|value| value.get("payload"))
                    .and_then(|value| value.get("due_at"))
                    .and_then(|value| value.as_str())
                    == Some(due_at.as_str())
        }))
}

fn reminder_message(reminder: &MembershipReminder) -> (&'static str, &'static str) {
    match reminder.kind {
        ReminderKind::Renewal => (
            "Membership ending soon",
            "Membership is approaching expiry. Renew before grace begins to avoid strategy interruption.",
        ),
        ReminderKind::GraceEnding => (
            "Grace period ending soon",
            "Membership grace is ending soon. Renew before grace expires to avoid auto-pause.",
        ),
    }
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

#[cfg(test)]
mod tests {
    use super::{due_membership_reminders, run_membership_reminders_once, ReminderKind};
    use crate::test_support::env_lock;
    use chrono::{DateTime, Duration, Utc};
    use shared_db::{MembershipRecord, SharedDb, TelegramBindingRecord};
    use shared_domain::membership::{MembershipSnapshot, MembershipStatus};
    use std::{
        collections::VecDeque,
        env,
        io::{Read, Write},
        net::TcpListener,
        sync::{Arc, Mutex},
        thread,
    };

    #[test]
    fn emits_due_membership_reminders() {
        let now = parse_time("2026-05-01T00:00:00Z");
        let memberships = vec![
            snapshot(
                "renew@example.com",
                MembershipStatus::Active,
                Some("2026-05-01T12:00:00Z"),
                None,
                None,
            ),
            snapshot(
                "grace@example.com",
                MembershipStatus::Grace,
                None,
                Some("2026-05-01T06:00:00Z"),
                None,
            ),
            snapshot(
                "later@example.com",
                MembershipStatus::Active,
                Some("2026-05-04T00:00:00Z"),
                None,
                None,
            ),
            snapshot(
                "override@example.com",
                MembershipStatus::Active,
                Some("2026-05-01T04:00:00Z"),
                None,
                Some(MembershipStatus::Frozen),
            ),
        ];

        let reminders = due_membership_reminders(&memberships, now, Duration::hours(24));

        assert_eq!(reminders.len(), 2);
        assert_eq!(reminders[0].email, "renew@example.com");
        assert_eq!(reminders[0].kind, ReminderKind::Renewal);
        assert_eq!(reminders[1].email, "grace@example.com");
        assert_eq!(reminders[1].kind, ReminderKind::GraceEnding);
    }

    #[test]
    fn run_once_inserts_in_app_and_telegram_logs_without_duplicates() {
        let _guard = env_lock().lock().unwrap();
        let _token = EnvGuard::set("TELEGRAM_BOT_TOKEN", "bot-test-token");
        let server = TestServer::start(vec![TestRoute {
            path_prefix: "/botbot-test-token/sendMessage",
            status_line: "HTTP/1.1 200 OK",
            body: r#"{"ok":true,"result":{"message_id":1}}"#,
        }]);
        let _base = EnvGuard::set("TELEGRAM_API_BASE_URL", &server.base_url);
        let db = SharedDb::ephemeral().expect("db");
        let now = parse_time("2026-05-01T00:00:00Z");
        db.upsert_membership_record(
            "renew@example.com",
            &MembershipRecord {
                activated_at: Some(parse_time("2026-04-01T00:00:00Z")),
                active_until: Some(parse_time("2026-05-01T12:00:00Z")),
                grace_until: Some(parse_time("2026-05-03T12:00:00Z")),
                override_status: None,
            },
        )
        .unwrap();
        db.upsert_telegram_binding(&TelegramBindingRecord {
            user_email: "renew@example.com".to_string(),
            telegram_user_id: "tg-1".to_string(),
            telegram_chat_id: "chat-1".to_string(),
            bound_at: now,
        })
        .unwrap();

        let first = run_membership_reminders_once(&db, now, Duration::hours(24)).unwrap();
        let second = run_membership_reminders_once(&db, now, Duration::hours(24)).unwrap();

        assert_eq!(first, 1);
        assert_eq!(second, 0);
        let logs = db.list_notification_logs("renew@example.com", 10).unwrap();
        assert!(logs.iter().any(|record| record.channel == "in_app"));
        assert!(logs
            .iter()
            .any(|record| record.channel == "telegram" && record.status == "delivered"));
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
