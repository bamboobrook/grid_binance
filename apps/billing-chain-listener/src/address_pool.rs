use chrono::{DateTime, Duration, Utc};
use shared_chain::assignment::AddressAssignment;

#[derive(Debug, Clone)]
pub struct AddressPool {
    chain: String,
    addresses: Vec<String>,
    next_index: usize,
    lease_duration: Duration,
}

impl AddressPool {
    pub fn new(chain: impl Into<String>, addresses: Vec<String>, lease_duration: Duration) -> Self {
        Self {
            chain: chain.into(),
            addresses,
            next_index: 0,
            lease_duration,
        }
    }

    pub fn assign(&mut self, requested_at: DateTime<Utc>) -> Option<AddressAssignment> {
        if self.addresses.is_empty() {
            return None;
        }

        let address = self.addresses[self.next_index].clone();
        self.next_index = (self.next_index + 1) % self.addresses.len();

        Some(AddressAssignment {
            chain: self.chain.clone(),
            address,
            expires_at: requested_at + self.lease_duration,
        })
    }
}
