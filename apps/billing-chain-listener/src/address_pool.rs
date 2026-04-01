use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use shared_chain::assignment::AddressAssignment;

#[derive(Debug, Clone)]
pub struct AddressPool {
    chain: String,
    addresses: Vec<String>,
    next_index: usize,
    lease_duration: Duration,
    active_leases: HashMap<String, DateTime<Utc>>,
}

impl AddressPool {
    pub fn new(chain: impl Into<String>, addresses: Vec<String>, lease_duration: Duration) -> Self {
        Self {
            chain: chain.into(),
            addresses,
            next_index: 0,
            lease_duration,
            active_leases: HashMap::new(),
        }
    }

    pub fn assign(&mut self, requested_at: DateTime<Utc>) -> Option<AddressAssignment> {
        if self.addresses.is_empty() {
            return None;
        }

        self.active_leases
            .retain(|_, expires_at| *expires_at > requested_at);

        let total = self.addresses.len();
        for offset in 0..total {
            let index = (self.next_index + offset) % total;
            let address = self.addresses[index].clone();

            if self.active_leases.contains_key(&address) {
                continue;
            }

            let expires_at = requested_at + self.lease_duration;
            self.active_leases.insert(address.clone(), expires_at);
            self.next_index = (index + 1) % total;

            return Some(AddressAssignment {
                chain: self.chain.clone(),
                address,
                expires_at,
            });
        }

        None
    }
}
