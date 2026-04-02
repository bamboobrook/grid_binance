use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Row, Transaction};

use shared_chain::assignment::AddressAssignment;
use shared_domain::membership::MembershipStatus;

use crate::{membership_status_to_str, parse_membership_status, SharedDbError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BillingOrderRecord {
    pub order_id: u64,
    pub email: String,
    pub chain: String,
    pub asset: String,
    pub plan_code: String,
    pub amount: String,
    pub requested_at: DateTime<Utc>,
    pub assignment: Option<AddressAssignment>,
    pub paid_at: Option<DateTime<Utc>>,
    pub tx_hash: Option<String>,
    pub status: String,
    pub enqueued_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MembershipRecord {
    pub activated_at: Option<DateTime<Utc>>,
    pub active_until: Option<DateTime<Utc>>,
    pub grace_until: Option<DateTime<Utc>>,
    pub override_status: Option<MembershipStatus>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MembershipPlanRecord {
    pub code: String,
    pub name: String,
    pub duration_days: i32,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MembershipPlanPriceRecord {
    pub plan_code: String,
    pub chain: String,
    pub asset: String,
    pub amount: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DepositAddressPoolRecord {
    pub chain: String,
    pub address: String,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepositTransactionRecord {
    pub tx_hash: String,
    pub chain: String,
    pub asset: String,
    pub address: String,
    pub amount: String,
    pub observed_at: DateTime<Utc>,
    pub order_id: Option<u64>,
    pub status: String,
    pub review_reason: Option<String>,
    pub processed_at: Option<DateTime<Utc>>,
    pub matched_order_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SweepTransferRecord {
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    pub tx_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SweepJobRecord {
    pub sweep_job_id: u64,
    pub chain: String,
    pub asset: String,
    pub status: String,
    pub requested_by: String,
    pub requested_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub transfers: Vec<SweepTransferRecord>,
}

#[derive(Clone)]
pub struct BillingRepository {
    pool: PgPool,
}

impl BillingRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_orders(&self) -> Result<Vec<BillingOrderRecord>, SharedDbError> {
        let rows = sqlx::query(
            "SELECT mo.order_id,
                    mo.user_email,
                    mo.chain,
                    COALESCE(sc.config_value->>'asset', '') AS asset,
                    mo.plan_code,
                    mo.amount,
                    mo.requested_at,
                    mo.assigned_address,
                    mo.address_expires_at,
                    mo.paid_at,
                    mo.tx_hash,
                    mo.status,
                    doq.enqueued_at
             FROM membership_orders mo
             LEFT JOIN deposit_order_queue doq ON doq.order_id = mo.order_id
             LEFT JOIN system_configs sc
               ON sc.config_key = ('billing.order.' || mo.order_id::text || '.meta')
             ORDER BY mo.order_id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        rows.into_iter().map(billing_order_from_row).collect()
    }

    pub async fn insert_order(&self, order: &BillingOrderRecord) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        sqlx::query(
            "INSERT INTO membership_orders (
                order_id,
                user_email,
                chain,
                plan_code,
                amount,
                requested_at,
                assigned_address,
                address_expires_at,
                paid_at,
                tx_hash,
                status,
                updated_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, now())",
        )
        .bind(order.order_id as i64)
        .bind(&order.email)
        .bind(&order.chain)
        .bind(&order.plan_code)
        .bind(&order.amount)
        .bind(order.requested_at)
        .bind(
            order
                .assignment
                .as_ref()
                .map(|assignment| assignment.address.as_str())
                .unwrap_or(""),
        )
        .bind(
            order
                .assignment
                .as_ref()
                .map(|assignment| assignment.expires_at)
                .unwrap_or(order.requested_at),
        )
        .bind(order.paid_at)
        .bind(&order.tx_hash)
        .bind(&order.status)
        .execute(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?;

        write_order_meta(
            &mut transaction,
            order.order_id,
            json!({ "asset": order.asset }),
        )
        .await?;

        if let Some(enqueued_at) = order.enqueued_at {
            sqlx::query(
                "INSERT INTO deposit_order_queue (order_id, chain, enqueued_at)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (order_id) DO UPDATE
                 SET chain = excluded.chain,
                     enqueued_at = excluded.enqueued_at",
            )
            .bind(order.order_id as i64)
            .bind(&order.chain)
            .bind(enqueued_at)
            .execute(&mut *transaction)
            .await
            .map_err(SharedDbError::from)?;
        }

        if let Some(assignment) = &order.assignment {
            upsert_allocation_in(&mut transaction, order.order_id, assignment).await?;
        }

        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn allocate_or_queue_order(
        &self,
        order_id: u64,
        chain: &str,
        requested_at: DateTime<Utc>,
    ) -> Result<Option<AddressAssignment>, SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        release_expired_allocations_in(&mut transaction, chain, requested_at).await?;

        let existing = sqlx::query(
            "SELECT assigned_address, address_expires_at, status, paid_at
             FROM membership_orders
             WHERE order_id = $1",
        )
        .bind(order_id as i64)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?;
        let Some(existing) = existing else {
            transaction.rollback().await.map_err(SharedDbError::from)?;
            return Err(SharedDbError::new("billing order not found"));
        };

        let paid_at: Option<DateTime<Utc>> = existing.try_get("paid_at").map_err(SharedDbError::from)?;
        if paid_at.is_some() {
            transaction.rollback().await.map_err(SharedDbError::from)?;
            return Ok(None);
        }

        let current_address: String = existing.try_get("assigned_address").map_err(SharedDbError::from)?;
        let current_expires_at: DateTime<Utc> =
            existing.try_get("address_expires_at").map_err(SharedDbError::from)?;
        if !current_address.is_empty() && current_expires_at > requested_at {
            transaction.rollback().await.map_err(SharedDbError::from)?;
            return Ok(Some(AddressAssignment {
                chain: chain.to_owned(),
                address: current_address,
                expires_at: current_expires_at,
            }));
        }

        sqlx::query(
            "UPDATE deposit_address_allocations
             SET released_at = $2
             WHERE order_id = $1
               AND released_at IS NULL
               AND expires_at <= $2",
        )
        .bind(order_id as i64)
        .bind(requested_at)
        .execute(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?;

        let candidates = sqlx::query(
            "SELECT dap.address
             FROM deposit_address_pool dap
             LEFT JOIN LATERAL (
                SELECT da.created_at
                FROM deposit_address_allocations da
                WHERE da.chain = dap.chain
                  AND da.address = dap.address
                ORDER BY da.created_at DESC
                LIMIT 1
             ) last_alloc ON TRUE
             WHERE dap.chain = $1
               AND dap.is_enabled = TRUE
             ORDER BY last_alloc.created_at NULLS FIRST, dap.address ASC",
        )
        .bind(chain)
        .fetch_all(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?;

        let mut allocated = None;
        for row in candidates {
            let address: String = row.try_get("address").map_err(SharedDbError::from)?;
            let assignment = AddressAssignment {
                chain: chain.to_owned(),
                address,
                expires_at: requested_at + chrono::Duration::hours(1),
            };
            let inserted = sqlx::query(
                "INSERT INTO deposit_address_allocations (order_id, chain, address, expires_at, created_at)
                 VALUES ($1, $2, $3, $4, now())
                 ON CONFLICT DO NOTHING",
            )
            .bind(order_id as i64)
            .bind(&assignment.chain)
            .bind(&assignment.address)
            .bind(assignment.expires_at)
            .execute(&mut *transaction)
            .await
            .map_err(SharedDbError::from)?;
            if inserted.rows_affected() == 1 {
                sqlx::query(
                    "UPDATE membership_orders
                     SET assigned_address = $2,
                         address_expires_at = $3,
                         status = 'pending',
                         updated_at = now()
                     WHERE order_id = $1",
                )
                .bind(order_id as i64)
                .bind(&assignment.address)
                .bind(assignment.expires_at)
                .execute(&mut *transaction)
                .await
                .map_err(SharedDbError::from)?;
                sqlx::query("DELETE FROM deposit_order_queue WHERE order_id = $1")
                    .bind(order_id as i64)
                    .execute(&mut *transaction)
                    .await
                    .map_err(SharedDbError::from)?;
                allocated = Some(assignment);
                break;
            }
        }

        if allocated.is_none() {
            sqlx::query(
                "INSERT INTO deposit_order_queue (order_id, chain, enqueued_at)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (order_id) DO UPDATE
                 SET chain = excluded.chain,
                     enqueued_at = excluded.enqueued_at",
            )
            .bind(order_id as i64)
            .bind(chain)
            .bind(requested_at)
            .execute(&mut *transaction)
            .await
            .map_err(SharedDbError::from)?;
            sqlx::query(
                "UPDATE membership_orders
                 SET assigned_address = '',
                     address_expires_at = $2,
                     status = 'queued',
                     updated_at = now()
                 WHERE order_id = $1",
            )
            .bind(order_id as i64)
            .bind(requested_at)
            .execute(&mut *transaction)
            .await
            .map_err(SharedDbError::from)?;
        }

        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(allocated)
    }

    pub async fn update_order_assignment(
        &self,
        order_id: u64,
        assignment: &AddressAssignment,
        status: &str,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        sqlx::query(
            "UPDATE membership_orders
             SET assigned_address = $2,
                 address_expires_at = $3,
                 status = $4,
                 updated_at = now()
             WHERE order_id = $1",
        )
        .bind(order_id as i64)
        .bind(&assignment.address)
        .bind(assignment.expires_at)
        .bind(status)
        .execute(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?;

        sqlx::query("DELETE FROM deposit_order_queue WHERE order_id = $1")
            .bind(order_id as i64)
            .execute(&mut *transaction)
            .await
            .map_err(SharedDbError::from)?;

        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn record_seen_transfer(
        &self,
        tx_hash: &str,
        chain: &str,
        observed_at: DateTime<Utc>,
    ) -> Result<bool, SharedDbError> {
        let inserted = sqlx::query(
            "INSERT INTO deposit_transactions (
                tx_hash,
                chain,
                observed_at,
                status,
                raw_payload,
                created_at,
                updated_at
             )
             VALUES ($1, $2, $3, 'observed', '{}'::jsonb, now(), now())
             ON CONFLICT (chain, tx_hash) DO NOTHING",
        )
        .bind(tx_hash)
        .bind(chain)
        .bind(observed_at)
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(inserted.rows_affected() == 1)
    }

    pub async fn upsert_deposit_transaction(
        &self,
        record: &DepositTransactionRecord,
    ) -> Result<(), SharedDbError> {
        sqlx::query(
            "INSERT INTO deposit_transactions (
                tx_hash,
                chain,
                order_id,
                observed_at,
                status,
                raw_payload,
                created_at,
                updated_at
             )
             VALUES ($1, $2, $3, $4, $5, $6, now(), now())
             ON CONFLICT (chain, tx_hash) DO UPDATE
             SET chain = excluded.chain,
                 order_id = excluded.order_id,
                 observed_at = excluded.observed_at,
                 status = excluded.status,
                 raw_payload = excluded.raw_payload,
                 updated_at = now()",
        )
        .bind(&record.tx_hash)
        .bind(&record.chain)
        .bind(record.order_id.map(|value| value as i64))
        .bind(record.observed_at)
        .bind(&record.status)
        .bind(deposit_payload(record))
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn list_deposit_transactions(
        &self,
    ) -> Result<Vec<DepositTransactionRecord>, SharedDbError> {
        let rows = sqlx::query(
            "SELECT tx_hash, chain, order_id, observed_at, status, raw_payload
             FROM deposit_transactions
             ORDER BY observed_at ASC, tx_hash ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        rows.into_iter().map(deposit_from_row).collect()
    }

    pub async fn list_membership_records(&self) -> Result<Vec<(String, MembershipRecord)>, SharedDbError> {
        let rows = sqlx::query(
            "SELECT user_email, activated_at, active_until, grace_until, override_status
             FROM membership_entitlements
             ORDER BY updated_at DESC, user_email ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        rows.into_iter()
            .map(|row| {
                let override_status: Option<String> =
                    row.try_get("override_status").map_err(SharedDbError::from)?;
                Ok((
                    row.try_get("user_email").map_err(SharedDbError::from)?,
                    MembershipRecord {
                        activated_at: row.try_get("activated_at").map_err(SharedDbError::from)?,
                        active_until: row.try_get("active_until").map_err(SharedDbError::from)?,
                        grace_until: row.try_get("grace_until").map_err(SharedDbError::from)?,
                        override_status: override_status
                            .as_deref()
                            .map(parse_membership_status)
                            .transpose()?,
                    },
                ))
            })
            .collect()
    }

    pub async fn find_membership_record(
        &self,
        email: &str,
    ) -> Result<Option<MembershipRecord>, SharedDbError> {
        let row = sqlx::query(
            "SELECT activated_at, active_until, grace_until, override_status
             FROM membership_entitlements
             WHERE lower(user_email) = lower($1)",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        row.map(|row| {
            let override_status: Option<String> =
                row.try_get("override_status").map_err(SharedDbError::from)?;
            Ok(MembershipRecord {
                activated_at: row.try_get("activated_at").map_err(SharedDbError::from)?,
                active_until: row.try_get("active_until").map_err(SharedDbError::from)?,
                grace_until: row.try_get("grace_until").map_err(SharedDbError::from)?,
                override_status: override_status
                    .as_deref()
                    .map(parse_membership_status)
                    .transpose()?,
            })
        })
        .transpose()
    }

    pub async fn upsert_membership_record(
        &self,
        email: &str,
        record: &MembershipRecord,
    ) -> Result<(), SharedDbError> {
        sqlx::query(
            "INSERT INTO membership_entitlements (
                user_email,
                activated_at,
                active_until,
                grace_until,
                override_status,
                updated_at
             ) VALUES ($1, $2, $3, $4, $5, now())
             ON CONFLICT (user_email) DO UPDATE
             SET activated_at = excluded.activated_at,
                 active_until = excluded.active_until,
                 grace_until = excluded.grace_until,
                 override_status = excluded.override_status,
                 updated_at = now()",
        )
        .bind(email)
        .bind(record.activated_at)
        .bind(record.active_until)
        .bind(record.grace_until)
        .bind(record.override_status.as_ref().map(membership_status_to_str))
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn update_membership_override(
        &self,
        email: &str,
        override_status: Option<&MembershipStatus>,
    ) -> Result<(), SharedDbError> {
        sqlx::query(
            "INSERT INTO membership_entitlements (
                user_email,
                override_status,
                updated_at
             ) VALUES ($1, $2, now())
             ON CONFLICT (user_email) DO UPDATE
             SET override_status = excluded.override_status,
                 updated_at = now()",
        )
        .bind(email)
        .bind(override_status.map(membership_status_to_str))
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn apply_payment(
        &self,
        order_id: u64,
        chain: &str,
        tx_hash: &str,
        paid_at: DateTime<Utc>,
        email: &str,
        active_until: DateTime<Utc>,
        grace_until: DateTime<Utc>,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        sqlx::query(
            "UPDATE membership_orders
             SET paid_at = $2,
                 tx_hash = $3,
                 status = 'paid',
                 updated_at = now()
             WHERE order_id = $1",
        )
        .bind(order_id as i64)
        .bind(paid_at)
        .bind(tx_hash)
        .execute(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?;

        sqlx::query("DELETE FROM deposit_order_queue WHERE order_id = $1")
            .bind(order_id as i64)
            .execute(&mut *transaction)
            .await
            .map_err(SharedDbError::from)?;

        sqlx::query(
            "UPDATE deposit_address_allocations
             SET released_at = $2
             WHERE order_id = $1
               AND released_at IS NULL",
        )
        .bind(order_id as i64)
        .bind(paid_at)
        .execute(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?;

        sqlx::query(
            "INSERT INTO membership_entitlements (
                user_email,
                source_order_id,
                activated_at,
                active_until,
                grace_until,
                updated_at
             ) VALUES ($1, $2, $3, $4, $5, now())
             ON CONFLICT (user_email) DO UPDATE
             SET source_order_id = excluded.source_order_id,
                 activated_at = excluded.activated_at,
                 active_until = excluded.active_until,
                 grace_until = excluded.grace_until,
                 updated_at = now()",
        )
        .bind(email)
        .bind(order_id as i64)
        .bind(paid_at)
        .bind(active_until)
        .bind(grace_until)
        .execute(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?;

        sqlx::query(
            "UPDATE deposit_transactions
             SET order_id = $3,
                 status = 'matched',
                 updated_at = now()
             WHERE chain = $1 AND tx_hash = $2",
        )
        .bind(chain)
        .bind(tx_hash)
        .bind(order_id as i64)
        .execute(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?;

        transaction.commit().await.map_err(SharedDbError::from)
    }

    pub async fn list_membership_plans(&self) -> Result<Vec<MembershipPlanRecord>, SharedDbError> {
        let rows = sqlx::query(
            "SELECT code, name, duration_days, is_active
             FROM membership_plans
             ORDER BY code ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        rows.into_iter()
            .map(|row| {
                Ok(MembershipPlanRecord {
                    code: row.try_get("code").map_err(SharedDbError::from)?,
                    name: row.try_get("name").map_err(SharedDbError::from)?,
                    duration_days: row.try_get("duration_days").map_err(SharedDbError::from)?,
                    is_active: row.try_get("is_active").map_err(SharedDbError::from)?,
                })
            })
            .collect()
    }

    pub async fn upsert_membership_plan(
        &self,
        plan: &MembershipPlanRecord,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        upsert_membership_plan_in(&mut transaction, plan).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn upsert_membership_plan_with_prices(
        &self,
        plan: &MembershipPlanRecord,
        prices: &[MembershipPlanPriceRecord],
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        upsert_membership_plan_in(&mut transaction, plan).await?;
        for price in prices {
            upsert_plan_price_in(&mut transaction, price).await?;
        }
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn list_plan_prices(
        &self,
    ) -> Result<Vec<MembershipPlanPriceRecord>, SharedDbError> {
        let rows = sqlx::query(
            "SELECT plan_code, chain, asset, amount
             FROM membership_plan_prices
             ORDER BY plan_code ASC, chain ASC, asset ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        rows.into_iter()
            .map(|row| {
                Ok(MembershipPlanPriceRecord {
                    plan_code: row.try_get("plan_code").map_err(SharedDbError::from)?,
                    chain: row.try_get("chain").map_err(SharedDbError::from)?,
                    asset: row.try_get("asset").map_err(SharedDbError::from)?,
                    amount: row.try_get("amount").map_err(SharedDbError::from)?,
                })
            })
            .collect()
    }

    pub async fn upsert_plan_price(
        &self,
        price: &MembershipPlanPriceRecord,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        upsert_plan_price_in(&mut transaction, price).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn list_deposit_addresses(
        &self,
    ) -> Result<Vec<DepositAddressPoolRecord>, SharedDbError> {
        let rows = sqlx::query(
            "SELECT chain, address, is_enabled
             FROM deposit_address_pool
             ORDER BY chain ASC, address ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        rows.into_iter()
            .map(|row| {
                Ok(DepositAddressPoolRecord {
                    chain: row.try_get("chain").map_err(SharedDbError::from)?,
                    address: row.try_get("address").map_err(SharedDbError::from)?,
                    is_enabled: row.try_get("is_enabled").map_err(SharedDbError::from)?,
                })
            })
            .collect()
    }

    pub async fn upsert_deposit_address(
        &self,
        address: &DepositAddressPoolRecord,
    ) -> Result<(), SharedDbError> {
        sqlx::query(
            "INSERT INTO deposit_address_pool (chain, address, is_enabled, created_at, updated_at)
             VALUES ($1, $2, $3, now(), now())
             ON CONFLICT (chain, address) DO UPDATE
             SET is_enabled = excluded.is_enabled,
                 updated_at = now()",
        )
        .bind(&address.chain)
        .bind(&address.address)
        .bind(address.is_enabled)
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn create_sweep_job(
        &self,
        job: &SweepJobRecord,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        sqlx::query(
            "INSERT INTO fund_sweep_jobs (
                sweep_job_id,
                chain,
                asset,
                status,
                requested_by,
                requested_at,
                completed_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(job.sweep_job_id as i64)
        .bind(&job.chain)
        .bind(&job.asset)
        .bind(&job.status)
        .bind(&job.requested_by)
        .bind(job.requested_at)
        .bind(job.completed_at)
        .execute(&mut *transaction)
        .await
        .map_err(SharedDbError::from)?;

        for transfer in &job.transfers {
            sqlx::query(
                "INSERT INTO fund_sweep_transfers (
                    sweep_job_id,
                    from_address,
                    to_address,
                    amount,
                    tx_hash,
                    created_at
                 ) VALUES ($1, $2, $3, $4, $5, now())",
            )
            .bind(job.sweep_job_id as i64)
            .bind(&transfer.from_address)
            .bind(&transfer.to_address)
            .bind(&transfer.amount)
            .bind(&transfer.tx_hash)
            .execute(&mut *transaction)
            .await
            .map_err(SharedDbError::from)?;
        }

        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn list_sweep_jobs(&self) -> Result<Vec<SweepJobRecord>, SharedDbError> {
        let jobs = sqlx::query(
            "SELECT sweep_job_id, chain, asset, status, requested_by, requested_at, completed_at
             FROM fund_sweep_jobs
             ORDER BY requested_at ASC, sweep_job_id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        let transfers = sqlx::query(
            "SELECT sweep_job_id, from_address, to_address, amount, tx_hash
             FROM fund_sweep_transfers
             ORDER BY sweep_job_id ASC, transfer_id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        let mut grouped: HashMap<u64, Vec<SweepTransferRecord>> = HashMap::new();
        for row in transfers {
            let sweep_job_id = row
                .try_get::<i64, _>("sweep_job_id")
                .map_err(SharedDbError::from)? as u64;
            grouped.entry(sweep_job_id).or_default().push(SweepTransferRecord {
                from_address: row.try_get("from_address").map_err(SharedDbError::from)?,
                to_address: row.try_get("to_address").map_err(SharedDbError::from)?,
                amount: row.try_get("amount").map_err(SharedDbError::from)?,
                tx_hash: row.try_get("tx_hash").map_err(SharedDbError::from)?,
            });
        }

        jobs.into_iter()
            .map(|row| {
                let sweep_job_id = row
                    .try_get::<i64, _>("sweep_job_id")
                    .map_err(SharedDbError::from)? as u64;
                Ok(SweepJobRecord {
                    sweep_job_id,
                    chain: row.try_get("chain").map_err(SharedDbError::from)?,
                    asset: row.try_get("asset").map_err(SharedDbError::from)?,
                    status: row.try_get("status").map_err(SharedDbError::from)?,
                    requested_by: row.try_get("requested_by").map_err(SharedDbError::from)?,
                    requested_at: row.try_get("requested_at").map_err(SharedDbError::from)?,
                    completed_at: row.try_get("completed_at").map_err(SharedDbError::from)?,
                    transfers: grouped.remove(&sweep_job_id).unwrap_or_default(),
                })
            })
            .collect()
    }
}

async fn write_order_meta(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    order_id: u64,
    config_value: Value,
) -> Result<(), SharedDbError> {
    sqlx::query(
        "INSERT INTO system_configs (config_key, config_value, updated_at)
         VALUES ($1, $2, now())
         ON CONFLICT (config_key) DO UPDATE
         SET config_value = excluded.config_value,
             updated_at = now()",
    )
    .bind(order_meta_key(order_id))
    .bind(config_value)
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;
    Ok(())
}

async fn release_expired_allocations_in(
    transaction: &mut Transaction<'_, Postgres>,
    chain: &str,
    released_at: DateTime<Utc>,
) -> Result<(), SharedDbError> {
    sqlx::query(
        "UPDATE deposit_address_allocations
         SET released_at = $2
         WHERE chain = $1
           AND released_at IS NULL
           AND expires_at <= $2",
    )
    .bind(chain)
    .bind(released_at)
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;
    Ok(())
}

async fn upsert_allocation_in(
    transaction: &mut Transaction<'_, Postgres>,
    order_id: u64,
    assignment: &AddressAssignment,
) -> Result<(), SharedDbError> {
    sqlx::query(
        "INSERT INTO deposit_address_allocations (
            order_id,
            chain,
            address,
            expires_at,
            released_at,
            created_at
         ) VALUES ($1, $2, $3, $4, NULL, now())
         ON CONFLICT (order_id) DO UPDATE
         SET chain = excluded.chain,
             address = excluded.address,
             expires_at = excluded.expires_at,
             released_at = NULL",
    )
    .bind(order_id as i64)
    .bind(&assignment.chain)
    .bind(&assignment.address)
    .bind(assignment.expires_at)
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;
    Ok(())
}

async fn upsert_membership_plan_in(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &MembershipPlanRecord,
) -> Result<(), SharedDbError> {
    sqlx::query(
        "INSERT INTO membership_plans (code, name, duration_days, is_active, created_at, updated_at)
         VALUES ($1, $2, $3, $4, now(), now())
         ON CONFLICT (code) DO UPDATE
         SET name = excluded.name,
             duration_days = excluded.duration_days,
             is_active = excluded.is_active,
             updated_at = now()",
    )
    .bind(&plan.code)
    .bind(&plan.name)
    .bind(plan.duration_days)
    .bind(plan.is_active)
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;
    Ok(())
}

async fn upsert_plan_price_in(
    transaction: &mut Transaction<'_, Postgres>,
    price: &MembershipPlanPriceRecord,
) -> Result<(), SharedDbError> {
    sqlx::query(
        "INSERT INTO membership_plan_prices (plan_code, chain, asset, amount, created_at, updated_at)
         VALUES ($1, $2, $3, $4, now(), now())
         ON CONFLICT (plan_code, chain, asset) DO UPDATE
         SET amount = excluded.amount,
             updated_at = now()",
    )
    .bind(&price.plan_code)
    .bind(&price.chain)
    .bind(&price.asset)
    .bind(&price.amount)
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;
    Ok(())
}

fn order_meta_key(order_id: u64) -> String {
    format!("billing.order.{order_id}.meta")
}

fn billing_order_from_row(row: sqlx::postgres::PgRow) -> Result<BillingOrderRecord, SharedDbError> {
    let assigned_address: String = row.try_get("assigned_address").map_err(SharedDbError::from)?;
    let assignment = if assigned_address.is_empty() {
        None
    } else {
        Some(AddressAssignment {
            chain: row.try_get("chain").map_err(SharedDbError::from)?,
            address: assigned_address,
            expires_at: row.try_get("address_expires_at").map_err(SharedDbError::from)?,
        })
    };

    Ok(BillingOrderRecord {
        order_id: row
            .try_get::<i64, _>("order_id")
            .map_err(SharedDbError::from)? as u64,
        email: row.try_get("user_email").map_err(SharedDbError::from)?,
        chain: row.try_get("chain").map_err(SharedDbError::from)?,
        asset: row.try_get("asset").map_err(SharedDbError::from)?,
        plan_code: row.try_get("plan_code").map_err(SharedDbError::from)?,
        amount: row.try_get("amount").map_err(SharedDbError::from)?,
        requested_at: row.try_get("requested_at").map_err(SharedDbError::from)?,
        assignment,
        paid_at: row.try_get("paid_at").map_err(SharedDbError::from)?,
        tx_hash: row.try_get("tx_hash").map_err(SharedDbError::from)?,
        status: row.try_get("status").map_err(SharedDbError::from)?,
        enqueued_at: row.try_get("enqueued_at").map_err(SharedDbError::from)?,
    })
}

fn deposit_payload(record: &DepositTransactionRecord) -> Value {
    json!({
        "asset": record.asset,
        "address": record.address,
        "amount": record.amount,
        "review_reason": record.review_reason,
        "processed_at": record.processed_at,
        "matched_order_id": record.matched_order_id,
    })
}

fn deposit_from_row(row: sqlx::postgres::PgRow) -> Result<DepositTransactionRecord, SharedDbError> {
    let payload: Value = row.try_get("raw_payload").map_err(SharedDbError::from)?;

    Ok(DepositTransactionRecord {
        tx_hash: row.try_get("tx_hash").map_err(SharedDbError::from)?,
        chain: row.try_get("chain").map_err(SharedDbError::from)?,
        asset: payload
            .get("asset")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        address: payload
            .get("address")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        amount: payload
            .get("amount")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        observed_at: row.try_get("observed_at").map_err(SharedDbError::from)?,
        order_id: row
            .try_get::<Option<i64>, _>("order_id")
            .map_err(SharedDbError::from)?
            .map(|value| value as u64),
        status: row.try_get("status").map_err(SharedDbError::from)?,
        review_reason: payload
            .get("review_reason")
            .and_then(Value::as_str)
            .map(str::to_owned),
        processed_at: payload
            .get("processed_at")
            .and_then(Value::as_str)
            .and_then(|value| value.parse().ok()),
        matched_order_id: payload
            .get("matched_order_id")
            .and_then(Value::as_u64),
    })
}
