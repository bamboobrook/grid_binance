use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use billing_chain_listener::order_matcher::canonicalize_amount;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use shared_db::{
    AuditLogRecord, BillingOrderRecord, DepositAddressPoolRecord, DepositTransactionRecord,
    MembershipPlanPriceRecord, MembershipPlanRecord, MembershipRecord,
    SweepJobRecord, SweepTransferRecord, SharedDb,
};
use shared_domain::membership::{MembershipSnapshot, MembershipStatus};

use crate::services::auth_service::AuthError;

const GRACE_HOURS: i64 = 48;

const DEFAULT_PLAN_CONFIG: [(&str, &str, i32, &str); 3] = [
    ("monthly", "Monthly", 30, "20.00000000"),
    ("quarterly", "Quarterly", 90, "54.00000000"),
    ("yearly", "Yearly", 365, "180.00000000"),
];

const DEFAULT_CHAIN_CODES: [&str; 3] = ["ETH", "BSC", "SOL"];
const DEFAULT_ASSETS: [&str; 2] = ["USDT", "USDC"];
const DEFAULT_ETH_ADDRESSES: [&str; 5] = [
    "eth-addr-1",
    "eth-addr-2",
    "eth-addr-3",
    "eth-addr-4",
    "eth-addr-5",
];
const DEFAULT_BSC_ADDRESSES: [&str; 5] = [
    "bsc-addr-1",
    "bsc-addr-2",
    "bsc-addr-3",
    "bsc-addr-4",
    "bsc-addr-5",
];
const DEFAULT_SOL_ADDRESSES: [&str; 5] = [
    "sol-addr-1",
    "sol-addr-2",
    "sol-addr-3",
    "sol-addr-4",
    "sol-addr-5",
];

#[derive(Clone)]
pub struct MembershipService {
    db: SharedDb,
}

#[derive(Debug, Deserialize)]
pub struct CreateBillingOrderRequest {
    pub email: String,
    pub chain: String,
    pub asset: String,
    pub plan_code: String,
    pub requested_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CreateBillingOrderResponse {
    pub order_id: u64,
    pub chain: String,
    pub asset: String,
    pub address: Option<String>,
    pub amount: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: MembershipStatus,
    pub queue_position: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct MatchBillingOrderRequest {
    pub chain: String,
    pub asset: String,
    pub address: String,
    pub amount: String,
    pub tx_hash: String,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct MatchBillingOrderResponse {
    pub matched: bool,
    pub reason: Option<String>,
    pub order_id: Option<u64>,
    pub email: Option<String>,
    pub membership_status: Option<MembershipStatus>,
    pub active_until: Option<DateTime<Utc>>,
    pub grace_until: Option<DateTime<Utc>>,
    pub deposit_status: String,
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

#[derive(Debug, Deserialize)]
pub struct ManualMembershipRequest {
    pub email: String,
    pub action: String,
    pub duration_days: Option<i64>,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpsertMembershipPlanRequest {
    pub code: String,
    pub name: String,
    pub duration_days: i32,
    pub is_active: bool,
    pub prices: Vec<UpsertMembershipPlanPriceRequest>,
}

#[derive(Debug, Deserialize)]
pub struct UpsertMembershipPlanPriceRequest {
    pub chain: String,
    pub asset: String,
    pub amount: String,
}

#[derive(Debug, Serialize)]
pub struct MembershipPlanConfigResponse {
    pub code: String,
    pub name: String,
    pub duration_days: i32,
    pub is_active: bool,
    pub prices: Vec<MembershipPlanPriceResponse>,
}

#[derive(Debug, Serialize)]
pub struct MembershipPlanPriceResponse {
    pub chain: String,
    pub asset: String,
    pub amount: String,
}

#[derive(Debug, Serialize)]
pub struct MembershipPlanConfigListResponse {
    pub plans: Vec<MembershipPlanConfigResponse>,
}

#[derive(Debug, Deserialize)]
pub struct UpsertAddressPoolEntryRequest {
    pub chain: String,
    pub address: String,
    pub is_enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct AddressPoolEntryResponse {
    pub chain: String,
    pub address: String,
    pub is_enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct AddressPoolListResponse {
    pub addresses: Vec<AddressPoolEntryResponse>,
}

#[derive(Debug, Deserialize)]
pub struct AdminDepositsQuery {
    pub at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct AdminBillingOrderView {
    pub order_id: u64,
    pub email: String,
    pub chain: String,
    pub asset: String,
    pub address: Option<String>,
    pub amount: String,
    pub status: String,
    pub queue_position: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct AdminDepositView {
    pub tx_hash: String,
    pub chain: String,
    pub asset: String,
    pub address: String,
    pub amount: String,
    pub status: String,
    pub review_reason: Option<String>,
    pub order_id: Option<u64>,
    pub matched_order_id: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct AdminDepositsResponse {
    pub orders: Vec<AdminBillingOrderView>,
    pub abnormal_deposits: Vec<AdminDepositView>,
}

#[derive(Debug, Deserialize)]
pub struct ProcessAbnormalDepositRequest {
    pub tx_hash: String,
    pub decision: String,
    pub order_id: Option<u64>,
    pub processed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProcessAbnormalDepositResponse {
    pub tx_hash: String,
    pub deposit_status: String,
    pub membership_status: Option<MembershipStatus>,
    pub order_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSweepJobRequest {
    pub chain: String,
    pub asset: String,
    pub treasury_address: String,
    pub requested_at: DateTime<Utc>,
    pub transfers: Vec<CreateSweepTransferRequest>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSweepTransferRequest {
    pub from_address: String,
    pub amount: String,
}

#[derive(Debug, Serialize)]
pub struct SweepJobResponse {
    pub sweep_job_id: u64,
    pub chain: String,
    pub asset: String,
    pub status: String,
    pub requested_by: String,
    pub transfer_count: usize,
}

#[derive(Debug, Serialize)]
pub struct SweepJobListResponse {
    pub jobs: Vec<SweepJobResponse>,
}

impl Default for MembershipService {
    fn default() -> Self {
        Self::new(SharedDb::ephemeral().expect("ephemeral membership db should initialize"))
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
        self.bootstrap_defaults()?;

        let email = normalize_email(&request.email);
        let chain = normalize_chain(&request.chain);
        let asset = normalize_asset(&request.asset);
        let plan_code = normalize_code(&request.plan_code);

        if email.is_empty() || chain.is_empty() || asset.is_empty() || plan_code.is_empty() {
            return Err(MembershipError::bad_request(
                "email, chain, asset, and plan_code are required",
            ));
        }

        if !DEFAULT_CHAIN_CODES.contains(&chain.as_str()) {
            return Err(MembershipError::bad_request("unsupported chain"));
        }
        if !DEFAULT_ASSETS.contains(&asset.as_str()) {
            return Err(MembershipError::bad_request("unsupported asset"));
        }

        self.promote_queued_orders(request.requested_at)?;
        let plans = self.db.list_membership_plans().map_err(MembershipError::storage)?;
        let prices = self.db.list_plan_prices().map_err(MembershipError::storage)?;
        let _plan = plans
            .into_iter()
            .find(|plan| plan.code == plan_code && plan.is_active)
            .ok_or_else(|| MembershipError::bad_request("unknown plan_code"))?;
        let amount = prices
            .into_iter()
            .find(|price| price.plan_code == plan_code && price.chain == chain && price.asset == asset)
            .map(|price| price.amount)
            .ok_or_else(|| MembershipError::bad_request("price not configured for chain and asset"))?;

        let order_id = self
            .db
            .next_sequence("billing_order_id")
            .map_err(MembershipError::storage)?;

        self.db
            .insert_billing_order(&BillingOrderRecord {
                order_id,
                email: email.clone(),
                chain: chain.clone(),
                asset: asset.clone(),
                plan_code,
                amount: amount.clone(),
                requested_at: request.requested_at,
                assignment: None,
                paid_at: None,
                tx_hash: None,
                status: "queued".to_owned(),
                enqueued_at: Some(request.requested_at),
            })
            .map_err(MembershipError::storage)?;

        let assignment = self
            .db
            .allocate_or_queue_billing_order(order_id, &chain, request.requested_at)
            .map_err(MembershipError::storage)?;

        let refreshed = self
            .db
            .list_billing_orders()
            .map_err(MembershipError::storage)?;
        let queue_position = assignment
            .is_none()
            .then(|| queue_position_for(order_id, &chain, &refreshed))
            .flatten();

        Ok(CreateBillingOrderResponse {
            order_id,
            chain,
            asset,
            address: assignment.as_ref().map(|value| value.address.clone()),
            amount,
            expires_at: assignment.as_ref().map(|value| value.expires_at),
            status: MembershipStatus::Pending,
            queue_position,
        })
    }

    pub fn match_order(
        &self,
        actor_email: &str,
        request: MatchBillingOrderRequest,
    ) -> Result<MatchBillingOrderResponse, MembershipError> {
        self.bootstrap_defaults()?;

        let chain = normalize_chain(&request.chain);
        let asset = normalize_asset(&request.asset);
        let address = request.address.trim().to_owned();
        let tx_hash = request.tx_hash.trim().to_owned();
        let amount = canonicalize_amount(&request.amount)
            .map_err(|_| MembershipError::bad_request("invalid amount"))?;

        if chain.is_empty() || asset.is_empty() || address.is_empty() || tx_hash.is_empty() {
            return Err(MembershipError::bad_request(
                "chain, asset, address, amount, and tx_hash are required",
            ));
        }

        self.promote_queued_orders(request.observed_at)?;

        if !self
            .db
            .record_seen_transfer(&tx_hash, &chain, request.observed_at)
            .map_err(MembershipError::storage)?
        {
            return Ok(MatchBillingOrderResponse {
                matched: false,
                reason: Some("duplicate_transaction".to_owned()),
                order_id: None,
                email: None,
                membership_status: None,
                active_until: None,
                grace_until: None,
                deposit_status: "duplicate_ignored".to_owned(),
            });
        }

        let orders = self
            .db
            .list_billing_orders()
            .map_err(MembershipError::storage)?;
        let address_candidates: Vec<_> = orders
            .iter()
            .filter(|order| {
                order.paid_at.is_none()
                    && order.requested_at <= request.observed_at
                    && order.chain == chain
                    && order
                        .assignment
                        .as_ref()
                        .is_some_and(|assignment| assignment.address == address)
            })
            .cloned()
            .collect();

        let valid_candidates: Vec<_> = address_candidates
            .iter()
            .filter(|order| order.asset == asset && order.amount == amount)
            .filter(|order| {
                order
                    .assignment
                    .as_ref()
                    .is_some_and(|assignment| request.observed_at <= assignment.expires_at)
            })
            .cloned()
            .collect();

        if valid_candidates.len() == 1 {
            let order = valid_candidates.into_iter().next().expect("one candidate");
            let (active_until, grace_until) =
                self.apply_membership_entitlement(&order, &tx_hash, request.observed_at)?;
            let snapshot = snapshot_for(
                &order.email,
                self.db
                    .find_membership_record(&order.email)
                    .map_err(MembershipError::storage)?
                    .as_ref(),
                Some(request.observed_at),
            );
            self.insert_audit(AuditLogRecord {
                actor_email: actor_email.to_owned(),
                action: "membership.payment_applied".to_owned(),
                target_type: "membership_order".to_owned(),
                target_id: order.order_id.to_string(),
                payload: json!({
                    "tx_hash": tx_hash,
                    "chain": chain,
                    "asset": asset,
                    "active_until": active_until,
                    "grace_until": grace_until,
                }),
                created_at: request.observed_at,
            })?;

            return Ok(MatchBillingOrderResponse {
                matched: true,
                reason: None,
                order_id: Some(order.order_id),
                email: Some(order.email),
                membership_status: Some(snapshot.status),
                active_until: snapshot.active_until,
                grace_until: snapshot.grace_until,
                deposit_status: "matched".to_owned(),
            });
        }

        if valid_candidates.len() > 1 {
            self.record_abnormal_deposit(DepositTransactionRecord {
                tx_hash,
                chain,
                asset,
                address,
                amount,
                observed_at: request.observed_at,
                order_id: None,
                status: "manual_review_required".to_owned(),
                review_reason: Some("ambiguous_match".to_owned()),
                processed_at: None,
                matched_order_id: None,
            })?;
            return Ok(unmatched_response("ambiguous_match", "manual_review_required"));
        }

        if address_candidates.len() > 1 && valid_candidates.is_empty() {
            let reason = if address_candidates.iter().any(|order| order.asset != asset) {
                "wrong_asset"
            } else if address_candidates.iter().all(|order| {
                order
                    .assignment
                    .as_ref()
                    .is_some_and(|assignment| request.observed_at > assignment.expires_at)
            }) {
                "order_expired"
            } else {
                "exact_amount_required"
            };
            let order_id = if reason == "order_expired" {
                address_candidates.first().map(|order| order.order_id)
            } else {
                None
            };
            self.record_abnormal_deposit(DepositTransactionRecord {
                tx_hash: tx_hash.clone(),
                chain: chain.clone(),
                asset,
                address,
                amount,
                observed_at: request.observed_at,
                order_id,
                status: "manual_review_required".to_owned(),
                review_reason: Some(reason.to_owned()),
                processed_at: None,
                matched_order_id: None,
            })?;
            return Ok(unmatched_response(reason, "manual_review_required"));
        }

        if let Some(order) = address_candidates.first() {
            if order.asset != asset {
                self.record_abnormal_deposit(DepositTransactionRecord {
                    tx_hash,
                    chain,
                    asset,
                    address,
                    amount,
                    observed_at: request.observed_at,
                    order_id: Some(order.order_id),
                    status: "manual_review_required".to_owned(),
                    review_reason: Some("wrong_asset".to_owned()),
                    processed_at: None,
                    matched_order_id: None,
                })?;
                return Ok(unmatched_response("wrong_asset", "manual_review_required"));
            }

            if order.amount != amount {
                self.record_abnormal_deposit(DepositTransactionRecord {
                    tx_hash,
                    chain,
                    asset,
                    address,
                    amount,
                    observed_at: request.observed_at,
                    order_id: Some(order.order_id),
                    status: "manual_review_required".to_owned(),
                    review_reason: Some("exact_amount_required".to_owned()),
                    processed_at: None,
                    matched_order_id: None,
                })?;
                return Ok(unmatched_response(
                    "exact_amount_required",
                    "manual_review_required",
                ));
            }

            if order
                .assignment
                .as_ref()
                .is_some_and(|assignment| request.observed_at > assignment.expires_at)
            {
                self.record_abnormal_deposit(DepositTransactionRecord {
                    tx_hash,
                    chain,
                    asset,
                    address,
                    amount,
                    observed_at: request.observed_at,
                    order_id: Some(order.order_id),
                    status: "manual_review_required".to_owned(),
                    review_reason: Some("order_expired".to_owned()),
                    processed_at: None,
                    matched_order_id: None,
                })?;
                return Ok(unmatched_response("order_expired", "manual_review_required"));
            }
        }

        self.record_abnormal_deposit(DepositTransactionRecord {
            tx_hash,
            chain,
            asset,
            address,
            amount,
            observed_at: request.observed_at,
            order_id: None,
            status: "manual_review_required".to_owned(),
            review_reason: Some("order_not_found".to_owned()),
            processed_at: None,
            matched_order_id: None,
        })?;
        Ok(unmatched_response("order_not_found", "manual_review_required"))
    }

    pub fn membership_status(
        &self,
        request: MembershipStatusRequest,
    ) -> Result<MembershipSnapshot, MembershipError> {
        self.bootstrap_defaults()?;
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
        actor_email: &str,
        request: MembershipOverrideRequest,
    ) -> Result<MembershipSnapshot, MembershipError> {
        self.bootstrap_defaults()?;
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

        self.insert_audit(AuditLogRecord {
            actor_email: actor_email.to_owned(),
            action: "membership.override_updated".to_owned(),
            target_type: "membership".to_owned(),
            target_id: email.clone(),
            payload: json!({
                "override_status": request.status,
            }),
            created_at: request.at.unwrap_or_else(Utc::now),
        })?;

        Ok(snapshot_for(&email, record.as_ref(), effective_at))
    }

    pub fn manage_membership(
        &self,
        actor_email: &str,
        request: ManualMembershipRequest,
    ) -> Result<MembershipSnapshot, MembershipError> {
        self.bootstrap_defaults()?;
        let email = normalize_email(&request.email);
        let action = normalize_code(&request.action);
        if email.is_empty() {
            return Err(MembershipError::bad_request("email is required"));
        }

        let current = self
            .db
            .find_membership_record(&email)
            .map_err(MembershipError::storage)?
            .unwrap_or_default();

        let updated = match action.as_str() {
            "open" => {
                let duration_days = request
                    .duration_days
                    .ok_or_else(|| MembershipError::bad_request("duration_days is required"))?;
                MembershipRecord {
                    activated_at: Some(request.at),
                    active_until: Some(request.at + Duration::days(duration_days)),
                    grace_until: Some(request.at + Duration::days(duration_days) + Duration::hours(GRACE_HOURS)),
                    override_status: None,
                }
            }
            "extend" => {
                let duration_days = request
                    .duration_days
                    .ok_or_else(|| MembershipError::bad_request("duration_days is required"))?;
                let base = if current
                    .grace_until
                    .is_some_and(|grace_until| request.at <= grace_until)
                {
                    current.active_until.unwrap_or(request.at)
                } else {
                    current
                        .active_until
                        .filter(|until| *until > request.at)
                        .unwrap_or(request.at)
                };
                let active_until = base + Duration::days(duration_days);
                MembershipRecord {
                    activated_at: current.activated_at.or(Some(request.at)),
                    active_until: Some(active_until),
                    grace_until: Some(active_until + Duration::hours(GRACE_HOURS)),
                    override_status: None,
                }
            }
            "unfreeze" => MembershipRecord {
                override_status: None,
                ..current.clone()
            },
            _ => return Err(MembershipError::bad_request("unsupported membership action")),
        };

        self.db
            .upsert_membership_record(&email, &updated)
            .map_err(MembershipError::storage)?;

        let action_name = match action.as_str() {
            "open" => "membership.manual_opened",
            "extend" => "membership.manual_extended",
            "unfreeze" => "membership.manual_unfrozen",
            _ => unreachable!(),
        };
        self.insert_audit(AuditLogRecord {
            actor_email: actor_email.to_owned(),
            action: action_name.to_owned(),
            target_type: "membership".to_owned(),
            target_id: email.clone(),
            payload: json!({
                "duration_days": request.duration_days,
                "at": request.at,
            }),
            created_at: request.at,
        })?;

        let record = self
            .db
            .find_membership_record(&email)
            .map_err(MembershipError::storage)?;
        Ok(snapshot_for(&email, record.as_ref(), Some(request.at)))
    }

    pub fn list_plan_configs(&self) -> Result<MembershipPlanConfigListResponse, MembershipError> {
        self.bootstrap_defaults()?;
        let plans = self.db.list_membership_plans().map_err(MembershipError::storage)?;
        let prices = self.db.list_plan_prices().map_err(MembershipError::storage)?;
        Ok(MembershipPlanConfigListResponse {
            plans: plans
                .into_iter()
                .map(|plan| MembershipPlanConfigResponse {
                    code: plan.code.clone(),
                    name: plan.name,
                    duration_days: plan.duration_days,
                    is_active: plan.is_active,
                    prices: prices
                        .iter()
                        .filter(|price| price.plan_code == plan.code)
                        .map(|price| MembershipPlanPriceResponse {
                            chain: price.chain.clone(),
                            asset: price.asset.clone(),
                            amount: price.amount.clone(),
                        })
                        .collect(),
                })
                .collect(),
        })
    }

    pub fn upsert_plan_config(
        &self,
        actor_email: &str,
        request: UpsertMembershipPlanRequest,
    ) -> Result<MembershipPlanConfigResponse, MembershipError> {
        self.bootstrap_defaults()?;
        let code = normalize_code(&request.code);
        let name = request.name.trim().to_owned();
        if code.is_empty() || name.is_empty() || request.duration_days <= 0 {
            return Err(MembershipError::bad_request(
                "code, name, and positive duration_days are required",
            ));
        }
        if request.prices.is_empty() {
            return Err(MembershipError::bad_request("at least one price is required"));
        }

        let mut prices = Vec::with_capacity(request.prices.len());
        for price in request.prices {
            let chain = normalize_chain(&price.chain);
            let asset = normalize_asset(&price.asset);
            if !DEFAULT_CHAIN_CODES.contains(&chain.as_str()) || !DEFAULT_ASSETS.contains(&asset.as_str()) {
                return Err(MembershipError::bad_request("unsupported chain or asset"));
            }
            let amount = canonicalize_amount(&price.amount)
                .map_err(|_| MembershipError::bad_request("invalid amount"))?;
            prices.push(MembershipPlanPriceResponse { chain, asset, amount });
        }

        let stored_prices = prices
            .iter()
            .map(|price| MembershipPlanPriceRecord {
                plan_code: code.clone(),
                chain: price.chain.clone(),
                asset: price.asset.clone(),
                amount: price.amount.clone(),
            })
            .collect::<Vec<_>>();
        self.db
            .upsert_membership_plan_with_prices(
                &MembershipPlanRecord {
                    code: code.clone(),
                    name: name.clone(),
                    duration_days: request.duration_days,
                    is_active: request.is_active,
                },
                &stored_prices,
            )
            .map_err(MembershipError::storage)?;

        self.insert_audit(AuditLogRecord {
            actor_email: actor_email.to_owned(),
            action: "membership.plan_config_updated".to_owned(),
            target_type: "membership_plan".to_owned(),
            target_id: code.clone(),
            payload: json!({
                "duration_days": request.duration_days,
                "is_active": request.is_active,
                "price_count": prices.len(),
            }),
            created_at: Utc::now(),
        })?;

        Ok(MembershipPlanConfigResponse {
            code,
            name,
            duration_days: request.duration_days,
            is_active: request.is_active,
            prices,
        })
    }

    pub fn list_address_pools(&self) -> Result<AddressPoolListResponse, MembershipError> {
        self.bootstrap_defaults()?;
        let mut addresses = self
            .db
            .list_deposit_addresses()
            .map_err(MembershipError::storage)?
            .into_iter()
            .map(|record| AddressPoolEntryResponse {
                chain: record.chain,
                address: record.address,
                is_enabled: record.is_enabled,
            })
            .collect::<Vec<_>>();
        addresses.sort_by(|left, right| left.chain.cmp(&right.chain).then_with(|| left.address.cmp(&right.address)));
        Ok(AddressPoolListResponse { addresses })
    }

    pub fn upsert_address_pool_entry(
        &self,
        actor_email: &str,
        request: UpsertAddressPoolEntryRequest,
    ) -> Result<AddressPoolEntryResponse, MembershipError> {
        self.bootstrap_defaults()?;
        let chain = normalize_chain(&request.chain);
        let address = request.address.trim().to_owned();
        if chain.is_empty() || address.is_empty() {
            return Err(MembershipError::bad_request("chain and address are required"));
        }
        if !DEFAULT_CHAIN_CODES.contains(&chain.as_str()) {
            return Err(MembershipError::bad_request("unsupported chain"));
        }

        self.db
            .upsert_deposit_address(&DepositAddressPoolRecord {
                chain: chain.clone(),
                address: address.clone(),
                is_enabled: request.is_enabled,
            })
            .map_err(MembershipError::storage)?;
        self.insert_audit(AuditLogRecord {
            actor_email: actor_email.to_owned(),
            action: "billing.address_pool_updated".to_owned(),
            target_type: "deposit_address".to_owned(),
            target_id: format!("{chain}:{address}"),
            payload: json!({ "is_enabled": request.is_enabled }),
            created_at: Utc::now(),
        })?;
        Ok(AddressPoolEntryResponse {
            chain,
            address,
            is_enabled: request.is_enabled,
        })
    }

    pub fn admin_list_deposits(
        &self,
        at: DateTime<Utc>,
    ) -> Result<AdminDepositsResponse, MembershipError> {
        self.bootstrap_defaults()?;
        self.promote_queued_orders(at)?;
        let orders = self
            .db
            .list_billing_orders()
            .map_err(MembershipError::storage)?;
        let abnormal_deposits = self
            .db
            .list_deposit_transactions()
            .map_err(MembershipError::storage)?
            .into_iter()
            .filter(|deposit| deposit.status != "matched")
            .map(|deposit| AdminDepositView {
                tx_hash: deposit.tx_hash,
                chain: deposit.chain,
                asset: deposit.asset,
                address: deposit.address,
                amount: deposit.amount,
                status: deposit.status,
                review_reason: deposit.review_reason,
                order_id: deposit.order_id,
                matched_order_id: deposit.matched_order_id,
            })
            .collect();

        Ok(AdminDepositsResponse {
            orders: orders
                .iter()
                .map(|order| AdminBillingOrderView {
                    order_id: order.order_id,
                    email: order.email.clone(),
                    chain: order.chain.clone(),
                    asset: order.asset.clone(),
                    address: order.assignment.as_ref().map(|assignment| assignment.address.clone()),
                    amount: order.amount.clone(),
                    status: order.status.clone(),
                    queue_position: if order.status == "queued" {
                        queue_position_for(order.order_id, &order.chain, &orders)
                    } else {
                        None
                    },
                })
                .collect(),
            abnormal_deposits,
        })
    }

    pub fn process_abnormal_deposit(
        &self,
        actor_email: &str,
        request: ProcessAbnormalDepositRequest,
    ) -> Result<ProcessAbnormalDepositResponse, MembershipError> {
        self.bootstrap_defaults()?;
        let tx_hash = request.tx_hash.trim().to_owned();
        let decision = normalize_code(&request.decision);
        let mut deposits = self
            .db
            .list_deposit_transactions()
            .map_err(MembershipError::storage)?;
        let matching_indexes = deposits
            .iter()
            .enumerate()
            .filter_map(|(index, deposit)| (deposit.tx_hash == tx_hash).then_some(index))
            .collect::<Vec<_>>();
        if matching_indexes.len() > 1 {
            return Err(MembershipError::bad_request(
                "multiple deposits share this tx_hash across chains; refine the lookup",
            ));
        }
        let deposit = matching_indexes
            .into_iter()
            .next()
            .and_then(|index| deposits.get_mut(index))
            .ok_or_else(|| MembershipError::not_found("deposit not found"))?;

        if deposit.status != "manual_review_required" {
            return Err(MembershipError::bad_request("deposit is not pending manual review"));
        }

        match decision.as_str() {
            "credit_membership" => {
                let order_id = request
                    .order_id
                    .or(deposit.order_id)
                    .ok_or_else(|| MembershipError::bad_request("order_id is required"))?;
                let orders = self
                    .db
                    .list_billing_orders()
                    .map_err(MembershipError::storage)?;
                let order = orders
                    .into_iter()
                    .find(|order| order.order_id == order_id)
                    .ok_or_else(|| MembershipError::not_found("order not found"))?;
                if order.paid_at.is_some() {
                    return Err(MembershipError::bad_request("order already paid"));
                }
                if deposit.order_id.is_some()
                    && deposit.order_id != Some(order_id)
                    && !matches!(
                        deposit.review_reason.as_deref(),
                        Some("order_not_found") | Some("ambiguous_match")
                    )
                {
                    return Err(MembershipError::bad_request(
                        "manual credit cannot reassign this deposit to a different order",
                    ));
                }
                let (_active_until, _grace_until) =
                    self.apply_membership_entitlement(&order, &tx_hash, request.processed_at)?;
                deposit.status = "manual_approved".to_owned();
                deposit.processed_at = Some(request.processed_at);
                deposit.order_id = Some(order_id);
                deposit.matched_order_id = Some(order_id);
                self.db
                    .upsert_deposit_transaction(deposit)
                    .map_err(MembershipError::storage)?;
                self.insert_audit(AuditLogRecord {
                    actor_email: actor_email.to_owned(),
                    action: "deposit.manual_credited".to_owned(),
                    target_type: "deposit".to_owned(),
                    target_id: tx_hash.clone(),
                    payload: json!({ "order_id": order_id }),
                    created_at: request.processed_at,
                })?;
                let snapshot = snapshot_for(
                    &order.email,
                    self.db
                        .find_membership_record(&order.email)
                        .map_err(MembershipError::storage)?
                        .as_ref(),
                    Some(request.processed_at),
                );
                Ok(ProcessAbnormalDepositResponse {
                    tx_hash,
                    deposit_status: "manual_approved".to_owned(),
                    membership_status: Some(snapshot.status),
                    order_id: Some(order_id),
                })
            }
            "reject" => {
                deposit.status = "manual_rejected".to_owned();
                deposit.processed_at = Some(request.processed_at);
                self.db
                    .upsert_deposit_transaction(deposit)
                    .map_err(MembershipError::storage)?;
                self.insert_audit(AuditLogRecord {
                    actor_email: actor_email.to_owned(),
                    action: "deposit.manual_rejected".to_owned(),
                    target_type: "deposit".to_owned(),
                    target_id: tx_hash.clone(),
                    payload: json!({ "decision": "reject" }),
                    created_at: request.processed_at,
                })?;
                Ok(ProcessAbnormalDepositResponse {
                    tx_hash,
                    deposit_status: "manual_rejected".to_owned(),
                    membership_status: None,
                    order_id: None,
                })
            }
            _ => Err(MembershipError::bad_request("unsupported decision")),
        }
    }

    pub fn create_sweep_job(
        &self,
        actor_email: &str,
        request: CreateSweepJobRequest,
    ) -> Result<SweepJobResponse, MembershipError> {
        self.bootstrap_defaults()?;
        let chain = normalize_chain(&request.chain);
        let asset = normalize_asset(&request.asset);
        let treasury_address = request.treasury_address.trim().to_owned();
        if chain.is_empty()
            || asset.is_empty()
            || treasury_address.is_empty()
            || request.transfers.is_empty()
        {
            return Err(MembershipError::bad_request(
                "chain, asset, treasury_address, and transfers are required",
            ));
        }

        let sweep_job_id = self
            .db
            .next_sequence("sweep_job_id")
            .map_err(MembershipError::storage)?;
        let transfers = request
            .transfers
            .into_iter()
            .map(|transfer| {
                canonicalize_amount(&transfer.amount)
                    .map_err(|_| MembershipError::bad_request("invalid amount"))
                    .map(|amount| SweepTransferRecord {
                        from_address: transfer.from_address,
                        to_address: treasury_address.clone(),
                        amount,
                        tx_hash: None,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let job = SweepJobRecord {
            sweep_job_id,
            chain: chain.clone(),
            asset: asset.clone(),
            status: "queued".to_owned(),
            requested_by: actor_email.to_owned(),
            requested_at: request.requested_at,
            completed_at: None,
            transfers: transfers.clone(),
        };
        self.db
            .create_sweep_job(&job)
            .map_err(MembershipError::storage)?;
        self.insert_audit(AuditLogRecord {
            actor_email: actor_email.to_owned(),
            action: "treasury.sweep_requested".to_owned(),
            target_type: "sweep_job".to_owned(),
            target_id: sweep_job_id.to_string(),
            payload: json!({
                "chain": chain,
                "asset": asset,
                "transfer_count": transfers.len(),
            }),
            created_at: request.requested_at,
        })?;
        Ok(SweepJobResponse {
            sweep_job_id,
            chain,
            asset,
            status: "queued".to_owned(),
            requested_by: actor_email.to_owned(),
            transfer_count: transfers.len(),
        })
    }

    pub fn list_sweep_jobs(&self) -> Result<SweepJobListResponse, MembershipError> {
        self.bootstrap_defaults()?;
        let jobs = self
            .db
            .list_sweep_jobs()
            .map_err(MembershipError::storage)?;
        Ok(SweepJobListResponse {
            jobs: jobs
                .into_iter()
                .map(|job| SweepJobResponse {
                    sweep_job_id: job.sweep_job_id,
                    chain: job.chain,
                    asset: job.asset,
                    status: job.status,
                    requested_by: job.requested_by,
                    transfer_count: job.transfers.len(),
                })
                .collect(),
        })
    }

    fn bootstrap_defaults(&self) -> Result<(), MembershipError> {
        let existing_plans = self
            .db
            .list_membership_plans()
            .map_err(MembershipError::storage)?;
        let existing_prices = self
            .db
            .list_plan_prices()
            .map_err(MembershipError::storage)?;
        let existing_addresses = self
            .db
            .list_deposit_addresses()
            .map_err(MembershipError::storage)?;
        for (code, name, duration_days, amount) in DEFAULT_PLAN_CONFIG {
            if !existing_plans.iter().any(|plan| plan.code == code) {
                self.db
                    .upsert_membership_plan(&MembershipPlanRecord {
                        code: code.to_owned(),
                        name: name.to_owned(),
                        duration_days,
                        is_active: true,
                    })
                    .map_err(MembershipError::storage)?;
            }
            for chain in DEFAULT_CHAIN_CODES {
                for asset in DEFAULT_ASSETS {
                    if !existing_prices.iter().any(|price| {
                        price.plan_code == code && price.chain == chain && price.asset == asset
                    }) {
                        self.db
                            .upsert_plan_price(&MembershipPlanPriceRecord {
                                plan_code: code.to_owned(),
                                chain: chain.to_owned(),
                                asset: asset.to_owned(),
                                amount: amount.to_owned(),
                            })
                            .map_err(MembershipError::storage)?;
                    }
                }
            }
        }

        for (chain, addresses) in [
            ("ETH", DEFAULT_ETH_ADDRESSES.as_slice()),
            ("BSC", DEFAULT_BSC_ADDRESSES.as_slice()),
            ("SOL", DEFAULT_SOL_ADDRESSES.as_slice()),
        ] {
            for address in addresses {
                if existing_addresses
                    .iter()
                    .any(|record| record.chain == chain && record.address == *address)
                {
                    continue;
                }
                self.db
                    .upsert_deposit_address(&DepositAddressPoolRecord {
                        chain: chain.to_owned(),
                        address: (*address).to_owned(),
                        is_enabled: true,
                    })
                    .map_err(MembershipError::storage)?;
            }
        }

        Ok(())
    }

    fn promote_queued_orders(&self, at: DateTime<Utc>) -> Result<(), MembershipError> {
        let addresses = self
            .db
            .list_deposit_addresses()
            .map_err(MembershipError::storage)?;
        let orders = self
            .db
            .list_billing_orders()
            .map_err(MembershipError::storage)?;
        let mut chains = Vec::new();
        for order in &orders {
            if !chains.contains(&order.chain) {
                chains.push(order.chain.clone());
            }
        }
        for address in &addresses {
            if !chains.contains(&address.chain) {
                chains.push(address.chain.clone());
            }
        }

        for chain in chains {
            let queued_orders = queued_orders_for(&chain, &orders);
            if queued_orders.is_empty() {
                continue;
            }

            for order in queued_orders {
                self.db
                    .allocate_or_queue_billing_order(order.order_id, &chain, at)
                    .map_err(MembershipError::storage)?;
            }
        }

        Ok(())
    }

    fn apply_membership_entitlement(
        &self,
        order: &BillingOrderRecord,
        tx_hash: &str,
        paid_at: DateTime<Utc>,
    ) -> Result<(DateTime<Utc>, DateTime<Utc>), MembershipError> {
        let plan = self
            .db
            .list_membership_plans()
            .map_err(MembershipError::storage)?
            .into_iter()
            .find(|plan| plan.code == order.plan_code)
            .ok_or_else(|| MembershipError::bad_request("plan not configured"))?;
        let current = self
            .db
            .find_membership_record(&order.email)
            .map_err(MembershipError::storage)?;

        let base = current
            .as_ref()
            .and_then(|record| record.active_until)
            .filter(|active_until| {
                current
                    .as_ref()
                    .and_then(|record| record.grace_until)
                    .is_some_and(|grace_until| paid_at <= grace_until)
                    || *active_until >= paid_at
            })
            .unwrap_or(paid_at);
        let active_until = base + Duration::days(i64::from(plan.duration_days));
        let grace_until = active_until + Duration::hours(GRACE_HOURS);

        self.db
            .apply_membership_payment(
                order.order_id,
                &order.chain,
                tx_hash,
                paid_at,
                &order.email,
                active_until,
                grace_until,
            )
            .map_err(MembershipError::storage)?;
        Ok((active_until, grace_until))
    }

    fn record_abnormal_deposit(
        &self,
        record: DepositTransactionRecord,
    ) -> Result<(), MembershipError> {
        self.db
            .upsert_deposit_transaction(&record)
            .map_err(MembershipError::storage)
    }

    fn insert_audit(&self, record: AuditLogRecord) -> Result<(), MembershipError> {
        let _ = self.db.insert_audit_log(&record);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MembershipError {
    status: StatusCode,
    message: String,
}

impl MembershipError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn storage(_error: shared_db::SharedDbError) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal storage error".to_owned(),
        }
    }
}

impl IntoResponse for MembershipError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(MembershipErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

impl From<AuthError> for MembershipError {
    fn from(value: AuthError) -> Self {
        Self {
            status: value.status,
            message: value.message.to_owned(),
        }
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

fn unmatched_response(reason: &str, deposit_status: &str) -> MatchBillingOrderResponse {
    MatchBillingOrderResponse {
        matched: false,
        reason: Some(reason.to_owned()),
        order_id: None,
        email: None,
        membership_status: None,
        active_until: None,
        grace_until: None,
        deposit_status: deposit_status.to_owned(),
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
fn queued_orders_for<'a>(chain: &str, orders: &'a [BillingOrderRecord]) -> Vec<&'a BillingOrderRecord> {
    let mut queued = orders
        .iter()
        .filter(|order| order.chain == chain && order.paid_at.is_none() && order.status == "queued")
        .collect::<Vec<_>>();
    queued.sort_by_key(|order| order.enqueued_at.unwrap_or(order.requested_at));
    queued
}

fn queue_position_for(order_id: u64, chain: &str, orders: &[BillingOrderRecord]) -> Option<u64> {
    queued_orders_for(chain, orders)
        .iter()
        .position(|order| order.order_id == order_id)
        .map(|index| index as u64 + 1)
}

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

fn normalize_chain(chain: &str) -> String {
    chain.trim().to_uppercase()
}

fn normalize_asset(asset: &str) -> String {
    asset.trim().to_uppercase()
}

fn normalize_code(value: &str) -> String {
    value.trim().to_lowercase()
}
