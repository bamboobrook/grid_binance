use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};

use shared_chain::assignment::AddressAssignment;
use shared_domain::membership::MembershipStatus;

use crate::{membership_status_to_str, parse_membership_status, SharedDbError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingOrderRecord {
    pub order_id: u64,
    pub email: String,
    pub chain: String,
    pub plan_code: String,
    pub amount: String,
    pub requested_at: DateTime<Utc>,
    pub assignment: AddressAssignment,
    pub paid_at: Option<DateTime<Utc>>,
    pub tx_hash: Option<String>,
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
            "SELECT order_id,
                    user_email,
                    chain,
                    plan_code,
                    amount,
                    requested_at,
                    assigned_address,
                    address_expires_at,
                    paid_at,
                    tx_hash
             FROM membership_orders
             ORDER BY order_id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        rows.into_iter().map(billing_order_from_row).collect()
    }

    pub async fn insert_order(&self, order: &BillingOrderRecord) -> Result<(), SharedDbError> {
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
        .bind(&order.assignment.address)
        .bind(order.assignment.expires_at)
        .bind(order.paid_at)
        .bind(&order.tx_hash)
        .bind(if order.paid_at.is_some() { "paid" } else { "pending" })
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn record_seen_transfer(
        &self,
        tx_hash: &str,
        chain: &str,
        observed_at: DateTime<Utc>,
    ) -> Result<bool, SharedDbError> {
        let inserted = sqlx::query(
            "INSERT INTO deposit_transactions (tx_hash, chain, observed_at, status, raw_payload, created_at, updated_at)
             VALUES ($1, $2, $3, 'observed', '{}'::jsonb, now(), now())
             ON CONFLICT (tx_hash) DO NOTHING",
        )
        .bind(tx_hash)
        .bind(chain)
        .bind(observed_at)
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(inserted.rows_affected() == 1)
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
        transaction.commit().await.map_err(SharedDbError::from)
    }

    pub async fn upsert_membership_plan(
        &self,
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
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn upsert_plan_price(
        &self,
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
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn upsert_deposit_address(&self, address: &DepositAddressPoolRecord) -> Result<(), SharedDbError> {
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
}

fn billing_order_from_row(row: sqlx::postgres::PgRow) -> Result<BillingOrderRecord, SharedDbError> {
    let chain: String = row.try_get("chain").map_err(SharedDbError::from)?;
    let address: String = row.try_get("assigned_address").map_err(SharedDbError::from)?;

    Ok(BillingOrderRecord {
        order_id: row.try_get::<i64, _>("order_id").map_err(SharedDbError::from)? as u64,
        email: row.try_get("user_email").map_err(SharedDbError::from)?,
        chain: chain.clone(),
        plan_code: row.try_get("plan_code").map_err(SharedDbError::from)?,
        amount: row.try_get("amount").map_err(SharedDbError::from)?,
        requested_at: row.try_get("requested_at").map_err(SharedDbError::from)?,
        assignment: AddressAssignment {
            chain,
            address,
            expires_at: row.try_get("address_expires_at").map_err(SharedDbError::from)?,
        },
        paid_at: row.try_get("paid_at").map_err(SharedDbError::from)?,
        tx_hash: row.try_get("tx_hash").map_err(SharedDbError::from)?,
    })
}
