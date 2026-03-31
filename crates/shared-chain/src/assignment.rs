use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressAssignment {
    pub chain: String,
    pub address: String,
    pub expires_at: DateTime<Utc>,
}
