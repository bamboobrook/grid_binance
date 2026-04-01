use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use billing_chain_listener::order_matcher::{canonicalize_amount, matches_assignment, ObservedTransfer};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shared_chain::assignment::AddressAssignment;
use shared_db::{BillingOrderRecord, MembershipRecord as StoredMembershipRecord, SharedDb};
use shared_domain::membership::{MembershipSnapshot, MembershipStatus};

use crate::services::auth_service::AuthError;

const ORDER_LEASE_MINUTES: i64 = 15;
const MEMBERSHIP_DAYS: i64 = 30;
const GRACE_DAYS: i64 = 3;
const BSC_ADDRESSES: [&str; 3] = ["bsc-addr-1", "bsc-addr-2", "bsc-addr-3"];

#[derive(Clone)]
pub struct MembershipService {
    db: SharedDb,
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
        Self::new(SharedDb::in_memory().expect("in-memory membership db should initialize"))
    }
}

impl MembershipService {
    pub fn new(db: SharedDb) -> Self {
        Self { db }
    }

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

        if supported_addresses(&chain).is_none() {
            return Err(MembershipError::bad_request("unsupported chain"));
        }

        let orders = self.db.list_billing_orders().map_err(MembershipError::storage)?;
        let assignment = assign_address(&chain, request.requested_at, &orders)
            .ok_or_else(|| MembershipError::conflict("no address available"))?;
        let order_id = self
            .db
            .next_sequence("billing_order_id")
            .map_err(MembershipError::storage)?;

        self.db
            .insert_billing_order(&BillingOrderRecord {
                order_id,
                email,
                chain: assignment.chain.clone(),
                plan_code,
                amount: amount.clone(),
                requested_at: request.requested_at,
                assignment: assignment.clone(),
                paid_at: None,
                tx_hash: None,
            })
            .map_err(MembershipError::storage)?;

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

        if !self
            .db
            .record_seen_transfer(&transfer.tx_hash, &transfer.chain, transfer.observed_at)
            .map_err(MembershipError::storage)?
        {
            return Ok(unmatched_response("duplicate_transaction"));
        }

        let orders = self.db.list_billing_orders().map_err(MembershipError::storage)?;
        let mut has_address_candidate = false;
        let mut has_exact_amount_candidate = false;
        let mut has_expired_exact_amount_candidate = false;
        let mut valid_candidates = Vec::new();

        for order in &orders {
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

            valid_candidates.push(order.clone());
        }

        if valid_candidates.len() > 1 {
            return Ok(unmatched_response("ambiguous_match"));
        }

        let Some(order) = valid_candidates.into_iter().next() else {
            return Ok(unmatched_response(resolve_unmatched_reason(
                has_address_candidate,
                has_exact_amount_candidate,
                has_expired_exact_amount_candidate,
            )));
        };

        let paid_at = transfer.observed_at;
        let active_until = paid_at + Duration::days(MEMBERSHIP_DAYS);
        let grace_until = active_until + Duration::days(GRACE_DAYS);

        self.db
            .apply_membership_payment(
                order.order_id,
                &transfer.tx_hash,
                paid_at,
                &order.email,
                active_until,
                grace_until,
            )
            .map_err(MembershipError::storage)?;

        let snapshot = snapshot_for(
            &order.email,
            self.db
                .find_membership_record(&order.email)
                .map_err(MembershipError::storage)?
                .as_ref(),
            Some(paid_at),
        );

        Ok(MatchBillingOrderResponse {
            matched: true,
            reason: None,
            order_id: Some(order.order_id),
            email: Some(order.email),
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

        let record = self
            .db
            .find_membership_record(&email)
            .map_err(MembershipError::storage)?;

        Ok(snapshot_for(&email, record.as_ref(), Some(request.at)))
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

        self.db
            .update_membership_override(&email, request.status.as_ref())
            .map_err(MembershipError::storage)?;
        let record = self
            .db
            .find_membership_record(&email)
            .map_err(MembershipError::storage)?;
        let effective_at = {
            let activated_at = record.as_ref().and_then(|record| record.activated_at);
            request.at.or(activated_at)
        };

        Ok(snapshot_for(&email, record.as_ref(), effective_at))
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

    fn storage(_error: shared_db::SharedDbError) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal storage error",
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

impl From<AuthError> for MembershipError {
    fn from(value: AuthError) -> Self {
        Self {
            status: value.status,
            message: value.message,
        }
    }
}

#[derive(Debug, Serialize)]
struct MembershipErrorResponse {
    error: String,
}

fn snapshot_for(
    email: &str,
    record: Option<&StoredMembershipRecord>,
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

fn resolve_status(
    record: &StoredMembershipRecord,
    at: Option<DateTime<Utc>>,
) -> MembershipStatus {
    match (at, record.active_until, record.grace_until) {
        (Some(at), Some(active_until), _) if at <= active_until => MembershipStatus::Active,
        (Some(at), Some(_), Some(grace_until)) if at <= grace_until => MembershipStatus::Grace,
        (_, Some(_), Some(_)) => MembershipStatus::Expired,
        _ => MembershipStatus::Pending,
    }
}

fn assign_address(
    chain: &str,
    requested_at: DateTime<Utc>,
    orders: &[BillingOrderRecord],
) -> Option<AddressAssignment> {
    let addresses = supported_addresses(chain)?;
    let expires_at = requested_at + Duration::minutes(ORDER_LEASE_MINUTES);

    addresses.iter().find_map(|address| {
        let reserved = orders.iter().any(|order| {
            order.assignment.chain == chain
                && order.assignment.address == *address
                && order.assignment.expires_at > requested_at
        });

        if reserved {
            None
        } else {
            Some(AddressAssignment {
                chain: chain.to_owned(),
                address: (*address).to_owned(),
                expires_at,
            })
        }
    })
}

fn supported_addresses(chain: &str) -> Option<&'static [&'static str]> {
    match chain {
        "BSC" => Some(&BSC_ADDRESSES),
        _ => None,
    }
}

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

fn normalize_chain(chain: &str) -> String {
    chain.trim().to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::{
        CreateBillingOrderRequest, MatchBillingOrderRequest, MembershipService,
        MembershipStatusRequest,
    };
    use chrono::{Duration, Utc};
    use shared_db::SharedDb;
    use shared_domain::membership::MembershipStatus;
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn membership_orders_and_status_survive_service_restart() {
        let db_path = temp_db_path("membership");
        let requested_at = Utc::now();
        let db = SharedDb::open(&db_path).expect("open db");
        let service = MembershipService::new(db.clone());

        let order = service
            .create_order(CreateBillingOrderRequest {
                email: "member@example.com".to_string(),
                chain: "BSC".to_string(),
                plan_code: "pro-monthly".to_string(),
                amount: "12.34".to_string(),
                requested_at,
            })
            .expect("create order");

        let reopened = MembershipService::new(SharedDb::open(&db_path).expect("reopen db"));
        reopened
            .match_order(MatchBillingOrderRequest {
                chain: order.chain.clone(),
                address: order.address.clone(),
                amount: order.amount.clone(),
                tx_hash: "0xtesthash".to_string(),
                observed_at: requested_at + Duration::minutes(1),
            })
            .expect("match order");

        let restarted = MembershipService::new(SharedDb::open(&db_path).expect("reopen db"));
        let snapshot = restarted
            .membership_status(MembershipStatusRequest {
                email: "member@example.com".to_string(),
                at: requested_at + Duration::minutes(2),
            })
            .expect("membership status");

        assert_eq!(snapshot.status, MembershipStatus::Active);
    }

    fn temp_db_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic")
            .as_nanos();
        std::env::temp_dir().join(format!("grid-binance-{label}-{nonce}.sqlite3"))
    }
}
