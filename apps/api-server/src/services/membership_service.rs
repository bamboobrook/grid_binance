use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use billing_chain_listener::{
    address_pool::AddressPool,
    order_matcher::{canonicalize_amount, matches_assignment, ObservedTransfer},
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shared_chain::assignment::AddressAssignment;
use shared_domain::membership::{MembershipSnapshot, MembershipStatus};

const ORDER_LEASE_MINUTES: i64 = 15;
const MEMBERSHIP_DAYS: i64 = 30;
const GRACE_DAYS: i64 = 3;

#[derive(Clone)]
pub struct MembershipService {
    inner: Arc<Mutex<MembershipState>>,
}

struct MembershipState {
    next_order_id: u64,
    address_pools: HashMap<String, AddressPool>,
    orders: HashMap<u64, BillingOrder>,
    memberships: HashMap<String, MembershipRecord>,
    seen_transfers: HashSet<String>,
}

#[derive(Debug, Clone)]
struct BillingOrder {
    email: String,
    amount: String,
    requested_at: DateTime<Utc>,
    assignment: AddressAssignment,
    paid_at: Option<DateTime<Utc>>,
    tx_hash: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct MembershipRecord {
    activated_at: Option<DateTime<Utc>>,
    active_until: Option<DateTime<Utc>>,
    grace_until: Option<DateTime<Utc>>,
    override_status: Option<MembershipStatus>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBillingOrderRequest {
    pub email: String,
    pub chain: String,
    pub plan_code: String,
    pub amount: String,
    pub requested_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CreateBillingOrderResponse {
    pub order_id: u64,
    pub chain: String,
    pub address: String,
    pub amount: String,
    pub expires_at: DateTime<Utc>,
    pub status: MembershipStatus,
}

#[derive(Debug, Deserialize)]
pub struct MatchBillingOrderRequest {
    pub chain: String,
    pub address: String,
    pub amount: String,
    pub tx_hash: String,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct MatchBillingOrderResponse {
    pub matched: bool,
    pub reason: Option<&'static str>,
    pub order_id: Option<u64>,
    pub email: Option<String>,
    pub membership_status: Option<MembershipStatus>,
    pub active_until: Option<DateTime<Utc>>,
    pub grace_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct MembershipStatusRequest {
    pub email: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct MembershipOverrideRequest {
    pub email: String,
    pub status: Option<MembershipStatus>,
    pub at: Option<DateTime<Utc>>,
}

impl Default for MembershipService {
    fn default() -> Self {
        let mut address_pools = HashMap::new();
        address_pools.insert(
            "BSC".to_owned(),
            AddressPool::new(
                "BSC",
                vec![
                    "bsc-addr-1".to_owned(),
                    "bsc-addr-2".to_owned(),
                    "bsc-addr-3".to_owned(),
                ],
                Duration::minutes(ORDER_LEASE_MINUTES),
            ),
        );

        Self {
            inner: Arc::new(Mutex::new(MembershipState {
                next_order_id: 0,
                address_pools,
                orders: HashMap::new(),
                memberships: HashMap::new(),
                seen_transfers: HashSet::new(),
            })),
        }
    }
}

impl MembershipService {
    pub fn create_order(
        &self,
        request: CreateBillingOrderRequest,
    ) -> Result<CreateBillingOrderResponse, MembershipError> {
        let email = normalize_email(&request.email);
        let chain = normalize_chain(&request.chain);
        let plan_code = request.plan_code.trim().to_owned();
        let amount = canonicalize_amount(&request.amount)
            .map_err(|_| MembershipError::bad_request("invalid amount"))?;

        if email.is_empty() || chain.is_empty() || plan_code.is_empty() {
            return Err(MembershipError::bad_request(
                "email, chain, plan_code, and amount are required",
            ));
        }

        let mut inner = self.inner.lock().expect("membership state poisoned");
        let assignment = inner
            .address_pools
            .get_mut(&chain)
            .ok_or_else(|| MembershipError::bad_request("unsupported chain"))?
            .assign(request.requested_at)
            .ok_or_else(|| MembershipError::conflict("no address available"))?;

        inner.next_order_id += 1;
        let order_id = inner.next_order_id;

        inner.orders.insert(
            order_id,
            BillingOrder {
                email,
                amount: amount.clone(),
                requested_at: request.requested_at,
                assignment: assignment.clone(),
                paid_at: None,
                tx_hash: None,
            },
        );

        Ok(CreateBillingOrderResponse {
            order_id,
            chain: assignment.chain,
            address: assignment.address,
            amount,
            expires_at: assignment.expires_at,
            status: MembershipStatus::Pending,
        })
    }

    pub fn match_order(
        &self,
        request: MatchBillingOrderRequest,
    ) -> Result<MatchBillingOrderResponse, MembershipError> {
        let transfer = ObservedTransfer {
            chain: normalize_chain(&request.chain),
            address: request.address.trim().to_owned(),
            amount: request.amount,
            tx_hash: request.tx_hash.trim().to_owned(),
            observed_at: request.observed_at,
        };

        if transfer.chain.is_empty() || transfer.address.is_empty() || transfer.tx_hash.is_empty() {
            return Err(MembershipError::bad_request(
                "chain, address, amount, and tx_hash are required",
            ));
        }

        let mut inner = self.inner.lock().expect("membership state poisoned");
        if inner.seen_transfers.contains(&transfer.tx_hash) {
            return Ok(unmatched_response("duplicate_transaction"));
        }

        inner.seen_transfers.insert(transfer.tx_hash.clone());

        let mut has_address_candidate = false;
        let mut has_exact_amount_candidate = false;
        let mut has_expired_exact_amount_candidate = false;
        let mut valid_candidates = Vec::new();

        for (order_id, order) in &inner.orders {
            if order.paid_at.is_some()
                || order.requested_at > transfer.observed_at
                || order.assignment.chain != transfer.chain
                || order.assignment.address != transfer.address
            {
                continue;
            }

            has_address_candidate = true;

            if !matches_assignment(&order.assignment, &order.amount, &transfer)
                .map_err(|_| MembershipError::bad_request("invalid amount"))?
            {
                continue;
            }

            has_exact_amount_candidate = true;

            if transfer.observed_at > order.assignment.expires_at {
                has_expired_exact_amount_candidate = true;
                continue;
            }

            valid_candidates.push(*order_id);
        }

        if valid_candidates.len() > 1 {
            return Ok(unmatched_response("ambiguous_match"));
        }

        let Some(order_id) = valid_candidates.first().copied() else {
            return Ok(unmatched_response(resolve_unmatched_reason(
                has_address_candidate,
                has_exact_amount_candidate,
                has_expired_exact_amount_candidate,
            )));
        };

        let (email, snapshot) = {
            let order = inner
                .orders
                .get_mut(&order_id)
                .expect("matched order should exist");
            order.paid_at = Some(transfer.observed_at);
            order.tx_hash = Some(transfer.tx_hash.clone());
            let email = order.email.clone();
            let paid_at = transfer.observed_at;

            let record = inner.memberships.entry(email.clone()).or_default();
            record.activated_at = Some(paid_at);
            record.active_until = Some(paid_at + Duration::days(MEMBERSHIP_DAYS));
            record.grace_until = record
                .active_until
                .map(|active_until| active_until + Duration::days(GRACE_DAYS));

            let snapshot = snapshot_for(&email, Some(record), Some(paid_at));
            (email, snapshot)
        };

        Ok(MatchBillingOrderResponse {
            matched: true,
            reason: None,
            order_id: Some(order_id),
            email: Some(email),
            membership_status: Some(snapshot.status),
            active_until: snapshot.active_until,
            grace_until: snapshot.grace_until,
        })
    }

    pub fn membership_status(
        &self,
        request: MembershipStatusRequest,
    ) -> Result<MembershipSnapshot, MembershipError> {
        let email = normalize_email(&request.email);
        if email.is_empty() {
            return Err(MembershipError::bad_request("email is required"));
        }

        let inner = self.inner.lock().expect("membership state poisoned");
        Ok(snapshot_for(
            &email,
            inner.memberships.get(&email),
            Some(request.at),
        ))
    }

    pub fn override_membership(
        &self,
        request: MembershipOverrideRequest,
    ) -> Result<MembershipSnapshot, MembershipError> {
        let email = normalize_email(&request.email);
        if email.is_empty() {
            return Err(MembershipError::bad_request("email is required"));
        }

        if !matches!(
            request.status,
            None | Some(MembershipStatus::Frozen) | Some(MembershipStatus::Revoked)
        ) {
            return Err(MembershipError::bad_request(
                "override only supports Frozen or Revoked",
            ));
        }

        let mut inner = self.inner.lock().expect("membership state poisoned");
        let effective_at = {
            let record = inner.memberships.entry(email.clone()).or_default();
            record.override_status = request.status;
            request.at.or(record.activated_at)
        };

        Ok(snapshot_for(
            &email,
            inner.memberships.get(&email),
            effective_at,
        ))
    }
}

#[derive(Debug)]
pub struct MembershipError {
    status: StatusCode,
    message: &'static str,
}

impl MembershipError {
    fn bad_request(message: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message,
        }
    }

    fn conflict(message: &'static str) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message,
        }
    }
}

impl IntoResponse for MembershipError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(MembershipErrorResponse {
                error: self.message.to_owned(),
            }),
        )
            .into_response()
    }
}

#[derive(Debug, Serialize)]
struct MembershipErrorResponse {
    error: String,
}

fn snapshot_for(
    email: &str,
    record: Option<&MembershipRecord>,
    at: Option<DateTime<Utc>>,
) -> MembershipSnapshot {
    let active_until = record.and_then(|value| value.active_until);
    let grace_until = record.and_then(|value| value.grace_until);
    let override_status = record.and_then(|value| value.override_status.clone());

    let status = if let Some(status) = override_status.clone() {
        status
    } else if let Some(record) = record {
        let reference_at = at.or(record.activated_at);
        resolve_status(record, reference_at)
    } else {
        MembershipStatus::Pending
    };

    MembershipSnapshot {
        email: email.to_owned(),
        status,
        active_until,
        grace_until,
        override_status,
    }
}

fn unmatched_response(reason: &'static str) -> MatchBillingOrderResponse {
    MatchBillingOrderResponse {
        matched: false,
        reason: Some(reason),
        order_id: None,
        email: None,
        membership_status: None,
        active_until: None,
        grace_until: None,
    }
}

fn resolve_unmatched_reason(
    has_address_candidate: bool,
    has_exact_amount_candidate: bool,
    has_expired_exact_amount_candidate: bool,
) -> &'static str {
    if has_expired_exact_amount_candidate {
        "order_expired"
    } else if has_exact_amount_candidate {
        "ambiguous_match"
    } else if has_address_candidate {
        "exact_amount_required"
    } else {
        "order_not_found"
    }
}

fn resolve_status(record: &MembershipRecord, at: Option<DateTime<Utc>>) -> MembershipStatus {
    match (at, record.active_until, record.grace_until) {
        (Some(at), Some(active_until), _) if at <= active_until => MembershipStatus::Active,
        (Some(at), Some(_), Some(grace_until)) if at <= grace_until => MembershipStatus::Grace,
        (_, Some(_), Some(_)) => MembershipStatus::Expired,
        _ => MembershipStatus::Pending,
    }
}

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

fn normalize_chain(chain: &str) -> String {
    chain.trim().to_uppercase()
}
