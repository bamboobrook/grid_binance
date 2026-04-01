use chrono::{DateTime, Utc};
use shared_domain::membership::{MembershipSnapshot, MembershipStatus};

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

#[cfg(test)]
mod tests {
    use super::due_grace_pauses;
    use chrono::{DateTime, Utc};
    use shared_domain::membership::{MembershipSnapshot, MembershipStatus};

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
