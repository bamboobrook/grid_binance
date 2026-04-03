use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Row, Transaction};

use shared_domain::{
    membership::MembershipStatus,
    strategy::{
        GridGeneration, GridLevel, PostTriggerAction, StrategyMarket, StrategyMode,
        StrategyTemplate,
    },
};

use crate::{
    membership_status_to_str, DepositAddressPoolRecord, DepositTransactionRecord,
    MembershipPlanPriceRecord, MembershipPlanRecord, MembershipRecord, SharedDbError,
    StoredStrategyTemplate, SweepJobRecord,
};

#[derive(Debug, Clone, PartialEq)]
pub struct AuditLogRecord {
    pub actor_email: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemConfigRecord {
    pub config_key: String,
    pub config_value: Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct AdminRepository {
    pool: PgPool,
}

impl AdminRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_templates(&self) -> Result<Vec<StrategyTemplate>, SharedDbError> {
        let rows = sqlx::query(
            "SELECT id,
                    name,
                    symbol,
                    market,
                    mode,
                    generation,
                    levels,
                    budget,
                    grid_spacing_bps,
                    membership_ready,
                    exchange_ready,
                    permissions_ready,
                    withdrawals_disabled,
                    hedge_mode_ready,
                    symbol_ready,
                    filters_ready,
                    margin_ready,
                    conflict_ready,
                    balance_ready,
                    overall_take_profit_bps,
                    overall_stop_loss_bps,
                    post_trigger_action
             FROM strategy_templates
             ORDER BY sequence_id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        rows.into_iter().map(template_from_row).collect()
    }

    pub async fn find_template(
        &self,
        template_id: &str,
    ) -> Result<Option<StrategyTemplate>, SharedDbError> {
        sqlx::query(
            "SELECT id,
                    name,
                    symbol,
                    market,
                    mode,
                    generation,
                    levels,
                    budget,
                    grid_spacing_bps,
                    membership_ready,
                    exchange_ready,
                    permissions_ready,
                    withdrawals_disabled,
                    hedge_mode_ready,
                    symbol_ready,
                    filters_ready,
                    margin_ready,
                    conflict_ready,
                    balance_ready,
                    overall_take_profit_bps,
                    overall_stop_loss_bps,
                    post_trigger_action
             FROM strategy_templates
             WHERE id = $1",
        )
        .bind(template_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(SharedDbError::from)?
        .map(template_from_row)
        .transpose()
    }

    pub async fn insert_template(
        &self,
        template: &StoredStrategyTemplate,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        insert_template_in(&mut transaction, template).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn insert_template_with_audit(
        &self,
        template: &StoredStrategyTemplate,
        audit: &AuditLogRecord,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        insert_template_in(&mut transaction, template).await?;
        insert_audit_log_in(&mut transaction, audit).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn update_template_with_audit(
        &self,
        template: &StrategyTemplate,
        audit: &AuditLogRecord,
    ) -> Result<usize, SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        let updated = update_template_in(&mut transaction, template).await?;
        if updated == 0 {
            transaction.rollback().await.map_err(SharedDbError::from)?;
            return Ok(0);
        }
        insert_audit_log_in(&mut transaction, audit).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(updated)
    }

    pub async fn insert_audit_log(&self, record: &AuditLogRecord) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        insert_audit_log_in(&mut transaction, record).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn list_audit_logs(&self) -> Result<Vec<AuditLogRecord>, SharedDbError> {
        let rows = sqlx::query(
            "SELECT actor_email, action, target_type, target_id, payload, created_at
             FROM audit_logs
             ORDER BY created_at DESC, audit_id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SharedDbError::from)?;

        rows.into_iter()
            .map(|row| {
                Ok(AuditLogRecord {
                    actor_email: row.try_get("actor_email").map_err(SharedDbError::from)?,
                    action: row.try_get("action").map_err(SharedDbError::from)?,
                    target_type: row.try_get("target_type").map_err(SharedDbError::from)?,
                    target_id: row.try_get("target_id").map_err(SharedDbError::from)?,
                    payload: row.try_get("payload").map_err(SharedDbError::from)?,
                    created_at: row.try_get("created_at").map_err(SharedDbError::from)?,
                })
            })
            .collect()
    }

    pub async fn upsert_system_config(
        &self,
        record: &SystemConfigRecord,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        upsert_system_config_in(&mut transaction, record).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn upsert_system_configs_with_audit(
        &self,
        records: &[SystemConfigRecord],
        audit: &AuditLogRecord,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        for record in records {
            upsert_system_config_in(&mut transaction, record).await?;
        }
        insert_audit_log_in(&mut transaction, audit).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn upsert_membership_plan_with_prices_and_audit(
        &self,
        plan: &MembershipPlanRecord,
        prices: &[MembershipPlanPriceRecord],
        audit: &AuditLogRecord,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        upsert_membership_plan_in(&mut transaction, plan).await?;
        for price in prices {
            upsert_plan_price_in(&mut transaction, price).await?;
        }
        insert_audit_log_in(&mut transaction, audit).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn upsert_deposit_address_with_audit(
        &self,
        address: &DepositAddressPoolRecord,
        audit: &AuditLogRecord,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        upsert_deposit_address_in(&mut transaction, address).await?;
        insert_audit_log_in(&mut transaction, audit).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn upsert_membership_record_with_audit(
        &self,
        email: &str,
        record: &MembershipRecord,
        audit: &AuditLogRecord,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        upsert_membership_record_in(&mut transaction, email, record).await?;
        insert_audit_log_in(&mut transaction, audit).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn update_membership_override_with_audit(
        &self,
        email: &str,
        override_status: Option<&MembershipStatus>,
        audit: &AuditLogRecord,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        update_membership_override_in(&mut transaction, email, override_status).await?;
        insert_audit_log_in(&mut transaction, audit).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn upsert_deposit_transaction_with_audit(
        &self,
        record: &DepositTransactionRecord,
        audit: &AuditLogRecord,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        upsert_deposit_transaction_in(&mut transaction, record).await?;
        insert_audit_log_in(&mut transaction, audit).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn create_sweep_job_with_audit(
        &self,
        job: &SweepJobRecord,
        audit: &AuditLogRecord,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        create_sweep_job_in(&mut transaction, job).await?;
        insert_audit_log_in(&mut transaction, audit).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn apply_membership_payment_with_audit(
        &self,
        order_id: u64,
        chain: &str,
        tx_hash: &str,
        paid_at: DateTime<Utc>,
        email: &str,
        active_until: DateTime<Utc>,
        grace_until: DateTime<Utc>,
        audit: &AuditLogRecord,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        apply_membership_payment_in(
            &mut transaction,
            order_id,
            chain,
            tx_hash,
            paid_at,
            email,
            active_until,
            grace_until,
        )
        .await?;
        insert_audit_log_in(&mut transaction, audit).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn apply_membership_payment_with_deposit_and_audit(
        &self,
        order_id: u64,
        chain: &str,
        tx_hash: &str,
        paid_at: DateTime<Utc>,
        email: &str,
        active_until: DateTime<Utc>,
        grace_until: DateTime<Utc>,
        deposit: &DepositTransactionRecord,
        audit: &AuditLogRecord,
    ) -> Result<(), SharedDbError> {
        let mut transaction = self.pool.begin().await.map_err(SharedDbError::from)?;
        apply_membership_payment_in(
            &mut transaction,
            order_id,
            chain,
            tx_hash,
            paid_at,
            email,
            active_until,
            grace_until,
        )
        .await?;
        upsert_deposit_transaction_in(&mut transaction, deposit).await?;
        insert_audit_log_in(&mut transaction, audit).await?;
        transaction.commit().await.map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn get_system_config(
        &self,
        config_key: &str,
    ) -> Result<Option<SystemConfigRecord>, SharedDbError> {
        sqlx::query(
            "SELECT config_key, config_value, updated_at
             FROM system_configs
             WHERE config_key = $1",
        )
        .bind(config_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(SharedDbError::from)?
        .map(|row| {
            Ok(SystemConfigRecord {
                config_key: row.try_get("config_key").map_err(SharedDbError::from)?,
                config_value: row.try_get("config_value").map_err(SharedDbError::from)?,
                updated_at: row.try_get("updated_at").map_err(SharedDbError::from)?,
            })
        })
        .transpose()
    }
}

pub(crate) async fn insert_audit_log_in(
    transaction: &mut Transaction<'_, Postgres>,
    record: &AuditLogRecord,
) -> Result<(), SharedDbError> {
    sqlx::query(
        "INSERT INTO audit_logs (
            actor_email,
            action,
            target_type,
            target_id,
            payload,
            created_at
         ) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&record.actor_email)
    .bind(&record.action)
    .bind(&record.target_type)
    .bind(&record.target_id)
    .bind(&record.payload)
    .bind(record.created_at)
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;
    Ok(())
}

async fn insert_template_in(
    transaction: &mut Transaction<'_, Postgres>,
    template: &StoredStrategyTemplate,
) -> Result<(), SharedDbError> {
    let levels = serde_json::to_value(&template.template.levels)
        .map_err(|error| SharedDbError::new(error.to_string()))?;
    sqlx::query(
        "INSERT INTO strategy_templates (
            id,
            sequence_id,
            name,
            symbol,
            market,
            mode,
            generation,
            levels,
            budget,
            grid_spacing_bps,
            membership_ready,
            exchange_ready,
            permissions_ready,
            withdrawals_disabled,
            hedge_mode_ready,
            symbol_ready,
            filters_ready,
            margin_ready,
            conflict_ready,
            balance_ready,
            overall_take_profit_bps,
            overall_stop_loss_bps,
            post_trigger_action,
            created_at,
            updated_at
         ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
            $21, $22, $23, now(), now()
         )",
    )
    .bind(&template.template.id)
    .bind(template.sequence_id as i64)
    .bind(&template.template.name)
    .bind(&template.template.symbol)
    .bind(strategy_market_to_str(template.template.market))
    .bind(strategy_mode_to_str(template.template.mode))
    .bind(grid_generation_to_str(template.template.generation))
    .bind(levels)
    .bind(&template.template.budget)
    .bind(template.template.grid_spacing_bps as i32)
    .bind(template.template.membership_ready)
    .bind(template.template.exchange_ready)
    .bind(template.template.permissions_ready)
    .bind(template.template.withdrawals_disabled)
    .bind(template.template.hedge_mode_ready)
    .bind(template.template.symbol_ready)
    .bind(template.template.filters_ready)
    .bind(template.template.margin_ready)
    .bind(template.template.conflict_ready)
    .bind(template.template.balance_ready)
    .bind(
        template
            .template
            .overall_take_profit_bps
            .map(|value| value as i32),
    )
    .bind(
        template
            .template
            .overall_stop_loss_bps
            .map(|value| value as i32),
    )
    .bind(post_trigger_action_to_str(
        template.template.post_trigger_action,
    ))
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;
    Ok(())
}

async fn update_template_in(
    transaction: &mut Transaction<'_, Postgres>,
    template: &StrategyTemplate,
) -> Result<usize, SharedDbError> {
    let levels = serde_json::to_value(&template.levels)
        .map_err(|error| SharedDbError::new(error.to_string()))?;
    let updated = sqlx::query(
        "UPDATE strategy_templates
         SET name = $2,
             symbol = $3,
             market = $4,
             mode = $5,
             generation = $6,
             levels = $7,
             budget = $8,
             grid_spacing_bps = $9,
             membership_ready = $10,
             exchange_ready = $11,
             permissions_ready = $12,
             withdrawals_disabled = $13,
             hedge_mode_ready = $14,
             symbol_ready = $15,
             filters_ready = $16,
             margin_ready = $17,
             conflict_ready = $18,
             balance_ready = $19,
             overall_take_profit_bps = $20,
             overall_stop_loss_bps = $21,
             post_trigger_action = $22,
             updated_at = now()
         WHERE id = $1",
    )
    .bind(&template.id)
    .bind(&template.name)
    .bind(&template.symbol)
    .bind(strategy_market_to_str(template.market))
    .bind(strategy_mode_to_str(template.mode))
    .bind(grid_generation_to_str(template.generation))
    .bind(levels)
    .bind(&template.budget)
    .bind(template.grid_spacing_bps as i32)
    .bind(template.membership_ready)
    .bind(template.exchange_ready)
    .bind(template.permissions_ready)
    .bind(template.withdrawals_disabled)
    .bind(template.hedge_mode_ready)
    .bind(template.symbol_ready)
    .bind(template.filters_ready)
    .bind(template.margin_ready)
    .bind(template.conflict_ready)
    .bind(template.balance_ready)
    .bind(template.overall_take_profit_bps.map(|value| value as i32))
    .bind(template.overall_stop_loss_bps.map(|value| value as i32))
    .bind(post_trigger_action_to_str(template.post_trigger_action))
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;
    Ok(updated.rows_affected() as usize)
}

async fn upsert_system_config_in(
    transaction: &mut Transaction<'_, Postgres>,
    record: &SystemConfigRecord,
) -> Result<(), SharedDbError> {
    sqlx::query(
        "INSERT INTO system_configs (config_key, config_value, updated_at)
         VALUES ($1, $2, $3)
         ON CONFLICT (config_key) DO UPDATE
         SET config_value = excluded.config_value,
             updated_at = excluded.updated_at",
    )
    .bind(&record.config_key)
    .bind(&record.config_value)
    .bind(record.updated_at)
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
        "INSERT INTO membership_plan_prices (
            plan_code,
            chain,
            asset,
            amount,
            created_at,
            updated_at
         ) VALUES ($1, $2, $3, $4, now(), now())
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

async fn upsert_deposit_address_in(
    transaction: &mut Transaction<'_, Postgres>,
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
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;
    Ok(())
}

async fn upsert_deposit_transaction_in(
    transaction: &mut Transaction<'_, Postgres>,
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
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;
    Ok(())
}

async fn upsert_membership_record_in(
    transaction: &mut Transaction<'_, Postgres>,
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
    .bind(
        record
            .override_status
            .as_ref()
            .map(membership_status_to_str),
    )
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;
    Ok(())
}

async fn update_membership_override_in(
    transaction: &mut Transaction<'_, Postgres>,
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
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;
    Ok(())
}

async fn create_sweep_job_in(
    transaction: &mut Transaction<'_, Postgres>,
    job: &SweepJobRecord,
) -> Result<(), SharedDbError> {
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
    .execute(&mut **transaction)
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
        .execute(&mut **transaction)
        .await
        .map_err(SharedDbError::from)?;
    }

    Ok(())
}

async fn apply_membership_payment_in(
    transaction: &mut Transaction<'_, Postgres>,
    order_id: u64,
    chain: &str,
    tx_hash: &str,
    paid_at: DateTime<Utc>,
    email: &str,
    active_until: DateTime<Utc>,
    grace_until: DateTime<Utc>,
) -> Result<(), SharedDbError> {
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
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;

    sqlx::query("DELETE FROM deposit_order_queue WHERE order_id = $1")
        .bind(order_id as i64)
        .execute(&mut **transaction)
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
    .execute(&mut **transaction)
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
    .execute(&mut **transaction)
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
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;

    Ok(())
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

fn template_from_row(row: sqlx::postgres::PgRow) -> Result<StrategyTemplate, SharedDbError> {
    let levels_value: Value = row.try_get("levels").map_err(SharedDbError::from)?;
    let levels: Vec<GridLevel> = serde_json::from_value(levels_value)
        .map_err(|error| SharedDbError::new(error.to_string()))?;

    Ok(StrategyTemplate {
        id: row.try_get("id").map_err(SharedDbError::from)?,
        name: row.try_get("name").map_err(SharedDbError::from)?,
        symbol: row.try_get("symbol").map_err(SharedDbError::from)?,
        market: parse_strategy_market(
            &row.try_get::<String, _>("market")
                .map_err(SharedDbError::from)?,
        )?,
        mode: parse_strategy_mode(
            &row.try_get::<String, _>("mode")
                .map_err(SharedDbError::from)?,
        )?,
        generation: parse_grid_generation(
            &row.try_get::<String, _>("generation")
                .map_err(SharedDbError::from)?,
        )?,
        levels,
        budget: row.try_get("budget").map_err(SharedDbError::from)?,
        grid_spacing_bps: row
            .try_get::<i32, _>("grid_spacing_bps")
            .map_err(SharedDbError::from)? as u32,
        membership_ready: row
            .try_get("membership_ready")
            .map_err(SharedDbError::from)?,
        exchange_ready: row.try_get("exchange_ready").map_err(SharedDbError::from)?,
        permissions_ready: row
            .try_get("permissions_ready")
            .map_err(SharedDbError::from)?,
        withdrawals_disabled: row
            .try_get("withdrawals_disabled")
            .map_err(SharedDbError::from)?,
        hedge_mode_ready: row
            .try_get("hedge_mode_ready")
            .map_err(SharedDbError::from)?,
        symbol_ready: row.try_get("symbol_ready").map_err(SharedDbError::from)?,
        filters_ready: row.try_get("filters_ready").map_err(SharedDbError::from)?,
        margin_ready: row.try_get("margin_ready").map_err(SharedDbError::from)?,
        conflict_ready: row.try_get("conflict_ready").map_err(SharedDbError::from)?,
        balance_ready: row.try_get("balance_ready").map_err(SharedDbError::from)?,
        overall_take_profit_bps: row
            .try_get::<Option<i32>, _>("overall_take_profit_bps")
            .map_err(SharedDbError::from)?
            .map(|value| value as u32),
        overall_stop_loss_bps: row
            .try_get::<Option<i32>, _>("overall_stop_loss_bps")
            .map_err(SharedDbError::from)?
            .map(|value| value as u32),
        post_trigger_action: parse_post_trigger_action(
            &row.try_get::<String, _>("post_trigger_action")
                .map_err(SharedDbError::from)?,
        )?,
    })
}

fn parse_strategy_market(value: &str) -> Result<StrategyMarket, SharedDbError> {
    match value {
        "Spot" => Ok(StrategyMarket::Spot),
        "FuturesUsdM" => Ok(StrategyMarket::FuturesUsdM),
        "FuturesCoinM" => Ok(StrategyMarket::FuturesCoinM),
        _ => Err(SharedDbError::new(format!(
            "unknown strategy market: {value}"
        ))),
    }
}

fn strategy_market_to_str(value: StrategyMarket) -> &'static str {
    match value {
        StrategyMarket::Spot => "Spot",
        StrategyMarket::FuturesUsdM => "FuturesUsdM",
        StrategyMarket::FuturesCoinM => "FuturesCoinM",
    }
}

fn parse_strategy_mode(value: &str) -> Result<StrategyMode, SharedDbError> {
    match value {
        "SpotClassic" => Ok(StrategyMode::SpotClassic),
        "SpotBuyOnly" => Ok(StrategyMode::SpotBuyOnly),
        "SpotSellOnly" => Ok(StrategyMode::SpotSellOnly),
        "FuturesLong" => Ok(StrategyMode::FuturesLong),
        "FuturesShort" => Ok(StrategyMode::FuturesShort),
        "FuturesNeutral" => Ok(StrategyMode::FuturesNeutral),
        _ => Err(SharedDbError::new(format!(
            "unknown strategy mode: {value}"
        ))),
    }
}

fn strategy_mode_to_str(value: StrategyMode) -> &'static str {
    match value {
        StrategyMode::SpotClassic => "SpotClassic",
        StrategyMode::SpotBuyOnly => "SpotBuyOnly",
        StrategyMode::SpotSellOnly => "SpotSellOnly",
        StrategyMode::FuturesLong => "FuturesLong",
        StrategyMode::FuturesShort => "FuturesShort",
        StrategyMode::FuturesNeutral => "FuturesNeutral",
    }
}

fn parse_grid_generation(value: &str) -> Result<GridGeneration, SharedDbError> {
    match value {
        "Arithmetic" => Ok(GridGeneration::Arithmetic),
        "Geometric" => Ok(GridGeneration::Geometric),
        "Custom" => Ok(GridGeneration::Custom),
        _ => Err(SharedDbError::new(format!(
            "unknown grid generation: {value}"
        ))),
    }
}

fn grid_generation_to_str(value: GridGeneration) -> &'static str {
    match value {
        GridGeneration::Arithmetic => "Arithmetic",
        GridGeneration::Geometric => "Geometric",
        GridGeneration::Custom => "Custom",
    }
}

fn parse_post_trigger_action(value: &str) -> Result<PostTriggerAction, SharedDbError> {
    match value {
        "Stop" => Ok(PostTriggerAction::Stop),
        "Rebuild" => Ok(PostTriggerAction::Rebuild),
        _ => Err(SharedDbError::new(format!(
            "unknown post trigger action: {value}"
        ))),
    }
}

fn post_trigger_action_to_str(value: PostTriggerAction) -> &'static str {
    match value {
        PostTriggerAction::Stop => "Stop",
        PostTriggerAction::Rebuild => "Rebuild",
    }
}
