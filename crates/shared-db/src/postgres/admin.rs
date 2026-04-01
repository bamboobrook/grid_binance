use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Row};

use shared_domain::strategy::StrategyTemplate;

use crate::{SharedDbError, StoredStrategyTemplate};

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
                    budget,
                    grid_spacing_bps,
                    membership_ready,
                    exchange_ready,
                    symbol_ready
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
                    budget,
                    grid_spacing_bps,
                    membership_ready,
                    exchange_ready,
                    symbol_ready
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

    pub async fn insert_template(&self, template: &StoredStrategyTemplate) -> Result<(), SharedDbError> {
        sqlx::query(
            "INSERT INTO strategy_templates (
                id,
                sequence_id,
                name,
                symbol,
                budget,
                grid_spacing_bps,
                membership_ready,
                exchange_ready,
                symbol_ready,
                created_at,
                updated_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, now(), now())",
        )
        .bind(&template.template.id)
        .bind(template.sequence_id as i64)
        .bind(&template.template.name)
        .bind(&template.template.symbol)
        .bind(&template.template.budget)
        .bind(template.template.grid_spacing_bps as i32)
        .bind(template.template.membership_ready)
        .bind(template.template.exchange_ready)
        .bind(template.template.symbol_ready)
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn insert_audit_log(&self, record: &AuditLogRecord) -> Result<(), SharedDbError> {
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
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn upsert_system_config(&self, record: &SystemConfigRecord) -> Result<(), SharedDbError> {
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
        .execute(&self.pool)
        .await
        .map_err(SharedDbError::from)?;
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

fn template_from_row(row: sqlx::postgres::PgRow) -> Result<StrategyTemplate, SharedDbError> {
    Ok(StrategyTemplate {
        id: row.try_get("id").map_err(SharedDbError::from)?,
        name: row.try_get("name").map_err(SharedDbError::from)?,
        symbol: row.try_get("symbol").map_err(SharedDbError::from)?,
        budget: row.try_get("budget").map_err(SharedDbError::from)?,
        grid_spacing_bps: row.try_get::<i32, _>("grid_spacing_bps").map_err(SharedDbError::from)? as u32,
        membership_ready: row.try_get("membership_ready").map_err(SharedDbError::from)?,
        exchange_ready: row.try_get("exchange_ready").map_err(SharedDbError::from)?,
        symbol_ready: row.try_get("symbol_ready").map_err(SharedDbError::from)?,
    })
}
