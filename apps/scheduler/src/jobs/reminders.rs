use chrono::{DateTime, Duration, Utc};
use shared_domain::membership::{MembershipSnapshot, MembershipStatus};

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

#[cfg(test)]
mod tests {
    use super::{due_membership_reminders, ReminderKind};
    use chrono::{DateTime, Duration, Utc};
    use shared_domain::membership::{MembershipSnapshot, MembershipStatus};

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
}
