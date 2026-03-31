use std::{error::Error, fmt};

use chrono::{DateTime, Utc};
use shared_chain::assignment::AddressAssignment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedTransfer {
    pub chain: String,
    pub address: String,
    pub amount: String,
    pub tx_hash: String,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmountParseError;

impl fmt::Display for AmountParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid amount")
    }
}

impl Error for AmountParseError {}

pub fn canonicalize_amount(amount: &str) -> Result<String, AmountParseError> {
    let trimmed = amount.trim();
    if trimmed.is_empty() {
        return Err(AmountParseError);
    }

    let (integer, fractional) = match trimmed.split_once('.') {
        Some((integer, fractional)) => (integer, fractional),
        None => (trimmed, ""),
    };

    if integer.is_empty()
        || !integer.chars().all(|ch| ch.is_ascii_digit())
        || !fractional.chars().all(|ch| ch.is_ascii_digit())
        || fractional.len() > 8
    {
        return Err(AmountParseError);
    }

    let normalized_integer = integer.trim_start_matches('0');
    let integer_part = if normalized_integer.is_empty() {
        "0"
    } else {
        normalized_integer
    };

    let mut normalized_fractional = fractional.to_owned();
    while normalized_fractional.len() < 8 {
        normalized_fractional.push('0');
    }

    Ok(format!("{integer_part}.{normalized_fractional}"))
}

pub fn matches_assignment(
    assignment: &AddressAssignment,
    expected_amount: &str,
    transfer: &ObservedTransfer,
) -> Result<bool, AmountParseError> {
    Ok(assignment.chain == transfer.chain
        && assignment.address == transfer.address
        && canonicalize_amount(expected_amount)? == canonicalize_amount(&transfer.amount)?)
}
