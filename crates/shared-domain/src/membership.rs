use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MembershipStatus {
    Pending,
    Active,
    Grace,
    Expired,
    Frozen,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipSnapshot {
    pub email: String,
    pub status: MembershipStatus,
    pub active_until: Option<DateTime<Utc>>,
    pub grace_until: Option<DateTime<Utc>>,
    pub override_status: Option<MembershipStatus>,
}
